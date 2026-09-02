---
provenance: agent
stage: derived
---

# ADR-0034: git leaves the runtime, and a base declares its own private layer

**Search for:** `git`, `git ls-files`, `sem git`, `without git`, `git init`, `privacidade`, `privacy`, `camada privada`, `private layer`, `private =`, `agent.txt`, `manifesto`, `manifest`, `declaracao`, `declaration`, `desconhecido nao e publico`, `unknown is not public`, `tracked`, `rastreado`, `untracked`, `nao rastreado`, `--all`, `deploy sem .git`, `bundle`, `serverless`, `escala`, `scale`, `centenas de agentes`, `hundreds of agents`, `subprocesso`, `subprocess`, `nota invisivel`, `invisible note`, `mem0`, `letta`, `local first`, `backup`, `versionamento`, `versioning`, `kb commit`, `gitignore`, `ADR-0025`, `ADR-0021`, `ADR-0030`, `ADR-0034`

**Exists to:** Why `kb` stops asking git which files it may serve, what replaces the question, what that costs, and which uses of git survive as optional tools rather than as requirements.

- **Date:** 2026-09-01
- **Status:** accepted 2026-09-01, implemented the same day
- **Scope:** system. Binds every agent and every consumer of `kb`.
- **Deciders:** Richard, Zed
- **Supersedes:** the runtime half of [[0025-the-shape-is-public-the-person-is-not]]. The
  publication half of that record stands untouched: what of this repository may be published,
  and the skeletons that carry the shape.
- **Reversibility:** reversible as code. **Not reversible as a promise**: the README and the
  release notes say "git decides what is public", and consumers built on `--all` to get around
  it. Once the new rule ships, the old sentence has to be removed everywhere it was written.

## Context

`kb` today decides what it may serve by asking git. `Base::discover` runs `git ls-files` in
every base it opens; a file git tracks is public and served, a file git does not track is
private and held back, and a base where git cannot answer is refused unless the caller
passes `--all`. The rule was named "unknown is not public", and it was chosen for a real
reason: the thing that decides what `kb` serves was the same thing that decides what gets
published, so the two could never disagree.

That reason has stopped paying for what it costs. The costs, all of them met rather than
imagined:

1. **A folder is not a base until somebody runs `git init`.** A person who makes a directory,
   writes a note and asks a question is refused. For a memory layer, that is the first
   interaction, and it fails.
2. **A note is invisible until it is tracked.** `git ls-files` reads "not tracked" as "private",
   but a note written a minute ago is also not tracked. `kb write` grew a `git add` of its own
   to paper over this, which is a write tool calling a version control tool so that a router
   can see a file.
3. **A deployment bundle has no `.git`, so a deployment serves nothing.** The first
   integration of `kb` into a hosted function hit exactly this, and passed `--all` for the
   wrong reason: not to include a private layer, but to make anything work at all. The flag's
   meaning was lost on the first person who needed it.
4. **It does not scale.** The init is one per fleet, but the question is one subprocess per
   base per open: a fleet of a hundred agents shells out to git a hundred times to answer
   one question, before a single index is read.
5. **It protects nothing in daily use.** The boot hook runs `kb boot --all`. The promotion
   hook runs `kb promote --all`. Every surface the owner actually uses bypasses the gate,
   because the owner's own agents are supposed to see the owner's own layer. The gate bites
   only at the edges listed above, which are exactly the edges where it is wrong.

And the frame was wrong in a way Richard named directly: a memory layer for agents is used
as a database. Nobody asks mem0 or Letta to commit their memory to a repository before it
can be read. A local first system is one where the person keeps their own copies, by
whatever means they choose, and versioning is one of those means and not a precondition
for the system to answer.

## What git was actually doing, file by file

Listed because "remove git" is vague and the decision is not.

| Where | What it does | After this record |
|---|---|---|
| `base.rs`, `git ls-files` | The privacy oracle on every open | **Removed.** Replaced by the declaration below |
| `write.rs`, `git add` after a note is written | Makes the new note visible to the oracle | **Removed.** Nothing to make visible to |
| `init.rs`, `git init`, `check-ignore`, `rev-parse` | Makes a new base answerable by the oracle | **Removed as a requirement.** `kb init` still writes a `.gitignore` matching the declaration, so a person who later chooses git gets the right ignores for free. A courtesy, not a dependency |
| `store.rs`, the `tracked` column | Which files were public when indexed | **Renamed `private`**, filled from the declaration |
| `commit.rs`, `kb commit` | Committing under concurrency, ADR-0021 | **Stays, as an optional verb** for anyone who versions a fleet, and for developing this repository. It is never called by any serving path |
| The self-limiting rule of ADR-0030, "junk rate countable from `git diff`" | Unbuilt | Needs a measure that does not assume git. Promotion writes files at stage `captured`; count those and their later deletions. Named here so it is not forgotten, not decided here |

## Options

### A. Keep git as the oracle

The status quo. Cost: the five items above, permanently, and the first interaction with the
tool failing for anyone who did not read the README. Gain: the guarantee is by identity, not
by verification. Rejected. A memory system that refuses a folder is not a memory system.

### B. A visibility flag per file

`visibility: private` in each file's front matter. Rejected. It is the rule in prose of
ADR-0025 at file granularity: forgetting the line is publishing the file, and the safe
default, private unless declared, makes every knowledge file need a line before it is
served. A per file switch is the mechanism that fails one file at a time.

### C. One declaration per base, with the folder map as the default

`agent.txt` gains one line:

```
private = profile/, projects/, records/
```

The value shown is the default, and it is the folder map's private half, so a base with no
such line behaves exactly as the folder map says. `kb` reads the line off disk and serves
everything outside those folders. `--all` means what its help text always said: include
the private layer. A folder with a note and no manifest at all is a base, and it serves
its note.

**`inbox/` is deliberately not in that list, and the first draft of this record had it
there.** Richard's correction, recorded because it is a design and not a detail: the
deposit is the short memory, and the short memory should be searched. Hiding it loses real
facts. Serving it unmarked lets a raw drop read as settled knowledge. So it is served with
the label on: `memory: "short"` on every result in `kb route --json`, `[SHORT MEMORY:
recent, not distilled]` on the passage header `kb answer` hands the model together with a
rule about it, and `[short memory]` on the terminal line and in the MCP `kb_retrieve` text.
The model decides whether to lean on it, and decides consciously. That reverses the
sentence the deposit's own comment carried, "not indexed, and that is the feature", which
was never true in code: nothing excluded the deposit from the text index, it merely had no
`Search for:` line, so the router never named it while the text scorer surfaced it
unlabelled whenever its words matched. The label makes the real behaviour the intended one.

Cost: the guarantee changes kind. Today the serving decision and the publication decision
cannot disagree because they are the same fact. After this, `kb` never publishes anything
and never asks who does, so there is no second decision to disagree with. The only person
who can publish a fleet is one who runs git on it, and `kb init` hands them a `.gitignore`
that matches the declaration. If they edit one and not the other, that is their repository
and their call, which is what "local first" means.

**Chosen: C.**

## The decision

1. `kb` never consults git to decide what it serves. The runtime dependency on a `git`
   binary is gone from every path that answers a question.
2. A base's private layer is declared by `private =` in `agent.txt` and defaults to
   `profile/, projects/, records/`. `.` declares the whole base. A base named `person`
   is private as a whole by name, because that is the name `kb init --person` writes and
   the person's base carries no manifest of its own, as ADR-0025 already says. The
   deposit, `inbox/`, is served in every scope and every passage from it is labelled
   short memory, at every surface, from one rule in `retrieve::layer_of`.
3. `--all` includes the private layer and means nothing else.
4. A directory holding markdown is a base. No marker, no init, no repository.
5. `kb init` writes a `.gitignore` that mirrors the declaration, as a courtesy to anyone who
   later versions the fleet. Nothing checks that it is used.
6. `kb commit` remains, optional, for whoever versions a fleet and for this repository.

The sentence "unknown is not public" is retired. What replaces it: **undeclared is the folder
map.**

## What changed, 2026-09-01

- `base.rs`: `tracked_files` and `tracked_only` are gone; `private_layer(root)` reads the
  manifest and applies the default, and `MdFile.private` replaces `tracked: Option<bool>`.
  Two states, because there is no longer a third.
- `write.rs`: the `git add` and the `Staged` outcome are gone.
- `init.rs`: no `git init`, no `Vcs`; the `.gitignore` is still written, and a test pins
  that it mirrors `PRIVATE_DEFAULT`. The generated `agent.txt` shows the `private =` line,
  commented, with the default, so it is there to be edited and not to be needed.
- `store.rs`: `tracked` became `private`; an index carrying the old column is emptied on
  open and reported through `index_was_rebuilt`, with a test that builds one in the old
  shape and opens it.
- `memory.rs`: `PrivacyUnknowable` is gone. `skipped` stays on the contract, empty, because
  callers read it.
- `retrieve.rs`: `Layer` and `layer_of`; `Retrieved.layer`; the label at four surfaces.
- `checks.rs`: W08, a `.gitignore` present and missing a declared private folder. A
  warning, never a refusal.
- Both READMEs, `--help`, the release notes: every sentence saying git decides is gone.
- The published `agent-skeleton/agent.txt` regenerated, because the drift test says the
  skeleton is what `kb init` writes.
- 272 tests, from 261. Run against a copy of `examples/demo` outside any repository: three
  agents served without `--all`; `profile/` held back without it and served with it;
  `inbox/` served in both and labelled `short`.

## Consequences

- A hosted consumer deploys a base with no `.git` and it serves. `--all` is passed only by
  someone who means it.
- A new note is served the moment it is written.
- A fleet of a hundred agents opens with a hundred directory reads and zero subprocesses.
- The first interaction with the tool, a folder and a question, works.
- Anybody who wants their fleet versioned still has `kb commit` and the ignores written for
  them. Anybody who does not never hears the word git.
- A raw drop reaches a model with its label on. Whether to lean on it is the model's call,
  made knowing what it holds, which is what Richard asked for in as many words.

## Revisit trigger

A consumer that needs a third serving scope, neither "everything" nor "everything outside the
private folders": for example a hosted agent that may read `projects/` but not `profile/`.
That is a per consumer scope rather than a per base declaration, and this record does not
provide it. Build it when the second real consumer asks, not before.
