---
provenance: agent
stage: derived
---

# ADR-0016: a note and the entry that makes it reachable are one write

- **Date:** 2026-08-17
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible

## Context

[[0007-memory-architecture]] made every write a proposal a human approves, and
`remember.rs` is the proposing half: it measures a claim against the base and returns ADD, UPDATE or
NOOP without touching disk. **Nothing ever turned an approved proposal into a file.**

Richard named the consequence directly, and it is the sharpest thing said all day:

> de nada adianta kb_retrieve super rapido se nem tem o que puxar

He is right, and the whole session had been proving it without noticing: a routing audit, an alias
audit, a suggestion engine and a miss log, all of them improving retrieval over a corpus that nothing
fills. A fresh `kb init` has a complete constitution and an empty library, and it stays empty until
somebody writes markdown by hand.

The second fact comes from the same day's measurements. The largest single cause of routing failure
was vocabulary the map did not carry, and a separate audit found 25 alias lines pointing at canonical
terms **no map mentions anywhere**. Both are the same defect: notes and their keys drift apart because
nothing makes them arrive together.

## Options

### Option A: write the note, let the linter catch the missing entry

`kb write` creates the file. `kb check` reports E02 when a note has no map entry, and somebody fixes
it later.

- Cost: nothing to build, the check already exists.
- **Failure mode: the note is unreachable in the meantime, and "the meantime" is unbounded.** A note
  with no entry cannot be ranked by the keyword scorer at all, so the only way to it is the full text
  scorer alone, which is the single scorer case this system already reports as a guess rather than an
  answer.
- What it forecloses: nothing, but it makes the base quietly worse in a way only a linter run reveals.

### Option B: refuse to write a note without its entry

The note and the map entry are produced by one command, the keys are a required argument, and there is
no flag to skip them.

- Cost: the caller has to decide the keys at write time, which is real work and is exactly the work
  that was being deferred.
- Failure mode: a caller in a hurry writes bad keys. Still strictly better than no keys, because a bad
  key can be found and fixed by a question that missed, and a missing key cannot.
- What it forecloses: bulk importing notes without thinking about how they will be found. Deliberate.

### Option C: generate the keys automatically

Derive the `Search for:` line from the note text.

- Cost: it is the summarisation problem, and doing it without a model produces term frequency noise
  rather than the words a person would ask with.
- **Failure mode, and it is fatal to the idea: the keys exist to bridge the gap between how somebody
  asks and how the file was written.** Deriving them from the file guarantees they carry the file's
  vocabulary, which is the half we already have. `nunca` never appears in a note about limits.

## Decision

**Option B.** A tool that can create an unreachable note has handed you a way to grow a base while
making it worse, and leaving that to the linter means finding it later instead of preventing it now.
Option C is what the alias table and the miss log exist to correct, so building it would be building
the problem.

Two consequences settled while implementing, both of which changed the design:

**The write stages the files, and staging is part of writing rather than tidiness.** `kb` reads
`git ls-files` to know what is public, so an untracked note is a note the router will not serve. The
end to end test caught it: the note was written, the command reported success, and the same question
still missed. **A model would have written a memory, been told it worked, and failed to find it.**
Staged and not committed, because `git ls-files` reads the index so staging is enough to make it
findable, while what the history says stays a human decision and `git status` still shows what
changed. A path a `.gitignore` covers is reported as private rather than forced, since that is the
private layer working.

**It is a command and not an MCP tool.** [[0010-memory-as-mcp-server]] deferred a model reachable
write deliberately: a write tool reachable by a model is a different security surface and gets built
deliberately rather than as an afterthought while the retrieval side is still warm. That still holds.
The model in the loop today has a shell, so the path is usable immediately with the harness's own
permission prompt as the human gate, and exposing it over MCP stays a separate decision.

## Consequences

- The bootstrap loop closes. Demonstrated on a fresh `kb init`: the empty base says it has no
  knowledge files, the note is written with its keys, and the same question then returns it.
- Every note created this way has a `Search for:` line, so E02 and W02 stop being findings about
  notes this tool made.
- The map grows by appending to the section for the note's folder, creating that section when absent.
  **Appending rather than sorting**, because a map's order carries meaning nothing mechanical can
  read: entries are often ordered by how they build on each other.
- `kb write` does not reindex. It says to run `kb index`, because a command that quietly rewrote a
  database while you were writing a note is a command doing two things under one name.
- Nothing yet decides *what* is worth recording. That judgement is the model's and it is the next
  piece.

## Revisit trigger

- A real import arrives, a hundred notes at once from another system, and requiring keys per note
  makes it unusable. At that point the question is whether an import is the same operation as a
  write, and the answer is probably no.
- Or `kb_remember` gains an approval surface in the tray, at which point the write stops being a
  command a human types and the gate moves.

## Notes

Verified 2026-08-17: 134 tests, eight new, including that a note without keys is refused and leaves
nothing behind, that an existing note is never overwritten, and that the entry lands in the section
for its own folder rather than wherever the map happens to end.
