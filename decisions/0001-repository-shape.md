# ADR-0001: Zed is built on Yaron's three file split, extended for engineering

**Search for:** `repository shape`, `three file split`, `boot path`, `CLAUDE.md`, `index.md`, `MAP.md`, `mandatory reading order`, `routing map`, `ADR`, `architecture decision record`, `decisions folder`, `session records`, `session continuity`, `fleet folder`, `folder layout`, `file placement`, `adding a rule`, `plain text`, `grep`, `retrieval service`, `indexed base`, `boot cost`, `cost per query`, `bloated boot file`, `reversible decision`, `MAP entries`, `self improvement`, `third agent`, `engineering agent`, `agent architecture`, `publish the repository`, `fleet backlog`, `repository rules`, `Zed`, `Steve`, `Yaron`, `estrutura do repositorio`, `organizacao de pastas`, `tres arquivos`, `arquivo de boot`, `ordem de leitura`, `leitura obrigatoria`, `mapa de roteamento`, `registro de decisao`, `registrar uma decisao`, `pasta de decisoes`, `historico de sessoes`, `continuidade entre sessoes`, `pasta fleet`, `texto puro`, `custo de boot`, `arquivo inchado`, `criar um agente`, `terceiro agente`, `agente de engenharia`, `arquitetura de agentes`, `onde colocar um arquivo`, `criar uma pasta`, `adicionar uma regra`, `regras do repositorio`, `publicar o repositorio`, `dividir o CLAUDE.md`, `onde documentar uma decisao`, `repositorio desorganizado`

**Exists to:** Why this repository is split into boot, index and map, and where a new folder or rule belongs.

- **Date:** 2026-08-13
- **Status:** accepted
- **Scope:** repository
- **Deciders:** Richard, Zed
- **Reversibility:** reversible, cheaply, while the base is small

## Context

Richard asked for a third agent, Zed, a software architect and right hand for everything he builds.
Two working agents already exist and were read first: Steve (marketing, English, substantial base) and
Yaron (health, Portuguese, seeded base). They share one architecture with meaningful differences, so
the first decision is which of the two shapes Zed inherits.

Richard's answers that constrain this: repository in English, conversation in English or Portuguese;
full autonomy to build; Zed participates in setting the rules rather than receiving them; Zed will keep
records; the repository becomes public one day but not yet; the rulers are not defined yet; and Zed is
expected to improve itself and the other agents, probably with real software eventually.

## Options

### Option A: copy Steve's shape

Instructions, folder map, privacy rules, protocols and style all inside `CLAUDE.md`, with `INDEX.md`
as a combined map and summary.

- Cost: the boot file grows without limit, and everything in it is paid on every question. Steve's
  `INDEX.md` is already about 99 KB and is declared the first read, every time.
- Failure mode: as the base grows the fixed cost per query grows with it, and the natural response is
  to stop reading the map, which defeats the design.

### Option B: follow Yaron's split, extended

Thin `CLAUDE.md` for boot, `index.md` for the operating instructions, `MAP.md` for routing. Add what
engineering needs and health does not: decision records, session continuity, a fleet section, and an
evidence ruler built for claims that expire.

- Cost: more files at day zero, and three places to keep consistent instead of one.
- Failure mode: the split invites putting rules in the wrong file. Mitigated by a stated rule: boot
  points, index governs, map routes.

### Option C: build something new, code first

An indexed base with embeddings, a retrieval service, a real application around the knowledge.

- Cost: infrastructure to run and debug, opaque state, and the user loses the ability to read and edit
  everything by hand.
- Failure mode: we would be maintaining a retrieval system instead of building things. The plain text
  and `grep` design is working in two live agents; replacing it before it has failed is complexity
  bought on speculation.

## Decision

**Option B.** Yaron's split, extended with four additions that come from the domain rather than from
the pattern:

1. **`decisions/`,** ADRs. Engineering decisions outlive conversations and get re-argued from memory
   otherwise. Neither existing agent has this and both would benefit.
2. **`records/sessions/`,** continuity. Steve keeps no longitudinal record and Yaron's is for health
   data. Zed's work spans sessions, so what was decided and what is open has to survive the end of a
   conversation.
3. **`fleet/`,** because improving the agents is explicitly part of Zed's domain, which makes the
   architecture itself a subject the base has to hold.
4. **An evidence ruler with two extra checks**, staleness and scale mismatch, which do not exist in
   health or marketing in the same form. A true claim about software rots on a version bump, and a
   true claim at Google's scale can be actively harmful at ours.

Option C is not rejected forever. It is rejected until the manual procedure has been run enough times
to know its shape, per `protocols/self-improvement.md`.

## Consequences

- Easier: the boot path stays cheap, rules can grow without inflating what loads every time, and every
  structural decision from here on has a place to live.
- Harder: three files to keep consistent, and a discipline to maintain (`MAP.md` updated in the same
  move as any new file).
- New maintenance: `MAP.md`, the session records, and the fleet backlog only stay useful if they are
  actually written.
- Irreversible: nothing yet. The base is empty, which is the cheapest moment to be wrong.

## Revisit trigger

- `MAP.md` passes fifty entries, or the mandatory reading order stops being cheap. At that point apply
  the fix recorded as S1 in `fleet/backlog.md` here, before Zed needs it rather than after.
- Any structural decision that turns out to be load bearing for a real project, which raises the cost
  of changing it.

## Notes

The pattern this decision rests on is written up in `fleet/agent-architecture.md`, extracted by
reading both existing repositories on 2026-08-13.
