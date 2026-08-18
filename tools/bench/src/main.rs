//! kb-bench, the measuring instrument for retrieval candidates.
//!
//! ADR-0017 declined a dense scorer with numbers produced by a throwaway Python
//! environment. Its revisit triggers are real, which means the measurement has to be
//! repeatable without rebuilding that environment, and Richard asked for the review
//! to happen in Rust. This is that instrument.
//!
//! Three subcommands, one shape: load the corpus **through `kb` itself**, score it
//! with the candidate, grade against a gold file, and print the two numbers a
//! decision needs: top-1 accuracy, and whether the score separates hits from
//! misses. The second number is the one that matters here. ADR-0017's finding was
//! that BGE-M3's dense scores overlap completely between right and wrong answers,
//! while the keyword scorer's one error scored 3.82 against 9.55 for everything
//! correct. A scorer that cannot tell when it is wrong removes the abstention
//! property that separates this system from the ones it competes with.
//!
//! The corpus goes through `kb::store::chunk` and `kb::index::build`, so what is
//! measured is the system, not a reimplementation of it drifting quietly.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod engine;

const USAGE: &str = "\
kb-bench, retrieval measurement against a real fleet

usage:
    kb-bench embed  <fleet-root> <questions.txt> [--gold gold.tsv] [--entries]
    kb-bench rerank <fleet-root> <questions.txt> [--gold gold.tsv] [--top N] [--model jina]
    kb-bench fuse   <fleet-root> <questions.txt> [--gold gold.tsv] [--top N]

    embed    rank the whole corpus by BGE-M3 similarity, grading the dense head
             and the learned lexical (sparse) head separately from one forward
             pass. The sparse head is the one ADR-0017 never measured
             (--entries scores map entries instead of chunked note bodies)
    rerank   let kb route pick the top N files, then re-score each with a
             cross encoder reading question and passage together
    fuse     RRF-fuse kb's keyword ranking with the dense ranking, which is
             the exact experiment ADR-0017 ran in Python

Models are downloaded once into %LOCALAPPDATA%/kb-bench and read from disk after
that. Scores print per question; with --gold, a summary states top-1 accuracy and
the hit/miss score separation, which is the abstention question in one number.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::from(2);
    }

    let mode = args[0].as_str();
    let root = PathBuf::from(&args[1]);
    let questions = match read_lines(Path::new(&args[2])) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("kb-bench: cannot read questions: {e}");
            return ExitCode::from(1);
        }
    };
    let gold = flag_value(&args, "--gold").map(|p| read_gold(Path::new(&p)));
    let top: usize = flag_value(&args, "--top").and_then(|v| v.parse().ok()).unwrap_or(5);

    let outcome = match mode {
        "embed" => embed_mode(&root, &questions, gold.as_ref(), args.iter().any(|a| a == "--entries")),
        "rerank" => rerank_mode(
            &root,
            &questions,
            gold.as_ref(),
            top,
            if flag_value(&args, "--model").as_deref() == Some("jina") {
                engine::RerankChoice::Jina
            } else {
                engine::RerankChoice::Bge
            },
        ),
        "fuse" => fuse_mode(&root, &questions, gold.as_ref(), top),
        other => {
            eprintln!("kb-bench: unknown mode '{other}'\n");
            print!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kb-bench: {e}");
            ExitCode::from(1)
        }
    }
}

// ---------------------------------------------------------------------------
// Corpus, loaded through kb itself
// ---------------------------------------------------------------------------

/// One scoreable document: which file it belongs to, and the text to score.
struct Doc {
    owner: String,
    text: String,
}

/// Every tracked markdown file of every agent, chunked by the exact chunker the
/// real index uses. Tracked only, because that is what `kb` serves in the public
/// scope, and measuring text the router would refuse to serve measures nothing.
fn body_chunks(root: &Path) -> Result<Vec<Doc>, String> {
    let agents_dir = root.join("fleet");
    let mut agents: Vec<PathBuf> = std::fs::read_dir(&agents_dir)
        .map_err(|e| format!("cannot read {}: {e}", agents_dir.display()))?
        .flatten()
        .map(|d| d.path())
        .filter(|p| p.join("MAP.md").is_file())
        .collect();
    agents.sort();

    let mut out = Vec::new();
    for agent in agents {
        let name = agent.file_name().unwrap_or_default().to_string_lossy().to_string();
        let listing = kb::base::quiet("git")
            .arg("-C")
            .arg(&agent)
            .args(["ls-files", "*.md"])
            .output()
            .map_err(|e| format!("git ls-files in {}: {e}", agent.display()))?;
        for rel in String::from_utf8_lossy(&listing.stdout).lines() {
            let rel = rel.trim();
            if rel.is_empty() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(agent.join(rel)) else { continue };
            for c in kb::store::chunk(&text) {
                out.push(Doc { owner: format!("{name}/{rel}"), text: c.text });
            }
        }
    }
    Ok(out)
}

/// Map entries in the same composition the Python benchmark used, so the two
/// instruments stay comparable: title, the Search for terms, then the summary.
fn entry_docs(root: &Path) -> Result<Vec<Doc>, String> {
    let agents_dir = root.join("fleet");
    let mut agents: Vec<PathBuf> = std::fs::read_dir(&agents_dir)
        .map_err(|e| format!("cannot read {}: {e}", agents_dir.display()))?
        .flatten()
        .map(|d| d.path())
        .filter(|p| p.join("MAP.md").is_file())
        .collect();
    agents.sort();

    let mut out = Vec::new();
    for agent in agents {
        let base = kb::base::Base::discover(&agent, false)
            .map_err(|e| format!("cannot open {}: {e}", agent.display()))?;
        for e in kb::index::build(&base) {
            if e.rel.is_empty() {
                continue;
            }
            out.push(Doc {
                owner: format!("{}/{}", e.base, e.rel),
                text: format!("{}. {}. {}", e.title, e.keywords.join(", "), e.summary),
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Modes
// ---------------------------------------------------------------------------

fn embed_mode(
    root: &Path,
    questions: &[String],
    gold: Option<&Gold>,
    entries: bool,
) -> Result<(), String> {
    let docs = if entries { entry_docs(root)? } else { body_chunks(root)? };
    let owners: Vec<&str> = docs.iter().map(|d| d.owner.as_str()).collect();
    println!(
        "corpus: {} {} across {} files",
        docs.len(),
        if entries { "entries" } else { "chunks" },
        distinct(&owners)
    );

    let mut model = engine::Embedder::new(&cache_dir())?;

    let t = std::time::Instant::now();
    let doc_heads = model.embed(docs.iter().map(|d| d.text.clone()).collect())?;
    let index_s = t.elapsed().as_secs_f64();
    println!(
        "index:  {index_s:.1} s ({:.0} ms per item, both heads)
",
        index_s * 1000.0 / docs.len() as f64
    );

    let q_heads = model.embed(questions.to_vec())?;

    for sparse in [false, true] {
        println!("==== {} head ====", if sparse { "sparse (learned lexical)" } else { "dense" });
        let mut graded = Grades::default();
        for (qi, q) in questions.iter().enumerate() {
            // Score per chunk, then a file is its best chunk. The mean of a file's
            // chunks would reward short files and punish thorough ones.
            let mut best: BTreeMap<&str, f32> = BTreeMap::new();
            for (di, doc) in docs.iter().enumerate() {
                let s = if sparse {
                    engine::sparse_dot(&q_heads.sparse[qi], &doc_heads.sparse[di])
                } else {
                    dot(&q_heads.dense[qi], &doc_heads.dense[di])
                };
                let e = best.entry(doc.owner.as_str()).or_insert(f32::MIN);
                if s > *e {
                    *e = s;
                }
            }
            let mut ranked: Vec<(&str, f32)> = best.into_iter().collect();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            ranked.truncate(3);
            report(q, &ranked, gold, &mut graded);
        }
        graded.summarise();
        println!();
    }
    Ok(())
}

fn rerank_mode(
    root: &Path,
    questions: &[String],
    gold: Option<&Gold>,
    top: usize,
    which: engine::RerankChoice,
) -> Result<(), String> {
    let memory = kb::memory::Memory::open(&[root], false).map_err(|e| e.to_string())?;
    let mut model = engine::Reranker::new(&cache_dir(), which)?;

    let mut graded = Grades::default();
    let mut pair_ms: Vec<f64> = Vec::new();

    for q in questions {
        // kb picks the candidates exactly as the agent loop would, and the cross
        // encoder re-reads each one against the question. The reranker cannot
        // rescue a file kb never surfaced: that recall ceiling is a property of
        // this design, stated rather than hidden.
        let hits = memory.route(q, top);
        if hits.is_empty() {
            println!("{:<50} (kb routed nothing, reranker has no candidates)", clip(q, 48));
            graded.abstained(q, gold);
            continue;
        }

        let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
        for h in &hits {
            let owner = format!("{}/{}", h.entry.base, h.entry.rel);
            let path = memory
                .agents
                .iter()
                .find(|a| a.name == h.entry.base)
                .map(|a| a.root.join(&h.entry.rel));
            let Some(path) = path else { continue };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let chunks: Vec<String> = kb::store::chunk(&text).into_iter().map(|c| c.text).collect();
            candidates.push((owner, chunks));
        }

        let t = std::time::Instant::now();
        let mut ranked: Vec<(&str, f32)> = Vec::new();
        let mut pairs = 0usize;
        for (owner, chunks) in &candidates {
            let scores = model.score(q, chunks)?;
            pairs += chunks.len();
            let best = scores.iter().cloned().fold(f32::MIN, f32::max);
            ranked.push((owner.as_str(), best));
        }
        if pairs > 0 {
            pair_ms.push(t.elapsed().as_secs_f64() * 1000.0 / pairs as f64);
        }

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        report(q, &ranked, gold, &mut graded);
    }

    if !pair_ms.is_empty() {
        pair_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        println!(
            "\nreranker: {:.0} ms per pair median, {:.0} ms worst",
            pair_ms[pair_ms.len() / 2],
            pair_ms.last().unwrap()
        );
    }
    graded.summarise();
    Ok(())
}

/// The ADR-0017 experiment, reproduced in Rust: kb's keyword ranking RRF-fused
/// with the dense ranking, same K the real fusion uses.
fn fuse_mode(
    root: &Path,
    questions: &[String],
    gold: Option<&Gold>,
    top: usize,
) -> Result<(), String> {
    let memory = kb::memory::Memory::open(&[root], false).map_err(|e| e.to_string())?;
    let docs = body_chunks(root)?;
    let mut model = engine::Embedder::new(&cache_dir())?;
    let doc_vecs = model.embed(docs.iter().map(|d| d.text.clone()).collect())?.dense;
    let q_vecs = model.embed(questions.to_vec())?.dense;

    let mut graded = Grades::default();
    for (q, qv) in questions.iter().zip(&q_vecs) {
        let keyword: Vec<String> = memory
            .route(q, top)
            .iter()
            .map(|h| format!("{}/{}", h.entry.base, h.entry.rel))
            .collect();

        let mut best: BTreeMap<&str, f32> = BTreeMap::new();
        for (doc, dv) in docs.iter().zip(&doc_vecs) {
            let s = dot(qv, dv);
            let e = best.entry(doc.owner.as_str()).or_insert(f32::MIN);
            if s > *e {
                *e = s;
            }
        }
        let mut dense: Vec<(&str, f32)> = best.into_iter().collect();
        dense.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        dense.truncate(top);

        let mut fused: BTreeMap<String, f64> = BTreeMap::new();
        for (rank, owner) in keyword.iter().enumerate() {
            *fused.entry(owner.clone()).or_default() += 1.0 / (kb::retrieve::RRF_K + rank as f64 + 1.0);
        }
        for (rank, (owner, _)) in dense.iter().enumerate() {
            *fused.entry(owner.to_string()).or_default() += 1.0 / (kb::retrieve::RRF_K + rank as f64 + 1.0);
        }
        let mut ranked: Vec<(String, f64)> = fused.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(3);
        let view: Vec<(&str, f32)> = ranked.iter().map(|(o, s)| (o.as_str(), *s as f32)).collect();
        report(q, &view, gold, &mut graded);
    }
    graded.summarise();
    Ok(())
}

// ---------------------------------------------------------------------------
// Grading
// ---------------------------------------------------------------------------

type Gold = BTreeMap<String, Vec<String>>;

/// question<TAB>path[|path...], `-` meaning the base does not hold the answer and
/// abstaining is the correct behaviour.
fn read_gold(path: &Path) -> Gold {
    let mut out = Gold::new();
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some((q, answers)) = line.split_once('\t') {
            out.insert(
                q.trim().to_lowercase(),
                answers.split('|').map(|a| a.trim().to_string()).collect(),
            );
        }
    }
    out
}

#[derive(Default)]
struct Grades {
    hits: Vec<f32>,
    misses: Vec<f32>,
    correct_abstain: usize,
    wrong_abstain: usize,
    unanswerable_scores: Vec<f32>,
}

impl Grades {
    fn abstained(&mut self, q: &str, gold: Option<&Gold>) {
        match gold.and_then(|g| g.get(&q.to_lowercase())) {
            Some(a) if a == &vec!["-".to_string()] => self.correct_abstain += 1,
            Some(_) => self.wrong_abstain += 1,
            None => {}
        }
    }

    /// The two numbers a decision needs, and the second is the one ADR-0017
    /// turned on: a scorer whose hit scores and miss scores overlap cannot power
    /// an abstention gate, whatever its accuracy.
    fn summarise(&self) {
        let answerable = self.hits.len() + self.misses.len() + self.wrong_abstain;
        if answerable == 0 {
            return;
        }
        println!("\ntop-1:  {} of {} answerable questions", self.hits.len(), answerable);
        if self.correct_abstain + self.wrong_abstain > 0 {
            println!(
                "abstain: {} correct, {} wrong",
                self.correct_abstain, self.wrong_abstain
            );
        }
        let lo = |v: &Vec<f32>| v.iter().cloned().fold(f32::MAX, f32::min);
        let hi = |v: &Vec<f32>| v.iter().cloned().fold(f32::MIN, f32::max);
        if !self.hits.is_empty() {
            println!("hit  scores: {:.3} to {:.3}", lo(&self.hits), hi(&self.hits));
        }
        if !self.misses.is_empty() {
            println!("miss scores: {:.3} to {:.3}", lo(&self.misses), hi(&self.misses));
        }
        if !self.unanswerable_scores.is_empty() {
            println!(
                "scores handed out on unanswerable questions: {:.3} to {:.3}",
                lo(&self.unanswerable_scores),
                hi(&self.unanswerable_scores)
            );
        }
        if !self.hits.is_empty() && !self.misses.is_empty() {
            let gap = lo(&self.hits) - hi(&self.misses);
            if gap > 0.0 {
                println!("SEPARATES: every hit outscored every miss, gap {gap:.3}");
            } else {
                println!("OVERLAPS: no threshold tells a hit from a miss (gap {gap:.3})");
            }
        }
    }
}

fn report(q: &str, ranked: &[(&str, f32)], gold: Option<&Gold>, grades: &mut Grades) {
    let verdict = gold.and_then(|g| g.get(&q.to_lowercase())).map(|answers| {
        if answers == &vec!["-".to_string()] {
            grades.unanswerable_scores.push(ranked.first().map(|r| r.1).unwrap_or(0.0));
            "none-expected"
        } else if ranked.first().is_some_and(|(top, _)| answers.iter().any(|a| a == top)) {
            grades.hits.push(ranked[0].1);
            "HIT"
        } else {
            grades.misses.push(ranked.first().map(|r| r.1).unwrap_or(0.0));
            "miss"
        }
    });
    println!(
        "{:<50} {:<13} {}",
        clip(q, 48),
        verdict.unwrap_or(""),
        ranked
            .first()
            .map(|(o, s)| format!("{o}  {s:.3}"))
            .unwrap_or_else(|| "(nothing)".into())
    );
    for (o, s) in ranked.iter().skip(1) {
        println!("{:<64} {o}  {s:.3}", "");
    }
}

// ---------------------------------------------------------------------------
// Small pieces
// ---------------------------------------------------------------------------

/// Outside the fleet on purpose: models are machine state, not fleet state, and a
/// fleet directory that syncs between machines must not carry gigabytes of ONNX.
fn cache_dir() -> PathBuf {
    std::env::var("KB_BENCH_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("LOCALAPPDATA")
                .map(|d| PathBuf::from(d).join("kb-bench"))
                .unwrap_or_else(|_| PathBuf::from(".kb-bench-cache"))
        })
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn distinct(owners: &[&str]) -> usize {
    let mut d: Vec<&str> = owners.to_vec();
    d.sort();
    d.dedup();
    d.len()
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn read_lines(path: &Path) -> Result<Vec<String>, String> {
    Ok(std::fs::read_to_string(path)
        .map_err(|e| e.to_string())?
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect())
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}
