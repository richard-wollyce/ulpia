---
provenance: agent
stage: derived
---

# ADR-0019: the system is Ulpia, and the fleet is the folder the agents live in

- **Date:** 2026-08-18
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Steve (naming), Zed (execution)
- **Reversibility:** expensive to reverse. Richard bought `ulpia.io`, and a public name
  accretes references the moment it ships

## Context

ADR-0012 named the system and its orchestrator both Vesta. Two things changed since. The project
adopted the library metaphor as its working language, after an explanation written in those terms
(books, labels, catalog, librarian) landed better with laypeople than any technical description, and
Richard made it the standing vocabulary. And with the metaphor in place, one name doing two jobs
became a real cost: the librarian and the library are different things, and the pitch wants both.

Richard set the constraint and delegated the invention to Steve: a name from the semantic field of
libraries, books, labels and collections, with Vesta keeping the librarian role.

## What Steve's process produced

Steve's own criteria, applied: Wollner's rule that a mark must be abstract and created, never
literal, because literal marks converge until indistinguishable; the phonetic check from ADR-0012;
one spelling across Portuguese and English; and a collision search per candidate. Eight candidates,
six killed by the search, which is the process working:

| Candidate | Killed by |
|---|---|
| Tabularium | an existing product shipping documents-and-search APIs for AI workflows |
| Scrinium | a dormant ICO holding `scrinium.ai` |
| Pinakes | an existing multi-agent trust product, plus a fatal Portuguese phonetic collision |
| Pérgamo | Pergamum, the dominant Brazilian university library system |
| Nínive | an existing book-cataloguing app, plus two spellings across languages |
| Serapeum | a well known Common Lisp library |

Survivors: **Ulpia** (recommended) and Florilegium (runner up, whose meaning fits distillation
perfectly but whose five syllables break the family register beside Vesta, Zed, Steve and Yaron).

## Decision

**Ulpia.** Richard sealed it by buying `ulpia.io`.

The mechanism, not the vibe: the Bibliotheca Ulpia was Trajan's library, and it was simultaneously
Rome's public reading room and its official record office. That pair is literally this software,
knowledge plus decision records, which this repository formalised the day before by moving the ADRs
to the root. The word depicts nothing and is the generic term for nothing, which satisfies Wollner
where "Library", "Archive" and "Codex" fail by convergence. It is one string in both of the
project's languages, short, and Roman, the same register as Vesta: the Vestals guarded Rome's wills,
the Ulpia guarded its records, and the librarian-and-library sentence writes itself.

**What this supersedes in ADR-0012:** the system name, and the position that no new domain was
needed. `ulpia.io` is the product's home; `richardwollyce.com` stays the personal umbrella. What
survives from 0012 untouched: Vesta as the orchestrator's name, and the reasoning that produced it.

**The layout that follows**, Richard's design: the repository root directory takes the product's
name, and the agents' directory is renamed `agents/` to `fleet/`, because "fleet" describes the set
of agents, not the product. The code constant, every path join, the manifest comments, the README
and the skeleton documentation now say `fleet/`.

## Consequences

- Older checkouts with an `agents/` directory stop being recognised as fleet roots. Acceptable: the
  user count is one, and the rename is one command.
- Both directory renames could not be executed from inside the running session, because the serving
  process holds the index files open. They are the one manual step, documented in the migration
  notes, and everything else shipped ready for them.
- The tray still carries the identifier `com.fleet.tray` (Z16) and now also a stale product name;
  its rename remains a separate migration because the identifier decides where the pointer file
  lives.
- `hello@ulpia.io` is the public address, with `security@ulpia.io` aliased from day one, because a
  software project heading to public needs a disclosure door before it needs anything else.

## Revisit trigger

Shipping to a second real user under this name, which is when renaming stops being one command. Or
a trademark conflict surfacing in software retrieval or AI tooling, the classes where confusion is
arguable.

## Notes

The naming run and its collision evidence were produced through Steve's base on 2026-08-18; the raw
brief and shortlist are archived in Steve's inbox for distillation into his own knowledge.
