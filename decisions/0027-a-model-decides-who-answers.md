---
provenance: agent
stage: derived
---

# ADR-0027: a model decides who answers, because choosing an agent is classification

**Search for:** `classifier`, `classificador`, `classificacao`, `roteamento`, `routing`, `router`, `kb route`, `quem responde`, `who answers`, `escolher agente`, `selecao de agente`, `falha de roteamento`, `routing failure`, `agente incorreto`, `dossier`, `dossie`, `roster`, `lista de agentes`, `cobertura`, `coverage`, `covered`, `adjacent`, `uncovered`, `ninguem cobre`, `nobody covers`, `criar agente`, `ends`, `role`, `agent.txt`, `IDF`, `keyword score`, `stopwords`, `alias`, `margem do incumbente`, `cascade`, `cascata`, `latencia`, `latency`, `lentidao`, `slow`, `hook`, `UserPromptSubmit`, `subprocesso`, `working directory`, `diretorio de trabalho`, `CLAUDE.md`, `contaminacao`, `stdin`, `stdout`, `fallback`, `Ollama`, `modelo local`, `classify.rs`, `kb eval`, `gold set`, `DevOps`, `deploy`, `downtime`, `monitoramento`, `infra`, `semantico`, `semantic`, `lexico`, `lexical`, `retrieval`, `Steve`, `Zed`, `ADR-0013`, `ADR-0018`

**Exists to:** Why a model, not a sum of keyword scores, picks which agent answers; what it costs in latency and where that cost actually came from; and how a subject nobody covers finally gets named.

- **Date:** 2026-08-19
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. The deterministic choice remains as the fallback and runs
  whenever no classifier is configured.

## Context

Richard, after four sessions of routing failures: *nosso sistema deve funcionar
INDEPENDENTEMENTE da base de conhecimento dos agentes. Deve rotear corretamente
INDEPENDENTE da quantidade de palavras em uma ou outra base. Eu já cansei de falar, nosso
sistema deveria ter sim um modelo pré invocação de um agente.*

He is right, and **the answer was already written in this repository**.
[[0013-retrieval-precedes-classification]] says, verbatim: *classification is the model's
job and lookup is the code's job*, and names the failure it was written against, an earlier
version that *classified questions and answered with strings we had written, which is code
doing a model's job badly.*

Choosing an agent is classification. It was built as a sum of IDF weighted keyword scores,
and the three days after that were spent patching that sum: a stopword list completed
twice, alias files, an incumbent margin, a corpus share normalisation, a base-share filter.
Most were measured and removed. **The patching was the symptom.**

### Why arithmetic cannot do this job

Retrieval and routing ask different questions.

- *Which file contains this* is **lexical**. A keyword index answers it exactly, and
  measurably: 18 of 26 on real questions.
- *Who understands this subject* is **semantic**. No count of shared words answers it,
  because the answer depends on what an agent's work is, not on which words are in its
  files.

The proof is a subject nobody has written about. Asked *como faco deploy com zero downtime
e monitoramento de infra*, the word **zero** matched three of Steve's research notes at
15.89 each, so the marketing agent held 100% of the field and won. A reader who knows only
that Zed *builds software and stops before running it* places that question instantly, and
says the thing that matters more than the placement: **nobody here operates systems**.

## Options

### Option A: keep patching the arithmetic

- Cost: unbounded. Every new agent, every new language, every collision needs another rule,
  and this session shipped four of them.
- Failure mode: **it cannot express the useful answer.** A score of zero says "no match".
  It can never say "no one here does this kind of work", which is the sentence Richard asked
  for and the one that leads to creating an agent.

### Option B: an embedding model in the retrieval path

- Failure mode: measured and rejected already, in
  [[0018-no-model-in-the-retrieval-path]]. It also solves the wrong problem: semantic
  *retrieval* still returns files, and the question here is about agents.

### Option C: a model reads the roster and the evidence, and names the owner

- Cost: one model call per message. Measured at 9 to 15 seconds through the Claude Code CLI
  on this machine, and the honest weakness of this decision.
- Failure mode: a model that invents an agent, handled by refusing any name off the roster;
  and a model that is unavailable, handled by falling back to the arithmetic.

## Decision

**Option C, with the split kept exactly where ADR-0013 put it.**

1. **Retrieval does not change.** Deterministic, no model, reproducible, and still the only
   thing that reads the corpus.
2. **The classifier receives a dossier**: the roster, the evidence retrieval found, and the
   question. About three hundred tokens, and never the base. So it cannot invent a file,
   and its answer is a name from a list it was handed.
3. **Agents declare where they stop.** `ends =` in `agent.txt`, alongside `role =`. This is
   load bearing rather than decorative: **a roster of roles tells a reader what each agent
   does and never what none of them does**, and without edges the classifier called DevOps
   `covered` because "software architecture and building" plausibly includes it.
4. **Coverage is a first class answer.** `covered`, `adjacent`, `uncovered`. Adjacent names
   the nearest agent and says plainly that answering from there is a stretch, and that the
   honest options are to teach that agent or to create a new one. That is the sentence
   Richard described and the reason this record exists.
5. **The contract is a process, not a provider:** dossier on stdin, verdict on stdout. Any
   model behind any runtime satisfies it, including a local one, which is what makes this
   independent of whether the client is Claude Code, Claude Desktop, opencode, or Ulpia's
   own interface. `kb` gains no dependency and no network code.
6. **No classifier, or a failing one, falls back to the arithmetic.** The fleet never stops
   routing because a model was unavailable.

## What was built and rejected inside this decision

**A cascade**, gating the model on whether the deterministic choice dominated its field.
It worked on cost: the common case fell from about 14 seconds to about 1. It broke the case
the classifier exists for, routing the DevOps question to marketing in 971 ms, because
"zero" gave Steve 100% of a field nobody else was in.

**The mechanism, and it is why no threshold rescues it:** a cascade can only gate on the
deterministic score, and the deterministic score does not know when it is wrong. "One agent
alone in the field" does not distinguish *plainly theirs* from *a coincidence of
vocabulary*; those are the same number. **A gate built on a blind signal inherits the
blindness.** The constant and the measurement are kept in `classify.rs` so the idea is met
rather than rediscovered.

## Consequences

- **9 to 15 seconds are added to every message** through the CLI classifier. That is the
  price of the decision and it is not hidden. The way down is a faster classifier, a local
  model or a resident process, and not a cheaper decision about when to think.

  This line read *13 to 16* until 2026-08-20 and the real figure was **43 to 48**, over a
  hook budget of 30. The gap was not the model and not the network. The classifier was
  spawned with its working directory set to the fleet root, so the CLI inspected 11,510
  files and 2.5 GB of build output on every message before reading a dossier that arrives
  whole on stdin. It also loaded the fleet's `CLAUDE.md`, which is a contamination this
  record's own isolation rules exist to prevent. Both are fixed by starting the process in
  an empty directory outside the fleet.

  **The lesson generalises past this bug.** The cost of a subprocess is not only what it
  computes. It inherits a working directory, an environment and whatever a runtime decides
  to read from them, and none of that appears in the design. It was found by measuring the
  shipped path end to end rather than the piece being reasoned about, which is the same way
  the stale binary was found and the same way it will be found next time.
- Routing stops depending on whether anyone wrote the right word in a `Search for:` line.
  Aliases and keywords still serve retrieval, where they belong.
- The fleet can now say **nobody here covers this**, which is the input to deciding whether
  an agent should exist. That answer did not exist in any previous version.
- `kb eval --classify` measures the classifier beside the arithmetic. On the 26 question
  set both score 18 of 20, because that set only contains questions that *have* an owner.
  The difference appears on coverage, which is what the set was missing and what a new
  gold file now tests.

## Revisit trigger

- **A local classifier**, which would collapse the latency argument and is the obvious next
  move. Ollama is not installed on this machine; installing it is Richard's call.
- The first time the classifier names an owner that is clearly wrong on a subject the fleet
  does covers. One measured failure of that kind reopens the comparison.
- A fleet large enough that the roster no longer fits comfortably in a dossier, which is
  well past the scale this decision was made at.
