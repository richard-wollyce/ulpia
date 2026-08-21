---
provenance: agent
stage: derived
---

# ADR-0030: two promoters, and the second one is not a second opinion

**Search for:** `promotor`, `promoter`, `promoters`, `dois promotores`, `two promoters`, `promocao`, `promotion`, `promover`, `promote`, `kb promote`, `ingestao`, `ingestion`, `ingestao de sessao`, `session ingestion`, `deposito`, `deposit`, `inbox`, `quarentena`, `quarantine`, `memoria episodica`, `episodic memory`, `memoria semantica`, `semantic memory`, `quem escreve`, `who writes`, `extracao automatica`, `automatic extraction`, `taxa de lixo`, `junk rate`, `97.8`, `mem0`, `issue 4573`, `dreaming`, `sonhar`, `sleep-time`, `consolidacao`, `consolidation`, `Letta`, `revisor`, `reviewer`, `revisao`, `review`, `lente`, `lens`, `lentes`, `lenses`, `contradicao`, `contradiction`, `duplicacao`, `duplication`, `escopo`, `scope`, `unanimidade`, `unanimity`, `maioria`, `majority`, `independencia`, `independence`, `entrada diferente`, `different input`, `captured`, `stage captured`, `kb-rejections.txt`, `rejeicao`, `rejection`, `recusa`, `gatilho`, `trigger`, `SessionEnd`, `ociosidade`, `idle`, `promote-claude.cmd`, `review-claude.cmd`, `Opus`, `Sonnet`, `ADR-0030`

**Exists to:** record why promotion is two models with different inputs rather than one model
checked by another, and what the second one is actually independent of

- **Date:** 2026-08-20
- **Status:** accepted
- **Scope:** fleet
- **Builds on:** [[0016-writing-a-note-includes-its-entry]], whose rule that a note and its
  keys arrive together is what makes a proposal reviewable at all, and
  [[0027-a-model-decides-who-answers]], whose process contract this reuses verbatim.
- **Reversibility:** reversible. Nothing else calls `promote.rs`, and deleting the two
  manifest lines turns the command off without touching a single note already written.

## Context

Richard: *precisamos de algo que "receba" esse amontoado de informações, e que a partir de
determinado trigger... elas de fato sejam revisadas, analisadas, decididas e ai sim gravadas
semanticamente.* And then, on the shape: *um outro promotor (colega de equipe do primeiro)
que revisa o trabalho dele... esse precisa ser mais competente ainda.* And on what matters:
*aqui o nosso objetivo nem é reduzir latencia mas sim uma qualidade absurda do que está
sendo gravado e mantendo coerência com o que já existia antes.*

The pressure behind it is a product decision, not a technical one. This base has always been
written deliberately, and [[ai-memory-layers-market]] recorded both the reason and its
boundary condition on 2026-08-13: *this analysis holds for a base written deliberately, at
low volume, for one person. It inverts at high volume with many users, where nobody can hand
write memories.* Ulpia now expects users on an API who will never open an editor. The
condition the note named has been crossed, so the decision it recorded has to be reopened.

What is not reopened is the measurement that produced it. A production audit filed as
[mem0 issue #4573](https://github.com/mem0ai/mem0/issues/4573) counted **10,134 entries over
32 days, of which 97.8 percent were judged junk.** That is what happens when the thing that
writes is also the only thing that decides, and no model fixes it, because it is not a
capability gap.

## The decision

**Promotion is two models with different inputs, not one model reviewed by another.**

Richard's first sketch was a colleague who reviews the first one's work. The sketch is
right about the split and wrong about what makes the second reader worth paying for. Two
models reading the same material reach the same mistake with twice the confidence.
[[letta-architecture]] already filed the verdict on that arrangement, about Letta's dreaming:
*a model reviewing a model is a second opinion from the same species of error.* Reviewing
someone's homework with the same homework in front of you is not independence.

So the second reader is given a **different input**:

| | promoter one | promoter two |
|---|---|---|
| reads | the deposit, `inbox/` | the proposal, plus what the base already holds |
| never sees | the base | promoter one's reasoning |
| asks | what here is worth keeping | is this new, and does it agree with what is written |
| model | Sonnet | Opus, on purpose |
| runs | once per deposit file | three times per proposal, one per lens |

The second reader's view of the base does not come from a model. It comes from
`Memory::ask`, our own router, asked with the proposal's own keys, returning the files that
already answer to those words together with the confidence evidence. That path has no model
in it, which is the only reason the word independent applies to anything here.

### The independence is a property of the type, not of the prompt

`promote::Proposal` carries agent, slug, folder, summary, keys, body and source. **It has no
field in which reasoning could travel**, and the reviewer prompt is built from that struct
and from the router's evidence and from nothing else. There is no version of this code where
somebody editing a prompt accidentally shows the reviewer the argument it is supposed to be
independent of.

That is pinned by a test, `a_proposal_carries_no_place_to_put_reasoning`, which fails if the
struct ever gains a field named for an argument. A rule that lives only in a comment is a
rule that survives until the next person is in a hurry.

### Three lenses rather than one stronger call

Latency is explicitly not the constraint, so the reviewer runs three times with three
different questions rather than once:

- **contradiction**: does this disagree with something the base already states
- **duplication**: is this already held
- **scope**: does this belong to this agent at all

Redundancy only catches what one reader misses when the readers differ. Three identical calls
to a strong model mostly agree with themselves, which measures determinism rather than truth.

### Unanimity, not majority

Any lens refusing is a refusal. Two of three would let a lens be overruled by two readers who
were never asked its question, which throws away the reason for asking three different
questions. A lens that did not answer at all is not a lens that agreed: a reviewer that
cannot be reached writes nothing, and an answer with no `VERDICT:` line is parsed as a
refusal. **Every failure mode of this command degrades toward writing nothing**, because it
mutates the base.

### A refusal is evidence

Refusals are counted in `kb-rejections.txt`, beside `kb-misses.txt` and for the same reason.
The same proposal refused three times by the same lens is not a bad proposal, it is the base
being asked for something it does not hold, or an agent being handed material that belongs
to somebody else. `misses.rs` makes that argument about questions nobody could answer; this
is the writing side of it.

### `captured` is a new rung and not a synonym for `distilled`

A promoted note lands at `stage: captured`. It was read by a model and reviewed by a model
and by no person. [[0007-memory-architecture]]'s rule that an agent claim is never quietly
promoted to human or external is untouched, and the word is what keeps it visible.

### The deposit stays invisible to the router

`inbox/` is exempt from the reachability check and absent from the index, and that is the
feature rather than an oversight. A base that answers from unreviewed material is the exact
failure this record exists against. `checks::is_exempt` and `promote::DEPOSIT` hold the two
halves of that rule.

## The options that were not taken

**Ingest straight into the base and clean up later with a dreaming pass**, which is what
EverOS (`reflect_episodes`, cron, default off), Synap (conscious forgetting) and Letta
(dreaming) all do. Buys: it scales without any gate, and it makes the whole of LongMemEval
runnable. Costs: it is the 97.8 percent arrangement, and the cleanup pass is a model
reviewing a model, so the thing meant to fix the problem shares its failure mode.

**One reviewer, stronger model, one call.** Buys: a third of the cost and a simpler command.
Costs: one question asked once. The three lenses were chosen because they fail differently,
and a single call has to hold all three at once, which is where a reader starts trading one
against another silently.

**Human approval on every promotion.** Buys: the strongest possible gate, and it is what
Richard has today by opening the file. Costs: it does not exist for an API user, which is
the entire reason this record was written. Kept as the outer gate rather than the only one:
`stage: captured` marks every note that no person has read.

## What is not built yet, and is named so it is not mistaken for built

- **The trigger.** Richard's design is idle detection: a session that stopped receiving
  input. In Claude Code that is a `Stop` or `SessionEnd` hook, not a cron, because
  ociosidade is observable and a clock is not. `kb promote` runs by hand today.
- **What writes into the deposit.** Nothing does. Files arrive there by hand. Session capture
  is the other half of the episodic story and is its own decision.
- **The self-limiting rule.** The measurement that makes automatic promotion safe is the
  junk rate of what it wrote, countable from `git diff` because the output is files. A run
  that exceeds a threshold should stop and wait for a person. The number is not yet chosen,
  and choosing it from zero runs would be inventing it.

## Consequences

Ulpia gains episodic capture without crossing the axis [[ai-memory-layers-market]] draws:
the machine may now propose, and it still may not decide alone. The cost is three Opus calls
per proposal, which is deliberate and is the answer to *qualidade absurda*, and a command
that refuses to run at all when either model is missing rather than degrading into the thing
it replaced.
