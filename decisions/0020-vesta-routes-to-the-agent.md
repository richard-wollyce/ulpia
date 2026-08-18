---
provenance: agent
stage: derived
---

# ADR-0020: Vesta chooses which agent answers, and each scorer does the job it was measured to be better at

- **Date:** 2026-08-18
- **Status:** proposed
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible for the routing behaviour, which is one boot file and one
  function. **Not reversible for the split inside retrieval**, because callers will start
  depending on which scorer answers which question.

## Context

Richard asked why a conversation opened in this repository becomes Zed without Vesta
choosing. The answer is that nothing routes. The public [`CLAUDE.md`](../CLAUDE.md)
carries a static conditional, *when `fleet/zed/` is here, read its `CLAUDE.md` and follow
it instead*, and that names one agent literally. It never reads the question. Ask it about
protein and you still get the architect.

Vesta cannot fix this today because Vesta answers a different question. `kb route` answers
*which files should this question open*. *Which agent should answer this conversation* is
not the same question and nothing computes it. [[0013-retrieval-precedes-classification]]
made the router the first door and left this half empty on purpose; the roster records the
coordinator as planned with Richard holding the role; Z8 says `kb` is not wired to
anything.

His second instruction was the useful one: **measure whether routing actually beats the
hardcoded line**, on speed and on quality, rather than assuming that dynamic beats static
because it sounds better.

## What was measured

`kb eval`, built for this and shipped in the same change, against the 19 answerable
questions of the gold set, release binary, 92 map entries across 5 bases, 2026-08-18. The
instrument is one command with no model and no network.

**First, the gold set was wrong and would have produced a confident false number.** Twelve
of its answers still pointed at `zed/decisions/...` after the records moved to the fleet
root the day before. Graded in that state it would have scored twelve correct answers as
misses. `kb eval` now refuses to grade until every gold path resolves, which is the only
reason the rest of this table can be trusted.

### The result that decides the design

| | picks the right file | picks the right agent |
|---|---|---|
| keyword scorer alone | **18/19 (95%)** | **16/19 (84%)** |
| the two scorers fused by RRF | 11/19 (58%) | 13/19 (68%) |
| always Zed, the best fixed choice on this set | n/a | 10/19 (53%) |

**Fusion is materially worse at picking a single winner, and the mechanism is RRF working
as designed.** RRF rewards agreement: a file both scorers rank fourth beats a file one
scorer ranks first. That is exactly what you want when assembling passages for a person to
read, because a file both scorers noticed deserves to be in front of them. It is the wrong
rule for choosing one owner, and it halves top-1 precision to buy recall nobody asked for
at that step.

So the two scorers are not two attempts at one job, and the system had been treating them
as if they were.

### The gate

| | |
|---|---|
| keyword hit scores | 9.29 to 179.24 |
| keyword miss scores | 0.00 |
| misses the gate flagged as a guess | 1/1 |
| correct answers the gate demoted | 0/18 |
| the abstain question | correctly not called a hit |

The floor separates completely on this set. **The margin over the runner-up does not, and
that was a prediction of mine that the instrument refuted on its first run.** I argued that
an IDF weighted sum scales with the query so a fixed floor is fragile and the scale free
margin should be the primary gate. Correct answers turned out to have margins of 1.00,
1.12, 1.16, 1.18, 1.19, 1.21, 1.30, 1.30, 1.39, 1.44, 1.86, 2.20, 2.61, 2.71, 3.34, 4.00
and 7.00. No cut in that range avoids throwing away correct answers, and 1.5 threw away
twelve of eighteen. The reasoning failed because the floor is not ranking hits against each
other, where query scale would matter. It is asking whether any meaningful term matched at
all, and the keyword scorer gives a file no score whatsoever unless one did, so the real
distribution is a gap between zero and the first real match and query scale moves both
sides of it together.

### Speed

| | per question, release binary |
|---|---|
| keyword only, which is what routing needs | **8.6 to 10.5 ms** |
| both scorers fused | 14.1 to 17.6 ms |
| a local 4B deciding instead | seconds, per [[0004-local-first-inference]] |

The range is machine load, not variance in the work. **And it is roughly nine times the
"about a millisecond" this repository's README claims**, because `index::route` rebuilds
the prepared entry list and the document frequency table on every call, which is O(entries)
per query at 92 entries. That is the number that will bite at the 1,000 file re-measure
[[0018-no-model-in-the-retrieval-path]] already asked for, and it is a cost of the current
implementation rather than of the design.

## Options

### Option A: keep the hardcoded pointer

Agent selection stays a human act: open the folder of the agent you want.

- Cost: measured at 10/19, and that is against a question set whose largest single owner is
  the agent the line names. On Richard's real mix it is worse, because the line is right
  exactly as often as he happens to want the agent it names.
- Failure mode, and it is the disqualifying one: **it is silent.** The wrong specialist
  answers confidently from the wrong base, and nothing in the transcript says a choice was
  made at all.
- Forecloses: nothing. It is where we are.

### Option B: Vesta routes, hands off, and leaves

The first message is routed. The winning agent's constitution is loaded and that agent
holds the conversation.

- Cost: one keyword pass, 8.6 to 10.5 ms, once per conversation.
- Failure mode: the conversation changes domain at message three and the boot is stale.
  Recoverable, because the router can be re-run per message for another 9 ms and the gate
  says when it is unsure.
- Forecloses: nothing. Option C remains buildable on top.

### Option C: Vesta routes every message and stays in front

Vesta is the only agent addressed; it loads the winning agent's constitution as context per
question rather than becoming that agent.

- Cost: the same 9 ms, plus reloading a constitution on every topic switch.
- Failure mode: the agents lose their distinct voice. Vesta doing an impression of Yaron is
  worse than Yaron, and the whole argument for a fleet of specialists is that they are
  actually different.
- Forecloses: the agent level autonomy limits, which are per agent files today.

## Decision

**Option B, and the split inside retrieval that makes it work.**

1. **Agent selection and the confidence verdict are computed from the keyword ranking**,
   not the fused one. 16/19 against 13/19, for free, from a list already being computed.
2. **Passages stay fused**, because that is what RRF is better at and the measurement says
   so in the other direction.
3. Both come back from one call, `Memory::ask`, so no caller can pair them differently.
   Before this, `confidence` read the keyword score of *fusion's* pick, which is neither
   number and was nobody's intention.
4. **The gate is the floor alone.** The margin is measured and reported as evidence and
   does not decide. Agreement is reported and does not decide either, because the one
   measured wrong answer, "quem e voce?" at 3.82, had *both* scorers voting: agreement is
   evidence that a file is on topic and no evidence that the topic is covered.

Option B beat Option A by six questions out of nineteen against the strongest fixed choice
available, at 9 ms, with an abstention signal that flagged its own only miss. It beat
Option C because the cost of C is the thing the fleet exists for.

**What this does not decide:** the boot mechanism itself. Making `CLAUDE.md` call the
router is a change to how every conversation in the system starts, and it is not covered by
this record.

## Consequences

- `Memory::ask` becomes the entry point every surface should use. `retrieve` and
  `confidence` stay for the tray and the existing MCP path and are now the older door.
- **The public README's "about a millisecond" is wrong by roughly 9x** and has to be
  corrected or the claim withdrawn. It is a published number about software we are asking
  people to trust on the grounds that it is measurable.
- The keyword scorer is now load bearing for two decisions rather than one, which raises
  the cost of a bad `Search for:` line. An entry with no keywords was already unreachable;
  now it also cannot own its agent.
- `kb eval` becomes the thing that has to pass before a retrieval change lands, and it is
  the first check in this repository that grades behaviour rather than form.

## Revisit trigger

- **The gold set is graded by its author and tuned against itself.** The keyword numbers are
  flattered and the note at the top of the file says so. The first measurement on questions
  Zed did not write replaces this table rather than supplementing it.
- The negative sample is **n=2**: one keyword miss and one abstain question. The floor
  separates perfectly on a sample that small by construction. Ten real misses is the number
  at which it means something.
- Any fleet where one base grows past roughly half the entries, at which point per agent
  sums favour the largest base for reasons that have nothing to do with the question. The
  sparse head already failed this way at ADR-0018.
- The 1,000 file re-measure, where the O(entries) per query cost is the thing to watch.

## Notes

Measured with `kb eval fleet/zed/fleet/eval/gold.tsv . --all` on the release binary built
into a scratch target directory, because a running MCP server holds `kb.exe` open, which is
Z17. The 151 tests pass. The gold file lives in the private repository because its questions
quote real private notes; the instrument that reads it is public.
