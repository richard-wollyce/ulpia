---
provenance: agent
stage: derived
---

# ADR-0038: a meaning tier sits after the verdict, and it ships with nothing in it

**Search for:** `meaning tier`, `camada de significado`, `tier de significado`, `suggester`, `sugeridor`, `Suggester`, `Trigram`, `suggester.rs`, `with_suggester`, `segundo scorer`, `second scorer`, `onde um modelo pode entrar`, `where a model may land`, `modelo depois do veredito`, `model after the verdict`, `traducao`, `translation`, `traduzir pergunta`, `cross language`, `cross lingual`, `idioma diferente`, `paraphrase`, `parafrase`, `pergunta em outra lingua`, `question in another language`, `nunca never`, `cognate`, `cognato`, `typo`, `erro de digitacao`, `trigram`, `trigrama`, `Dice`, `look_alike`, `SUGGEST_FLOOR`, `index::suggest`, `confidence_of`, `Memory::confidence_of`, `recall_loss`, `Verdict::Nothing`, `veredito`, `verdict`, `seam`, `costura`, `emenda`, `inert`, `inerte`, `candidato nomeado`, `empty slot`, `embedding local`, `local embedding`, `privacidade da consulta`, `query privacy`, `consulta sai da maquina`, `query leaves the machine`, `kb-aliases.txt`, `alias`, `alias table`, `tabela de alias`, `expand_query`, `index::expand_query`, `expansao de consulta`, `query expansion`, `kb misses`, `misses.rs`, `kb-misses.txt`, `log de perdas`, `recall loss log`, `estudo comparativo`, `comparative study`, `camada hospedada`, `hosted memory layer study`, `revisit trigger`, `gatilho de revisao`, `ADR-0038`

**Exists to:** Record that the slot where a meaning scorer would go already exists, that the ordering which makes it safe is a property of the call graph and not of a type, and that no model fills it until ADR-0018's bar is cleared by a named candidate with a measurement.

- **Date:** 2026-09-02
- **Status:** accepted. Nothing is built by it: the seam already exists and this record is what it is for
- **Scope:** system. The refusal path at every surface
- **Deciders:** Richard, Zed
- **Builds on:** [[0018-no-model-in-the-retrieval-path]], whose revisit trigger is the bar and is
  quoted verbatim below rather than summarised. [[0017-no-dense-scorer-yet]], which measured the
  candidate this would have used and recorded why the score overlap disqualified it.
  [[0032-the-answerer-sits-after-the-verdict]] is the closest precedent: it put a model after the
  gate too, and the line it defended is the line defended here.
  [[0006-language-architecture]] step 2, whose two halves this splits.
- **Reversibility:** nothing to reverse today. Deleting `suggester.rs` and calling `index::suggest`
  from `Memory::suggest` again restores the code exactly as it was, and loses only the statement
  of where a second scorer is allowed to land.

## Context

The hosted memory layer study of 2026-09-02 (`reports/2026-09-02-a-hosted-memory-layer-read-against-ours.md`) compared two memory layers
of the same shape and opposite bets, and found exactly one query shape their product is aimed at
and ours structurally cannot reach: **a paraphrase, or a question asked in one language against a
note whose keys were written in another.** Our own limit is ours to state, and the study states it
from our source rather than from a claim: `index::normalise` matches tokens exactly, and
`index::suggest` is a Dice coefficient over character trigrams at `SUGGEST_FLOOR` 0.65, so `nunca`
never reaches `never`.

The honest half of the comparison, carried here because dropping it would turn an inference into a
warrant: **that their product reaches that query is not sourced.** No page of theirs names an
embedding model or claims cross-language retrieval, and three shipped locales are a distribution
fact rather than a retrieval property. What the study establishes is that the query shape is real
and that ours refuses it. Nobody has measured anyone answering it.

The occupant of that slot today says so in its own reply text, on every surface. `main.rs:775-776`:

```
  that is spelling and not meaning, so it finds a typo or a cognate
  and never finds a translation.
```

and `mcp.rs:596-601`, to a model rather than to a person:

> That comparison is spelling, not meaning, so it finds a typo or a cognate and never finds a
> translation. If the question was asked in one language and the base was written in another,
> rewrite it with the terms above or with the canonical ones you expect, and ask again.

That sentence is a promise, and while trigram overlap is the only implementation it is true. It is
the first thing a meaning tier would falsify.

## The mechanism, which is the entire argument for the seam

**The verdict is computed from the keyword list alone, before anything else runs.**
`Memory::confidence_of` (memory.rs:1038) takes the keyword ranking and the text ranking and reaches
its answer from one number on one of them:

```rust
let verdict = if self.clears_floor(top.score) {
    Verdict::Hit
} else {
    Verdict::Guess
};
```

with `Verdict::Nothing` returned earlier still, from the empty ranking, before any score exists:

```rust
let Some(top) = hits.first() else {
    return Confidence {
        verdict: Verdict::Nothing,
        agreement: 0,
        keyword_score: 0.0,
        margin: 0.0,
        floor,
    };
};
```

`top.score` is `index::Hit::score`, produced by `index::route` from the keyword lines and the alias
table. The text ranking is read for one thing only, `agreement`, which is reported and does not
gate, and the function's own comment says why it does not. **So a scorer invoked after this point
is structurally incapable of turning a refusal into a hit.** It has no parameter to write to and
no caller that would read it if it did.

The ordering that puts a suggester after that point is `Memory::recall_loss` (memory.rs:828), which
consults the verdict before it asks for words:

```rust
if self.is_empty() || confidence.verdict != Verdict::Nothing {
    return None;
}
let looked_like = self.suggest(question, Self::SUGGEST_LIMIT);
```

`Memory::suggest` is the single call site, and it is one line onto a trait rather than a hard
reference to `index::suggest`, which is what makes "a second scorer lands here" a fact a signature
states instead of a fact somebody has to rediscover from the call graph. `Memory::with_suggester`
consumes the memory rather than borrowing it mutably, so no surface can swap the scorer between the
gate and the miss reply and have one question judged by one thing and answered by another.

Two tests hold it rather than the prose holding it.
`a_suggester_that_answers_everything_cannot_move_a_verdict` installs a suggester that answers every
question with two words trigrams would not produce, asserts every field of `Confidence` unchanged,
and then asserts the replacement actually ran, so the unchanged verdict is a measurement and not an
artefact of nothing having happened. `the_suggester_never_runs_on_a_verdict_that_was_served`
installs one that panics on sight and asks for a `Hit` and a `Guess`.

**So the tier changes what a refusal says, not what a verdict is.** The worst a wrong suggestion
can do is cost a reader one wasted retry after being told the base does not cover the question.

One thing this does not guarantee, stated here because `suggester.rs` states it and a record that
softened it would be lowering a bar: `Memory::suggest` is `pub`. A future caller could feed these
words back into a query before the gate runs, and nothing in the compiler stops it. **The safety is
in the ordering, not in the trait.**

## The deterministic half already crosses languages, and a person writes it

This matters because it makes the model an accelerator rather than the only path.

`kb-aliases.txt` and `index::expand_query` cross languages today, for every pair somebody has
written down. Expansion is additive by construction, so the mechanism has a bounded worst case: the
original words always survive, a wrong line can add noise and can never remove signal. Replacing
the query would have made a bad translation silently fatal, which is the reason the table can be
maintained by hand at all.

The table is deliberately not a dictionary. Its own header:

> **Only add a line after a real question missed.** This is a record of misses, not a dictionary,
> and a dictionary is what makes it unmaintainable.

What turns a lost question into the alias line to write is `kb misses`, the reader over
`kb-misses.txt` built in this same batch. It reads the log back most asked first, prints what the
base offered back and what nearly caught the question, and separates the two kinds of work: a
question with near misses is a keys problem, and one with none prints `nothing in the base comes
near it today: this is coverage, not keys`. It closes on the instruction and on the refusal to
automate it:

> Write the alias line, or add the key to that file's `Search for:` line, then delete the question
> from the log. This verb will not do it for you: kb-aliases.txt is a record of real misses, not a
> dictionary.

So the loop already closes: a question misses, the miss is counted, the reader ranks it by how
often it was asked, and a person writes one line that closes that question and every future
rephrasing of it, for free, forever, at zero query cost. A meaning tier would shorten the interval
between the miss and the line. It would not be the first thing to cross a language here.

## Options

### A. Pick a candidate now and ship it behind the seam

The seam is built, the ordering is tested, and the query shape is real. What is missing is the only
thing that ever mattered: a measurement. ADR-0018 measured six model configurations and every one
of them lost, including two rerankers that **degraded the ranking they were handed**, and none
produced a usable abstention signal. Shipping one now would be choosing on the strength of a
competitor's marketing page, against our own numbers.

### B. Ship the seam, ship no model, and name the bar

Costs nothing at query time, costs nothing to a base that opens today, and converts an undocumented
property of the call graph into a stated one. The cost is honest: the query shape stays unreachable
and the alias treadmill stays the only path, which ADR-0017 already accepted when it wrote that
"the cross lingual gap stays open".

### C. Delete the seam and keep calling `index::suggest` directly

The argument for it is that a slot invites something to be put in it. The argument against it is
what produced the slot: the first person to want a second scorer would otherwise have to
rediscover, from the call graph, which call sites are safe, and the surfaces where they would guess
wrong are the ones before the gate.

## Decision

**Option B. The seam exists, it is inert, and no model ships until ADR-0018's revisit trigger is
met by a named model with a measurement.**

The trigger, verbatim from ADR-0018, all four bullets, because quoting one and paraphrasing the
rest is how a bar gets lowered:

> - The corpus passes roughly 1,000 files, where hand written keys plausibly stop covering, then
>   re-run `kb-bench` end to end. One command, no Python.
> - A question set Richard produces, not Zed, scores the keyword system below any model column.
> - The score floor produces a false abstention or a confident wrong answer on a real question,
>   which the miss log will show.
> - An embedding or reranking model appears that is Apache/MIT licensed, under 1 GB, and
>   demonstrates hit/miss separation on someone else's benchmark, which is the one property no
>   candidate had today.

The fourth is the one that admits a model to this slot. Someone else's benchmark, because ours was
written by the person who tuned the keys against it and its bias is stated in its own header.

**Choosing that model is a measurement nobody has made.** No candidate has been named, nothing has
been downloaded since ADR-0018's artefacts were deleted on Richard's instruction, and this record
measures nothing. It records where the thing would go and what it would owe.

## What a candidate pays, when one is proposed

A checklist, so a proposal that skips a line is visibly incomplete rather than merely optimistic.

1. **A download, into a process that has none.** The README's one dependency claim is literally
   true today and would stop being true: ADR-0017 recorded that the production path is `fastembed`,
   which pulls `ort`, `tokenizers`, `ndarray` and `hf-hub`. The size limit is ADR-0018's, under 1 GB,
   and the licence limit is ADR-0008's, which is why jina-reranker-v2 was disqualified at 14/19 for
   being CC-BY-NC before its accuracy was even the argument.
2. **A residency cost on a four core laptop with no accelerator.** ADR-0004's mechanism is the one
   that decides this: prefill is compute bound and generation is memory bandwidth bound, and on this
   hardware the model is resident against the same RAM the fleet and the editor are using. A number
   measured on somebody's workstation does not transfer.
3. **A latency spike concentrated on the queries that already failed.** This is the one cost the
   ordering improves rather than worsens: it is paid only on `Verdict::Nothing`, never on a hit,
   never on a guess, so a warm hit stays where it is (warm route p50 0.68 ms in process over 1000
   samples, Windows laptop on `examples/demo`, 2026-08-23). It is also paid at the exact
   moment the person has already been refused once, which is the worst moment to add seconds. The
   proposal states the p50 and the p90 on the refusal path, on this laptop, or it is not a proposal.
4. **A similarity printed in units that cannot be mistaken for the evidence line's score.** Every
   reply already carries a keyword score against the floor that applied. A second number in the same
   reply, on a different scale, gets read by a model as a third scorer's opinion about the same
   question. It needs its own name, its own units, and its own sentence saying what it measures.
5. **It runs locally or it does not run.** A query sent off the machine violates query privacy,
   which is the property the whole product is built on, and the refusal path is the worst possible
   place to break it: `misses.rs` records that these are "the user's real questions, verbatim", that
   "a Yaron miss is a health question", and that the log is gitignored wherever it lands for exactly
   that reason. A meaning tier over a network would send the private half of the corpus to a vendor
   one question at a time. There is no version of this that is hosted.
6. **A diff to the promise, in the same commit.** "It finds a typo or a cognate and never finds a
   translation" is printed at `main.rs:775-776`, said in its own words at `mcp.rs:596-601`, and
   reproduced verbatim in `README.md:123-124` and in three built site pages
   (`site/frontend/docs/how-it-works/index.html` twice, `site/frontend/docs/local/index.html` once,
   each mirrored under `dist/`). A meaning tier falsifies that sentence everywhere it appears. It
   changes with the code or the product lies on six surfaces.

## Consequences

- No code changes today. The seam and its two tests already exist; this is the record they were
  missing.
- `kb-misses.txt` and `kb misses` stay the instrument that decides when this is reopened, which is
  what they were built for. A log that fills with paraphrase misses which alias lines keep closing
  is the treadmill converging. A log that fills faster than a person closes it is ADR-0018's first
  and third bullets arriving together, and it will be visible as a count rather than as a feeling.
- The trigram floor of 0.65 stays the weakest number in `index.rs` and stays tuned against a handful
  of pairs from one session, and a meaning tier arriving would not fix that, because it would sit
  beside spelling rather than replace it. A candidate that proposes replacing `index::suggest`
  rather than joining it is proposing to lose the typo, which is the half that works.

## Notes

Nothing was measured for this record. No model was downloaded, no candidate was named, and the only
command run was `cargo test` in `tools/kb`. Everything above about the code was read at this commit
and quoted from it: `memory.rs` `confidence_of`, `recall_loss`, `suggest` and `with_suggester`,
`suggester.rs` in full, `index.rs` `suggest`, `look_alike`, `SUGGEST_FLOOR` and `expand`,
`misses.rs`, and `cmd_misses` in `main.rs`. Everything about the subject is second hand, from a study
whose own method section says nobody installed the app.

The precedent this leans on hardest is ADR-0032, and the parallel is worth naming rather than
implying: there, a model was allowed to write prose only after retrieval had already decided, and
the defence was that a `Nothing` verdict never reaches the model, because fabrication needs a vacuum
and gets none. Here the vacuum is the same one, seen from the other side. The model would only ever
be reached **because** the verdict was `Nothing`, and all it may do with that is hand back words
from the base's own vocabulary for the reader to try again with.
