# Ulpia

[![ci](https://github.com/richard-wollyce/ulpia/actions/workflows/ci.yml/badge.svg)](https://github.com/richard-wollyce/ulpia/actions/workflows/ci.yml)

**A fleet of agents, each with its own knowledge base, and one memory layer under all
of them. Everything on your machine.**

Ulpia is the library. **Vesta is its librarian**: the orchestrator that knows who is
in the fleet and routes what arrives. The name is the Bibliotheca Ulpia, Trajan's
library, which was Rome's public reading room and its official record office in one
building, and that pair is exactly what this software is.

Vesta answers *which of your files a question should open*, across every agent you
have, in a few tens of milliseconds, without sending anything anywhere and without a
model in the loop. Five runs of `kb eval` on an
otherwise idle machine, over a base of 113 entries (the fleet had grown to the 120
the transcripts below print by the time they were captured), put both scorers together between 22
and 33 ms per question, and the keyword scorer alone between 16 and 24. Runs taken while a
build shared the machine roughly doubled that, which is the whole reason this gives a
range and no headline figure: a single number here measures how busy the laptop was, and
so does an average taken across runs that were not competing for the same cores.

Then you hand those files to whatever model you like. Yours, locally. Claude, through
MCP. Something else next year. The memory outlives the model.

---

## Why this exists

Most memory layers for agents put an embedding model in the retrieval path. That buys
semantic matching and costs you four things: you cannot run it offline, you cannot
explain a bad result, you cannot reproduce yesterday's answer, and your notes have been
somewhere you did not choose.

Vesta takes the other trade. **Retrieval is plain software.** Same question, same
answer, forever, and when it is wrong it tells you why in words you can act on.

```
$ kb route "posso dar git push sozinho ou preciso perguntar" .

question: posso dar git push sozinho ou preciso perguntar
indexed:  120 entries across 10 agents, 216 aliases

   1.  72.62  zed/protocols/limits-and-autonomy.md
      matched: git push, sozinho
   2.  47.96  tullius/index.md
      matched: git push
```

The question is Portuguese over an English README on purpose: the router matches
the keys each file declares, in whatever languages they were written, so a fleet
answers in every language its authors used. The Italian miss further down is the
same demonstration from the failing side.

That is one scorer, the keyword index built from your own map, which is what `kb route`
prints by default. There is a second, SQLite full text search over the content, and
`kb route --hybrid` runs both and fuses the two rankings with Reciprocal Rank Fusion.

**Each scorer does the job it was measured to be better at, and they are not
interchangeable.** Fusion rewards agreement, which is what you want when assembling
passages for a person to read: a file both scorers noticed belongs in front of them. It
is the wrong rule for picking a single winner, because a file each scorer ranks fourth
beats a file one scorer ranks first. Measured over the 24 answerable questions of this
fleet's 33 question answer key: the keyword scorer alone picks the right file 11 times,
the fusion it feeds picks it 8 times.

**Read that as a comparison, not as a score.** The answer key's own header records that
the keyword lines in the maps were tuned against these same questions on the day they
were graded, so the keyword column is flattered and 11/24 is not a clean benchmark of
anything. What survives the bias is the direction: the fusion, fed by that same
flattered scorer, still lands behind it when the job is to name one file.

So **the reading comes from agreement and the verdict comes from intent.** Vesta ranks
who should answer using the keywords each file declares in its own `Search for:`
header (a `MAP.md`, when you keep one, is a reading list for people; the router does
not consult it), and on the same 24
questions that deterministic fold names the right agent 22 times. What `kb boot`
actually hands over, with the classifier that sits in front of it, is the number that
counts and it is 21. The best a fixed choice could do on this set is 13. These figures
are re-measured before anything quotes them, because they have gone stale on this very
page twice; the eval takes seconds and there is no excuse. It also grades its own
confidence, and on this set it fails that grade:

```
$ kb eval fleet/zed/fleet/eval/gold.tsv .   # the doubled fleet/ is real: Zed keeps his eval set in a nested fleet

  GATE   flagged 1/13 of its own misses as a guess
         demoted 0/11 correct answers to a guess
         abstained on 4/9 question(s) the set says to decline
         hit scores  29.43 to 129.87
         miss scores 7.45 to 91.86
         OVERLAPS: no floor tells a hit from a miss on this set.
```

**`OVERLAPS` is the tool failing its own test in public, and that is why the block is
still here.** A miss reaches 91.86 while a hit starts at 29.43, so no single confidence floor
separates the two, and the gate flags only 1 of its 13 misses as a guess. What it does
not do is demote a correct answer, and it declines 4 of the 9 questions the key says to
decline. An earlier version of this page quoted a run that separated cleanly; that run
was a 19 question set that no longer exists.

That is `kb eval`, and it ships in the tool rather than in a benchmark harness, so these
are numbers re-run rather than numbers a harness produced once. **You cannot reproduce
these exact figures**, because the answer key lives in a private fleet this repository
gitignores; you point the same command at your own. The key behind the numbers above is
33 questions, 24 answerable and 9 the fleet is supposed to decline. `kb eval` grades
against it and **refuses to run if it points at files that have moved**, which is how
the last stale measurement was caught.

**And when nothing matches, the base answers with its own vocabulary.**

```
$ kb route "come funziona il registro delle decisioni" .

question: come funziona il registro delle decisioni
indexed:  120 entries across 10 agents, 216 aliases

  nothing matched. Either the base does not cover it, or the
  Search for lines do not carry the words a real question uses.

  the base does know these, and they look like words you used:
    apagar registro, registro de decisao, registro de marca, registro de release, registro de sessao, registro de treino, registro imutavel, registro longitudinal
  that is spelling and not meaning, so it finds a typo or a cognate
  and never finds a translation.
```

A miss that offers nothing back teaches you to stop asking. That comparison is
character trigram overlap, so it reaches a typo and a cognate across languages and
it **never** reaches a translation, which is why the reply says which kind of help
it is. The other half of the problem belongs to whatever model is reading, and this
is the candidate space it works from instead of guessing.

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
each with a `.sha256` beside it:

```
gh release download kb-v0.1.0 -R richard-wollyce/ulpia -p "kb-linux-x64*"
sha256sum -c kb-linux-x64.sha256
```

The Linux one is `x86_64-unknown-linux-musl`, statically linked, so it depends on nothing
outside the file and runs on Alpine, Debian and the Amazon Linux images serverless
functions use. That claim is not taken on trust: the release workflow executes the
artifact inside `amazonlinux:2023` and `alpine:3` before publishing anything.

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

`kb answer` reads five files by default, a table sized for a personal question. Two
more modes exist and are the caller's choice, never an automatic switch: `--expanded`
reads up to twelve files, and `--complete` reads every keyed file in the base in batches of
ten and composes the answer from what each batch extracted. Complete mode prints a
time estimate before the first model call and restates it after the first batch
lands, because a whole-base read costs minutes, and a caller on any surface deserves
that number before paying it.

That eval is the reproducible half of every number on this page: 13 questions, 10 it
should answer and 3 it should refuse, graded in front of you. Run today on the demo:
file 10/10, agent fold 10/10, all 3 refusals refused, and the confidence floor
separates every hit from every miss on this set. Your screen will also show
`routes 8/10` and two hits demoted to guesses: that is the stricter
classifier-included layer declining two low scorers, printed so the flattering
line never travels without the harsher one beside it. The private-fleet numbers further up
are the same command pointed at a fleet you cannot see, which is the product working.

Then create your own first agent:

```
kb init yaron
```

That writes the full agent shape, initialises git inside the agent, and makes the
first commit, so the agent it creates can be opened by the system that created it.
When a git repository already covers that path and does not ignore it, no repository is
created: the files are staged into the one that is there and the first commit is yours to
write. The tool says which of the two happened.

Now write a note. **The order of the next four steps is the order, and getting it wrong
fails silently**, which is the one thing worth reading twice on this page:

```
1. write   fleet/yaron/knowledge/creatina.md, with a **Search for:** line at the top
2. commit  kb commit fleet/yaron/knowledge/creatina.md -m "the dose"
3. index   kb index .
4. ask     kb route "quanto de creatina por dia" .
```

**Anything git does not track is not served**, because unknown is not public. That rule is
load bearing and it has no exception, so a note written and not committed is a note the
router will not open. Here is what skipping step 2 actually looks like, run in a fresh
clone:

```
$ kb index .
  yaron      2 reindexed, 0 unchanged, 0 removed, 5 chunks
  total: 3 files, 5 chunks, 1 indexes

$ kb route "quanto de creatina por dia" .
indexed:  1 entries across 1 agents, 0 aliases

  nothing matched. Either the base does not cover it, or the
  Search for lines do not carry the words a real question uses.
```

**The index reports success and the miss blames your keywords, and neither is what
happened.** The file is uncommitted, so it was never eligible. The tell is `1 entries`
where you expected 2. Commit it and the same question scores 17.97 and lands first.

Step 3 has to come after step 2 for the same reason, and this half is quieter still: the
keyword scorer re-reads your files on every run, so committing alone brings it back
immediately. **The full text index does not**, because whether a file was public is
recorded in it at index time. Commit after indexing and `--hybrid` silently drops to one
scorer and reports the result as a weak guess with no passage attached. Re-run `kb index`
and both scorers agree again.

While you are still drafting and nothing is committed yet, `--all` on any command includes
what git does not track. It is the right tool for a draft and the wrong one for a habit:
it turns the privacy filter off.

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
against one five file base, confidence floor 17.5:

```
"quanto custa o frete"                             hit     20.68   matched: quanto custa o frete, frete
"o frete e gratis a partir de quanto"              hit     19.24   matched: frete gratis, frete, gratis
"quanto tempo demora pra cair o estorno no cartao" guess    6.19   matched: tempo, estorno
```

The third note is not worse written. Its keys are `reembolso, estorno, devolucao do
dinheiro`, which name the subject correctly and match no phrase anybody types.

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

## Beside the neighbours

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

The honest column note: the neighbours automate ingestion at scales Ulpia refuses on
purpose, and each of them is a good tool for the job it names. The row that is ours
alone is the refusal: a librarian who can say "no one here owns this" instead of
handing you the least wrong book.

---

## Benchmarks a clone can re-run

Four instruments live in [`benchmarks/`](benchmarks/), each results file carrying
the exact command, commit, machine and date, because a number that cannot say where
it came from is marketing. The harness is `tools/bench`, a second crate in this
repository; its `--trace` flag writes every intermediate of a complete-mode run to
disk, which is what made the autopsy below possible at all.

| instrument | headline | the footnote that keeps it honest |
|---|---|---|
| [abstention](benchmarks/abstention/RESULTS.md) | 28 of 30 out-of-scope questions not answered confidently, deterministic layer alone | the 50 questions were authored blind and adversarially checked; the two misses are named medical baits, and the answer layer above caught both |
| [latency](benchmarks/latency/RESULTS.md) | warm route p50 0.68 ms, p95 1.16 ms, the whole deterministic pipeline in process | the vendors' own published figures (0.148 to 0.3 s) measure their servers under their harnesses; the table quotes each claim with its URL and compares mechanisms, not machines |
| [LongMemEval-S](benchmarks/longmemeval/RESULTS.md) | 500 questions: 61 percent with the reading mode declared per question nature, 28 of 30 abstentions correct, under the weakest honest ingestion; 49 percent under the all-default first run, kept published |
| [LongMemEval-V2](benchmarks/longmemeval-v2/README.md) | pre-reader so far: the served context holds the full gold evidence for 88 percent (enterprise) and 81 percent (web) of deterministic questions at under a second per query | not a score: the official run needs the protocol's fixed reader and judge, and neither key lives in the repository | judged locally by claude-haiku, which is not the official protocol; the hypotheses file ships for official GPT-4o re-judging, and every number is a floor and labelled as one |

The multi-session story inside the third file is the method on display: 18 percent
under the five-file default, a traced autopsy that overturned the working theory
(91 percent of the failures were extraction, not composition), three fixes shipped
as product decisions in ADR-0032, a re-measure on the identical questions from
8/30 to 17/30, and the full 121 confirming it at 81/121 (67 percent). Changing the product to chase a benchmark is the tuning these
instruments exist to refuse, so every change lands as a product decision first and
gets measured after.

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
alone**, which is the step a person skips by hand. This output is a real commit,
made the day this page was last reviewed, and the hash is in this repository's
history:

```
committed 88622d0
  benchmarks/README.md
  benchmarks/abstention/RESULTS.md
  benchmarks/latency/RESULTS.md
  benchmarks/longmemeval/RESULTS.md
  decisions/0033-the-text-scorer-prunes-what-cannot-rank.md
  site/frontend/privacy/index.html
  site/frontend/terms/index.html
  site/frontend/tools/build-posts.mjs

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

## Privacy is a property of the layout, not a promise

Vesta reads git to know what is private. **A file git does not track is a file Vesta will
not serve** unless you ask for it explicitly, and where git cannot be consulted at all,
unknown is not public: that base is left out and none of its files are served.

```
$ kb route "how do I roll back" /somewhere/outside/any/repo

kb: left out /somewhere/outside/any/repo/zed: git could not be consulted there, so its
files could be private and none are being served. A husk left by a rename in progress
looks exactly like this, and so does a base nobody has committed yet.
```

**It leaves that base out and keeps the rest of the fleet open**, rather than failing the
whole open, and that is a deliberate change from an earlier version of this page: one
directory whose git could not be read used to close every base beside it, which happened
twice in one day for the ordinary reason that a rename in progress leaves a husk behind
and a husk looks exactly like a base. The guarantee is per base and skipping keeps it.

The cost of that choice is that **the reply is an empty result set and the exit code is
zero**, which reads exactly like a library that does not cover your question. The notice
goes to stderr, and `kb route --json` names the left out bases in a `skipped` array for
callers that only read stdout. If you are automating anything against this, check that
field before you conclude the base is thin.

`fleet/` is a separate repository and is gitignored by this one, so `git add -A` here
cannot descend into it. Publishing a note is not a mistake you are able to make.

---

## What an agent is

Browse [`agent-skeleton/`](agent-skeleton/) to see the exact shape, or run `kb init`.
They cannot disagree: the skeleton in this repository is generated by `kb init` and a
test fails if the two ever differ.

```
fleet/<name>/
  CLAUDE.md         who the agent is
  index.md          its operating instructions
  MAP.md            what exists in its base, and the words a question would use
  agent.txt         name and role, read by the orchestrator
  blocks.txt        the constitution, ordered by how often each block changes
  kb-aliases.txt    a record of real questions that missed
  knowledge/        the distillations. The brain
  inbox/            raw material awaiting distillation
  decisions/  protocols/  templates/
```

**A `[[wikilink]]` stops at the edge of its base.** It resolves inside that base and
nowhere else, and both the linter and the reading room enforce the same rule. To point at
another agent's file, write the path. The reason is privacy rather than tidiness: a base is
a privacy boundary, and a link that silently crossed one is how a reference to private
material lands in a file you meant to publish. Writing the path out means you knew.

`MAP.md` is doing more work than it looks. Every entry carries a `Search for:` line with
the words a real question would use, and that line is what the keyword scorer matches.
An entry without one is an entry nothing can reach.

## And it must know who it works for

A fleet has agents, and it has exactly one person, and **the person is not an agent**:

```
kb init --person
```

That writes `fleet/person/` with no `agent.txt`, which is the whole trick. The router
reads the base and **can never elect it as the one who answers**, because a question about
you belongs to the librarian, not to a specialist impersonating you. Browse
[`person-skeleton/`](person-skeleton/) for the exact shape.

Every agent `kb init` creates carries a `[user]` block pointing at `../person/core.md`,
so **an agent cannot be born not knowing who it works for.** That is not hypothetical: this
fleet ran with two agents that had no such block, and the marketing one answered a question
about its owner's CV without knowing his name.

One file is the truth, and residency is selective: the small core is resident everywhere,
the domain files are retrieved when a question calls for them. Fill them. An empty profile
is not a neutral state, it is an agent giving generic answers confidently.

**The shape is public and the content is not.** `person-skeleton/` and the generator ship
here; what you write into your own `fleet/person/` lives in your fleet repository, which
this one gitignores. That is the same split the agents already make, applied to you.

---

`kb-aliases.txt` is a record of misses, not a dictionary. Add a line **only after a real
question failed to find something.** Expansion is additive, so a wrong line can add
noise and can never remove signal. It is also how a fleet answers questions in one
language over a base written in another.

---

## Use it from Claude, or anything else that speaks MCP

```
kb serve .
```

Four read-only tools over stdio: `kb_route`, `kb_retrieve`, `kb_remember`, `kb_fleet`.
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

- **A deployment bundle has no `.git`, so by default nothing is served.** This is the
  failure above wearing different clothes, and it fails only in production: the same
  command works on your machine, where git answers. Pass `--all` over a base you built to
  be entirely publishable, and read `skipped` in the JSON.
- **A read-only filesystem is survivable.** The index is built before the deploy and only
  read at runtime. The one file a query writes is the miss log, and failing to write it
  prints on stderr without failing the query.
- **Every process start pays the cold open**: 136 ms to open the fleet and answer the
  first question, then a warm p50 of 0.68 ms, measured on one laptop. Spawning the binary
  per request pays it every time; a long lived `kb serve` pays it once.
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
