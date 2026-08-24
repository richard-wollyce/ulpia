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

## What this unlocks, named as unbuilt

LongMemEval grades free-text answers; this shim is the half that produces them. The
other half, converting chat-session histories into a fleet the shim can read, does not
exist, and the promotion pipeline is deliberately too expensive per note to be it. Both
stay unbuilt until decided.
