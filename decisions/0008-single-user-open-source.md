# ADR-0008: build for one self hosted user, keep the hosted service possible

**Search for:** `open source`, `self hosted`, `single user`, `multi tenant`, `hosted service`

- **Date:** 2026-08-13
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** this direction is reversible. Its opposite is not, which is the whole argument.

## Context

Richard confirmed both futures are real and they are not alternatives, they are a sequence:

- **A. Open source, self hosted.** Anyone clones the repository and runs it on their own machine, with
  their own base. This is what gets built and released.
- **B. A paid hosted service**, later, unnamed so far, for people who cannot or will not self host. We
  would run their base on our infrastructure and charge monthly.

The instruction for today is A only, with B kept possible.

## Decision

**Build for exactly one user, on their own machine, and let that user be Richard.**

Not as a simplification to be undone later. **As the shape of the product.** B becomes "we run A for
you", which is a hosting decision rather than an architecture decision, and that is the only version of
B worth offering.

### Why the order matters and cannot be reversed

**A is a directory. B is a database.** If a user's entire memory is a folder they own, then hosting it
later is straightforward and honest: we run the same software, they export by copying a folder, and
leaving costs them nothing. If we build multi tenancy first, going back to A means handing someone a
database dump and an apology.

That asymmetry is the same one in [[0003-knowledge-storage]], one level up. Files now and a service
later is an afternoon. A service now and files later is an export, a migration, and a loss.

**And it is the north star's durability clause made concrete.** Data the user can take with them is not
a feature we would add to B, it is a property A has for free and B would otherwise destroy.

### What building for one user actually means

| Decision | Consequence |
|---|---|
| No authentication, no accounts, no tenancy | Nothing in the code knows what a user is. The base is the folder it was pointed at |
| No server in the source of truth | Files and git. Anything that needs a daemon is derived and disposable |
| Everything runs offline | Per [[0004-local-first-inference]]. The network is an exception, never a step |
| No telemetry, no phoning home | An agent that reports on its owner is not the thing we are building |
| Configuration is a file in the base | Not a database row, not an environment service |

### What we do now so that B stays cheap later

Three things, each of which is good practice for A anyway. **Nothing here is built for B, only kept
uncontradicted by it.**

1. **The base is addressed by path, never assumed.** `kb` already takes a path argument. No code
   anywhere assumes there is one base, or that it lives at a fixed location. Multi tenancy later is a
   loop over directories rather than a rewrite.
2. **Nothing derived is ever the source of truth.** Already [[0003-knowledge-storage]]. It also happens
   to be what lets a hosted version rebuild any user's index from their files after any failure.
3. **`kb init` exists**, so a base can be created from a spec rather than by hand. Zed, Steve and Yaron
   were hand built, which is fine for three and impossible for three hundred. This is the
   productisation step and it is small.

### What we deliberately do not do now

No accounts, no tenant IDs threaded through code "for later", no abstraction layer over storage in case
we swap it, no configuration for a deployment mode nobody runs. **Every one of those is complexity
bought on speculation**, which `protocols/the-bar.md` names as the same failure as a lazy shortcut,
pointed the other way.

The honest way to keep B possible is to keep A clean, not to half build B.

## Consequences

- The public repository is the product. It has to be installable by a stranger, which means `kb init`,
  a README that assumes nothing, and no step that only works on this machine.
- The private layer stops being an implementation detail and becomes a promise: a user's `profile/` and
  `records/` are theirs, on their disk, and nothing in the software moves them anywhere.
- **The paid product is convenience, not capability.** Anyone who wants what we built can have it for
  free by cloning. That is a deliberate position and it is worth saying out loud, because it is the
  thing that makes the open source release honest rather than a funnel.

## Revisit trigger

- The first paying user, which turns B from a plan into a system with its own ADRs.
- Any requirement that cannot be met without knowing who a user is, which would be the first real
  pressure on this decision.
