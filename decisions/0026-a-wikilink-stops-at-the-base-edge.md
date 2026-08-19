---
provenance: agent
stage: derived
---

# ADR-0026: a wikilink resolves inside its own base and nowhere else

- **Date:** 2026-08-19
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible in code, and the wrong direction is expensive: relaxing the
  rule later means auditing every existing link for what it started reaching.

## Context

Z38. Moving `yaron/profile/profile.md` into the shared person base broke its two
`[[labels-analysed]]` links, and that exposed a disagreement rather than a bug:

| | Rule it applied |
|---|---|
| `kb check` | resolve inside the base, otherwise E01 |
| `ui::resolve` | resolve at home, then anywhere in the fleet |

So the reading room drew edges the linter called broken, and the same file was
simultaneously fine and not. Richard: pick one rule and apply it in both.

## Options

### Option A: fleet-wide resolution, the reading room's rule

- Cost: `checks::run` takes one base and would have to take the fleet, which is an
  architecture change to the one function every base is graded by.
- **Failure mode, and it is disqualifying: it crosses a privacy boundary silently.** A base
  is exactly where privacy is decided here: `fleet/` is gitignored by the public
  repository, `yaron/profile/` is gitignored inside `fleet/`, and the decision records were
  released to the public root only after an audit that **edited eight files, converting
  wikilinks whose targets stay private into plain names**. Fleet-wide resolution makes that
  audit permanent work: every new link is a possible reference from a publishable file into
  a private one, resolving quietly, forever.
- Second failure: it has no correct answer when two bases hold the same stem, which they
  do. "Whichever discovery found first" is an ordering, not a rule.
- Third: it breaks the base as a unit. ADR-0008 says a base is addressed by path and may be
  opened alone, so **a link that resolves only when a sibling happens to be mounted is not
  a link, it is a coincidence of mounting.**

### Option B: base scope, the linter's rule, and cross-base references written as paths

- Cost: a reference to another base is longer to write, and moving a file between bases
  breaks its links home, loudly, at the moment of the move.
- Failure mode: someone writes the path wrong and gets no warning, because a path in prose
  is not checked. Real, and smaller than the alternative, since the mistake is visible in
  the text rather than hidden in a resolver.
- Forecloses: nothing. A checked cross-base link syntax could be added later.

## Decision

**Option B. The linter was right and the reading room changed.**

1. `ui::resolve` resolves inside the home base only. Both surfaces now agree about every
   file.
2. **Crossing a base is written out as a path**, which is what the publication audit
   already produced and what the fleet already contains.
3. **The ribbons in the stacks are rebuilt on that.** They marked "another agent works this
   document", detected through cross-base wikilinks, which this decision makes impossible.
   They now count written paths, and the change made them honest: they showed **zero**
   cross-base marks on the real fleet before, because the audit had already converted those
   links away, and they show a real one now. A feature whose data source was abolished by
   a rule is a feature that was measuring nothing.
4. **The linter teaches instead of only refusing.** When a broken link's target exists in
   another base, the message names it: *that note is in yaron, not here. A wikilink stops
   at the base edge, so write the path instead.* The rule does not bend; the error becomes
   actionable. A rule people cannot follow is a rule people work around.

## Consequences

- One rule, two surfaces, and a test in `ui.rs` that fails if they diverge again.
- Moving a file between bases now breaks its links **at the move**, visibly, instead of
  producing an edge in one tool and an error in the other.
- The private line gains a mechanical guarantee it did not have: **a wikilink cannot reach
  private material from a publishable file**, because it cannot reach out of its base at
  all. That is worth more than the convenience it costs.
- The convention is written into the agent instructions and the README, next to the link
  convention it qualifies.

## Revisit trigger

- A cross-base link syntax that is checkable, for example `[[base:note]]`, which would let
  the linter validate what is currently prose. Worth building the first time a written path
  goes stale without anything noticing.
- A base that stops being a privacy boundary, which would remove the argument that decided
  this. None exists today and the layout makes one unlikely.
