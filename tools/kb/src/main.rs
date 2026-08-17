//! kb, a linter and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any index is derived
//! from them. This is that derived thing: it stores nothing, reads the files on every run, and either
//! reports what is broken (`check`) or answers which files a question should open (`route`).


use kb::checks::{Finding, Level};
use kb::{base, blocks, checks, index, init, mcp, memory, remember, store};
use base::Base;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
kb, a linter and router for file based knowledge bases

usage:
    kb check [path]... [--strict] [--all]
    kb index [path]... [--json] [--all]
    kb route <question> [path]... [--top N] [--hybrid] [--all]
    kb remember <claim> [path]... [--all]
    kb init <name> [fleet-root]
    kb fleet [path]... [--all]
    kb blocks [path] [--emit]
    kb serve [path]... [--top N] [--all]

    path        base to work on, defaults to the current directory
    --emit      blocks: print the assembled resident constitution instead of the report
    --strict    check: count warnings toward the exit code
    --all       include files git does not track, normally the private layer
    --top N     route: how many candidates to print, default 5

Each agent keeps its own index at <agent>/.kb/index.db. There is no shared index
and no --db flag: which database you get used to depend on where you were standing,
and that cost three separate incidents.
    --json      index: print the map entries as JSON instead of building the index
    --hybrid    route: fuse the keyword scorer with full text search over chunks

serve speaks MCP over stdio, so Claude Code, Claude Desktop or any other MCP client
can search the base. It never serves what git ignores unless --all says so, and it
refuses to start when git cannot be consulted, because unknown is not public.

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

    let json = args.iter().any(|a| a == "--json");
    let hybrid = args.iter().any(|a| a == "--hybrid");

    match args[0].as_str() {
        "check" => cmd_check(&paths_or_default(&positional), all, strict),
        "index" => cmd_index(&paths_or_default(&positional), all, json),
        "route" => {
            if positional.is_empty() {
                eprintln!("kb: route needs a question\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let question = positional[0];
            let paths = paths_or_default(&positional[1..]);
            cmd_route(question, &paths, all, top, hybrid)
        }
        "init" => {
            if positional.is_empty() {
                eprintln!("kb: init needs a name for the agent
");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let name = positional[0];
            let fleet = positional.get(1).copied().unwrap_or(".");
            cmd_init(name, Path::new(fleet))
        }
        "serve" => {
            let paths = paths_or_default(&positional);
            mcp::serve(&paths, all, top)
        }
        "fleet" => cmd_fleet(&paths_or_default(&positional), all),
        "blocks" => {
            let paths = paths_or_default(&positional);
            cmd_blocks(paths[0], args.iter().any(|a| a == "--emit"))
        }
        "remember" => {
            if positional.is_empty() {
                eprintln!("kb: remember needs a claim\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let claim = positional[0];
            let paths = paths_or_default(&positional[1..]);
            cmd_remember(claim, &paths, all)
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

/// Prints the fleet's own name and roster. The same text `kb_fleet` returns over MCP,
/// so what a model sees and what a person sees cannot drift apart.
fn cmd_fleet(paths: &[&str], all: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    match memory::Memory::open(&given, all) {
        Ok(m) => {
            print!("{}", m.describe().to_text());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            ExitCode::from(1)
        }
    }
}

/// Creates an agent in the shape ADR-0011 defines, under `<fleet>/agents/<name>`.
fn cmd_init(name: &str, fleet: &Path) -> ExitCode {
    match init::agent(fleet, name, None) {
        Ok(made) => {
            println!("created {}", made.path.display());
            println!("  {} files and directories", made.files);
            if made.git {
                println!("  git initialised");
            } else {
                // Not cosmetic. The privacy gate reads `git ls-files` per base, so a
                // base outside git has no knowable private layer and `Memory::open`
                // refuses to serve it.
                eprintln!("  git could NOT be initialised. Run `git init` here before");
                eprintln!("  serving this agent, or its private layer is unknowable.");
            }
            println!();
            println!("Next: fill in agent.txt's role, then index.md. Both are placeholders,");
            println!("and a generated file left unedited is a file nobody owns.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_check(paths: &[&str], all: bool, strict: bool) -> ExitCode {
    let mut errors = 0usize;
    let mut warnings = 0usize;

    // A fleet root expands into its agents here exactly as it does for retrieval.
    // When it did not, `kb check` on a fleet read three clean bases as one soup and
    // reported 18 errors and 276 warnings.
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let bases = memory::expand_roots(&given);

    for (i, path) in bases.iter().enumerate() {
        if i > 0 {
            println!();
        }
        match check_one(path, all) {
            Ok((e, w)) => {
                errors += e;
                warnings += w;
            }
            Err(e) => {
                eprintln!("kb: cannot read {}: {e}", path.display());
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

fn cmd_index(paths: &[&str], all: bool, json: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let roots = memory::expand_roots(&given);

    let mut bases = Vec::new();
    for root in &roots {
        match Base::discover(root, all) {
            Ok(base) => bases.push((root.clone(), base)),
            Err(e) => {
                eprintln!("kb: cannot read {}: {e}", root.display());
                return ExitCode::from(1);
            }
        }
    }

    if json {
        let entries: Vec<index::Entry> = bases.iter().flat_map(|(_, b)| index::build(b)).collect();
        print!("{}", index::to_json(&entries));
        return ExitCode::SUCCESS;
    }

    // One index per agent, written beside the agent. The shared index this replaces
    // defaulted to a path relative to the working directory, so which database you
    // got depended on where you happened to be standing, and that cost three
    // separate incidents in one week.
    let mut files = 0usize;
    let mut chunks = 0usize;
    for (root, base) in &bases {
        let name = label(base);
        let path = memory::index_path(root);

        let mut store = match store::Store::open(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("kb: cannot open {}: {e}", path.display());
                return ExitCode::from(1);
            }
        };
        match store.sync(base, &name) {
            Ok(r) => {
                println!(
                    "  {name:<10} {} reindexed, {} unchanged, {} removed, {} chunks",
                    r.reindexed, r.unchanged, r.removed, r.chunks
                );
                chunks += r.chunks;
            }
            Err(e) => {
                eprintln!("kb: indexing {name} failed: {e}");
                return ExitCode::from(1);
            }
        }
        files += base.files.len();
    }

    println!("  total: {files} files, {chunks} chunks, {} indexes", bases.len());
    ExitCode::SUCCESS
}

/// What the base knows that looks like what was asked, printed on a miss.
///
/// The miss message on its own is honest and useless: it says the keyword lines may
/// not carry the right words and gives the reader no way to find out which words
/// they do carry. This turns the dead end into the candidate space.
///
/// It reaches a typo or a cognate, because trigram overlap is a measure of spelling.
/// It never reaches a translation, and the line printed with it says so, because a
/// suggestion whose limits are not stated gets read as the whole answer.
fn print_suggestions(memory: &memory::Memory, question: &str) {
    let words = memory.suggest(question, 8);
    memory.record_miss(question, &words);
    if words.is_empty() {
        return;
    }
    println!();
    println!("  the base does know these, and they look like words you used:");
    println!("    {}", words.join(", "));
    println!("  that is spelling and not meaning, so it finds a typo or a cognate");
    println!("  and never finds a translation.");
}

/// Printed instead of a miss when there is nothing to search.
///
/// Returns true when it handled the case, so the caller stops. See
/// [`memory::Memory::is_empty`] for why these are different answers.
fn print_if_nothing_to_search(memory: &memory::Memory) -> bool {
    if !memory.is_empty() {
        return false;
    }
    println!("  this base has no knowledge files yet, so nothing could have matched.");
    println!("  That is a fact about the base, not about the question.");
    println!();
    println!("  Put markdown in the agent's knowledge/ folder, list each file in");
    println!("  MAP.md with a `Search for:` line naming the words a real question");
    println!("  would use, then run `kb index`. An entry without that line is an");
    println!("  entry nothing can reach.");
    println!();
    println!("  `kb fleet` works regardless: identity is read from fleet.txt and");
    println!("  agent.txt, never from the index.");
    true
}

fn cmd_route(question: &str, paths: &[&str], all: bool, top: usize, hybrid: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };
    if memory.index_was_rebuilt {
        eprintln!("kb: an index predated the tracked column and was emptied. Run `kb index`.");
    }

    println!("question: {question}");
    println!(
        "indexed:  {} entries across {} agents, {} aliases",
        memory.entry_count(),
        memory.agents.len(),
        memory.alias_count()
    );
    println!();

    if !hybrid {
        let hits = memory.route(question, top);
        if hits.is_empty() {
            if !print_if_nothing_to_search(&memory) {
                println!("  nothing matched. Either the base does not cover it, or the");
                println!("  Search for lines do not carry the words a real question uses.");
                print_suggestions(&memory, question);
            }
            return ExitCode::SUCCESS;
        }
        for (i, hit) in hits.iter().enumerate() {
            println!(
                "  {:>2}. {:>6.2}  {}/{}",
                i + 1,
                hit.score,
                hit.entry.base,
                hit.entry.rel
            );
            println!("      matched: {}", hit.matched.join(", "));
        }
        return ExitCode::SUCCESS;
    }

    let found = memory.retrieve(question, top);
    if found.is_empty() {
        if !print_if_nothing_to_search(&memory) {
            println!("  nothing matched, in either scorer. Either the base does not cover");
            println!("  it, or the Search for lines do not carry the words a real question");
            println!("  uses.");
            print_suggestions(&memory, question);
        }
        return ExitCode::SUCCESS;
    }

    // Agreement between the two scorers is the signal, not the magnitude. Measured on
    // three real questions: the two that routed correctly had both scorers voting and
    // the one that returned marketing psychology for "quem e voce?" had one.
    if memory.no_agreement(&found) {
        println!("  NOTE: only one scorer ranked any of these, so this is a guess rather");
        println!("  than an answer. The base may not cover the question at all.");
        println!();
    }

    for f in &found {
        println!("  {:>5.3}  {:<8} {:<44} {}", f.score, f.base, f.path, f.why.join(" + "));
        if let Some(p) = f.passages.first() {
            println!("         {}: {}", p.heading_path, p.excerpt.replace("
", " ").trim());
        }
    }

    ExitCode::SUCCESS
}


// ---------------------------------------------------------------------------
// blocks
// ---------------------------------------------------------------------------

fn cmd_blocks(path: &str, emit: bool) -> ExitCode {
    let root = Path::new(path);
    let blocks = match blocks::read(root) {
        Some(b) => b,
        None => {
            eprintln!("kb: no blocks.txt at {path}");
            return ExitCode::from(1);
        }
    };

    if emit {
        print!("{}", blocks::assemble(root, &blocks));
        return ExitCode::SUCCESS;
    }

    println!("{path}/blocks.txt");
    println!();
    println!(
        "  {:<3} {:<10} {:<10} {:>5} {:>9} {:>9} {:>11}",
        "#", "block", "mode", "files", "bytes", "~tokens", "cumulative"
    );

    let mut cumulative = 0usize;
    for (i, b) in blocks.iter().enumerate() {
        let resident = b.mode == blocks::Mode::Resident;
        if resident {
            cumulative += b.tokens();
        }
        println!(
            "  {:<3} {:<10} {:<10} {:>5} {:>9} {:>9} {:>11}",
            i + 1,
            b.name,
            if resident { "resident" } else { "on-demand" },
            b.files.len(),
            b.bytes,
            b.tokens(),
            if resident { cumulative.to_string() } else { "-".to_string() }
        );
        for m in &b.missing {
            println!("      missing file: {m}");
        }
    }

    println!();
    println!("  resident total: about {cumulative} tokens");
    println!();
    println!("  cost of changing a block, in tokens that have to be prefilled again:");
    for (name, cost) in blocks::invalidation_cost(&blocks) {
        println!("    {name:<10} {cost:>7}");
    }
    println!();
    println!("  A change invalidates its own block and everything after it, so the");
    println!("  first block is the most expensive to touch. That is why the order is");
    println!("  by how often a block changes, most stable first.");

    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// remember
// ---------------------------------------------------------------------------

fn cmd_remember(claim: &str, paths: &[&str], all: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };
    if memory.index_was_rebuilt {
        eprintln!("kb: an index predated the tracked column and was emptied. Run `kb index`.");
    }

    let a = memory.remember(claim);

    println!("claim: {claim}");
    println!("proposal: {}", a.outcome.label());
    println!("reason: {}", a.reason);

    if a.evidence.is_empty() {
        println!();
        println!("  nothing in the base overlaps this claim.");
    } else {
        println!();
        println!("evidence, closest first:");
        for e in &a.evidence {
            println!();
            println!("  {:.2} contained  {}/{}  {}", e.containment, e.base, e.path, e.heading_path);
            println!("    shared: {}", if e.shared.is_empty() { "-".into() } else { e.shared.join(", ") });
            println!("    new:    {}", if e.missing.is_empty() { "-".into() } else { e.missing.join(", ") });
            println!("    {}", e.excerpt.replace("
", " ").trim());
        }
    }

    println!();
    println!("---");
    for line in remember::DISCLAIMER.lines() {
        println!("{line}");
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
