//! The derived index, and routing on top of it.
//!
//! [ADR-0003](../../../decisions/0003-knowledge-storage.md) says the files are the source of truth and
//! any index is derived and disposable. This builds that index in memory on every run, because
//! rebuilding it takes milliseconds and a cache that can go stale is a second source of truth.
//!
//! [ADR-0005](../../../decisions/0005-wake-with-the-constitution.md) says routing starts with no model
//! at all. The `Search for:` line on every map entry is already a keyword index, written by hand by
//! whoever knew the file best. Scoring a question against it is a lookup: instant, free, explainable,
//! and incapable of inventing a file that does not exist.

use crate::base::Base;
use crate::checks::map_entries;
use std::collections::HashMap;

pub struct Entry {
    /// Which agent this belongs to, taken from the base directory name.
    pub base: String,
    pub rel: String,
    pub stem: String,
    pub title: String,
    pub keywords: Vec<String>,
    /// Truncated, for display and for the JSON index.
    pub summary: String,
    /// The whole entry body, for matching.
    ///
    /// Kept separate because the two jobs conflict: a display summary wants to be
    /// short, and truncating what gets matched silently makes a file unfindable by
    /// the words that happened to fall past the cut. That is exactly how
    /// `formulas` lost to `rotulos-analisados` on a question about protein per
    /// meal: "refeicao" was in its entry, just past character 400.
    pub body: String,
}

pub struct Hit<'a> {
    pub entry: &'a Entry,
    pub score: f32,
    /// The query words that matched, so a bad ranking can be diagnosed instead of guessed at.
    pub matched: Vec<String>,
}

/// Words too common to carry meaning, in both languages the fleet uses.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "of", "to", "in", "on", "for", "and", "or", "is", "are", "it", "that", "this",
    "with", "what", "how", "do", "does", "i", "we", "my", "our", "be", "as", "at", "by", "from",
    "o", "os", "as", "um", "uma", "de", "da", "do", "das", "dos", "em", "no", "na", "nos", "nas",
    "para", "por", "com", "que", "qual", "quais", "como", "e", "ou", "se", "sobre", "eu", "voce",
    "meu", "minha", "nosso", "nossa", "ser", "esta", "isso", "sem", "ao", "aos",
];

// ---------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------

pub fn build(base: &Base) -> Vec<Entry> {
    let base_name = base
        .root
        .canonicalize()
        .unwrap_or_else(|_| base.root.clone())
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| base.root.display().to_string());

    let map = match base.map_file() {
        Some(m) => m,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    for entry in map_entries(&map.text) {
        let file = base.files.iter().find(|f| f.stem == entry.name);
        let (rel, title) = match file {
            Some(f) => (f.rel.clone(), first_heading(&f.text)),
            None => (String::new(), String::new()),
        };
        out.push(Entry {
            base: base_name.clone(),
            rel,
            stem: entry.name.clone(),
            title,
            keywords: keywords_in(&entry.body),
            summary: summarise(&entry.body, 400),
            body: summarise(&entry.body, usize::MAX),
        });
    }
    out
}

/// The first markdown heading, skipping front matter.
fn first_heading(text: &str) -> String {
    let mut in_front_matter = false;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if i == 0 && trimmed == "---" {
            in_front_matter = true;
            continue;
        }
        if in_front_matter {
            if trimmed == "---" {
                in_front_matter = false;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return rest.trim().to_string();
        }
    }
    String::new()
}

/// Terms from the `Search for:` or `Buscar por:` line, which may wrap over several lines.
pub fn keywords_in(body: &str) -> Vec<String> {
    let lines: Vec<&str> = body.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains("Search for:") || l.contains("Buscar por:"));

    let start = match start {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut collected = String::new();
    for line in &lines[start..] {
        let piece = match line.split_once(':') {
            Some((head, tail)) if head.contains("Search for") || head.contains("Buscar por") => tail,
            _ => line,
        };
        collected.push(' ');
        collected.push_str(piece);
    }

    collected
        .split(',')
        .map(|term| term.trim().trim_matches(|c| c == '`' || c == '.' || c == ' ').to_string())
        .filter(|term| !term.is_empty())
        .collect()
}

/// The entry text without its keyword line, flattened and trimmed to `limit` chars.
fn summarise(body: &str, limit: usize) -> String {
    let text: Vec<&str> = body
        .lines()
        .take_while(|l| !(l.contains("Search for:") || l.contains("Buscar por:")))
        .collect();
    let joined = text.join(" ");
    let cleaned: String = joined
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    cleaned.chars().take(limit).collect()
}

// ---------------------------------------------------------------------------
// Normalising
// ---------------------------------------------------------------------------

/// Lowercases, strips accents, and splits into meaningful words.
///
/// Accent folding matters because half the fleet writes Portuguese: without it,
/// "calculo" would not match "cálculo" and the lookup would fail silently, which
/// is the failure mode this whole tool exists to prevent.
pub fn normalise(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        let folded = fold(c);
        if folded.is_alphanumeric() {
            current.push(folded.to_ascii_lowercase());
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    words
        .into_iter()
        .filter(|w| w.len() > 1 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'ã' | 'â' | 'ä' | 'Á' | 'À' | 'Ã' | 'Â' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'õ' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Õ' | 'Ô' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

/// Weights, in the order they should matter.
///
/// Keywords are hand written for exactly this purpose, so they carry the most.
/// The title and the file name are close behind because they were also chosen
/// deliberately. The summary is prose and matches things incidentally, so it
/// only breaks ties.
const W_KEYWORD: f32 = 6.0;
const W_PHRASE: f32 = 10.0;
const W_TITLE: f32 = 3.0;
const W_STEM: f32 = 3.0;
const W_SUMMARY: f32 = 1.0;

struct Prepared<'a> {
    entry: &'a Entry,
    keywords: Vec<String>,
    /// Multi word keywords, kept as (original, normalised) for phrase matching.
    phrases: Vec<(String, String)>,
    title: Vec<String>,
    stem: Vec<String>,
    summary: Vec<String>,
}

impl<'a> Prepared<'a> {
    fn new(entry: &'a Entry) -> Self {
        let phrases = entry
            .keywords
            .iter()
            .map(|k| (k.clone(), normalise(k).join(" ")))
            .filter(|(_, n)| n.split_whitespace().count() > 1)
            .collect();

        Prepared {
            entry,
            keywords: entry.keywords.iter().flat_map(|k| normalise(k)).collect(),
            phrases,
            title: normalise(&entry.title),
            stem: normalise(&entry.stem),
            summary: normalise(&entry.body),
        }
    }

    /// Every distinct word this entry can be found by, for document frequency.
    fn vocabulary(&self) -> Vec<&String> {
        let mut all: Vec<&String> = self
            .keywords
            .iter()
            .chain(self.title.iter())
            .chain(self.stem.iter())
            .chain(self.summary.iter())
            .collect();
        all.sort();
        all.dedup();
        all
    }
}

/// Inverse document frequency: how much a word discriminates between entries.
///
/// Without it, "proteina" scores the same on the one file that computes protein
/// per meal and on every other file in a nutrition base that merely mentions it,
/// and the tie is broken by accident. A word that appears everywhere carries
/// almost no information about which file to open, and this is the standard way
/// to say so in arithmetic.
fn idf(term: &str, df: &HashMap<String, usize>, total: usize) -> f32 {
    let seen = *df.get(term).unwrap_or(&0) as f32;
    (1.0 + (total as f32 / (1.0 + seen))).ln()
}

/// Expands a question with canonical terms from the alias table.
///
/// Additive on purpose: the original words are always kept, so a wrong alias can
/// only add noise and can never remove signal. Replacing the query would make a
/// bad translation silently fatal.
fn expand(terms: &[String], aliases: &[(String, String)]) -> Vec<String> {
    let query_norm = terms.join(" ");
    let mut out = terms.to_vec();

    for (alias, canonical) in aliases {
        let alias_norm = normalise(alias).join(" ");
        if alias_norm.is_empty() {
            continue;
        }
        let matches = if alias_norm.split_whitespace().count() > 1 {
            query_norm.contains(&alias_norm)
        } else {
            terms.iter().any(|t| t == &alias_norm)
        };
        if matches {
            for word in normalise(canonical) {
                if !out.contains(&word) {
                    out.push(word);
                }
            }
        }
    }
    out
}

pub fn route<'a>(
    query: &str,
    entries: &'a [Entry],
    aliases: &[(String, String)],
    top: usize,
) -> Vec<Hit<'a>> {
    let prepared: Vec<Prepared> = entries.iter().map(Prepared::new).collect();

    let mut df: HashMap<String, usize> = HashMap::new();
    for p in &prepared {
        for word in p.vocabulary() {
            *df.entry(word.clone()).or_insert(0) += 1;
        }
    }
    let total = prepared.len();

    let terms = expand(&normalise(query), aliases);
    let query_norm = terms.join(" ");

    let mut hits: Vec<Hit<'a>> = Vec::new();

    for p in &prepared {
        let mut score = 0.0f32;
        let mut matched: Vec<String> = Vec::new();

        // A multi word keyword appearing whole in the question is the strongest
        // signal there is, so it is scored before the individual words.
        for (original, norm) in &p.phrases {
            if query_norm.contains(norm) {
                let weight: f32 = norm
                    .split_whitespace()
                    .map(|w| idf(w, &df, total))
                    .sum::<f32>()
                    / norm.split_whitespace().count().max(1) as f32;
                score += W_PHRASE * weight;
                matched.push(original.clone());
            }
        }

        for term in &terms {
            let weight = idf(term, &df, total);
            let mut hit = false;
            if p.keywords.contains(term) {
                score += W_KEYWORD * weight;
                hit = true;
            }
            if p.title.contains(term) {
                score += W_TITLE * weight;
                hit = true;
            }
            if p.stem.contains(term) {
                score += W_STEM * weight;
                hit = true;
            }
            if p.summary.contains(term) {
                score += W_SUMMARY * weight;
                hit = true;
            }
            if hit && !matched.iter().any(|m| m == term) {
                matched.push(term.clone());
            }
        }

        if score > 0.0 {
            hits.push(Hit { entry: p.entry, score, matched });
        }
    }

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.entry.rel.cmp(&b.entry.rel))
    });
    hits.truncate(top);
    hits
}

// ---------------------------------------------------------------------------
// JSON, hand written because one escape function is cheaper than a dependency
// ---------------------------------------------------------------------------

pub fn to_json(entries: &[Entry]) -> String {
    let mut out = String::from("[\n");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str("  {");
        out.push_str(&format!("\"base\":{},", esc(&e.base)));
        out.push_str(&format!("\"path\":{},", esc(&e.rel)));
        out.push_str(&format!("\"name\":{},", esc(&e.stem)));
        out.push_str(&format!("\"title\":{},", esc(&e.title)));
        out.push_str("\"keywords\":[");
        for (j, k) in e.keywords.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&esc(k));
        }
        out.push_str("],");
        out.push_str(&format!("\"summary\":{}", esc(&e.summary)));
        out.push('}');
    }
    out.push_str("\n]\n");
    out
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_folds_accents_and_drops_stopwords() {
        let words = normalise("Qual e o cálculo da proteína?");
        assert!(words.contains(&"calculo".to_string()));
        assert!(words.contains(&"proteina".to_string()));
        assert!(!words.contains(&"da".to_string()));
    }

    #[test]
    fn normalise_drops_single_characters() {
        assert!(normalise("a b index").iter().all(|w| w.len() > 1));
    }

    #[test]
    fn keywords_come_off_the_search_line() {
        let body = "- **[[note]]** does a thing.\n  Search for: `one`, `two words`, `three`.";
        let k = keywords_in(body);
        assert_eq!(k, vec!["one", "two words", "three"]);
    }

    #[test]
    fn keywords_wrap_across_lines() {
        let body = "- **[[note]]** x.\n  Search for: `alpha`,\n  `beta`, `gamma`.";
        let k = keywords_in(body);
        assert_eq!(k, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn portuguese_keyword_line_is_read_too() {
        let body = "- **[[nota]]** faz algo.\n  Buscar por: `piso calorico`, `bandeira vermelha`.";
        assert_eq!(keywords_in(body), vec!["piso calorico", "bandeira vermelha"]);
    }

    #[test]
    fn no_keyword_line_gives_no_keywords() {
        assert!(keywords_in("- **[[note]]** just prose.").is_empty());
    }

    fn entry(stem: &str, title: &str, keywords: &[&str]) -> Entry {
        Entry {
            base: "zed".into(),
            rel: format!("knowledge/{stem}.md"),
            stem: stem.into(),
            title: title.into(),
            keywords: keywords.iter().map(|k| k.to_string()).collect(),
            summary: String::new(),
            body: String::new(),
        }
    }

    #[test]
    fn routes_to_the_entry_whose_keywords_match() {
        let entries = vec![
            entry("quantization", "Quantization", &["q4_K_M", "bits per weight"]),
            entry("the-bar", "The bar", &["garbage", "weak decision"]),
        ];
        let hits = route("o que e um weak decision", &entries, &[], 5);
        assert_eq!(hits[0].entry.stem, "the-bar");
    }

    #[test]
    fn a_whole_phrase_outranks_scattered_words() {
        let entries = vec![
            entry("a", "A", &["memory bandwidth"]),
            entry("b", "B", &["memory", "bandwidth"]),
        ];
        let hits = route("explique memory bandwidth", &entries, &[], 5);
        assert_eq!(hits[0].entry.stem, "a");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn a_question_about_nothing_in_the_base_returns_nothing() {
        let entries = vec![entry("quantization", "Quantization", &["q4_K_M"])];
        assert!(route("qual a capital da australia", &entries, &[], 5).is_empty());
    }

    /// The Z11 failure, reproduced: a word every entry shares must not decide the
    /// ranking. Only the rare word carries information about which file to open.
    #[test]
    fn a_common_word_loses_to_a_rare_one() {
        let entries = vec![
            entry("labels", "Labels analysed", &["protein", "sodium", "price per gram"]),
            entry("formulas", "Formulas", &["protein", "protein per meal", "leucine"]),
            entry("training", "Training", &["protein", "volume", "sets"]),
            entry("sleep", "Sleep", &["protein", "circadian", "caffeine"]),
        ];
        let hits = route("quanta protein per meal eu preciso", &entries, &[], 5);
        assert_eq!(hits[0].entry.stem, "formulas");
    }

    #[test]
    fn an_alias_bridges_a_question_asked_in_the_other_language() {
        let entries = vec![
            entry("compliance", "Compliance", &["personal attributes", "policy"]),
            entry("hooks", "Hooks", &["hook", "opening line"]),
        ];
        let aliases = vec![("atributos pessoais".to_string(), "personal attributes".to_string())];

        assert!(route("meu anuncio tem atributos pessoais", &entries, &[], 5).is_empty());

        let hits = route("meu anuncio tem atributos pessoais", &entries, &aliases, 5);
        assert_eq!(hits[0].entry.stem, "compliance");
    }

    #[test]
    fn expansion_adds_and_never_replaces() {
        let terms = normalise("preciso do hook certo");
        let aliases = vec![("hook".to_string(), "gancho".to_string())];
        let out = expand(&terms, &aliases);
        assert!(out.contains(&"hook".to_string()), "original survives");
        assert!(out.contains(&"gancho".to_string()), "canonical added");
    }

    #[test]
    fn json_escapes_quotes_and_newlines() {
        let e = vec![Entry {
            base: "zed".into(),
            rel: "a.md".into(),
            stem: "a".into(),
            title: "he said \"hi\"\nthen left".into(),
            keywords: vec![],
            summary: String::new(),
            body: String::new(),
        }];
        let json = to_json(&e);
        assert!(json.contains("\\\"hi\\\""));
        assert!(json.contains("\\n"));
    }
}
