//! The derived index: chunking, hashing, and a SQLite store with FTS5.
//!
//! ADR-0003 says the markdown is the source of truth and any index is a projection
//! that can be deleted and rebuilt. Everything here honours that literally: the
//! database holds nothing that is not recomputable from the files, and `kb index`
//! on an empty database produces the same result as on a warm one.
//!
//! ADR-0007 takes rusqlite as the first dependency, for FTS5 with BM25 built in and
//! for a file that stays readable with any `sqlite3` binary.

use crate::base::Base;
use rusqlite::{Connection, params};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// Roughly 512 tokens of prose. Code tokenises denser, at about 2.7 characters per
/// token against 4 for prose, so a code heavy chunk lands nearer 650 tokens. That is
/// acceptable: the cost of a slightly large chunk is context budget, and the cost of
/// a small one is a fact split across two chunks and found by neither.
const MAX_CHARS: usize = 1800;

/// Overlap between windows of an oversized section, so a fact sitting on the seam is
/// still whole in one of them.
const OVERLAP_CHARS: usize = 300;

pub struct Chunk {
    /// "Document title > Section > Subsection", embedded with the text because it
    /// carries context the chunk itself usually does not repeat.
    pub heading_path: String,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Chunking
// ---------------------------------------------------------------------------

/// Splits a markdown document at heading boundaries, then splits any section that is
/// too long into overlapping windows.
///
/// Heading boundaries are free structure: the note template already forces named
/// sections, so cutting there produces chunks that mean something on their own. That
/// was not planned when the template was written and it is the main reason this is
/// twenty lines rather than a parser.
pub fn chunk(text: &str) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut buffer: Vec<&str> = Vec::new();
    let mut in_front_matter = false;
    let mut in_fence = false;

    for (i, line) in text.lines().enumerate() {
        if i == 0 && line.trim() == "---" {
            in_front_matter = true;
            continue;
        }
        if in_front_matter {
            if line.trim() == "---" {
                in_front_matter = false;
            }
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }

        // A `#` inside a fenced block is a comment in someone's code, not a heading.
        if !in_fence {
            if let Some((level, title)) = heading(line) {
                flush(&mut buffer, &path_of(&stack), &mut out);
                stack.retain(|(l, _)| *l < level);
                stack.push((level, title));
                continue;
            }
        }

        // The keyword line belongs to the keyword scorer, and indexing it here too
        // would count the same signal twice.
        //
        // That matters more than it looks. RRF treats agreement between two lists as
        // strong evidence, and agreement is only evidence when the lists are
        // independent. If both scorers read the same words, they are one scorer
        // wearing two hats and the fusion inflates whatever they share.
        if is_keyword_line(line) || is_keyword_section(&stack) {
            continue;
        }

        buffer.push(line);
    }

    flush(&mut buffer, &path_of(&stack), &mut out);
    out
}

fn is_keyword_line(line: &str) -> bool {
    line.contains("Search for:") || line.contains("Buscar por:")
}

/// True while we are inside a `## Search for` section, whose whole body is keywords.
fn is_keyword_section(stack: &[(usize, String)]) -> bool {
    stack
        .last()
        .map(|(_, t)| {
            let t = t.to_lowercase();
            t.starts_with("search for") || t.starts_with("buscar por")
        })
        .unwrap_or(false)
}

fn heading(line: &str) -> Option<(usize, String)> {
    if !line.starts_with('#') {
        return None;
    }
    let level = line.chars().take_while(|c| *c == '#').count();
    if level > 6 {
        return None;
    }
    let rest = line[level..].strip_prefix(' ')?;
    Some((level, rest.trim().to_string()))
}

fn path_of(stack: &[(usize, String)]) -> String {
    stack
        .iter()
        .map(|(_, t)| t.as_str())
        .collect::<Vec<&str>>()
        .join(" > ")
}

fn flush(buffer: &mut Vec<&str>, heading_path: &str, out: &mut Vec<Chunk>) {
    let body = buffer.join("\n");
    buffer.clear();

    let body = body.trim();
    if body.is_empty() {
        return;
    }

    for window in windows(body) {
        out.push(Chunk {
            heading_path: heading_path.to_string(),
            text: window,
        });
    }
}

/// Splits an oversized section into overlapping windows, cutting at a blank line when
/// one is available so a window does not end mid table or mid list.
fn windows(body: &str) -> Vec<String> {
    if body.chars().count() <= MAX_CHARS {
        return vec![body.to_string()];
    }

    let chars: Vec<char> = body.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let hard_end = (start + MAX_CHARS).min(chars.len());
        let mut end = hard_end;

        if hard_end < chars.len() {
            let slice: String = chars[start..hard_end].iter().collect();
            if let Some(brk) = slice.rfind("\n\n") {
                // Only respect the break if it does not produce a stub.
                if brk > MAX_CHARS / 3 {
                    end = start + slice[..brk].chars().count();
                }
            }
        }

        out.push(chars[start..end].iter().collect::<String>().trim().to_string());

        if end >= chars.len() {
            break;
        }
        // Always advance, whatever the break logic decided, or this loops forever.
        start = (end.saturating_sub(OVERLAP_CHARS)).max(start + 1);
    }

    out.retain(|w| !w.is_empty());
    out
}

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

/// A content hash for change detection.
///
/// Deliberately not cryptographic. This answers "did this file change since the last
/// index", where the adversary is a text editor rather than a person, and SipHash
/// from `std` answers it in one line with no dependency. If the question ever becomes
/// "prove this file was not tampered with", this is the wrong function and the
/// comment should stop anyone from assuming otherwise.
pub fn content_hash(text: &str) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

pub struct SyncReport {
    pub unchanged: usize,
    pub reindexed: usize,
    pub removed: usize,
    pub chunks: usize,
}

pub struct Hit {
    pub base: String,
    pub path: String,
    pub heading_path: String,
    pub excerpt: String,
    // No score field on purpose. BM25 still decides the SQL ordering, but the fusion
    // in main.rs is Reciprocal Rank Fusion, which uses position and deliberately
    // ignores the raw value, so carrying it out of here would be a field nothing
    // reads. If fusion ever becomes a weighted sum, this is where the score comes
    // back, along with the normalisation problem that made RRF the better choice.
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> R<Store> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Store { conn };
        store.schema()?;
        Ok(store)
    }

    fn schema(&self) -> R<()> {
        // `remove_diacritics 2` folds accents inside the tokenizer, which is the same
        // job the hand written fold() does for the keyword scorer, done properly and
        // for free. Portuguese questions against an English base depend on it.
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS files (
                base        TEXT NOT NULL,
                path        TEXT NOT NULL,
                hash        TEXT NOT NULL,
                provenance  TEXT,
                stage       TEXT,
                PRIMARY KEY (base, path)
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks USING fts5(
                base         UNINDEXED,
                path         UNINDEXED,
                heading_path,
                text,
                tokenize = 'unicode61 remove_diacritics 2'
            );
            ",
        )?;
        Ok(())
    }

    /// Brings the index in line with the files, touching only what changed.
    pub fn sync(&mut self, base: &Base, base_name: &str) -> R<SyncReport> {
        let mut report = SyncReport { unchanged: 0, reindexed: 0, removed: 0, chunks: 0 };
        let tx = self.conn.transaction()?;

        let mut seen: Vec<String> = Vec::new();

        for file in &base.files {
            // The map is the keyword scorer's corpus. Indexing it here too would make
            // the two scorers read the same words and call their agreement evidence,
            // which is the independence problem again, one level up from the keyword
            // line.
            if Some(&file.rel) == base.map.as_ref() {
                continue;
            }

            seen.push(file.rel.clone());
            let hash = content_hash(&file.text);

            let known: Option<String> = tx
                .query_row(
                    "SELECT hash FROM files WHERE base = ?1 AND path = ?2",
                    params![base_name, file.rel],
                    |row| row.get(0),
                )
                .ok();

            if known.as_deref() == Some(hash.as_str()) {
                report.unchanged += 1;
                continue;
            }

            tx.execute(
                "DELETE FROM chunks WHERE base = ?1 AND path = ?2",
                params![base_name, file.rel],
            )?;

            let fm = crate::checks::front_matter(&file.text).unwrap_or_default();
            let field = |k: &str| {
                fm.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone())
            };

            tx.execute(
                "INSERT OR REPLACE INTO files (base, path, hash, provenance, stage)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![base_name, file.rel, hash, field("provenance"), field("stage")],
            )?;

            for c in chunk(&file.text) {
                tx.execute(
                    "INSERT INTO chunks (base, path, heading_path, text)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![base_name, file.rel, c.heading_path, c.text],
                )?;
                report.chunks += 1;
            }
            report.reindexed += 1;
        }

        // A file deleted on disk has to leave the index, or the router keeps offering
        // something that is not there any more.
        let mut stale: Vec<String> = Vec::new();
        {
            let mut stmt = tx.prepare("SELECT path FROM files WHERE base = ?1")?;
            let rows = stmt.query_map(params![base_name], |row| row.get::<_, String>(0))?;
            for row in rows {
                let path = row?;
                if !seen.contains(&path) {
                    stale.push(path);
                }
            }
        }
        for path in stale {
            tx.execute("DELETE FROM chunks WHERE base = ?1 AND path = ?2", params![base_name, path])?;
            tx.execute("DELETE FROM files WHERE base = ?1 AND path = ?2", params![base_name, path])?;
            report.removed += 1;
        }

        tx.commit()?;
        Ok(report)
    }

    /// Full text search, ranked by BM25.
    ///
    /// Takes terms rather than a question, so the caller expands the aliases once and
    /// both scorers see exactly the same words.
    pub fn search(&self, terms: &[String], limit: usize) -> R<Vec<Hit>> {
        let expr = match fts_query(terms) {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        let mut stmt = self.conn.prepare(
            "SELECT base, path, heading_path,
                    snippet(chunks, 3, '', '', ' ... ', 14),
                    bm25(chunks, 0.0, 0.0, 2.0, 1.0) AS score
             FROM chunks
             WHERE chunks MATCH ?1
             ORDER BY score
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![expr, limit as i64], |row| {
            Ok(Hit {
                base: row.get(0)?,
                path: row.get(1)?,
                heading_path: row.get(2)?,
                excerpt: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn counts(&self) -> R<(i64, i64)> {
        let files: i64 = self.conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        let chunks: i64 = self.conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        Ok((files, chunks))
    }
}

/// Turns a human question into an FTS5 expression.
///
/// Every term is wrapped in double quotes and joined with OR. The quoting is not
/// cosmetic: FTS5 reads bare punctuation as syntax, so an unquoted question with a
/// hyphen or a colon in it is a syntax error rather than a search. OR rather than AND
/// because a question rarely uses the file's exact vocabulary, and BM25 already
/// rewards the documents that match more of it.
fn fts_query(terms: &[String]) -> Option<String> {
    let quoted: Vec<String> = terms.iter().map(|t| format!("\"{t}\"")).collect();
    if quoted.is_empty() {
        return None;
    }
    Some(quoted.join(" OR "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_split_at_headings() {
        let doc = "# Title\n\nintro text\n\n## One\n\nbody one\n\n## Two\n\nbody two";
        let chunks = chunk(doc);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading_path, "Title");
        assert_eq!(chunks[1].heading_path, "Title > One");
        assert_eq!(chunks[2].heading_path, "Title > Two");
        assert!(chunks[2].text.contains("body two"));
    }

    #[test]
    fn front_matter_is_not_indexed() {
        let doc = "---\nprovenance: agent\n---\n\n# Title\n\nbody";
        let chunks = chunk(doc);
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].text.contains("provenance"));
    }

    #[test]
    fn a_hash_inside_a_code_fence_is_not_a_heading() {
        let doc = "# Title\n\n```bash\n# this is a shell comment\necho hi\n```\n\ntail";
        let chunks = chunk(doc);
        assert_eq!(chunks.len(), 1, "the fence must not open a new section");
    }

    #[test]
    fn a_deeper_heading_nests_and_a_shallower_one_pops() {
        let doc = "# T\n\na\n\n## A\n\nb\n\n### A1\n\nc\n\n## B\n\nd";
        let paths: Vec<String> = chunk(doc).into_iter().map(|c| c.heading_path).collect();
        assert_eq!(paths, vec!["T", "T > A", "T > A > A1", "T > B"]);
    }

    #[test]
    fn an_oversized_section_is_split_with_overlap() {
        let para = "word ".repeat(200); // 1000 chars
        let doc = format!("# T\n\n{para}\n\n{para}\n\n{para}");
        let chunks = chunk(&doc);
        assert!(chunks.len() > 1, "3000 characters must not be one chunk");
        for c in &chunks {
            assert!(c.text.chars().count() <= MAX_CHARS + 1, "window over the cap");
        }
    }

    #[test]
    fn splitting_terminates_on_a_section_with_no_blank_lines() {
        // The break-at-blank-line logic must not stall when there is no blank line.
        let doc = format!("# T\n\n{}", "x".repeat(5000));
        let chunks = chunk(&doc);
        assert!(chunks.len() >= 3);
    }

    #[test]
    fn the_keyword_line_is_not_indexed_as_text() {
        let doc = "# T\n\nreal content here\n\nSearch for: `alpha`, `beta`, `gamma`.";
        let text: String = chunk(doc).into_iter().map(|c| c.text).collect();
        assert!(text.contains("real content"));
        assert!(!text.contains("alpha"), "keywords must stay in the keyword scorer");
    }

    #[test]
    fn a_whole_search_for_section_is_skipped() {
        let doc = "# T\n\nbody\n\n## Search for\n\n`one`, `two`, `three`\n";
        let chunks = chunk(doc);
        let text: String = chunks.iter().map(|c| c.text.clone()).collect();
        assert!(text.contains("body"));
        assert!(!text.contains("one"), "the section body is keywords too");
    }

    #[test]
    fn the_hash_changes_with_the_content() {
        assert_ne!(content_hash("a"), content_hash("b"));
        assert_eq!(content_hash("same"), content_hash("same"));
    }

    #[test]
    fn an_fts_query_quotes_every_term() {
        let terms = crate::index::normalise("qual e o piso calorico?");
        let q = fts_query(&terms).expect("terms");
        assert!(q.contains("\"piso\""));
        assert!(q.contains(" OR "));
        assert!(!q.contains('?'), "punctuation would be read as syntax");
    }

    #[test]
    fn a_query_of_only_stopwords_produces_nothing() {
        let terms = crate::index::normalise("de o a");
        assert!(fts_query(&terms).is_none());
    }

    #[test]
    fn the_alias_expansion_reaches_the_text_scorer() {
        // The bug this guards: "piso calorico" routed by keyword and matched zero
        // chunks by text, because the English term was never substituted here.
        let aliases = vec![("piso calorico".to_string(), "calorie floor".to_string())];
        let terms = crate::index::expand_query("qual o piso calorico", &aliases);
        let q = fts_query(&terms).expect("terms");
        assert!(q.contains("\"calorie\""));
        assert!(q.contains("\"floor\""));
        assert!(q.contains("\"piso\""), "the original survives too");
    }
}
