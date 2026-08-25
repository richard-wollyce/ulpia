# LongMemEval S, the full 500, first run

| | |
|---|---|
| Date | 2026-08-24 |
| Commit | c6ba44e |
| Machine | 11th Gen Intel i5-1135G7, 16 GB, Windows 11, release build |
| Dataset | `longmemeval_s_cleaned.json`, 500 instances, huggingface.co/datasets/xiaowu0162/longmemeval-cleaned |
| Command | `kb-bench longmem data/longmemeval_s_cleaned.json --answerer tools/answer-claude.cmd --judge judge-claude.cmd --workers 6` |
| Answerer | claude-sonnet-5, the shipped `answer-claude.cmd`, grounding rules unmodified |
| Judge | claude-haiku-4-5, **not the official protocol** (official judges with GPT-4o); `hypotheses-s.jsonl` ships for official re-judging |
| Ingestion | mechanical keys, the weakest honest ingestion; every number below is a floor |

## The scores, local judge

| ability | score | |
|---|---|---|
| **abstention** | **29/30 (97%)** | the column this system exists for |
| single-session-assistant | 51/56 (91%) | |
| single-session-user | 40/64 (63%) | |
| temporal-reasoning | 66/127 (52%) | |
| knowledge-update | 35/72 (49%) | |
| multi-session | 22/121 (18%) | mechanism below |
| single-session-preference | 3/30 (10%) | mechanism below |
| **TOTAL** | **246/500 (49%)** | |

## Read it in this order

**The abstention number is the point.** 29 of 30 unanswerable questions were answered
with "the history does not hold this" instead of a fluent invention, and nothing on the
answering side knows which questions those are; the refusal is the product's own
verdict plus the grounding rules, the same ones every `kb answer` call runs under.
LongMemEval's own paper reports abstention as the ability long-memory systems fail
hardest; no competitor's marketing quotes their abstention split at all.

**The total is a floor, and the floor is labelled.** Keys are generated mechanically
from each session's own vocabulary; a real fleet's keys are authored, and the
abstention benchmark one directory over measures what authored-vs-blind phrasing is
worth. The published vendor numbers for this benchmark (57.5 to 92 percent, each
self-judged under its own configuration, several of them mutually contradictory) are
not comparable to this run or to each other; this repository documented one of those
harnesses granting itself ten times the retrieval budget of its competitors, and
declines to join that genre. This number is reproducible from a clone.

**The weak categories have named mechanisms, not excuses.** `multi-session` (18%)
needs evidence assembled across many sessions, and the answer surface reads at most
five files, two passages each, a product default chosen for a personal fleet rather
than for cross-session aggregation. `single-session-preference` (10%) grades whether
the answer adopts the person's stated preferences, which the grounding rules actively
resist: the model is ordered to cite passages, not to roleplay from them. Both are
product decisions meeting a benchmark's expectations; changing them to chase the
score, then re-running, is the tuning this harness exists to refuse, so any change
lands as a product decision first and gets measured after.

## Addendum 2026-08-25: the mode ladder on multi-session

The 18 percent had a named mechanism (a five-file table starving aggregation), so the
mechanism was changed as a product decision first (ADR-0032's amendment: three modes,
caller-chosen) and measured after, with the mode declared here:

| run | mode | multi-session |
|---|---|---|
| full 500 | fast (default) | 22/121 (18%) |
| all 121 of the type | `--expanded` (12 files) | 36/121 (30%) |
| same first 30 ids, both modes | `--expanded` | 6/30 |
| same first 30 ids, both modes | `--complete` (whole base, map-reduce) | 9/30 |

On identical questions the complete read buys half again over the expanded table
(5 flips to correct, 2 away), at roughly seven model calls per question instead of
one. The residual ceiling is no longer retrieval: the detective read every session
and still missed 21 of 30, which is composition (counting and assembling across
extracted facts) plus the official rubric's strictness, which counts a partial
aggregation as wrong. That residual is the next mechanism to name, not a number to
massage.

**One instrument was rebuilt mid-run: none.** The run completed on the first attempt,
500 of 500, roughly one hour, six workers, on the machine above.
