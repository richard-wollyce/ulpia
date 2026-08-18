---
provenance: agent
stage: derived
---

# ADR-0015: expansion is split by kind of distance, and the code half ships first

- **Date:** 2026-08-17
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. The local model half stays specified and unbuilt, not foreclosed

## Context

[[0013-retrieval-precedes-classification]] blocks taking the map out of the resident set until the
expansion step exists, so this is the next thing. [[0006-language-architecture]] specifies it:

> **Step 2. Query expansion by the local model. Only on a miss or a tie.** Rewrite the question into
> candidate canonical terms. Bounded output, 20 to 40 tokens, which is the cheapest kind of local
> generation there is.

Two facts checked before building, rather than designed against memory:

**There is no local model on this machine.** No `.gguf`, no llama.cpp, no ollama, nothing on PATH.
Disk is no longer the blocker it was in F6, at 51 GB free rather than 25, so one could be installed.

**"The cheapest kind of local generation there is" is true relatively and expensive absolutely.**
`local-inference-latitude-3420` measures generation at **5.55 to 5.88 t/s** here, so 20 to 40 tokens
is **3.4 to 7.2 seconds**, before prefill. `kb_retrieve` end to end was measured at 4.43 ms. Step 2 as
written therefore costs roughly a thousand times the entire retrieval, and it is paid **on the miss
path**, which means the person waits five seconds to be told the base may not cover the subject.

The measurement in 0013 also shows the misses are not one kind of problem:

| Real miss | What was needed | Reachable by a string measure |
|---|---|---|
| `repositorio publico` | `repository`, `public` | Yes, they are cognates |
| a typo in any English term | the term | Yes |
| `nunca` | `never` | No |
| `canal simples` | `single channel` | No |

## Options

### Option A: build step 2 as specified, with a local model

Install a quantized model, call it from `kb` on the miss path, take back 20 to 40 candidate terms.

- Cost: about 2.5 GB of disk, a subprocess or an HTTP server, and **the property the README
  advertises**: one dependency, no build scripts, no network at runtime. That becomes "one dependency
  plus a model server" and the claim has to be rewritten.
- Latency: 3.4 to 7.2 seconds per miss on this hardware, measured.
- Failure mode: the slowest reply the system produces is the one that tells you it failed.
- Covers: everything, including translation.

### Option B: split the problem by kind of distance

**Orthographic distance is plain software.** Character trigram overlap between the question's words and
the keyword vocabulary the base already holds, 849 distinct terms across this fleet. Microseconds, no
dependency, deterministic, and it explains itself.

**Semantic distance needs a model, and one is already in the loop.** So on a miss the base returns the
candidate terms rather than a dead end, and whatever model is reading expands against a real vocabulary
instead of guessing blind.

- Cost: zero new dependency. About 1 KB added to the miss reply.
- Failure mode: a caller that is not a model gets a shortlist and no expansion. Acceptable, since a
  script has nothing to expand with either way.
- Covers: typos and cognates. Not translation.

### Option C: do nothing

Keep growing the alias table by hand. It is the treadmill Richard named, and 0013 accepted that it is
unbounded.

## Decision

**Option B, and Option A stays specified and unbuilt.**

The reason it beat A is not cost, it is **honesty about which half is which**. A local model returning
expansion terms gives one answer with one confidence for two problems that have very different
reliability. Trigram overlap on the other hand can state its own limit exactly: it finds a typo and a
cognate and it never finds a translation, and the reply says so. A suggestion whose limits are not
stated gets read as the whole answer.

The second reason is the cascade's own rule, applied consistently. Step 2 was deferred in 0006 because
step 1 covered the cases we had. The same test applies one level down: build the free half, log what it
misses, and let the log decide whether the paid half is needed. Installing a model on the strength of
an argument rather than a measurement is exactly the move `the-bar` calls complexity before the
second use case.

**What decided it against C is that B costs almost nothing and closes a real class**, demonstrated:
`ingestao` now reaches `ingest a source`, a Portuguese cognate finding an English keyword with no alias
line, no model, and no dependency.

## Consequences

- `kb route` and `kb_retrieve` return candidate vocabulary on a miss, on the CLI and over MCP. The
  `kb_route` tool description now tells the model this exists and what its limit is, because a
  capability a caller does not know about is not a capability.
- **The miss reply is now the only place the base volunteers its own vocabulary**, which makes the
  failure path the most instructive reply the system produces rather than the least.
- The alias table stays. Trigrams do not replace it, they cover the cases nobody thought to write down.
- We now maintain one tuned number, `SUGGEST_FLOOR`, and it is **the weakest thing in this decision**.
  It is 0.65 because at 0.5 the word `quando` reached the keyword `K-quant` at 0.571 on a shared
  prefix. It was calibrated against a handful of pairs from one session.
- Boundary padding, the standard fix for short word noise, was rejected on the arithmetic: it rewards
  a shared prefix, which is what the false positive already had, taking that pair up to 0.615.

## Revisit trigger

- **The miss log shows suggestions failing to help on most misses**, which would mean the orthographic
  half is not where the failures live and Option A is due.
- Or the vocabulary passes a few thousand terms, where a linear scan of every keyword on every miss
  stops being free.
- Or `SUGGEST_FLOOR` produces a false positive or a false negative on a real question Richard asks. It
  is tuned to a sample and the sample was small.

## Notes

Built and verified on 2026-08-17. 119 tests passing, five of them new and one of them
`a_translation_is_never_suggested`, which guards the claim the reply makes rather than the behaviour
that is convenient.

**A wrong turn worth keeping:** the first implementation required *every* word of a multi word keyword
to find a partner in the question, so `ingestao` scored 0.80 against `ingest` and nothing against
`source`, averaging to 0.40 and hiding the one suggestion the question deserved. That rule was borrowed
from matching, where precision matters because the output is presented as an answer. **Suggesting is
the opposite trade:** the reader filters, so the strictness belongs in the per word floor and the term
should rank on its best alignment.
