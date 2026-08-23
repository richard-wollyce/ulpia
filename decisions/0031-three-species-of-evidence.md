---
provenance: agent
stage: derived
---

# ADR-0031: an agent's evidence has three species, and mass in one cannot outvote purpose in another

**Search for:** `tres especies`, `three species`, `tres niveis`, `three levels`, `memory`, `memoria`, `skills`, `habilidades`, `competencias`, `tools`, `ferramentas`, `pasta tools`, `pasta skills`, `pasta memory`, `taxonomia de pastas`, `folder taxonomy`, `kind`, `especie`, `kind_of`, `vies de volume`, `volume bias`, `massa de memoria`, `memory mass`, `base densa`, `dense base`, `agente novo`, `purpose built agent`, `agente de proposito`, `fold por especie`, `per kind fold`, `max por especie`, `max per kind`, `soma entre especies`, `sum across kinds`, `choose_agent_by_keyword`, `tally`, `normalizacao por share`, `corpus share`, `dossie por especie`, `sabe sobre`, `sabe fazer`, `tem o meio`, `knows about`, `knows how`, `has the means`, `biblioteca de anuncios`, `ads library`, `noticias de tech`, `tech news`, `declaracao de ferramenta`, `tool declaration`, `MCP por agente`, `permissoes por agente`, `ingestao so produz memoria`, `ingestion writes memory only`, `epifania do aviao`, `ADR-0031`

**Exists to:** record why the router ranks three kinds of evidence per agent instead of one
pool, what aggregation rule replaces the sum, and why ingestion may only ever produce memory

- **Date:** 2026-08-21
- **Status:** accepted
- **Scope:** fleet
- **Builds on:** [[0013-retrieval-precedes-classification]], whose order (scorer, then
  classifier, then Vesta) is unchanged and is the line this whole design decorates.
  [[0027-a-model-decides-who-answers]]: the classifier still judges; what changes is what it
  is shown. [[0030-two-promoters-and-the-second-is-not-a-second-opinion]]: promotion gains a
  hard rule from the species split.
- **Reversibility:** the fold change is one function and reverts in one edit. The folder
  taxonomy is file moves, reversible with `git mv` for tracked files and by hand for the
  private layer, which has no history.

## Context

Richard, from a plane: the folders inside every agent become three, `tools/` (ferramentas,
MCP, plugins, permissões), `skills/` (como ele trabalha, o que sabe fazer, o que não deve
fazer), `memory/` (documentos markdown auditáveis pelo git). And then the part that names
the defect rather than the layout: *precisamos que o scorer ranqueie 3 níveis diferentes,
justamente para que um agente como o Steve, que tem memória densa pra caramba, não ranqueie
melhor do que um agente que tem 0 de memória mas foi recém criado com o propósito
(ferramentas + skills) preparadas para agir no tópico em questão.*

His two examples, kept because they are the acceptance test:

- *"buscar algo na biblioteca de anúncios"*: Steve should win. His memory scores, his tools
  score, and both pointing the same way is exactly what being the right agent looks like.
- *"pesquisar notícias sobre tech"*: nobody has memory about it. Steve's files still share
  incidental words (`pesquisa`, `internet`). A purpose-built agent whose skills say
  *pesquisador assíduo sobre tecnologia* should win despite having zero memory, and today
  it cannot, because zero memory means zero mass and mass is what the fold counts.

The defect is old and documented. `choose_agent_by_keyword` sums scores per base over the
top hits, so a base that puts many files in the list wins by volume. The comment at
`memory.rs` records the first attempt at fixing it, corpus share normalisation, built and
measured on 2026-08-18 and removed: dividing by base share boosts small bases instead of
levelling large ones, and the gold set scored 12/13 either way. It closes with *the volume
concern is real and still open; the answer is not this.* This record is the second answer.

Measured the day before this record, *"um anúncio no meta foi bloqueado"* returned five
files and all five were `knowledge/`: five memories, zero skills, zero tools. The fifth was
a transcript **about** the Meta Ads MCP, which is memory about a tool and not the tool. The
question that decides whether Steve can go look at the account, rather than talk about it,
was unanswerable from what retrieval carried.

## The decision, in four parts

### 1. Every indexed file has a species, and the folder declares it

`memory/` is what the agent knows: markdown, git-auditable, the only species ingestion may
produce. `skills/` is how it works, what it can do, what it must not do. `tools/` is text
declarations (`.md` or `.txt`) of ferramentas, MCP, plugins and permissions: written to be
found, indexed like everything else, because a scorer can read text and only text.

The species is computed from the path by one pure function, `index::kind_of`, and travels
nowhere as data: the fold and the dossier both call it on the path they already hold. A
folder is a location, not a copy, so it cannot go stale the way a duplicated header field
can; moving a file to another species folder is *supposed* to change what it means.

Existing folder names map onto the three rather than forcing 149 files to move on day one:
`knowledge` and the domain-data folders read as memory, `protocols` and `templates` read as
skills, `tools` reads as tools. The full table lives beside `kind_of` in the source, and the
per-base migration, including true moves and the seeding of empty `skills/` and `tools/`
folders, is decided base by base with critique rather than mechanically. `inbox/` stays
outside all three: it is quarantine, per [[0030-two-promoters-and-the-second-is-not-a-second-opinion]].

### 2. The fold takes the best of each species and sums the species

Per agent: group its hits by species, keep only the **maximum** score in each, and the
agent's total is the sum of those at most three maxima.

Why this shape and not another, since an aggregation rule asserted without its rivals is a
preference:

- **Sum per species** still rewards mass inside memory, which is the defect itself.
- **Max overall** flattens Steve's ads case: memory and tools agreeing would count the same
  as memory alone, and breadth is precisely the evidence that he is prepared, not merely
  informed.
- **Corpus share normalisation** was measured dead on 2026-08-18.
- **Max per species, summed** makes the two examples come out right by construction. Dense
  memory contributes exactly one number no matter how many files matched, so Steve's fifty
  nine files are worth their best one. A purpose-built agent's strong skills file meets that
  best-of-memory head on. And an agent scoring in two species beats an agent scoring the
  same in one, which is Richard's ads example.

The keyword scores being summable across species is the same property that made the old
fold legitimate: one document frequency table over the whole fleet.

### 3. The dossier says which species answered

The classifier used to see five files and a score each. It now sees each file labelled
`[memory]`, `[skills]` or `[tools]`, and for the leading agent, its best file per species.
Coverage stops being one question and becomes three that a reader can tell apart: knows
about it, knows how to do it, has the means to do it. "Nobody knows this" routes nobody and
suggests creating an agent; "knows but has no tool" routes the agent and names the gap.

### 4. Ingestion writes memory, and the writer refuses anything else

A conversation can teach the fleet a fact. It cannot grant a competence or install a
ferramenta, because those are written and configured deliberately. So `kb promote` refuses
any proposal whose target folder is not memory-kind, as a hard rule in code rather than a
lens that sometimes catches it. The refusal lands in `kb-rejections.txt` like any other,
because a promoter that keeps proposing skills is a signal about the promoter.

## Consequences

- The volume problem open since 2026-08-18 gets its second, structural answer, and the
  failed first answer stays documented next to it.
- `AgentChoice.files` changes meaning: it counts species that contributed, no longer files.
  Callers that printed "N files" now print evidence breadth, which is more honest anyway.
- The eval moves and is measured in the change that ships this, both directions, against
  the baseline of FILE 8/24 and 11/24, AGENT routes 18/24 and keyword 19/24, abstention 4/9.
- Until agents grow real `skills/` and `tools/` content, most bases are memory-only and the
  fold degrades gracefully to max-of-memory, which alone already removes the mass bias.
- A new agent becoming routable the day it is born is the point of the whole design:
  `kb init` seeding `skills/` and `tools/` declarations is the next step, **not yet built**,
  and blocked deliberately: `init.rs` is mid-edit by another session and two sessions in one
  file is how commit cdc0e52 happened.

## Revisit trigger

- An agent that legitimately holds many strong memory files on *distinct* subtopics of one
  question and loses to a shallow specialist: max-per-species throws away corroboration
  inside a species, and if that costs a real question, the species aggregation needs a
  second look (log-sum or top-2, measured, not guessed).
- Per-agent MCP configuration becoming real, which turns `tools/` declarations from
  routing evidence into something the runtime enforces, and those two must not drift.
