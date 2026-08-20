---
provenance: agent
stage: derived
---

# ADR-0018: no model enters the retrieval path, and the score floor becomes the mechanism

**Search for:** `reranker`, `rerank`, `reranking`, `reordenar`, `reordenacao`, `ordenacao`, `cross encoder`, `bge-reranker-v2-m3`, `jina-reranker-v2`, `jina`, `BGE-M3`, `dense`, `sparse`, `esparso`, `ColBERT`, `INT8`, `kb-bench`, `tools/bench`, `benchmark`, `gold set`, `gold.tsv`, `eval`, `avaliacao`, `measurement`, `medicao`, `medir modelos`, `score floor`, `piso de score`, `margin`, `margem`, `abstention`, `abstencao`, `abster`, `confidence`, `confianca`, `confidence gate`, `Memory::route`, `retrieval path`, `keyword scorer`, `RRF`, `fusion`, `fusao`, `CC-BY-NC`, `Apache`, `MIT`, `licenca do modelo`, `adversarial review`, `skeptic`, `cetico`, `prediction`, `previsao`, `mem0`, `Zep`, `competitors`, `concorrentes`, `hf-hub`, `symlink`, `top-1`, `miss log`, `latency`, `latencia`, `modelo na busca`, `custo do modelo`, `ADR-0018`

**Exists to:** Rerankers and dense heads measured in Rust and all kept out of retrieval, leaving the keyword score floor and runner-up margin as the confidence mechanism.

- **Date:** 2026-08-18
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible, and the instrument to re-decide is one Rust command

## Context

Richard asked the question that forced this: what improves the scoring and ranking that RRF computes
on its own, given that keyword routing has holes and mem0-class competitors ship embedding retrieval?
ADR-0017 had declined BGE-M3's dense head. Its gaps were named there: the sparse and ColBERT heads
were unmeasured, reranking was unmeasured, and the measurement lived in a throwaway Python
environment. Richard asked for the review to run in Rust, and to keep whichever model measured well.

`kb-bench` was built for it: a separate crate so `kb` keeps its one dependency, linking `kb` the
library so the corpus is loaded, chunked and routed by the exact code the real index uses. Before any
result was in, an adversarial review was commissioned against the reranker plan. Its prediction,
registered first: the reranker adds roughly zero on this failure set, because the measured failures
are recall failures the reranker never sees, and the calibrated abstention it would be bought for
already exists free in the keyword score that RRF discards.

## The measurement

Twenty real questions, 19 answerable, graded against the versioned gold set in `fleet/eval/gold.tsv` whose bias is
stated in its header (Zed wrote the questions and tuned the keyword lines against them). Corpus: 122
tracked files, 1,529 chunks through `kb::store::chunk`. Machine: the Latitude 3420, CPU only.

| Scorer | Top-1 of 19 | Score separates hit from miss? | Cost |
|---|---|---|---|
| **Keyword (current system)** | **16** | **Yes: one miss at 3.82, every hit >= 9.55** | microseconds, no model |
| BGE-M3 dense, map entries | 8 | no (gap -0.104) | 570 MB INT8 |
| BGE-M3 sparse, map entries | 10 | no (gap -0.071) | same pass |
| BGE-M3 dense, note bodies | 13 (Python fp32, ADR-0017; Rust INT8 cross-checked on entries) | no | 47 min index fp32 |
| BGE-M3 sparse, note bodies | **4** | no (gap -0.244) | same pass |
| bge-reranker-v2-m3 over keyword top-5 | **11** | no, worst of all (gap -7.546) | **2,571 ms per pair median** |
| jina-reranker-v2 over keyword top-5 | 14 | no (gap -1.599) | 647 ms per pair median |

Three findings the table compresses:

- **Both rerankers degraded the ranking they were given.** The input had the right file first 16
  times; BGE returned 11, Jina 14. On "quando eu escrevo um ADR" the correct `templates/adr.md`
  arrived first and was demoted. The skeptic predicted zero marginal gain and the reality was
  negative.
- **The sparse head inverts with document length.** 10/19 over short map entries, 4/19 over chunked
  bodies, where learned lexical weights light up everywhere and the largest file (`fleet/backlog.md`)
  absorbs questions that belong to other agents, including a question that belongs to another agent's base.
- **No model produced a usable abstention signal.** Every hit/miss score range overlapped. The only
  separation observed anywhere, all day, is the one the current system already computes and throws
  away at the fusion boundary.

## Options

### A. Adopt the best model anyway

Jina at 14/19 is the closest. It is still two answers worse than what it re-read, 647 ms per pair on
the miss-free path, and **CC-BY-NC licensed**, which forecloses shipping it in a product per
[[0008-single-user-open-source]] regardless of quality.

### B. No model in the retrieval path; promote the keyword score floor to a mechanism

Expose the top score and the runner-up margin that `Memory::route` already computes, and gate on
them: below the floor, the system says "guess" or "nothing" instead of answering with confidence.
Costs nothing at query time. The known weakness is that the floor is calibrated on one wrong answer
(3.82 against 9.55), so the gate has to ship together with its instrument: the miss log grows the
sample, and `kb-bench` re-runs the comparison when the corpus grows.

### C. Keep measuring models until one wins

The treadmill again, pointed at models instead of aliases. Nothing in today's data suggests the next
568M-parameter candidate behaves differently on a 122 file corpus, and the instrument exists to
detect when that stops being true.

## Decision

**Option B.** The models lost on accuracy, lost on abstention, and lost on latency, on our corpus, at
our scale, measured by the system's own code. The free signal won on the only axis nobody else could
even enter: knowing when it is wrong. What separates this system from mem0 and Zep is not retrieval
technique, it is that retrieval here states its own confidence honestly, and every model measured
today would have destroyed that property.

The next code change this decision orders: carry the keyword score and margin through retrieval
instead of discarding them at fusion, and act on the floor.

## Revisit trigger

- The corpus passes roughly 1,000 files, where hand written keys plausibly stop covering, then re-run
  `kb-bench` end to end. One command, no Python.
- A question set Richard produces, not Zed, scores the keyword system below any model column.
- The score floor produces a false abstention or a confident wrong answer on a real question, which
  the miss log will show.
- An embedding or reranking model appears that is Apache/MIT licensed, under 1 GB, and demonstrates
  hit/miss separation on someone else's benchmark, which is the one property no candidate had today.

## Notes

Instrument: `tools/bench` in the public repository. Models were fetched into a materialised hf-hub
layout because hf-hub 0.5.0 creates relative symlinks Windows then fails to resolve, killing its own
`assert!(pointer_path.exists())`; the workaround and the ColBERT memory blowup are documented in the
bench README and its commit. All model artefacts were deleted after the review, per Richard's
instruction; re-running downloads them again.

The adversarial review that predicted this outcome ran before the numbers existed. Worth repeating:
a skeptic commissioned against your own plan, whose prediction is registered before the measurement,
is the cheapest way this repository has found to avoid building the wrong thing well.
