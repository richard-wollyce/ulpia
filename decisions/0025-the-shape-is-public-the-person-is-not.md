---
provenance: agent
stage: derived
---

# ADR-0025: a fleet must declare its human, and the shape of that declaration is the part that can be published

**Search for:** `open source`, `publico`, `public`, `publish`, `licenca`, `license`, `skeleton`

- **Date:** 2026-08-19
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible as code. **Not reversible as a promise**: once the shape
  ships publicly, anyone building on it inherits the split, and moving the line later
  moves it for them too.

## Context

ADR-0024 gave the person one base. Richard then named the rule that has to hold around it,
and it is a rule about publication rather than about layout:

> a estrutura do nosso projeto deve ter essa regra clara e definida de que um usuário
> precisa ser propriamente declarado (humano) e documentado na medida que os agentes forem
> aprendendo sobre ele. Porém, se caso o projeto vier a publico, o meu arquivo pessoal não
> deve transmitir o que está anotado sobre mim.

And he named the precedent himself: the agents here are his, but **how an agent is shaped,
how agents relate, how they answer, all of that is documentable and eventually public.**

So the question is not whether to publish. It is which half.

## The split, stated once

| | Public | Private |
|---|---|---|
| Agent | `agent-skeleton/`, `kb init`, the constitution's block structure | Every base under `fleet/` |
| **Person** | **`person-skeleton/`, `kb init --person`, the rule that a fleet declares its human** | **`fleet/profile/`, every word of it** |

The row for the agent already existed and has been working since ADR-0011. The row for the
person is what this record adds, and the symmetry is the argument: **the same mechanism,
applied to the human, gives the same guarantee.**

## Options

### Option A: document the rule in prose

A section in the README saying a fleet should declare its person.

- Cost: nothing.
- Failure mode, and this repository has met it three times: **a rule that lives only in
  prose is followed by whoever read it.** The em dash rule went into `kb check`, the commit
  rule into `kb commit`, and the boot rule into a hook, each after prose failed.
- Forecloses: nothing.

### Option B: ship the shape as a generator plus a published skeleton, guarded by a test

`kb init --person` writes the base; `person-skeleton/` is that output committed; a test
fails if they ever differ.

- Cost: one function, five template files, one test.
- Failure mode: the templates rot into ceremony if nobody fills them, which the empty files
  say out loud rather than pretending otherwise.
- Forecloses: nothing.

### Option C: put the person inside the agent skeleton

One skeleton, with a `profile/` folder in it.

- Failure mode, and it is the disqualifying one: **it re-teaches the error ADR-0024 just
  removed.** A profile inside the agent shape says every agent owns a copy of the person,
  which is exactly the arrangement that drifted into two languages and two holes.

## Decision

**Option B.**

1. **`kb init --person`** writes `fleet/profile/`: a map, a gitignore, and `core.md`,
   `work.md`, `presence.md` **empty, with the questions in them instead of answers**. No
   `agent.txt`, because a person is not an agent and the router must never elect one. No
   constitution, because a person does not boot.
2. **`person-skeleton/` is that output, committed to the public repository**, so somebody
   browsing can see how a fleet records its human without installing a toolchain. Guarded
   by a drift test, the same one the agent skeleton has had, for the same reason.
3. **`kb init` now writes a `[user]` block into every agent it creates**, pointing at
   `../profile/core.md`. This is the part that makes the rule structural rather than
   advisory: **a new agent cannot be born not knowing who it works for.** Steve and Aldo
   were born that way, and one of them answered a question about Richard's own CV without
   knowing his name.
4. **The content is never published, by the mechanism already in place**: `fleet/` is a
   separate repository, gitignored by the public one, so publishing a profile is not a
   mistake anyone can make here.

**The general form, because it is the reusable part.** Publish the shape, keep the
content. A skeleton is a claim about how something is built and is worth showing; a base is
a claim about a person or a business and is not ours to show. This fleet already applies
that to agents, to the decision records (mechanisms out, real queries and paths edited),
and to Steve's products. The person was the last thing without a published shape.

## Consequences

- Anyone cloning the public repository can build a correctly shaped fleet, including the
  human's base, and will find no fact about Richard in it.
- **The empty templates carry questions rather than placeholders.** An empty profile is not
  a neutral state: an agent that does not know who it works for gives generic answers
  confidently, which is worse than one that knows it is missing something. The files say so.
- Two skeletons now have drift tests, which is two tests that fail when a template changes
  and tell you to regenerate. That cost is accepted; the alternative is published
  documentation that lies about its own product.
- **The open source question stays open.** Nothing here decides whether this repository
  ships under a licence; it decides what would be publishable if it does, which is a
  prerequisite rather than a commitment. The licence line in the README still reads not
  chosen.

## Revisit trigger

- **A second human**, which turns `profile/` into a directory of people and makes the
  `[user]` block a per-session choice. ADR-0024 carries the same trigger and they move
  together.
- The first time someone fills a person skeleton and finds a field missing that every fleet
  would need. The template is a guess at the general case and has been tested on exactly
  one person.
- A decision to publish under a licence, which is when the private line stops being a
  convention of this machine and becomes a promise to strangers.
