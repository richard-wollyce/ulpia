//! kb, a linter and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any index is derived
//! from them. This is that derived thing: it stores nothing, reads the files on every run, and either
//! reports what is broken (`check`) or answers which files a question should open (`route`).


use kb::checks::{Finding, Level};
use kb::{
    abstain, answer, base, blocks, boot, capture, checks, classify, commit, eval, gate, index,
    init, ingest, json,
    list,
    mcp, memory, misroute, misses, panel, promote, remember, sources, store, ui, write,
};
use base::Base;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
kb, a linter and router for file based knowledge bases

usage:
    kb --version
    kb check [path]... [--strict] [--all]
    kb index [path]... [--json] [--all]
    kb list [path]... [--base B] [--folder F] [--kind K] [--stage S] [--provenance P] [--json] [--all]
    kb route <question> [path]... [--top N] [--hybrid] [--json] [--all]
    kb answer <question> [path]... [--top N] [--all] [--expanded | --complete]
    kb remember <claim> [path]... [--all] [--json]
    kb init <name> [fleet-root]
    kb init --person [fleet-root]
    kb write <agent> <slug> [fleet-root] --keys <a, b> --summary <one line> [--folder F]
                [--captured-from PATH]
    kb source add [path] --type T --title X --retrieved-on DATE --retrieval-status S
                  [--author A]... [--year Y] [--container C] [--volume V] [--pages P]
                  [--publisher P] [--url U] [--doi D] [--isbn I] [--lang L]
                  [--replaces KEY] [--note N]
    kb sources [path]... [--json] [--all]
    kb fleet [path]... [--all]
    kb blocks [path] [--emit]
    kb eval <gold.tsv> [path]... [--top N] [--all] [--classify]
    kb commit <path>... -m <message>
    kb boot [path]... [--top N] [--all] [--session ID] [--cwd DIR] [--text]
    kb ingest <file> [path]... [--agent NAME] [--keep] [--dry-run] [--top N] [--all]
    kb promote [path]... [--top N] [--all] [--dry-run] [--max N] [--lock]
    kb ui [path]... [--port N] [--all]
    kb capture [path] [--session ID]
    kb serve [path]... [--top N] [--all]
    kb misses [path]... [--all] [--top N] [--json] [--apply --gold <tsv>]
    kb misroute <message> --chose <agent> --owner <agent|none> [--why <text>] [path]
    kb misroutes [path] [--top N]
    kb abstentions [path] [--top N] [--all]
    kb panel <artifact> [path] --owner <agent> [--reviewer <agent>]... [--out D] [--json]
    kb panel <artifact> [path] --from <agent> (--objection <text> [--blocking]
                                              | --nothing | --not-returned) [--why <text>]
    kb panel <artifact> [path] --resolve <n> (--taken | --refused | --escalated) [--why <text>]
    kb panel [<artifact>] [path] --ledger [--json]

    path        base to work on, defaults to the current directory
    --emit      blocks: print the assembled resident constitution instead of the report
    --strict    check: count warnings toward the exit code
    --all       include the private layer: the folders a base declares with a
                `private =` line in agent.txt, or profile/, projects/ and records/
                when it declares nothing. `.` declares the whole base, and the
                person's base is whole by name. Nothing else is consulted, and no
                repository is needed (ADR-0034)
    --top N     how many candidates to carry, default 5. route, boot, eval,
                promote, serve and misses all read it
    --keys      write: the words a real question would use. Required, no way to skip
    --summary   write: one line saying what the note is about, for its `Exists to:`
                header and for the map entry
    --folder    write: where under the agent it lands, default knowledge.
                list: a directory and everything under it
    --provenance write: human, agent or external. Default agent.
                list: narrow to one of them
    --stage     write: raw, distilled or derived. Default derived.
                list: narrow to raw, captured, distilled or derived
    --agent     ingest: the base to file the document in. Without it the router
                chooses, and refuses rather than guessing when no base owns the
                subject: a document filed in the wrong base is worse than one not
                filed, because the note is then reachable and wrong
    --keep      ingest: leave the document on disk after it has been absorbed.
                Without it the staged copy and its extraction are destroyed once
                a note on disk names them, which is the point of the verb
    --captured-from write: the deposit this note was distilled from. Optional, and
                written into the note's front matter rather than its prose, so it
                travels as a column on the index and reaches every passage without
                competing with them. `kb promote` always sets it. It is deliberately
                not called `source`: that word makes `kb check` demand an
                evidence_tier and a valid_for beside it, and those are gradings, which
                a writer must not be able to award itself
    --base      list: one agent, by its directory name
    --kind      list: the species read from the folder: memory, skills or tools
    --session   boot, capture: the conversation this message belongs to. boot uses
                it to emit a constitution only when the routed agent changes, so
                without it the constitution is emitted every time. capture files
                the deposit under it and refuses without one
    --cwd       boot: the fleet root, when no path is named. A host with no config
                file needs no config file: `kb boot --cwd $PWD` is complete
    --text      boot: stdin is the message and never a hook payload. Without it the
                first non-space byte decides, and a message that is itself a JSON
                object carrying a `prompt` key would be read as an envelope
    -m          commit: the message. Required, and so is at least one path

source add mints an opaque key and writes `sources/<KEY>.txt`. The key is ten
characters from an alphabet with 0, 1 and O removed, drawn from operating system
entropy, never derived from the title, the author or the path, and never
regenerated. A note cites it as [src:KEY] and carries one bibliography line
`- [src:KEY] <what the record renders>`, which kb check compares byte for byte,
so the reference list is a projection of the pointers and cannot disagree with
the body. --type is one of book, chapter, article, report, webpage, recording.
--retrieval-status is one of full, partial, metadata, unreachable, and it is
required because a source nobody opened has to say so out loud.

sources lists what a base has read. --json emits CSL JSON, which is the export
format from day one: a stable published shape every processor already reads, so
the sources can leave this system without a converter written under pressure.
It is an export and not an import; nothing here reads CSL back.

misses reads kb-misses.txt back: every question the free stage could not answer,
most asked first, with the count, the dates and the vocabulary the base offered at
the time. Beside each one it names the files today's index nearly caught it with,
which is the half a log cannot hold, and prints the keys each of those files
declares. A file the text scorer reached with a keyword score of zero is the whole
finding: the words are in the note and not on its Search for line, so the fix is an
alias line or another key rather than another note. It always prints the path it
read, because KB_MISSES_PATH can move the log and two fleets pointed at one path
share it. --top N is candidates carried per question, the same meaning it has for
every other verb, and not how many questions to print.

It proposes and never applies, unless --apply is given with a gold set, and the
--gold requirement is the whole safety property rather than an argument.

The rule this replaced said the step from proposing an alias to writing one is a
person, on purpose, because a machine that appends on evidence it produced itself
closes the loop with nobody in it. That reason was weaker than it looked: the
evidence is a question a real person asked and this base really failed, which is
as external as evidence gets here.

The reason that does hold is about signal. recall_loss records nothing unless the
verdict is `nothing`, so only a total miss is ever logged; a guess is not, and a
confident hit on the wrong file is not. Alias expansion is additive, so an alias
can never cause a miss, only a confident hit on something else. The one feedback
this system keeps is therefore blind to the only damage an alias can do, and a
writer fed by it would improve the column it can see while quietly degrading the
one it cannot.

So --apply does not close that loop with a model. It closes it with arithmetic.
Every candidate must pass two sides, and both are required: the question it was
proposed for has to stop missing, and no deterministic column of kb eval may drop.
A model proposes the lines and never decides one, which is the division ADR-0018
already draws through retrieval, moved one layer up. The gate never consults the
classifier, so the same candidate judged twice gets the same verdict and an
admitted line can be re-derived. Each admitted line is written with the two
measurements above it, and every refusal is counted in kb-alias-rejections.txt,
because a proposer that keeps offering the same refused line is a signal about
the proposer.

misroute is the half the miss log cannot see, and the agent reports it rather than
you. kb-misses.txt only records a question that reached nothing, so a router that was
confident and wrong leaves it empty. Alias expansion is additive and can only ever
cause that second kind of failure, so a loop reading the miss log alone optimises the
column it can see and degrades the one it cannot.

The reporter is the agent because the agent already knows. kb boot hands it the
constitution above a line saying the choice was the router's and to say so if it is
wrong; it does say so, in prose, and until this verb existed nothing kept it. It sees
the message, the payload and its own base at once, which is the only vantage point in
the loop that can tell `this is not mine` while it is still true.

Evidence, never action. Nothing reads this log and edits a base. It widens what the
proposer can see, and every proposal still has to survive the gate.

abstentions is the third failure and the only one nobody could report. kb-misses.txt
holds a question the library answered nothing for; kb-misroutes.txt holds a confident
wrong choice, filed by the agent that was handed it. When the classifier answers that
no agent owns a subject, there is no miss, because the message often scores well, and
there is no agent to file anything, because none was booted. It left no trace at all.
On 2026-09-04 an abstention on landing page copy produced a report that this fleet had
no copywriter. It has one, and only Richard's own memory caught it.

So the router writes kb-abstentions.txt itself, at the moment it abstains, keyed on the
subject the classifier named rather than on the message: the question a log like this
has to answer is whether the fleet keeps failing to own a kind of work, and keyed on the
message every gap is a count of one forever. Each row carries the classifier's own
reason and the agents that scored, both already produced and both previously discarded.
That second field is what separates a gap that is really missing from an agent that was
passed over, and those two have opposite fixes.

Nothing is recorded when there is no verdict: on a fleet with no classifier configured
that branch fires on every message under the floor, and the count would measure a
missing classifier instead of a missing agent. That population is the miss log's.

The reader re-scores each stored message against the fleet as it stands now, which is
the half the file cannot hold, and the same thing kb misses does with today's index.
It is the keyword fold and it says so: which agents have the vocabulary today, never
which one owns the subject. Delete a row once the fleet covers it.

panel is the objection round, and the router usually opens it rather than you. `kb boot`
already asks a model who owns a message, with every agent's role and edge in front of it;
that verdict now carries REVIEWERS as well, so a message landing in two domains arrives
with the owner, the panel, and the command below already written. Running it by hand is
how a round opens for a piece nobody asked a question about.

One agent owns the piece and is accountable for it. The panel returns named objections
and never rewrites. Every objection is taken, refused in writing, or escalated, and a
blocking objection cannot be refused at all. `--ledger` prints the table that travels with
the piece and exits 1 while the round is open, so a release step can gate on it.

Four things it enforces rather than asks for: the owner cannot review their own piece, an
agent with no blocks.txt cannot be seated because a subagent with no constitution is a
reviewer in costume, blocking is bounded to one per reviewer, and a reviewer who never
answered is recorded as `not-returned` and never as `nothing`.
    --owner     the one agent accountable for the piece
    --reviewer  an agent that must object before it ships. Repeatable, capped at
                three, and never the owner. Left out, the command proposes a panel
                with each agent's role, edge and boot cost, and opens nothing
    --out       where the assembled constitutions are written, default .kb/panel
    --from      which reviewer an answer belongs to, and which reviewer's objection
                a --resolve number refers to when two of them raised one
    --objection what that reviewer found wrong, in one line
    --blocking  this one cannot be refused. One per reviewer
    --resolve   the objection number, as --ledger prints it
    --why       the owner's reason. Required to refuse, because a refusal with no
                reason cannot be audited after the piece underperforms

With no classifier configured there is no panel. margin and contenders look like the
signal and were measured against this fleet's 49 question gold set on 2026-09-04: every
single-owner question has 2 to 10 contenders, and a margin cut of 1.5 fires on 25% of
them, 2.0 on 40%. No cut separates the cases, and the errors are not symmetric: a panel
not convened costs one review, a panel convened wrongly costs about 206,000 tokens.

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
    W06 thin-keywords   a `Search for:` line too short to be found by a real question
    W08 unignored       a .gitignore is here and misses a folder declared private
    W07 dead-key        a key that reaches neither the keyword nor the phrase index
    W03 dash            an em dash or en dash, which house style forbids
    W04 front-matter    front matter declaring source or type with no evidence_tier
                        or valid_for
    W05 no-provenance   a note with no provenance or stage, so who wrote it is unknown
    E04 bad-provenance  provenance or stage carries a value outside the legal set
    E05 dead-citation   a [src:KEY] with no record behind it in this base
    E06 uncited-source  a source record no note points at
    E08 bibliography    a cited key with no bibliography line, or one that drifted
                        from the record
    E09 source-record   a file in sources/ that is not a source record
    W09 superseded      a citation of a source another record replaces

E02, W06 and W07 are asked of every file the index walks, not only of the knowledge
folder, and they skip the files nobody searches for: README.md, MAP.md, AGENTS.md, CLAUDE.md,
what-goes-here.md, MOVED.md, and anything under inbox/ or records/. There is no rule
demanding a MAP.md any more; the index walks files and a base without one indexes.

exit code is 1 when check finds errors, or when --strict and it finds warnings.
";

/// How many line numbers to print before collapsing into a count.
const LINES_SHOWN: usize = 3;

/// Flags that consume the argument after them.
const VALUE_FLAGS: &[&str] = &[
    "--top", "--keys", "--summary", "--folder", "--provenance", "--stage", "--base", "--kind",
    "--captured-from", "--agent", "--session", "--cwd",
    "-m", "--port", "--max", "--gold", "--chose", "--owner", "--why",
    "--reviewer", "--out", "--from", "--objection", "--resolve",
    // `kb source add`. Every one takes a value, and `--author` is the only one that may
    // be repeated, which `flag_values` already knows how to read.
    "--type", "--title", "--author", "--year", "--container", "--volume", "--pages",
    "--publisher", "--url", "--doi", "--isbn", "--lang", "--retrieved-on",
    "--retrieval-status", "--replaces", "--note",
];

/// What build this is, in one line: `kb 0.2.1 (2269ba0, x86_64 linux)`.
///
/// **A binary that cannot name itself cannot be diagnosed once it is somewhere else.**
/// `kb` is vendored into other repositories and shipped inside deployment bundles, so
/// the copy answering a question is routinely not the copy anybody has in front of
/// them. Without this, "the release broke it" is a sentence with no way to check which
/// release, and the version in `Cargo.toml` is the version of the source, not of the
/// file on that host.
///
/// **The commit is compiled in rather than asked for at runtime.** `option_env!` reads
/// the variable when the binary is built, so the release workflow sets `KB_BUILD_SHA`
/// and the value is baked into the artifact it publishes. The alternative was a build
/// script shelling out to git, and it was refused for the reason ADR-0003 already
/// gives: `kb` runs where there is no `.git` and no git binary, and a build that needs
/// git to describe itself describes itself wrong in exactly the place the answer
/// matters. Unset, it says `unknown`, which is the honest name for a local build.
fn version_line() -> String {
    format!(
        "kb {} ({}, {} {})",
        env!("CARGO_PKG_VERSION"),
        option_env!("KB_BUILD_SHA").unwrap_or("unknown"),
        std::env::consts::ARCH,
        std::env::consts::OS,
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    if args[0] == "version" || args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", version_line());
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
        "list" => match list::Filter::parse(
            flag_value(&args, "--base").as_deref(),
            flag_value(&args, "--folder").as_deref(),
            flag_value(&args, "--kind").as_deref(),
            flag_value(&args, "--stage").as_deref(),
            flag_value(&args, "--provenance").as_deref(),
        ) {
            Ok(f) => cmd_list(&paths_or_default(&positional), all, &f, json),
            Err(e) => {
                // Exit 2, the code every other bad argument uses here, and the legal set
                // named on stderr. An empty result set would have been exit 0 and a lie.
                eprintln!("kb: {e}");
                ExitCode::from(2)
            }
        },
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
        // `positional` raw, not `paths_or_default`, because "no path was named" is a state
        // this command can still answer: `--cwd`, or the envelope's own field, supplies it.
        "boot" => cmd_boot(
            &positional,
            all,
            top,
            flag_value(&args, "--session").as_deref(),
            flag_value(&args, "--cwd").as_deref(),
            args.iter().any(|a| a == "--text"),
        ),
        "misses" => cmd_misses(
            &paths_or_default(&positional),
            all,
            top,
            json,
            args.iter().any(|a| a == "--apply"),
            flag_value(&args, "--gold").as_deref(),
        ),
        "misroute" => cmd_misroute(
            &positional,
            flag_value(&args, "--chose").as_deref(),
            flag_value(&args, "--owner").as_deref(),
            flag_value(&args, "--why").as_deref(),
        ),
        "misroutes" => cmd_misroutes(paths_or_default(&positional)[0], top),
        "abstentions" => cmd_abstentions(paths_or_default(&positional)[0], all, top),
        "panel" => cmd_panel(&args, &positional, all, top, json),
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
        "ingest" => {
            if positional.is_empty() {
                eprintln!(
                    "kb ingest: name the document to ingest. `kb ingest <file> [path] \
                     [--agent NAME] [--keep]`"
                );
                return ExitCode::from(2);
            }
            cmd_ingest(
                Path::new(positional[0]),
                &paths_or_default(&positional[1..]),
                all,
                top,
                flag_value(&args, "--agent"),
                args.iter().any(|a| a == "--keep"),
                args.iter().any(|a| a == "--dry-run"),
            )
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
        "source" => {
            if positional.first() != Some(&"add") {
                eprintln!("kb: the only source verb is `add`
");
                print!("{USAGE}");
                return ExitCode::from(2);
            }
            let root = positional.get(1).copied().unwrap_or(".");
            cmd_source_add(Path::new(root), &args)
        }
        "sources" => cmd_sources(&paths_or_default(&positional), all, json),
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

/// Every value given for a flag that may be repeated, as in `--reviewer a --reviewer b`.
///
/// `flag_value` takes the first and drops the rest, which is right for `--top` and wrong
/// for a panel: a round opened with three reviewers named and one seated is a round that
/// looks reviewed and was not.
fn flag_values(args: &[String], flag: &str) -> Vec<String> {
    args.iter()
        .enumerate()
        .filter(|(_, a)| *a == flag)
        .filter_map(|(i, _)| args.get(i + 1).cloned())
        .filter(|v| !v.starts_with("--"))
        .collect()
}

fn paths_or_default<'a>(given: &[&'a str]) -> Vec<&'a str> {
    if given.is_empty() { vec!["."] } else { given.to_vec() }
}

// ---------------------------------------------------------------------------
// ingest
// ---------------------------------------------------------------------------

/// The text inside a document, by whatever means that document needs.
///
/// **Extraction shells out and does not link.** `kb` has one dependency and keeping it
/// there is worth more than the convenience of a PDF crate: a parser for every format
/// anyone might hand over is an unbounded surface, and the formats change. So `.txt` and
/// `.md` are read directly, and everything else runs `tools/extract/<ext>.cmd` with the
/// document as its argument and takes stdout, which is the same process contract
/// `promote-claude.cmd` and the classifier already use.
fn extract(fleet_root: &Path, source: &Path) -> Result<String, String> {
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if ext == "txt" || ext == "md" {
        return std::fs::read_to_string(source).map_err(|e| e.to_string());
    }

    let script = fleet_root.join("tools").join("extract").join(format!("{ext}.cmd"));
    if !script.is_file() {
        return Err(format!(
            "nothing here knows how to read a .{ext}. Write {} so that it prints the \
             document's text on stdout, given the path as its one argument. `pdftotext \
             -layout -enc UTF-8 %1 -` is the whole of the pdf one.",
            script.display()
        ));
    }
    let out = std::process::Command::new(&script)
        .arg(source)
        .output()
        .map_err(|e| format!("could not run {}: {e}", script.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} exited with {}: {}",
            script.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// One document, from wherever it is, to notes in the base that owns its subject.
fn cmd_ingest(
    source: &Path,
    paths: &[&str],
    all: bool,
    top: usize,
    agent: Option<String>,
    keep: bool,
    dry_run: bool,
) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let root = given.first().copied().unwrap_or_else(|| Path::new("."));

    if !source.is_file() {
        eprintln!("kb ingest: there is no file at {}", source.display());
        return ExitCode::from(1);
    }

    let mut memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };

    let promoter = memory.promoter();
    let reviewer = memory.reviewer();
    if matches!(promoter, classify::Classifier::None)
        || matches!(reviewer, classify::Classifier::None)
    {
        eprintln!(
            "kb ingest: the fleet manifest needs both `promoter = ...` and `reviewer = ...`. \
             Ingestion writes into the base, and a run with no reviewer is automatic \
             extraction into durable memory, which is the one thing promotion exists to \
             not be."
        );
        return ExitCode::from(2);
    }

    let text = match extract(root, source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kb ingest: {e}");
            return ExitCode::from(1);
        }
    };
    if text.trim().is_empty() {
        eprintln!(
            "kb ingest: {} produced no text. It has not been moved, because the original \
             is still the only copy of whatever it holds.",
            source.display()
        );
        return ExitCode::from(1);
    }

    // **Extraction happens before the move, and the reason is routing.** A document cannot
    // be filed into the deposit of an agent nobody has chosen yet, and choosing needs the
    // words. Nothing is destroyed in this window: reading a file is not a mutation, and if
    // routing abstains the document is exactly where the caller left it.
    let owner = match agent {
        Some(name) => name,
        None => {
            // The document's own words, capped, because a router asked a whole textbook
            // scores every base on vocabulary breadth rather than on subject.
            let question: String = text.chars().take(4000).collect();
            let answer = memory.ask(&question, top);
            match answer.agent {
                Some(choice) => {
                    println!(
                        "routed to {} (score {:.1}, {:.2}x over the runner-up, {} agent(s) \
                         scored)",
                        choice.agent, choice.score, choice.margin, choice.contenders
                    );
                    choice.agent
                }
                None => {
                    // A router that always picks is the failure ADR-0013 spent a day
                    // measuring. Filing a document into the wrong base is worse than not
                    // filing it, because the note is then reachable and wrong.
                    eprintln!(
                        "kb ingest: no agent in this fleet owns what {} is about, so there \
                         is nowhere to file it. Name one with --agent, or decide that this \
                         subject needs an agent. Nothing was moved.",
                        source.display()
                    );
                    return ExitCode::from(1);
                }
            }
        }
    };

    let Some(agent_root) = memory
        .agents
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&owner))
        .map(|a| a.root.clone())
    else {
        eprintln!("kb ingest: no agent called '{owner}'. `kb fleet` lists the ones that exist.");
        return ExitCode::from(1);
    };

    if dry_run {
        println!(
            "dry run: {} would be staged into {}/{} and distilled there. Nothing moved.",
            source.display(),
            owner,
            promote::DEPOSIT
        );
        return ExitCode::SUCCESS;
    }

    let today = today();
    let staged = match ingest::stage(&agent_root, &owner, source, &text, &today) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("kb ingest: {e}");
            return ExitCode::from(1);
        }
    };
    println!("staged  {}", staged.deposit.display());
    println!("        original at {}", staged.original.display());

    // The memory was opened before the deposit existed, so promotion would walk a roster
    // that has it and an index that does not.
    if let Err(e) = memory.reread() {
        eprintln!("kb ingest: could not re-read the base after staging: {e}");
        return ExitCode::from(1);
    }

    let outcome = promote::run(
        &mut memory,
        root,
        &promoter,
        &reviewer,
        top,
        false,
        &today,
        None,
        Some(&staged.deposit),
    );

    for d in &outcome.decided {
        let head = format!("{}/{}", d.proposal.agent, d.proposal.slug);
        match &d.written {
            Some(p) => println!("  wrote   {head}\n          {}", p.display()),
            None => {
                println!("  refused {head}");
                for r in d.refusals() {
                    println!("          {} says: {}", r.lens.name(), r.reason);
                }
            }
        }
    }
    for u in &outcome.unreachable {
        println!("  could not reach {u}");
    }
    for (slug, keys) in &outcome.dropped_keys {
        println!("  {slug}: dropped unreachable key(s) {}", keys.join(", "));
    }

    // The name promotion recorded in `captured_from`, which is what the proof matches on.
    let deposit_name = format!(
        "{}/{}",
        owner.to_lowercase(),
        staged
            .deposit
            .strip_prefix(&agent_root)
            .unwrap_or(&staged.deposit)
            .to_string_lossy()
            .replace('\\', "/")
    );

    let verdict = if keep {
        Err(ingest::Kept::Asked)
    } else {
        ingest::may_delete(root, &deposit_name, &outcome)
    };

    match verdict {
        Ok(()) => {
            let gone = ingest::destroy(&staged);
            for p in &gone {
                println!("removed {}", p.display());
            }
            println!(
                "\n{} note(s) written. The document is gone from this machine and what it \
                 said is in the base.",
                outcome.written()
            );
            ExitCode::SUCCESS
        }
        Err(why) => {
            println!("\nkept: {why}.");
            println!("      the document is at {}", staged.original.display());
            println!("      its text is at     {}", staged.deposit.display());
            // Exit 1 on anything but --keep: the caller asked for a document to be
            // absorbed and it was not, and a command that reports success for that is a
            // command whose failures nobody finds.
            if matches!(why, ingest::Kept::Asked) { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
    }
}

// ---------------------------------------------------------------------------
// check
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// sources
// ---------------------------------------------------------------------------

/// The flags `kb source add` turns into record fields, and the whole argument surface.
///
/// A list rather than a signature, so the parser stays one loop and adding a field is one
/// line here. `--author` is the only repeatable one; the rest take the first value given,
/// which is what `flag_value` already does everywhere else in this file.
const SOURCE_FIELDS: &[&str] = &[
    "type", "title", "author", "year", "container", "volume", "pages", "publisher",
    "url", "doi", "isbn", "lang", "retrieved-on", "retrieval-status", "replaces", "note",
];

/// Mints a key and writes a source record, then prints the line a note has to carry.
///
/// **The rendered bibliography line is printed here on purpose.** `kb check` compares that
/// line byte for byte against the record, so a writer who has to invent it by hand fails the
/// check on their first try and learns to distrust it. Printing it makes the copy a paste
/// rather than a restatement, which is the difference between a projection and a fourth
/// place the same facts are maintained.
fn cmd_source_add(root: &Path, args: &[String]) -> ExitCode {
    let mut given: Vec<(String, String)> = Vec::new();
    for field in SOURCE_FIELDS {
        let flag = format!("--{field}");
        if *field == "author" {
            for v in flag_values(args, &flag) {
                given.push((field.to_string(), v));
            }
        } else if let Some(v) = flag_value(args, &flag) {
            given.push((field.to_string(), v));
        }
    }

    match sources::add(root, &given) {
        Ok(s) => {
            println!("wrote {}", s.path.display());
            println!();
            println!("Cite it as [src:{}], and carry one bibliography line:", s.key);
            println!("- [src:{}] {}", s.key, sources::render_line(&s));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            // Exit 2 for a bad argument, the code every other argument failure uses here.
            // An IO failure is the machine and not the caller, so it takes 1.
            match e {
                sources::AddError::Io(_, _) => ExitCode::from(1),
                _ => ExitCode::from(2),
            }
        }
    }
}

/// Lists a base's sources, or emits them as CSL JSON.
///
/// Read off disk rather than out of the index, deliberately: the records are the truth by
/// ADR-0003 and the SQLite table is derived from them, so an export that read the index
/// would be an export of a cache and would be stale exactly when somebody had just added
/// something and not reindexed.
fn cmd_sources(paths: &[&str], all: bool, json_out: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let bases = memory::expand_roots(&given);

    let mut every = Vec::new();
    let mut broken = 0usize;
    for path in &bases {
        // A base may declare `sources/` private, and the person's base is private as a
        // whole. Listing it anyway would be `kb` publishing something the base said not to,
        // which is the one thing ADR-0034 says it never does.
        if !all && base::private_layer(path).covers(sources::DIR) {
            continue;
        }
        let (found, bad) = sources::load(path);
        broken += bad.len();
        for b in &bad {
            eprintln!("kb: {}/{}: {}", path.display(), b.path, b.problem);
        }
        every.extend(found);
    }

    if json_out {
        println!("{}", sources::to_csl(&every).to_string());
    } else if every.is_empty() {
        println!("no source records");
    } else {
        for s in &every {
            println!("[src:{}] {}", s.key, sources::render_line(s));
        }
        println!();
        println!("{} sources in {} base(s)", every.len(), bases.len());
    }
    if broken > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

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
        // Optional here and mandatory in nothing: a note typed by a person is not
        // captured from anywhere, and demanding a source would only teach the caller to
        // invent one. `kb promote` always passes it, because it always knows.
        captured_from: flag_value(args, "--captured-from"),
        body,
    };

    match write::note(fleet, agent, slug, &spec) {
        Ok(made) => {
            println!("wrote {}", made.note.display());
            println!("  listed in {} under {}", made.map.display(), made.section);
            if !made.dropped_keys.is_empty() {
                // Said out loud rather than swallowed. The note is reachable by what is
                // left, and the caller should know which of the words it chose reach
                // nothing, because it will choose them again otherwise.
                println!(
                    "  dropped {} key(s) the index cannot reach: {}",
                    made.dropped_keys.len(),
                    made.dropped_keys.join(", ")
                );
            }
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
        // **The shape stays a bare array on purpose.** Wrapping it in an object to carry
        // the unreachable count would break every consumer that parses this, and the
        // count has two other places to be printed.
        let entries: Vec<index::Entry> =
            bases.iter().flat_map(|(_, b)| index::build(b).entries).collect();
        print!("{}", index::to_json(&entries));
        return ExitCode::SUCCESS;
    }

    // One index per agent, written beside the agent. The shared index this replaces
    // defaulted to a path relative to the working directory, so which database you
    // got depended on where you happened to be standing, and that cost three
    // separate incidents in one week.
    let mut files = 0usize;
    let mut chunks = 0usize;
    // What the walk could not build an entry for, and what it was never meant to. Counted
    // here rather than left to `kb check`, because a file that scores zero on every question
    // is silent everywhere else: it is on disk, it opens, and it reads fine.
    let mut unreachable: Vec<String> = Vec::new();
    let mut exempt = 0usize;
    for (root, base) in &bases {
        let name = label(base);
        let path = memory::index_path(root);

        let built = index::build(base);
        exempt += built.exempt;
        unreachable.extend(built.unreachable.iter().map(|rel| format!("{name}/{rel}")));

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
                    "  {name:<10} {} reindexed, {} unchanged, {} removed, {} chunks, {} unreachable",
                    r.reindexed,
                    r.unchanged,
                    r.removed,
                    r.chunks,
                    built.unreachable.len()
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

    // **`unsearchable by design` is on the line because the remainder needs a name.** A
    // reader who subtracts the entries from the files is otherwise left with a gap and no
    // way to tell a deliberate exemption from a defect: on this fleet every keyless file is
    // exempt, so the two numbers are 0 and 63 and the wrong reading of one number would be
    // that the base is broken.
    println!(
        "  total: {files} files, {chunks} chunks, {} indexes, {} unreachable, {exempt} unsearchable by design",
        bases.len(),
        unreachable.len()
    );

    if !unreachable.is_empty() {
        // The first few, and a pointer rather than the whole list. `kb check` reports the
        // same set as E02, file by file with the reason attached, and two outputs both
        // claiming to be the authority is how they come to disagree.
        for path in unreachable.iter().take(memory::Memory::PATHS_SHOWN) {
            println!("    {path}");
        }
        if unreachable.len() > memory::Memory::PATHS_SHOWN {
            println!("    ... and {} more", unreachable.len() - memory::Memory::PATHS_SHOWN);
        }
        println!("    no `Search for:` line, so no question reaches them. `kb check` names each one as E02");
    }

    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/// Lists what the bases hold, by facet, with nothing ranked.
///
/// **The verb exists because the other four all score a question against a floor.** A
/// filter shaped ask carries no ranking problem, so putting it through `kb route` gets
/// it compared to `SCORE_FLOOR` and returned as a `Guess`, and the reader cannot tell a
/// base holding three matching files from a base holding none. There is no floor here
/// and no verdict, because nothing was asked that a number could answer.
///
/// Opened through `Memory::open` exactly as `cmd_fleet` and `cmd_route` do, so the
/// private layer is decided in one place for every surface.
fn cmd_list(paths: &[&str], all: bool, filter: &list::Filter, as_json: bool) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };

    let rows = match memory.list(filter) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };

    if as_json {
        println!("{}", list::to_json(&rows).to_string());
        return ExitCode::SUCCESS;
    }

    print!("{}", list::to_text(&rows));
    // The count on its own line, always, including when it is zero. An empty listing
    // with no line under it reads as a command that did not run, which is the reading
    // `print_if_nothing_to_search` exists to prevent one surface over.
    println!("
{} files", rows.len());
    if !all {
        println!("  the private layer is not listed. --all includes it");
    }
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

/// Where the shortfall sentences fold, matching the hand-wrapped literals around them.
///
/// Every other block on this terminal is wrapped by whoever typed it, at roughly this
/// width, because those lines are string literals in the source. The shortfall sentences
/// are built at runtime and were not, and `kb route` on a one entry base printed a 230
/// character line straight off the edge of the window. Same defect as the near miss key
/// list two changes ago, and caught the same way: by running it rather than by asserting
/// on the string, which is happy either way.
const SHORTFALL_WIDTH: usize = 72;

/// Greedy word wrap. **Never splits a word**, so a path or a backticked verb comes out
/// whole and overruns rather than arriving as two halves neither of which can be searched
/// for or pasted.
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match rows.last_mut() {
            Some(row) if row.chars().count() + 1 + word.chars().count() <= width => {
                row.push(' ');
                row.push_str(word);
            }
            _ => rows.push(word.to_string()),
        }
    }
    rows
}

/// The state the refusal above was refused from, under it.
///
/// **The refusal arrived at the wrong moment to be understood, and this is the moment.**
/// A reader who is told only that nothing matched has no way to tell a fleet of one entry
/// from a fleet of a thousand, a base whose notes carry no keys from one whose notes are
/// fine and simply do not cover the subject. Those want opposite work, and the numbers
/// that separate them were already computed and thrown away.
///
/// Prints nothing at all when there is nothing true to say, including the blank line: a
/// gap under a refusal reads as a section that failed to render. The sentences are
/// [`memory::Shortfall::lines`]'s, not this function's, so `mcp.rs` and `boot.rs` cannot
/// drift from what a terminal says and a test can hold the wording.
fn print_shortfall(memory: &memory::Memory, confidence: &memory::Confidence) {
    let said = memory.shortfall(confidence).lines();
    if said.is_empty() {
        return;
    }
    println!();
    for line in said {
        for row in wrapped(&line, SHORTFALL_WIDTH) {
            println!("  {row}");
        }
    }
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
    // **The diagnosis the paragraph above only gestures at.** A base can be empty in the
    // sense that matters here while holding markdown somebody already wrote: no
    // `Search for:` line means no entry, so the files are on disk and the library is
    // still empty. Telling that reader to start writing files is telling them to do
    // again what they have already done.
    let keyless = memory.unreachable().len();
    if keyless > 0 {
        println!();
        let (are, them) = match keyless {
            1 => ("file is", "it"),
            _ => ("files are", "them"),
        };
        println!("  {keyless} markdown {are} already here without that line, which is");
        println!("  why there is nothing to search. `kb check` names {them}.");
    }
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
                print_shortfall(&memory, &answer.confidence);
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
            print_shortfall(&memory, &answer.confidence);
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
/// - `unreachable` says what the base holds and cannot reach, in two counts because they
///   are two answers to one caller question and want opposite work. `files` is an
///   authoring problem: a file with no `Search for:` line is on disk, readable, and scores
///   zero on every question, and until now it was reported only through `kb check` E02,
///   which nobody is required to run. `unindexed` is a `kb index` that never ran, which is
///   the window the SessionEnd hook opens every time: it runs `kb capture`, detaches
///   `kb promote`, and never indexes. It sits at the top level rather than inside
///   `indexed`, because that object counts what worked and burying a failure inside a
///   success is how it gets skipped.
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
    gate.set("floor", score(answer.confidence.floor as f64));
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

    // **Beside `gate`, not inside `indexed`.** The counts are exact and only the arrays are
    // capped at `Memory::PATHS_SHOWN`: a capped count beside a capped list would understate
    // the size of the problem by exactly as much as the list was shortened.
    let paths = |all: &[&str]| {
        json::Value::Arr(
            all.iter()
                .take(memory::Memory::PATHS_SHOWN)
                .map(|p| (*p).into())
                .collect(),
        )
    };
    let cannot_reach = memory.unreachable();
    let lagging = memory.unindexed();
    let mut unreachable = json::Value::obj();
    unreachable.set("files", cannot_reach.len().into());
    unreachable.set("paths", paths(&cannot_reach));
    unreachable.set("unindexed", lagging.len().into());
    unreachable.set("unindexed_paths", paths(&lagging));
    out.set("unreachable", unreachable);

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

/// `kb misses`: the recall loss log, read back with what nearly caught each question.
///
/// **Reads the log from `Memory::opened.first()` and not from `paths[0]`.** That is
/// exactly where [`memory::Memory::recall_loss`] writes it, and a reader that recomputed
/// the root from the argument list would be reading a different file the moment the path
/// list is defaulted, reordered, or a fleet root is expanded into its agents.
///
/// Exit 0 when there is no log: a fleet nothing has missed against is healthy, not
/// broken. Exit 1 only when the base cannot be opened or the log cannot be read.
fn cmd_misses(
    paths: &[&str],
    all: bool,
    top: usize,
    as_json: bool,
    apply: bool,
    gold: Option<&str>,
) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let memory = match memory::Memory::open(&given, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };
    // **The same warning `cmd_route` prints, for a sharper reason here.** An emptied
    // index makes every near miss list empty, which reads as "nothing nearly matched
    // this question" when the truth is "there is nothing to match against". That would
    // send a reader off to write alias lines for a problem that is an unrun `kb index`.
    if memory.index_was_rebuilt {
        eprintln!("kb: an index predated the private column (ADR-0034) and was emptied. Run `kb index`.");
    }

    let root = memory.opened.first().cloned().unwrap_or_default();
    let log = misses::path_in(&root);
    let lost = match misses::load(&log) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };

    if as_json {
        println!("{}", misses_payload(&log, &lost, &memory, top).to_string());
        return ExitCode::SUCCESS;
    }

    // Printed whether or not anything was found. A reader told "nothing was lost"
    // without being told which file was consulted cannot check the claim, and
    // `KB_MISSES_PATH` makes that a real question rather than a pedantic one.
    println!("log:      {}", log.display());
    println!(
        "indexed:  {} entries across {} agents, floor {:.2}",
        memory.entry_count(),
        memory.agents.len(),
        memory.floor()
    );

    if lost.is_empty() {
        println!();
        println!("  nothing has missed here. Either no question has reached this base and");
        println!("  failed, or the log lives somewhere else: KB_MISSES_PATH moves it.");
        return ExitCode::SUCCESS;
    }

    let asks: u32 = lost.iter().map(|m| m.count).sum();
    println!("lost:     {} distinct questions, {asks} asks", lost.len());

    let floor = memory.floor();
    for m in &lost {
        print_miss(m, &memory.near_misses(&m.question, top), floor);
    }

    if apply {
        return apply_aliases(&memory, &root, &lost, top, gold);
    }

    println!();
    println!("  Write the alias line, or add the key to that file's `Search for:` line,");
    println!("  then delete the question from the log. Or hand it to `--apply --gold <tsv>`,");
    println!("  which proposes with a model and admits only what the gold set survives.");

    ExitCode::SUCCESS
}

/// The apply half, kept out of `cmd_misses` so the reading path stays the reading path.
///
/// Two preconditions, and both refuse rather than degrade. **No gold set, no apply**: the
/// gate is the only thing that makes writing safe, and a gate with nothing to measure
/// against is a rubber stamp. **No model, no apply**: falling back to generating candidates
/// mechanically would put an unreviewed proposer in the loop, which is the shape this whole
/// design exists to avoid. `kb promote` refuses on the same grounds and for the same reason.
fn apply_aliases(
    memory: &memory::Memory,
    root: &Path,
    lost: &[misses::Miss],
    top: usize,
    gold: Option<&str>,
) -> ExitCode {
    let Some(gold_path) = gold else {
        eprintln!();
        eprintln!("kb: --apply needs --gold <tsv>, and that is the safety property rather than");
        eprintln!("    an argument. The gate admits a line only when the question it was");
        eprintln!("    proposed for stops missing AND no column of the gold set drops. With no");
        eprintln!("    set to measure against, the second half cannot run and the first half");
        eprintln!("    alone admits any line that widens recall at any cost to precision.");
        return ExitCode::from(2);
    };

    let rows = match eval::read_gold(Path::new(gold_path)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(2);
        }
    };
    let stale = eval::stale_answers(&rows, memory);
    if !stale.is_empty() {
        eprintln!("kb: the gold set points at {} file(s) that are not here:", stale.len());
        for s in &stale {
            eprintln!("      {s}");
        }
        eprintln!("    Refusing to gate against a set that has gone stale, for the same reason");
        eprintln!("    `kb eval` refuses to grade against one.");
        return ExitCode::from(2);
    }

    let model = memory.promoter();
    if matches!(model, classify::Classifier::None) {
        eprintln!();
        eprintln!("kb: --apply needs a model to propose with. Add `promoter = ...` to fleet.txt.");
        eprintln!("    The model proposes and never decides; the gate decides and never guesses.");
        return ExitCode::from(2);
    }

    let paths: Vec<&Path> = vec![root];
    let today = misses::today();
    let (mut admitted, mut refused, mut proposed) = (0usize, 0usize, 0usize);
    let (mut silent, mut declined, mut unparseable) = (0usize, 0usize, 0usize);

    println!();
    println!("applying, gated on {} ({} rows)", gold_path, rows.len());

    for m in lost.iter().take(top) {
        let near = memory.near_misses(&m.question, top);
        println!();
        println!("  {}x  {}", m.count, m.question);
        let candidates = match gate::propose(&model, root, &m.question, &m.looked_like, &near) {
            gate::Proposal::ModelSilent => {
                silent += 1;
                println!("      the proposer did not answer. Not a decision, an outage.");
                continue;
            }
            gate::Proposal::Declined => {
                declined += 1;
                println!("      the proposer declined: no alias here would honestly help.");
                continue;
            }
            gate::Proposal::Lines { candidates, dropped } => {
                if dropped > 0 {
                    unparseable += dropped;
                    println!("      {dropped} line(s) dropped as unparseable, not repaired.");
                }
                candidates
            }
        };
        for c in candidates {
            proposed += 1;
            match gate::judge(&paths, true, &rows, top, &c) {
                Err(e) => {
                    refused += 1;
                    println!("      refused  {}", e);
                    gate::record_refusal(root, &c, &e, &today);
                }
                Ok(v) if !v.admitted => {
                    refused += 1;
                    println!("      refused  {}  {}", c.line(), v.reason);
                    gate::record_refusal(root, &c, &v.reason, &today);
                }
                Ok(v) => {
                    let base_root = memory
                        .agents
                        .iter()
                        .find(|a| a.name.eq_ignore_ascii_case(&c.base))
                        .map(|a| a.root.clone());
                    let Some(base_root) = base_root else {
                        refused += 1;
                        let why = format!("no base named `{}` in this fleet", c.base);
                        println!("      refused  {}  {why}", c.line());
                        gate::record_refusal(root, &c, &why, &today);
                        continue;
                    };
                    match gate::write_line(&base_root, &c, &v, &today) {
                        Ok(path) => {
                            admitted += 1;
                            println!("      ADMITTED {}", c.line());
                            println!("               {} -> {}", v.before.line(), v.after.line());
                            println!("               written to {}", path.display());
                        }
                        Err(e) => {
                            refused += 1;
                            println!("      refused  {}  {e}", c.line());
                            gate::record_refusal(root, &c, &e, &today);
                        }
                    }
                }
            }
        }
    }

    println!();
    println!("  proposed {proposed}, admitted {admitted}, refused {refused}");
    // Kept apart on purpose. A run that admits nothing because the proposer never ran and a
    // run that admits nothing because the proposer judged there was nothing to add are the
    // same number and opposite facts.
    println!("  proposer: declined {declined}, silent {silent}, unparseable lines {unparseable}");
    if silent > 0 {
        println!("  A silent proposer is an outage and not a verdict. Check the `promoter =`");
        println!("  command in fleet.txt before reading this run as `nothing to fix`.");
    }
    if admitted > 0 {
        println!("  Run `kb index` so the second scorer sees the widened queries, then re-run");
        println!("  `kb eval` yourself: this gate proves no column dropped, not that anything");
        println!("  improved beyond the questions it was measured on.");
    }
    if refused > 0 {
        println!("  Refusals are counted in {}.", root.join(gate::ALIAS_REJECTIONS_TXT).display());
    }
    ExitCode::SUCCESS
}

/// Records one misroute, reported by the agent that was handed the message.
///
/// **Both agent names are checked against the roster.** A typo here is not a harmless
/// typo: it produces a row that reads like evidence, folds into its own count, and points
/// at an agent that does not exist, so nobody can act on it and nobody can tell it apart
/// from a real report by looking. `none` and `-` are accepted for the owner, and they mean
/// something a name cannot: that no agent should have taken this at all, which is a finding
/// about the fleet rather than about the router.
fn cmd_misroute(
    positional: &[&str],
    chose: Option<&str>,
    owner: Option<&str>,
    why: Option<&str>,
) -> ExitCode {
    let (Some(chose), Some(owner)) = (chose, owner) else {
        eprintln!("kb: misroute needs --chose <agent> and --owner <agent|none>.");
        eprintln!("    --chose is who kb boot picked, --owner is who should have had it.");
        eprintln!("    Reading the log back is `kb misroutes`.");
        return ExitCode::from(2);
    };
    let Some(question) = positional.first() else {
        eprintln!("kb: misroute needs the message that was routed wrong, as one argument.");
        return ExitCode::from(2);
    };
    let root = Path::new(positional.get(1).copied().unwrap_or("."));

    let memory = match memory::Memory::open(&[root], true) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };
    let roster = memory.roster();
    let known = |name: &str| roster.iter().any(|r| r.eq_ignore_ascii_case(name));
    let nobody = owner.eq_ignore_ascii_case("none") || owner == "-";

    if !known(chose) {
        eprintln!("kb: no routable agent named `{chose}`. The fleet: {}", roster.join(", "));
        return ExitCode::from(2);
    }
    if !nobody && !known(owner) {
        eprintln!("kb: no routable agent named `{owner}`. The fleet: {}", roster.join(", "));
        eprintln!("    Use `none` when the message belonged to no agent at all.");
        return ExitCode::from(2);
    }
    if !nobody && chose.eq_ignore_ascii_case(owner) {
        eprintln!("kb: `{chose}` was chosen and is named as the owner, so nothing was wrong.");
        return ExitCode::from(2);
    }

    let stored_owner = if nobody { "-" } else { owner };
    misroute::record(root, chose, stored_owner, question, why.unwrap_or(""), &misses::today());
    println!("recorded: {chose} was handed a message that belongs to {}", if nobody { "nobody" } else { owner });
    println!("  {}", misroute::path_in(root).display());
    println!();
    println!("  This is evidence and not a fix. Nothing reads it and edits a base: it widens");
    println!("  what `kb misses --apply` can propose, and the gate still decides.");
    ExitCode::SUCCESS
}

/// Reads the misroute log back, busiest first, which is which one to fix next.
fn cmd_misroutes(path: &str, top: usize) -> ExitCode {
    let root = Path::new(path);
    let log = misroute::path_in(root);
    let rows = misroute::load(&log);

    println!("log:      {}", log.display());
    if rows.is_empty() {
        println!();
        println!("  nothing reported. Either the router has not been caught yet, or the agents");
        println!("  are not calling `kb misroute` when they notice. The second one is the more");
        println!("  likely of the two and it is silent, which is why this line says so.");
        return ExitCode::SUCCESS;
    }
    let total: u32 = rows.iter().map(|r| r.count).sum();
    println!("reported: {} distinct, {total} in total", rows.len());
    for r in rows.iter().take(top.max(1) * 4) {
        println!();
        let owner = if r.owner == "-" { "nobody".to_string() } else { r.owner.clone() };
        println!("   {}x  {} to {}  ({} to {})", r.count, r.chose, owner, r.first, r.last);
        println!("        {}", r.question);
        if !r.why.is_empty() {
            println!("        why: {}", r.why);
        }
    }
    println!();
    println!("  A count above one is a standing defect in the keys or the aliases, not an");
    println!("  unusual message. Delete the line once the routing it describes is fixed.");
    ExitCode::SUCCESS
}

/// Reads the abstention log back, and re-scores each gap against the fleet as it stands now.
///
/// **The recomputed line is the whole reason this is a verb and not `cat`.** The stored
/// contenders describe the fleet on the day it abstained, and the fleet changes: an agent is
/// created, a card gains an edge, a note lands. A row whose contenders today name somebody
/// they did not name then is a gap that may already be closed, and nobody can see that by
/// reading the file. Same mechanism `kb misses` uses when it names the files today's index
/// nearly caught a logged question with: the log holds what was true then, the reader holds
/// what is true now, and only the pair is actionable.
///
/// It is the keyword fold and it says so on the line. Re-running the classifier would be the
/// literal re-judgement and costs a model call of 13 to 16 seconds per row, on a fleet where
/// the operator asked to read a log. What the fold answers is narrower and enough: which
/// agents have vocabulary for this message today.
fn cmd_abstentions(path: &str, all: bool, top: usize) -> ExitCode {
    let root = Path::new(path);
    let log = abstain::path_in(root);
    let rows = abstain::load(&log);

    println!("log:      {}", log.display());
    if rows.is_empty() {
        println!();
        println!("  no coverage gap recorded. Either the router has not abstained on this fleet,");
        println!("  or it could not write the log, which it says on stderr at the time and");
        println!("  nowhere afterwards. A fleet with no classifier configured records nothing");
        println!("  here on purpose: with no model reading the roster there is no judgement");
        println!("  that nobody owns a subject, only a score under the floor, and questions");
        println!("  that score nothing are counted in kb-misses.txt already.");
        return ExitCode::SUCCESS;
    }

    let memory = match memory::Memory::open(&[root], all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };

    let total: u32 = rows.iter().map(|r| r.count).sum();
    println!("recorded: {} distinct subject(s), {total} abstention(s) in total", rows.len());
    for r in rows.iter().take(top.max(1) * 4) {
        let nearest = match r.nearest.as_str() {
            "-" | "" => "no agent near enough to name".to_string(),
            name => format!("nearest {name}"),
        };
        println!();
        println!("   {}x  {}, {nearest}  ({} to {})", r.count, r.coverage, r.first, r.last);
        println!("        subject: {}", if r.subject.is_empty() { "(unnamed)" } else { &r.subject });
        println!("        message: {}", r.message);
        if !r.reason.is_empty() {
            println!("        reason:  {}", r.reason);
        }
        println!(
            "        scored then: {}",
            if r.contenders.is_empty() { "nothing at all" } else { &r.contenders }
        );

        // The half the file cannot hold, computed against the fleet as it stands now.
        let now = memory.ask(&r.message, top);
        let today: Vec<String> = now
            .agent
            .iter()
            .flat_map(|c| c.totals.iter())
            .take(3)
            .map(|(name, weight)| format!("{name} {weight:.1}"))
            .collect();
        println!(
            "        scored now:  {}",
            if today.is_empty() { "nothing at all".to_string() } else { today.join(", ") }
        );
    }
    println!();
    println!("  `scored now` is the keyword fold against today's fleet, not a re-judgement:");
    println!("  it says which agents have the vocabulary, never which one owns the subject.");
    println!("  A name there that is not in `scored then` is an agent created or a card");
    println!("  widened since, and the gap may already be closed. An agent that scored well");
    println!("  and was still not chosen is that agent's card to fix, not a new agent to");
    println!("  create. Delete the row once the fleet covers the subject.");
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// panel
// ---------------------------------------------------------------------------

/// `kb panel`: the objection round, for any owner and any artifact.
///
/// **It does not coordinate and it does not call a model.** The protocol it promotes says
/// exactly that about itself: it runs on the mechanism this fleet already has rather than
/// on a coordinator agent that does not exist. So this assembles what a session needs to
/// run the round, prices it before anything is spent, and keeps the accounting.
///
/// **The output is written for the model driving the session, not only for a terminal.**
/// That is why it prints the exact instruction to hand a subagent and the exact commands
/// that record what comes back: a report a model has to translate into commands is a
/// report a model will translate wrongly. `--json` carries the same facts for a caller
/// that parses rather than reads.
fn cmd_panel(args: &[String], positional: &[&str], all: bool, top: usize, json: bool) -> ExitCode {
    let ledger_mode = args.iter().any(|a| a == "--ledger");
    let from = flag_value(args, "--from");
    let resolving = flag_value(args, "--resolve");

    // The artifact is the first positional and the fleet root is the second, so a round
    // and the base it is scored against never have to be told apart by shape.
    let artifact = positional.first().copied();
    let root = Path::new(positional.get(1).copied().unwrap_or("."));

    if ledger_mode {
        return panel_ledger(root, artifact, json);
    }
    // **`--resolve` is tested before `--from`, and the order is not arbitrary.** `--from`
    // also disambiguates which reviewer's objection a number refers to, so a resolve that
    // names one would otherwise be dispatched as a recording and refused for lacking an
    // answer flag. Found by reading the dispatch after documenting the flag.
    if let Some(n) = resolving {
        return panel_resolve(args, root, artifact, &n);
    }
    if let Some(agent) = from {
        return panel_record(args, root, artifact, &agent);
    }
    panel_open(args, root, artifact, all, top, json)
}

/// Opens a round, or proposes a panel when none was named.
///
/// **A proposed panel is never seated.** The router elects an owner from a question, and a
/// reviewer is not an owner: an agent whose subject the piece stakes a claim on without
/// using its vocabulary scores zero here and would be silently left off. So the ranking is
/// printed with every agent's edge beside it and `--reviewer` is still required, which is
/// the same division `kb misses` draws between proposing an alias and writing one.
fn panel_open(
    args: &[String],
    root: &Path,
    artifact: Option<&str>,
    all: bool,
    top: usize,
    json: bool,
) -> ExitCode {
    let Some(artifact) = artifact else {
        eprintln!("kb: panel needs the artifact under review, as a path or a name.");
        eprintln!("    `kb panel --ledger` lists the rounds that are already open.");
        return ExitCode::from(2);
    };
    let Some(owner) = flag_value(args, "--owner") else {
        eprintln!("kb: panel needs --owner <agent>: the one agent accountable for this piece.");
        eprintln!("    A panel with no owner is a committee, and a committee cannot own an arc.");
        return ExitCode::from(2);
    };

    let memory = match memory::Memory::open(&[root], all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(1);
        }
    };
    let roster = memory.roster();
    if !roster.iter().any(|r| r.eq_ignore_ascii_case(&owner)) {
        eprintln!("kb: no routable agent named `{owner}`. The fleet: {}", roster.join(", "));
        return ExitCode::from(2);
    }

    // Read once, here, so the round is priced on the piece rather than on the boot alone.
    let body = std::fs::read_to_string(artifact).ok();
    let artifact_bytes = body.as_ref().map(|b| b.len()).unwrap_or(0);

    let reviewers = flag_values(args, "--reviewer");
    if reviewers.is_empty() {
        return panel_propose(&memory, artifact, body.as_deref(), &owner, top);
    }

    let out_dir = flag_value(args, "--out")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join(".kb").join("panel"));

    // Checked before anything is assembled. `panel::open` refuses this too, and would
    // refuse it after three constitutions had already been written to disk for a round
    // that was never going to exist.
    if let Some(same) = reviewers.iter().find(|r| r.eq_ignore_ascii_case(&owner)) {
        eprintln!("kb: {}", panel::Error::OwnerOnPanel(same.to_lowercase()));
        return ExitCode::from(2);
    }

    let mut booted = Vec::new();
    for name in &reviewers {
        let Some(agent) = memory
            .agents
            .iter()
            .find(|a| a.routable && a.name.eq_ignore_ascii_case(name))
        else {
            eprintln!("kb: {}", panel::Error::NoSuchAgent(name.clone()));
            return ExitCode::from(2);
        };
        match panel::boot(&agent.name.to_lowercase(), &agent.root, &out_dir) {
            Ok(b) => booted.push(b),
            Err(e) => {
                eprintln!("kb: {e}");
                return ExitCode::from(2);
            }
        }
    }

    let seated = match panel::open(root, artifact, &owner, &reviewers, &misses::today()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("kb: {e}");
            return ExitCode::from(2);
        }
    };
    let cost = panel::cost(&booted, Path::new(artifact), body.as_deref().unwrap_or(""));
    let key = panel::key_of(artifact);

    if json {
        let mut v = json::Value::obj();
        v.set("artifact", key.as_str().into());
        v.set("owner", owner.to_lowercase().into());
        v.set("seated", json::Value::from(seated.clone()));
        v.set(
            "panel",
            json::Value::Arr(
                booted
                    .iter()
                    .map(|b| {
                        let mut o = json::Value::obj();
                        o.set("agent", b.agent.as_str().into());
                        o.set("constitution", b.path.display().to_string().into());
                        o.set("tokens", b.tokens.into());
                        o
                    })
                    .collect(),
            ),
        );
        let mut c = json::Value::obj();
        c.set("boot", cost.boot.into());
        c.set("reading", cost.reading.into());
        c.set("total", cost.total().into());
        c.set("reviewers", cost.reviewers.into());
        c.set("artifact_tokens", blocks::tokens_of_artifact(Path::new(artifact), body.as_deref().unwrap_or("")).into());
        v.set("cost", c);
        v.set("ask", panel_ask(artifact).into());
        v.set("log", panel::path_in(root).display().to_string().into());
        println!("{}", v.to_string());
        return ExitCode::SUCCESS;
    }

    println!("round open on {key}, owned by {}", owner.to_lowercase());
    if seated.len() < reviewers.len() {
        println!(
            "  ({} already seated, so nothing was added for them)",
            reviewers.len() - seated.len()
        );
    }
    println!();
    println!("  {:<12} {:>7}   {}", "reviewer", "~boot", "constitution");
    for b in &booted {
        println!("  {:<12} {:>7}   {}", b.agent, b.tokens, b.path.display());
    }
    println!();
    println!("  boot            {:>7}", cost.boot);
    match artifact_bytes {
        0 => println!(
            "  reading         {:>7}   {artifact} is not a readable file, so nothing was priced for it",
            0
        ),
        _ => println!(
            "  reading         {:>7}   {} tokens of artifact, read once by each of {}",
            cost.reading,
            blocks::tokens_of_artifact(Path::new(artifact), body.as_deref().unwrap_or("")),
            cost.reviewers
        ),
    }
    println!("  {:-<24}", "");
    println!(
        "  total           {:>7} tokens, before a word of the review is written",
        cost.total()
    );
    println!();
    println!("  One owner pays that once per revision cycle. A round table pays it again on");
    println!("  every exchange, and convergence has no bounded number of exchanges, so the");
    println!("  cost is unbounded in exactly the case where the disagreement is real. That is");
    println!("  why this is an owner with named objections and not a table.");
    println!();
    println!("Boot one subagent per reviewer, and never two constitutions in one context:");
    println!("two identities in one context is one model averaging them.");
    println!();
    println!("Hand each subagent its own file, and ask it exactly this:");
    println!();
    for line in panel_ask(artifact).lines() {
        println!("  {line}");
    }
    println!();
    println!("Record every answer, the silences included:");
    println!();
    println!("  kb panel {key} --from <agent> --objection \"<text>\" [--blocking]");
    println!("  kb panel {key} --from <agent> --nothing");
    println!("  kb panel {key} --from <agent> --not-returned --why \"<what was decided instead>\"");
    println!();
    println!("Account for every objection, then read the round back:");
    println!();
    println!("  kb panel {key} --resolve <n> --taken --why \"<text>\"");
    println!("  kb panel {key} --resolve <n> --refused --why \"<text>\"");
    println!("  kb panel {key} --resolve <n> --escalated --why \"<text>\"   (blocking only)");
    println!("  kb panel {key} --ledger");
    ExitCode::SUCCESS
}

/// The instruction a reviewer is booted with, in one place.
///
/// It is printed for a person, handed to a subagent, and carried in `--json`, and those
/// three drifting apart is how a panel ends up with one reviewer that was asked for
/// objections and another that was asked for feedback.
fn panel_ask(artifact: &str) -> String {
    format!(
        "Read <constitution> in full and answer as that agent for as long as this task holds.\n\
         Then read {artifact}.\n\
         Return the single strongest thing wrong with it from inside your own domain, or one\n\
         line saying you found nothing. \"I like it\" is not an objection and neither is praise\n\
         with a caveat attached. Do not rewrite it: a suggested line is allowed only as an\n\
         illustration of an objection, never as a patch, because two writers on one sentence is\n\
         how a voice dies. An objection from outside your own domain is advisory and has to say\n\
         so. You may mark at most one objection blocking, and only when both hold: it sits\n\
         inside your own domain, and it names something falsifiable."
    )
}

/// Who the artifact's own words reach, with every agent's edge beside it.
///
/// **The ranking is a suggestion and the reason it can be wrong is printed with it.** It
/// scores the piece against each base's keys, so it finds the agents the piece talks like
/// and misses the agent whose subject the piece stakes a claim on in somebody else's
/// vocabulary. The unranked remainder is printed for exactly that case, because a menu
/// that hides half the kitchen is worse than a long menu.
fn panel_propose(
    memory: &memory::Memory,
    artifact: &str,
    body: Option<&str>,
    owner: &str,
    top: usize,
) -> ExitCode {
    println!("nothing was opened. This proposes a panel; --reviewer seats one.");
    println!();
    println!("artifact: {artifact}");
    match body {
        Some(b) => println!(
            "          {} bytes, about {} tokens, paid once by every reviewer",
            b.len(),
            blocks::tokens_of_artifact(Path::new(artifact), b)
        ),
        None => println!("          not a readable file, so it could not be scored or priced"),
    }
    println!("owner:    {owner}");
    println!();

    let ranked: Vec<(String, f64)> = body
        .map(|b| memory.ask(b, top))
        .and_then(|a| a.agent)
        .map(|c| c.totals)
        .unwrap_or_default();

    let mut listed: Vec<String> = Vec::new();
    if !ranked.is_empty() {
        println!("  Ranked by what this artifact's own words reach:");
        println!();
        for (name, score) in ranked.iter() {
            if name.eq_ignore_ascii_case(owner) {
                continue;
            }
            panel_agent_line(memory, name, Some(*score));
            listed.push(name.to_lowercase());
        }
        println!();
    }

    let rest: Vec<String> = memory
        .roster()
        .into_iter()
        .filter(|n| !n.eq_ignore_ascii_case(owner) && !listed.iter().any(|l| l == n))
        .collect();
    if !rest.is_empty() {
        println!("  Everyone else, unranked:");
        println!();
        for name in &rest {
            panel_agent_line(memory, name, None);
        }
        println!();
    }

    println!("  The ranking scores the piece against each base's keys, so it finds the agents");
    println!("  this piece talks like. An agent whose subject the piece stakes a claim on in");
    println!("  somebody else's vocabulary scores zero here and is in the second list. Read the");
    println!("  edges: what none of them covers is the judgement a ranking cannot make.");
    println!();
    println!("  kb panel {artifact} --owner {owner} --reviewer <agent> --reviewer <agent>");
    ExitCode::SUCCESS
}

/// One agent on the proposal menu: name, what it owns, and where it stops.
///
/// The edge is not decoration here. On 2026-09-04 a session read a roster of bare names,
/// could not tell which agent owned written surfaces, and reported that the fleet had none.
/// A panel picked off a list of words has the same failure available to it.
fn panel_agent_line(memory: &memory::Memory, name: &str, score: Option<f64>) {
    let Some(agent) = memory.agents.iter().find(|a| a.name.eq_ignore_ascii_case(name)) else {
        return;
    };
    let card = kb::fleet::card(&agent.root, "agent.txt", &agent.name);
    let tokens = blocks::read(&agent.root)
        .map(|bs| {
            bs.iter()
                .filter(|b| b.mode == blocks::Mode::Resident)
                .map(|b| b.tokens())
                .sum::<usize>()
        })
        .unwrap_or(0);

    match score {
        Some(s) => println!("    {:<12} {:>7} tokens   score {s:.2}", name.to_lowercase(), tokens),
        None => println!("    {:<12} {:>7} tokens", name.to_lowercase(), tokens),
    }
    if tokens == 0 {
        println!("      cannot be booted as itself: no blocks.txt in {}", agent.root.display());
    }
    if let Some(role) = card.role {
        println!("      owns:     {}", first_sentence(&role, 100));
    }
    if let Some(ends) = card.ends {
        println!("      stops at: {}", first_sentence(&ends, 150));
    }
}

/// The opening sentence of a mandate, capped, so one wordy agent cannot push the rest off
/// the screen. Cuts on a word boundary because a role sliced mid-word reads as corruption.
fn first_sentence(text: &str, cap: usize) -> String {
    let text = text.trim();
    if let Some(end) = text.find(". ") {
        if end < cap {
            return text[..end].to_string();
        }
    }
    if text.len() <= cap {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|i| *i <= cap)
        .last()
        .unwrap_or(0);
    match text[..end].rfind(char::is_whitespace) {
        Some(cut) => format!("{}...", text[..cut].trim_end()),
        None => text.to_string(),
    }
}

fn panel_record(args: &[String], root: &Path, artifact: Option<&str>, agent: &str) -> ExitCode {
    let Some(artifact) = artifact else {
        eprintln!("kb: panel needs the artifact the answer is about.");
        return ExitCode::from(2);
    };
    let nothing = args.iter().any(|a| a == "--nothing");
    let not_returned = args.iter().any(|a| a == "--not-returned");
    let objection = flag_value(args, "--objection");
    let why = flag_value(args, "--why").unwrap_or_default();

    // **Three flags and not one with three values, because two of them are the pair that
    // must never merge.** A reviewer that found nothing and a reviewer that never answered
    // are different facts, and the protocol stalled once on exactly that collapse.
    let answer = match (objection, nothing, not_returned) {
        (Some(text), false, false) => panel::Answer::Objection {
            text,
            blocking: args.iter().any(|a| a == "--blocking"),
        },
        (None, true, false) => panel::Answer::Nothing,
        (None, false, true) => panel::Answer::NotReturned { why: why.clone() },
        _ => {
            eprintln!(
                "kb: --from needs exactly one of --objection <text>, --nothing or --not-returned."
            );
            eprintln!("    --nothing means the reviewer read it and found nothing from inside its own");
            eprintln!("    domain. --not-returned means it never answered. Those are different facts");
            eprintln!("    and there is deliberately no flag that means either.");
            return ExitCode::from(2);
        }
    };

    match panel::record(root, artifact, agent, &answer, &misses::today()) {
        Ok(Some(n)) => {
            println!("objection {n} from {agent} on {}", panel::key_of(artifact));
            println!("  account for it with --resolve {n}. The round does not close until you do.");
            ExitCode::SUCCESS
        }
        Ok(None) => {
            match answer {
                panel::Answer::Nothing => println!(
                    "{agent} found nothing on {}, and that is on the record too: a reviewer who \
                     never objects is a reviewer who is not being read.",
                    panel::key_of(artifact)
                ),
                _ => println!(
                    "{agent} did not return on {}. Recorded as `not returned`, never as `no \
                     objection`.",
                    panel::key_of(artifact)
                ),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            ExitCode::from(2)
        }
    }
}

fn panel_resolve(args: &[String], root: &Path, artifact: Option<&str>, n: &str) -> ExitCode {
    let Some(artifact) = artifact else {
        eprintln!("kb: panel needs the artifact the objection is about.");
        return ExitCode::from(2);
    };
    let Ok(seq) = n.parse::<u32>() else {
        eprintln!(
            "kb: --resolve takes an objection number. `kb panel {artifact} --ledger` numbers them."
        );
        return ExitCode::from(2);
    };
    let taken = args.iter().any(|a| a == "--taken");
    let refused = args.iter().any(|a| a == "--refused");
    let escalated = args.iter().any(|a| a == "--escalated");
    let outcome = match (taken, refused, escalated) {
        (true, false, false) => panel::Outcome::Taken,
        (false, true, false) => panel::Outcome::Refused,
        (false, false, true) => panel::Outcome::Escalated,
        _ => {
            eprintln!("kb: --resolve needs exactly one of --taken, --refused or --escalated.");
            return ExitCode::from(2);
        }
    };
    let why = flag_value(args, "--why").unwrap_or_default();
    if why.trim().is_empty() && outcome == panel::Outcome::Refused {
        eprintln!("kb: a refusal needs --why. The refusal is the price of single ownership and it");
        eprintln!("    is paid in writing: a refusal with no reason cannot be audited after the");
        eprintln!("    piece underperforms, which is the only thing that makes refusing legitimate.");
        return ExitCode::from(2);
    }

    match panel::resolve(
        root,
        artifact,
        seq,
        flag_value(args, "--from").as_deref(),
        outcome,
        &why,
        &misses::today(),
    ) {
        Ok(row) => {
            println!("objection {seq} from {}: {}", row.reviewer, row.state.as_str());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kb: {e}");
            ExitCode::from(2)
        }
    }
}

/// The round read back: the table that travels with the piece, and what is still open.
///
/// **Exit 1 while a round is open**, so a build step or a hook can ask whether a piece has
/// been through its round without parsing prose. A ledger that always exits 0 is a ledger
/// nothing can gate on.
fn panel_ledger(root: &Path, artifact: Option<&str>, json: bool) -> ExitCode {
    let log = panel::path_in(root);
    let rows = panel::load(&log);

    let Some(artifact) = artifact else {
        println!("log: {}", log.display());
        let all = panel::artifacts(&rows);
        if all.is_empty() {
            println!();
            println!("  no round has been opened here.");
            return ExitCode::SUCCESS;
        }
        println!();
        for a in &all {
            let l = panel::ledger(&rows, a);
            println!(
                "  {:<40} {}  ({} objections, {} still out)",
                a,
                if l.closed() { "closed" } else { "OPEN  " },
                l.objections.len(),
                l.silent().len()
            );
        }
        return ExitCode::SUCCESS;
    };

    let l = panel::ledger(&rows, artifact);
    if !l.exists() {
        eprintln!("kb: {}", panel::Error::NoRound(panel::key_of(artifact)));
        return ExitCode::from(2);
    }

    if json {
        let mut v = json::Value::obj();
        v.set("artifact", l.artifact.as_str().into());
        v.set("owner", l.owner.as_str().into());
        v.set("closed", l.closed().into());
        v.set(
            "objections",
            json::Value::Arr(
                l.objections
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let mut o = json::Value::obj();
                        o.set("n", (i + 1).into());
                        o.set("reviewer", r.reviewer.as_str().into());
                        o.set("blocking", r.blocking.into());
                        o.set("state", r.state.as_str().into());
                        o.set("text", r.text.as_str().into());
                        o.set("why", r.why.as_str().into());
                        o
                    })
                    .collect(),
            ),
        );
        v.set(
            "still_out",
            json::Value::from(l.silent().iter().map(|r| r.reviewer.clone()).collect::<Vec<_>>()),
        );
        v.set("markdown", l.to_markdown().into());
        println!("{}", v.to_string());
        return match l.closed() {
            true => ExitCode::SUCCESS,
            false => ExitCode::from(1),
        };
    }

    println!("{}  owned by {}", l.artifact, l.owner);
    println!();
    print!("{}", l.to_markdown());
    println!();

    if l.closed() {
        println!("  Closed. Every objection is accounted for and nobody is still out.");
        println!("  The table above is part of the deliverable: a piece that arrives without its");
        println!("  ledger has not been through the round, whatever was said in chat.");
        return ExitCode::SUCCESS;
    }

    println!("  OPEN.");
    for (n, r) in l.blocking_open() {
        println!(
            "    objection {n} from {} is blocking and has not gone to the person yet.",
            r.reviewer
        );
        println!("      It cannot be refused. --taken, or --escalated.");
    }
    for (n, r) in l.unaccounted() {
        if r.blocking {
            continue;
        }
        println!("    objection {n} from {} is unaccounted for.", r.reviewer);
    }
    for r in l.silent() {
        println!("    {} was booted and has not answered.", r.reviewer);
        println!("      --not-returned when the window closes, never --nothing.");
    }
    ExitCode::from(1)
}

/// One logged question and what nearly caught it, on a terminal.
///
/// Split out of [`cmd_misses`] so the loop stays one line, the same shape
/// [`print_suggestions`] and [`format_group`] have. Prints and decides nothing.
fn print_miss(m: &misses::Miss, near: &[memory::NearMiss], floor: f32) {
    println!();
    let dates = if m.first == m.last {
        m.first.clone()
    } else {
        format!("{} to {}", m.first, m.last)
    };
    println!("  {:>3}x  {dates}  {}", m.count, m.question);

    if !m.looked_like.is_empty() {
        println!("        looked like: {}", m.looked_like.join(", "));
    }

    if near.is_empty() {
        // **Says what was observed and stops.** This line used to conclude "this is
        // coverage, not keys", and that does not follow: an empty near-miss list means
        // the text scorer ranked nothing either, which is true both of a base that does
        // not cover the subject and of a base that covers it in words this question did
        // not use. Nothing here can tell those apart, so nothing here claims to. Run it
        // against `examples/demo` and the old sentence prints for `quem aprova um
        // deploi`, a typo of a question the base answers.
        println!("        no file was ranked by either scorer, so there is nothing to point at.");
        return;
    }

    println!("        under this corpus floor of {floor:.2} today:");
    for n in near {
        let title = if n.title.is_empty() { String::new() } else { format!("  {}", n.title) };
        println!(
            "          {}/{}  keyword {:.2}  {}{title}",
            n.base,
            n.rel,
            n.keyword_score,
            n.why.join(" + ")
        );
        println!("            {}", keys_line(&n.keys));
    }
}

/// How many of a file's keys the terminal shows before it gives up and counts.
///
/// **Measured on the real fleet, not chosen.** The first run of this verb against
/// `fleet/` printed one note's 70 keys as a single unwrapped line, three times in one
/// screen, and the file path and score they were attached to scrolled out of reach. The
/// keys are here so a reader can see what the file thinks it is about; a dump is
/// something they have to read past. Same size and same reasoning as
/// [`memory::Memory::SUGGEST_LIMIT`], kept separate because that one bounds what the
/// contract returns and this one bounds what a terminal prints.
const KEYS_SHOWN: usize = 8;

/// The keys a near miss carries, shortened for a terminal and honest about it.
///
/// Pure, so the cap can be tested without capturing stdout, which is the only reason it
/// is not three lines inside [`print_miss`].
///
/// **The count is exact and only the list is short**, the rule
/// [`memory::Memory::PATHS_SHOWN`] states: a truncated list with nothing saying it was
/// truncated understates the problem by exactly as much as it was truncated. `kb misses
/// --json` caps nothing, because a program can hold seventy strings and a caller
/// comparing keys against a question needs all of them.
///
/// Empty is printed rather than skipped, because empty is the finding: the index holds
/// no entry for the file, so the text scorer reached something the keyword scorer can
/// never see, and the work there is a `Search for:` line rather than an alias.
fn keys_line(keys: &[String]) -> String {
    if keys.is_empty() {
        return "keys: none. The index holds no entry for this file.".to_string();
    }
    let shown = keys.iter().take(KEYS_SHOWN).cloned().collect::<Vec<_>>().join(", ");
    if keys.len() > KEYS_SHOWN {
        format!("keys: {shown}, and {} more", keys.len() - KEYS_SHOWN)
    } else {
        format!("keys: {shown}")
    }
}

/// The body of `kb misses --json`, as a value rather than as a line of stdout.
///
/// Split from the command for the reason [`route_payload`] is: a function that prints
/// can be read by a person and by nothing else, so the shape an integrator parses would
/// have no test under it.
///
/// `exists` is separate from an empty `misses` array on purpose. A log that was never
/// written and a log somebody has emptied by acting on it are the same array and two
/// different facts, and only one of them means the recording is wired up at all.
fn misses_payload(
    log: &Path,
    lost: &[misses::Miss],
    memory: &memory::Memory,
    top: usize,
) -> json::Value {
    let mut out = json::Value::obj();
    out.set("log", log.display().to_string().into());
    out.set("exists", log.exists().into());
    out.set("floor", score(memory.floor() as f64));

    let mut indexed = json::Value::obj();
    indexed.set("entries", memory.entry_count().into());
    indexed.set("agents", memory.agents.len().into());
    out.set("indexed", indexed);

    // Distinct questions and total asks are two sizes of one problem: how much there is
    // to fix, and what leaving it unfixed is costing.
    out.set("total_questions", lost.len().into());
    out.set("total_asks", lost.iter().map(|m| m.count as usize).sum::<usize>().into());

    out.set(
        "misses",
        json::Value::Arr(
            lost.iter()
                .map(|m| {
                    let mut one = json::Value::obj();
                    one.set("count", (m.count as usize).into());
                    one.set("first", m.first.as_str().into());
                    one.set("last", m.last.as_str().into());
                    one.set("question", m.question.as_str().into());
                    one.set(
                        "looked_like",
                        json::Value::Arr(
                            m.looked_like.iter().map(|w| w.as_str().into()).collect(),
                        ),
                    );
                    one.set(
                        "near",
                        json::Value::Arr(
                            memory
                                .near_misses(&m.question, top)
                                .iter()
                                .map(near_as_json)
                                .collect(),
                        ),
                    );
                    one
                })
                .collect(),
        ),
    );

    out
}

/// One candidate the gate refused, and the keys it carries instead.
///
/// `path` rather than `rel`, so a caller reading this beside `route --json` finds the
/// field where the other payload put it. The score goes through [`score`] for the same
/// reason every other one does: an `f32` widened to `f64` prints precision it never had.
fn near_as_json(n: &memory::NearMiss) -> json::Value {
    let mut out = json::Value::obj();
    out.set("base", n.base.as_str().into());
    out.set("path", n.rel.as_str().into());
    out.set("title", n.title.as_str().into());
    out.set("keyword_score", score(n.keyword_score as f64));
    out.set("why", json::Value::Arr(n.why.iter().map(|w| w.as_str().into()).collect()));
    out.set("keys", json::Value::Arr(n.keys.iter().map(|k| k.as_str().into()).collect()));
    out
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
        //
        // **The empty base guard was missing here alone.** `cmd_route` and both MCP tools
        // ask `is_empty` first; this path did not, so a fresh `kb init` asking its first
        // question was told its phrasing did not match keyword lines that do not exist.
        // That is the exact sentence measured wrong on 2026-08-17 and fixed on every
        // other surface, still printing here because this refusal was written separately
        // from the one it copies.
        if !print_if_nothing_to_search(&memory) {
            println!("nothing in the library matched. Either it does not cover this, or the");
            println!("Search for lines do not carry the words the question used.");
            print_shortfall(&memory, &a.confidence);
            if !looked_like.is_empty() {
                println!("  it does know: {}", looked_like.join(", "));
            }
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
                a.confidence.floor,
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

/// `kb promote`: the deposit becomes knowledge, or it does not and says why.
///
/// The whole design is in `promote.rs`. What lives here is the reporting, and it reports
/// refusals as loudly as writes: a promotion run whose output is only what it wrote is a
/// run that looks successful when it accepted everything.
fn cmd_promote(
    paths: &[&str],
    all: bool,
    top: usize,
    dry_run: bool,
    max: Option<usize>,
    lock: bool,
) -> ExitCode {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let mut memory = match memory::Memory::open(&given, all) {
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
    let outcome =
        promote::run(&mut memory, root, &promoter, &reviewer, top, dry_run, &today, max, None);

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

/// `kb boot`: who answers this message, decided before the model sees it.
///
/// **Two input shapes, and the flags are how a host with no hook says the same things.**
/// `boot::parse_request` reads either a prompt-hook envelope or the message alone;
/// `--session` and `--cwd` carry what an envelope would have carried, and `--text` refuses
/// the envelope reading for a caller that knows it is handing over raw text. The flags win
/// over the payload, the same order `kb capture` uses, so one adapter can pass both without
/// having to know which one the other end will believe.
fn cmd_boot(
    paths: &[&str],
    all: bool,
    top: usize,
    session: Option<&str>,
    cwd: Option<&str>,
    text_only: bool,
) -> ExitCode {
    use std::io::Read;

    let mut stdin = String::new();
    if std::io::stdin().read_to_string(&mut stdin).is_err() {
        return ExitCode::SUCCESS;
    }
    let parse = if text_only { boot::parse_text } else { boot::parse_request };
    let Some(mut req) = parse(&stdin) else {
        return ExitCode::SUCCESS;
    };
    if let Some(s) = session.map(str::trim).filter(|s| !s.is_empty()) {
        req.session = Some(s.to_string());
    }
    if let Some(c) = cwd.map(str::trim).filter(|c| !c.is_empty()) {
        req.cwd = Some(PathBuf::from(c));
    }

    // **The path is positional first, and the working directory is the fallback.** Every
    // caller that has a config file names the fleet there, so nothing that works today
    // changes. What this adds is the caller that has no config file at all: `kb boot
    // --cwd "$PWD"` is a complete invocation, and the envelope's own `cwd` field stops
    // being a value this command parsed and never read.
    let fallback = req.cwd.clone();
    let given: Vec<&Path> = match (paths.is_empty(), &fallback) {
        (true, Some(c)) => vec![c.as_path()],
        (true, None) => vec![Path::new(".")],
        _ => paths.iter().map(Path::new).collect(),
    };
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
            match boot::parse_request(&stdin).and_then(|req| req.session) {
                Some(s) => s,
                None => {
                    eprintln!("kb capture: no session named. Pass --session, or the hook payload on stdin.");
                    return ExitCode::from(2);
                }
            }
        }
    };

    // **Every outcome is printed, because one session can leave more than one deposit.**
    // A conversation that changed subject passed through more than one agent, and each
    // agent's refusals go to that agent's inbox. Printing only the first would hide the
    // rest from the log this hook writes to, which is the only place anybody sees it.
    match capture::write_deposit(Path::new(root), &session, &kb::misses::today()) {
        Ok(outcomes) => {
            for outcome in &outcomes {
                match outcome {
                    capture::Outcome::Written(path) => {
                        println!("captured session {session} into {}", path.display())
                    }
                    capture::Outcome::Nothing(why) => {
                        println!("nothing captured for session {session}: {why}")
                    }
                }
            }
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
                memory.floor()
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
        "  {:<3} {:<10} {:<10} {:>5} {:>9} {:>9} {:>9} {:>11}",
        "#", "block", "mode", "files", "on disk", "sent", "~tokens", "cumulative"
    );

    let mut cumulative = 0usize;
    let mut trimmed = 0usize;
    for (i, b) in blocks.iter().enumerate() {
        let resident = b.mode == blocks::Mode::Resident;
        if resident {
            cumulative += b.tokens();
            trimmed += b.trimmed();
        }
        println!(
            "  {:<3} {:<10} {:<10} {:>5} {:>9} {:>9} {:>9} {:>11}",
            i + 1,
            b.name,
            if resident { "resident" } else { "on-demand" },
            b.files.len(),
            b.file_bytes,
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
    // **Two columns, because one number that changed meaning is the drift these reports
    // exist to catch.** `sent` is what the model is handed and what every cost here is
    // priced on; `on disk` is what the files measure. The gap is the `Search for:` lines,
    // which the router reads out of the index and the model never scores against.
    if trimmed > 0 {
        println!(
            "  of the resident files, {trimmed} bytes (about {} tokens) are `Search for:`",
            blocks::tokens(trimmed)
        );
        println!("  lines, kept on disk for `kb check` and stripped out of the prompt.");
    }
    println!();
    println!("  worst case cost of changing a block, in tokens prefilled again:");
    for (name, cost) in blocks::invalidation_cost(&blocks) {
        println!("    {name:<10} {cost:>7}");
    }
    println!();
    println!("  A change invalidates its own block and everything after it, so the");
    println!("  first block is the most expensive to touch. That is why the order is");
    println!("  by how often a block changes, most stable first.");
    println!();
    println!("  Worst case because a prefix cache invalidates from the first byte that");
    println!("  differs, not from the start of the block that holds it. Measured over");
    println!("  the fleet's fifteen maps on 2026-09-08, a new entry lands at the end of");
    println!("  its section with about 48% of the map behind it, so a note written today");
    println!("  costs roughly half the map row above and not all of it.");

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

/// The name printed beside a base's numbers, which has to be the name stamped on its
/// entries. It was the same six lines as `index::build`'s copy until one of them moved.
fn label(base: &Base) -> String {
    index::base_name(&base.root)
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
                captured_from: None,
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

    /// The premise of `kb list`, pinned where a caller can actually observe it.
    ///
    /// There is no ranking question, so there is no number to argue with and nothing to
    /// branch on. `route_payload` carries `score`, `gate`, `verdict`, `keyword_score`
    /// and `matched` because a caller has to decide whether to trust a ranking; a
    /// listing answers a filter, and a score on it would be a number computed against
    /// no question at all. The facets are asserted present in the same test, because a
    /// payload that carries nothing also carries no score.
    #[test]
    fn a_listing_payload_carries_no_score_no_gate_and_no_verdict() {
        let (_, memory) = indexed_fleet("listing", &one_note_the_keyword_scorer_can_reach());
        let rows = memory.list(&list::Filter::default()).expect("lists");
        let out = list::to_json(&rows);

        let items = match &out {
            json::Value::Arr(a) => a.clone(),
            other => panic!("a listing is an array of files: {other:?}"),
        };
        assert!(!items.is_empty(), "the fixture has files: {}", out.to_string());

        for row in &items {
            for absent in ["score", "gate", "verdict", "keyword_score", "matched", "why"] {
                assert!(
                    row.get(absent).is_none(),
                    "{absent} is a ranking answer and nothing here was ranked: {}",
                    out.to_string()
                );
            }
            for present in ["base", "path", "kind", "folder", "layer", "private"] {
                assert!(
                    row.get(present).is_some(),
                    "{present} left the payload: {}",
                    out.to_string()
                );
            }
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
            "unreachable",
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
        // A key every note carries is worth `6 × ln(1 + 4/5)`, 3.5, under this size's
        // floor of 4.1: the one shape that is a guess at four entries. One unique key
        // used to be the fixture and it stopped being a guess when the floor learned to
        // scale (ADR-0036), which is the change working rather than the test breaking.
        let mut notes = one_note_the_keyword_scorer_can_reach();
        for (_, text) in notes.iter_mut() {
            *text = text.replacen("**Search for:** ", "**Search for:** `everywhere`, ", 1);
        }
        let (_, memory) = indexed_fleet("guess", &notes);
        let out = route_payload("everywhere", &memory, 4);

        assert_eq!(text_of(&out, "verdict"), "guess", "{}", out.to_string());
        assert!(flag_of(&out, ["gate", "served"]), "{}", out.to_string());
        assert!(!flag_of(&out, ["gate", "ranked_by_text_only"]), "{}", out.to_string());
    }

    /// The floor travels so a caller can disagree with the gate without guessing what
    /// it was measured against. Asserted through `Memory::floor` rather than through
    /// a literal: the floor scales with the corpus since ADR-0036, so on this four note
    /// fleet it is well under the calibration constant, and the payload has to carry
    /// the number that actually applied.
    #[test]
    fn the_floor_in_the_payload_is_the_one_the_verdict_was_measured_against() {
        let (_, memory) = indexed_fleet("floor", &one_note_the_keyword_scorer_can_reach());
        assert!(memory.floor() < memory::SCORE_FLOOR, "a four entry fleet gets a lower floor");

        for question in ["como faco rollback de um deploy sem downtime na release", "downtime", "xyzzy"] {
            let out = route_payload(question, &memory, 4);
            assert_eq!(
                num_of(&out, ["gate", "floor"]),
                ((memory.floor() as f64) * 1e6).round() / 1e6,
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
    /// **Found by running it, not by asserting on it.** The shortfall sentences are the
    /// only block on this terminal built at runtime rather than typed as a literal, so
    /// they were the only block nobody had wrapped, and the first real run printed one 230
    /// characters wide. A test on the string is happy either way, which is why this one is
    /// about the shape and not the words.
    #[test]
    fn a_runtime_sentence_folds_like_the_literals_around_it() {
        let long = "The library holds 1 entry across 1 agent in all, and under 2 entries \
                    nothing can be a hit whatever it scores.";
        let rows = wrapped(long, SHORTFALL_WIDTH);

        assert!(rows.len() > 1, "a 130 character sentence is more than one row: {rows:?}");
        for row in &rows {
            assert!(row.chars().count() <= SHORTFALL_WIDTH, "over the width: {row:?}");
        }
        assert_eq!(
            rows.join(" ").split_whitespace().collect::<Vec<_>>(),
            long.split_whitespace().collect::<Vec<_>>(),
            "every word survives, in order and unsplit"
        );

        // A word longer than the width overruns rather than being cut in two: half a path
        // is a string nobody can search for or paste.
        let path = "probe/knowledge/a-very-long-file-name-that-exceeds-the-width-on-its-own.md";
        assert_eq!(wrapped(path, 20), vec![path.to_string()]);
    }

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

    fn arr_len(v: &json::Value, path: [&str; 2]) -> usize {
        match v.get(path[0]).and_then(|o| o.get(path[1])) {
            Some(json::Value::Arr(a)) => a.len(),
            other => panic!("{}.{} is not an array: {other:?}", path[0], path[1]),
        }
    }

    /// **A base that holds files it cannot reach says so to a caller, not only to whoever
    /// runs `kb check`.**
    ///
    /// The two counts sit in one object because they are two answers to one question, what
    /// is in this base that I cannot reach, and they call for opposite work: `files` is an
    /// authoring problem and `unindexed` is a `kb index` that never ran.
    #[test]
    fn the_route_payload_says_what_the_base_cannot_reach() {
        let mut notes: Vec<(&str, String)> = one_note_the_keyword_scorer_can_reach();
        notes.push(("knowledge/orphan.md", "# Orphan\n\nno keys at all.\n".into()));
        let (_, memory) = indexed_fleet("unreachable", &notes);

        let out = route_payload("como faco rollback de um deploy sem downtime", &memory, 4);
        let u = out.get("unreachable").expect("unreachable left the payload");
        for field in ["files", "paths", "unindexed", "unindexed_paths"] {
            assert!(u.get(field).is_some(), "unreachable.{field} is missing: {}", out.to_string());
        }

        assert_eq!(num_of(&out, ["unreachable", "files"]), memory.unreachable().len() as f64);
        assert_eq!(num_of(&out, ["unreachable", "files"]), 1.0, "{}", out.to_string());
        assert_eq!(arr_len(&out, ["unreachable", "paths"]), 1);
        assert_eq!(
            num_of(&out, ["unreachable", "unindexed"]),
            memory.unindexed().len() as f64
        );
        assert_eq!(
            num_of(&out, ["unreachable", "unindexed"]),
            0.0,
            "the fixture syncs before it opens, so nothing is lagging"
        );
    }

    /// A capped array beside a capped count is a payload that lies about the size of the
    /// problem. The count is always exact; only the list of paths is shortened.
    #[test]
    fn the_count_stays_exact_when_the_path_list_is_capped() {
        let names: Vec<String> =
            (0..12).map(|i| format!("knowledge/orphan-{i}.md")).collect();
        let mut notes: Vec<(&str, String)> = one_note_the_keyword_scorer_can_reach();
        for name in &names {
            notes.push((name.as_str(), "# Orphan\n\nno keys at all.\n".into()));
        }
        let (_, memory) = indexed_fleet("capped", &notes);

        let out = route_payload("como faco rollback de um deploy sem downtime", &memory, 4);
        assert_eq!(num_of(&out, ["unreachable", "files"]), 12.0, "{}", out.to_string());
        assert_eq!(
            arr_len(&out, ["unreachable", "paths"]),
            memory::Memory::PATHS_SHOWN,
            "the list is capped"
        );
    }
    // -----------------------------------------------------------------------
    // `kb misses`, the reader over the recall loss log
    // -----------------------------------------------------------------------

    /// **The reason the verb exists, end to end.** F-02: the base holds the answer
    /// and only its keys are wrong.
    ///
    /// The striped note's keys, title and purpose are about an animal and its body is
    /// about rollbacks, so "rollback sem downtime" is refused by the gate, recorded as
    /// a recall loss, and still reached by the text scorer. What the log cannot hold is
    /// which file that was, because that depends on the base as it stands today rather
    /// than on the question that was lost.
    ///
    /// A `keyword_score` of 0.0 beside a `text #N` is the whole finding: the words are
    /// in the file and not on its `Search for:` line, so the work is an alias line or
    /// another key, not a new note. The keys come back so the reader can see which
    /// words are there instead.
    #[test]
    fn the_file_whose_body_matched_is_named_with_the_keys_that_did_not() {
        let (root, memory) = indexed_fleet("misses-verb", &one_note_only_the_text_scorer_can_reach());

        let refused = route_payload("rollback sem downtime", &memory, 4);
        assert_eq!(text_of(&refused, "verdict"), "nothing", "{}", refused.to_string());

        let log = kb::misses::path_in(&root);
        let lost = misses::load(&log).expect("the log the refusal just wrote");
        let out = misses_payload(&log, &lost, &memory, 4);

        let questions = match out.get("misses") {
            Some(json::Value::Arr(a)) => a.clone(),
            other => panic!("misses is not an array: {other:?}"),
        };
        assert_eq!(questions.len(), 1, "{}", out.to_string());
        assert_eq!(text_of(&questions[0], "question"), "rollback sem downtime");

        let near = match questions[0].get("near") {
            Some(json::Value::Arr(a)) => a.clone(),
            other => panic!("near is not an array: {other:?}"),
        };
        assert!(!near.is_empty(), "the text scorer reached the note: {}", out.to_string());
        assert_eq!(text_of(&near[0], "path"), "knowledge/striped.md", "{}", out.to_string());
        assert_eq!(
            near[0].get("keyword_score"),
            Some(&json::Value::Num(0.0)),
            "the keyword scorer never saw it: {}",
            out.to_string()
        );
        match near[0].get("why") {
            Some(json::Value::Arr(w)) => assert!(
                w.iter().any(|s| matches!(s, json::Value::Str(t) if t.starts_with("text #"))),
                "only the text scorer voted: {}",
                out.to_string()
            ),
            other => panic!("why is not an array: {other:?}"),
        }
        match near[0].get("keys") {
            Some(json::Value::Arr(k)) => assert_eq!(
                k.clone(),
                vec![
                    json::Value::Str("zebra".into()),
                    json::Value::Str("quagga".into())
                ],
                "the line the reader edits: {}",
                out.to_string()
            ),
            other => panic!("keys is not an array: {other:?}"),
        }
    }

    /// Nothing lost is exit 0 and a named file, not an error and not silence.
    ///
    /// The path matters more than it looks: `KB_MISSES_PATH` can move the log
    /// anywhere, so a reader that reports nothing without saying where it looked is
    /// unfalsifiable, and two fleets pointed at one path share one log.
    #[test]
    fn a_fleet_that_has_missed_nothing_exits_zero_and_says_which_file_it_looked_in() {
        let (root, memory) = indexed_fleet("misses-empty", &one_note_the_keyword_scorer_can_reach());
        let log = kb::misses::path_in(&root);
        assert!(!log.exists(), "nothing has missed against this fleet");

        let out = misses_payload(&log, &[], &memory, 4);
        assert_eq!(text_of(&out, "log"), log.display().to_string(), "{}", out.to_string());
        assert_eq!(out.get("exists"), Some(&json::Value::Bool(false)), "{}", out.to_string());
        assert_eq!(len_of(&out, "misses"), 0, "{}", out.to_string());

        let path = root.to_str().expect("a utf-8 scratch path");
        assert_eq!(
            format!("{:?}", cmd_misses(&[path], true, 4, true, false, None)),
            format!("{:?}", ExitCode::SUCCESS),
            "a healthy fleet is not a failure"
        );
    }

    /// **Found by running it, not by the tests above.** On the real fleet one near miss
    /// carried 70 keys and printed them as a single line wider than any terminal, three
    /// times over, which buries the one thing the verb exists to show.
    ///
    /// The rule is `Memory::PATHS_SHOWN`'s: the count stays exact and only the list is
    /// shortened, because a short list with no count beside it understates the problem by
    /// exactly as much as it was shortened. The JSON is not capped: a program can hold
    /// seventy strings, and a caller diffing keys against a question needs all of them.
    #[test]
    fn a_terminal_line_shows_a_shortlist_of_keys_and_says_how_many_it_left_out() {
        let many: Vec<String> = (0..70).map(|i| format!("k{i}")).collect();
        let line = keys_line(&many);
        assert!(line.contains("k0"), "{line}");
        assert!(
            !line.contains(&format!("k{}", KEYS_SHOWN)),
            "the list stops at the cap: {line}"
        );
        assert!(
            line.contains(&format!("{} more", 70 - KEYS_SHOWN)),
            "the count is exact even though the list is not: {line}"
        );

        let few: Vec<String> = vec!["zebra".into(), "quagga".into()];
        let short = keys_line(&few);
        assert!(short.contains("zebra, quagga"), "{short}");
        assert!(!short.contains("more"), "nothing was left out: {short}");

        // Empty is a finding and not a blank: no index entry means the keyword scorer
        // cannot see the file at all, so the work is a `Search for:` line.
        assert!(keys_line(&[]).contains("none"), "{}", keys_line(&[]));

        // And the payload a program reads keeps every one of them.
        let near = memory::NearMiss {
            base: "probe".into(),
            rel: "knowledge/striped.md".into(),
            title: String::new(),
            keyword_score: 0.0,
            why: vec!["text #1".into()],
            keys: many.clone(),
        };
        match near_as_json(&near).get("keys") {
            Some(json::Value::Arr(k)) => assert_eq!(k.len(), 70, "the JSON is not capped"),
            other => panic!("keys is not an array: {other:?}"),
        }
    }

    /// The line a deployed copy answers with, and the two things it must not lose.
    ///
    /// The version alone is not enough. Two builds of `0.2.1` can differ by every commit
    /// made between the bump and the tag, and the copy that matters is usually vendored
    /// into another repository where nobody can run `git log`. So the commit rides along,
    /// and the fallback is the word `unknown` rather than a guess: a local build that
    /// claimed a commit it did not come from would be worse than one that admits it.
    #[test]
    fn the_version_line_names_the_version_and_the_build() {
        let line = version_line();
        assert!(
            line.contains(env!("CARGO_PKG_VERSION")),
            "the line does not carry the crate version: {line}"
        );
        assert!(line.starts_with("kb "), "the line does not name the binary: {line}");
        // Unset in a local build, set by the release workflow. Either way the field is
        // occupied, because an empty parenthesis is a line a parser cannot read.
        let build = option_env!("KB_BUILD_SHA").unwrap_or("unknown");
        assert!(line.contains(build), "the build is missing from: {line}");
        assert!(USAGE.contains("kb --version"), "the verb is not in the usage text");
    }

    /// The step that used to be missing, and the condition that replaced its absence.
    ///
    /// This test asserted the opposite until 2026-09-03: that `--apply` did not exist and
    /// that the reason travelled with its absence. **It is kept and re-pointed rather than
    /// deleted**, because the thing worth guarding never was the absence of the flag. It
    /// was that the decision could not be removed by accident.
    ///
    /// What the flag may not lose: the gold set. `--apply` without `--gold` is a writer with
    /// nothing to measure against, and every argument for letting a machine write here rests
    /// on the measurement. So the requirement is asserted in the text a person reads, and
    /// the reason is asserted beside it.
    #[test]
    fn applying_an_alias_cannot_lose_the_gold_set_that_makes_it_safe() {
        assert!(
            USAGE.contains("[--apply --gold <tsv>]"),
            "the flag is never offered without the set that gates it:
{USAGE}"
        );
        assert!(VALUE_FLAGS.contains(&"--gold"), "and --gold takes a value, so it is parsed");
        assert!(
            USAGE.contains("the whole safety property rather than an argument"),
            "the reason travels with the requirement:
{USAGE}"
        );
        assert!(
            USAGE.contains("A model proposes the lines and never decides one"),
            "and so does the division of labour that makes it auditable:
{USAGE}"
        );
    }

    /// The asymmetry the gate exists for, pinned where a reader of the CLI will find it.
    ///
    /// An alias cannot cause a miss, only a confident hit on the wrong thing, and the miss
    /// log is the only feedback kept. Anybody reading `--apply` and reaching for a simpler
    /// design needs that sentence in front of them, so it is asserted rather than trusted
    /// to survive an edit.
    #[test]
    fn the_help_says_why_recall_is_not_the_only_column_that_matters() {
        assert!(
            USAGE.contains("can never cause a miss, only a confident hit on something else"),
            "the mechanism travels with the feature:
{USAGE}"
        );
    }
}
