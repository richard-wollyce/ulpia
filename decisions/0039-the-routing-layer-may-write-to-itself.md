---
provenance: agent
stage: derived
---

# ADR-0039: the routing layer may write to itself, and what admits a line is a measurement rather than a person

**Search for:** `escrever alias automaticamente`, `write an alias automatically`, `alias sem pessoa`, `alias without a person`, `kb misses --apply`, `--apply`, `--gold`, `portao do alias`, `alias gate`, `auto aperfeicoamento do agente`, `agent self improvement`, `laco de correcao`, `correction loop`, `quem escreve o alias`, `who writes the alias`, `expansao aditiva`, `additive expansion`, `perda de precisao`, `precision loss`, `perda de recall`, `recall loss`, `so o miss e registrado`, `only a miss is logged`, `acerto confiante e errado`, `confident and wrong`, `registro de roteamento errado`, `misroute log`, `kb misroute`, `kb-misroutes.txt`, `kb-alias-rejections.txt`, `o agente reporta`, `the agent reports it`, `modelo propoe e nao decide`, `model proposes and does not decide`, `portao deterministico`, `deterministic gate`, `duas metades do portao`, `two sides of the gate`, `eficacia e nao dano`, `efficacy and no harm`, `coluna de abstencao`, `abstention column`, `interface igual design system`, `ADR-0039`

**Exists to:** record why the rule that only a person may write an alias was reversed, what replaced it, and why the replacement is a measurement rather than a second model

- **Date:** 2026-09-03
- **Status:** accepted
- **Scope:** fleet and tooling
- **Builds on:** [[0018-no-model-in-the-retrieval-path]], whose division between what a model may do and what arithmetic decides is the one this record moves one layer up. [[0030-two-promoters-and-the-second-is-not-a-second-opinion]], which gated ingestion the same way and is the precedent for refusing to run without a configured model. [[0031-three-species-of-evidence]], whose rule that ingestion may only ever produce memory is unchanged and still binds: nothing here writes a skill or a tool.
- **Reversibility:** the flag is one branch and reverts in one edit. Lines already written are text, each with the measurement that admitted it in a comment above it, so an undo is a delete with an audit trail. The misroute log is additive and reverts by deletion.

## Context

Richard, pushing back on a sentence that said an alias line is written by a person on
purpose: *voces como agentes tem direito e dever de se auto-aperfeiçoar quando algum
roteamento der errado, deveria ter uma função no nosso sistema que registra isso e leva
pra Vesta corrigir.*

He is right, and the rule he is pushing against was defended by a weak argument. The one
in `main.rs` said a machine that appends to `kb-aliases.txt` on evidence it produced itself
closes the loop with nobody in it. **The evidence is not produced by the machine.**
`kb-misses.txt` records questions a real person asked and this base really failed, which is
as external as evidence gets here. A test named
`there_is_no_way_to_apply_a_suggestion_from_this_verb` pinned the flag's absence and the
sentence together, which was good practice protecting a bad reason.

The argument that does hold was found by reading the code rather than the doc, and it is
about signal:

- `Memory::recall_loss` returns `None` unless the verdict is `Nothing`, so **only a total
  miss is ever logged.** A guess is not. A confident hit on the wrong file is not.
- Nothing anywhere recorded a misroute. `capture::note_routed` writes which agent answered
  and never whether that was right.
- **Alias expansion is additive**, so an alias cannot cause a miss. It can only ever cause
  a confident hit on something else.

Put together: the only feedback the system keeps is structurally blind to the only damage
an alias can do. A writer fed by that log would improve the column it can see and quietly
degrade the one it cannot, producing no evidence either way.

That is not hypothetical. `interface = design system`, one hand-written line in one base,
pulls every question in the fleet carrying the word `interface` to its author. Found on
2026-09-03 while measuring a new note, which reached rank 4 at 50.10 behind a file scoring
92.17 on `design system, interface, design, system` against a question containing neither
the word design nor the word system. It had been there for weeks and had produced zero
lines of evidence anywhere, because it never made anything missing.

## The decision, in three parts

### 1. `kb misses --apply` exists, and it cannot run without a gold set

The model proposes candidate lines; **the gate decides and the gate is arithmetic.** That
is the division [[0018-no-model-in-the-retrieval-path]] already draws through retrieval,
applied one layer up: a gate that shells out to a model gives a different answer on Tuesday,
and an admission that cannot be reproduced cannot be audited.

`--apply` without `--gold` refuses with exit 2 rather than degrading, and so does `--apply`
with no `promoter =` configured, on the same grounds as `kb promote`.

### 2. Two sides, and both are required

- **Efficacy.** The question the candidate was proposed for must stop missing. Without it
  the gate admits any line that is merely harmless, and harmless is not useful.
- **No harm.** No deterministic column of `kb eval` may drop: top-1 file, the keyword agent
  fold, and abstention answers. **The third is compared the other way round**, because
  answering more questions the set says to decline is harm and reads as improvement on
  every other column.

Rejected alternatives, since a rule asserted without its rivals is a preference:

- **A second model as the judge**, mirroring ADR-0030. Rejected here and not there: a note
  admitted wrongly costs the questions it later wins, which a reader can find; an alias
  admitted wrongly costs precision fleet-wide and silently, and the failure needs an
  instrument rather than an opinion.
- **Efficacy alone.** This is what a naive self-improvement loop is, and it is the exact
  failure the context section describes.
- **No harm alone.** Admits lines that change nothing, so the log never shrinks.

The first real run justified the shape within three questions: the proposer declined on
`isso ai` and `ok obrigado`, correctly refusing to invent aliases for conversational
filler, and its one real proposal was **refused by the abstention column**, which it moved
from 6 to 7. The gate caught, on its first run, the precise failure it was built for.

### 3. The misroute log exists, and the agent writes it, not Richard

`kb misroute <message> --chose <agent> --owner <agent|none>` records the half the miss log
cannot see, into `kb-misroutes.txt`, folded by count.

**The reporter is the agent because the agent already knows.** `kb boot` hands the winning
agent its constitution above a line saying the choice was the router's and to say so if it
is wrong. It does say so, in prose, and until this change nothing kept it. Waiting for
Richard to notice is strictly worse: he sees one session at a time, while the agent sees
the boot payload, the message and its own base in the same breath and is the only party in
the loop that can tell *this is not mine* at the moment it is true. So the boot message now
names the verb, and that is the whole of the change to the hot path.

**It is evidence and never action.** Nothing reads that log and edits a base. It widens
what the proposer can see and not what the loop may do, and those two staying apart is what
keeps the gate from being decoration.

## Consequences

- The routing layer becomes the second thing in this system that can grow without a human
  keystroke, and the first one that can do it against a measurement rather than a second
  opinion.
- Refusals are counted in `kb-alias-rejections.txt`, because a proposer that keeps offering
  the same refused line is a signal about the proposer. Same discipline as
  `kb-rejections.txt`, separate file, because two shapes in one log is a log nobody parses.
- Both new logs hold real questions verbatim and are gitignored in the same change that
  created them.
- **The gold set stops being a report and becomes a control surface.** Every question it
  does not contain is a question the gate cannot protect, so a thin set is now a safety
  property and not just a weak measurement.
- `there_is_no_way_to_apply_a_suggestion_from_this_verb` is re-pointed rather than deleted,
  to `applying_an_alias_cannot_lose_the_gold_set_that_makes_it_safe`. What was worth
  guarding was never the flag's absence. It was that the decision could not be removed by
  accident.

## Revisit trigger

- **The first admitted line that later turns out to be wrong.** It would mean the gold set
  was too thin to catch it, and the answer is a wider set rather than a wider gate.
- A misroute log that stays empty while misroutes keep happening in conversation, which
  would mean the agents are not calling the verb and the boot line is not enough.
- `kb-alias-rejections.txt` filling with the same line repeatedly, which is a finding about
  the proposer rather than about the base.
- Any proposal to let this loop write a `Search for:` line rather than an alias. That is a
  different blast radius, because a key is not additive, and it needs its own record.
