//! LongMemEval, end to end: their chat histories become a fleet, the fleet answers.
//!
//! ## The shape
//!
//! LongMemEval hands each instance a question and a haystack of timestamped chat
//! sessions, and grades a free-text answer. Ulpia's side of that: the **converter**
//! turns each instance's sessions into a one-agent fleet of markdown memory files, the
//! same shape a person's fleet has, and then the shipped pipeline runs unmodified:
//! `kb`'s own walker, index, scorers, verdict, and the `kb answer` grounding rules,
//! through the library, not a reimplementation. What is benchmarked is the product.
//!
//! ## The keys are mechanical, and that is stated rather than hidden
//!
//! A real fleet's `Search for:` lines are authored. Benchmark ingestion cannot afford
//! an author per session, so keys here are the session's own most frequent surviving
//! words and bigrams, filtered by the same survival rules the router enforces. That is
//! the weakest honest ingestion, deliberately: every point scored on top of mechanical
//! keys is a floor, and an authored fleet only does better.
//!
//! ## Judging
//!
//! The official protocol judges with GPT-4o. This harness produces the official
//! hypotheses JSONL for anyone to grade with the official script, and can also judge
//! locally through any judge command (a Claude model here), clearly labelled: a score
//! judged by a different model is not comparable to the leaderboard until re-judged by
//! the official protocol.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use kb::classify::Classifier;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Instance {
    pub question_id: String,
    #[serde(default)]
    pub question_type: String,
    pub question: String,
    #[serde(default)]
    pub answer: serde_json::Value,
    #[serde(default)]
    pub question_date: String,
    #[serde(default)]
    pub haystack_dates: Vec<String>,
    pub haystack_sessions: Vec<Vec<Turn>>,
}

#[derive(Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// The mechanical keyer: the session's own vocabulary, shaped to survive the router.
///
/// Unigrams must be one word that stays one word; bigrams must keep two surviving
/// words. The stoplist is a small English function-word list rather than kb's full
/// STOPWORDS, because over-filtering here only weakens the floor being measured.
fn mechanical_keys(text: &str, cap: usize) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "that", "with", "this", "you", "your", "have", "has", "had",
        "was", "were", "are", "not", "but", "they", "their", "them", "from", "what",
        "when", "where", "which", "will", "would", "could", "should", "about", "there",
        "been", "being", "into", "over", "also", "just", "like", "some", "more", "can",
        "than", "then", "out", "get", "got", "how", "who", "why", "his", "her", "she",
        "him", "its", "our", "ours", "any", "all", "one", "two", "did", "does", "doing",
        "assistant", "user", "yes", "okay", "sure", "thanks", "thank", "help", "know",
        "want", "need", "make", "made", "really", "very", "much", "many", "here",
    ];
    let stop = |w: &str| w.len() < 4 || STOP.contains(&w);

    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();

    let mut uni: std::collections::HashMap<&str, usize> = Default::default();
    let mut bi: std::collections::HashMap<(&str, &str), usize> = Default::default();
    for w in &words {
        if !stop(w) {
            *uni.entry(w.as_str()).or_default() += 1;
        }
    }
    for pair in words.windows(2) {
        let (a, b) = (pair[0].as_str(), pair[1].as_str());
        if !stop(a) && !stop(b) {
            *bi.entry((a, b)).or_default() += 1;
        }
    }

    let mut ranked: Vec<(String, usize)> = uni.into_iter().map(|(w, n)| (w.to_string(), n)).collect();
    let mut ranked_bi: Vec<(String, usize)> =
        bi.into_iter().filter(|(_, n)| *n >= 2).map(|((a, b), n)| (format!("{a} {b}"), n)).collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked_bi.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut keys: Vec<String> = Vec::new();
    for (k, _) in ranked_bi.into_iter().take(cap / 3) {
        keys.push(k);
    }
    for (k, _) in ranked.into_iter() {
        if keys.len() >= cap {
            break;
        }
        if !keys.iter().any(|have| have.contains(&k)) {
            keys.push(k);
        }
    }
    keys
}

/// One instance's sessions, written as a one-agent fleet under `root`.
pub fn build_fleet(root: &Path, inst: &Instance) -> std::io::Result<()> {
    let agent = root.join("history");
    std::fs::create_dir_all(agent.join("memory"))?;

    std::fs::write(
        agent.join("agent.txt"),
        "name = History\nrole = The person's chat history, one file per session\n",
    )?;
    std::fs::write(
        agent.join("index.md"),
        "# History\n\n**Search for:** `history`, `chat history`, `past sessions`, \
         `previous conversation`, `earlier conversation`, `we discussed`, `you told me`\n\n\
         **Exists to:** Hold the person's past sessions as memory files, one per session\n",
    )?;

    for (i, session) in inst.haystack_sessions.iter().enumerate() {
        let date = inst.haystack_dates.get(i).map(String::as_str).unwrap_or("undated");
        let mut body = String::new();
        for t in session {
            body.push_str(&format!("**{}:** {}\n\n", t.role, t.content.replace('\r', "")));
        }
        let keys = mechanical_keys(&body, 45);
        let keyline = keys.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ");
        let file = format!(
            "# Session of {date}\n\n**Search for:** {keyline}\n\n\
             **Exists to:** Record the chat session of {date}\n\n{body}",
        );
        std::fs::write(agent.join("memory").join(format!("{:03}-{}.md", i, sanitize(date))), file)?;
    }
    Ok(())
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' }).collect()
}

/// Index, ask, answer: the shipped pipeline over the generated fleet.
pub fn answer_one(root: &Path, inst: &Instance, answerer: &Classifier, mode: kb::answer::Mode) -> Result<String, String> {
    // The text store, built the way `kb index` builds it: through the library.
    let agent_dir = root.join("history");
    let base = kb::base::Base::discover(&agent_dir, true).map_err(|e| e.to_string())?;
    let mut store = kb::store::Store::open(&agent_dir.join(".kb").join("index.db"))
        .map_err(|e| e.to_string())?;
    store.sync(&base, "history").map_err(|e| e.to_string())?;
    drop(store);

    let memory = kb::memory::Memory::open(&[root], true).map_err(|e| e.to_string())?;
    let question = format!("(today is {}) {}", inst.question_date, inst.question);
    let a = memory.ask(&question, 5usize.max(mode.files().min(64)));

    if mode == kb::answer::Mode::Complete {
        // The whole-base read, the harness way: the same map and reduce prompts the
        // product ships, batched through the same answerer.
        let plan = kb::answer::complete_plan(&memory);
        let mut facts = String::new();
        let mut batch: Vec<(String, String)> = Vec::new();
        let mut run_batch = |batch: &mut Vec<(String, String)>, facts: &mut String| {
            if batch.is_empty() {
                return;
            }
            let p = kb::answer::map_prompt(&question, batch);
            if let Some(reply) = kb::promote::ask_model(answerer, root, &p) {
                for line in reply.lines() {
                    let l = line.trim();
                    if !l.is_empty() && !l.eq_ignore_ascii_case("none") {
                        facts.push_str(l);
                        facts.push('\n');
                    }
                }
            }
        };
        for (name, path) in &plan.files {
            batch.push((name.clone(), std::fs::read_to_string(path).unwrap_or_default()));
            if batch.len() >= kb::answer::BATCH {
                run_batch(&mut batch, &mut facts);
                batch.clear();
            }
        }
        run_batch(&mut batch, &mut facts);
        if facts.trim().is_empty() {
            return Ok("The history does not hold this; I don't know.".into());
        }
        let reduce = kb::answer::reduce_prompt(&question, &facts);
        return kb::promote::ask_model(answerer, root, &reduce)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "the reduce call got no reply".into());
    }

    if !kb::answer::worth_asking(&a.confidence, &a.found) {
        // The library's honest refusal is also LongMemEval's abstention answer.
        return Ok("The history does not hold this; I don't know.".into());
    }
    let prompt = kb::answer::prompt(&question, &a, mode);
    kb::promote::ask_model(answerer, root, &prompt)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "the answerer did not reply".into())
}

/// The local judge, clearly labelled non-official.
fn judge_one(
    judge: &Classifier,
    root: &Path,
    inst: &Instance,
    hypothesis: &str,
) -> Option<bool> {
    let abstention = inst.question_id.ends_with("_abs");
    let prompt = if abstention {
        format!(
            "The following question is UNANSWERABLE from the person's history, so the \
             correct behaviour is to say the information is not available. Does the \
             response correctly express that it does not know or that the history does \
             not contain this, rather than inventing an answer?\n\nQuestion: {}\n\n\
             Response: {}\n\nAnswer strictly yes or no.",
            inst.question, hypothesis
        )
    } else {
        format!(
            "I will give you a question, the correct answer, and a model's response. \
             Judge whether the response contains the correct answer. Small wording \
             differences, extra correct detail, or equivalent phrasings count as \
             correct. Missing the answer, contradicting it, or answering something \
             else counts as incorrect.\n\nQuestion: {}\n\nCorrect answer: {}\n\n\
             Response: {}\n\nAnswer strictly yes or no.",
            inst.question, inst.answer, hypothesis
        )
    };
    let reply = kb::promote::ask_model(judge, root, &prompt)?;
    let r = reply.trim().to_lowercase();
    Some(r.starts_with("yes") || r.contains("\nyes"))
}

pub struct Options {
    /// fast, expanded or complete: the product mode this run declares in its header.
    pub mode: kb::answer::Mode,
    pub limit: usize,
    pub offset: usize,
    pub workers: usize,
    pub out: PathBuf,
    pub answerer: String,
    pub judge: Option<String>,
    pub keep: bool,
    /// Only instances of this question_type, when set: for re-measuring one ability.
    pub only_type: Option<String>,
}

pub fn run(dataset: &Path, opt: &Options) -> Result<(), String> {
    let text = std::fs::read_to_string(dataset).map_err(|e| format!("{}: {e}", dataset.display()))?;
    let all: Vec<Instance> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let total_available = all.len();
    let slice: Vec<Instance> = all
        .into_iter()
        .filter(|i| opt.only_type.as_deref().is_none_or(|t| i.question_type == t))
        .skip(opt.offset)
        .take(if opt.limit == 0 { usize::MAX } else { opt.limit })
        .collect();
    println!(
        "longmem: {} instance(s) of {} (offset {}), {} worker(s), answers -> {}",
        slice.len(),
        total_available,
        opt.offset,
        opt.workers,
        opt.out.display()
    );

    let scratch = std::env::temp_dir().join(format!("kb-longmem-{}", std::process::id()));
    let answerer = Classifier::Command(opt.answerer.clone());
    let judge = opt.judge.clone().map(Classifier::Command);

    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<(String, String, Option<bool>, String)>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);

    std::thread::scope(|s| {
        for _ in 0..opt.workers.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(inst) = slice.get(i) else { break };
                let root = scratch.join(&inst.question_id);
                let outcome = std::fs::create_dir_all(&root)
                    .map_err(|e| e.to_string())
                    .and_then(|_| build_fleet(&root, inst).map_err(|e| e.to_string()))
                    .and_then(|_| answer_one(&root, inst, &answerer, opt.mode));
                let (hyp, verdict) = match outcome {
                    Ok(h) => {
                        let v = judge.as_ref().and_then(|j| judge_one(j, &root, inst, &h));
                        (h, v)
                    }
                    Err(e) => (format!("[harness error: {e}]"), Some(false)),
                };
                if !opt.keep {
                    let _ = std::fs::remove_dir_all(&root);
                }
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                eprintln!(
                    "  [{n}/{}] {} {} {}",
                    slice.len(),
                    inst.question_id,
                    match verdict {
                        Some(true) => "CORRECT",
                        Some(false) => "wrong  ",
                        None => "unjudged",
                    },
                    inst.question_type
                );
                results.lock().unwrap().push((
                    inst.question_id.clone(),
                    hyp,
                    verdict,
                    inst.question_type.clone(),
                ));
            });
        }
    });

    let mut rows = results.into_inner().unwrap();
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut f = std::fs::File::create(&opt.out).map_err(|e| e.to_string())?;
    for (id, hyp, _, _) in &rows {
        let line = serde_json::json!({ "question_id": id, "hypothesis": hyp });
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }

    if judge.is_some() {
        let mut per: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for (id, _, v, qt) in &rows {
            let key = if id.ends_with("_abs") { "abstention".to_string() } else { qt.clone() };
            let e = per.entry(key).or_default();
            e.1 += 1;
            if *v == Some(true) {
                e.0 += 1;
            }
        }
        let (mut ok, mut n) = (0usize, 0usize);
        println!();
        println!("JUDGED LOCALLY, non-official judge; not leaderboard-comparable until");
        println!("re-judged by the official protocol over the hypotheses file.");
        for (qt, (c, t)) in &per {
            println!("  {qt:<24} {c}/{t}");
            ok += c;
            n += t;
        }
        println!("  {:<24} {ok}/{n} ({:.0}%)", "TOTAL", 100.0 * ok as f64 / n.max(1) as f64);
    }
    Ok(())
}
