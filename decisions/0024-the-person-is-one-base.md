---
provenance: agent
stage: derived
---

# ADR-0024: the person is one base every agent reads, and never an agent that answers

**Search for:** `quem sou eu`, `who am i`, `richard`, `usuario`, `user`, `perfil`, `profile`, `persona`, `identidade`, `identity`, `dados pessoais`, `perfil global`, `global file`, `fleet/profile`, `profile base`, `core.md`, `work.md`, `presence.md`, `richard.md`, `bloco user`, `[user] block`, `blocks assemble`, `resident block`, `bloco residente`, `residency`, `tokens residentes`, `12.1k`, `9689`, `2708`, `agent.txt`, `z32`, `abstain`, `abstencao`, `steve`, `yaron`, `aldo`, `zed`, `duplicacao`, `duplication`, `copias`, `drift`, `divergencia`, `fragmentacao`, `consolidacao`, `ingestion`, `ingestao`, `contradicao`, `contradiction`, `richardwollyce.com`, `cv`, `curriculo`, `site pessoal`, `docker`, `rust`, `lidera engenharia`, `engineering leader`, `job title`, `profissao`, `carreira`, `metas`, `goals`, `gitignore`, `tracked`, `privacidade`, `privacy`, `second human`, `pessoa`, `operador`, `miss log`, `biografia`

**Exists to:** Where the record of who Richard is lives: one profile base every agent reads, resident core plus retrieved files, never an agent that answers.

- **Date:** 2026-08-19
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible for the layout. **Not reversible for the habit**: once four
  constitutions resolve the user block to one path, a later split means re-fragmenting a
  person on purpose.

## Context

Richard asked the desk "quem sou eu" and was told the base did not cover it. That answer
was half wrong, and the right half was worse.

What the fleet actually held, verified:

| Where | What | Language | Written |
|---|---|---|---|
| `zed/profile/richard.md` | 145 lines: identity, level, projects, how they work | English | 2026-08-13 |
| `yaron/profile/profile.md` | 168 lines: body, training, health, sleep, mind | Portuguese | 2026-08-10 |
| `steve/blocks.txt` | **no `[user]` block at all** | | |
| `aldo/blocks.txt` | **no `[user]` block at all** | | |

So the same human was written twice, in two languages, in two private folders, and two
agents had nothing. **The cost was paid the same day, in public.** The router woke Steve to
answer a question about Richard's own website and CV, and Steve's constitution carried no
line saying who Richard is, while two folders away a file recorded that *presence strong
enough that relevant, well paid work comes to him* is half of his twelve month goal. The
marketing agent is the one that most needed that sentence and the only one that could not
read it.

Richard's position, and it is the correct one: the user should be **a global file** that
Vesta and every agent can consult, rather than each agent holding a limited scope of who
he is.

## Options

### Option A: per-agent scope, which is the status quo

Each agent keeps the slice of Richard its domain needs, in its own private folder.

- Cost: the same person written N times.
- Failure mode, and it is not hypothetical, it is what produced this record: **N copies
  drift, silently, and the gaps are invisible from inside any one agent.** Five days
  produced two languages, two dates and two agents with nothing, and nobody noticed until
  a question crossed a boundary.
- Forecloses: any agent ever having the whole picture.

### Option B: one global file, resident everywhere

One `richard.md`, in every agent's resident block.

- Cost: every agent pays for every domain on every question. Zed's constitution measured
  12.1k resident tokens the day before this; adding health history and anthropometry makes
  every architecture question carry sleep data.
- Failure mode: the boot path grows without bound, which is the finding ADR-0005 exists to
  manage and Z5 and Z12 already track. Also widens what each agent holds for no benefit:
  the design agent would carry medical history.
- Forecloses: nothing, but it trades one real problem for another.

### Option C: one base, several files, one small resident core

- Cost: one base and a split that has to be judged once.
- Failure mode: a fact filed in the wrong file is retrieved less often than it should be,
  which the miss log catches the same way it catches everything else.
- Forecloses: nothing.

## Decision

**Option C, which is Option B with one correction: global is about ownership, not
residency.**

`fleet/profile/` is a base like the decision records at the fleet root:

1. **No `agent.txt`.** The router reads it and can never choose it as the agent who
   answers, because **a person is not an agent**. This is the Z32 rule already in the code
   doing exactly the work it was built for. Verified: "quem sou eu" abstains, and the
   librarian answers from the base rather than an agent impersonating one.
2. **`core.md` is resident in all four constitutions**, through a `[user]` block pointing
   at `../profile/core.md`. Verified to resolve: `blocks::assemble` joins the path against
   the agent root, so a sibling base is reachable and no absolute path enters the fleet.
3. **`work.md` and `presence.md` are retrieved**, so nobody pays for the whole person on
   every question.
4. **It is tracked**, unlike the per-agent `profile/` folders which are gitignored even
   inside the private repository. That is not a relaxation of privacy: **Vesta refuses to
   serve what git does not track, so an untracked base is unroutable by construction.**
   The tier that fits is a tracked file inside a repository that is itself private and has
   no remote. The server proved the point by refusing to start until the base was
   committed.

**Measured:** Zed's `[user]` payload went from 9,689 bytes to 2,708, so his resident set
dropped about 1,700 tokens **while** three other agents gained the file entirely. The
consolidation was cheaper than the duplication it replaced.

## Consequences

- Updating who Richard is means editing one file, and every agent sees it on its next boot.
- **`yaron/profile/` still holds a second Richard, in Portuguese, and it was not moved.**
  That folder is a named private layer in `limits-and-autonomy`, which wins over every
  other file, so it stops for a human. Yaron carries both his own profile and the shared
  core meanwhile, which is duplication accepted for exactly as long as it takes to ask.
- `richardwollyce.com` entered as a source and **contradicted the base twice**, recorded in
  `work.md` rather than overwritten, per the ingestion protocol: Docker sits in his
  professional infrastructure although the base filed it under hates, and Rust stopped
  being an aspiration.
- A class of profile error is now named: **a profile assembled by asking about goals misses
  the job**, because nobody volunteers their own title to someone they assume already knows
  it. The base had no record that he leads engineering for a living until his own public
  site was read.

## Revisit trigger

- **A second human.** Everything here assumes one operator; a second one makes `profile/`
  a directory of people and the `[user]` block a per-session choice rather than a constant.
- `core.md` passing roughly 4 KB, which is the point where the resident cost of the person
  starts competing with the map and the split needs re-judging.
- An agent needing a fact that is in `work.md` on every question, which would mean the
  resident and retrieved line was drawn in the wrong place for that agent.
