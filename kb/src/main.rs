//! kb, a linter and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any index is derived
//! from them. This is that derived thing: it stores nothing, reads the files on every run, and either
//! reports what is broken (`check`) or answers which files a question should open (`route`).

mod base;
mod checks;
mod index;
mod remember;
mod store;

use base::Base;
use checks::{Finding, Level};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
kb, a linter and router for file based knowledge bases

usage:
    kb check [path]... [--strict] [--all]
    kb index [path]... [--db FILE] [--json] [--all]
    kb route <question> [path]... [--top N] [--hybrid] [--db FILE]
    kb remember <claim> [path]... [--db FILE]

    path        base to work on, defaults to the current directory
    --strict    check: count warnings toward the exit code
    --all       include files git does not track, normally the private layer
    --top N     route: how many candidates to print, default 5
    --db FILE   the derived index, default .kb/index.db
    --json      index: print the map entries as JSON instead of building the index
    --hybrid    route: fuse the keyword scorer with full text search over chunks

checks:
    E01 broken-link     a [[link]] with no file behind it
    E02 not-indexed     a note in the knowledge folder with no entry in the map
    E03 no-map          no MAP.md, INDEX.md or MAPA.md at the root
    W01 ambiguous-link  a [[link]] matching more than one file
    W02 no-search-line  a map entry with no Search for line
    W03 dash            an em dash or en dash, which house style forbids
    W04 front-matter    a note declaring a source with no evidence_tier or valid_for
    W05 no-provenance   a note with no provenance or stage, so who wrote it is unknown
    E04 bad-provenance  provenance or stage carries a value outside the legal set

exit code is 1 when check finds errors, or when --strict and it finds warnings.
";

/// How many line numbers to print before collapsing into a count.
const LINES_SHOWN: usize = 3;

/// Flags that consume the argument after them.
const VALUE_FLAGS: &[&str] = &["--top", "--db"];

/// The derived index. Disposable by design: deleting it costs a rebuild.
const DEFAULT_DB: &str = ".kb/index.db";

/// Reciprocal Rank Fusion's damping constant, 60 in the original paper.
///
/// RRF fuses ranked lists by position alone, which is the point: a BM25 score and a
/// keyword score live in different numeric universes, and normalising them into a
/// weighted sum means inventing a conversion factor and then tuning it per corpus.
/// Ranks need no conversion.
const RRF_K: f64 = 60.0;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let all = args.iter().any(|a| a == "--all");
    let strict = args.iter().any(|a| a == "--strict");
    let top = flag_value(&args, "--top").and_then(|v| v.parse().ok()).unwrap_or(5);

    // Flags that take a value swallow the argument after them, otherwise that
    // value gets read as a path and the error message blames the wrong thing.
    let mut positional: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for arg in &args[1..] {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg.starts_with("--") {
            skip_next = VALUE_FLAGS.contains(&arg.as_str());
            continue;
        }
        positional.push(arg.as_str());
    }

    let db = flag_value(&args, "--db").unwrap_or_else(|| DEFAULT_DB.to_string());
    let json = args.iter().any(|a| a == "--json");
    let hybrid = args.iter().any(|a| a == "--hybrid");

    match args[0].as_str() {
        "check" => cmd_check(&paths_or_default(&positional), all, strict),
        "index" => cmd_index(&paths_or_default(&positional), all, &db, json),
        "route" => {
            if positional.is_empty() {
                eprintln!("kb: route needs a question\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let question = positional[0];
            let paths = paths_or_default(&positional[1..]);
            cmd_route(question, &paths, all, top, hybrid, &db)
        }
        "remember" => {
            if positional.is_empty() {
                eprintln!("kb: remember needs a claim\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let claim = positional[0];
            let paths = paths_or_default(&positional[1..]);
            cmd_remember(claim, &paths, all, &db)
        }
        other => {
            eprintln!("kb: unknown command '{other}'\n");
            print!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The value after a flag, as in `--top 8`. Returns None when the flag is absent.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

fn paths_or_default<'a>(given: &[&'a str]) -> Vec<&'a str> {
    if given.is_empty() { vec!["."] } else { given.to_vec() }
}

// ---------------------------------------------------------------------------
// check
// ---------------------------------------------------------------------------

fn cmd_check(paths: &[&str], all: bool, strict: bool) -> ExitCode {
    let mut errors = 0usize;
    let mut warnings = 0usize;

    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            println!();
        }
        match check_one(Path::new(path), all) {
            Ok((e, w)) => {
                errors += e;
                warnings += w;
            }
            Err(e) => {
                eprintln!("kb: cannot read {path}: {e}");
                errors += 1;
            }
        }
    }

    if errors > 0 || (strict && warnings > 0) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn check_one(path: &Path, all: bool) -> std::io::Result<(usize, usize)> {
    let base = Base::discover(path, all)?;
    let findings = checks::run(&base);

    let map = base.map.clone().unwrap_or_else(|| "none".to_string());
    let scope = if base.tracked_only {
        "tracked files"
    } else {
        "files, git not consulted"
    };
    println!("{}  ({} {scope}, map: {map})", label(&base), base.files.len());

    for (file, reason) in &base.unreadable {
        println!("  skipped  {file}: {reason}");
    }

    let errors = findings.iter().filter(|f| f.level == Level::Error).count();
    let warnings = findings.len() - errors;

    for group in group(&findings) {
        println!("  {}", format_group(&group));
    }

    if findings.is_empty() {
        println!("  clean");
    } else {
        println!("  {errors} errors, {warnings} warnings");
    }

    Ok((errors, warnings))
}

// ---------------------------------------------------------------------------
// index and route
// ---------------------------------------------------------------------------

fn cmd_index(paths: &[&str], all: bool, db: &str, json: bool) -> ExitCode {
    let mut bases = Vec::new();
    for path in paths {
        match Base::discover(Path::new(path), all) {
            Ok(base) => bases.push(base),
            Err(e) => {
                eprintln!("kb: cannot read {path}: {e}");
                return ExitCode::from(1);
            }
        }
    }

    if json {
        let entries: Vec<index::Entry> = bases.iter().flat_map(index::build).collect();
        print!("{}", index::to_json(&entries));
        return ExitCode::SUCCESS;
    }

    let mut store = match store::Store::open(&PathBuf::from(db)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("kb: cannot open {db}: {e}");
            return ExitCode::from(1);
        }
    };

    println!("index: {db}");
    for base in &bases {
        let name = label(base);
        match store.sync(base, &name) {
            Ok(r) => println!(
                "  {name:<8} {} reindexed, {} unchanged, {} removed, {} chunks written",
                r.reindexed, r.unchanged, r.removed, r.chunks
            ),
            Err(e) => {
                eprintln!("kb: indexing {name} failed: {e}");
                return ExitCode::from(1);
            }
        }
    }

    match store.counts() {
        Ok((files, chunks)) => println!("  total: {files} files, {chunks} chunks"),
        Err(e) => eprintln!("kb: cannot count: {e}"),
    }

    ExitCode::SUCCESS
}

fn cmd_route(
    question: &str,
    paths: &[&str],
    all: bool,
    top: usize,
    hybrid: bool,
    db: &str,
) -> ExitCode {
    let mut entries = Vec::new();
    let mut aliases = Vec::new();
    for path in paths {
        match Base::discover(Path::new(path), all) {
            Ok(base) => {
                entries.extend(index::build(&base));
                aliases.extend(base.aliases.clone());
            }
            Err(e) => {
                eprintln!("kb: cannot read {path}: {e}");
                return ExitCode::from(1);
            }
        }
    }

    // One expansion, two consumers. Expanding for the keyword scorer and not for the
    // text one was a real bug: a Portuguese question routed correctly by keyword and
    // matched zero chunks by text.
    let terms = index::expand_query(question, &aliases);

    // Oversample each list: fusion only helps when it has more than the final answer
    // to choose from.
    let keyword = index::route(question, &entries, &aliases, top * 4);

    println!("question: {question}");
    println!(
        "indexed:  {} entries across {} bases, {} aliases",
        entries.len(),
        paths.len(),
        aliases.len()
    );

    if !hybrid {
        println!();
        return report_keyword(&keyword, top);
    }

    let text = match store::Store::open(&PathBuf::from(db)) {
        Ok(s) => s.search(&terms, top * 6).unwrap_or_default(),
        Err(e) => {
            eprintln!("kb: cannot open {db}: {e}. Run `kb index` first.");
            return ExitCode::from(1);
        }
    };
    println!("full text: {} chunks matched", text.len());
    println!();

    let fused = fuse(&keyword, &text, top);

    if fused.is_empty() {
        println!("  nothing matched, in either scorer. Either the base does not cover");
        println!("  it, or the Search for lines do not carry the words a real question");
        println!("  uses.");
        return ExitCode::SUCCESS;
    }

    for f in &fused {
        println!("  {:>5.3}  {:<8} {:<44} {}", f.score, f.base, f.path, f.why.join(" + "));
        if let Some(excerpt) = &f.excerpt {
            println!("         {}", excerpt.replace('\n', " ").trim());
        }
    }

    ExitCode::SUCCESS
}

fn report_keyword(hits: &[index::Hit], top: usize) -> ExitCode {
    if hits.is_empty() {
        // Saying so plainly is the point. A router that always returns something
        // teaches you to trust a guess.
        println!("  nothing matched. Either the base does not cover it, or the");
        println!("  Search for lines do not carry the words a real question uses.");
        return ExitCode::SUCCESS;
    }

    for hit in hits.iter().take(top) {
        println!(
            "  {:>6.1}  {:<8} {:<44} {}",
            hit.score,
            hit.entry.base,
            if hit.entry.rel.is_empty() { hit.entry.stem.clone() } else { hit.entry.rel.clone() },
            hit.entry.title
        );
        println!("       matched: {}", hit.matched.join(", "));
    }
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// remember
// ---------------------------------------------------------------------------

fn cmd_remember(claim: &str, paths: &[&str], all: bool, db: &str) -> ExitCode {
    let mut aliases = Vec::new();
    for path in paths {
        match Base::discover(Path::new(path), all) {
            Ok(base) => aliases.extend(base.aliases.clone()),
            Err(e) => {
                eprintln!("kb: cannot read {path}: {e}");
                return ExitCode::from(1);
            }
        }
    }

    let terms = index::expand_query(claim, &aliases);

    let hits = match store::Store::open(&PathBuf::from(db)) {
        Ok(s) => s.search(&terms, 25).unwrap_or_default(),
        Err(e) => {
            eprintln!("kb: cannot open {db}: {e}. Run `kb index` first.");
            return ExitCode::from(1);
        }
    };

    let assessment = remember::assess(claim, &terms, &hits);

    println!("claim: {claim}");
    println!("terms: {}", terms.join(", "));
    println!();

    if assessment.evidence.is_empty() {
        println!("closest in the base: nothing matched.");
    } else {
        println!("closest in the base:");
        for e in &assessment.evidence {
            println!();
            println!("  {:.2}  {:<8} {}", e.containment, e.base, e.path);
            println!("        {}", e.heading_path);
            println!("        \"{}\"", e.excerpt.replace('\n', " ").trim());
            println!("        shared:  {}", e.shared.join(", "));
            println!(
                "        missing: {}",
                if e.missing.is_empty() { "nothing".to_string() } else { e.missing.join(", ") }
            );
        }
    }

    println!();
    println!("proposal: {}", assessment.outcome.label());
    println!("  {}", assessment.reason);
    println!();
    for line in remember::DISCLAIMER.lines() {
        println!("  {line}");
    }

    ExitCode::SUCCESS
}

struct Fused {
    base: String,
    path: String,
    score: f64,
    why: Vec<String>,
    excerpt: Option<String>,
}

/// Reciprocal Rank Fusion over the two scorers.
///
/// Each list contributes `1 / (K + rank)` to every document it ranks, and the sums
/// are compared. A document both scorers like beats one that either likes a lot,
/// which is the behaviour we want: the hand written keywords carry intent and the
/// full text carries the actual words, and agreement between them is the strongest
/// signal available without a model.
fn fuse(keyword: &[index::Hit], text: &[store::Hit], top: usize) -> Vec<Fused> {
    let mut acc: HashMap<(String, String), Fused> = HashMap::new();

    for (rank, hit) in keyword.iter().enumerate() {
        if hit.entry.rel.is_empty() {
            continue;
        }
        let key = (hit.entry.base.clone(), hit.entry.rel.clone());
        let entry = acc.entry(key.clone()).or_insert_with(|| Fused {
            base: key.0.clone(),
            path: key.1.clone(),
            score: 0.0,
            why: Vec::new(),
            excerpt: None,
        });
        entry.score += 1.0 / (RRF_K + rank as f64 + 1.0);
        entry.why.push(format!("keywords #{}", rank + 1));
    }

    // The text list ranks chunks, and RRF ranks documents. A file only contributes
    // once, at the rank of its best chunk.
    //
    // Getting this wrong is subtle and it happened: scoring every matching chunk made
    // a long file accumulate one contribution per section, so the ranking quietly
    // became "which file has the most matching pieces" rather than "which file ranked
    // highest". The safety protocol, which defines the calorie floor in one table
    // row, lost to a longer file that mentioned it in passing three times.
    let mut counted: HashSet<(String, String)> = HashSet::new();

    for (rank, hit) in text.iter().enumerate() {
        let key = (hit.base.clone(), hit.path.clone());
        if !counted.insert(key.clone()) {
            continue;
        }
        let entry = acc.entry(key.clone()).or_insert_with(|| Fused {
            base: key.0.clone(),
            path: key.1.clone(),
            score: 0.0,
            why: Vec::new(),
            excerpt: None,
        });
        entry.score += 1.0 / (RRF_K + rank as f64 + 1.0);
        entry.excerpt = Some(format!("{}: {}", hit.heading_path, hit.excerpt));
        entry.why.push(format!("text #{}", rank + 1));
    }

    let mut out: Vec<Fused> = acc.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.path.cmp(&b.path))
    });
    out.truncate(top);
    out
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn label(base: &Base) -> String {
    base.root
        .canonicalize()
        .unwrap_or_else(|_| base.root.clone())
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| base.root.display().to_string())
}

struct Group<'a> {
    level: Level,
    code: &'static str,
    file: &'a str,
    message: &'a str,
    lines: Vec<usize>,
}

/// Collapses findings that repeat the same message in the same file.
///
/// Two hundred separate lines saying "em dash" is not two hundred findings, it is one finding with a
/// count, and printing it the long way buries everything else. The count is always shown, so nothing
/// is hidden by the collapse.
fn group<'a>(findings: &'a [Finding]) -> Vec<Group<'a>> {
    let mut groups: Vec<Group<'a>> = Vec::new();

    for f in findings {
        match groups
            .iter_mut()
            .find(|g| g.file == f.file && g.code == f.code && g.message == f.message)
        {
            Some(g) => g.lines.push(f.line),
            None => groups.push(Group {
                level: f.level,
                code: f.code,
                file: &f.file,
                message: &f.message,
                lines: vec![f.line],
            }),
        }
    }

    groups.sort_by_key(|g| (g.file, g.lines.first().copied().unwrap_or(0), g.code));
    groups
}

fn format_group(g: &Group) -> String {
    let level = match g.level {
        Level::Error => "error",
        Level::Warning => "warn ",
    };

    let first = g.lines.first().copied().unwrap_or(0);
    let place = if first == 0 {
        g.file.to_string()
    } else {
        format!("{}:{}", g.file, first)
    };

    let mut line = format!("{level} {}  {:<46} {}", g.code, place, g.message);

    if g.lines.len() > 1 {
        let shown: Vec<String> = g
            .lines
            .iter()
            .skip(1)
            .take(LINES_SHOWN)
            .map(|l| l.to_string())
            .collect();
        let rest = g.lines.len() - 1 - shown.len();
        let more = if rest > 0 {
            format!(", and {rest} more")
        } else {
            String::new()
        };
        line.push_str(&format!(
            "  ({} times, also on {}{})",
            g.lines.len(),
            shown.join(", "),
            more
        ));
    }

    line
}
