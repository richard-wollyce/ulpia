//! kb, a linter and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any index is derived
//! from them. This is that derived thing: it stores nothing, reads the files on every run, and either
//! reports what is broken (`check`) or answers which files a question should open (`route`).


use kb::checks::{Finding, Level};
use kb::{
    answer, base, blocks, boot, checks, classify, commit, eval, index, init, mcp, memory,
    promote, remember, store, ui, write,
};
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
    kb init --person [fleet-root]
    kb write <agent> <slug> [fleet-root] --keys <a, b> --summary <one line> [--folder F]
    kb fleet [path]... [--all]
    kb blocks [path] [--emit]
    kb eval <gold.tsv> [path]... [--top N] [--all] [--classify]
    kb commit <path>... -m <message>
    kb boot [path]... [--top N] [--all]
    kb promote [path]... [--top N] [--all] [--dry-run] [--max N] [--lock]
    kb ui [path]... [--port N] [--all]
    kb serve [path]... [--top N] [--all]

    path        base to work on, defaults to the current directory
    --emit      blocks: print the assembled resident constitution instead of the report
    --strict    check: count warnings toward the exit code
    --all       include files git does not track, normally the private layer
    --top N     how many candidates to carry, default 5. route, boot, eval,
                promote and serve all read it
    --keys      write: the words a real question would use. Required, no way to skip
    --summary   write: one line saying what the note is about, for its `Exists to:`
                header and for the map entry
    --folder    write: where under the agent it lands, default knowledge
    --provenance write: human, agent or external. Default agent
    --stage     write: raw, distilled or derived. Default derived
    -m          commit: the message. Required, and so is at least one path

commit exists because more than one session writes these repositories at once.
It commits exactly the paths you name and then reads the commit back to prove it,
so another session's in-flight work cannot be swept into your message. There is
deliberately no flag meaning everything.

answer has three table sizes, chosen by the caller and never guessed:
    (default)    fast search: the top five files, one model call
    --expanded   the bigger table: up to twelve files, one call, for evidence
                 spread across several files
    --complete   the whole base, read in batches and composed: for questions whose
                 answer is crumbs across many files (how many, which ones, sum it
                 up). Costs one model call per batch plus one; the estimate prints
                 before anything runs, and rides the output so a model reading it
                 through another surface gets the same warning a person does

answer asks the question, then hands what retrieval found to the model named by
`answerer = ...` in the fleet manifest, which must ground every claim in the served
passages and say plainly when they do not hold the answer. A `nothing` verdict never
reaches the model, and without the manifest line the command prints the reading list
`kb route` would have printed. The model sits after the verdict, never inside
retrieval, so ADR-0018 stands.

promote reads each agent's inbox/ and offers what it finds to two promoters. The first
proposes notes and never sees the base; the second decides, three times through three
different questions, and never sees the first one's reasoning. Only a unanimous accept
writes, at stage `captured`. A refusal is counted in kb-rejections.txt, because the same
proposal refused three times is a gap in the base rather than a bad proposal.
    --dry-run   decide everything and write nothing
    --max N     stop after N proposals are admitted and leave the rest of the deposit
                where it is. A bound on the blast radius of a run nobody is watching,
                counted the same in a dry run so the cap can be rehearsed
    --lock      refuse to start while another run holds .kb-promote.lock. Needed when
                promotion runs from a session-end hook, because sessions end together
                and two runs over one deposit both propose the same note before either
                has written it, which is a duplicate no lens can see

write reads the note body from stdin and writes the keys twice: into the note's own
`**Search for:**` header, which since ADR-0028 is the only thing the router indexes,
and into an entry in the agent's MAP.md, which is a reading list for a person. Keys
are required and there is no flag to skip them, because a file with no `Search for:`
line builds no index entry and scores zero on every question. The map is optional to
`kb check` and to the index, and not to this command: write refuses when the agent
has no MAP.md, and a failed map write deletes the note again rather than leaving one
half behind.

Each agent keeps its own index at <agent>/.kb/index.db. There is no shared index
and no --db flag: which database you get used to depend on where you were standing,
and that cost three separate incidents.
    --json      index: print the index entries as JSON instead of building the index
    --hybrid    route: fuse the keyword scorer with full text search over chunks

serve speaks MCP over stdio, so Claude Code, Claude Desktop or any other MCP client
can search the base. It never serves what git ignores unless --all says so, and a
base where git could not be consulted is left out of the open with a notice on
stderr rather than served, because unknown is not public. The rest of the fleet
still opens: one husk from a rename in progress used to close every base.

checks:
    E01 broken-link     a [[link]] with no file behind it
    E02 not-indexed     a file with no `Search for:` line, so the index has no entry
    W01 ambiguous-link  a [[link]] matching more than one file
    W02 no-search-line  a map entry with no Search for line, where a map exists
    W06 thin-keywords   a `Search for:` line too short to be found by a real question
    W07 dead-key        a key that reaches neither the keyword nor the phrase index
    W03 dash            an em dash or en dash, which house style forbids
    W04 front-matter    front matter declaring source or type with no evidence_tier
                        or valid_for
    W05 no-provenance   a note with no provenance or stage, so who wrote it is unknown
    E04 bad-provenance  provenance or stage carries a value outside the legal set

E02, W06 and W07 are asked of every file the index walks, not only of the knowledge
folder, and they skip the files nobody searches for: README.md, MAP.md, CLAUDE.md,
what-goes-here.md, MOVED.md, and anything under inbox/ or records/. There is no rule
demanding a MAP.md any more; the index walks files and a base without one indexes.

exit code is 1 when check finds errors, or when --strict and it finds warnings.
";

/// How many line numbers to print before collapsing into a count.
const LINES_SHOWN: usize = 3;

/// Flags that consume the argument after them.
const VALUE_FLAGS: &[&str] =
    &["--top", "--keys", "--summary", "--folder", "--provenance", "--stage", "-m", "--port", "--max"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let all = args.iter().any(|a| a == "--all");
    let strict = args.iter().any(|a| a == "--strict");
    let top = flag_value(&args, "--top").and_then(|v| v.parse().ok()).unwrap_or(5);

    let rest: Vec<&str> = args[1..].iter().map(|a| a.as_str()).collect();
    let positional = positionals(&rest);

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
        "init" if args.iter().any(|a| a == "--person") => {
            let fleet = positional.first().copied().unwrap_or(".");
            match init::person(Path::new(fleet), None) {
                Ok(made) => {
                    println!("wrote {}", made.path.display());
                    println!("  {} files, no agent.txt: the router reads it and can never", made.files);
                    println!("  choose it as the one who answers, because a person is not an agent.");
                    println!();
                    println!("Next: fill core.md. Every agent carries it resident, so a fleet");
                    println!("whose person is blank gives generic answers confidently.");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("kb: {e}");
                    ExitCode::from(1)
                }
            }
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
        "write" => {
            if positional.len() < 2 {
                eprintln!("kb: write needs an agent and a name for the note
");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let fleet = positional.get(2).copied().unwrap_or(".");
            cmd_write(positional[0], positional[1], Path::new(fleet), &args)
        }
        "commit" => {
            let message = flag_value(&args, "-m").unwrap_or_default();
            cmd_commit(&positional, &message)
        }
        "boot" => cmd_boot(&paths_or_default(&positional), all, top),
        "answer" => {
            if positional.is_empty() {
                eprintln!("kb: answer needs a question\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let mode = if args.iter().any(|a| a == "--complete") {
                answer::Mode::Complete
            } else if args.iter().any(|a| a == "--expanded") {
                answer::Mode::Expanded
            } else {
                answer::Mode::Fast
            };
            cmd_answer(positional[0], &paths_or_default(&positional[1..]), all, top, mode)
        }
        "promote" => cmd_promote(
            &paths_or_default(&positional),
            all,
            top,
            args.iter().any(|a| a == "--dry-run"),
            flag_value(&args, "--max").and_then(|v| v.parse().ok()),
            args.iter().any(|a| a == "--lock"),
        ),
        "ui" => {
            let port = flag_value(&args, "--port")
                .and_then(|v| v.parse().ok())
                .unwrap_or(ui::DEFAULT_PORT);
            match ui::serve(&paths_or_default(&positional), all, port) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("kb ui: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "eval" => {
            if positional.is_empty() {
                eprintln!("kb: eval needs a gold file\n");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let gold = positional[0];
            let paths = paths_or_default(&positional[1..]);
            cmd_eval(Path::new(gold), &paths, all, top, args.iter().any(|a| a == "--classify"))
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


/// The arguments that are not flags and not a flag's value.
///
/// Extracted and tested because the comment that used to sit here predicted its own
/// bug and did not prevent it. It said: a flag that takes a value swallows the argument
/// after it, otherwise that value gets read as a path and the error message blames the
/// wrong thing. The guard then tested `starts_with("--")` only, so adding the short
/// `-m` to `VALUE_FLAGS` did nothing, and `kb commit ... -m "long message"` reported
/// the entire commit message as a path that is not inside a git repository.
fn positionals<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        // Long flags, and any short flag that is known to take a value. A bare short
        // flag that takes no value is still a positional to this function, which is
        // wrong in principle and has no instance today; the day one exists it belongs
        // in a list beside VALUE_FLAGS rather than in a new prefix rule.
        if arg.starts_with("--") || VALUE_FLAGS.contains(arg) {
            skip_next = VALUE_FLAGS.contains(arg);
            continue;
        }
        out.push(*arg);
    }
    out
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

/// Writes a note and the map entry that makes it reachable, as one act.
///
/// The body comes from stdin rather than a flag, because a note is markdown with
/// blank lines and headings in it and a shell argument is the wrong shape for that.
/// The keys come from a flag and are required: see `write.rs` for why there is no
/// way to skip them.
fn cmd_write(agent: &str, slug: &str, fleet: &Path, args: &[String]) -> ExitCode {
    use std::io::Read;

    let keys: Vec<String> = flag_value(args, "--keys")
        .unwrap_or_default()
        .split(',')
        .map(|k| k.trim().trim_matches('`').to_string())
        .filter(|k| !k.is_empty())
        .collect();

    let mut body = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut body) {
        eprintln!("kb: cannot read the note from stdin: {e}");
        return ExitCode::from(1);
    }

    let spec = write::Note {
        summary: flag_value(args, "--summary").unwrap_or_default(),
        keys,
        folder: flag_value(args, "--folder").unwrap_or_else(|| "knowledge".to_string()),
        provenance: flag_value(args, "--provenance").unwrap_or_else(|| "agent".to_string()),
        stage: flag_value(args, "--stage").unwrap_or_else(|| "derived".to_string()),
        body,
    };

    match write::note(fleet, agent, slug, &spec) {
        Ok(made) => {
            println!("wrote {}", made.note.display());
            println!("  listed in {} under {}", made.map.display(), made.section);
            if made.section_created {
                println!("  that section did not exist and was created");
            }
            match made.staged {
                write::Staged::Yes => println!("  staged, so the router can serve it"),
                write::Staged::Ignored => println!(
                    "  a .gitignore rule covers it, so it stays private and the public
                       scope will not serve it. That is the private layer working."
                ),
                write::Staged::NoGit => eprintln!(
                    "  NOT staged: no git here, so nothing can tell which files are
                       private, and kb refuses to open such a base at all. Run `git init`."
                ),
                write::Staged::Failed(ref e) => eprintln!("  NOT staged: {e}"),
            }
            // Said rather than done. The index is derived and rebuilding it is cheap,
            // but a command that quietly rewrote a database while you were writing a
            // note is a command that does two things under one name.
            println!();
            println!("Next: `kb index` to make it findable, then `kb check` to be sure");
            println!("the note says what the entry claims it says.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            ExitCode::from(1)
        }
    }
}

/// Creates an agent in the shape ADR-0011 defines, under `<root>/fleet/<name>`.
fn cmd_init(name: &str, fleet: &Path) -> ExitCode {
    match init::agent(fleet, name, None) {
        Ok(made) => {
            println!("created {}", made.path.display());
            println!("  {} files and directories", made.files);
            match made.vcs {
                init::Vcs::Initialised => println!("  git initialised"),
                // Said out loud rather than left implicit, because the alternative was
                // a repository nested inside the fleet's own, and nothing complains
                // about that until a gitlink is committed months later.
                init::Vcs::Enclosing => {
                    println!("  a repository already covers this path, so none was created:");
                    println!("  the files are staged into it and yours is the commit to write.");
                }
                // Not cosmetic. The privacy gate reads `git ls-files` per base, so a
                // base outside git has no knowable private layer and `Memory::open`
                // refuses to serve it.
                init::Vcs::Untouched => {
                    eprintln!("  git could NOT be run here. Put this base under a repository");
                    eprintln!("  before serving the agent, or its private layer is unknowable.");
                }
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

    // Every base's stems, gathered before any base is graded, so a broken link can be
    // told the one thing that makes it actionable: whether the note exists somewhere
    // else. The rule does not change, the message does. See `elsewhere`.
    let mut stems: Vec<(String, Vec<String>)> = Vec::new();
    for path in &bases {
        if let Ok(base) = Base::discover(path, all) {
            let name = label(&base).to_string();
            stems.push((name, base.files.iter().map(|f| f.stem.clone()).collect()));
        }
    }

    for (i, path) in bases.iter().enumerate() {
        if i > 0 {
            println!();
        }
        match check_one(path, all, &stems) {
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

/// Which other base holds a note by this stem, if any.
///
/// **This does not relax the rule, it teaches it.** A `[[wikilink]]` resolves inside its
/// own base and nowhere else, because a base is a privacy boundary and a link that
/// crosses one silently is how private material gets referenced from a publishable file.
/// So a link to another base's note is still an error. It is simply an error with an
/// obvious fix, and saying "this lives in yaron, write the path" is the difference
/// between a rule that gets followed and a rule that gets worked around.
fn elsewhere(stems: &[(String, Vec<String>)], home: &str, target: &str) -> Option<String> {
    stems
        .iter()
        .find(|(name, list)| {
            name != home && list.iter().any(|s| s.eq_ignore_ascii_case(target))
        })
        .map(|(name, _)| name.clone())
}

fn check_one(
    path: &Path,
    all: bool,
    stems: &[(String, Vec<String>)],
) -> std::io::Result<(usize, usize)> {
    let base = Base::discover(path, all)?;
    let mut findings = checks::run(&base);

    let home = label(&base).to_string();
    for finding in &mut findings {
        if finding.code != "E01" {
            continue;
        }
        // The target is between the brackets in the message the check built.
        let Some(open) = finding.message.find("[[") else { continue };
        let Some(close) = finding.message[open..].find("]]") else { continue };
        let target = &finding.message[open + 2..open + close];
        if let Some(other) = elsewhere(stems, &home, target) {
            finding.message = format!(
                "broken link [[{target}]]: that note is in {other}, not here. A wikilink \
                 stops at the base edge, so write the path instead"
            );
        }
    }

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

    // The notice this loop used to print now comes out of `Memory::open` itself, so every
    // surface gets it instead of this one.
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
// boot
// ---------------------------------------------------------------------------

/// The `UserPromptSubmit` hook entry point: the fleet routing a message before the model
/// sees it.
///
/// **Always exits 0.** Exit 2 on this event blocks the prompt and erases it, and exit 1
/// shows the user a hook error. Neither is an acceptable outcome for a routing step that
/// failed: the message is the user's and it must reach the model whatever the router
/// thinks. So every failure path here prints nothing and succeeds.


/// Complete mode: the estimate first, then the map batches, then the reduce.
///
/// The estimate is not a courtesy, it is the mode's contract: "this will read all N
/// files in M model calls" prints before the first call, and the same line leads the
/// final output, because on surfaces where no person watches the screen (an agent
/// calling through a shell or MCP) the model reading the output deserves the warning
/// a person got. Timing is stated per run rather than promised: the first batch is
/// timed and the remainder estimated from it.
fn cmd_answer_complete(question: &str, memory: &memory::Memory, root: &Path) -> ExitCode {
    let answerer = memory.answerer();
    if matches!(answerer, classify::Classifier::None) {
        eprintln!("kb answer --complete: no `answerer = ...` in the fleet manifest, and this");
        eprintln!("mode is nothing but model calls. Configure one or use the default mode.");
        return ExitCode::from(2);
    }

    let plan = answer::complete_plan(memory);
    if plan.files.is_empty() {
        println!("the base serves no keyed files, so there is nothing to read.");
        return ExitCode::SUCCESS;
    }
    println!(
        "complete search: reading all {} files in {} batch(es), {} model call(s) total.",
        plan.files.len(),
        plan.batches,
        plan.batches + 1
    );
    println!("This is the slow mode by design; timing follows the first batch.");

    let mut facts = String::new();
    let mut batch: Vec<(String, String)> = Vec::new();
    let mut done = 0usize;
    let started = std::time::Instant::now();
    let mut first_batch_ms: Option<u128> = None;

    let mut flush = |batch: &mut Vec<(String, String)>, facts: &mut String, done: &mut usize| -> u128 {
        if batch.is_empty() {
            return 0;
        }
        let t = std::time::Instant::now();
        let p = answer::map_prompt(question, batch);
        match promote::ask_model(&answerer, root, &p) {
            Some(reply) => {
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
            None => {
                eprintln!("kb answer --complete: a map batch got no reply; its files are");
                eprintln!("missing from the answer, which is now incomplete by that much.");
            }
        }
        *done += 1;
        t.elapsed().as_millis()
    };

    let total_batches = plan.batches;
    for (name, path) in &plan.files {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        batch.push((name.clone(), text));
        if batch.len() >= answer::BATCH {
            let ms = flush(&mut batch, &mut facts, &mut done);
            if first_batch_ms.is_none() {
                first_batch_ms = Some(ms);
            }
            if done == 1 {
                // The estimate, from the one batch actually timed: honest arithmetic,
                // not a promise, and restated because network and model load move it.
                if let Some(ms) = first_batch_ms {
                    eprintln!(
                        "  batch 1/{} took {:.1}s; at that pace the whole read is ~{:.1} min",
                        total_batches,
                        ms as f64 / 1000.0,
                        (ms as f64 / 1000.0) * (total_batches + 1) as f64 / 60.0
                    );
                }
            } else {
                eprintln!("  batch {done}/{total_batches} done");
            }
            batch.clear();
        }
    }
    let _ = flush(&mut batch, &mut facts, &mut done);

    if facts.trim().is_empty() {
        println!();
        println!("complete search read every file and found nothing bearing on the question.");
        println!("The library does not hold this.");
        return ExitCode::SUCCESS;
    }

    let reduce = answer::reduce_prompt(question, &facts);
    match promote::ask_model(&answerer, root, &reduce) {
        Some(text) if !text.trim().is_empty() => {
            println!();
            println!("{}", text.trim());
            println!();
            println!(
                "mode: complete | read {} files in {} model calls | {:.1}s total",
                plan.files.len(),
                done + 1,
                started.elapsed().as_secs_f64()
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("kb answer --complete: the reduce call failed; the extracted facts:");
            eprintln!("{facts}");
            ExitCode::from(1)
        }
    }
}

/// `kb promote`: the deposit becomes knowledge, or it does not and says why.
///
/// The whole design is in `promote.rs`. What lives here is the reporting, and it reports
/// refusals as loudly as writes: a promotion run whose output is only what it wrote is a
/// run that looks successful when it accepted everything.

/// `kb answer`: retrieval's findings, written up by the manifest's answerer.
///
/// The command is three refusals wrapped around one model call, in this order: no
/// passages means no call (fabrication needs a vacuum), no answerer means the reading
/// list (the fleet never stops answering because a model is missing), and an answerer
/// that fails mid-call means the reading list too, said out loud.
fn cmd_answer(question: &str, paths: &[&str], all: bool, top: usize, mode: answer::Mode) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };
    let root = given.first().copied().unwrap_or_else(|| Path::new("."));

    // Complete mode never goes through the top-k table at all: its whole point is
    // that ranking starves aggregation. It reads the base, warned and estimated.
    if mode == answer::Mode::Complete {
        return cmd_answer_complete(question, &memory, root);
    }

    let a = memory.ask(question, top.max(mode.files()));

    if !answer::worth_asking(&a.confidence, &a.found) {
        // The same refusal `kb route` gives, with the same suggestions: absence is an
        // answer, and it costs zero model calls.
        println!("nothing in the library matched. Either it does not cover this, or the");
        println!("Search for lines do not carry the words the question used.");
        let looked_like = memory.suggest(question, 8);
        if !looked_like.is_empty() {
            println!("  it does know: {}", looked_like.join(", "));
        }
        return ExitCode::SUCCESS;
    }

    let answerer = memory.answerer();
    if matches!(answerer, classify::Classifier::None) {
        println!("no `answerer = ...` in the fleet manifest, so here is the reading list:");
        println!();
        for (i, f) in a.found.iter().take(5).enumerate() {
            println!("  {}. {}/{}", i + 1, f.base, f.path);
        }
        return ExitCode::SUCCESS;
    }

    let p = answer::prompt(question, &a, mode);
    match promote::ask_model(&answerer, root, &p) {
        Some(text) if !text.trim().is_empty() => {
            println!("{}", text.trim());
            println!();
            println!("{}", answer::sources_line(&a, mode));
            println!(
                "mode: {} | confidence: score {:.1} vs floor {:.1}; {}",
                mode.label(),
                a.confidence.keyword_score,
                memory::SCORE_FLOOR,
                match a.confidence.verdict {
                    memory::Verdict::Hit => "hit",
                    memory::Verdict::Guess => "guess, read the sources yourself",
                    memory::Verdict::Nothing => "nothing",
                }
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("kb answer: the answerer did not reply, so here is the reading list:");
            for (i, f) in a.found.iter().take(5).enumerate() {
                eprintln!("  {}. {}/{}", i + 1, f.base, f.path);
            }
            ExitCode::from(1)
        }
    }
}

fn cmd_promote(
    paths: &[&str],
    all: bool,
    top: usize,
    dry_run: bool,
    max: Option<usize>,
    lock: bool,
) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };

    let root = given.first().copied().unwrap_or_else(|| Path::new("."));
    let promoter = memory.promoter();
    let reviewer = memory.reviewer();

    // **Both must be configured, and they must not be assumed.** A promotion run with no
    // reviewer is automatic extraction into the durable base, which is the exact thing
    // `promote.rs` was written to not be. Refusing here is cheaper than discovering it in
    // a diff a week later.
    if matches!(promoter, classify::Classifier::None) {
        eprintln!("kb promote: no `promoter = ...` in the fleet manifest, so there is nothing to propose with.");
        return ExitCode::from(2);
    }
    if matches!(reviewer, classify::Classifier::None) {
        eprintln!(
            "kb promote: no `reviewer = ...` in the fleet manifest. Running the proposer alone \
             would write straight into the base from unreviewed material, which is what this \
             command exists to not do."
        );
        return ExitCode::from(2);
    }

    // Taken after both promoters are known to be configured and before the first model
    // call, so a misconfigured fleet does not leave a marker behind for the run that would
    // have worked. Held by the binding until this function returns: `Lock` releases on
    // drop, which covers the early returns below without any of them remembering to.
    let _held = if lock {
        match promote::Lock::take(root) {
            Ok(l) => {
                if let Some(note) = &l.took_over {
                    eprintln!("kb promote: {note}");
                }
                Some(l)
            }
            Err(e) => {
                // Exit 0, not an error. Declining because a run is already in flight is
                // this flag doing its job, and a hook that reports success only when it
                // did work is a hook whose failures nobody can find.
                println!("kb promote: {e} Nothing was read and nothing was written.");
                return ExitCode::SUCCESS;
            }
        }
    } else {
        None
    };

    let today = today();
    let outcome = promote::run(&memory, root, &promoter, &reviewer, top, dry_run, &today, max);

    if dry_run {
        println!("dry run: nothing was written and no refusal was recorded.\n");
    }

    for d in &outcome.decided {
        let head = format!("{}/{}", d.proposal.agent, d.proposal.slug);
        if d.accepted() {
            match &d.written {
                Some(p) => println!("  wrote   {head}\n          {}", p.display()),
                None => println!("  would write {head}"),
            }
        } else {
            println!("  refused {head}");
            // Every lens that refused, not only the first. Reporting one made it look
            // like the contradiction lens was doing all the work, when what was really
            // happening is that it had been given duplication's question.
            for r in d.refusals() {
                println!("          {} says: {}", r.lens.name(), r.reason);
            }
        }
        println!("          from {}", d.proposal.source);
    }

    for b in &outcome.barren {
        println!("  nothing worth keeping in {b}");
    }

    // Degraded and silent is the combination this repository keeps paying for.
    for u in &outcome.unreachable {
        eprintln!("kb promote: could not reach {u}, so nothing was written for it.");
    }

    println!(
        "\n{} proposal(s): {} written, {} refused. {} deposit file(s) held nothing.",
        outcome.decided.len(),
        outcome.written(),
        outcome.refused(),
        outcome.barren.len()
    );
    if outcome.refused() > 0 && !dry_run {
        println!("Refusals are counted in {}.", promote::REJECTIONS_TXT);
    }
    // Said out loud, because the counts above look identical to a run that finished.
    if let Some(cap) = outcome.stopped_at {
        println!(
            "Stopped at the cap of {cap}. The rest of the deposit was not read and is \
             still there. Run again, or raise --max, once you have looked at these."
        );
    }

    if outcome.unreachable.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

/// Today, as YYYY-MM-DD, for the rejection record.
///
/// Days since the epoch, converted by the civil-from-days algorithm. The crate has one
/// dependency and it is not a date library, which is a constraint ADR-0001 set and this is
/// not the feature worth spending it on.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs / 86_400 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn cmd_boot(paths: &[&str], all: bool, top: usize) -> ExitCode {
    use std::io::Read;

    let mut stdin = String::new();
    if std::io::stdin().read_to_string(&mut stdin).is_err() {
        return ExitCode::SUCCESS;
    }
    let Some(req) = boot::parse_request(&stdin) else {
        return ExitCode::SUCCESS;
    };

    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let Ok(memory) = memory::Memory::open(&given, all) else {
        return ExitCode::SUCCESS;
    };

    let root = given.first().copied().unwrap_or_else(|| Path::new("."));
    let briefing = boot::brief(&memory, root, &req, top);
    print!("{}", briefing.text);
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// commit
// ---------------------------------------------------------------------------

fn cmd_commit(paths: &[&str], message: &str) -> ExitCode {
    let owned: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
    match commit::commit(&owned, message) {
        Ok(done) => {
            println!("committed {}", done.sha);
            for f in &done.files {
                println!("  {f}");
            }
            // Printed on every success, because "I did not take your files" is a claim
            // that should arrive with its evidence rather than as reassurance.
            if done.left_alone.is_empty() {
                println!("
nothing else was dirty in this repository.");
            } else {
                println!("
left untouched, still dirty ({}):", done.left_alone.len());
                for f in &done.left_alone {
                    println!("  {f}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            ExitCode::from(1)
        }
    }
}

// ---------------------------------------------------------------------------
// eval
// ---------------------------------------------------------------------------

fn cmd_eval(gold_path: &Path, paths: &[&str], all: bool, top: usize, classify: bool) -> ExitCode {
    let rows = match eval::read_gold(gold_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(2);
        }
    };

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

    // Before any grading. A gold file pointing at files that moved produces a precise
    // and entirely wrong verdict, which is exactly what the decision records' move
    // set up and nothing caught.
    let stale = eval::stale_answers(&rows, &memory);
    if !stale.is_empty() {
        eprintln!("kb: {} gold answers name files this fleet does not have:", stale.len());
        for path in &stale {
            eprintln!("      {path}");
        }
        eprintln!("    Grading against a stale gold file reports a wrong number confidently.");
        return ExitCode::from(1);
    }

    if classify {
        eprintln!("kb eval: asking the classifier about every question, one model call each.");
    }
    let root = given.first().copied().unwrap_or_else(|| Path::new("."));
    let graded = eval::run_with(&memory, &rows, top, classify, root);
    let s = eval::summarise(&graded);

    println!("gold:     {} questions, {} answerable", graded.len(), s.answerable);
    println!("indexed:  {} entries across {} agents", memory.entry_count(), memory.agents.len());
    println!();

    // File and agent are shown as separate columns on purpose. Collapsing them into
    // one verdict hid the case that matters most here: the top file being right while
    // the agent aggregate points elsewhere, which is a routing failure that a file
    // level "ok" would have concealed.
    println!("  file agent kw    score  marg verdict question");
    for row in &graded {
        let (file, agent_mark) = if row.expects_abstention() {
            let ok = row.confidence.verdict != memory::Verdict::Hit;
            (if ok { "ok" } else { "BAD" }, "-")
        } else {
            (
                if row.file_hit() { "ok" } else { "MISS" },
                // The router's own choice, so the column a reader scans for failures is
                // the same router the headline reports. It showed the fused fold while the
                // summary above it reported routing, so the two disagreed on which
                // questions missed.
                if row.keyword_agent_hit() { "ok" } else { "MISS" },
            )
        };
        println!(
            "  {:<4} {:<5} {:<4} {:>6.1} {:>5.2} {:<6} {}",
            file,
            agent_mark,
            if row.expects_abstention() {
                "-"
            } else if row.keyword_hit() {
                "ok"
            } else {
                "MISS"
            },
            row.confidence.keyword_score,
            row.confidence.margin,
            match row.confidence.verdict {
                memory::Verdict::Hit => "hit",
                memory::Verdict::Guess => "guess",
                memory::Verdict::Nothing => "none",
            },
            row.question
        );
    }
    println!();

    let pct = |n: usize, d: usize| if d == 0 { 0.0 } else { 100.0 * n as f64 / d as f64 };
    println!(
        "FILE   fused   {}/{}  ({:.0}%)",
        s.file_hits,
        s.answerable,
        pct(s.file_hits, s.answerable)
    );
    println!(
        "       keyword {}/{}  ({:.0}%), the same question asked of the keyword scorer alone",
        s.keyword_hits,
        s.answerable,
        pct(s.keyword_hits, s.answerable)
    );
    // **The routing line goes first because it is the one that ships.** These two read as
    // headline and footnote, and they were the other way round: the fused number sat where
    // the eye lands while `Memory::ask` populates `Answer.agent` from
    // `choose_agent_by_keyword` (memory.rs) and `boot::brief` routes on that. A day of
    // measurements was quoted off the top line before anybody checked which function it
    // called. An instrument that is easy to misread is a broken instrument.
    // **The whole decision, not one of its arms.** This printed the arithmetic fold twice
    // over, first the fused one and then the keyword one, and both are the FALLBACK: with a
    // classifier configured `boot::brief` routes on the verdict's owner and only reaches the
    // arithmetic through `.or_else`. Calling the fallback "the choice the hook actually
    // makes" measured what happens when the model is down.
    println!(
        "AGENT  routes  {}/{}  ({:.0}%), what boot hands over, classifier included",
        s.routed_hits,
        s.ownable,
        pct(s.routed_hits, s.ownable)
    );
    println!(
        "       keyword {}/{}  ({:.0}%), the deterministic fold alone, which is the fallback          when no classifier answers",
        s.keyword_agent_hits,
        s.ownable,
        pct(s.keyword_agent_hits, s.ownable)
    );
    println!(
        "       fused   {}/{}  ({:.0}%), the same fold over the fused list, which ADR-0018 \
         measured and rejected. Kept so a regression in it cannot hide",
        s.agent_hits,
        s.ownable,
        pct(s.agent_hits, s.ownable)
    );
    if s.ownable < s.answerable {
        println!(
            "       ({} question(s) excluded: answered only from an attached base, which              can be read but cannot be the agent who answers)",
            s.answerable - s.ownable
        );
    }
    if s.classified_asked > 0 {
        println!(
            "       model     {}/{}  ({:.0}%), a classifier reading the roster and the evidence",
            s.classified_hits,
            s.ownable,
            pct(s.classified_hits, s.ownable)
        );
    }
    println!(
        "       always-{}   {}/{}  ({:.0}%), the best a fixed choice can do on this set",
        s.baseline_agent,
        s.baseline_hits,
        s.ownable,
        pct(s.baseline_hits, s.ownable)
    );
    // **Against the number that ships, not the better of two.** This was
    // `agent_hits.max(keyword_agent_hits)`, so the headline claimed whichever variant
    // happened to be ahead. It read correctly only because the routing choice is currently
    // the better one; the day the rejected fold won, the report would have credited routing
    // with a score no user ever gets. A summary allowed to pick its own number is not a
    // measurement.
    let delta = s.routed_hits as i64 - s.baseline_hits as i64;
    println!(
        "       routing beats the fixed choice by {}{} question{}",
        if delta >= 0 { "+" } else { "-" },
        delta.abs(),
        if delta.abs() == 1 { "" } else { "s" }
    );
    println!();

    println!("GATE   flagged {}/{} of its own misses as a guess", s.misses_flagged, s.misses_total);
    println!(
        "       demoted {}/{} correct answers to a guess",
        s.hits_demoted, s.gate_denominator
    );
    // **Every abstain row, not the first one.** This said "the abstain question", singular,
    // and graded whichever one `find` returned. The set holds three, the per-question table
    // grades all three, and two of them could regress to Hit without this line moving.
    if s.abstention_expected > 0 {
        println!(
            "       abstained on {}/{} question(s) the set says to decline",
            s.abstention_correct, s.abstention_expected
        );
        if s.classified_asked > 0 {
            println!(
                "       model     {}/{}, and this is the answer it exists to give",
                s.classified_abstention_correct, s.abstention_expected
            );
        }
    }

    // The separation question in one line, which is what decides whether the floor is
    // a real threshold or a number sitting in the middle of one distribution.
    let range = |v: &[f32]| {
        let lo = v.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = v.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (lo, hi)
    };
    if !s.hit_scores.is_empty() && !s.miss_scores.is_empty() {
        let (hlo, hhi) = range(&s.hit_scores);
        let (mlo, mhi) = range(&s.miss_scores);
        println!("       hit scores  {hlo:.2} to {hhi:.2}");
        println!("       miss scores {mlo:.2} to {mhi:.2}");
        if hlo > mhi {
            println!(
                "       SEPARATES: every hit outscored every miss. Floor {:.1} sits in the gap.",
                memory::SCORE_FLOOR
            );
        } else {
            println!("       OVERLAPS: no floor tells a hit from a miss on this set.");
        }
    }
    println!();

    let n = graded.len().max(1) as u128;
    println!(
        "SPEED  {} us per question fused, {} us keyword only. No model, no network.",
        s.total_micros / n,
        s.keyword_micros / n
    );
    // **The classifier's own time, which this block used to leave out entirely.** It timed
    // `Memory::ask` and printed "No model, no network" beside the number, which was true of
    // what it measured and false about what the hook runs. The classifier is where the wall
    // clock lives: milliseconds against seconds, and the ratio is the whole argument for or
    // against configuring one.
    if s.classified_asked > 0 {
        let cls = s.classified_micros / n;
        println!(
            "       {} us per question in the classifier, {}x the arithmetic. That is the \
             cost of the model, and the hook pays it on every message",
            cls,
            if s.total_micros > 0 { cls / (s.total_micros / n).max(1) } else { 0 }
        );
    }
    println!(
        "       {}",
        if cfg!(debug_assertions) {
            "MEASURED ON THE DEBUG BINARY. Release is materially faster; do not quote this."
        } else {
            "Release binary."
        }
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this function was extracted for: a multi line commit message reaching
    /// `kb commit` as a path, because the flag guard only recognised `--` prefixes.
    #[test]
    fn a_short_flags_value_is_not_a_path() {
        let args = ["commit", "a.md", "-m", "a message

with a body"];
        assert_eq!(positionals(&args[1..]), vec!["a.md"]);
    }

    #[test]
    fn a_long_flags_value_is_not_a_path() {
        let args = ["route", "question", "--top", "8", "."];
        assert_eq!(positionals(&args[1..]), vec!["question", "."]);
    }

    /// A flag that takes no value must not swallow the argument after it, or the last
    /// path on every `--all` command line quietly disappears.
    #[test]
    fn a_valueless_flag_swallows_nothing() {
        let args = ["check", "--all", "fleet/zed"];
        assert_eq!(positionals(&args[1..]), vec!["fleet/zed"]);
    }

    #[test]
    fn several_value_flags_in_a_row_are_all_consumed() {
        let args = ["write", "zed", "note", "--keys", "a, b", "--summary", "one line", "."];
        assert_eq!(positionals(&args[1..]), vec!["zed", "note", "."]);
    }
}
