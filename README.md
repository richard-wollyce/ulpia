# Ulpia

[![ci](https://github.com/richard-wollyce/ulpia/actions/workflows/ci.yml/badge.svg)](https://github.com/richard-wollyce/ulpia/actions/workflows/ci.yml)

Your coding agent answered out of the wrong file and sounded certain. It could not tell
you which file it used, could not give the same answer twice, and had no way to know the
difference between remembering and guessing.

**Ulpia is the memory it asks instead.** It indexes the markdown you already have, where
it already sits, and answers with the files to open and the words that matched under
each. Retrieval is plain software: no embedding model, no network, nothing in the path
that improvises. Same question, same answer, today and in a year, and when the answer is
wrong you can read why in words you can act on.

Because that confidence is measured rather than implied, Ulpia also knows when your
library does not hold the answer, and says so instead of handing over the closest file.
On [LongMemEval](benchmarks/longmemeval/RESULTS.md), a public benchmark of 500 instances,
it got that call right 29 times out of 30 (2026-08-24).

**The price is writing, and it is paid per note.** Each one carries a hand written
`Search for:` line, roughly thirty terms, naming the words a real question would use.
Nothing infers it for you. That is the trade: you spend a minute per note, and every
answer afterwards is one you can audit.

---

## What you would use it for

One binary, no daemon, no account, no service to sign up for. The verbs that earn their
place in an ordinary day:

- **Ask your own notes from the terminal.** `kb route "<question>" .` returns the files
  to open and the words that matched under each, in about a millisecond, with no model
  involved anywhere.
- **Let your coding agent ask instead of grepping.** `kb serve .` speaks MCP over stdio,
  so Claude Code, Claude Desktop or anything else that speaks the protocol queries the
  library directly rather than reading whichever file its own search surfaced.
- **Ask in the language you are thinking in.** The router matches the keys each note
  declares, in whatever language they were written, so a Portuguese question reaches an
  English note when its keys say it should.
- **Keep several specialists and get the right one.** `kb boot` scores a message across
  every base and hands the conversation to the one that owns the subject, so the
  architect does not end up answering a security question.
- **Get more than one specialist on the work that needs it.** When the router judges a
  message to land in a second agent's domain, it names the owner and the panel, and
  `kb panel` boots each reviewer from its own constitution, prices the round before it is
  spent, and keeps a ledger where every objection is taken, refused with a reason, or
  escalated. One agent stays accountable; nobody votes.
- **Get prose when a file list is not what you wanted.** `kb answer` puts a reader after
  the verdict, and the retrieval underneath it stays deterministic.
- **Write in a tree several sessions are editing at once.** `kb commit <paths>` builds the
  commit from your paths alone, then reads it back off git and prints what it left dirty.
- **Keep part of the library private by layout rather than by promise.** Each base
  declares its own private layer, and nothing inside it is served, indexed or suggested
  unless you ask for it with `--all`.
- **Grade your own retrieval.** `kb eval` runs your own answer key and refuses to run
  against a stale one; `kb check` names the notes nothing can reach.
- **Do all of it on a plane.** Nothing in the retrieval path touches a network, so the
  library works the same in a tunnel as it does at your desk.

---

## How it answers

```
$ kb route "posso dar git push sozinho ou preciso perguntar" .

question: posso dar git push sozinho ou preciso perguntar
indexed:  250 entries across 15 agents, 220 aliases

   1.  84.27  zed/protocols/limits-and-autonomy.md
      matched: git push, sozinho
   2.  55.25  tullius/index.md
      matched: git push
```

The question is Portuguese over an English README on purpose: the router matches
the keys each file declares, in whatever languages they were written, so a fleet
answers in every language its authors used. The miss below is the same demonstration
from the failing side. Both transcripts were re-run on 2026-09-04, because a pasted
transcript rots and this one already had.

**And when nothing matches, the base answers with its own vocabulary.**

```
$ kb route "qual e o padrao de qualidadi aqui" .   # qualidadi, misspelt on purpose

question: qual e o padrao de qualidadi aqui
indexed:  250 entries across 15 agents, 220 aliases

  nothing matched. Either the base does not cover it, or the
  Search for lines do not carry the words a real question uses.

  2 markdown files across the open bases carry no `Search for:` line, so
  the index holds no entry for them and they score zero on every question.
  `kb check` names them.
  No term in the question matched any key here, so nothing was ranked at
  all: this is a vocabulary miss and not a near miss, and asking again
  with the words the notes declare is what changes it.

  the base does know these, and they look like words you used:
    anti padrao de teste, capa padrao, desvio padrao, dia padrao, eixo do padrao, interrupcao de padrao, padrao de qualidade, padrao documentado do repositorio
  that is spelling and not meaning, so it finds a typo or a cognate
  and never finds a translation.
```

A miss that offers nothing back teaches you to stop asking. This one names the key it
should have found, `padrao de qualidade`, and says why it did not: the comparison is
character trigram overlap, so it reaches a typo and it reaches a cognate across
languages, and it **never** reaches a translation. That is why the reply says which kind
of help it is rather than just failing. The other half of the problem belongs to
whatever model is reading, and this is the candidate space it works from instead of
guessing.

---

## Beside the neighbours

Two different kinds of tool get used as a project's memory, and they lose to Ulpia for
two different reasons. Worth separating, because the swap is worth making in one case
and not in the other.

### A notes app, and Obsidian is the one people name

Obsidian keeps markdown on your disk and is good at what it is built for: graph view,
backlinks, canvas, sync, and a plugin ecosystem years deep. Ulpia has none of those, and
if your job is reading your own notes with your own eyes, keep Obsidian and stop here.

**The mechanism is who the organising is aimed at.** Every one of those features is drawn
for a person to look at, and none of them is a call an agent can make. The agent working
in your repository opens a shell, not a vault, so what it does with a folder of markdown
is grep it, and grep has no opinion about which file is the right one.

Ulpia organises the same files for the query instead of for the eye. The trade is exact:
you give up the graph, the canvas, the plugins and the sync, and you write a `Search for:`
line on every note you want found. What you get is a folder an agent can ask, in any
language its notes were written in, that answers with the files and the words that
matched, and that is honest about the edge of what it holds.

*Obsidian's feature list is read from its own documentation and is not something this
repository measured. The mechanism claim is narrower and checkable in an afternoon: point
an agent at a vault and watch what it does with the graph.*

### A vector memory layer

Read from each project's own repository and documentation on 2026-08-23; stars move,
mechanisms rarely do.

| | Ulpia | mem0 | Letta | Zep / Graphiti |
|---|---|---|---|---|
| Memory lives in | markdown files you edit | vector store + history DB | database rows | temporal knowledge graph |
| Who writes memory | you, or a gated two-model promotion that may refuse | an LLM, automatically | the agent, through tools | async extraction |
| Model in the retrieval path | none | embedder required | embedder for archival | embedder + graph |
| Can answer "nobody covers this" | yes, as a first-class verdict | no, ranking always returns a top hit | no | no |
| Runs fully offline | yes, retrieval has no network | possible with local stack | self-host possible | self-host of core |
| Licence | Apache-2.0 | Apache-2.0 | Apache-2.0 | Apache-2.0 |

**The trade underneath the table.** An embedding model in the retrieval path buys
semantic matching, and it costs four things: you cannot run it offline, you cannot
explain a bad result, you cannot reproduce yesterday's answer, and your notes have been
somewhere you did not choose. Ulpia takes the other side of that trade, and the
`Search for:` line is the bill.

The honest column note: the neighbours automate ingestion at scales Ulpia does not
attempt, and each of them is a good tool for the job it names. What is ours is the middle
of that table rather than the end of it. No model in the retrieval path is what makes one
answer explainable, reproducible and offline at the same time, and those three are the
rows you feel every day. The last row follows from them: a score you can trust is also a
score that can come up short, and Ulpia says so rather than handing you the least wrong
book.

---

## Quickstart

Requires Rust. One dependency, no build scripts of our own; that dependency compiles
bundled SQLite, so a fresh clone needs a C toolchain (MSVC Build Tools on Windows).
No network at runtime.

```
git clone https://github.com/richard-wollyce/ulpia && cd ulpia
cargo build --release --manifest-path tools/kb/Cargo.toml
```

The binary lands at `tools/kb/target/release/kb` (`kb.exe` on Windows); put it on
your PATH or call it by that path in everything below.

Building it yourself is the honest default for a tool that reads your notes, and it is not
the only way. Tagged releases carry a prebuilt `kb-linux-x64` and `kb-windows-x64.exe`,
each with a `.sha256` beside it. **There is no macOS build**: nobody here runs macOS, so a
published artifact would be one nothing has ever executed, which is worth less than its
absence. Build from source there.

```
gh release download -R richard-wollyce/ulpia -p "kb-linux-x64*"
sha256sum -c kb-linux-x64.sha256
```

**No tag in that command, deliberately**, so it resolves to the latest release and cannot
rot into pointing at an old binary while the prose describes a newer one. It already did
that once, between `kb-v0.1.0` and `kb-v0.2.0`. Name a tag only when you want a specific
version: `gh release download kb-v0.2.0 -R ...`.

The Linux artifact is `x86_64-unknown-linux-musl` and statically linked, so it runs on
Alpine, Debian and the Amazon Linux images serverless functions use. The release workflow
executes it inside `amazonlinux:2023` and `alpine:3` before publishing, so that is a test
rather than a claim.

Route your first question against the demo fleet that ships in the repository,
three tiny agents with an answer key, so the first run works before you have
written anything:

```
kb index examples/demo
kb route "quem decide se um deploy pode ir pra producao" examples/demo
kb eval examples/demo/gold.tsv examples/demo
kb answer "how much protein per meal" examples/demo
```

The last command needs a model: `answerer = ...` in the demo's `fleet.txt` points at a
command that reads a prompt on stdin and answers on stdout (the shipped one uses the
Claude CLI). The answer must ground every claim in the served passages and cite them,
and when the library does not hold the answer it says so instead of inventing one; the
model sits after retrieval's verdict, never inside retrieval.

`kb answer` reads five files by default. Two wider modes are the caller's choice and never
an automatic switch: `--expanded` reads up to twelve, and `--complete` reads every keyed
file in batches of ten. Complete mode prints a time estimate before the first model call
and restates it after the first batch, because a whole-base read costs minutes and nobody
should pay that without the number first.

That eval is the reproducible half of every number on this page: 13 questions, 10 it
should answer and 3 it should decline, graded in front of you. Run 2026-09-02 on the demo:
file 10/10, agent fold 10/10, routes 10/10, all 3 declines correct, and no right answer
demoted to a guess. It used to read `routes 8/10`, correctly at the time, until the
confidence floor stopped being one number and started scaling with the corpus
([ADR-0036](decisions/0036-the-floor-scales-with-the-corpus.md)).

Then create your own first agent:

```
kb init yaron
```

That writes the full agent shape and nothing else: no repository is created and none is
needed. The agent it creates is served by the system that created it, as it stands.

Now write a note. Three steps, and only one order between them matters:

```
1. write   fleet/yaron/knowledge/creatina.md, with a **Search for:** line at the top
2. index   kb index .
3. ask     kb route "quanto de creatina por dia" .
```

**A note is served the moment it is on disk.** The keyword scorer re-reads your files on
every run, so step 3 finds the new note with or without step 2. What step 2 buys is the
second scorer: the full text index holds the chunks, and a note written after the last
`kb index` has none, so `--hybrid` on it drops to one scorer and reports a weak guess with
no passage attached. Re-run `kb index` and both scorers agree again. That is the only
silent failure left in this flow, and it is the quiet kind: the answer is not wrong, it is
thinner than it should be.

`--all` on any command includes the private layer, `profile/`, `projects/` and `records/`
unless the base declares otherwise. It is the right flag for your own agents and the wrong
one for a consumer that other people talk to: it turns the privacy filter off.

```
kb index .                      build one index per agent
kb route "your question" .      your own fleet; the dot is the repo root, the demo uses examples/demo
kb route "your question" . --hybrid   fuse the keyword scorer with full text search
kb route "your question" . --json     the same answer as one line of JSON, for a program
kb check .                      lint every agent, including keys no question can reach
kb fleet .                      who is in the fleet
kb eval examples/demo/gold.tsv examples/demo    the graded demo above
kb ui .                         the reading room: http://127.0.0.1:4114
```

### Write the keys as questions, not as subjects

The `Search for:` line is the whole ranking, and the single most common way to end up with
a base that holds the answer and cannot find it is to fill that line with topic words.
Query words are weighted by inverse document frequency, and **a multi word key found whole
inside the question scores far above the same words counted separately.** Three questions
against one five file base, as they printed under the fixed floor of 17.5:

```
"quanto custa o frete"                             hit     20.68   matched: quanto custa o frete, frete
"o frete e gratis a partir de quanto"              hit     19.24   matched: frete gratis, frete, gratis
"quanto tempo demora pra cair o estorno no cartao" guess    6.19   matched: tempo, estorno
```

The third note is not worse written. Its keys are `reembolso, estorno, devolucao do
dinheiro`, which name the subject correctly and match no phrase anybody types. Under the
floor a five file base gets today, 4.6, that third line is a `hit` at 6.19 rather than a
guess: right file, a third of the score. That is arithmetic from `floor_for(5)` and not a
re-run, because that base is not in this repository. The point about phrases stands either
way, and it is the point.

`kb check` grades that line for you, and two of its warnings are the ones worth acting on
before anything else:

- **W06, thin keyword line.** Fires under twelve terms and asks for thirty, in both
  languages, including the words somebody types from inside the problem. The number comes
  from a real miss: a file keyed `eating out, restaurant, poke, salmon` scored zero on
  "hoje vou sair com meus amigos, to com azia, o que vou comer", and widened to thirty
  terms it answers at 130.21.
- **W07, unsearchable key.** Fires when a written key collapses to one word after
  stopwords, so it reaches neither index. The example in the message is the one that
  earned the check: `nao contestar` indexed as `contestar`, its own opposite, until
  somebody rewrote it as `proibido contestar`.

`kb write` builds the note and its map entry in one move and refuses to skip the keys. It
resolves an agent as `<fleet-root>/fleet/<name>` and fails outside that shape, which is
worth knowing before you tidy the directory: routing accepts a base at any path, writing
does not.

---

## The numbers, and the date on each one

Numbers here are secondary to the point of the tool, and they are load bearing
only with a date attached. Every figure below says when it was measured and what
measured it.

### What this fleet's own eval says today

Two scorers rank your files: the keyword index built from the `Search for:` lines, and
SQLite full text search over the content. `kb route --hybrid` fuses them with Reciprocal
Rank Fusion. They are not interchangeable, and
[the how-it-works page](https://ulpia.io/docs/how-it-works/) has the mechanics. The short
form: fusion rewards agreement, which is right when you are assembling passages for a
person to read and wrong when you are naming one winner, because a file each scorer ranks
fourth beats a file one scorer ranks first. Measured 2026-09-04 over the 40 answerable
questions of this fleet's 49 question key, keyword alone names the right file 26 times and
the fusion it feeds names it 14.

**Read that as a comparison, not as a score.** The key's own header records that these
keyword lines were tuned against these questions, so 26/40 is flattered and is not a clean
benchmark of anything. What survives the bias is the direction.

Routing is the number that actually ships. On the same 40 questions the deterministic fold
names the right agent 36 times, and `kb boot` with its classifier hands over 35. The best a
fixed choice could do is 24, so routing is worth eleven questions over picking one agent
and never moving.

**These figures go stale, and this page is the proof.** They have now been wrong here three
times, this one included, and the key grew from 33 questions to 49 between the second and
the third. Re-measuring takes seconds, so nothing quotes them without re-running first.

The same command grades its own confidence, and on this set it fails:

```
$ kb eval fleet/zed/fleet/eval/gold.tsv .   # the doubled fleet/ is real: Zed keeps his eval set in a nested fleet

  GATE   flagged 1/14 of its own misses as a guess
         demoted 0/26 correct answers to a guess
         of 9 question(s) the set says to decline: refused 3, hedged 0, answered 6
         hit scores  26.61 to 144.20
         miss scores 7.31 to 95.78
         OVERLAPS: no floor tells a hit from a miss on this set.
```

*Run 2026-09-04 against the release binary built from this commit.*

**`OVERLAPS` is the tool failing its own test in public, and that is why the block is
here.** A miss reaches 95.78 while a hit starts at 26.61, so no single floor separates
them. The gate never demotes a right answer, which is the column that protects your daily
use, and it gets only 3 of 9 declines right on this set, which is the open problem rather
than a footnote to one.

**That is a different gate from the 29 of 30 at the top of this page**, over a different
corpus: this one is the deterministic score threshold over a private fleet of prose, that
one is the abstention layer sitting above it, over LongMemEval's chat sessions. The layer
exists because the floor alone overlaps.

**You cannot reproduce these exact figures**, because the key lives in a private fleet this
repository gitignores. You point the same command at your own. `kb eval` refuses to run
against a key whose files have moved, which is how the last stale measurement was caught.

---

### Benchmarks a clone can re-run

Four instruments live in [`benchmarks/`](benchmarks/), each results file carrying
the exact command, commit, machine and date, because a number that cannot say where
it came from is marketing. The harness is `tools/bench`, a second crate in this
repository; its `--trace` flag writes every intermediate of a complete-mode run to
disk, which is what made the autopsy below possible at all.

| instrument | best result, and when | the footnote that keeps it honest |
|---|---|---|
| [abstention](benchmarks/abstention/RESULTS.md) | 28 of 30 out-of-scope questions not answered confidently, deterministic layer alone, 2026-08-23 | the 50 questions were authored blind and adversarially checked; the two misses are named medical baits, and the answer layer above caught both |
| [latency](benchmarks/latency/RESULTS.md) | warm route p50 1.16 ms, p95 2.16 ms, the whole deterministic pipeline in process, re-measured 2026-09-02 against the August code on the same quiet machine; cold open fell from 184.6 ms to 11.8 ms when git left the runtime | the vendors' own published figures (0.148 to 0.3 s) measure their servers under their harnesses; the table quotes each claim with its URL and compares mechanisms, not machines |
| [LongMemEval-S](benchmarks/longmemeval/RESULTS.md) | 500 questions, 2026-08-24: 29 of 30 abstentions correct and 51 of 56 on single-session-assistant in the first all-fast run; 61 percent overall when the reading mode is declared per question, which costs one abstention and lifts multi-session from 18 to 67 percent | both runs stay published because they trade against each other rather than one superseding the other; the ingestion is the weakest honest one, no retrieval tuned per question |
| [LongMemEval-V2](benchmarks/longmemeval-v2/README.md) | pre-reader so far: the served context holds the full gold evidence for 88 percent (enterprise) and 81 percent (web) of deterministic questions at under a second per query | not a score. The official run needs the protocol's fixed reader and judge and neither key lives here, so it was judged locally by claude-haiku; every number is a floor and labelled as one, and the hypotheses file ships for official re-judging |

The multi-session story inside the third file is the method on display, and it is the
reason the two runs both stay published: 18 percent
under the five-file default, a traced autopsy that overturned the working theory
(91 percent of the failures were extraction, not composition), three fixes shipped
as product decisions in ADR-0032, a re-measure on the identical questions from
8/30 to 17/30, and the full 121 confirming it at 81/121 (67 percent). Changing the
product to chase a benchmark is the tuning these instruments exist to refuse, so every
change lands as a product decision first and gets measured after.

---

## Your files are the source of truth

Markdown, in folders, in git. Nothing else is authoritative.

The index is **derived and disposable**: delete `.kb/` and you have lost a rebuild, not
a fact. That is not a slogan, it is what makes the whole thing portable. Move the
directory and everything moves with it, because **no absolute path exists anywhere
inside a fleet.** Backup, sync, and moving to a new machine are all the same operation.

---

## More than one agent writes this at once

Agents run in parallel, in the same working tree, all day. So committing is a verb the
tool owns:

```
kb commit benchmarks/README.md decisions/0033-the-text-scorer-prunes-what-cannot-rank.md -m "message"
```

Name every path. **There is deliberately no flag meaning everything**, because that one
affordance is how a commit ends up carrying somebody else's half finished work under your
message. The damage there is not lost work, it is an audit trail that lies.

The mechanism: `git commit -- <paths>` builds the commit from only those paths and ignores
the rest of the index, so whatever another session staged a second ago cannot land in
yours. `kb commit` does that, then **reads the commit back off git and prints what it left
alone**, which is the step a person skips by hand:

```
committed 88622d0
  benchmarks/README.md
  decisions/0033-the-text-scorer-prunes-what-cannot-rank.md
  ... 6 more

left untouched, still dirty (2):
  README.md
  .claude/launch.json
```

A tracked `pre-commit` hook refuses raw `git commit` so the safe path is the default one.
Enable it once per clone, because git will not let a repository set its own hook path from
tracked content, and that restriction is a security feature:

```
git config core.hooksPath .githooks
```

What this does **not** solve: two sessions editing the same file still clobber each other.
No git technique fixes that, because the race is at the filesystem before git sees
anything.

---

## Privacy, agents, and the person

Three things shape a fleet, and the reference for all three is
[the concepts page](https://ulpia.io/docs/concepts/). The short version:

**Privacy is a property of the layout, not a promise.** Each base declares its own private
layer as one line in `agent.txt`, defaulting to `profile/`, `projects/` and `records/`.
What that line covers is served, indexed and suggested only when the caller passes
`--all`. Git is not consulted and used to be; it left the runtime in
[ADR-0034](decisions/0034-git-leaves-the-runtime.md), because a folder should not have to
be committed before a memory layer will answer from it.

**An agent is a folder with a shape.** Run `kb init`, or browse
[`agent-skeleton/`](agent-skeleton/) for the exact one; a test fails if the two ever
differ. `knowledge/` holds the distillations, `MAP.md` names what exists and the words a
question would use, and `agent.txt` carries the name and role the orchestrator reads. A
`[[wikilink]]` stops at the edge of its base, because a base is a privacy boundary and a
link that silently crossed one is how private material lands in a file you meant to
publish. To point at another base, write the path out, so that crossing is deliberate.

**A fleet has exactly one person, and the person is not an agent.** `kb init --person`
writes `fleet/person/` with no `agent.txt`, the shape of which is in
[`person-skeleton/`](person-skeleton/), and that absence is the whole trick: the
router reads the base but can never elect it as the one who answers, because a question
about you belongs to the librarian rather than to a specialist impersonating you. Every
agent `kb init` creates carries a `[user]` block pointing at that core, so an agent cannot
be born not knowing who it works for. This fleet once ran two agents without it, and the
marketing one answered a question about its owner's CV without knowing his name. The shape
ships here; what you write into it lives in your own fleet repository, which this one
gitignores.

`kb-aliases.txt` is a record of misses rather than a dictionary. Add a line **only after a
real question failed to find something.** Expansion is additive, so a wrong line can add
noise and can never remove signal.

---

## Use it from Claude, or anything else that speaks MCP

```
kb serve .
```

Five read-only tools over stdio: `kb_route`, `kb_retrieve`, `kb_remember`, `kb_fleet` and
`kb_list`. The first four answer a question; `kb_list` answers what exists, filtered by
facet, with nothing ranked and no verdict, because a filter has no ranking problem to
solve. **None of them writes**, and that is a decision rather than an omission: a write
tool reached by a model is a different security surface and gets built deliberately.

The server speaks both the current stateless revision and the older handshake era, and
answers `initialize` when asked without ever requiring it.

For Claude Desktop, in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "vesta": {
      "command": "/path/to/kb",
      "args": ["serve", "/path/to/your/fleet"]
    }
  }
}
```

**There is deliberately no write tool.** `kb_remember` measures a claim against what the
base already says and proposes ADD, UPDATE or NOOP with its evidence. It writes nothing
and decides nothing. A write tool reachable by a model is a different security surface
and gets built deliberately, not as an afterthought while the retrieval side is still
warm.

---

## Running it somewhere that is not your machine

Local first is a design position, not a limitation waiting to be lifted: the index lives
beside the files, nothing talks to a server, and there is no hosted instance to point at.
The base has to be on the same filesystem as the binary. That works on a server as well as
on a laptop, and four things about it surprise people. The full version, with what each one
is measured on, is in [`tools/kb/README.md`](tools/kb/README.md). The short version:

- **A deployment bundle needs no `.git`.** The private layer is read off the base you
  shipped, so ship the base and the `.kb/` index built beside it, and pass `--all` only if
  the consumer is meant to see the private layer. It used to serve nothing without git,
  and the first deployment passed `--all` just to get past that, which is the wrong reason.
- **A read-only filesystem is survivable.** The index is built before the deploy and only
  read at runtime. The one file a query writes is the miss log, and failing to write it
  prints on stderr without failing the query.
- **Every process start pays the cold open, and the operating system decides what that
  costs.** Spawn, open and answer measured at p50 184.8 ms on a Windows laptop and at
  p50 9.6 ms on Linux, the second by the first team to deploy this and not reproduced
  here. On Linux the spawn is not worth engineering around; a long lived `kb serve` pays
  the open once and is for a loop, not for a request.
- **The libc has to match, or there has to be none.** Use the musl build above.

`kb route --json` is the surface for all of this. One line on stdout carrying the verdict,
the owner, the ranked files and the passages, from the same call `kb serve` makes, so no
two machine surfaces can drift into different opinions about one question.

---

## Status

Early, used daily, and honest about which is which.

| | |
|---|---|
| `tools/kb` | Works. `cargo test` in `tools/kb` is green, and the count is left out on purpose: it was published as 208 and was 212 by the time somebody checked and 217 an hour later, because more than one session writes this. One dependency. |
| `kb ui` | The reading room, set in the site's own type and palette: the fleet, the catalog, the stacks (shelves and book spines, ribbons where another agent works the document), the desk (chat routed by the same boot hook as every session), block budgets, doctor. One embedded page plus three Garamond faces, loopback only. |
| `tools/tray` | Windows only, and young. |
| `site` | The page at [ulpia.io](https://ulpia.io). Static front, one Rust binary behind it. |
| Local model routing | Not built. |
| Voice | Not built. |
| Licence | **Apache 2.0.** Use it, fork it, build on it; keep the notice and the attribution. The private layer under `fleet/` is not part of the repository and is not licensed, because it is not here. |

---

## Contributing

The house rules, which apply to issues and pull requests as much as to code:

- **Name the mechanism.** A change without the reason it works does not land.
- **Two options and their consequences**, or it is a preference, not a decision.
- **Mark what is unverified.** Ran it, read the source, read the docs, or guessing. Say
  which.
- **Never claim something works without running it.**

Run `cargo test` in `tools/kb` before opening anything. If you are changing what an
agent looks like, the skeleton test will tell you to regenerate `agent-skeleton/`, and
it is right.
