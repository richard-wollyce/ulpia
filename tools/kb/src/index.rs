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
///
/// **The interrogatives are here as a complete set, and that was a bug fix.** The
/// list carried `qual`, `quais`, `como`, `what` and `how` but not `quem`, `quanto`,
/// `quando`, `onde`, `who`, `when` or `why`, which is half a category and the half
/// that was missing cost real answers twice.
///
/// `memory.rs` already documented the first one without fixing its cause: "quem é
/// você?" survived normalisation as the single term `quem`, and `quem` is common in
/// notes about audience research, so the question came back with marketing
/// psychology. The second was found by the miss log on the day it was built, where
/// `quanto` reached the keyword `K-quant` at 0.857 and would have fired on every
/// question beginning "quanto custa" or "quanto tempo".
///
/// **No threshold separates that pair**, which is why the fix belongs here. A word
/// that opens a question tells you nothing about which file answers it, in either
/// language.
///
/// **The same failure fired a third time on 2026-08-18, from the other half of the
/// category**, the day the boot hook made routing decide who answers. Three real
/// messages in one session: "try it **out**" routed to a recipes file at 28.44 on
/// `out`, "I really **like** what I saw" and "did **you** navigate" routed to a
/// marketing glossary at 38.86 on `like` and `you`, and "**let**'s build our **own**"
/// pulled `let` and `own`. The list had `i`, `we`, `my` and `do` but not `you`,
/// `did`, `like`, `let`, `out` or `own`: pronouns, auxiliaries and particles were
/// each covered by their most common member and no further.
///
/// **IDF makes this worse, not better, and that is the mechanism to remember.**
/// Function words are rare in map keyword lines precisely because nobody writes
/// them as keywords, so document frequency reads them as informative and amplifies
/// exactly the words that carry nothing. A frequency statistic cannot tell rare
/// from meaningless; only a closed-class list can, which is why the fix is this
/// list being actually complete rather than any change to the scoring.
///
/// The set below is the closed classes done whole: pronouns, auxiliaries and
/// modals, articles, prepositions, conjunctions, demonstratives and the common
/// discourse particles, in both languages. Content words stay out of it however
/// common they are, because "memoria" being frequent is information and "you"
/// being frequent is not. Every extension re-runs `kb eval` before it lands;
/// this one was measured at file 18/19 and agent 12/13, unchanged from before
/// the extension, with the three live misroutes all corrected.
const STOPWORDS: &[&str] = &[
    // English: articles, copulas, auxiliaries, modals
    "the", "a", "an", "is", "are", "was", "were", "am", "be", "been", "being",
    "do", "does", "did", "have", "has", "had", "will", "would", "can", "could",
    "should", "shall", "may", "might", "must",
    // English: pronouns and possessives, the whole class this time
    "i", "we", "you", "he", "she", "it", "they", "them", "me", "us", "him", "her",
    "my", "our", "your", "his", "its", "their", "mine", "ours", "yours", "theirs",
    "this", "that", "these", "those",
    // English: prepositions and particles
    "of", "to", "in", "on", "for", "with", "as", "at", "by", "from", "up", "out",
    "off", "over", "under", "into", "about", "after", "before", "between", "through",
    // English: conjunctions, question words, discourse
    "and", "or", "but", "if", "so", "not", "no", "yes", "what", "how", "who",
    "when", "where", "why", "which", "than", "then", "there", "here", "just",
    "also", "too", "very", "now", "let", "lets", "like", "own", "some", "any",
    "all", "more", "most",
    // Portuguese: articles, contractions, prepositions
    "o", "os", "as", "um", "uma", "uns", "umas", "de", "da", "do", "das", "dos",
    "em", "no", "na", "nos", "nas", "para", "por", "com", "sem", "ao", "aos",
    "pelo", "pela", "pelos", "pelas", "num", "numa", "ate", "entre", "sobre",
    // Portuguese: pronouns and possessives, the whole class
    "eu", "voce", "voces", "ele", "ela", "eles", "elas", "te", "me", "mim", "lhe",
    "meu", "minha", "meus", "minhas", "seu", "sua", "seus", "suas", "nosso",
    "nossa", "nossos", "nossas", "teu", "tua", "isso", "isto", "aquilo", "esse",
    "essa", "esses", "essas", "este", "esta", "estes", "estas",
    // Portuguese: copulas, auxiliaries, common light verbs
    "ser", "sou", "era", "foi", "sao", "estou", "estao", "estava", "ter", "tem",
    "tenho", "tinha", "vou", "vai", "vamos", "pode", "posso", "podemos", "deve",
    "devo", "fazer", "faz", "quero", "quer", "dar",
    // Portuguese: conjunctions, question words, discourse
    "e", "ou", "mas", "se", "que", "qual", "quais", "como", "quem", "quanto",
    "quanta", "quantos", "quantas", "quando", "onde", "porque", "nao", "sim",
    "ja", "la", "mais", "menos", "muito", "muita", "muitos", "muitas", "tambem",
    // "ai" is the Portuguese interjection and filler, and it collides head-on with
    // the English initialism AI. Measured 2026-08-19: the message "isso ai", which
    // carries no content at all, scored 34.42 and routed confidently to marketing,
    // because a research note about AI slop matched on those two letters. Dropping
    // it costs the ability to search for "AI" alone, which was never a good search
    // term: it appears in everything and the questions people actually ask say IA,
    // inteligencia artificial, modelo or LLM.
    "ai",
    "so", "apenas", "ainda", "agora", "hoje", "tudo", "todo", "toda", "todos",
    "todas", "outro", "outra", "entao", "assim", "aqui", "ali",
    // **Generic adjectives and ordinals, added 2026-08-20 when the keyword lines widened
    // from a median of six terms to about seventy.** At six terms nobody wrote `Nova York`
    // or `My First Million` or `text-embedding-3-small`, so the components never appeared.
    // At seventy they do, and every component is matched alone: `nova`, `first` and `small`
    // would have been three of the most informative words in the index by IDF, for the same
    // reason the comment above gives, because nobody writes them as keywords deliberately.
    //
    // The alternative was refusing the terms, and that was tried first and was wrong: it
    // threw out proper nouns and terms of art to protect the index from a word. A word that
    // carries nothing belongs on this list; the phrase around it is nobody's problem.
    "new", "old", "first", "last", "next", "small", "big", "large", "good", "bad",
    "best", "worst", "high", "low", "long", "short", "full", "real", "true", "false",
    "novo", "nova", "novos", "novas", "velho", "velha", "primeiro", "primeira",
    "ultimo", "ultima", "proximo", "proxima", "pequeno", "pequena", "grande",
    "grandes", "bom", "boa", "ruim", "melhor", "pior", "alto", "alta", "baixo",
    "baixa", "longo", "curto", "cheio", "vazio", "real", "certo", "errado",
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

    // **The walk is over files, not over map entries, and that is [[0028]].** It used to
    // iterate the base's `MAP.md` and look a file up for each entry, so one file's formatting
    // decided whether another file existed to the router.
    //
    // **This landed only after the keyword lines were widened, and the order was not
    // optional.** Tried first against lines with a median of six terms, it cost seven
    // questions of twenty six at file level, because the map entry's prose body had been
    // carrying the matching surface all along and the curated list was never doing the work.
    // The lines now carry thirty to seventy terms each, in both languages, which is what the
    // list was always supposed to be.
    let mut out = Vec::new();
    for f in &base.files {
        let (keywords, purpose) = header_of(&f.text);
        if keywords.is_empty() {
            continue;
        }
        out.push(Entry {
            base: base_name.clone(),
            rel: f.rel.clone(),
            stem: f.stem.clone(),
            title: first_heading(&f.text),
            summary: purpose.clone(),
            keywords,
            // **What the keyword scorer matches narrows on purpose.** It used to be the whole
            // map entry, prose included, which put the same words in front of both scorers.
            // Agreement between two independent signals is what separates a hit from a guess,
            // and it means nothing if they read the same source.
            body: purpose,
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

/// The keys and the purpose a file declares about itself, per [[0028-a-note-carries-its-own-keys]].
///
/// **Scans only what sits before the first section heading**, and that bound is doing real
/// work rather than being tidy. `MAP.md` carries one `Search for:` line per entry and would
/// otherwise index itself as a file holding every keyword in the base. Its first section
/// heading arrives before its first entry, so the bound excludes it by construction rather
/// than by a filename check that a rename would defeat.
///
/// Tolerant on read, one shape on write. Each tolerance below was a real parse failure
/// before it was a tolerance: the label bolded corrupted the first term, the label lower
/// case yielded an empty list, and a semicolon collapsed the list into one giant keyword.
pub fn header_of(text: &str) -> (Vec<String>, String) {
    let mut keywords = Vec::new();
    let mut purpose = String::new();

    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("##") {
            break;
        }
        if let Some(rest) = labelled(t, &["search for", "buscar por"]) {
            if keywords.is_empty() {
                keywords = terms_in(rest);
            }
        } else if let Some(rest) = labelled(t, &["exists to", "existe para"]) {
            if purpose.is_empty() {
                purpose = rest.trim().trim_end_matches('.').to_string();
            }
        }
    }
    (keywords, purpose)
}

/// The text after a known label, whatever decoration the label is wearing.
fn labelled<'a>(line: &'a str, labels: &[&str]) -> Option<&'a str> {
    let (head, tail) = line.split_once(':')?;
    let head = head.trim().trim_matches(|c| c == '*' || c == '_' || c == ' ');
    let head_lower = head.to_ascii_lowercase();
    if labels.iter().any(|l| head_lower == *l) {
        Some(tail.trim_start_matches('*').trim())
    } else {
        None
    }
}

/// One list of terms, however it was punctuated.
fn terms_in(rest: &str) -> Vec<String> {
    rest.split([',', ';'])
        .map(|t| {
            t.trim()
                .trim_start_matches(['-', '*', '+', ' '])
                .trim_matches(|c: char| c == '`' || c == '"' || c == '*' || c == '.' || c == ' ')
                .to_string()
        })
        .filter(|t| !t.is_empty())
        .collect()
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

// ---------------------------------------------------------------------------
// Suggesting, which is what the base says when it matched nothing
// ---------------------------------------------------------------------------

/// How alike two words have to look before the base offers one for the other.
///
/// Measured against the real pairs of 2026-08-17, not chosen: `publico` against
/// `public` scores 0.89 and `repositorio` against `repository` 0.74, which are the
/// cognates this exists to catch. The floor sits at 0.65 because of what a lower
/// one let through: at 0.5, `quando` reached `K-quant` with 0.571 on a shared
/// prefix, and a single noisy suggestion offered on its own reads as an answer
/// rather than as a candidate.
///
/// **This is tuned against a handful of pairs from one session and is the weakest
/// number in the file.** Boundary padding, the usual fix for short word noise, was
/// tried on paper and is wrong here: it rewards a shared prefix, which is exactly
/// what the false positive had, taking `quando` against `K-quant` up to 0.615.
const SUGGEST_FLOOR: f32 = 0.65;

/// Character trigrams, with the whole word kept when it is too short to have any.
fn trigrams(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 3 {
        return vec![word.to_string()];
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Dice coefficient over character trigrams: how alike two words *look*.
///
/// Deliberately a measure of spelling and not of meaning. It is what separates the
/// half of expansion that is plain software from the half that needs a model, and
/// each half should say which one it is rather than blur into the other.
fn look_alike(a: &str, b: &str) -> f32 {
    let (ta, tb) = (trigrams(a), trigrams(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let mut unmatched = tb.clone();
    let mut shared = 0usize;
    for g in &ta {
        if let Some(pos) = unmatched.iter().position(|h| h == g) {
            unmatched.remove(pos);
            shared += 1;
        }
    }
    (2 * shared) as f32 / (ta.len() + tb.len()) as f32
}

/// The words the base actually knows that look like the words the question used.
///
/// **This is the base answering a miss with its own vocabulary instead of with a
/// dead end.** The old miss message said the keyword lines might not carry the
/// words a real question uses, which is true and leaves the caller nothing to act
/// on: a model reading it can only guess new words blind, and a person cannot see
/// the keyword space at all.
///
/// It closes exactly one of the two halves of [[0006-language-architecture]]'s step
/// 2, and closes it without a model. A typo and a cognate are *orthographic*
/// distance, which trigrams measure directly: `repositorio` reaches `repository`
/// here, and so does `repositry`. A real translation is *semantic* distance, which
/// no string measure reaches, so `nunca` will never find `never` in this function
/// and is not supposed to. That half belongs to whatever model is in the loop, and
/// what this returns is the candidate space it expands against.
///
/// Scored on the **best single alignment**, not on every word of a term finding a
/// partner. The strict version was written first and was wrong: `ingestao` scores
/// 0.80 against `ingest` and nothing against `source`, so requiring both took the
/// term `ingest a source` down to 0.40 and hid the one suggestion the question
/// deserved.
///
/// The rule that produced that mistake was borrowed from matching, where precision
/// is what matters because the result is presented as an answer. **Suggesting is the
/// opposite trade**: the reader filters, so a term worth looking at should be shown,
/// and the strictness belongs in the per word floor instead. `single channel`
/// ranking on `channel` alone is the intended behaviour here.
pub fn suggest(question: &str, entries: &[Entry], limit: usize) -> Vec<String> {
    let asked = normalise(question);
    if asked.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(String, f32)> = Vec::new();
    for entry in entries {
        for keyword in &entry.keywords {
            let score = normalise(keyword)
                .iter()
                .flat_map(|part| asked.iter().map(move |w| look_alike(w, part)))
                .fold(0.0f32, f32::max);

            if score >= SUGGEST_FLOOR && !scored.iter().any(|(k, _)| k == keyword) {
                scored.push((keyword.clone(), score));
            }
        }
    }

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.truncate(limit);
    scored.into_iter().map(|(k, _)| k).collect()
}

/// Expands a question with canonical terms from the alias table.
///
/// Additive on purpose: the original words are always kept, so a wrong alias can
/// only add noise and can never remove signal. Replacing the query would make a
/// bad translation silently fatal.
/// Normalises a question and expands it through the alias table.
///
/// Public because both scorers have to see the same terms. Expanding for the keyword
/// scorer and not for full text was a real bug: "qual o piso calorico" routed
/// correctly by keyword and matched zero chunks by text, because `calorie floor` was
/// never substituted on the text side.
pub fn expand_query(query: &str, aliases: &[(String, String)]) -> Vec<String> {
    expand(&normalise(query), aliases)
}

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

    /// The case this exists for, taken from a real miss: a Portuguese question
    /// reaching an English keyword that no alias covers, with no model involved.
    #[test]
    fn a_cognate_reaches_the_keyword_it_looks_like() {
        let entries = vec![entry("ingestion", "Ingestion", &["ingest a source", "distill"])];
        let out = suggest("o que e um protocolo de ingestao", &entries, 5);
        assert_eq!(out, vec!["ingest a source"]);
    }

    #[test]
    fn a_typo_reaches_the_word_it_meant() {
        let entries = vec![entry("layout", "Fleet layout", &["repository", "tenancy"])];
        let out = suggest("onde fica o repositry", &entries, 5);
        assert_eq!(out, vec!["repository"]);
    }

    /// The boundary that has to hold, because the reply claims it.
    ///
    /// `nunca` and `never` mean the same thing and look nothing alike. If a
    /// spelling measure ever returned this pair it would be by accident, and the
    /// message printed alongside would become a lie.
    #[test]
    fn a_translation_is_never_suggested() {
        let entries = vec![entry("limits", "Limits", &["never", "stop and ask"])];
        assert!(suggest("o que voce nunca faz", &entries, 5).is_empty());
    }

    /// A shared prefix on short words is the noise this floor exists to exclude.
    /// Measured at 0.571, which is why the floor is not 0.5.
    #[test]
    fn a_shared_prefix_on_short_words_is_not_a_suggestion() {
        let entries = vec![entry("quant", "Quantization", &["K-quant"])];
        assert!(suggest("o que acontece quando falha", &entries, 5).is_empty());
    }

    /// Found by the miss log on the day it was built. `quanto` against `K-quant`
    /// scores 0.857, which no floor excludes without excluding real cognates too,
    /// so the fix is that a word opening a question is not a routing term at all.
    #[test]
    fn an_interrogative_never_becomes_a_suggestion() {
        let entries = vec![entry("quant", "Quantization", &["K-quant"])];
        for question in [
            "quanto custa hospedar isso na nuvem",
            "quanto tempo devo esperar",
            "quando isso acontece",
            "quem e voce",
        ] {
            assert!(
                suggest(question, &entries, 5).is_empty(),
                "an interrogative carries no signal: {question}"
            );
        }
    }

    #[test]
    fn a_term_ranks_on_its_best_word_rather_than_on_all_of_them() {
        let entries = vec![entry("ram", "RAM", &["single channel"])];
        let out = suggest("o que e um channel", &entries, 5);
        assert_eq!(out, vec!["single channel"], "one word aligning is enough to offer it");
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
