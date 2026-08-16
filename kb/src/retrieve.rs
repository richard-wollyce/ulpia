//! Fusing the two scorers into passages a caller can actually read.
//!
//! This used to live in `main.rs` as a private function that returned display
//! strings, which was fine while the only caller was a terminal. It is not fine for
//! a tool surface: a model asking what the base says needs the passage, its heading
//! path and where it came from, not a fourteen token snippet with the heading glued
//! on by a `": "`.
//!
//! The chunk text was already being read out of SQLite and thrown away one line
//! later. Nothing here fetches anything new; it stops discarding.

use std::collections::{HashMap, HashSet};

use crate::index;
use crate::store;

/// Reciprocal Rank Fusion constant. 60 is the value from the original paper and
/// there is no corpus specific tuning behind it, which is the point: RRF compares
/// positions, so it needs no conversion factor between a BM25 value and a keyword
/// score, and therefore has nothing to tune per base.
pub const RRF_K: f64 = 60.0;

/// How far past `top` each scorer is asked to look. Fusion can only choose from what
/// it is given, so both lists are oversampled. They differ because the text side
/// ranks chunks and several chunks can belong to one file.
pub const KEYWORD_OVERSAMPLE: usize = 4;
pub const TEXT_OVERSAMPLE: usize = 6;

/// One matching chunk, with everything needed to quote it and say where it came from.
#[derive(Debug, Clone)]
pub struct Passage {
    pub heading_path: String,
    /// The whole chunk, up to the chunker's ceiling. This is the field that did not
    /// used to survive fusion.
    pub text: String,
    /// The FTS5 snippet, about a line. Useful for a list, useless as evidence.
    pub excerpt: String,
    pub provenance: Option<String>,
    pub stage: Option<String>,
}

/// One file, with the passages that matched inside it.
#[derive(Debug, Clone)]
pub struct Retrieved {
    pub base: String,
    pub path: String,
    /// From the map entry, when the keyword scorer ranked this file. Empty when only
    /// the text scorer found it.
    pub title: String,
    pub score: f64,
    /// Which scorer ranked this file and at what position, kept so a bad ranking can
    /// be diagnosed instead of guessed at.
    pub why: Vec<String>,
    /// The query words that matched on the keyword side.
    pub matched: Vec<String>,
    /// Empty when the file was ranked by keywords alone, which happens when the map
    /// entry describes a file whose text does not repeat the question's words.
    pub passages: Vec<Passage>,
}

/// Reciprocal Rank Fusion over the two scorers.
///
/// Each list contributes `1 / (K + rank)` to every document it ranks, and the sums
/// are compared. A document both scorers like beats one that either likes a lot,
/// which is the behaviour we want: the hand written keywords carry intent, the full
/// text carries the actual words, and agreement between them is the strongest signal
/// available without a model.
pub fn fuse(keyword: &[index::Hit], text: &[store::Hit], top: usize) -> Vec<Retrieved> {
    let mut acc: HashMap<(String, String), Retrieved> = HashMap::new();

    for (rank, hit) in keyword.iter().enumerate() {
        // A map entry naming a `[[note]]` with no file behind it produces an Entry
        // with an empty path. Emitting it would offer the caller a file that is not
        // there; the broken link is `kb check`'s problem, not the router's.
        if hit.entry.rel.is_empty() {
            continue;
        }
        let key = (hit.entry.base.clone(), hit.entry.rel.clone());
        let entry = acc.entry(key.clone()).or_insert_with(|| blank(&key));
        entry.score += 1.0 / (RRF_K + rank as f64 + 1.0);
        entry.why.push(format!("keywords #{}", rank + 1));
        entry.title = hit.entry.title.clone();
        for word in &hit.matched {
            if !entry.matched.contains(word) {
                entry.matched.push(word.clone());
            }
        }
    }

    // The text list ranks chunks, and RRF ranks documents. A file contributes to the
    // SCORE once, at the rank of its best chunk.
    //
    // Getting this wrong is subtle and it happened: scoring every matching chunk made
    // a long file accumulate one contribution per section, so the ranking quietly
    // became "which file has the most matching pieces" rather than "which file ranked
    // highest". Yaron's safety protocol, which states the calorie floor in one table
    // row, lost to a longer file that mentioned it three times in passing.
    //
    // The passages are a different question and take the opposite answer: a caller
    // reading the file wants every matching section, not just the best one. So
    // `counted` gates the score and nothing else. Collapsing the two again is the way
    // this bug comes back.
    let mut counted: HashSet<(String, String)> = HashSet::new();

    for (rank, hit) in text.iter().enumerate() {
        let key = (hit.base.clone(), hit.path.clone());
        let entry = acc.entry(key.clone()).or_insert_with(|| blank(&key));

        if counted.insert(key) {
            entry.score += 1.0 / (RRF_K + rank as f64 + 1.0);
            entry.why.push(format!("text #{}", rank + 1));
        }

        entry.passages.push(Passage {
            heading_path: hit.heading_path.clone(),
            text: hit.text.clone(),
            excerpt: hit.excerpt.clone(),
            provenance: hit.provenance.clone(),
            stage: hit.stage.clone(),
        });
    }

    let mut out: Vec<Retrieved> = acc.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.path.cmp(&b.path))
    });
    out.truncate(top);
    out
}

fn blank(key: &(String, String)) -> Retrieved {
    Retrieved {
        base: key.0.clone(),
        path: key.1.clone(),
        title: String::new(),
        score: 0.0,
        why: Vec::new(),
        matched: Vec::new(),
        passages: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Entry;

    fn entry(base: &str, rel: &str, title: &str) -> Entry {
        Entry {
            base: base.into(),
            rel: rel.into(),
            stem: rel.rsplit('/').next().unwrap().trim_end_matches(".md").into(),
            title: title.into(),
            keywords: Vec::new(),
            summary: String::new(),
            body: String::new(),
        }
    }

    fn shit(base: &str, path: &str, heading: &str, text: &str) -> store::Hit {
        store::Hit {
            base: base.into(),
            path: path.into(),
            heading_path: heading.into(),
            excerpt: text.chars().take(20).collect(),
            text: text.into(),
            provenance: Some("agent".into()),
            stage: Some("distilled".into()),
        }
    }

    /// The regression this whole module was extracted to make testable. A long file
    /// matching in three sections must not outscore a short file that states the
    /// fact once and ranked higher.
    #[test]
    fn a_file_contributes_to_the_score_once_however_many_chunks_match() {
        let text = vec![
            shit("yaron", "protocols/safety.md", "Limits", "the calorie floor is 1500"),
            shit("yaron", "recipes/long.md", "A", "calorie"),
            shit("yaron", "recipes/long.md", "B", "calorie"),
            shit("yaron", "recipes/long.md", "C", "calorie"),
        ];
        let out = fuse(&[], &text, 5);

        assert_eq!(out[0].path, "protocols/safety.md", "best rank wins, not most chunks");
        let long = out.iter().find(|r| r.path == "recipes/long.md").expect("present");
        assert_eq!(long.why.len(), 1, "one file, one score contribution");
    }

    /// The other half of the same rule, and the reason it is easy to get wrong: the
    /// score is capped at one contribution, the passages are not capped at all.
    #[test]
    fn every_matching_chunk_still_comes_back_as_a_passage() {
        let text = vec![
            shit("yaron", "recipes/long.md", "A", "first matching section"),
            shit("yaron", "recipes/long.md", "B", "second matching section"),
            shit("yaron", "recipes/long.md", "C", "third matching section"),
        ];
        let out = fuse(&[], &text, 5);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].passages.len(), 3, "a reader wants every section, not the best one");
        assert_eq!(out[0].passages[1].heading_path, "B");
        assert!(out[0].passages[0].text.contains("first matching section"));
    }

    #[test]
    fn provenance_travels_with_the_passage() {
        let out = fuse(&[], &[shit("zed", "knowledge/a.md", "H", "vulkan")], 5);
        assert_eq!(out[0].passages[0].provenance.as_deref(), Some("agent"));
        assert_eq!(out[0].passages[0].stage.as_deref(), Some("distilled"));
    }

    /// A map entry pointing at a file that does not exist has an empty path. Offering
    /// it would hand the caller something it cannot open.
    #[test]
    fn a_broken_map_entry_is_not_offered() {
        let entries = vec![entry("zed", "", "Ghost")];
        let keyword: Vec<index::Hit> = entries
            .iter()
            .map(|e| index::Hit { entry: e, score: 1.0, matched: vec!["ghost".into()] })
            .collect();
        assert!(fuse(&keyword, &[], 5).is_empty());
    }

    /// Agreement between two independent scorers is the strongest signal available
    /// without a model, so it has to beat a strong showing in either one alone.
    #[test]
    fn agreement_between_the_two_scorers_beats_either_alone() {
        let entries = vec![entry("zed", "knowledge/both.md", "Both"), entry("zed", "knowledge/kw.md", "Kw")];
        let keyword: Vec<index::Hit> = vec![
            index::Hit { entry: &entries[1], score: 9.0, matched: vec!["a".into()] },
            index::Hit { entry: &entries[0], score: 1.0, matched: vec!["a".into()] },
        ];
        let text = vec![shit("zed", "knowledge/both.md", "H", "a")];

        let out = fuse(&keyword, &text, 5);
        assert_eq!(out[0].path, "knowledge/both.md");
        assert_eq!(out[0].why.len(), 2, "both scorers are recorded, so the ranking is explainable");
    }

    #[test]
    fn matched_words_survive_fusion() {
        let entries = vec![entry("zed", "knowledge/a.md", "A")];
        let keyword = vec![index::Hit {
            entry: &entries[0],
            score: 1.0,
            matched: vec!["vulkan".into(), "prefill".into()],
        }];
        let out = fuse(&keyword, &[], 5);
        assert_eq!(out[0].matched, vec!["vulkan", "prefill"]);
        assert_eq!(out[0].title, "A");
    }
}
