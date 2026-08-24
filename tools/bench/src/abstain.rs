//! The abstention benchmark: can the router say no, and how fast is the whole thing.
//!
//! ## Why this exists
//!
//! Every retrieval system this one competes with always returns a rank one, because
//! ranking cannot express absence. Ulpia's differentiating claim is the refusal, and a
//! claim that differentiates is a claim that must be measured or it is marketing. This
//! subcommand turns it into two numbers over a labelled question set: the decline rate
//! on questions the corpus should refuse, and the false-decline rate on questions it
//! should answer.
//!
//! ## What "declined" means here, stated so nobody reads more into it
//!
//! This measures the **deterministic layer only**: a question is declined when the
//! keyword scorer returns nothing, or when its best score sits under `SCORE_FLOOR`.
//! The classifier that sits in front of this in `kb boot` can only decline *more*
//! (coverage verdicts route nobody on adjacent and uncovered), so the number printed
//! here is the floor of the system's abstention, not its ceiling. No model, no
//! network, byte-for-byte reproducible.
//!
//! ## The baseline beside it
//!
//! The same questions also run against a top-k baseline: the corpus's own full-text
//! ranking with the abstention affordance removed, answering with its best hit no
//! matter what. That is the shape a plain retrieval API returns, and running it on
//! the same corpus shows what the refusal is worth without putting a number in a
//! competitor's mouth: their products are not run here, and this file does not claim
//! to score them.

use std::path::Path;
use std::time::Instant;

use kb::memory::{Memory, SCORE_FLOOR};

pub struct Row {
    pub question: String,
    /// "in-scope", or anything else meaning the corpus should refuse.
    pub label: String,
}

/// Reads `question<TAB>label` lines, `#` comments ignored.
pub fn load(path: &Path) -> Result<Vec<Row>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((q, label)) = line.split_once('\t') else {
            return Err(format!("a row without a tab: {line}"));
        };
        rows.push(Row { question: q.trim().to_string(), label: label.trim().to_string() });
    }
    Ok(rows)
}

pub fn run(root: &Path, questions: &Path) -> Result<(), String> {
    let memory = Memory::open(&[root], false).map_err(|e| e.to_string())?;
    let rows = load(questions)?;

    // Three outcomes rather than a binary, because the system has three and collapsing
    // them lied in the first run of this instrument: below the floor the router still
    // answers, labelled a guess, and "guess" is neither a confident answer nor silence.
    // For out-of-scope questions the failure is CONFIDENT only; for in-scope ones the
    // failure is NOTHING only, and a guess in either column is the gate doing its job
    // with its uncertainty stated.
    #[derive(PartialEq)]
    enum Out {
        Confident,
        Guess,
        Nothing,
    }
    let mut m: std::collections::BTreeMap<(String, u8), usize> = Default::default();
    let mut baseline_answers_oos = 0usize;
    let mut confident_oos: Vec<(String, f32, String)> = Vec::new();

    println!("  outcome     baseline   score  label      question");
    for row in &rows {
        let answer = memory.ask(&row.question, 5);
        let score = answer.confidence.keyword_score;
        let out = if answer.keyword_top.is_none() {
            Out::Nothing
        } else if score >= SCORE_FLOOR {
            Out::Confident
        } else {
            Out::Guess
        };

        // The top-k baseline: same corpus, same fused ranking, refusal removed. It
        // returns its best hit whenever anything shares a token, which is what
        // "always returns a top hit" looks like on an out-of-scope question.
        let baseline_answers = !answer.found.is_empty();
        if baseline_answers && row.label != "in-scope" {
            baseline_answers_oos += 1;
        }
        if out == Out::Confident && row.label != "in-scope" {
            confident_oos.push((
                row.question.clone(),
                score,
                answer.keyword_top.clone().unwrap_or_default(),
            ));
        }

        let code = match out {
            Out::Confident => 0u8,
            Out::Guess => 1,
            Out::Nothing => 2,
        };
        *m.entry((row.label.clone(), code)).or_default() += 1;

        println!(
            "  {}   {}   {:>6.1}  {:<9}  {}",
            match out {
                Out::Confident => "CONFIDENT",
                Out::Guess => "guess    ",
                Out::Nothing => "nothing  ",
            },
            if baseline_answers { "answers " } else { "silent  " },
            score,
            row.label,
            row.question
        );
    }

    let labels: Vec<String> = {
        let mut l: Vec<String> =
            rows.iter().map(|r| r.label.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        l.sort_by_key(|x| if x == "in-scope" { 0 } else { 1 });
        l
    };
    let count = |lab: &str, code: u8| m.get(&(lab.to_string(), code)).copied().unwrap_or(0);
    let total = |lab: &str| (0..3u8).map(|c| count(lab, c)).sum::<usize>();

    println!();
    println!("MATRIX   per label: confident answer / answered as a guess / nothing at all");
    for lab in &labels {
        println!(
            "  {:<10} {:>3} confident   {:>3} guess   {:>3} nothing   of {}",
            lab,
            count(lab, 0),
            count(lab, 1),
            count(lab, 2),
            total(lab)
        );
    }

    let oos_total: usize = labels.iter().filter(|l| *l != "in-scope").map(|l| total(l)).sum();
    let oos_confident: usize = labels.iter().filter(|l| *l != "in-scope").map(|l| count(l, 0)).sum();
    println!();
    println!(
        "REFUSAL  {}/{} out-of-scope questions were NOT answered confidently: the failure mode",
        oos_total - oos_confident,
        oos_total
    );
    println!("         that matters is a confident wrong answer, and there were {oos_confident}. Each one:");
    for (q, score, file) in &confident_oos {
        println!("           {score:>6.1}  {file}  <-  {q}");
    }
    println!(
        "BASELINE the same corpus behind a top-k API answers {baseline_answers_oos}/{oos_total} out-of-scope questions,"
    );
    println!("         because ranking cannot express absence.");
    println!("PRICE    in-scope: {} confident, {} guess, {} nothing. A guess still answers, labelled;", count("in-scope", 0), count("in-scope", 1), count("in-scope", 2));
    println!("         the demo corpus's keys were written with the corpus, and these questions were");
    println!("         authored blind, so the guess column is what unturned phrasing costs a small base.");
    println!("FLOOR    {SCORE_FLOOR}, the same constant `kb route` and `kb boot` gate on. Deterministic layer only:");
    println!("         the classifier in front of `kb boot` can only decline more than this, never less.");
    Ok(())
}

/// The latency instrument: the whole deterministic pipeline, timed honestly.
///
/// Three numbers, because one would lie by omission: the cold start (process, open,
/// first index build if stale), the warm in-process per-question cost over many
/// iterations, and the TCP connect floor to the hosted competitors' own API
/// endpoints. The last one is measured, not quoted: it is the physics every cloud
/// memory pays before authentication, before embedding, before inference, and this
/// machine pays it to their real hostnames. No request is sent; the socket opens and
/// closes. A vendor's server-side latency claim sits on top of this floor, never
/// below it.
pub fn latency(root: &Path, questions: &Path, hosts: &[(String, String)]) -> Result<(), String> {
    let rows = load(questions)?;
    let qs: Vec<&str> = rows.iter().map(|r| r.question.as_str()).collect();

    // Cold: everything a first question pays, including opening the fleet.
    let cold_start = Instant::now();
    let memory = Memory::open(&[root], false).map_err(|e| e.to_string())?;
    let _ = memory.ask(qs.first().copied().unwrap_or("warmup"), 5);
    let cold = cold_start.elapsed();

    // Warm: the steady state, every question asked many times, worst and median kept.
    const ROUNDS: usize = 20;
    let mut samples: Vec<u128> = Vec::with_capacity(qs.len() * ROUNDS);
    for _ in 0..ROUNDS {
        for q in &qs {
            let t = Instant::now();
            let _ = memory.ask(q, 5);
            samples.push(t.elapsed().as_micros());
        }
    }
    samples.sort_unstable();
    let p = |q: f64| samples[((samples.len() - 1) as f64 * q) as usize];

    println!("LOCAL    cold start, open plus first question: {:.1} ms", cold.as_secs_f64() * 1000.0);
    println!(
        "         warm, {} samples over {} questions: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms",
        samples.len(),
        qs.len(),
        p(0.50) as f64 / 1000.0,
        p(0.95) as f64 / 1000.0,
        *samples.last().unwrap() as f64 / 1000.0
    );
    println!("         no model, no network, no cache warming tricks: the index is the program.");

    if !hosts.is_empty() {
        println!();
        println!("NETWORK FLOOR  TCP connect to the hosted competitors' own endpoints, 12 samples each,");
        println!("               median. No request sent, nothing authenticated: this is the distance");
        println!("               tax alone, paid before their server-side latency even begins.");
        for (vendor, host) in hosts {
            let mut times: Vec<u128> = Vec::new();
            for _ in 0..12 {
                let t = Instant::now();
                match std::net::TcpStream::connect((host.as_str(), 443)) {
                    Ok(_) => times.push(t.elapsed().as_micros()),
                    Err(_) => {}
                }
            }
            if times.is_empty() {
                println!("  {vendor:<12} {host:<24} unreachable from this machine");
            } else {
                times.sort_unstable();
                println!(
                    "  {vendor:<12} {host:<24} {:.1} ms",
                    times[times.len() / 2] as f64 / 1000.0
                );
            }
        }
    }
    Ok(())
}
