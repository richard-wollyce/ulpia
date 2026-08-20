---
provenance: agent
stage: derived
---

# ADR-0017: BGE-M3 measured and not adopted, because fusing it made the system worse

**Search for:** `embedding`, `embeddings`, `dense`, `BGE-M3`, `bge`, `e5`, `vector search`, `semantic`

- **Date:** 2026-08-17
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** fully reversible. Nothing was built, and the measurement is the artefact

## Context

[[0006-language-architecture]] specifies embeddings as step 3 of the cascade, for when hand written
keys stop covering the base. [[0015-expansion-split-by-distance]] then split expansion by kind of
distance and left the semantic half to whatever model is in the loop. The open question was whether a
dense scorer should join the keyword scorer inside `kb`, fused by the RRF that is already there.

Richard chose the candidate and the test: run BGE-M3 against the same twenty real questions, and keep
it if it is good. He picked BGE-M3 over `e5-small` for two reasons that hold up: **cross lingual
retrieval is our defining problem**, a Portuguese question against an English base, and BGE-M3 emits
dense, sparse and ColBERT vectors in one forward pass, which composes with the fusion we already have
rather than replacing it.

Two things settled before measuring. **Document length is not an argument here**, because `store.rs`
already chunks at 1800 characters with 300 of overlap, so an hour long transcript is chunked whatever
the model's context window is, and a single vector for an hour of speech is the average of an hour of
speech. And **two models is not an option**: two models are two vector spaces, so routing short
questions to one and long ones to the other would require indexing the whole corpus twice.

## Options

### Option A: adopt BGE-M3 as a second scorer, fused by RRF

- Cost: 2.2 GB model, and the `fastembed` crate pulls `ort`, `tokenizers`, `ndarray` and `hf-hub`
  against the **one dependency** the README advertises.
- Measured on this machine: **2,833 seconds to index 1,039 chunks**, which is 47 minutes at 2.7 s per
  chunk. Query time is fine at 71 ms.
- Upside if it worked: cross lingual recall without the alias treadmill.

### Option B: keep the keyword scorer alone until the measurement says otherwise

- Cost: the treadmill Richard named, unbounded because language is.
- Upside: nothing new to maintain, and the miss log now says whether the treadmill is converging.

## Decision

**Option B, and the reason is measured rather than argued.**

Twenty questions, two runs. Embedding **map entries**, apples to apples with what the keyword scorer
ranks, BGE-M3 got 9 of 20. That test was unfair to it: a map entry is a title, a comma separated
keyword list and a summary, which is not natural language and is out of distribution for a model
trained on passages. Embedding the **note bodies**, chunked the way `kb` chunks them, it got 13 right,
2 weak, 5 wrong, against roughly 16 for the keyword scorer.

Losing a head to head is not by itself disqualifying, because **RRF does not need the second scorer to
win, it needs it to be wrong about different questions.** So the fusion was computed directly:

**Nineteen of twenty answers are unchanged. The one that changes, changes for the worse**, moving "o
que voce nunca faz mesmo se eu pedir" off `index.md`, which is correct, onto an unrelated marketing dossier. And the question the keyword scorer honestly does not answer stops returning
nothing and starts returning another unrelated marketing dossier.

**An honest abstention became a confident wrong answer**, which is the exact failure this system is
built to avoid, and adding a dependency to buy it is the wrong trade at any price.

**The finding that outlives the decision** is about scores rather than about BGE-M3. The keyword
scorer's one wrong answer scored **3.82** while everything correct scored 9.55 or higher, so its score
carries some signal about its own wrongness. BGE-M3's correct answers scored 0.496 to 0.659 and its
wrong answers 0.510 to 0.608, **overlapping completely**, and its single worst error scored higher
than most of its correct answers. A dense scorer that never abstains removes the property that
separates this system from the ones it competes with.

## Consequences

- No new dependency, and the README's one dependency claim stays literally true.
- The cross lingual gap stays open and stays on the alias table plus the model in the loop, per 0015.
- The miss log becomes the instrument that decides when this is reopened, which is what it was built
  for.
- **A keyword score floor is now worth investigating** and was not before. 3.82 against 9.55 is a
  visible gap. It is one wrong answer, so this is a reason to look rather than a calibration.

## Revisit trigger

- **The base passes roughly a thousand files**, where hand written keys plausibly stop covering it and
  a dense scorer's advantage grows. Today it is 121 files and 1,039 chunks.
- Or a question set **nobody tuned against** shows the keyword scorer below the dense one. The set
  used here was written by Zed and the keyword lines were tuned against it the same day, which biases
  the comparison toward keywords and is the single largest weakness of this measurement.
- Or the indexing cost stops mattering, on hardware where 2.7 s per chunk is not 47 minutes per
  reindex.
- Or a smaller model changes the arithmetic. `e5-small` was not measured, only reasoned about.

## Notes

Measured 2026-08-17 on the Latitude 3420, CPU only, `BAAI/bge-m3` via sentence-transformers 5.7.0 and
torch 2.13.0+cpu, against 121 tracked files and 1,039 chunks. The Python environment was a throwaway
measuring instrument in a scratch directory and does not ship. If a dense scorer is ever adopted, the
production path is the `fastembed` crate, which runs ONNX in process from Rust with no Python and no
server, and carries `Bgem3Embedding` for dense, sparse and ColBERT in one pass.

**What was not tested:** `e5-small`, the quantized BGE-M3 that `fastembed` defaults to, reranking with
a cross encoder, and RRF over deeper lists than top three.
