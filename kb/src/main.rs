//! kb, a linter for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that
//! any index is derived from them. This is the first derived thing: it reads the
//! files and reports what the conventions promise but nothing was checking.

mod base;
mod checks;

use base::Base;
use checks::{Finding, Level};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
kb, a linter for file based knowledge bases

usage:
    kb check [path]... [--strict] [--all]

    path        base to check, defaults to the current directory
    --strict    count warnings toward the exit code
    --all       include files git does not track, normally the private layer

checks:
    E01 broken-link     a [[link]] with no file behind it
    E02 not-indexed     a note in the knowledge folder with no entry in the map
    E03 no-map          no MAP.md, INDEX.md or MAPA.md at the root
    W01 ambiguous-link  a [[link]] matching more than one file
    W02 no-search-line  a map entry with no Search for line
    W03 dash            an em dash or en dash, which house style forbids
    W04 front-matter    a note declaring a source with no evidence_tier or valid_for

exit code is 1 when there are errors, or when --strict and there are warnings.
";

/// How many line numbers to print before collapsing into a count.
const LINES_SHOWN: usize = 3;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    if args[0] != "check" {
        eprintln!("kb: unknown command '{}'\n", args[0]);
        print!("{USAGE}");
        return ExitCode::from(2);
    }

    let strict = args.iter().any(|a| a == "--strict");
    let all = args.iter().any(|a| a == "--all");

    let mut paths: Vec<&str> = args[1..]
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .collect();
    if paths.is_empty() {
        paths.push(".");
    }

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

    let label = base
        .root
        .canonicalize()
        .unwrap_or_else(|_| base.root.clone())
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    let map = base.map.clone().unwrap_or_else(|| "none".to_string());
    let scope = if base.tracked_only {
        "tracked files"
    } else {
        "files, git not consulted"
    };
    println!("{label}  ({} {scope}, map: {map})", base.files.len());

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

struct Group<'a> {
    level: Level,
    code: &'static str,
    file: &'a str,
    message: &'a str,
    lines: Vec<usize>,
}

/// Collapses findings that repeat the same message in the same file.
///
/// Two hundred separate lines saying "em dash" is not two hundred findings, it
/// is one finding with a count, and printing it the long way buries everything
/// else. The count is always shown, so nothing is hidden by the collapse.
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
