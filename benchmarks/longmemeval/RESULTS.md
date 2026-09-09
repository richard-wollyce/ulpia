# LongMemEval S, the full 500, first run

| | |
|---|---|
| Date | 2026-08-24 |
| Commit | c6ba44e |
| Machine | 11th Gen Intel i5-1135G7, 16 GB, Windows 11, release build |
| Dataset | `longmemeval_s_cleaned.json`, 500 instances, huggingface.co/datasets/xiaowu0162/longmemeval-cleaned |
| Command | `kb-bench longmem data/longmemeval_s_cleaned.json --answerer ../../tools/answer-claude.cmd --judge judge-claude.cmd --workers 6`, from `benchmarks/longmemeval/` |
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

## Addendum 2026-08-25, later: the autopsy, the fixes, and the re-measure

A traced re-run of the same 30 questions (scoring 8/30 where the untraced ladder
run above scored 9/30: one question of run-to-run model variance, and the autopsy
uses the run whose intermediates exist) wrote every intermediate to disk (per-batch
map replies, the fact sheet the reduce saw, the final answer), and a five-agent
autopsy classified all 30 against the dataset's own answer_session_ids. The verdict
overturned the working theory: **91 percent of failures were extraction, not
composition**. A map batch containing a gold file replied NONE while the file plainly
held the evidence; the arithmetic over what arrived was almost always right.

Three fixes shipped as product decisions (ADR-0032), none tuned to this benchmark:
a per-file verdict rule in the map (a batch-level NONE is no longer a legal output),
the session date on every extracted fact line, and an enumerate-then-commit scaffold
in the reduce (the ANSWER line is mandatory and precedes any caveat; refusal remains
a legal answer). Re-measured on the identical 30 questions, same judge, same seed
material:

| | before | after |
|---|---|---|
| extraction recall of gold files into the fact sheet | 60/100 (60%) | **91/100 (91%)** |
| multi-session, complete mode, same 30 ids | 8/30 (27%) | **17/30 (57%)** |
| flips | | 11 to correct, 2 away |

One of the two regressions is the autopsy's named lucky hit (right number from wrong
evidence), which the do-not-fix section ordered left unprotected; losing it is the
fix being honest. The residual 13 misses now sit above a 91 percent extraction floor,
which puts the next mechanism genuinely in composition and rubric territory for the
first time.

**One instrument was rebuilt mid-run: none.** The run completed on the first attempt,
500 of 500, roughly one hour, six workers, on the machine above.

## Addendum 2026-08-25, evening: the declared-modes run

The mode is the caller's choice, so the honest full-500 run declares a mode per
question nature and says so here: multi-session ran `--complete`, because
aggregation is what the whole-base read exists for and what a real caller would
buy for it, and everything else ran the factory default. Two runs, one harness
flag apiece (`--type multi-session` / `--skip-type multi-session`), same judge,
same mechanical-keys ingestion floor as the first run.

| ability | mode | first run, all fast | declared modes |
|---|---|---|---|
| abstention | mixed | 29/30 (97%) | 28/30 (93%) |
| multi-session | complete | 22/121 (18%) | **81/121 (67%)** |
| temporal-reasoning | fast | 66/127 (52%) | 66/127 (52%) |
| knowledge-update | fast | 35/72 (49%) | 36/72 (50%) |
| single-session-assistant | fast | 51/56 (91%) | 52/56 (93%) |
| single-session-user | fast | 40/64 (63%) | 40/64 (63%) |
| single-session-preference | fast | 3/30 (10%) | 2/30 (7%) |
| **TOTAL** | | **246/500 (49%)** | **305/500 (61%)** |

Read the flat rows as the control they are: every fast-mode ability moved within
noise, which is what should happen when the fixes live entirely inside complete
mode's map and reduce. The one abstention lost (a multi-session trap answered
instead of refused) is the cost of reading everything: more material is more
temptation, and 11/12 under the detective read is the gate holding at 93 percent
instead of 97. The multi-session jump from 18 to 67 percent is the mode plus the
extraction fix measured on the full population, and it confirms the 30-question
sample (57 percent) instead of flattering it.

`hypotheses-s-declared.jsonl` is this run's file and the one worth judging
officially. `hypotheses-s.jsonl` stays as the first run's, because a floor that
was published stays published.

## Addendum 2026-09-08: the judge, validated

Every score above rests on one grader that nothing was checking. Graphify publishes
90.6 percent agreement at Cohen's kappa 0.81 between its judge and a second independent
one, and that is the difference between a number and an auditable number. This run closes
half of that gap and reports honestly that the other half is still open.

Two new deterministic subcommands do the work, and the arithmetic in them is tested rather
than asserted: `kb-bench judge` grades an existing hypotheses file with any judge and
**writes the per-question labels**, which the published run threw away, and
`kb-bench agree` pairs two label files into raw agreement, Cohen's kappa, the 2x2 table,
the split by ability, and every disagreeing question by id.

**The sample, and why it is this one.** 125 of the 500 questions: every fifth question of
`hypotheses-s-declared.jsonl` (100), plus all 30 abstention instances, because abstention
is 6 percent of the set and the number this benchmark leads with, and a stride alone would
have left six of them. At an expected agreement near 0.9 the standard error of kappa is
about `0.6 / sqrt(n)`, so 125 questions place it within roughly plus or minus 0.11 at 95
percent: enough to separate substantial from moderate, not enough to publish a figure to
two decimals.

### 1. The published judge is reproducible

`claude-haiku-4-5`, the same `judge-claude.cmd` the published run used, over the same 125
hypotheses twice:

| | |
|---|---|
| questions graded in both runs | 122 |
| raw agreement | **99.2%** |
| Cohen's kappa | **0.981**, almost perfect |
| disagreements | 1, on `knowledge-update` |
| abstention disagreements | 0 of 29 |

That number did not exist before tonight, and it is the one Graphify does not publish
either. It bounds everything else: **no cross-judge kappa can exceed a judge's agreement
with itself**, so measuring it first is what makes a cross-judge number readable.

### 2. The cross-judge check did not validate, and the reason is the second judge

`claude-sonnet-5` as the second grader, identical prompt through
`longmem::judge_prompt`, because two judges given two prompts measure prompt sensitivity
and not judge agreement:

| | |
|---|---|
| questions graded by both | 123 |
| raw agreement | 78.0% |
| Cohen's kappa | 0.570, moderate |
| disagreements | 27 |
| direction | **27 of 27 the same way**: Haiku said correct, Sonnet said wrong. Never once the reverse |
| abstention disagreements | 14 of 30 |

A one-directional disagreement of that size reads as a leniency bias in the published
judge, and on the sample the two graders score abstention 29/30 against 15/30. So the
second judge was run twice over the 30 abstention questions before anything was concluded
from that:

| | |
|---|---|
| questions | 30 |
| raw agreement with itself | **60.0%** |
| Cohen's kappa with itself | **0.200**, slight |

**`claude-sonnet-5` through this harness is not a usable judge**, and the 0.570 above is
mostly its own noise rather than evidence about the primary judge. One case, worked
by hand: `19b5f2b3_abs` asks how long the person was in Korea, the answer says the
passages hold no Korea trip at all, and the second judge graded that same input `Yes`
three times and `No` twice. The primary judge graded the whole 122 the same way twice.

### What this changes, and what it does not

- **Nothing above this addendum moves.** The scores were graded by the judge that has now
  been measured as reproducible, and no more comparable judge has overturned any of them.
- **We still cannot publish a Graphify-style independent-judge kappa.** The honest claim
  today is test-retest reliability, not inter-rater reliability, and the two are not
  interchangeable. Saying otherwise would be the exact thing this file exists to refuse.
- **The unvalidated-judge gap named in `graphify-benchmark-claim` is half closed.** The
  judge is stable. Whether it is right is a different question, and it is the one that
  needs a second grader that can agree with itself.

### The costed recommendation, not run

The right second judge is the official protocol already sitting in this directory:
`rejudge.cmd` runs the paper's own `evaluate_qa.py` with `gpt-4o` at `temperature: 0`,
which is deterministic by construction and is the grader every published LongMemEval
number is compared against. It needs `OPENAI_API_KEY`, which is the owner's and is never
handled by an agent, so it is a decision and not a step.

Costed, so the decision has a number: 500 hypotheses at roughly 285 tokens of prompt and
10 of output is about 143k input and 5k output tokens, which is a few dollars at gpt-4o
list rates. Judging only the 125-question validation subset is a quarter of that. The
alternative, majority-voting three `claude-sonnet-5` calls per question to buy stability,
costs three times the second-judge run and still leaves both graders inside one model
family, which is a weaker independence claim than the official protocol for more money.

**What tonight's judge calls cost, marked as an estimate.** One `claude -p` call on
`claude-haiku-4-5` was measured through `--output-format json` at $0.0197, almost all of
it the CLI's own system prompt rather than the 285-token grading prompt. 415 judge calls
were made (260 Haiku, 155 Sonnet), so the run is on the order of ten dollars at list
rates and less with prefix cache reads. That is arithmetic from one measured call, not a
billed figure.
