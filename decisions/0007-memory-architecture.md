# ADR-0007: the memory pipeline, and provenance as a first class field

**Search for:** `ADD UPDATE DELETE NOOP`, `NOOP`, `provenance`, `stage`, `write gate`, `delete`

- **Date:** 2026-08-13
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** the front matter fields are cheap to change now and expensive after a hundred
  notes carry them. The pipeline itself is reversible at any time.

## Context

Surveying mem0, Letta and Zep produced four things worth taking and one thing worth refusing, recorded
in `ai-memory-layers-market`. None of them stores memory as files, so none is a drop in. What they
have that we do not is a **pipeline**: an explicit sequence for deciding what enters the base, what
replaces what, and what is thrown away.

We have the same steps written as prose in `protocols/source-ingestion.md`. Prose is where a step goes
to be skipped.

## Decision

### 1. Four outcomes, named, on every write

Every claim that reaches the base resolves to exactly one of:

| Outcome | Meaning | Who can do it |
|---|---|---|
| **ADD** | New, not in the base, worth keeping | Agent |
| **UPDATE** | Already present and changed. The old text is struck and dated in place, never silently overwritten | Agent |
| **DELETE** | Present and no longer true, or never should have entered | **Agent**, recorded, never silent |
| **NOOP** | Already in the base, unchanged. Do nothing | Agent |

**NOOP is the one that looks like nothing and is the most important.** Without it, a pipeline
re-writes the same fact every time it sees it. That is not a hypothetical: **52.7% of the junk in the
mem0 audit was the agent re-extracting its own boot file, session after session.** More than half of a
10,134 entry base was one missing branch. NOOP is that branch.

### 2. The agent may delete, and that is the safer design

Richard's call, and it corrects the draft. The draft required a human for every DELETE.

**A delete gate produces hoarding.** If removing something is expensive, nothing gets removed, the base
rots by accumulation, and you arrive at ten thousand entries with a 2% signal rate by a different road.
`protocols/checkin.md` already has the monthly rule of deleting one thing precisely because bases rot
that way.

**And the reason a gate is unnecessary here is structural: the base is files under git.** A delete is a
commit. It is visible in a diff, recoverable with `git revert`, and attributable. That is exactly the
property mem0 lacks, and it is why deletion is dangerous there and cheap here.

So: **the agent deletes, and the constraint is not permission, it is disclosure.**

- The reason goes in the commit message. A delete with no stated reason is the failure, not the delete.
- The agent asks when it is genuinely unsure, and it is trusted to judge when that is.
- What was deleted and why remains findable by `git log`, forever.

### 3. Provenance and stage, as front matter, on everything

Two orthogonal axes, never collapsed into one:

```yaml
provenance: human | agent | external
stage: raw | distilled | derived
```

| Provenance | Means |
|---|---|
| `human` | Richard said it or wrote it |
| `agent` | The agent derived, inferred or concluded it |
| `external` | A third party source: paper, article, video, repository, label |

| Stage | Means |
|---|---|
| `raw` | As received, unprocessed |
| `distilled` | Processed into a note, carrying an evidence tier |
| `derived` | Machine generated and disposable: index, embeddings, projections |

**The trust of a claim is provenance times verification, not the folder it happens to sit in.**

**And the rule that makes it worth having:** an `agent` claim is never promoted to `human` or
`external` without a human act. That is the write gate from
`ai-memory-layers-market`, turned into something a linter can check instead of something a reader has
to remember.

This was already being done by hand and nobody noticed. the user profile note already does this by hand, separating what the user said from the agent's reading of it, in labelled sections, because the writing needed the distinction. The
field formalises what the prose already did.

### 4. The constitution becomes labelled blocks

From Letta. Today the constitution is one blob. It becomes blocks, each labelled by purpose and
independently swappable:

| Block | Contents | Changes |
|---|---|---|
| `identity` | Who the agent is, the method, the bar, the limits | Rarely, by ADR |
| `user` | Who it works with, from the profile | Occasionally |
| `project` | The project currently in focus | Per project |
| `session` | What is open right now | Per session |

**The mechanical payoff is prefix caching.** The blocks are concatenated in stability order, most
stable first, so swapping the project block invalidates the KV cache only from that point forward
instead of from the beginning. Measured basis in `local-inference-latitude-3420`: a cold 8,259 token
constitution costs 129 seconds and a warm one costs 10.7. Ordering the blocks by how often they change
is what keeps that 12x.

### 5. Content hash to decide what gets reindexed

From `sqlite-memory`. Unchanged file, skipped. Changed file, its chunks replaced atomically. Deleted
file, its rows cleaned. This is what makes a rebuild cheap enough to run at every startup, which is
what lets the index stay derived instead of becoming a thing we are afraid to lose.

### 6. The index becomes SQLite, and that costs the zero dependency stance

The derived index outgrows a hand written scan the moment it holds embeddings. It becomes one SQLite
file: FTS5 for BM25 keyword search, `sqlite-vec` for vectors, both in the same file, no server.

**That means `kb` takes its first dependency, and the earlier stance has to be named rather than
quietly dropped.** Zero dependencies was a choice made for a linter that parses brackets, where a
regex crate would have bought a supply chain against parsing a bracket pair. It was never a religion.
Writing a B-tree and a full text index by hand to avoid a dependency would be the same mistake in the
opposite direction. `rusqlite` with the bundled feature compiles SQLite in, so there is still no system
package to install and nothing to run.

## What we deliberately do not take

- **Automatic extraction of facts from conversation into the durable base.** The thing that produced
  the 97.8%. Ingestion stays deliberate.
- **A vector database as the source of truth.** [[0003-knowledge-storage]] settled it and nothing found
  in the survey moves it.
- **A hosted memory service.** Contradicts [[0004-local-first-inference]] and the durability clause in
  the north star.

## Consequences

- Every note gains two front matter fields, and old notes need backfilling.
- `kb` gains two checks, that the fields exist and that their values are legal.
- `kb` gains two verbs, `ingest` and `remember`, and the second one is where the four outcomes live.
- The first dependency lands, deliberately, with the reason recorded.
- **Automatically written memory and hand written knowledge never share a namespace.** If they mix, no
  reader can tell which claims were verified, and the evidence ruler becomes decorative on both.

## Revisit trigger

- The first time a DELETE removes something we wanted, which would say the disclosure rule is not
  enough on its own.
- Provenance turning out to need a fourth value, most likely for something produced jointly.
- The index outgrowing SQLite, which at our scale would be a surprise worth investigating before
  believing.
