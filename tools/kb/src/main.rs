//! kb, a linter and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any index is derived
//! from them. This is that derived thing: it stores nothing, reads the files on every run, and either
//! reports what is broken (`check`) or answers which files a question should open (`route`).


use kb::checks::{Finding, Level};
use kb::{
    answer, base, blocks, boot, capture, checks, classify, commit, eval, index, init, json, mcp,
    memory, promote, remember, store, ui, write,
};
use base::Base;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
kb, a linter and router for file based knowledge bases

usage:
    kb check [path]... [--strict] [--all]
    kb index [path]... [--json] [--all]
    kb route <question> [path]... [--top N] [--hybrid] [--json] [--all]
    kb remember <claim> [path]... [--all] [--json]
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
    kb capture [path] [--session ID]
    kb serve [path]... [--top N] [--all]

    path        base to work on, defaults to the current directory
    --emit      blocks: print the assembled resident constitution instead of the report
    --strict    check: count warnings toward the exit code
    --all       include the private layer: the folders a base declares with a
                `private =` line in agent.txt, or profile/, projects/ and records/
                when it declares nothing. `.` declares the whole base, and the
                person's base is whole by name. Nothing else is consulted, and no
                repository is needed (ADR-0034)
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

capture turns a session's record into a deposit. kb boot appends to the record on every
message: the questions the base refused and the agents it routed to. At session end,
capture writes that as one markdown file into the last routed agent's inbox/, raw and
without a Search for line, so the router never names it and every passage from it is
labelled short memory. Then promote reads it. No model runs. The session id comes from
--session, or from the hook payload on stdin, the same JSON kb boot reads. A session
that was never routed to an agent is not captured, and the record is kept: a deposit
with no owner is a question filed in a base that never saw it.

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
    --json      remember: print the proposal, its evidence and its caveat as one line
                of JSON, which is what lets an agent with no writable filesystem ask
                whether a fact is worth keeping, queue the answer, and apply it later
                on a machine that has the repository.
                index: print the index entries as JSON instead of building the index.
                route: print the whole answer as one line of JSON on stdout, which is
                what a program calls instead of parsing prose meant for a terminal. It
                always fuses both scorers, because the verdict is agreement between
                them, so --hybrid adds nothing on top of it. Diagnostics stay on
                stderr, and a base that was left out is named in `skipped` rather than
                only on stderr, because a caller that reads stdout alone must not read
                an empty result set as a base that does not cover the question.
                Branch on `gate.served`, never on the length of `results`: a refused
                answer still carries its candidates, and `gate.ranked_by_text_only`
                says whether they came from the text scorer alone, which is a base
                whose keys missed rather than a base without the subject. A refusal
                also carries `miss`, the recall loss itself, so a caller on a read only
                filesystem can keep what it could not write
    --hybrid    route: fuse the keyword scorer with full text search over chunks

serve speaks MCP over stdio, so Claude Code, Claude Desktop or any other MCP client
can search the base. It never serves the private layer unless --all says so. The
deposit, inbox/, is served and every passage from it is labelled short memory, so
a model leaning on a fact nobody has judged yet does so knowing that.

checks:
    E01 broken-link     a [[link]] with no file behind it
    E02 not-indexed     a file with no `Search for:` line, so the index has no entry
    W01 ambiguous-link  a [[link]] matching more than one file
    W02 no-search-line  a map entry with no Search for line, where a map exists
    W06 thin-keywords   a `Search for:` line too short to be found by a real question
    W08 unignored       a .gitignore is here and misses a folder declared private
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
            cmd_route(question, &paths, all, top, hybrid, json)
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
        "capture" => {
            let paths = paths_or_default(&positional);
            cmd_capture(paths[0], flag_value(&args, "--session").as_deref())
        }
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
            cmd_remember(claim, &paths, all, json)
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
            println!("  served as it stands: no repository is needed, and none was created");
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
    let scope = if all { "files, private layer included" } else { "public files" };
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
///
/// **Prints and decides nothing.** It took the memory and worked out the miss itself,
/// which put the recording on whichever branch happened to call it, and the hybrid
/// branch calls it only when the fused list is empty. So a refusal over passages it
/// went on to print recorded nothing, which is the same defect one level down from the
/// one `Memory::recall_loss` was extracted to fix. The caller asks the contract; this
/// puts words on a terminal.
fn print_suggestions(words: &[String]) {
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

fn cmd_route(
    question: &str,
    paths: &[&str],
    all: bool,
    top: usize,
    hybrid: bool,
    as_json: bool,
) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            // stdout is the contract in --json mode, the same rule `mcp.rs` follows,
            // and a caller that gets exit 1 with nothing on stdout has to guess what
            // went wrong from an exit code. The human line above stays: it costs
            // nothing and it is what shows up in a deployment's logs.
            if as_json {
                println!("{}", open_error_as_json("question", question, &e.to_string()).to_string());
            }
            return ExitCode::from(1);
        }
    };
    if memory.index_was_rebuilt {
        eprintln!("kb: an index predated the private column (ADR-0034) and was emptied. Run `kb index`.");
    }

    if as_json {
        return route_as_json(question, &memory, top);
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

    // **One `ask`, for the verdict, on both branches.** The ranked list below is still
    // the keyword scorer's, and this costs a second pass over the corpus to get the
    // number that decides whether the question was a recall loss. `mcp.rs` already pays
    // it for the same reason and states it: this is called once per question by a
    // person, never in a loop. Deriving the loss from the length of whichever list a
    // branch happened to hold is what gave one measurement four definitions.
    let answer = memory.ask(question, top);

    // **Asked unconditionally, because the branches below decide what to print and
    // that is a different question from whether the base failed to reach an answer.**
    // Hanging the record off a printing branch is exactly how this measurement came to
    // have four definitions, and doing it one level lower cost a run to catch.
    let loss = memory.recall_loss(question, &answer.confidence);
    let looked_like: &[String] = loss.as_ref().map_or(&[], |m| &m.looked_like);

    if !hybrid {
        let hits = memory.route(question, top);
        if hits.is_empty() {
            if !print_if_nothing_to_search(&memory) {
                println!("  nothing matched. Either the base does not cover it, or the");
                println!("  Search for lines do not carry the words a real question uses.");
                print_suggestions(looked_like);
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

    // `Memory::retrieve` is `Memory::ask` minus the confidence, over the same expansion
    // and the same two scorers, so the answer already in hand is the same list for no
    // extra work. The same substitution `mcp.rs` made, for the same reason.
    let found = &answer.found;
    if found.is_empty() {
        if !print_if_nothing_to_search(&memory) {
            println!("  nothing matched, in either scorer. Either the base does not cover");
            println!("  it, or the Search for lines do not carry the words a real question");
            println!("  uses.");
            print_suggestions(looked_like);
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

    for f in found {
        let layer = match f.layer {
            kb::retrieve::Layer::Short => "  [short memory]",
            kb::retrieve::Layer::Long => "",
        };
        println!("  {:>5.3}  {:<8} {:<44} {}{layer}", f.score, f.base, f.path, f.why.join(" + "));
        if let Some(p) = f.passages.first() {
            println!("         {}: {}", p.heading_path, p.excerpt.replace("
", " ").trim());
        }
    }

    ExitCode::SUCCESS
}

/// `kb route --json`: the whole answer as one line on stdout.
///
/// **Not a serialisation of the terminal output, and the difference is the design.**
/// The terminal has two modes because a person reading a ranked list and a person
/// reading passages want different things. A program wants both plus the verdict, in
/// one call, and that is exactly what [`memory::Memory::ask`] computes in one pass:
/// the owner and the verdict from the keyword ranking, the reading from fusion, over
/// one expansion. So `--json` always fuses and `--hybrid` adds nothing on top of it.
///
/// Calling `ask` rather than assembling the pieces here is the rule `lib.rs` states:
/// every machine surface answers from the contract, so `kb serve`, `kb boot` and this
/// cannot drift into three different opinions about one question.
///
/// Five fields exist for callers that are not sitting at a terminal, and each one is
/// a failure that has already happened somewhere:
///
/// - `gate` says whether `results` passed the verdict, whether the text scorer was the
///   only thing that ranked them, and what floor the score was measured against. It
///   exists because the first integrator to parse this could not tell a refusal over
///   real candidates from a question the base does not cover: both arrive as
///   `verdict: "nothing"` with an array beside them, and the rule that settles it was
///   written only in prose a program does not read.
/// - `skipped` names bases left out. Empty since ADR-0034, because the one reason a
///   base was ever left out, git not answering for its privacy, is gone; it stays so a
///   caller reading stdout alone has one field to check before reading `results: []`
///   as "the base does not cover this", which is the exact mistake a deployment used
///   to make when a bundle without `.git` served nothing.
/// - `suggestions` carries what the miss path prints, so a caller can offer the words
///   the base does know instead of a dead end.
/// - `miss` is the recall loss itself, whole, for a caller with nowhere to write. The
///   log lives beside the fleet, which is right on a machine somebody owns and
///   impossible on a hosted one, and the failure used to reach the caller as a line on
///   a child process's stderr. `recorded` says whether the file holds this too.
/// - `agent.margin` is `null` when only one agent scored. That is JSON's encoding of
///   infinity, and it means maximum confidence, not missing data.
///
/// **Returns the value instead of printing it, and the split is not cosmetic.** While
/// this printed, the contract an integrator parses had nothing under it: the first
/// report of its shape came back from somebody's production deployment, and two of the
/// findings in `reports/2026-08-29-first-integration.md` are about fields that say the
/// wrong thing to a caller who is not sitting at a terminal. A payload a test can hold
/// is the precondition for changing any of them on purpose.
fn route_payload(question: &str, memory: &memory::Memory, top: usize) -> json::Value {
    let answer = memory.ask(question, top);
    let refused = answer.confidence.verdict == memory::Verdict::Nothing;

    // **On the verdict, not on the list length, and that was the defect.** The two
    // agree only while the scorers do. A question the text scorer ranked and the
    // keyword scorer did not has a full `results` array and a refusal, and this
    // branch used to read the array: the refusal the caller had to help somebody
    // recover from was the one case that got no vocabulary back, and the recall loss
    // that rides with it was the one loss the log never counted. F-06 and F-02 in
    // `reports/2026-08-29-first-integration.md`. The decision itself is on the
    // contract now, so this surface no longer holds an opinion about it.
    let loss = memory.recall_loss(question, &answer.confidence);
    let suggestions: Vec<String> = loss.as_ref().map(|m| m.looked_like.clone()).unwrap_or_default();

    let mut out = json::Value::obj();
    out.set("question", question.into());
    out.set("verdict", answer.confidence.verdict.label().into());

    // **What the verdict alone could not say.** `verdict` answers "did the keyword
    // scorer rank anything" and `results` answers "did either of them", so a caller
    // holding both could not tell a refusal over real candidates from a subject the
    // base does not cover. Both arrive as `nothing` with a `results` array, and they
    // call for opposite work: the first is a keys problem in a base that may hold the
    // answer, the second is a coverage problem.
    //
    // Three facts rather than one enum, because each is separately checkable and none
    // of them needs versioning the day a fourth state appears:
    //
    // - `served` is our own rule, stated rather than left to be re-derived from a
    //   string. It was written only in the prose of `--help`, and an integrator who
    //   read `results.length > 0` instead served passages we had refused.
    // - `ranked_by_text_only` is the mechanism and not a diagnosis. It says which
    //   scorer found the file, which is a fact. Whether the base really covers the
    //   subject is not something a word match knows.
    // - `floor` is what `keyword_score` was measured against, so the gate can be
    //   disagreed with without guessing its threshold.
    let mut gate = json::Value::obj();
    gate.set("served", (!refused).into());
    gate.set("ranked_by_text_only", (refused && !answer.found.is_empty()).into());
    gate.set("floor", score(memory::SCORE_FLOOR as f64));
    out.set("gate", gate);

    let mut confidence = json::Value::obj();
    confidence.set("agreement", answer.confidence.agreement.into());
    confidence.set("keyword_score", score(answer.confidence.keyword_score as f64));
    confidence.set("margin", score(answer.confidence.margin as f64));
    out.set("confidence", confidence);

    out.set(
        "agent",
        match &answer.agent {
            Some(a) => {
                let mut agent = json::Value::obj();
                agent.set("name", a.agent.as_str().into());
                agent.set("score", score(a.score));
                agent.set("files", a.files.into());
                agent.set("margin", score(a.margin));
                agent.set("contenders", a.contenders.into());
                agent.set(
                    "totals",
                    json::Value::Arr(
                        a.totals
                            .iter()
                            .map(|(name, total)| {
                                let mut t = json::Value::obj();
                                t.set("agent", name.as_str().into());
                                t.set("score", score(*total));
                                t
                            })
                            .collect(),
                    ),
                );
                agent
            }
            None => json::Value::Null,
        },
    );

    out.set(
        "keyword_top",
        match &answer.keyword_top {
            Some(t) => t.as_str().into(),
            None => json::Value::Null,
        },
    );

    let mut indexed = json::Value::obj();
    indexed.set("entries", memory.entry_count().into());
    indexed.set("agents", memory.agents.len().into());
    indexed.set("aliases", memory.alias_count().into());
    out.set("indexed", indexed);

    out.set(
        "skipped",
        json::Value::Arr(
            memory
                .skipped
                .iter()
                .map(|p| p.display().to_string().into())
                .collect(),
        ),
    );
    out.set("index_was_rebuilt", memory.index_was_rebuilt.into());
    out.set(
        "suggestions",
        json::Value::Arr(suggestions.into_iter().map(Into::into).collect()),
    );

    // **Self contained on purpose.** `question` and `looked_like` repeat what is
    // already at the top level, because this object exists to be copied whole into
    // somebody else's store by a caller that has nowhere of its own to write, and
    // making every such caller reassemble it from four fields is how two of them end
    // up keeping different records. `recorded` and `error` are the half that could
    // not be said at all before: a failed write reached the caller as a line on the
    // stderr of a child process, which in a function's logs is not reaching anybody.
    out.set(
        "miss",
        match &loss {
            Some(m) => {
                let mut miss = json::Value::obj();
                miss.set("question", m.question.as_str().into());
                miss.set(
                    "looked_like",
                    json::Value::Arr(
                        m.looked_like.iter().map(|w| w.as_str().into()).collect(),
                    ),
                );
                miss.set("date", m.date.as_str().into());
                miss.set("log", m.log.display().to_string().into());
                miss.set("recorded", m.recorded().into());
                miss.set(
                    "error",
                    match &m.error {
                        Some(e) => e.as_str().into(),
                        None => json::Value::Null,
                    },
                );
                miss
            }
            None => json::Value::Null,
        },
    );
    out.set(
        "results",
        json::Value::Arr(answer.found.iter().map(retrieved_as_json).collect()),
    );

    out
}

/// The one line of JSON a program gets when the base could not be opened.
///
/// **Shared, because the two commands disagreed.** `route` printed this object and
/// `remember` printed nothing at all on stdout, so a caller of one got a failure it
/// could parse and a caller of the other got exit 1 and silence, with the sentence
/// only on stderr. A machine surface that fails unreadably is a machine surface that
/// fails silently, which is the shape `skipped` and `error` exist to prevent.
///
/// The input field is named after the input, `question` or `claim`, because a caller
/// correlating a failure with what it sent needs the thing it sent.
fn open_error_as_json(input_field: &str, input: &str, error: &str) -> json::Value {
    let mut out = json::Value::obj();
    out.set(input_field, input.into());
    out.set("error", error.into());
    out
}

/// One line on stdout, and nothing else in here.
///
/// Everything worth testing moved into [`route_payload`]. What is left is the part a
/// test would have to capture stdout to see, which is the part with nothing in it.
fn route_as_json(question: &str, memory: &memory::Memory, top: usize) -> ExitCode {
    println!("{}", route_payload(question, memory, top).to_string());
    ExitCode::SUCCESS
}

/// Every score in the JSON goes through here, rounded to six decimals.
///
/// **Not cosmetic.** The keyword score is an `f32` and widening it to `f64` prints
/// seventeen digits, so 11.23 leaves as 11.229999542236328: precision the number never
/// had, in an output a program reads and a person debugs. Six decimals is chosen
/// against the thing that needs the resolution, which is fusion, not the floor: RRF
/// scores are sums of `1 / (60 + rank)`, and adjacent ranks differ by around 1e-4, so
/// 1e-6 keeps two files that really are ordered from collapsing into a tie.
///
/// Infinity survives it and still encodes as `null`, which is what `agent.margin` needs.
fn score(n: f64) -> json::Value {
    json::Value::Num((n * 1e6).round() / 1e6)
}

/// One ranked file and the passages that matched inside it.
///
/// `score` is the fused score and `keyword_score` is the raw keyword sum, and both
/// travel because they answer different questions: fusion says which file to read
/// first, and the keyword score is the number the verdict is measured against. A
/// caller handed only one of them cannot check the other.
fn retrieved_as_json(f: &kb::retrieve::Retrieved) -> json::Value {
    let mut out = json::Value::obj();
    out.set("base", f.base.as_str().into());
    out.set("path", f.path.as_str().into());
    // Short or long memory, so a caller building a prompt can carry the label a
    // model needs, and a caller that wants only settled knowledge can filter on it.
    out.set("memory", f.layer.label().into());
    out.set("title", f.title.as_str().into());
    out.set("purpose", f.purpose.as_str().into());
    out.set("score", score(f.score));
    out.set("keyword_score", score(f.keyword_score as f64));
    out.set(
        "why",
        json::Value::Arr(f.why.iter().map(|w| w.as_str().into()).collect()),
    );
    out.set(
        "matched",
        json::Value::Arr(f.matched.iter().map(|m| m.as_str().into()).collect()),
    );
    out.set(
        "passages",
        json::Value::Arr(
            f.passages
                .iter()
                .map(|p| {
                    let mut v = json::Value::obj();
                    v.set("heading_path", p.heading_path.as_str().into());
                    v.set("text", p.text.as_str().into());
                    v.set("excerpt", p.excerpt.as_str().into());
                    v.set(
                        "provenance",
                        match &p.provenance {
                            Some(s) => s.as_str().into(),
                            None => json::Value::Null,
                        },
                    );
                    v.set(
                        "stage",
                        match &p.stage {
                            Some(s) => s.as_str().into(),
                            None => json::Value::Null,
                        },
                    );
                    v
                })
                .collect(),
        ),
    );
    out
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

    // **Unconditional, and it used to be absent.** This path suggested and moved on, so
    // a question that got all the way to the answerer and was refused left no trace in
    // the recall loss log. Outside the branch for the reason `cmd_route` records above:
    // a branch that decides what to print is not the thing that knows what was lost.
    let loss = memory.recall_loss(question, &a.confidence);
    let looked_like: &[String] = loss.as_ref().map_or(&[], |m| &m.looked_like);

    if !answer::worth_asking(&a.confidence, &a.found) {
        // The same refusal `kb route` gives, with the same suggestions: absence is an
        // answer, and it costs zero model calls.
        println!("nothing in the library matched. Either it does not cover this, or the");
        println!("Search for lines do not carry the words the question used.");
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
// capture
// ---------------------------------------------------------------------------

/// `kb capture`: the session's record into the deposit. ADR-0035.
///
/// Prints one sentence about what it did, because it runs from a hook nobody watches
/// and a feature that fails silently there is a feature that is off within a week.
fn cmd_capture(root: &str, session: Option<&str>) -> ExitCode {
    use std::io::Read;

    // The flag wins. Without it, the hook payload on stdin names the session, exactly
    // as `kb boot` reads it, so the same hook can call both without plumbing.
    let session = match session.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => {
            let mut stdin = String::new();
            let _ = std::io::stdin().read_to_string(&mut stdin);
            match boot::parse_request(&stdin) {
                Some(req) if req.session != "unknown" => req.session,
                _ => {
                    eprintln!("kb capture: no session named. Pass --session, or the hook payload on stdin.");
                    return ExitCode::from(2);
                }
            }
        }
    };

    match capture::write_deposit(Path::new(root), &session, &kb::misses::today()) {
        Ok(capture::Outcome::Written(path)) => {
            println!("captured session {session} into {}", path.display());
            ExitCode::SUCCESS
        }
        Ok(capture::Outcome::Nothing(why)) => {
            println!("nothing captured for session {session}: {why}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb capture: {e}");
            ExitCode::from(1)
        }
    }
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
        eprintln!("kb: an index predated the private column (ADR-0034) and was emptied. Run `kb index`.");
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
        "       keyword {}/{}  ({:.0}%), the deterministic fold alone, which is the fallback when no classifier answers",
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
            "       ({} question(s) excluded: answered only from an attached base, which can be read but cannot be the agent who answers)",
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
        // **Two columns, because one number was read two ways.** "Abstained on 4/4"
        // folded a `guess` in with silence, and a `guess` is served with a warning by
        // every surface here. Refused and hedged are different safety properties, and
        // the third column is the one that is simply wrong, so the three close.
        println!(
            "       of {} question(s) the set says to decline: refused {}, hedged {}, answered {}",
            s.abstention_expected, s.abstention_refused, s.abstention_hedged, s.abstention_answered
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

/// `kb remember --json`: the proposal as one line, for a caller that is not a person.
///
/// **The judgement was already here and unreachable from code.** `--json` is parsed
/// once for the whole process and this command dropped it, so the flag was accepted,
/// ignored, and answered with terminal prose. The first integrator to want exactly
/// this piece, the one that decides whether a fact is worth storing, measured it by
/// hand and could not wire it up. F-04 in `reports/2026-08-29-first-integration.md`.
///
/// **Nothing here decides anything**, which is what makes it useful to a hosted agent:
/// `remember::assess` measures overlap and proposes, it writes nothing, and it needs
/// no model. So a consumer with a read only filesystem can ask the question at the
/// moment the fact appears, keep the proposal, and apply it later with `kb write` on a
/// machine that has the repository. The caveat travels as `notice` for the same reason
/// `kb answer` rides its warning into its output: a model reading this through another
/// surface is told what a person reading the terminal is told.
///
/// Takes the assessment rather than the memory, so the three outcomes can be pinned
/// without arguing with the classifier about which one a fixture produces. What
/// `assess` decides is its own business and is tested beside it.
fn remember_payload(claim: &str, assessment: &remember::Assessment) -> json::Value {
    let mut out = json::Value::obj();
    out.set("claim", claim.into());
    out.set("proposal", assessment.outcome.label().into());
    out.set("reason", assessment.reason.as_str().into());
    out.set(
        "evidence",
        json::Value::Arr(
            assessment
                .evidence
                .iter()
                .map(|e| {
                    let mut v = json::Value::obj();
                    v.set("base", e.base.as_str().into());
                    v.set("path", e.path.as_str().into());
                    v.set("heading_path", e.heading_path.as_str().into());
                    v.set("excerpt", e.excerpt.as_str().into());
                    // The same rounding as every other number here: 4/7 widened to an
                    // f64 prints seventeen digits of precision a ratio over a handful
                    // of words never had.
                    v.set("containment", score(e.containment));
                    v.set(
                        "shared",
                        json::Value::Arr(e.shared.iter().map(|w| w.as_str().into()).collect()),
                    );
                    v.set(
                        "missing",
                        json::Value::Arr(e.missing.iter().map(|w| w.as_str().into()).collect()),
                    );
                    v
                })
                .collect(),
        ),
    );
    out.set("notice", remember::DISCLAIMER.into());
    out
}

fn cmd_remember(claim: &str, paths: &[&str], all: bool, as_json: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            if as_json {
                println!("{}", open_error_as_json("claim", claim, &e.to_string()).to_string());
            }
            return ExitCode::from(1);
        }
    };
    if memory.index_was_rebuilt {
        eprintln!("kb: an index predated the private column (ADR-0034) and was emptied. Run `kb index`.");
    }

    let a = memory.remember(claim);

    if as_json {
        println!("{}", remember_payload(claim, &a).to_string());
        return ExitCode::SUCCESS;
    }

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

    /// The trap in the JSON contract, written down as a test because a reader of the
    /// output cannot see it: `AgentChoice::margin` is infinite when only one agent
    /// scored, JSON has no infinity, and `null` is the only legal encoding. It means
    /// maximum confidence and it looks exactly like a missing field.
    #[test]
    fn an_infinite_margin_encodes_as_null_rather_than_as_a_broken_number() {
        let mut v = json::Value::obj();
        v.set("margin", f64::INFINITY.into());
        assert_eq!(v.to_string(), "{\"margin\":null}");
    }

    /// Both scores travel, and they are different numbers: `score` is fused and orders
    /// the reading list, `keyword_score` is the raw sum the verdict is measured
    /// against. A caller given one of them cannot check the other.
    #[test]
    fn a_result_carries_the_fused_score_and_the_keyword_score_separately() {
        let f = kb::retrieve::Retrieved {
            base: "zed".into(),
            path: "knowledge/deploy.md".into(),
            layer: kb::retrieve::Layer::Long,
            title: "Deploys".into(),
            purpose: "what a deploy needs".into(),
            score: 0.032,
            keyword_score: 11.23,
            why: vec!["keywords #1".into(), "text #1".into()],
            matched: vec!["rollback".into()],
            passages: vec![kb::retrieve::Passage {
                heading_path: "Deploys > Rollback".into(),
                text: "write the rollback down first".into(),
                excerpt: " ... rollback ... ".into(),
                provenance: Some("human".into()),
                stage: None,
            }],
        };

        let out = retrieved_as_json(&f).to_string();
        assert!(out.contains("\"memory\":\"long\""), "{out}");
        assert!(out.contains("\"score\":0.032"), "{out}");
        assert!(out.contains("\"keyword_score\":11.23"), "{out}");
        assert!(out.contains("\"heading_path\":\"Deploys > Rollback\""), "{out}");
        assert!(out.contains("\"provenance\":\"human\""), "{out}");
        // Absent, not omitted. A caller reading the key gets null and knows the note
        // declares no stage, which is a different fact from a key that is not there.
        assert!(out.contains("\"stage\":null"), "{out}");
    }

    // -----------------------------------------------------------------------
    // The route payload, as a value rather than as a line of stdout
    //
    // Everything below exists because `route_as_json` printed. A function that
    // prints can be read by a person and by nothing else, so the JSON contract an
    // integrator parses had no test under it at all: the first report of its shape
    // came from a deployment. See `reports/2026-08-29-first-integration.md`.
    // -----------------------------------------------------------------------

    /// One note on disk, in the shape the index actually reads.
    ///
    /// `title`, `keys` and `purpose` are separate arguments rather than one blob
    /// because the keyword scorer reads all three and the text scorer reads none of
    /// them. A helper that filled them from each other would make the two scorers
    /// agree by construction, which is precisely the condition these tests need to
    /// be able to break.
    fn note(title: &str, keys: &str, purpose: &str, body: &str) -> String {
        format!("# {title}\n\n**Search for:** {keys}\n\n**Exists to:** {purpose}\n\n## Body\n\n{body}\n")
    }

    /// A one agent fleet on disk, indexed the way `kb index` indexes it.
    ///
    /// Built rather than assembled in memory because the behaviour under test is a
    /// disagreement between two independent scorers, and a hand written `Answer`
    /// agrees with itself by construction.
    ///
    /// The sync is not optional. `Memory::open` reads an index and never builds one,
    /// so a fixture that skips it has a working keyword scorer and a text scorer that
    /// finds nothing, which is the exact asymmetry these tests exist to catch.
    /// Returns the fleet root beside the memory, because `record_miss` writes there and
    /// a test that recomputed the path would be asserting against its own copy of the
    /// formula rather than against the one the code uses.
    fn indexed_fleet(name: &str, notes: &[(&str, String)]) -> (std::path::PathBuf, memory::Memory) {
        let dir = std::env::temp_dir()
            .join("kb-route-payload-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let agent = dir.join("fleet").join("probe");
        std::fs::create_dir_all(agent.join("knowledge")).expect("mkdir");
        std::fs::write(agent.join("agent.txt"), "name = Probe\nrole = testing\n").expect("agent");
        std::fs::write(agent.join("MAP.md"), "# MAP\n").expect("map");
        for (stem, text) in notes {
            // A bare stem lands in `knowledge/`; a key with a slash is a path from the
            // agent root, which is how a test puts a file in the deposit.
            let path = if stem.contains('/') {
                agent.join(stem)
            } else {
                agent.join("knowledge").join(format!("{stem}.md"))
            };
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
            std::fs::write(path, text).expect("note");
        }

        let base = Base::discover(&agent, true).expect("discover");
        let mut db = store::Store::open(&memory::index_path(&agent)).expect("index");
        db.sync(&base, "probe").expect("sync");
        drop(db);

        let memory = memory::Memory::open(&[dir.as_path()], true).expect("opens");
        (dir, memory)
    }

    /// Four notes so the idf denominator is not degenerate: with two entries in the
    /// corpus a unique term is worth `ln(2)` and no realistic question clears
    /// `SCORE_FLOOR`, which would make a hit untestable for a reason that has nothing
    /// to do with what is being tested.
    fn one_note_the_keyword_scorer_can_reach() -> Vec<(&'static str, String)> {
        vec![
            (
                "rollback",
                note(
                    "Rollback",
                    "`rollback`, `deploy`, `downtime`, `release`",
                    "say how a deploy is undone",
                    "Keep the previous version serving while the new one takes traffic.",
                ),
            ),
            ("pasta", note("Pasta", "`pasta`, `tomato`", "hold one recipe", "Salt the water.")),
            ("zebra", note("Zebra", "`zebra`, `quagga`", "hold one animal", "Stripes differ.")),
            ("tide", note("Tide", "`tide`, `harbour`", "hold one port note", "It turns twice.")),
        ]
    }

    /// A note the text scorer can reach and the keyword scorer cannot: its keys, its
    /// title and its purpose are about an animal, and its body is about deploys.
    fn one_note_only_the_text_scorer_can_reach() -> Vec<(&'static str, String)> {
        vec![
            (
                "striped",
                note(
                    "Zebra",
                    "`zebra`, `quagga`",
                    "hold one striped animal",
                    "A rollback without downtime keeps the previous release serving.",
                ),
            ),
            ("pasta", note("Pasta", "`pasta`, `tomato`", "hold one recipe", "Salt the water.")),
            ("tide", note("Tide", "`tide`, `harbour`", "hold one port note", "It turns twice.")),
        ]
    }

    /// The same shape, with one key a question is likely to spell slightly wrong. The
    /// suggester measures spelling, so a base with no orthographic neighbour of any
    /// question word has nothing honest to offer and correctly offers nothing, which
    /// would make an empty `suggestions` prove the wrong thing.
    fn one_note_whose_key_is_a_near_miss() -> Vec<(&'static str, String)> {
        vec![
            (
                "striped",
                note(
                    "Striped animals",
                    "`zebras`, `quaggas`",
                    "hold one animal",
                    "A rollback without downtime keeps the previous release serving.",
                ),
            ),
            ("pasta", note("Pasta", "`pasta`, `tomato`", "hold one recipe", "Salt the water.")),
            ("tide", note("Tide", "`tide`, `harbour`", "hold one port note", "It turns twice.")),
        ]
    }

    fn text_of(v: &json::Value, key: &str) -> String {
        match v.get(key) {
            Some(json::Value::Str(s)) => s.clone(),
            other => panic!("{key} is not a string: {other:?}"),
        }
    }

    fn len_of(v: &json::Value, key: &str) -> usize {
        match v.get(key) {
            Some(json::Value::Arr(a)) => a.len(),
            other => panic!("{key} is not an array: {other:?}"),
        }
    }

    fn flag_of(v: &json::Value, path: [&str; 2]) -> bool {
        match v.get(path[0]).and_then(|o| o.get(path[1])) {
            Some(json::Value::Bool(b)) => *b,
            other => panic!("{}.{} is not a boolean: {other:?}", path[0], path[1]),
        }
    }

    fn num_of(v: &json::Value, path: [&str; 2]) -> f64 {
        match v.get(path[0]).and_then(|o| o.get(path[1])) {
            Some(json::Value::Num(n)) => *n,
            other => panic!("{}.{} is not a number: {other:?}", path[0], path[1]),
        }
    }

    /// The contract, listed by name. A field that leaves the payload breaks a caller
    /// silently, because a missing key and a key holding `null` read the same way in
    /// most languages, so the field list is pinned rather than described.
    #[test]
    fn the_whole_documented_contract_is_in_the_value_the_printer_is_handed() {
        let (_, memory) = indexed_fleet("shape", &one_note_the_keyword_scorer_can_reach());
        let out = route_payload("como faco rollback de um deploy sem downtime na release", &memory, 4);

        for key in [
            "question",
            "verdict",
            "gate",
            "confidence",
            "agent",
            "keyword_top",
            "indexed",
            "skipped",
            "index_was_rebuilt",
            "suggestions",
            "results",
        ] {
            assert!(out.get(key).is_some(), "{key} left the payload: {}", out.to_string());
        }

        assert_eq!(text_of(&out, "verdict"), "hit", "{}", out.to_string());
        assert!(len_of(&out, "results") > 0, "{}", out.to_string());
        assert!(flag_of(&out, ["gate", "served"]), "{}", out.to_string());
        assert!(!flag_of(&out, ["gate", "ranked_by_text_only"]), "{}", out.to_string());
        assert_eq!(
            len_of(&out, "suggestions"),
            0,
            "a hit offers no vocabulary, it offers the answer"
        );
    }

    /// F-01 in `reports/2026-08-29-first-integration.md`, from the other side. The
    /// verdict answers "did the keyword scorer rank anything" and `results` answers
    /// "did either scorer", and until this landed the payload said which question it
    /// had answered nowhere: a caller reading `results.length > 0` served passages we
    /// consider not found, and one trusting the verdict discarded passages we did find.
    ///
    /// The two refusals must be told apart, because they call for opposite work. One
    /// is a keys problem in a base that may well hold the answer; the other is a base
    /// that does not cover the subject.
    #[test]
    fn a_result_the_gate_refused_says_so_and_says_it_differently_from_an_uncovered_question() {
        let (_, memory) = indexed_fleet("gated", &one_note_only_the_text_scorer_can_reach());

        let gated = route_payload("rollback sem downtime", &memory, 4);
        let uncovered = route_payload("qual a taxa de juros do trimestre", &memory, 4);

        assert_eq!(text_of(&gated, "verdict"), "nothing", "{}", gated.to_string());
        assert!(
            len_of(&gated, "results") > 0,
            "the text scorer found the note: {}",
            gated.to_string()
        );
        assert!(!flag_of(&gated, ["gate", "served"]), "{}", gated.to_string());
        assert!(
            flag_of(&gated, ["gate", "ranked_by_text_only"]),
            "the state the verdict was hiding: {}",
            gated.to_string()
        );

        assert_eq!(text_of(&uncovered, "verdict"), "nothing", "{}", uncovered.to_string());
        assert_eq!(len_of(&uncovered, "results"), 0, "{}", uncovered.to_string());
        assert!(!flag_of(&uncovered, ["gate", "served"]), "{}", uncovered.to_string());
        assert!(
            !flag_of(&uncovered, ["gate", "ranked_by_text_only"]),
            "nothing ranked at all, so nothing ranked by text alone: {}",
            uncovered.to_string()
        );

        assert_ne!(
            gated.get("gate"),
            uncovered.get("gate"),
            "one field a caller can branch on, which is the whole finding"
        );
    }

    /// A `guess` is served and says it is weak. The gate reports the rule the type
    /// already states at `memory::Verdict::Guess`: dropping weak results loses real
    /// answers and saying "this is a guess" loses nothing, so a warning is not a
    /// filter. It is the subtle one, because `served` is false for exactly one of the
    /// three verdicts and a reader expects it to be false for two.
    #[test]
    fn a_guess_is_served_because_a_warning_is_not_a_filter() {
        let (_, memory) = indexed_fleet("guess", &one_note_the_keyword_scorer_can_reach());
        let out = route_payload("downtime", &memory, 4);

        assert_eq!(text_of(&out, "verdict"), "guess", "{}", out.to_string());
        assert!(flag_of(&out, ["gate", "served"]), "{}", out.to_string());
        assert!(!flag_of(&out, ["gate", "ranked_by_text_only"]), "{}", out.to_string());
    }

    /// The floor travels so a caller can disagree with the gate without guessing what
    /// it was measured against. Asserted through `memory::SCORE_FLOOR` rather than
    /// through the literal 17.5, because the constant has already moved twice and a
    /// literal here would pin the payload to a number the gate no longer uses.
    #[test]
    fn the_floor_in_the_payload_is_the_one_the_verdict_was_measured_against() {
        let (_, memory) = indexed_fleet("floor", &one_note_the_keyword_scorer_can_reach());

        for question in ["como faco rollback de um deploy sem downtime na release", "downtime", "xyzzy"] {
            let out = route_payload(question, &memory, 4);
            assert_eq!(
                num_of(&out, ["gate", "floor"]),
                memory::SCORE_FLOOR as f64,
                "{question}: {}",
                out.to_string()
            );

            let score = num_of(&out, ["confidence", "keyword_score"]);
            let floor = num_of(&out, ["gate", "floor"]);
            assert_eq!(
                text_of(&out, "verdict") == "hit",
                score >= floor,
                "the floor in the payload has to be the one that decided: {}",
                out.to_string()
            );
        }
    }

    /// F-06. `suggestions` was computed only when the fused list was empty, so the
    /// case it was built for, a refusal the person on screen has to recover from,
    /// was the case where it returned nothing.
    #[test]
    fn suggestions_arrive_whenever_the_gate_refuses_including_over_a_full_result_set() {
        let (_, memory) = indexed_fleet("suggest", &one_note_whose_key_is_a_near_miss());
        let out = route_payload("zebra sem downtime", &memory, 4);

        assert_eq!(text_of(&out, "verdict"), "nothing", "{}", out.to_string());
        assert!(len_of(&out, "results") > 0, "the text scorer found it: {}", out.to_string());
        assert!(
            len_of(&out, "suggestions") > 0,
            "the base knows `zebras` and was asked about `zebra`: {}",
            out.to_string()
        );
    }

    /// The honesty property, which the fix above must not buy its way past: a base
    /// with no orthographic neighbour of any question word offers nothing rather than
    /// the nearest thing it has. A suggester that always answers is one nobody can
    /// use, because trigram overlap measures spelling and never meaning.
    #[test]
    fn a_base_with_nothing_that_looks_like_the_question_still_suggests_nothing() {
        let (_, memory) = indexed_fleet("honest", &one_note_whose_key_is_a_near_miss());
        let out = route_payload("qual a taxa de juros do trimestre", &memory, 4);

        assert_eq!(text_of(&out, "verdict"), "nothing");
        assert_eq!(len_of(&out, "suggestions"), 0, "{}", out.to_string());
    }

    // -----------------------------------------------------------------------
    // `kb remember --json`, F-04
    //
    // The judgement underneath is `remember::assess` and it is tested there. What
    // is tested here is that all of it reaches a program: the flag was parsed
    // process wide and silently dropped on this command, so the one piece of the
    // write side a hosted agent could have called was unreachable from code.
    // -----------------------------------------------------------------------

    fn one_piece_of_evidence() -> remember::Evidence {
        remember::Evidence {
            base: "zed".into(),
            path: "knowledge/metrics.md".into(),
            heading_path: "Metrics > ROAS".into(),
            excerpt: "the roas here is net of fees".into(),
            containment: 4.0 / 7.0,
            shared: vec!["roas".into(), "liquido".into()],
            missing: vec!["reembolso".into()],
        }
    }

    #[test]
    fn a_proposal_serialises_every_field_the_prose_prints() {
        let assessment = remember::Assessment {
            outcome: remember::Outcome::Update,
            reason: "4 of 7 words already appear in one passage".into(),
            evidence: vec![one_piece_of_evidence()],
        };

        let out = remember_payload("o roas liquido desconta taxa", &assessment);

        assert_eq!(text_of(&out, "claim"), "o roas liquido desconta taxa");
        assert_eq!(text_of(&out, "proposal"), "UPDATE");
        assert_eq!(text_of(&out, "reason"), "4 of 7 words already appear in one passage");
        assert!(
            text_of(&out, "notice").contains("DELETE"),
            "the caveat rides the output, so a model reading this is told what a person is told"
        );

        let evidence = match out.get("evidence") {
            Some(json::Value::Arr(a)) => a.clone(),
            other => panic!("evidence is not an array: {other:?}"),
        };
        assert_eq!(evidence.len(), 1);
        let e = &evidence[0];
        assert_eq!(e.get("base"), Some(&json::Value::Str("zed".into())));
        assert_eq!(e.get("path"), Some(&json::Value::Str("knowledge/metrics.md".into())));
        assert_eq!(e.get("heading_path"), Some(&json::Value::Str("Metrics > ROAS".into())));
        assert_eq!(e.get("excerpt"), Some(&json::Value::Str("the roas here is net of fees".into())));
        assert_eq!(
            e.get("shared"),
            Some(&json::Value::Arr(vec!["roas".into(), "liquido".into()]))
        );
        assert_eq!(e.get("missing"), Some(&json::Value::Arr(vec!["reembolso".into()])));
    }

    /// Through the same rounding every other number in this binary goes through.
    /// `4/7` as an `f64` prints seventeen digits, which is precision a containment
    /// ratio over a handful of words never had.
    #[test]
    fn containment_is_rounded_the_way_every_other_number_is() {
        let assessment = remember::Assessment {
            outcome: remember::Outcome::Update,
            reason: String::new(),
            evidence: vec![one_piece_of_evidence()],
        };
        let out = remember_payload("c", &assessment).to_string();
        assert!(out.contains("\"containment\":0.571429"), "{out}");
    }

    /// The wire names, pinned. A caller branches on these and they are separate from
    /// whatever the terminal happens to print, for the same reason `Verdict::label`
    /// is separate from the sentences the terminal writes.
    #[test]
    fn the_three_outcomes_travel_by_their_wire_names() {
        for (outcome, name) in [
            (remember::Outcome::Add, "ADD"),
            (remember::Outcome::Update, "UPDATE"),
            (remember::Outcome::Noop, "NOOP"),
        ] {
            let a = remember::Assessment { outcome, reason: String::new(), evidence: vec![] };
            assert_eq!(text_of(&remember_payload("c", &a), "proposal"), name);
        }
    }

    /// Empty and present, never absent. A caller reading the key gets a list with
    /// nothing in it and knows the base holds nothing close; a missing key reads as a
    /// parse problem.
    #[test]
    fn no_overlap_is_an_empty_array_and_not_a_missing_key() {
        let a = remember::Assessment {
            outcome: remember::Outcome::Add,
            reason: "nothing in the base overlaps this".into(),
            evidence: vec![],
        };
        let out = remember_payload("o yago treina as tercas", &a);
        assert_eq!(out.get("evidence"), Some(&json::Value::Arr(vec![])), "{}", out.to_string());
    }

    /// The wiring, over a real base: the judgement reaches the payload rather than
    /// the payload being right about an assessment nobody produced. What the
    /// classifier decides is `remember.rs`'s business and is tested there, so this
    /// asserts only that a real overlap arrives with real evidence behind it.
    #[test]
    fn a_claim_the_base_already_holds_comes_back_with_the_passage_it_overlaps() {
        let (_, memory) = indexed_fleet("remember", &one_note_the_keyword_scorer_can_reach());
        let out = remember_payload(
            "keep the previous version serving while the new one takes traffic",
            &memory.remember("keep the previous version serving while the new one takes traffic"),
        );

        assert_eq!(text_of(&out, "proposal"), "NOOP", "{}", out.to_string());
        match out.get("evidence") {
            Some(json::Value::Arr(a)) => assert!(!a.is_empty(), "{}", out.to_string()),
            other => panic!("evidence is not an array: {other:?}"),
        }
    }

    /// **One error shape for both commands.** `route` printed a parseable object on
    /// stdout and `remember` printed nothing at all, so a program calling one got a
    /// failure it could read and a program calling the other got an exit code and
    /// silence. The input field is named after the input, because a caller correlating
    /// a failure with what it sent needs the thing it sent.
    #[test]
    fn both_commands_fail_in_the_same_readable_shape() {
        let route = open_error_as_json("question", "como faco rollback", "cannot open the index");
        assert_eq!(route.get("question"), Some(&json::Value::Str("como faco rollback".into())));
        assert_eq!(route.get("error"), Some(&json::Value::Str("cannot open the index".into())));

        let remember = open_error_as_json("claim", "o roas aqui e liquido", "cannot read the base");
        assert_eq!(remember.get("claim"), Some(&json::Value::Str("o roas aqui e liquido".into())));
        assert_eq!(remember.get("error"), Some(&json::Value::Str("cannot read the base".into())));
    }

    /// The label reaches the payload, per result, so a program building a prompt can
    /// carry it and a program that wants only settled knowledge can filter on it. Two
    /// notes, one in the deposit and one in the library, both reachable by the text
    /// scorer, and the field tells them apart.
    #[test]
    fn each_result_says_which_memory_it_came_from() {
        let (_, memory) = indexed_fleet(
            "layers",
            &[
                ("settled", note("Settled", "`quagga`", "hold one animal", "the quagga is extinct")),
                (
                    "inbox/dropped.md",
                    "# Dropped\n\nthe quagga population doubled last spring\n".to_string(),
                ),
            ],
        );
        let out = route_payload("quagga population", &memory, 4);
        let results = match out.get("results") {
            Some(json::Value::Arr(a)) => a.clone(),
            other => panic!("results is not an array: {other:?}"),
        };
        let memory_of = |path: &str| {
            results
                .iter()
                .find(|r| r.get("path") == Some(&json::Value::Str(path.into())))
                .and_then(|r| r.get("memory").cloned())
        };
        assert_eq!(memory_of("inbox/dropped.md"), Some(json::Value::Str("short".into())), "{}", out.to_string());
        assert_eq!(memory_of("knowledge/settled.md"), Some(json::Value::Str("long".into())), "{}", out.to_string());
    }

    /// F-03. The payload carries the loss itself, so a caller with nowhere to write
    /// can persist it where its own stack already writes. Self contained on purpose:
    /// `question` and `looked_like` repeat what is already at the top level, because
    /// this object is designed to be copied whole into somebody else's store rather
    /// than reassembled from four fields by every caller that tries.
    #[test]
    fn a_refusal_hands_the_caller_the_loss_it_can_persist_itself() {
        let (root, memory) = indexed_fleet("payload-miss", &one_note_whose_key_is_a_near_miss());
        let out = route_payload("zebra sem downtime", &memory, 4);

        let miss = out.get("miss").expect("the field exists");
        assert_eq!(
            miss.get("question"),
            Some(&json::Value::Str("zebra sem downtime".into())),
            "{}",
            out.to_string()
        );
        assert_eq!(miss.get("recorded"), Some(&json::Value::Bool(true)), "{}", out.to_string());
        assert_eq!(miss.get("error"), Some(&json::Value::Null), "{}", out.to_string());
        assert_eq!(
            miss.get("log"),
            Some(&json::Value::Str(root.join(kb::misses::MISSES_TXT).display().to_string())),
            "{}",
            out.to_string()
        );
        match miss.get("looked_like") {
            Some(json::Value::Arr(a)) => assert!(!a.is_empty(), "{}", out.to_string()),
            other => panic!("looked_like is not an array: {other:?}"),
        }
        assert!(miss.get("date").is_some(), "{}", out.to_string());
    }

    /// Null rather than absent, and never an empty object. A caller branching on the
    /// key has to be able to tell "no loss" from "a loss with nothing in it", and a
    /// missing key reads as neither in most languages.
    #[test]
    fn an_answer_that_was_served_carries_no_miss() {
        let (_, memory) = indexed_fleet("payload-hit", &one_note_the_keyword_scorer_can_reach());
        let out = route_payload("como faco rollback de um deploy sem downtime na release", &memory, 4);

        assert_eq!(text_of(&out, "verdict"), "hit");
        assert_eq!(out.get("miss"), Some(&json::Value::Null), "{}", out.to_string());
    }

    /// **Found by running it, not by the tests above.** Moving the recording inside
    /// `print_suggestions` left it riding on a branch that asks a different question:
    /// the hybrid terminal path prints the miss message only when the fused list is
    /// empty, so a refusal over passages it went on to print recorded nothing. The
    /// decision has to be asked unconditionally and the branches left to choose only
    /// what they print, which is the same shape `mcp.rs` needed.
    #[test]
    fn the_hybrid_terminal_path_records_a_refusal_it_still_prints_passages_for() {
        let (root, _) = indexed_fleet("hybrid", &one_note_only_the_text_scorer_can_reach());
        let log = root.join(kb::misses::MISSES_TXT);
        let path = root.to_str().expect("a utf-8 scratch path");

        cmd_route("rollback sem downtime", &[path], true, 4, true, false);

        let written = std::fs::read_to_string(&log).expect("the refusal was recorded");
        assert!(written.contains("rollback sem downtime"), "{written}");
    }

    /// And the plain terminal path, which prints no passages at all, records the same
    /// question. One question, one line, counted twice: the log counts distinct
    /// questions, so two surfaces asking the same thing must not become two entries.
    #[test]
    fn the_plain_terminal_path_records_the_same_question_the_hybrid_one_does() {
        let (root, _) = indexed_fleet("terminal", &one_note_only_the_text_scorer_can_reach());
        let log = root.join(kb::misses::MISSES_TXT);
        let path = root.to_str().expect("a utf-8 scratch path");

        cmd_route("rollback sem downtime", &[path], true, 4, false, false);
        cmd_route("rollback sem downtime", &[path], true, 4, true, false);

        let written = std::fs::read_to_string(&log).expect("the refusal was recorded");
        assert!(written.contains("2    "), "one question counted twice: {written}");
    }

    /// **`kb answer` refused and recorded nothing, which was a fourth definition.** It
    /// printed the same apology `kb route` prints and offered the same vocabulary, and
    /// then dropped the loss on the floor: `suggest` without `record_miss`. Found while
    /// unifying the other three, and it is the surface where the omission costs most,
    /// because a question that reaches the answerer is a question somebody actually
    /// wanted answered.
    #[test]
    fn the_answerer_records_the_refusal_it_used_to_only_apologise_for() {
        let (root, _) = indexed_fleet("answered", &one_note_only_the_text_scorer_can_reach());
        let log = root.join(kb::misses::MISSES_TXT);
        let path = root.to_str().expect("a utf-8 scratch path");

        // Verdict `nothing`, so this returns before it can reach for a model.
        cmd_answer("rollback sem downtime", &[path], true, 4, answer::Mode::Fast);

        let written = std::fs::read_to_string(&log).expect("the refusal was recorded");
        assert!(written.contains("rollback sem downtime"), "{written}");
    }

    /// The recall loss log travels with the suggestion, so moving one moved the other.
    /// This is the first half of F-02: the question that reached nothing while the
    /// text scorer held the file is exactly the loss the log exists to count, and it
    /// was the one case never recorded. The other half, making every surface agree on
    /// the definition, is step 3 of the report.
    #[test]
    fn a_refusal_over_a_full_result_set_is_recorded_as_a_recall_loss() {
        let (root, memory) = indexed_fleet("miss", &one_note_only_the_text_scorer_can_reach());
        let log = root.join(kb::misses::MISSES_TXT);
        assert!(!log.exists(), "nothing has missed yet");

        let out = route_payload("rollback sem downtime", &memory, 4);
        assert_eq!(text_of(&out, "verdict"), "nothing");
        assert!(len_of(&out, "results") > 0);

        let written = std::fs::read_to_string(&log).expect("the miss log was written");
        assert!(written.contains("rollback sem downtime"), "{written}");
    }
}
