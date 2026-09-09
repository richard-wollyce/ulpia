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
pub fn answer_one(
    root: &Path,
    inst: &Instance,
    answerer: &Classifier,
    mode: kb::answer::Mode,
    trace: Option<&Path>,
) -> Result<String, String> {
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
        let tdir = trace.map(|t| t.join(&inst.question_id));
        if let Some(d) = &tdir {
            let _ = std::fs::create_dir_all(d);
            let _ = std::fs::write(
                d.join("question.txt"),
                format!("{question}\ngold: {}\n", inst.answer),
            );
        }
        let mut batch_no = 0usize;
        let mut run_batch = |batch: &mut Vec<(String, String)>, facts: &mut String| {
            if batch.is_empty() {
                return;
            }
            batch_no += 1;
            let p = kb::answer::map_prompt(&question, batch);
            let reply = kb::promote::ask_model(answerer, root, &p);
            if let Some(d) = &tdir {
                let files: Vec<&str> = batch.iter().map(|(n, _)| n.as_str()).collect();
                let _ = std::fs::write(
                    d.join(format!("map-{batch_no:02}.txt")),
                    format!(
                        "FILES:\n{}\n\nREPLY:\n{}\n",
                        files.join("\n"),
                        reply.as_deref().unwrap_or("(no reply)")
                    ),
                );
            }
            if let Some(reply) = reply {
                for line in reply.lines() {
                    let l = line.trim();
                    // Only the dated fact bullets survive; verdict lines (`FILE ...:
                    // no relevant mention`) are the map being visible, not evidence.
                    if l.starts_with("- ") {
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
        if let Some(d) = &tdir {
            let _ = std::fs::write(d.join("facts.txt"), &facts);
        }
        let reduce = kb::answer::reduce_prompt(&question, &facts);
        let out = kb::promote::ask_model(answerer, root, &reduce)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "the reduce call got no reply".to_string());
        if let (Some(d), Ok(ans)) = (&tdir, &out) {
            let _ = std::fs::write(d.join("answer.txt"), ans);
        }
        return out;
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

/// The grading prompt, in the one place that builds it.
///
/// **It is public and separate from the call for one reason: a second judge validates
/// nothing unless it reads the identical prompt.** Two judges given two prompts measure
/// prompt sensitivity, not judge agreement, and the difference is invisible in the
/// resulting kappa. `kb-bench judge` and the run loop both come through here.
pub fn judge_prompt(inst: &Instance, hypothesis: &str) -> String {
    if inst.question_id.ends_with("_abs") {
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
    }
}

/// A judge's reply reduced to a verdict, or `None` when it said neither.
///
/// **`None` and not `false`.** A judge that returned an empty string, a refusal, or a
/// transport error has produced no evidence, and scoring that as "wrong" both lowers the
/// benchmark score and, in a validation run, invents a disagreement out of a failed call.
pub fn verdict_of(reply: &str) -> Option<bool> {
    let r = reply.trim().to_lowercase();
    if r.starts_with("yes") || r.contains("\nyes") {
        return Some(true);
    }
    if r.starts_with("no") || r.contains("\nno") {
        return Some(false);
    }
    None
}

/// The local judge, clearly labelled non-official.
fn judge_one(
    judge: &Classifier,
    root: &Path,
    inst: &Instance,
    hypothesis: &str,
) -> Option<bool> {
    let prompt = judge_prompt(inst, hypothesis);
    let reply = kb::promote::ask_model(judge, root, &prompt)?;
    // The run loop's historical behaviour: anything that is not a yes counted as wrong,
    // and it is kept so that a re-run of the published scores reproduces them. The
    // validation path uses `verdict_of`, which refuses to invent a verdict.
    let r = reply.trim().to_lowercase();
    Some(r.starts_with("yes") || r.contains("\nyes"))
}

/// Grades an existing hypotheses file with one judge, and writes the labels.
///
/// **This is the command that makes validation affordable.** The published run threw its
/// judge's labels away: `hypotheses-s.jsonl` carries the answers and no verdicts, so
/// there was nothing to compare a second judge against without paying for 500 answers
/// again. Judging a stored hypotheses file costs judge calls only, which is roughly two
/// orders of magnitude less than re-answering, and it is the only way the same answers
/// can be put in front of two graders.
pub fn judge_file(
    dataset: &Path,
    hyp_file: &Path,
    judge_cmd: &str,
    out: &Path,
    sample: usize,
    every: usize,
    workers: usize,
) -> Result<(), String> {
    let text = std::fs::read_to_string(dataset).map_err(|e| format!("{}: {e}", dataset.display()))?;
    let all: Vec<Instance> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let by_id: std::collections::BTreeMap<&str, &Instance> =
        all.iter().map(|i| (i.question_id.as_str(), i)).collect();

    let hyps = std::fs::read_to_string(hyp_file).map_err(|e| format!("{}: {e}", hyp_file.display()))?;
    let mut rows: Vec<(String, String)> = Vec::new();
    for line in hyps.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let (Some(id), Some(h)) = (v["question_id"].as_str(), v["hypothesis"].as_str()) else {
            continue;
        };
        rows.push((id.to_string(), h.to_string()));
    }

    // **A deterministic stride, not a random sample.** The file is in question_id order,
    // which is unrelated to ability and to difficulty, so every Nth row is a spread across
    // the whole set that a second run reproduces exactly. A seeded shuffle would need the
    // seed carried in the report to mean anything; this needs the stride, which is already
    // in the command line.
    let step = every.max(1);
    let selected: Vec<(String, String)> = rows
        .into_iter()
        .step_by(step)
        .take(if sample == 0 { usize::MAX } else { sample })
        .collect();

    eprintln!(
        "judging {} hypothes(es) with `{judge_cmd}`, {workers} worker(s) -> {}",
        selected.len(),
        out.display()
    );

    let judge = Classifier::Command(absolute_command(judge_cmd));
    let scratch = std::env::temp_dir().join(format!("kb-judge-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;

    let done = AtomicUsize::new(0);
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((id, hyp)) = selected.get(i) else { break };
                let Some(inst) = by_id.get(id.as_str()) else { continue };
                let prompt = judge_prompt(inst, hyp);
                let reply = kb::promote::ask_model(&judge, &scratch, &prompt).unwrap_or_default();
                let verdict = verdict_of(&reply);
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                eprintln!(
                    "  [{n}/{}] {id} {}",
                    selected.len(),
                    match verdict {
                        Some(true) => "correct",
                        Some(false) => "wrong",
                        None => "UNPARSED",
                    }
                );
                results.lock().unwrap().push(serde_json::json!({
                    "question_id": id,
                    "question_type": inst.question_type,
                    "abstention": id.ends_with("_abs"),
                    "correct": verdict,
                    "reply": reply.trim(),
                }));
            });
        }
    });

    let _ = std::fs::remove_dir_all(&scratch);

    let mut rows = results.into_inner().unwrap();
    rows.sort_by(|a, b| a["question_id"].as_str().cmp(&b["question_id"].as_str()));
    let unparsed = rows.iter().filter(|r| r["correct"].is_null()).count();
    let mut f = std::fs::File::create(out).map_err(|e| e.to_string())?;
    for r in &rows {
        writeln!(f, "{r}").map_err(|e| e.to_string())?;
    }
    eprintln!("wrote {} label(s) to {}", rows.len(), out.display());

    // **A judge that never answered is a broken command, not a judge with no opinion.**
    // The first run of this path wrote 3 of 3 nulls because the command was relative and
    // the child process could not find it, and the only sign was the word UNPARSED going
    // past in a progress line. A validation run built on that file would have reported an
    // empty table as a result.
    if unparsed == rows.len() && !rows.is_empty() {
        return Err(format!(
            "every one of the {} replies was empty or unparseable. The judge command \
             probably did not run: check that `{judge_cmd}` exists and is executable from \
             here, and note that a relative path is resolved against this process and not \
             against the scratch directory the judge runs in",
            rows.len()
        ));
    }
    if unparsed > 0 {
        eprintln!("  {unparsed} reply(ies) parsed as neither yes nor no, written as null");
    }
    Ok(())
}

/// The judge command with its program made absolute, when that program is a file here.
///
/// **The mechanism, and it is a Windows one.** `CreateProcess` resolves a bare program
/// name against the *calling* process's directory and current directory, not against the
/// working directory the child is given. The judge runs in a scratch directory, so
/// `--judge judge-claude.cmd` from the benchmark folder found nothing, produced an empty
/// stdout, and looked exactly like a judge that declined to answer. Making the program
/// absolute before the call removes the ambiguity; arguments are left alone.
pub fn absolute_command(cmd: &str) -> String {
    let mut parts = cmd.splitn(2, char::is_whitespace);
    let Some(program) = parts.next() else { return cmd.to_string() };
    let rest = parts.next();

    let resolved = match std::path::Path::new(program).canonicalize() {
        Ok(p) => p.to_string_lossy().trim_start_matches(r"\\?\").to_string(),
        Err(_) => return cmd.to_string(),
    };
    match rest {
        Some(r) => format!("{resolved} {r}"),
        None => resolved,
    }
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
    /// The complement of only_type, for a declared-modes run split by question
    /// nature: aggregation questions run one mode, everything else another, and
    /// the two halves are two runs whose headers each declare their mode.
    pub skip_type: Option<String>,
    /// When set, complete mode writes its intermediates here, one dir per question:
    /// the per-batch map replies, the fact sheet the reduce saw, and the final
    /// answer. An autopsy without intermediates is guesswork with a straight face.
    pub trace: Option<PathBuf>,
}

pub fn run(dataset: &Path, opt: &Options) -> Result<(), String> {
    let text = std::fs::read_to_string(dataset).map_err(|e| format!("{}: {e}", dataset.display()))?;
    let all: Vec<Instance> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let total_available = all.len();
    let slice: Vec<Instance> = all
        .into_iter()
        .filter(|i| opt.only_type.as_deref().is_none_or(|t| i.question_type == t))
        .filter(|i| opt.skip_type.as_deref().is_none_or(|t| i.question_type != t))
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
                    .and_then(|_| answer_one(&root, inst, &answerer, opt.mode, opt.trace.as_deref()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_judge_command_is_made_absolute_before_it_is_run() {
        // The failure this prevents is silent: on Windows a bare program name is looked
        // up against this process, not against the scratch directory the judge is given,
        // so a relative path produces an empty stdout instead of an error.
        let dir = std::env::temp_dir().join(format!("kb-judgecmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cmd = dir.join("j.cmd");
        std::fs::write(&cmd, "@echo off\n").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let out = absolute_command("j.cmd");
        std::env::set_current_dir(prev).unwrap();

        assert!(std::path::Path::new(&out).is_absolute(), "not absolute: {out}");
        assert!(out.ends_with("j.cmd"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_command_that_is_not_a_file_here_is_left_alone() {
        // `claude -p --model x` is resolved by the OS from PATH and must not be mangled.
        assert_eq!(absolute_command("claude -p --model x"), "claude -p --model x");
    }

    #[test]
    fn a_judge_reply_that_is_neither_yes_nor_no_produces_no_verdict() {
        assert_eq!(verdict_of("Yes"), Some(true));
        assert_eq!(verdict_of("no, the response misses the date"), Some(false));
        assert_eq!(verdict_of("Yes\n\nThe response names the cartoon."), Some(true));
        assert_eq!(verdict_of(""), None, "an empty reply is not a `wrong`");
        assert_eq!(verdict_of("I cannot judge this."), None);
    }
}
