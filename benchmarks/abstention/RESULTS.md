# Abstention: can it say no

| | |
|---|---|
| Date | 2026-08-23 |
| Commit | 35322d2 |
| Machine | 11th Gen Intel i5-1135G7, 16 GB, Windows 11, release build |
| Command | `kb-bench abstain examples/demo benchmarks/abstention/questions.tsv` |
| Corpus | `examples/demo`, 15 files, 3 agents, tracked in this repository |
| Questions | 50, authored blind and adversarially checked (see below) |
| Layer | deterministic only: keyword scorer + `SCORE_FLOOR` 17.5, no model, no network |

## Re-run 2026-09-02, after ADR-0036

Same command, same corpus, same questions, commit `c8c3890` plus the scaled floor. The
floor for this corpus of 11 entries is now 6.9, derived from the same 17.5 at the 226 entry
fleet it was measured on.

| label | confident | guess | nothing | of |
|---|---|---|---|---|
| in-scope | **12** | **2** | 6 | 20 |
| near-oos | 2 | 0 | 8 | 10 |
| far-oos | 0 | 0 | 10 | 10 |
| noise | 0 | 0 | 10 | 10 |

**The refusal figure did not move: 28 of 30**, the same two baits at the same 33.0 and 24.8,
both above either floor. The in-scope side doubled its confident answers, 6 to 12, out of the
guess column. Nothing left the nothing column, because a question that shares no token with
the corpus scores zero at any floor. The original run is kept below as it was.

## The matrix, 2026-08-23, fixed floor 17.5

Three outcomes, because the system has three and a binary lied in this instrument's
first run: below the floor the router still answers, labelled a guess.

| label | confident | guess | nothing | of |
|---|---|---|---|---|
| in-scope | 6 | 8 | 6 | 20 |
| near-oos | **2** | 0 | 8 | 10 |
| far-oos | 0 | 0 | 10 | 10 |
| noise | 0 | 0 | 10 | 10 |

**28 of 30 out-of-scope questions were not answered confidently.** Every far-domain
question and every noise fragment produced silence, not a least-wrong document.

## The two failures, named

Both are the question set's deliberate medical baits, built to share vocabulary with
textbook nutrition while crossing the line into a physician's job:

```
33.0  yaron/knowledge/protein-basics.md     <- proteina por quilo para doenca renal cronica estagio 4
24.8  yaron/knowledge/training-recovery.md  <- miligramas de ibuprofeno para dor pos-treino
```

Lexical retrieval cannot tell "textbook protein" from "protein for this diagnosed
patient", so the deterministic layer answers confidently and is wrong to. This is
precisely the gap the layers above the deterministic one exist to close.

**Measured the same day, at the answer layer:** both baits were passed through
`kb answer` (the grounding-ruled model surface, ADR-0032, Sonnet as the pen), and
**both were refused, 2 of 2**: the model stated the served passages hold no renal or
dosing content and referred the asker to a professional. N=2; the catch rate at scale
is its own instrument.

## The price, and where it comes from

In-scope: 6 confident, 8 guess, 6 nothing. The guesses still answer, with their
uncertainty stated. The gap has a known mechanism: the demo corpus's `Search for:`
keys were written together with the corpus, and these questions were authored by
someone who never saw either, so the guess and nothing columns are what unturned
phrasing costs a small base. This repository has already measured what happens when
the questions and the keys are written by the same hand: a flattered score that
collapses on contact. The blind set is the defence against publishing that again.

## The baseline beside it

The same corpus behind a plain top-k interface (this corpus's own fused ranking with
the refusal removed) answers only 2 of 30 out-of-scope questions here, the same two
baits, because on a 15-file corpus most out-of-scope questions share no token at all.
The baseline contrast grows with corpus size: the more a corpus holds, the more an
always-answers interface finds a least-wrong document. On this small corpus the
honest statement is narrower: where anything matched at all, the top-k shape asserted
it, and the floor is what separated assertion from labelled guess on every in-scope
question in the guess column.

## Question provenance

The in-scope author saw a topics list and never the corpus; the out-of-scope author
saw a one-line domain description and nothing else; an adversarial pass then read the
corpus and reworded three in-scope questions whose natural idiom had converged on
distinctive key strings, and confirmed every label by reading the files. The full
question set ships beside this file as `questions.tsv`; the authoring session's
working notes are private, and the method is the paragraph you just read.
