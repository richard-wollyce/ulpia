---
provenance: agent
stage: derived
---

# ADR-0025: a fleet must declare its human, and the shape of that declaration is the part that can be published

**Search for:** `open source`, `publicar`, `publish`, `publicacao`, `publication`, `publicavel`, `publishable`, `publico`, `public`, `privado`, `private`, `privacidade`, `privacy`, `licenca`, `license`, `licenciamento`, `skeleton`, `esqueleto`, `person-skeleton`, `agent-skeleton`, `template`, `scaffold`, `gerador`, `generator`, `kb init`, `init --person`, `drift test`, `teste de drift`, `humano`, `human`, `usuario`, `user`, `user block`, `bloco user`, `declarar usuario`, `declarar humano`, `base da pessoa`, `core.md`, `work.md`, `presence.md`, `gitignore`, `repositorio publico`, `public repo`, `github`, `clonar`, `clone`, `dados pessoais`, `personal data`, `dados sensiveis`, `sensitive data`, `confidencial`, `segredo`, `vazamento`, `leak`, `expor dados`, `esconder dados`, `compartilhar projeto`, `divulgar projeto`, `curriculo`, `cv`, `agent.txt`, `shape`, `perguntas no template`, `ADR-0024`, `ADR-0011`, `Richard`

**Exists to:** What of this fleet can be published and what stays private, why the person's declaration ships as an empty skeleton instead of a prose rule, and why every new agent is born pointing at the human's base.

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
   advisory: **a new agent cannot be born not knowing who it works for.** Steve and Aldus
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

## Amended 2026-08-20: the local hook reads the private layer, the server still does not

The split above is about **publishing**. It was being enforced as though it were about
**reading**, and those are not the same line.

`Base::discover(root, all)` narrows the file list to what git tracks unless `all` is set.
Every agent's `.gitignore` excludes `profile/`, `records/` and `projects/`, so those files
were absent from the index of any surface that did not pass the flag. `kb boot`, the
`UserPromptSubmit` hook, did not pass it. The effect, measured on this machine:

| question | without `--all` | with `--all` |
|---|---|---|
| *a meal-plan question of his, quoted in substance rather than verbatim* | `person/<a private file>` at 80.96 | `yaron/plans/<the standing plan>` at 110.62 |

The file that holds the answer, with the question's own words as literal keys, could not
be returned to Richard on his own machine about his own files. 28 files were in that
state, across the person's private base.

**The flag is now passed by the hook and by nothing else.** Richard's decision, on being
shown the split: *sim pode passar --all pro hook poder enxergar os arquivos git ignored.*

The distinction that makes this safe is which surface is which:

- **`kb boot`** runs locally, on Richard's machine, on Richard's own message, and injects
  context into his own session. Nothing leaves the machine. Withholding his files from him
  protected nobody.
- **`kb serve`**, the MCP server in `.mcp.json`, is invoked as `serve .` with **no `--all`**,
  and that is deliberate and unchanged. It is the surface a model queries and the one whose
  output could be carried anywhere, so it continues to serve only what git tracks.

So the promise this record makes is narrowed to what it always meant: **nothing private is
published, and nothing private leaves the machine.** It never meant that the person's own
fleet may not read the person's own files.

What to watch, because this is the direction the mistake would come from: any new surface
gets the public list by default and has to argue for `--all`, and the argument has to be
that it cannot carry data off the machine. `kb ui` serves over HTTP on localhost and does
not pass the flag today; if it ever binds to anything but loopback, that stays true.

## Revisit trigger

- **A second human**, which turns `profile/` into a directory of people and makes the
  `[user]` block a per-session choice. ADR-0024 carries the same trigger and they move
  together.
- The first time someone fills a person skeleton and finds a field missing that every fleet
  would need. The template is a guess at the general case and has been tested on exactly
  one person.
- A decision to publish under a licence, which is when the private line stops being a
  convention of this machine and becomes a promise to strangers.
- **Any surface that both passes `--all` and can send its output off the machine.** The
  2026-08-20 amendment holds only while the two sets do not overlap, and nothing in the code
  enforces that they do not.
