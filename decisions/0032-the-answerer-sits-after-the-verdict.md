---
provenance: agent
stage: derived
---

# ADR-0032: the answerer sits after the verdict, and the refusal survives it

**Search for:** `answerer`, `kb answer`, `respondedor`, `camada de resposta`, `answer layer`, `answer shim`, `shim de resposta`, `resposta com citacao`, `answer with citations`, `resposta fundamentada`, `grounded answer`, `grounding`, `fundamentacao`, `passagens servidas`, `served passages`, `citar arquivo`, `cite the file`, `alucinar resposta`, `fluent fabrication`, `fabricacao fluente`, `recusa na resposta`, `refusal in the answer`, `biblioteca nao contem`, `library does not hold`, `modelo depois do veredito`, `model after the verdict`, `prosa do retrieval`, `prose from retrieval`, `longmemeval`, `benchmark de resposta`, `answer benchmark`, `answer-claude.cmd`, `sonnet como escritor`, `bait medico segurado`, `medical bait caught`, `ADR-0032`

**Exists to:** record where the answering model sits, why ADR-0018 is untouched by it, and how the refusal is made to survive prose

- **Date:** 2026-08-24
- **Status:** accepted
- **Scope:** fleet
- **Builds on:** [[0018-no-model-in-the-retrieval-path]], untouched: the model reads what
  retrieval already found, never participates in finding it. [[0027-a-model-decides-who-answers]]:
  the same process contract, a command in the manifest, prompt on stdin, text on stdout.
- **Reversibility:** delete the `answerer =` line and `kb answer` degrades to the reading
  list `kb route` prints. Nothing else consumes the module.

## The decision

`kb answer` produces prose from a question: retrieval runs first, deterministic and
unchanged, and the model named by `answerer = ...` receives the question, the served
passages, and the gate's own evidence line, under hard grounding rules: every claim
cites a served file, and "the library does not hold this" is a correct and complete
answer. Three refusals wrap the one model call, in order: a `Nothing` verdict never
reaches the model (fabrication needs a vacuum and gets none), a missing answerer prints
the reading list, and a failed call prints the reading list out loud. The caller prints
`sources served:` itself from retrieval's own list, so a fabricated citation has nowhere
to hide.

## Measured on arrival, 2026-08-24, release build, Sonnet as the pen

The abstention benchmark had left exactly two out-of-scope questions answered
confidently by the deterministic layer, both deliberate medical baits (protein dosing
for stage-4 kidney disease at score 33.0, ibuprofen daily limits at 24.8). Passed
through the answer layer, **both were refused, 2 of 2**: the model stated the passages
hold no renal or dosing content, and referred the asker to a professional. The
end-to-end abstention story is now: the deterministic layer silences 28 of 30, and the
grounding rules caught the two that leaked. N=2 and the pen was Sonnet; the catch rate
at scale is its own instrument, not this record's claim.

## Amended 2026-08-25: three table sizes, because one default lied on aggregation

LongMemEval's multi-session split measured the lie: with the answer surface reading
five files, questions whose answer is crumbs across a dozen sessions scored 18
percent, not because retrieval ranked wrong but because most right files never
reached the table. Richard's ruling: three modes, chosen by the caller, never guessed.

- **fast** (default): the librarian's answer. Top five files, one call.
- **`--expanded`**: the bigger table, up to twelve files, one call.
- **`--complete`**: the detective's answer. Every keyed file the fleet serves, read in
  batches (map: extract facts, each cited to its file, or NONE) and composed (reduce,
  under the same grounding rules). Costs one call per batch plus one, and **the
  estimate is the mode's contract**: it prints before the first call, restates after
  the first timed batch, and leads the final output, because on surfaces where no
  person watches a screen the model reading the output deserves the warning a person
  got. The UI shows it on screen when it grows the control; until then that is a
  backlog item, not a claim.

Changing a mode's numbers to chase a benchmark and re-running is the tuning the
harness refuses; a change lands as a product decision here first and is measured
after, with the mode declared in the run's configuration header.

## Amended 2026-08-25, later: the map answers per file, and the reduce commits

The traced autopsy of the multi-session failures found 91 percent were a map batch
answering NONE for a batch containing the evidence: skimming made silent. Three
changes, approved as product decisions and measured on the identical 30 questions:
the map must emit a verdict per file (batch-level NONE stopped being a legal output),
every fact line carries its session date, and the reduce enumerates candidates then
emits a mandatory committed ANSWER line before any caveat, with refusal still legal.
Extraction recall of gold evidence went 60 to 91 percent and the score 27 to 57
percent, with the one preserved failure mode being the lucky hit the autopsy ordered
unprotected. The relevance criterion stays generic by rule; nothing in the prompts
names a benchmark's categories.

## What this unlocked, updated as it landed

LongMemEval grades free-text answers; this shim produces them, and the other half,
the session-to-fleet converter, landed the next day as `kb-bench longmem`
(mechanical keys, declared as the weakest honest ingestion). The full 500 ran on
2026-08-24: 49 percent total under the local judge, 97 percent on abstention, per
`benchmarks/longmemeval/RESULTS.md`. The promotion pipeline remains deliberately too
expensive to be benchmark ingestion, which is why the harness has its own.
