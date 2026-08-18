# ADR-0002: the evidence ruler for claims about software

- **Date:** 2026-08-13
- **Status:** **accepted**, ratified by Richard 2026-08-13.
- **Scope:** repository
- **Deciders:** Richard, Zed
- **Reversibility:** reversible now, expensive later. Every note written under a ruler carries that
  ruler's grades, so changing it after fifty notes means regrading fifty notes.

## Context

Richard said the rulers are not defined yet and that Zed takes part in defining them. Both sibling
agents have one and both work differently: Yaron grades the source once, A to D, and the grade travels
with every recommendation derived from it. Steve checks each claim at its primary source and records
the finding next to the analysis.

Software needs both mechanisms plus two checks neither domain requires:

- **Staleness.** A correct claim rots on a version bump. Nothing in nutrition changes because a vendor
  shipped a minor release.
- **Scale mismatch.** A claim can be true at the scale it was written for and harmful at ours. Most
  published architecture advice comes from systems orders of magnitude larger than anything we run.

And software has one advantage the others do not: most claims here are executable. You can stop
arguing and measure.

## Options

### Option A: adopt Yaron's A to D unchanged

Cheap, consistent across the fleet, one vocabulary. Fails to catch the two traps above, which are the
two ways software knowledge actually goes wrong.

### Option B: A to D, redefined for software, plus two independent checks

Tiers redefined around what is verifiable here (ran it, source and spec, docs and reproducible
benchmark, practitioner report, unsourced), then staleness and scale checked separately, because a
tier A claim can still be wrong for us on either.

### Option C: no tiers, only "verified" and "unverified"

Simplest and honest, and it loses the distinction between a maintainer's statement and a tweet, which
is the distinction that does most of the work when we cannot measure.

## Decision

**Option B**, drafted in [`../knowledge/evidence/evidence-tiers.md`](../knowledge/evidence/evidence-tiers.md)
as v0.1, including one rule that matters more than the tiers themselves: **a language model's output,
including Zed's own, is tier D until confirmed by something above tier C.** Fluent, confident and
unverified is exactly what tier D looks like from the inside, and that is the failure mode closest to
home.

## Consequences

- Every knowledge note carries a tier, a `valid_for` version and a recheck trigger in its front
  matter.
- Recommendations built on tier C or D announce themselves as such.
- Slower ingestion, because "run it and measure" is a real cost. That cost is the point: it is what
  separates the base from a collection of blog posts.

## Revisit trigger

The first time a tier A note turns out to be wrong for us. That will say more about the ruler than any
argument now.

## The three application questions, resolved 2026-08-13

- **Tier on the note, plus a per claim tag only when a claim sits below the rest of the file.**
  Tagging everything produces noise, and noise gets ignored, which is worse than no tag.
- **Recheck fires on a major version bump or on use, never on a calendar.** A fixed interval creates a
  review queue nobody works, and an unexecuted policy is worse than none because it implies the base
  is current.
- **Tier D material is recorded in the discard log only**, never as a recommendation and never as
  background a later reader could mistake for a finding.

Full text in the note itself.
