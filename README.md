# Ulpia

**A fleet of agents, each with its own knowledge base, and one memory layer under all
of them. Everything on your machine.**

Ulpia is the library. **Vesta is its librarian**: the orchestrator that knows who is
in the fleet and routes what arrives. The name is the Bibliotheca Ulpia, Trajan's
library, which was Rome's public reading room and its official record office in one
building, and that pair is exactly what this software is.

Vesta answers *which of your files a question should open*, across every agent you
have, in about ten milliseconds, without sending anything anywhere and without a model
in the loop.

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
$ kb route "quantas calorias posso comer hoje" .

  0.032  yaron    protocols/checkin.md          keywords #2 + text #5
  0.016  yaron    calculations/formulas.md      text #1
```

Two independent scorers rank every file: a keyword index built from your own map, and
SQLite full text search over the content. Their results are fused with Reciprocal Rank
Fusion.

**Each scorer does the job it was measured to be better at, and they are not
interchangeable.** Fusion rewards agreement, which is what you want when assembling
passages for a person to read: a file both scorers noticed belongs in front of them. It
is the wrong rule for picking a single winner, because a file each scorer ranks fourth
beats a file one scorer ranks first. Measured over 19 questions: the keyword scorer
alone picks the right file 18 times, the fusion it feeds picks it 11 times.

So **the reading comes from agreement and the verdict comes from intent.** Vesta ranks
who should answer using the keywords you wrote in your own map, and tells you when it is
guessing:

```
$ kb eval fleet/zed/fleet/eval/gold.tsv .

  GATE   flagged 1/1 of its own misses as a guess
         demoted 0/18 correct answers to a guess
         hit scores 9.29 to 179.24, miss scores 0.00
         SEPARATES: every hit outscored every miss.
```

That is `kb eval`, and it ships in the tool rather than in a benchmark harness, so the
numbers on this page are ones you can re-run. It grades against your own answer key and
**refuses to run if the answer key points at files that have moved**, which is how the
last stale measurement was caught.

**And when nothing matches, the base answers with its own vocabulary.**

```
$ kb route "o que e um protocolo de ingestao" .

  nothing matched. Either the base does not cover it, or the
  Search for lines do not carry the words a real question uses.

  the base does know these, and they look like words you used:
    ingest a source
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

Requires Rust. One dependency, no build scripts, no network at runtime.

```
git clone <this repo> && cd ulpia
cargo build --release --manifest-path tools/kb/Cargo.toml
```

Create your first agent:

```
kb init yaron
```

That writes the full agent shape, initialises git, and makes the first commit, so the
agent it creates can be opened by the system that created it. Drop markdown into
`fleet/yaron/knowledge/`, list it in `MAP.md`, then:

```
kb index .                      build one index per agent
kb route "your question" .      which files should this open
kb check .                      lint every agent
kb fleet .                      who is in the fleet
kb eval gold.tsv .              grade the routing against your own answer key
kb ui .                         the reading room: http://127.0.0.1:4114
```

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
kb commit decisions/0021.md tools/kb/src/commit.rs -m "message"
```

Name every path. **There is deliberately no flag meaning everything**, because that one
affordance is how a commit ends up carrying somebody else's half finished work under your
message. The damage there is not lost work, it is an audit trail that lies.

The mechanism: `git commit -- <paths>` builds the commit from only those paths and ignores
the rest of the index, so whatever another session staged a second ago cannot land in
yours. `kb commit` does that, then **reads the commit back off git and prints what it left
alone**, which is the step a person skips by hand:

```
committed 9fe6c10
  decisions/0021.md
  tools/kb/src/commit.rs

left untouched, still dirty (1):
  site/
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

Vesta reads git to know what is private. **A file git does not track is a file Vesta
will not serve** unless you ask for it explicitly, and if git cannot be consulted at all
it refuses to open the base rather than guessing:

```
refusing to open <path>: git could not be consulted, so there is no way to tell
which files are private.
```

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

`MAP.md` is doing more work than it looks. Every entry carries a `Search for:` line with
the words a real question would use, and that line is what the keyword scorer matches.
An entry without one is an entry nothing can reach.

## And it must know who it works for

A fleet has agents, and it has exactly one person, and **the person is not an agent**:

```
kb init --person
```

That writes `fleet/profile/` with no `agent.txt`, which is the whole trick. The router
reads the base and **can never elect it as the one who answers**, because a question about
you belongs to the librarian, not to a specialist impersonating you. Browse
[`person-skeleton/`](person-skeleton/) for the exact shape.

Every agent `kb init` creates carries a `[user]` block pointing at `../profile/core.md`,
so **an agent cannot be born not knowing who it works for.** That is not hypothetical: this
fleet ran with two agents that had no such block, and the marketing one answered a question
about its owner's CV without knowing his name.

One file is the truth, and residency is selective: the small core is resident everywhere,
the domain files are retrieved when a question calls for them. Fill them. An empty profile
is not a neutral state, it is an agent giving generic answers confidently.

**The shape is public and the content is not.** `person-skeleton/` and the generator ship
here; what you write into your own `fleet/profile/` lives in your fleet repository, which
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

## Status

Early, used daily, and honest about which is which.

| | |
|---|---|
| `tools/kb` | Works. 172 tests. One dependency. |
| `kb ui` | The reading room, set in the site's own type and palette: the fleet, the catalog, the stacks (shelves and book spines, ribbons where another agent works the document), the desk (chat routed by the same boot hook as every session), block budgets, doctor. One embedded page plus three Garamond faces, loopback only. |
| `tools/tray` | Windows only, and young. |
| `site` | The page at [ulpia.io](https://ulpia.io). Static front, one Rust binary behind it. |
| Local model routing | Not built. |
| Voice | Not built. |
| Licence | **Not chosen yet.** Until it is, this is source-available rather than open source. |

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
