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

pub struct Entry {
    /// Which agent this belongs to, taken from the base directory name.
    pub base: String,
    pub rel: String,
    pub stem: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub summary: String,
}

pub struct Hit<'a> {
    pub entry: &'a Entry,
    pub score: u32,
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
            summary: summarise(&entry.body),
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

/// The entry text without its keyword line, flattened and trimmed.
fn summarise(body: &str) -> String {
    let text: Vec<&str> = body
        .lines()
        .take_while(|l| !(l.contains("Search for:") || l.contains("Buscar por:")))
        .collect();
    let joined = text.join(" ");
    let cleaned: String = joined
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    cleaned.chars().take(400).collect()
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
const W_KEYWORD: u32 = 6;
const W_PHRASE: u32 = 10;
const W_TITLE: u32 = 3;
const W_STEM: u32 = 3;
const W_SUMMARY: u32 = 1;

pub fn route<'a>(query: &str, entries: &'a [Entry], top: usize) -> Vec<Hit<'a>> {
    let terms = normalise(query);
    let query_norm = terms.join(" ");

    let mut hits: Vec<Hit<'a>> = Vec::new();

    for entry in entries {
        let mut score = 0u32;
        let mut matched: Vec<String> = Vec::new();

        // A multi word keyword appearing whole in the question is the strongest
        // signal there is, so it is scored before the individual words.
        for keyword in &entry.keywords {
            let norm = normalise(keyword).join(" ");
            if norm.split_whitespace().count() > 1 && !norm.is_empty() && query_norm.contains(&norm) {
                score += W_PHRASE;
                matched.push(keyword.clone());
            }
        }

        let keyword_words: Vec<String> = entry
            .keywords
            .iter()
            .flat_map(|k| normalise(k))
            .collect();
        let title_words = normalise(&entry.title);
        let stem_words = normalise(&entry.stem);
        let summary_words = normalise(&entry.summary);

        for term in &terms {
            let mut hit = false;
            if keyword_words.contains(term) {
                score += W_KEYWORD;
                hit = true;
            }
            if title_words.contains(term) {
                score += W_TITLE;
                hit = true;
            }
            if stem_words.contains(term) {
                score += W_STEM;
                hit = true;
            }
            if summary_words.contains(term) {
                score += W_SUMMARY;
                hit = true;
            }
            if hit && !matched.iter().any(|m| m == term) {
                matched.push(term.clone());
            }
        }

        if score > 0 {
            hits.push(Hit { entry, score, matched });
        }
    }

    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.entry.rel.cmp(&b.entry.rel)));
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
        }
    }

    #[test]
    fn routes_to_the_entry_whose_keywords_match() {
        let entries = vec![
            entry("quantization", "Quantization", &["q4_K_M", "bits per weight"]),
            entry("the-bar", "The bar", &["garbage", "weak decision"]),
        ];
        let hits = route("o que e um weak decision", &entries, 5);
        assert_eq!(hits[0].entry.stem, "the-bar");
    }

    #[test]
    fn a_whole_phrase_outranks_scattered_words() {
        let entries = vec![
            entry("a", "A", &["memory bandwidth"]),
            entry("b", "B", &["memory", "bandwidth"]),
        ];
        let hits = route("explique memory bandwidth", &entries, 5);
        assert_eq!(hits[0].entry.stem, "a");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn a_question_about_nothing_in_the_base_returns_nothing() {
        let entries = vec![entry("quantization", "Quantization", &["q4_K_M"])];
        assert!(route("qual a capital da australia", &entries, 5).is_empty());
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
        }];
        let json = to_json(&e);
        assert!(json.contains("\\\"hi\\\""));
        assert!(json.contains("\\n"));
    }
}
