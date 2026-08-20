---
provenance: agent
stage: derived
---

# ADR-0029: the person's base is `person/`, because `profile` named three things

**Search for:** `profile`, `perfil`, `person`, `pessoa`, `fleet/person`, `fleet/profile`, `rename`, `renomear`, `renomear base`, `renomear pasta`, `renomear diretorio`, `nome da base`, `base name`, `colisao de nome`, `name collision`, `nome duplicado`, `ambiguidade`, `ambiguous`, `redundancia`, `redundante`, `termos redundantes`, `naming`, `nomenclatura`, `auditoria de nomes`, `auditoria`, `base do usuario`, `pasta da pessoa`, `diretorio da pessoa`, `diretorio`, `directory`, `usuario`, `yaron`, `zed`, `tombstone`, `lapide`, `superseded`, `special case`, `caso especial`, `PERSON_DIR`, `blocks.txt`, `kb blocks`, `bloco residente`, `missing file`, `arquivo faltando`, `kb serve`, `SQLite`, `lock`, `Memory::open`, `erro ao abrir fleet`, `setima base`, `kb`, `reiniciar`, `reiniciar MCP`, `referencia quebrada`, `ADR-0024`, `ADR-0028`

**Exists to:** record why the shared person base changed name, and what the rename cost,
because [[0024-the-person-is-one-base]] names the old path and is not edited.

- **Date:** 2026-08-20
- **Status:** accepted
- **Scope:** fleet
- **Supersedes:** the directory name in [[0024-the-person-is-one-base]] and nothing else in
  it. That record's decision, that the person is one base rather than a folder inside each
  agent, is unchanged and is the reason this one is small.
- **Reversibility:** reversible, at the cost of the same forty odd references.

## Context

Richard, reading a naming audit: *tem muita redundancia, arquivos claude.md em todas
pastas, felet pra la fleet pra ca, termos rendundantes ambiguos.*

The audit found something sharper than repetition. **One word named three different things
in the same tree:**

| path | what it held |
|---|---|
| `fleet/profile/` | the shared person base, resident in every agent |
| `fleet/zed/profile/` | a tombstone for a file superseded on 2026-08-19 |
| `fleet/yaron/profile/` | health data, private, unrelated to either |

This is not a tidiness complaint. `tools/kb/src/ui.rs` already carries a special case
written around the collision, and this session nearly treated one for another twice: once
when choosing a base predicate, and once when scoping a rename.

## Options

### Option A: leave it and rely on the path

- Cost: nothing today.
- Failure mode: **the collision is already producing code.** A special case exists to work
  around it, and a rule that needs a special case is a rule stated wrong.

### Option B: rename the shared base to `person/`

- Cost: about forty references, four of them in `blocks.txt` files, where a wrong path
  removes the person from an agent's constitution.
- Failure mode: a missed reference. Handled by the failure being loud, below.

## Decision

**Option B.** `fleet/profile/` is `fleet/person/`, `init::PERSON_DIR` follows it, and the
Zed tombstone is deleted. `fleet/yaron/profile/` keeps its name: with the other two gone
there is nothing left to confuse it with.

**The word `person` was already the vocabulary.** `kb init --person` created the base,
`init::PERSON_DIR` was the constant, `person-skeleton/` is the template. Only the directory
disagreed.

## What the rename actually cost, because it was not free

- **The fleet stopped opening.** `fleet/profile/` could not be removed while two `kb serve`
  processes held its SQLite index, so the tracked files moved out and the directory stayed
  behind holding only `.kb/`. Under [[0028-a-note-carries-its-own-keys]]'s new predicate, a
  directory in the fleet root is a base because it is there, so the empty shell became a
  seventh base, and `Memory::open` refused the whole fleet: *git could not be consulted, so
  there is no way to tell which files are private.*

  **That refusal is the design working.** Unknown is not public, and the alternative to a
  hard failure here is a base whose privacy nobody checked. It cost a restart of the MCP
  servers, which is what a disposable index is for.

- **A window where the person was missing.** The `blocks.txt` paths were repointed before
  the directory moved, so for one command `kb blocks` reported `missing file:
  ../person/core.md` and the resident set was 834 tokens lighter. It was visible because
  `blocks::read` records a missing path rather than failing, and `kb blocks` prints it.
  Verified after: 3337 bytes, the same as before.

## Consequences

- **0024 is not edited**, per the rule this repository keeps: a record says what was decided
  when it was written, and a change of mind gets a new record. Its table still names
  `zed/profile/richard.md` as a file that existed on 2026-08-13, which is true.
- The same applies to the session records and the backlog entries that mention the old
  paths. History describing the past correctly is not a broken link.
- `kb eval` unchanged across the rename: keyword 17/26 at file level, routing 19/20 at agent
  level. The person is reachable: *quanto eu peso* returns `person/body.md` at 43.54.

## Revisit trigger

- A second base needing a name that collides with a folder inside an agent. The lesson
  generalises past this instance: **a directory name inside an agent and a base name at the
  fleet root live in the same namespace as far as a reader is concerned**, and nothing in
  the tool enforces that they differ.
