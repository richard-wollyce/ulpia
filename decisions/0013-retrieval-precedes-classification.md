---
provenance: agent
stage: derived
---

# ADR-0013: the base is read before the model is allowed to decide it was not needed

- **Date:** 2026-08-17
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible

## Context

[[0006-language-architecture]] specifies recall as a cascade: a free keyword lookup first, a local
expansion pass on a miss, embeddings last. Only the first step was built, because the alias table
covered the cases we had. Nothing measured what the free step was dropping.

**It was measured on 2026-08-17.** Twenty real questions were run through `kb route`, which is the
tool an agent loop would call. Before any repair: **10 clean hits, 3 weak, 7 misses.** The fused
scorer, which is what `kb_retrieve` uses, rescued **zero of the seven**, so the defect was never in
the ranking.

Repairing the map vocabulary and the alias table took the same twenty questions to **17 hits, 1 weak,
2 misses**, with no code changed. That is the good news and it is also the problem, stated by Richard
in the same conversation:

> a base de codigo para "apontar para onde uma pergunta vai" vai ser infinita, pq sempre terá formas
> diferentes de fazer a mesma pergunta

He is right, and the strongest evidence is the repair itself. Five questions were fixed with twelve
alias lines and roughly twenty keywords, each one matching the exact phrasings that were tested.
"cargo test falhou no link" only ranked once `cargo test` was added as a phrase. "o build quebrou no
linker" would miss again. **The second measurement is also not independent of the first**, because the
same person wrote the questions and then tuned the map against them.

Two further facts bound the design:

- **The keyword router has no edit distance at all.** It folds accents and stops there, so a typo is a
  silent miss. A model handles typos for free.
- **Claude Code's native memory has no keyword stage whatsoever.** The whole index sits resident and a
  model reading prose is the search engine, which is why a Portuguese question retrieves an English
  note there for nothing. It does not scale: the index grows linear, and at 300 memories that is about
  20k tokens of prefill, which is roughly ninety seconds of silence on this machine per
  `local-inference-latitude-3420`. Evidence in `claude-native-memory-mechanism`.

**The two designs are the same design at different scales, failing on different axes.** Theirs is
immune to vocabulary and hostage to size. Ours is immune to size and hostage to vocabulary.

## Options

### Option A: a model gates retrieval

The question reaches a model first. It decides whether the base is needed at all, and only then is
anything retrieved. Obvious questions are answered directly.

- Cost: one local generation on every question, which is exactly what [[0004-local-first-inference]]
  exists to avoid paying per query.
- **Failure mode, and it is the disqualifying one: this is the decision a model is worst at.** It does
  not know what it does not know. "Qual o padrao de qualidade aqui" has a good generic answer and a
  different house answer, and a model that believes itself capable returns the generic one with full
  confidence. `index.md` section 1 exists precisely because of this: never answer from general
  knowledge alone when the base has material.
- What it forecloses: the guarantee that the base was consulted. The failure is silent, which is the
  same property that made the original routing misses dangerous.

### Option B: retrieve always, classify after

Routing is a reflex, not a decision. Every question is routed, because routing costs microseconds and
reads no text. The model sees what came back **and then** decides: answer from the base, or say the
base does not cover this and answer from reasoning, knowing that is what it is doing.

The system already carries the signal that makes this work. Two independent scorers rank every file,
and **agreement, not score, is what separates a hit from a guess**. `Memory::no_agreement` already
computes it. So the cascade becomes: route, and where agreement is weak or nothing matched, spend the
local model on expansion or hand the question up with an honest "this appears uncovered".

- Cost: one lookup per question, measured in microseconds, plus a bounded local generation **only on
  the miss path**.
- Failure mode: a question the base covers under vocabulary nobody guessed still misses, and the model
  then answers from general knowledge. Recoverable, because it is logged and the miss log is the
  worklist.
- What it forecloses: nothing. Option A remains buildable on top.

### Option C: do nothing

Keep growing the alias table and the keyword lines by hand. Every real miss becomes a line. It works,
it is free, and it is the treadmill Richard named: unbounded, because natural language is.

## Decision

**Option B, with Richard's sharpening, which is stronger than the version proposed to him.**

Zed proposed letting measured agreement decide when the model gets involved. Richard's amendment:
**always read the base first, and only then let the model decide whether to answer from its own
knowledge.** The difference matters. In the proposed version the model is invoked on a weak result. In
the accepted version the model is never in a position to conclude it did not need to look, because
looking has already happened by the time it is asked anything.

That is what beat Option A. Option A puts the one decision a model is unreliable at in front of the
one mechanism that is reliable and free. Option B puts the free mechanism first and spends
intelligence only where the free mechanism admitted it failed.

It beat Option C because Option C was measured today and the measurement is what produced this ADR.

**Classification is the model's job and lookup is the code's job**, which is not a new position here.
`Memory::describe` records that an earlier version classified questions and answered with strings we
had written, and calls it code doing a model's job badly. This ADR builds the half that was
deliberately left empty rather than changing the split.

## Consequences

- **`kb route` stops being the agent's only door and becomes its first door.** A miss is no longer
  the end of the question.
- **The miss log becomes a recall loss log**, which is what Z13 argued it had to be. Every entry is a
  question the free stage failed to answer, and reading it as a keyword worklist makes the cascade
  look like it is working exactly when it is dropping questions in silence.
- **The map cannot leave the resident set yet.** Flipping `[map]` to on-demand in the three
  `blocks.txt` was the plan for today and it is now blocked on this. If the router is the only door
  and it fails silently, removing the resident map removes the net. The order is: build the expansion
  step, measure the gap against the resident map, then decide.
- We now have to build and maintain a local expansion step, and pay for it on the miss path.
- Typos remain unhandled until that step exists. Naming it rather than hiding it.

## Revisit trigger

- The expansion step is built and measured, and the gap against a model reading the whole map is
  **under two questions in twenty**. At that point the resident map has no remaining job and
  [[0005-wake-with-the-constitution]] can drop it.
- Or the fleet's map passes roughly 300 entries, at which point the resident approach costs what
  `claude-native-memory-mechanism` measured and the comparison has to be redone with real numbers
  rather than the ones borrowed here.
- Or a measurement on questions **Zed did not write** contradicts the 17 in 20. That number is tuned
  against its own test set and is not independent.

## Notes

Measurement, causes and the reverted code experiment are recorded as Z13 in `backlog`. The
comparison system is distilled in `claude-native-memory-mechanism`.

A code change was proposed, built, measured and reverted the same day: scoping each agent's
`kb-aliases.txt` to its own base, on the theory that merging them let one agent's vocabulary decide
another's question. It changed the result by nothing, byte for byte, because **an alias can only match
if the canonical term appears in some file's indexed text**, so each table only ever helps its own
base. Reverted rather than kept.
