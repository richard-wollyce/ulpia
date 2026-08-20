---
provenance: agent
stage: derived
---

# ADR-0014: the system is written in English, the conversation is not

**Search for:** `language`, `idioma`, `lingua`, `linguagem`, `English`, `ingles`, `Portuguese`, `portugues`, `bilingue`, `translation`, `traducao`, `traduzir`, `localization`, `localisation`, `localizacao`, `i18n`, `interface strings`, `UI strings`, `textos da interface`, `texto do botao`, `botoes`, `buttons`, `error message`, `mensagem de erro`, `tray`, `bandeja`, `tools/tray`, `main.rs`, `store_hit`, `identifiers`, `identificadores`, `nome de variavel`, `nome de funcao`, `comments`, `comentarios`, `stopwords`, `accent folding`, `acentos`, `audit`, `auditoria de idioma`, `cargo check`, `Steve`, `verbatim`, `quoted creative`, `criativo publicitario`, `private layer`, `camada privada`, `Yaron`, `com.fleet.tray`, `bundle identifier`, `fleet-root.txt`, `mensagem de commit`, `ADR-0014`

**Exists to:** Which language the code, comments, notes and interface strings are written in, and the two places Portuguese is still allowed.

- **Date:** 2026-08-17
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible, and cheap while the surface is this small

## Context

[[0006-language-architecture]] settled that the knowledge bases are written in English and that
questions arrive in Portuguese, bridged by `kb-aliases.txt`. It did not say anything about the
software around the bases, and the gap produced a real split.

Found by a fleet wide audit on 2026-08-17, run at Richard's request:

- **`tools/kb` was already clean.** Every Portuguese string in it is *data about Portuguese*: the
  stopword list, the accent folding table, and test fixtures proving that `proteína` survives the JSON
  parser unmangled. Every identifier is English.
- **`tools/tray` was not.** Six user facing strings in `main.rs` and two buttons in the UI were
  Portuguese, changed deliberately on 2026-08-16 because Richard is the one reading the panel.
- One test helper was named `shit`, a contraction of "store hit", in a repository that is going
  public.
- Steve's tracked dossiers carry Portuguese, and correctly: the analysis is English and the **quoted
  creative is verbatim**, because the object of study is the exact wording. Translating a quoted line of published advertising creative destroys the evidence.

So there were two rules in force at once and they met exactly at the tray's interface strings.

## Options

### Option A: English everywhere, including what the user reads

One language for the whole system: identifiers, comments, markdown, commit messages, and interface
copy. Whatever the user types stays in whatever language they typed it.

- Cost: Richard reads his own tool in his second language.
- Failure mode: an error message that has to be understood under stress is understood a beat slower.
- What it forecloses: nothing, since interface copy is the cheapest thing in the system to change.

### Option B: English for the system, Portuguese for the interface

Code and knowledge in English, anything a human reads in Portuguese.

- Cost: **a second language boundary that has to be maintained forever**, and a rule with a case by
  case judgement in it. Is a notification title interface copy or a log line? Is a `problem` field
  rendered in a panel and also written to stderr one or the other?
- Failure mode: the boundary drifts, because nothing checks it. This is exactly how it drifted the
  first time.
- What it forecloses: shipping the tray to anyone who does not read Portuguese, which matters given
  that this repository is going public.

### Option C: do nothing

Leave the split. It is working today because there is one user.

## Decision

**Option A, Richard's call, stated directly: everything in English, only input and output allowed in
other languages.**

The reason it beat Option B is that Option B's cost is not the translation, it is the **boundary**.
A rule that requires deciding which side of a line each string falls on is a rule that drifts, and it
had already drifted once between two decisions four days apart. One language with one exception, the
content the user types and the content the agent answers with, is a rule with nothing to judge.

The exception is real and it is where it belongs: `kb-aliases.txt` exists precisely so a Portuguese
question reaches an English base, and Steve's quoted creative stays in the language it was written in
because it is evidence.

## Consequences

- The tray's six backend strings and two UI buttons are English. Verified by `cargo check`, which is a
  type check and not a run: **the tray was not launched.**
- `fn shit` is now `fn store_hit`.
- Two Portuguese sections in the 2026-08-16 session record are translated, with the fact that the
  panel had been switched to Portuguese preserved rather than dropped, because a record that loses a
  fact during translation is worse than an untranslated record.
- **Private layers were not touched and will not be.** Steve's private layer and Yaron's private layer are Portuguese, they are somebody's private work, and
  `limits-and-autonomy` forbids writing into them. They are named here so that their exclusion is a
  decision rather than an oversight.
- The tray's name is still `Fleet` and its bundle identifier `com.fleet.tray`, while
  [[0012-naming-and-hosting]] named the system Vesta. **Not fixed here**, because the identifier
  decides where `fleet-root.txt` lives and renaming it orphans the pointer. It needs its own migration
  and it is open.

## Revisit trigger

A second person uses the tray, or the tray ships to anyone. At that point interface language stops
being one person's preference and becomes a localisation question, which is a different decision with
a different shape. Also revisit if Richard finds that an error message in English costs him real time
in a real failure, which is the only evidence that would beat the boundary argument.

## Notes

The audit covered `tools/kb`, `tools/tray`, all three agent bases tracked and private, and the public
repository root. The root, `agent-skeleton/`, Yaron's tracked base and Zed's tracked base were already
clean.

Y7 in `backlog` was found already done during this audit: both remaining files are English prose with local proper nouns kept, and the finding had been sitting open for four days after being fixed.
