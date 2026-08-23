# ADR-0003: files stay the source of truth, the index is derived

**Search for:** `source of truth`, `fonte da verdade`, `derived index`, `indice derivado`, `Neo4j`, `graph database`, `banco de grafos`, `database`, `banco de dados`, `SQLite`, `Postgres`, `guardar em banco`, `migrar para banco`, `usar um banco de dados`, `markdown files`, `arquivos markdown`, `arquivos soltos`, `monte de arquivos`, `git history`, `historico do git`, `git revert`, `reverter mudanca`, `audit trail`, `auditoria`, `text editor`, `editar no editor de texto`, `corrigir a mao`, `publication format`, `publicar a base`, `zero infrastructure`, `sem infraestrutura`, `backup`, `migracao`, `exportar`, `export`, `validator`, `validador`, `broken links`, `links quebrados`, `link rot`, `staleness report`, `tools/kb`, `kb`, `Rust`, `projeto em Rust`, `CLI`, `file scan`, `varredura de arquivos`, `graph traversal`, `variable depth traversal`, `document store`, `disposable projection`, `projecao descartavel`, `reversibilidade`, `assimetria`, `orchestrator GUI`, `conversation transcripts`, `job queue`, `concurrent writes`, `multiplos escritores`, `escalar a base`, `mais robusto`, `integrado`, `greppable`

**Exists to:** Why the markdown files stay authoritative and any database or graph index is a disposable projection.

- **Date:** 2026-08-13
- **Status:** **accepted**, ratified by Richard 2026-08-13. Answers his question the same day.
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** the decision is reversible. **Its opposite is not**, which is most of the argument.

## Context

Richard asked, directly: we are going to expand these agents and eventually publish them, so instead
of a pile of scattered markdown files, would it not be better to store this in a database, something
like Neo4j, so the whole thing is more robust and integrated?

The instinct behind the question is right and the question underneath it is the real one:
**what is the source of truth, and what is a projection of it?** Those are two different decisions and
answering them together is how a project ends up with a knowledge base it can no longer read.

There is also a second system coming that muddies the question: the orchestrator GUI, with TTS and STT
later. That one has genuine database work in it. It is not this decision.

## What the files actually give us today

Worth stating explicitly, because these are usually noticed only after they are gone:

- **The agent runtime reads files natively.** Every rule in `index.md` works because reading a file is
  free and unconditional. Through a database, every read becomes a tool call the model has to decide
  to make, that can fail, and that the model can skip. The mandatory reading order stops being a
  convention and becomes a service dependency.
- **Git is the audit trail.** What the agent knew in March, what changed, who changed it, and how to
  revert it, are `git log`, `git diff` and `git revert`. In a graph database each of those is a
  feature you have to design, build and maintain.
- **A human can fix it with a text editor.** Correcting a wrong fact today costs seconds. Behind a
  database it costs whatever the GUI costs, and it cannot be done until the GUI exists and is good.
- **The repository is the publication format.** Steve is already built to be published as a repo. A
  database needs an export step before it can ever be public, and exports rot.
- **Zero infrastructure.** Nothing to run, back up, migrate, secure or pay for.

## The scale we are actually at

Hundreds of files, kilobytes each, one writer, no concurrency, no latency requirement. Nothing here is
a storage problem. **The pain today is not retrieval, it is validation:** `[[links]]` that nothing
checks, staleness nothing enforces, no cross agent view, no guarantee a note has the fields it should.
A database does not fix any of those. A validator fixes all of them.

## On Neo4j specifically

Is the graph the model here? Partly, and honestly: notes, links, conflicts, supersession, domains and
agents are genuinely a graph. But the queries we actually want are one or two hops. What links here,
what contradicts this, what is stale, what covers this domain. Those are trivial over a file scan and
trivial in a single table.

Neo4j earns its keep on **variable depth traversal and path finding**, where the query is
"how are these connected, at any distance". We do not have that query yet. Adopting the technology
first and looking for the problem afterwards is precisely the weak decision named at the top of
`the-bar.md` (a protocol in the private layer), and it would be the first thing this repository
did after writing that file down.

There is also a mismatch nobody mentions until they hit it: **graphs are good at relationships and bad
at being a document store.** Our nodes are long prose meant to be read, diffed and reviewed by a human.
In a graph they become markdown stuffed into a string property, which is the worst of both worlds:
you lose diffing, and you did not gain anything a text file did not already do.

## Options

### A. Files as they are, no index

What we have. Cost: link rot, silent staleness, no cross agent query, no structure guarantee. This is
the status quo and it does not survive expansion.

### B. Files stay the source of truth, the index is derived and disposable

A tool parses the files and produces an index: link validation, staleness report, structure check,
search, and later a graph projection if a real query demands it. **The index can be deleted and
rebuilt at any moment.** Cost: a tool to build and maintain, which is real software held to the same
bar. Failure mode: the index goes stale, which is visible and harmless because the files are still the
truth.

### C. Database as the source of truth

Neo4j, or anything else, holding the knowledge. Cost: infrastructure, backups, migrations, a GUI
before anyone can edit, an export before anyone can publish, and every agent read becomes a tool call.
Failure mode: a bad write is unrecoverable without a backup system we would then also have to build,
and the fluent, human readable, greppable base becomes an opaque one.

## Decision

**Option B.**

Files stay the source of truth. Any index is derived from them, rebuildable from scratch, and never
authoritative. If we one day want graph queries, we **project** the files into a graph store and the
projection stays disposable, so it can never corrupt the base and never becomes a thing we are afraid
to lose.

**The asymmetry is the whole argument.** Files now and a database later is a migration we can do in an
afternoon, because everything is text and the structure is already explicit. Database now and files
later is an export, a schema archaeology exercise, and a loss of everything git was giving us for
free. When two options are not equally reversible, the reversible one wins unless the irreversible one
is solving a problem we actually have. It is not: it is solving expansion we have not done yet.

**The GUI is a separate decision, and its answer is different.** The orchestrator will need real
storage for session history, conversation transcripts, the job queue, audio artefacts and preferences.
That is database work and it should have its own ADR. The likely answer there is SQLite first and
Postgres when it needs to be shared, not Neo4j. Keeping the two decisions apart is what stops us from
ending up with a knowledge base we cannot grep because the app needed a session table.

## Consequences

- We keep git history, human editing, agent native reads, publishability and zero infrastructure.
- We take on a tool to build and maintain, held to the same bar as client work.
- The first version is small and boring on purpose: parse front matter, resolve every `[[link]]`,
  report broken links, missing MAP entries, missing fields, and notes whose `valid_for` is behind what
  is installed.
- **It is the right first Rust artefact.** Text parsing, file walking, error handling and a clean CLI,
  which is exactly the shape of Rust that teaches ownership and error modelling without fighting async
  or lifetimes on day one. It is small, it is real, we use it every day, and it produces something
  worth publishing. That serves four of Richard's stated goals at once and none of them are the reason
  it is correct, which is why it is worth doing.

## Revisit trigger

Any one of these reopens it:

- A query we genuinely want and cannot answer with a file scan in under a second.
- A real variable depth traversal need, for example tracing every claim that transitively depends on a
  source we just retracted, across several hundred notes.
- More than one writer, or a web client, needing concurrent writes to the knowledge base.
- The index taking longer to build than the work it saves.

None of those are true today. The first one is the one to watch, because it will arrive quietly.
