# ADR-0011: the fleet has one shape, and the library still accepts any path

**Search for:** `fleet`, `fleet root`, `agent shape`, `layout`, `agents/`, `fleet.txt`, `attach`

- **Date:** 2026-08-16
- **Status:** proposed
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** **the least reversible decision made so far.** Every agent created from now on has
  this shape, the orchestrator's create path assumes it, and the hosted service inherits it as its
  tenancy model. Changing it later is a migration of everyone's data, not a refactor.

## Context

Richard asked for a defined structure from the beginning, on the grounds that it is what will make the
hosted service, the orchestrator's own organisation, and **the orchestrator creating new agents**
tractable. His words: *liberdade de criação não significa falta de estrutura*.

He is right, and [[0009-gui-runtime-boundary]] was incomplete on this point. It concluded that a fleet
root should be accepted and never required, citing [[0008-single-user-open-source]]'s rule that the
base is addressed by path and never assumed. That reasoning holds for the **library** and collapses two
layers that are not the same thing:

| Layer | Rule |
|---|---|
| What `kb` the library accepts | Any path. This is what makes it embeddable, testable and honest |
| Where the product **creates** and looks by default | One defined place, with a defined shape |

Git is the same shape twice: a repository lives anywhere, and `~/.gitconfig` lives in exactly one
place while `git init` produces exactly one layout.

**The argument that settles it:** an orchestrator that can create an agent has to know where to put it.
"Anywhere" is not an answer to that question. The moment creation is a feature, a default location
stops being a constraint on the user and becomes a fact the software needs.

And the failure mode Richard named is real and already has a precedent here. One agent on the Desktop,
one in Documents, one in OneDrive is the same class of problem as the index whose location depended on
which directory you happened to run from, which is how a benchmark in this repository measured the
wrong database and was believed.

## Decision

**A fleet is one directory tree with a defined shape. The orchestrator creates inside it. The library
still opens any path.**

```
<fleet root>/                     the unit of backup, of sync, and later of tenancy
  fleet.txt                       manifest, for exceptions only
  agents/
    zed/
    steve/
    yaron/
  outbox/                         artefacts produced for the user
```

### The rule that makes the whole thing work

**No absolute path exists anywhere inside the fleet.** Everything is relative to the fleet root.

That single rule is what buys:

- **Moving** the fleet is a directory move, with nothing to update inside it.
- **Backup and sync** are a copy. OneDrive, a zip, `rsync`, all equivalent.
- **The hosted service** is `tar` of one directory per tenant, and the export promise in
  [[0008-single-user-open-source]] stops being a sentence and becomes a property of the layout.
- **Leaving costs nothing**, which is the only version of the paid service worth offering.

There is **exactly one absolute path in the system**, and it lives outside the fleet: a pointer file in
the OS config location saying where the fleet root is. If that pointer is stale, the app says it cannot
find the fleet and asks, rather than silently creating an empty one somewhere else. A tool that
recreates missing state in silence is how a user loses a year of notes and finds out later.

### What an agent is, exactly

The shape below is not designed, it is what the three agents already converged on, written down so the
orchestrator can create a fourth without a human deciding anything.

```
agents/<name>/
  CLAUDE.md          the boot file, thin, points at index.md
  index.md           operating instructions
  MAP.md             the map, every entry with a Search for line
  agent.txt          name and role, machine readable
  blocks.txt         the constitution as blocks, stability ordered
  kb-aliases.txt     alias table, optional
  knowledge/         distilled notes, the brain
  inbox/             raw material awaiting distillation
  decisions/         ADRs that outlive a conversation
  protocols/         its own procedures
  templates/
  .kb/index.db       the derived index, gitignored
```

**Amended 2026-08-17: two repositories, split by who may read them, not by who owns them.**

```
fleet/            PUBLIC   the system: tools/, README, fleet.txt, .gitignore
  agents/         PRIVATE  a repository of its own, ignored by the one above
    zed/ steve/ yaron/
```

The original rule said each agent is its own repository and the fleet root is not, and it gave as its
mechanism that "the privacy gate in `kb` needs git per base, not per fleet". **That mechanism is
wrong, and it was asserted without being run.** Measured on 2026-08-17: `git ls-files` invoked from a
subdirectory of a larger repository returns paths **relative to that subdirectory**, which is exactly
what `Base::discover` compares against, and each agent's own `.gitignore` still governs its own
subtree. The gate does not care where the repository root sits.

That correction is what makes this layout possible at all. The requirement that produced it is
Richard's: the system is published, and **nothing anybody fed an agent ever is**, without having to
check before a commit. A rule you have to remember is a rule that fails on the day you are tired.

So the split is `/agents/` in the public `.gitignore`, one line, and `agents/` being its own
repository underneath. Three properties follow, and they were each measured rather than reasoned:

- **`git add -A` in the public repository cannot stage a note.** Git does not descend into an ignored
  directory, so there is no version of forgetting that publishes one.
- **The nested repository is invisible upward.** No gitlink, no submodule, no broken clone.
- **The privacy gate still runs.** `git ls-files` from `agents/zed` resolves to the repository at
  `agents/` and answers `MAP.md`, `knowledge/...`, agent relative. All three agents open without
  `--all` and `kb check` reports them clean.

**The software moved out of the agent.** `tools/` was inside `agents/zed/`, which put the one thing
that must be public inside the one tree that must not be. It now sits at the fleet root, where it was
always conceptually: the system that runs the fleet is not one agent's possession. Its history came
across with `git subtree split --prefix=tools`, 24 commits carrying `kb/` and `tray/` and no other
path, which was checked rather than assumed.

**What this costs.** A contributor who clones gets the system and an empty `agents/`, and builds a
fleet with `kb init` rather than reading somebody else's. The design record still lives with the
architect agent, on the private side, so publishing it is a later and deliberate act rather than a
side effect of this one.

Original text, kept because a decision reversed without its reasoning visible is a decision that gets
made again:

> Each agent is its own git repository. The fleet root is not. Nesting repositories buys nothing here
> and costs a class of confusing failures, and the privacy gate in `kb` needs git per base, not per
> fleet.

**`agent.txt` is a separate file and not front matter in `index.md`,** for a mechanical reason rather
than taste. `blocks.txt` orders the constitution by stability because prefix caching invalidates from
the first differing token onward, and `index.md` sits in the most stable block. A field the orchestrator
reads to draw a menu does not belong inside the block whose whole job is never changing.

```
name = Zed
role = Software architecture and building
```

### The index lives with the agent

`agents/<name>/.kb/index.db`, one per agent, replacing the single index with a `base` column.

Richard proposed this and it is right for the reason already accepted twice in this codebase: **a
missing predicate cannot leak a file that is not in the database you opened.** It also fixes three
things at once:

- The default location becomes unambiguous. Today it is relative to the working directory, which is
  how the benchmark measured an index containing one base while believing it held three.
- Moving an agent takes its index with it; deleting an agent deletes its index. Neither is true today.
- A `--all` run on one agent cannot contaminate queries against another.

Cross-agent retrieval then opens N databases instead of one, which is N connections and no new ideas.

### Convention first, manifest only for exceptions

Everything under `agents/` is an agent. No configuration says so, which means no configuration can be
wrong about it. This is the same stance as the rest of the codebase, where map files and knowledge
folders are **detected by shape rather than declared**.

`fleet.txt` exists only for what convention cannot express, in the same deliberately unclever format as
`kb-aliases.txt`, because a parser someone has to look up is a parser they will not use:

```
# Agents in agents/ are found automatically. This file is for exceptions.

# A base that lives outside the fleet, attached deliberately:
attach = ../work/client-notes

# An agent switched off without deleting it:
disable = steve
```

**`attach` is how freedom survives structure.** A user with a notes folder elsewhere is not told to move
it. But the attachment is recorded in one file inside the fleet, so there is always exactly one place
that knows where everything is. That is the difference between freedom and scatter: scatter is when
nothing knows.

### What the library keeps

`Memory::open` still takes paths and still accepts a base, a fleet root, or a bare directory.
[[0008-single-user-open-source]] is untouched: the base is addressed by path, never assumed. What
changes is that the **product** has a home, creates there, and looks there first.

## Consequences

- **`kb init` becomes necessary rather than a nicety.** The orchestrator creating an agent means
  generating this shape from a template, and the three existing agents were hand built, which is fine
  for three and impossible for three hundred.
- **The three agents move** into `<fleet>/agents/`. A filesystem move; each git repository survives it
  intact. Steve's uncommitted work survives it too, but it should be committed first anyway.
- **The tray's drop target has an answer.** A file dropped on the tray lands in an agent's `inbox/`, and
  which agent is a routing question `kb route` already answers.
- **`.mcp.json` and any registered MCP server command need their paths updated** after the move. This is
  the one place absolute paths legitimately exist, because they live outside the fleet.
- **The hosted service's tenancy model is decided by this ADR**, whether or not we admit it later. One
  tenant is one fleet root. That is why this is the least reversible decision so far.

## Revisit trigger

- The first agent that does not fit the shape, which would say the shape was derived from three
  examples that happened to be similar rather than from what an agent needs.
- A user wanting two fleets on one machine, which the single pointer file does not express and which
  would need a real answer rather than a second pointer.
- The first hosted tenant, which turns the layout from a convention into a contract with someone.
