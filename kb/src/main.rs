//! kb, a linter and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any index is derived
//! from them. This is that derived thing: it stores nothing, reads the files on every run, and either
//! reports what is broken (`check`) or answers which files a question should open (`route`).

mod base;
mod checks;
mod index;

use base::Base;
use checks::{Finding, Level};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
kb, a linter and router for file based knowledge bases

usage:
    kb check [path]... [--strict] [--all]
    kb index [path]... [--all]
    kb route <question> [path]... [--top N]

    path        base to work on, defaults to the current directory
    --strict    check: count warnings toward the exit code
    --all       include files git does not track, normally the private layer
    --top N     route: how many candidates to print, default 5

checks:
    E01 broken-link     a [[link]] with no file behind it
    E02 not-indexed     a note in the knowledge folder with no entry in the map
    E03 no-map          no MAP.md, INDEX.md or MAPA.md at the root
    W01 ambiguous-link  a [[link]] matching more than one file
    W02 no-search-line  a map entry with no Search for line
    W03 dash            an em dash or en dash, which house style forbids
    W04 front-matter    a note declaring a source with no evidence_tier or valid_for

exit code is 1 when check finds errors, or when --strict and it finds warnings.
";

/// How many line numbers to print before collapsing into a count.
const LINES_SHOWN: usize = 3;

/// Flags that consume the argument after them.
const VALUE_FLAGS: &[&str] = &["--top"];

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

    match args[0].as_str() {
        "check" => cmd_check(&paths_or_default(&positional), all, strict),
        "index" => cmd_index(&paths_or_default(&positional), all),
        "route" => {
            if positional.is_empty() {
                eprintln!("kb: route needs a question\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let question = positional[0];
            let paths = paths_or_default(&positional[1..]);
            cmd_route(question, &paths, all, top)
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

fn cmd_index(paths: &[&str], all: bool) -> ExitCode {
    let mut entries = Vec::new();
    for path in paths {
        match Base::discover(Path::new(path), all) {
            Ok(base) => entries.extend(index::build(&base)),
            Err(e) => {
                eprintln!("kb: cannot read {path}: {e}");
                return ExitCode::from(1);
            }
        }
    }
    print!("{}", index::to_json(&entries));
    ExitCode::SUCCESS
}

fn cmd_route(question: &str, paths: &[&str], all: bool, top: usize) -> ExitCode {
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

    let hits = index::route(question, &entries, &aliases, top);

    println!("question: {question}");
    println!(
        "indexed:  {} entries across {} bases, {} aliases",
        entries.len(),
        paths.len(),
        aliases.len()
    );
    println!();

    if hits.is_empty() {
        // Saying so plainly is the point. A router that always returns something
        // teaches you to trust a guess.
        println!("  nothing matched. Either the base does not cover it, or the");
        println!("  Search for lines do not carry the words a real question uses.");
        return ExitCode::SUCCESS;
    }

    for hit in &hits {
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
