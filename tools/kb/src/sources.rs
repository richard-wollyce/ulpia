//! Sources as first class objects, and citations as pointers to them.
//!
//! **The problem this exists to make structurally impossible.** Before it, a source was
//! restated in three unconnected places: a free prose `Sources, read <date>:` paragraph at
//! the foot of a note, a row in an inbox `SOURCES.md` ledger, and a `captured_from` string
//! in front matter. Nothing joined them, so nothing could check them against each other,
//! and every citation defect the fleet has shipped was a divergence between two
//! restatements. `Maslow (1965)` in a body against no entry in any list. Lax and Sebenius
//! dated 1996 in one paragraph and 1986 in another. `LAUREANO ... Cidade: Editora, 2005`,
//! a template placeholder that survived because there was no field it could have been left
//! empty in.
//!
//! Here a source is one record with an opaque key, and every mention of it is a pointer at
//! that record. The bibliography line at the foot of a note is a **rendering of the
//! record**, compared byte for byte by [`crate::checks`], so it is a cache with an equality
//! check rather than a fourth place a person maintains the same facts. A reference list
//! cannot disagree with the body, because there is one string and both readings come from
//! it.
//!
//! ## Why a key and not a name
//!
//! [`crate::ingest`] already asked for this in its own words and could not fix it there:
//! two documents whose stems slug the same, ingested on the same day, get one
//! `captured_from` string, "and afterwards nothing says which note came from which. That
//! wants the deposit name to carry something derived from the content". The answer is one
//! level up. Identity is not derived from content, from a path or from a formula over the
//! metadata; it is minted once and never changes. A formula key (`auth.lower + year`, which
//! is what Better BibTeX builds) changes the day somebody fixes a misspelled author, which
//! is the same bug wearing a nicer face.
//!
//! ## The record lives on disk, one file per source
//!
//! `sources/<KEY>.txt`, at the base root, in the `key = value` shape this repository
//! already uses for `agent.txt` and `kb-aliases.txt`. Three consequences, all deliberate:
//!
//! - **One file per source, not one ledger.** More than one session writes these
//!   repositories at once (ADR-0021), and two sessions adding two sources to one appended
//!   ledger is a merge conflict on every busy day. Two files are not.
//! - **`.txt` and not `.md`.** A record is structured data with no prose in it. As markdown
//!   it would join the chunk index, be graded for a keyword line it has no use for, and be
//!   reported by the house style check for an em dash that belongs to somebody else's book
//!   title.
//! - **The path is derived from the key and never the reverse.** This repository has paid
//!   for path keyed identity twice.
//!
//! The SQLite `sources` table is derived from these files, the same way every other index in
//! this crate is derived (ADR-0003), and is rebuilt wholesale on each sync.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The directory holding source records, relative to a base root.
pub const DIR: &str = "sources";

/// The transcription safe alphabet, stolen from Zotero's `utilities.js` for the reason it
/// is there: `0`, `1` and `O` are gone, so a key read aloud or copied off a screen has no
/// pair a person can confuse. `I` and `L` survive because `1` is the character they were
/// confusable with and `1` is the one that left.
pub const ALPHABET: &[u8] = b"23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

/// Ten characters, and the length is arithmetic rather than a copy of Zotero's eight.
///
/// A key is minted without coordination: bases do not see each other, private bases are not
/// readable from outside, and ADR-0037 assumes the whole thing can be restored onto a
/// machine that has never met the others. So uniqueness has to come from the draw and not
/// from a check, and the number that decides the length is the birthday bound
/// `p ~ n^2 / 2N` over the lifetime population `n`.
///
/// The measured scale. The busiest week this fleet has had read 63 sources, against 204
/// knowledge notes total after months of work. Sustained at three times that busiest week,
/// forever, a single person's fleet reaches about 1e5 sources in fifty years. 1e6 is the
/// paranoid bound, ten times wrong.
///
/// | length | N = 33^len | p at 1e5 | p at 1e6 |
/// |---|---|---|---|
/// | 8 | 1.41e12 | 1 in 280 | 1 in 2.8 |
/// | 10 | 1.53e15 | 1 in 306,000 | 1 in 3,060 |
/// | 12 | 1.67e18 | 1 in 3.3e8 | 1 in 3.3e6 |
///
/// **Eight is not enough here and it is enough in Zotero**, because Zotero checks
/// `UNIQUE (libraryID, key)` at insert against a library it can see all of, and we cannot.
/// Twelve buys three more orders of magnitude and spends them on a token nobody can hold in
/// one glance; the alphabet exists so a person can read a key aloud, and twelve characters
/// undoes the reason for the alphabet. Ten reads as two groups of five, and it survives
/// being wrong about the scale by a factor of ten, which eight does not.
pub const KEY_LEN: usize = 10;

/// The six types, and six is the whole argument.
///
/// Zotero has forty because it feeds citation style processors that render a thesis
/// differently from a manuscript. We are not producing journal ready bibliographies, that is
/// out of scope and stated so, and CSL JSON is the export for anything that ever needs it.
/// What the type has to do here is two jobs: say what kind of thing was read, and pick a CSL
/// type on the way out. Six covers everything the fleet has actually cited: books, chapters
/// with their own author, journal articles, institutional standards and reports, web pages,
/// and recordings.
pub const TYPES: &[&str] = &["book", "chapter", "article", "report", "webpage", "recording"];

/// What actually happened when somebody went to get this source.
///
/// **This is the field that would have caught `Cidade: Editora, 2005`.** Not by validating
/// the publisher, which nothing can do, but because there is no template to copy from any
/// more: a record is minted from arguments by [`add`], and the writer has to choose one of
/// these four words out loud. `metadata` is the word that stops "I only saw the catalogue
/// entry" from being invisible, and the fleet has already needed it once, for Wahba and
/// Bridwell 1976, paywalled and cited anyway.
///
/// The honest boundary: a typed field removes the template, it does not remove lying. A
/// person can still type `full` over a source they skimmed. What it removes is the case
/// where nobody was ever asked.
pub const RETRIEVAL: &[&str] = &["full", "partial", "metadata", "unreachable"];

/// One source, as read off its record file.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub key: String,
    pub kind: String,
    pub title: String,
    /// In the order written. A record may carry several `author =` lines.
    pub authors: Vec<String>,
    pub year: Option<String>,
    /// Journal, book, site or channel: whatever the title sits inside.
    pub container: Option<String>,
    pub volume: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub url: Option<String>,
    pub doi: Option<String>,
    pub isbn: Option<String>,
    pub lang: Option<String>,
    pub retrieved_on: String,
    pub retrieval_status: String,
    /// The key of the source this one supersedes, Dublin Core's `dc:replaces`.
    ///
    /// **One direction only, and that is the decision.** A back pointer on the old record
    /// would be a second fact saying the same thing, free to disagree with the first, which
    /// is the failure this whole module exists to remove. The replaced record is found by
    /// scanning, and every record is loaded anyway.
    pub replaces: Option<String>,
    /// Anything a person needed to say about the retrieval that no field holds.
    pub note: Option<String>,
    /// Where the record was read from. Not written to the file.
    pub path: PathBuf,
}

/// A record file that could not be read as one.
///
/// Reported rather than skipped. A loader that quietly drops a record with a typo in
/// `retrieval_status` makes every citation of it fail as "no such source", which blames the
/// note for a defect in the record and sends whoever is fixing it to the wrong file.
#[derive(Debug, Clone)]
pub struct BadRecord {
    pub path: String,
    pub problem: String,
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// True when this is a well formed key: the right length, and nothing outside the alphabet.
pub fn is_key(s: &str) -> bool {
    s.len() == KEY_LEN && s.bytes().all(|b| ALPHABET.contains(&b))
}

/// Mints a key from operating system entropy.
///
/// **Uniqueness comes from the draw, and the draw has to be random rather than clever.**
/// The tempting cheap source is the clock, and it is wrong here for a measurable reason:
/// Windows' default system clock granularity is milliseconds at best and often about 15 ms,
/// so two sessions minting inside the same tick get the same number. `RandomState` is seeded
/// from the operating system's own generator once per process and stepped per instance,
/// which is exactly the property needed, and it is in std, so it costs no dependency.
///
/// Modulo bias is rejected rather than tolerated: values at the top of the `u64` range that
/// do not divide evenly into `33^10` are redrawn, so every key is equally likely. The
/// rejection window is about one draw in 12,000.
pub fn mint() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let span: u64 = (ALPHABET.len() as u64).pow(KEY_LEN as u32);
    let limit = (u64::MAX / span) * span;

    loop {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
        );
        hasher.write_u32(std::process::id());
        let mut n = hasher.finish();
        if n >= limit {
            continue;
        }
        let mut out = String::with_capacity(KEY_LEN);
        for _ in 0..KEY_LEN {
            out.push(ALPHABET[(n % ALPHABET.len() as u64) as usize] as char);
            n /= ALPHABET.len() as u64;
        }
        return out;
    }
}

// ---------------------------------------------------------------------------
// Citations in a note
// ---------------------------------------------------------------------------

/// Every `[src:KEY]` in the text, as (line number, raw token).
///
/// **Single brackets, not `[[src:KEY]]`, and the choice is not cosmetic.** Double brackets
/// are this repository's wikilink, and ADR-0026 gives that syntax one meaning: a pointer to
/// a note in this base, resolved by file stem, refused across a base edge. Overloading it
/// would make one syntax mean two things and force every reader of a wikilink, the linter,
/// the router, Obsidian and a person, to learn the exception. ADR-0029 is the record of what
/// that costs when it is allowed once.
///
/// Single brackets carry no meaning in this base today, and `[1]` is already what a citation
/// marker looks like to anyone who has read a paper. A markdown link `[text](url)` cannot
/// collide with it, because the token has to start with `src:` immediately after the bracket.
///
/// Fenced blocks and inline code are skipped, for the same reason [`crate::checks::wikilinks`]
/// skips them: a file documenting the convention writes the token in backticks, and that is
/// an example rather than a citation. This module's own header depends on it.
pub fn citations(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_fence = false;

    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for segment in raw.split('`').step_by(2) {
            for token in tokens_in(segment) {
                out.push((i + 1, token));
            }
        }
    }
    out
}

/// The raw tokens between `[src:` and the next `]` in one fragment.
fn tokens_in(fragment: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = fragment;
    while let Some(at) = rest.find("[src:") {
        rest = &rest[at + 5..];
        match rest.find(']') {
            Some(end) => {
                out.push(rest[..end].to_string());
                rest = &rest[end + 1..];
            }
            // An unclosed marker is not a citation, the same way an unclosed `[[` is not a
            // link. Reporting it would mean guessing where the writer meant it to end.
            None => break,
        }
    }
    out
}

/// True when this line is a bibliography entry: a list item opening with a citation pointer.
///
/// **This is where the embedded copy lives**, and naming it with a shape rather than a
/// heading is what keeps the rule cheap. A note carries `- [src:KEY] <rendering>` under
/// whatever heading it likes; the check finds the line by its opening and compares the rest
/// to what the record renders. An inline `[src:KEY]` in a sentence is a pointer and is not
/// compared to anything, because the sentence is prose and the entry is the copy.
pub fn bibliography_line(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    let t = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "))?;
    let t = t.strip_prefix("[src:")?;
    let end = t.find(']')?;
    Some((t[..end].to_string(), t[end + 1..].trim().to_string()))
}

/// The line number of a hand written `Sources, ...:` paragraph, if the note carries one.
///
/// **This exists to be deleted.** It recognises the shape the fleet wrote by hand before
/// there were records, `Sources, read 2026-09-06: ...` at the foot of a note, 29 of them in
/// Cosimo on 2026-09-08 and none anywhere else. It is here for one job: a note that cites in
/// prose is a note making a sourced claim, so the evidence rules have to reach it, and
/// keying that off a `source:` line in front matter is what let 73 notes go ungraded.
///
/// It matches the opening of a line rather than the word anywhere, so a sentence about
/// sources in a body is not a citation. Both languages, because the fleet writes in both.
pub fn prose_paragraph(text: &str) -> Option<usize> {
    text.lines().enumerate().find_map(|(i, line)| {
        let lower = line.trim_start().to_lowercase();
        let opens = ["sources, ", "sources:", "fontes, ", "fontes:"]
            .iter()
            .any(|p| lower.starts_with(p));
        if opens { Some(i + 1) } else { None }
    })
}

// ---------------------------------------------------------------------------
// The record file
// ---------------------------------------------------------------------------

/// Reads a record file's `key = value` lines, keeping repeats in order.
///
/// A line whose first non space character is `#` is a comment. `#` anywhere else is content,
/// which matters: a URL fragment and a book title both carry one, and stripping at the first
/// `#` the way `kb-aliases.txt` does would silently truncate both.
fn fields(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect()
}

/// Parses one record. `key` comes from the file name, not from the file body: the two could
/// disagree, and a file that answers to a name it does not carry is the identity bug again.
pub fn parse(key: &str, text: &str) -> Result<Source, String> {
    if !is_key(key) {
        return Err(format!(
            "`{key}` is not a key: {KEY_LEN} characters from {}",
            String::from_utf8_lossy(ALPHABET)
        ));
    }
    let pairs = fields(text);
    let one = |name: &str| {
        pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    };
    let all = |name: &str| -> Vec<String> {
        pairs.iter().filter(|(k, _)| k == name).map(|(_, v)| v.clone()).collect()
    };

    let kind = one("type").ok_or_else(|| "no `type =` line".to_string())?;
    if !TYPES.contains(&kind.as_str()) {
        return Err(format!("type is `{kind}`, which is not one of {}", TYPES.join(", ")));
    }
    let retrieval_status =
        one("retrieval_status").ok_or_else(|| "no `retrieval_status =` line".to_string())?;
    if !RETRIEVAL.contains(&retrieval_status.as_str()) {
        return Err(format!(
            "retrieval_status is `{retrieval_status}`, which is not one of {}",
            RETRIEVAL.join(", ")
        ));
    }
    let replaces = one("replaces");
    if let Some(r) = &replaces {
        if !is_key(r) {
            return Err(format!("replaces is `{r}`, which is not a key"));
        }
        if r == key {
            return Err("replaces names this record, so it supersedes itself".to_string());
        }
    }

    Ok(Source {
        key: key.to_string(),
        kind,
        title: one("title").ok_or_else(|| "no `title =` line".to_string())?,
        authors: all("author"),
        year: one("year"),
        container: one("container"),
        volume: one("volume"),
        pages: one("pages"),
        publisher: one("publisher"),
        url: one("url"),
        doi: one("doi"),
        isbn: one("isbn"),
        lang: one("lang"),
        retrieved_on: one("retrieved_on")
            .ok_or_else(|| "no `retrieved_on =` line".to_string())?,
        retrieval_status,
        replaces,
        note: one("note"),
        path: PathBuf::new(),
    })
}

/// The record file's text, in a fixed field order so two records diff against each other.
pub fn render_record(s: &Source) -> String {
    let mut out = String::new();
    let mut line = |k: &str, v: &str| {
        if !v.is_empty() {
            out.push_str(k);
            out.push_str(" = ");
            out.push_str(v);
            out.push('\n');
        }
    };
    line("type", &s.kind);
    line("title", &s.title);
    for a in &s.authors {
        line("author", a);
    }
    for (k, v) in [
        ("year", &s.year),
        ("container", &s.container),
        ("volume", &s.volume),
        ("pages", &s.pages),
        ("publisher", &s.publisher),
        ("url", &s.url),
        ("doi", &s.doi),
        ("isbn", &s.isbn),
        ("lang", &s.lang),
    ] {
        line(k, v.as_deref().unwrap_or(""));
    }
    line("retrieved_on", &s.retrieved_on);
    line("retrieval_status", &s.retrieval_status);
    line("replaces", s.replaces.as_deref().unwrap_or(""));
    line("note", s.note.as_deref().unwrap_or(""));
    out
}

/// Every record in a base, plus the files that are in the directory and are not records.
pub fn load(root: &Path) -> (Vec<Source>, Vec<BadRecord>) {
    let dir = root.join(DIR);
    let mut found = Vec::new();
    let mut bad = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (found, bad);
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "txt"))
        .collect();
    paths.sort();

    for path in paths {
        let rel = format!("{DIR}/{}", path.file_name().unwrap_or_default().to_string_lossy());
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        match std::fs::read_to_string(&path) {
            Err(e) => bad.push(BadRecord { path: rel, problem: e.to_string() }),
            Ok(text) => match parse(&stem, &text) {
                Ok(mut s) => {
                    s.path = path;
                    found.push(s);
                }
                Err(problem) => bad.push(BadRecord { path: rel, problem }),
            },
        }
    }
    (found, bad)
}

/// Whether anything in the set supersedes this key.
pub fn replacement_of<'a>(all: &'a [Source], key: &str) -> Option<&'a Source> {
    all.iter().find(|s| s.replaces.as_deref() == Some(key))
}

// ---------------------------------------------------------------------------
// The rendering that the bibliography line has to equal
// ---------------------------------------------------------------------------

/// One line, deterministic, that a note's bibliography entry has to match exactly.
///
/// **This is not a citation style and it must not become one.** A style engine is out of
/// scope, deliberately: CSL and its processors exist and [`to_csl`] is the door to them. What
/// this is, is a canonical form, chosen so equality is decidable by a machine and readable by
/// a person. That is why every title is quoted, including a book's, which no style would do.
///
/// Deterministic means no wrapping and no locale: the check compares strings, and a renderer
/// that reflowed at eighty columns would make the comparison depend on the length of the
/// author's name.
pub fn render_line(s: &Source) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !s.authors.is_empty() {
        parts.push(s.authors.join("; "));
    }
    parts.push(format!("\"{}\"", s.title));

    let mut where_it_sits = String::new();
    if let Some(c) = &s.container {
        where_it_sits.push_str(c);
        if let Some(v) = &s.volume {
            where_it_sits.push(' ');
            where_it_sits.push_str(v);
        }
    }
    if let Some(p) = &s.pages {
        if !where_it_sits.is_empty() {
            where_it_sits.push_str(", ");
        }
        where_it_sits.push_str(p);
    }
    if !where_it_sits.is_empty() {
        parts.push(where_it_sits);
    }

    for opt in [&s.publisher, &s.year] {
        if let Some(v) = opt {
            parts.push(v.clone());
        }
    }
    if let Some(d) = &s.doi {
        parts.push(format!("DOI {d}"));
    }
    if let Some(i) = &s.isbn {
        parts.push(format!("ISBN {i}"));
    }
    if let Some(u) = &s.url {
        parts.push(u.clone());
    }
    parts.push(format!("{}, {}", status_phrase(&s.retrieval_status), s.retrieved_on));
    join(&parts)
}

/// Joins the segments with a full stop, without doubling one that is already there.
///
/// `Maslow, A. H.` ends in the abbreviation's own stop, and a blind `join(". ")` renders it
/// `Maslow, A. H.. "A Theory..."`. Worth a function because the check compares this string
/// byte for byte: whatever it produces is what every note on disk has to carry, so the ugly
/// version would have been permanent.
fn join(parts: &[String]) -> String {
    let mut out = String::new();
    for part in parts {
        if !out.is_empty() {
            out.push_str(if out.ends_with('.') { " " } else { ". " });
        }
        out.push_str(part);
    }
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// The four words spelled out, because `metadata` on its own reads like a field name rather
/// than like the admission it is.
fn status_phrase(status: &str) -> &'static str {
    match status {
        "full" => "read in full",
        "partial" => "read in part",
        "metadata" => "catalogue record only, not read",
        _ => "could not be reached",
    }
}

// ---------------------------------------------------------------------------
// CSL JSON
// ---------------------------------------------------------------------------

/// Our type names to CSL's, which is the only place the six type set has to be defensible
/// against somebody else's vocabulary.
fn csl_type(kind: &str) -> &'static str {
    match kind {
        "book" => "book",
        "chapter" => "chapter",
        "article" => "article-journal",
        "report" => "report",
        "recording" => "motion_picture",
        _ => "webpage",
    }
}

/// Splits `Family, Given` into CSL's two fields, and leaves anything without a comma as a
/// `literal` name.
///
/// **The literal case is the honest one and not a fallback.** `ISACA`, `Program on
/// Negotiation at Harvard Law School` and `Axelos` are all authors the fleet has actually
/// cited, and forcing an institution into a family name is how a bibliography ends up
/// alphabetised under `Negotiation`.
fn csl_author(name: &str) -> crate::json::Value {
    let mut v = crate::json::Value::obj();
    match name.split_once(',') {
        Some((family, given)) if !given.trim().is_empty() => {
            v.set("family", crate::json::Value::Str(family.trim().to_string()));
            v.set("given", crate::json::Value::Str(given.trim().to_string()));
        }
        _ => {
            v.set("literal", crate::json::Value::Str(name.to_string()));
        }
    }
    v
}

/// A `date-parts` value from a `YYYY-MM-DD` or `YYYY` string, dropping anything that is not
/// a number rather than guessing at it.
fn date_parts(raw: &str) -> crate::json::Value {
    use crate::json::Value;
    let nums: Vec<Value> = raw
        .split('-')
        .take(3)
        .filter_map(|p| p.trim().parse::<f64>().ok())
        .map(Value::Num)
        .collect();
    let mut v = Value::obj();
    v.set("date-parts", Value::Arr(vec![Value::Arr(nums)]));
    v
}

/// The whole set as a CSL JSON array.
///
/// **CSL JSON from day one, and the reason is that it costs nothing today and buys the exit
/// later.** It is a published, stable shape that Zotero, Pandoc and every processor already
/// read, so the sources can leave this system without a converter being written under
/// pressure. It is an export and not an import: nothing here reads CSL back, because that
/// would be a translator, which is out of scope.
///
/// `accessed` carries `retrieved_on`, which is the field CSL has for exactly this and which
/// the prose paragraphs were retyping by hand, note by note.
pub fn to_csl(all: &[Source]) -> crate::json::Value {
    use crate::json::Value;
    let mut out = Vec::new();
    for s in all {
        let mut v = Value::obj();
        v.set("id", Value::Str(s.key.clone()));
        v.set("type", Value::Str(csl_type(&s.kind).to_string()));
        v.set("title", Value::Str(s.title.clone()));
        if !s.authors.is_empty() {
            v.set(
                "author",
                Value::Arr(s.authors.iter().map(|a| csl_author(a)).collect()),
            );
        }
        for (field, value) in [
            ("container-title", &s.container),
            ("volume", &s.volume),
            ("page", &s.pages),
            ("publisher", &s.publisher),
            ("URL", &s.url),
            ("DOI", &s.doi),
            ("ISBN", &s.isbn),
            ("language", &s.lang),
            ("note", &s.note),
        ] {
            if let Some(x) = value {
                v.set(field, Value::Str(x.clone()));
            }
        }
        if let Some(y) = &s.year {
            v.set("issued", date_parts(y));
        }
        v.set("accessed", date_parts(&s.retrieved_on));
        // Not a CSL field. Carried under `custom`, which is where CSL JSON puts anything a
        // processor should pass through untouched, so the verdict travels with the export
        // instead of being the one fact that only exists inside kb.
        let mut custom = Value::obj();
        custom.set("retrieval_status", Value::Str(s.retrieval_status.clone()));
        if let Some(r) = &s.replaces {
            custom.set("replaces", Value::Str(r.clone()));
        }
        v.set("custom", custom);
        out.push(v);
    }
    Value::Arr(out)
}

// ---------------------------------------------------------------------------
// Writing a record
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum AddError {
    BadField(&'static str, String, String),
    Missing(&'static str),
    NoKeyLeft,
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::BadField(name, got, legal) => {
                write!(f, "{name} is `{got}`, which is not one of {legal}")
            }
            AddError::Missing(name) => write!(f, "--{name} is required"),
            AddError::NoKeyLeft => write!(
                f,
                "could not mint a free key in eight tries, which at 33^{KEY_LEN} means \
                 the entropy source is broken rather than the space being full"
            ),
            AddError::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

/// Mints a key and writes the record, refusing anything the record format cannot hold.
///
/// The fields arrive as `(name, value)` pairs so the caller stays one argument parser rather
/// than a fourteen argument signature. Unknown names are ignored here and rejected by
/// `main`, which is the layer that knows what a flag is.
pub fn add(root: &Path, given: &[(String, String)]) -> Result<Source, AddError> {
    let get = |name: &str| {
        given.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty())
    };
    let all = |name: &str| -> Vec<String> {
        given
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect()
    };

    let kind = get("type").ok_or(AddError::Missing("type"))?;
    if !TYPES.contains(&kind.as_str()) {
        return Err(AddError::BadField("type", kind, TYPES.join(", ")));
    }
    let status = get("retrieval-status").ok_or(AddError::Missing("retrieval-status"))?;
    if !RETRIEVAL.contains(&status.as_str()) {
        return Err(AddError::BadField("retrieval-status", status, RETRIEVAL.join(", ")));
    }
    if let Some(r) = get("replaces") {
        if !is_key(&r) {
            return Err(AddError::BadField("replaces", r, format!("{KEY_LEN} characters from the key alphabet")));
        }
    }

    let dir = root.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| AddError::Io(dir.clone(), e))?;

    // Eight tries against the directory, which is belt over braces: the draw is already
    // birthday safe by [`KEY_LEN`], and this catches the case the arithmetic cannot, which
    // is an entropy source that has stopped moving.
    let mut key = String::new();
    for _ in 0..8 {
        let candidate = mint();
        if !dir.join(format!("{candidate}.txt")).exists() {
            key = candidate;
            break;
        }
    }
    if key.is_empty() {
        return Err(AddError::NoKeyLeft);
    }

    let source = Source {
        key: key.clone(),
        kind,
        title: get("title").ok_or(AddError::Missing("title"))?,
        authors: all("author"),
        year: get("year"),
        container: get("container"),
        volume: get("volume"),
        pages: get("pages"),
        publisher: get("publisher"),
        url: get("url"),
        doi: get("doi"),
        isbn: get("isbn"),
        lang: get("lang"),
        retrieved_on: get("retrieved-on").ok_or(AddError::Missing("retrieved-on"))?,
        retrieval_status: status,
        replaces: get("replaces"),
        note: get("note"),
        path: dir.join(format!("{key}.txt")),
    };

    std::fs::write(&source.path, render_record(&source))
        .map_err(|e| AddError::Io(source.path.clone(), e))?;
    Ok(source)
}

/// Every source keyed by key, for a caller that resolves pointers.
pub fn by_key(all: &[Source]) -> BTreeMap<&str, &Source> {
    all.iter().map(|s| (s.key.as_str(), s)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-sources-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn maslow() -> Source {
        Source {
            key: "K7M2QX4BTF".into(),
            kind: "article".into(),
            title: "A Theory of Human Motivation".into(),
            authors: vec!["Maslow, A. H.".into()],
            year: Some("1943".into()),
            container: Some("Psychological Review".into()),
            volume: Some("50".into()),
            pages: Some("370-396".into()),
            publisher: None,
            url: Some("https://psychclassics.yorku.ca/Maslow/motivation.htm".into()),
            doi: None,
            isbn: None,
            lang: Some("en".into()),
            retrieved_on: "2026-09-06".into(),
            retrieval_status: "full".into(),
            replaces: None,
            note: None,
            path: PathBuf::new(),
        }
    }

    /// The alphabet is the whole reason a key can be read aloud, so it is pinned rather
    /// than assumed: 33 characters and none of the three confusable ones.
    #[test]
    fn the_alphabet_drops_the_characters_a_person_would_misread() {
        assert_eq!(ALPHABET.len(), 33);
        for c in [b'0', b'1', b'O'] {
            assert!(!ALPHABET.contains(&c), "{} is confusable and must not be mintable", c as char);
        }
    }

    /// Not a test of randomness, which cannot be tested here. A test that the draw moves at
    /// all: the clock based version this replaced returned the same key twice inside one
    /// Windows timer tick, which is the failure that would otherwise ship silently.
    #[test]
    fn minting_a_thousand_keys_produces_a_thousand_distinct_well_formed_keys() {
        let keys: std::collections::BTreeSet<String> = (0..1000).map(|_| mint()).collect();
        assert_eq!(keys.len(), 1000, "the entropy source is not moving");
        for k in &keys {
            assert!(is_key(k), "{k} is not a well formed key");
        }
    }

    #[test]
    fn a_key_of_the_wrong_length_or_alphabet_is_not_a_key() {
        assert!(is_key("K7M2QX4BTF"));
        assert!(!is_key("K7M2QX4BT"), "nine characters");
        assert!(!is_key("K7M2QX4BTFA"), "eleven characters");
        assert!(!is_key("K7M2QX4BT0"), "zero is not in the alphabet");
        assert!(!is_key("k7m2qx4btf"), "lower case is a different string");
    }

    #[test]
    fn a_citation_is_found_and_a_code_span_is_not() {
        let text = "As [src:K7M2QX4BTF] shows.\nThe syntax is `[src:AAAAAAAAAA]`, an example.";
        assert_eq!(citations(text), vec![(1, "K7M2QX4BTF".to_string())]);
    }

    #[test]
    fn a_fenced_block_carries_no_citations() {
        let text = "before\n```\n[src:K7M2QX4BTF]\n```\nafter [src:AAAAAAAAAA]";
        assert_eq!(citations(text), vec![(5, "AAAAAAAAAA".to_string())]);
    }

    /// A markdown link is the one shape that could have collided with single brackets, and
    /// the `src:` prefix is what stops it.
    #[test]
    fn a_markdown_link_is_not_a_citation() {
        assert!(citations("see [the note](https://example.org/src:AAAAAAAAAA)").is_empty());
    }

    #[test]
    fn a_bibliography_line_splits_into_its_key_and_its_rendering() {
        let (key, rest) = bibliography_line("- [src:K7M2QX4BTF] Maslow. \"A Theory\".")
            .expect("a list item opening with a pointer");
        assert_eq!(key, "K7M2QX4BTF");
        assert_eq!(rest, "Maslow. \"A Theory\".");
        assert!(
            bibliography_line("As [src:K7M2QX4BTF] shows.").is_none(),
            "an inline pointer is not the embedded copy"
        );
    }

    /// The rendering is the thing `kb check` compares byte for byte, so its exact shape is
    /// the contract and a change to it is a change every note has to be told about.
    #[test]
    fn the_rendering_is_one_deterministic_line() {
        assert_eq!(
            render_line(&maslow()),
            "Maslow, A. H. \"A Theory of Human Motivation\". Psychological Review 50, 370-396. \
             1943. https://psychclassics.yorku.ca/Maslow/motivation.htm. read in full, 2026-09-06."
        );
        assert!(!render_line(&maslow()).contains('\n'), "a wrapped line is not comparable");
    }

    /// The field that exists so `Cidade: Editora, 2005` cannot happen quietly: a source
    /// nobody opened says so in the rendering, in words.
    #[test]
    fn a_source_only_seen_in_a_catalogue_says_so_in_its_own_line() {
        let mut s = maslow();
        s.retrieval_status = "metadata".into();
        assert!(render_line(&s).contains("catalogue record only, not read"));
    }

    #[test]
    fn a_record_round_trips_through_its_file_format() {
        let s = maslow();
        let back = parse(&s.key, &render_record(&s)).expect("parse");
        assert_eq!(back, Source { path: PathBuf::new(), ..s });
    }

    /// A `#` inside a value is content. Stripping at the first one, which is what the alias
    /// table does, would truncate a URL fragment and half of any title carrying one.
    #[test]
    fn a_hash_inside_a_value_survives_and_a_hash_starting_a_line_does_not() {
        let text = "# a comment\ntype = webpage\ntitle = C# and its history\n\
                    url = https://example.org/page#section\nretrieved_on = 2026-09-06\n\
                    retrieval_status = full\n";
        let s = parse("K7M2QX4BTF", text).expect("parse");
        assert_eq!(s.title, "C# and its history");
        assert_eq!(s.url.as_deref(), Some("https://example.org/page#section"));
    }

    #[test]
    fn a_record_with_an_illegal_type_or_status_is_refused_by_name() {
        let base = "title = x\nretrieved_on = 2026-09-06\n";
        let e = parse("K7M2QX4BTF", &format!("type = tweet\nretrieval_status = full\n{base}"))
            .expect_err("tweet is not a type");
        assert!(e.contains("tweet") && e.contains("book"), "{e}");
        let e = parse("K7M2QX4BTF", &format!("type = book\nretrieval_status = skimmed\n{base}"))
            .expect_err("skimmed is not a status");
        assert!(e.contains("skimmed") && e.contains("metadata"), "{e}");
    }

    #[test]
    fn several_author_lines_are_kept_in_order() {
        let text = "type = article\ntitle = Who Built Maslow's Pyramid\n\
                    author = Bridgman, T.\nauthor = Cummings, S.\nauthor = Ballard, J.\n\
                    retrieved_on = 2026-09-06\nretrieval_status = full\n";
        let s = parse("K7M2QX4BTF", text).expect("parse");
        assert_eq!(s.authors.len(), 3);
        assert_eq!(s.authors[0], "Bridgman, T.");
        assert!(render_line(&s).starts_with("Bridgman, T.; Cummings, S.; Ballard, J."));
    }

    #[test]
    fn add_mints_a_key_writes_the_file_and_load_reads_it_back() {
        let dir = scratch("add");
        let given: Vec<(String, String)> = [
            ("type", "book"),
            ("title", "Eupsychian Management"),
            ("author", "Maslow, A. H."),
            ("year", "1965"),
            ("publisher", "R. D. Irwin"),
            ("retrieved-on", "2026-09-06"),
            ("retrieval-status", "metadata"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        let made = add(&dir, &given).expect("add");
        assert!(is_key(&made.key));
        assert!(made.path.ends_with(format!("{}.txt", made.key)));

        let (loaded, bad) = load(&dir);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].key, made.key);
        assert_eq!(loaded[0].title, "Eupsychian Management");
        assert_eq!(loaded[0].retrieval_status, "metadata");
    }

    /// The loader reports a broken record instead of dropping it, because a dropped record
    /// makes every citation of it fail as "no such source" and sends the reader to the note.
    #[test]
    fn a_broken_record_is_reported_rather_than_skipped() {
        let dir = scratch("bad");
        std::fs::create_dir_all(dir.join(DIR)).expect("mkdir");
        std::fs::write(dir.join(DIR).join("K7M2QX4BTF.txt"), "type = book\n").expect("write");
        std::fs::write(dir.join(DIR).join("not-a-key.txt"), "type = book\n").expect("write");

        let (loaded, bad) = load(&dir);
        assert!(loaded.is_empty());
        assert_eq!(bad.len(), 2);
        assert!(bad.iter().any(|b| b.problem.contains("retrieval_status")), "{bad:?}");
        assert!(bad.iter().any(|b| b.problem.contains("not a key")), "{bad:?}");
    }

    #[test]
    fn a_record_cannot_supersede_itself_and_cannot_supersede_a_non_key() {
        let base = "type = book\ntitle = x\nretrieved_on = 2026-09-06\nretrieval_status = full\n";
        assert!(
            parse("K7M2QX4BTF", &format!("{base}replaces = K7M2QX4BTF\n"))
                .expect_err("self reference")
                .contains("supersedes itself")
        );
        assert!(
            parse("K7M2QX4BTF", &format!("{base}replaces = maslow-1965\n"))
                .expect_err("not a key")
                .contains("not a key")
        );
    }

    #[test]
    fn csl_json_carries_the_fields_a_processor_reads() {
        let out = to_csl(&[maslow()]).to_string();
        assert!(out.contains(r#""id":"K7M2QX4BTF""#), "{out}");
        assert!(out.contains(r#""type":"article-journal""#), "{out}");
        assert!(out.contains(r#""family":"Maslow""#), "{out}");
        assert!(out.contains(r#""given":"A. H.""#), "{out}");
        assert!(out.contains(r#""issued":{"date-parts":[[1943]]}"#), "{out}");
        assert!(out.contains(r#""accessed":{"date-parts":[[2026,9,6]]}"#), "{out}");
        assert!(out.contains(r#""container-title":"Psychological Review""#), "{out}");
    }

    /// An institution is not a person, and CSL has a field for saying so. Forcing `ISACA`
    /// into a family name is how a bibliography sorts an organisation under its last word.
    #[test]
    fn an_institutional_author_travels_as_a_literal_name() {
        let mut s = maslow();
        s.authors = vec!["ISACA".into()];
        let out = to_csl(&[s]).to_string();
        assert!(out.contains(r#""literal":"ISACA""#), "{out}");
        assert!(!out.contains("family"), "{out}");
    }

    #[test]
    fn the_replacement_of_a_key_is_the_record_that_names_it() {
        let mut old = maslow();
        old.key = "AAAAAAAAAA".into();
        let mut new = maslow();
        new.key = "BBBBBBBBBB".into();
        new.replaces = Some("AAAAAAAAAA".into());
        let all = vec![old, new];
        assert_eq!(replacement_of(&all, "AAAAAAAAAA").map(|s| s.key.as_str()), Some("BBBBBBBBBB"));
        assert!(replacement_of(&all, "BBBBBBBBBB").is_none());
    }
}
