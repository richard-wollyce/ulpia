---
provenance: agent
stage: derived
---

# ADR-0028: a note carries its own keys, and the map stops deciding what exists

**Search for:** `keywords`, `palavras chave`, `search for`, `onde ficam as keywords`, `MAP.md`,
`mapa`, `indice`, `index`, `como um arquivo e encontrado`, `nota invisivel`, `formato da nota`,
`cabecalho da nota`, `why this file exists`, `proposito do arquivo`

**Exists to:** record why the keys that make a note findable moved out of the base's map and
into the note itself, and what that costs. This file is written in the format it decides.

- **Date:** 2026-08-20
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible in code, expensive in content. The index can be pointed back
  at map entries in one commit; the keys written into 98 files would then be duplicated in
  two places, which is the state this record exists to leave.

## Context

Richard, proposing this: *todo arquivo de conhecimento dos agentes deve ter logo no começo do
arquivo a section Keywords, que será a primeira coisa que nosso validador vai parsear.* And
the reason, which is about people rather than code: *o usuário humano pode ajudar a manter a
knowledge base, sem necessariamente ficar escrevendo textos extensos de memória, que é onde a
IA brilha de fato.* Curating eight keywords costs a person thirty seconds. Writing the note
costs an hour. The keyword line is where human judgement earns the most per second spent, and
today it is buried in a different file.

**The mechanical argument arrived the same day, as a bug.** `index::build` iterates the
entries in each base's `MAP.md` and looks up a file for each one. Seven of Aldo's eight
knowledge notes were listed as `- [[name]] : summary` instead of `- **[[name]]**`, which
`map_entries` does not parse, so the base contributed **one** entry to the router instead of
eight. `kb check` printed `clean`, because its not-indexed rule accepts a wikilink anywhere in
the file. `kb route "IntersectionObserver scroll reveal"` answered *nothing matched* for a
tracked file containing that literal word and offered three of Steve's marketing terms
instead, reached by trigram similarity.

The defect is not the seven lines. It is that **one file's formatting decides whether another
file exists to the router.** A note cannot be missing its own header; it can very easily be
missing from somebody else's list, or present in a shape that list's parser does not read.

The measured scale of that: **98 knowledge notes, 114 map entries, and 84 tracked markdown
files with no entry at all.**

### What 0016 decided, and what survives

[[0016-writing-a-note-includes-its-entry]] refused to write a note without its keys, and named
the failure it prevents: *a note with no entry cannot be ranked by the keyword scorer at all,
so the only way to it is the full text scorer alone, which is the single scorer case this
system already reports as a guess rather than an answer.*

**That principle is not being reversed, it is being made structural.** 0016 enforced it with a
command that refuses. This record enforces it with a file that carries its own keys, so the
guarantee no longer depends on which command wrote the note. `kb write` keeps refusing without
keys. Only the destination changes.

## Options

### Option A: keep the map as the index source, fix the parser

- Cost: nearly nothing. `E07` shipped this morning and already catches the exact shape that
  failed.
- Failure mode: **it treats the instance and not the class.** The map remains a second place
  that must agree with the first, and 84 files remain unindexed because nobody listed them.
  Every future note is one formatting slip from invisible.

### Option B: keywords and a purpose line move into each file, the index walks files

- Cost: a format, a parser, a migration of 98 files, and four consumers rewritten.
- Failure mode: a file with no keyword section is invisible, which is today's failure moved
  rather than removed, unless the linter makes that loud. It is made loud below.

### Option C: extract keywords automatically from the file text

- Rejected without building it, on evidence already in this repository. Automatic extraction
  is what the full text scorer already does, and [[0018-no-model-in-the-retrieval-path]]
  measured what a second automatic signal buys. The value of the keyword line is precisely
  that a person chose the words: Richard's own example is `ai`, which is Artificial
  Intelligence and also the Portuguese interjection, and no extractor can tell those apart in
  a corpus that contains both. A curated list carries intent; an extracted one carries
  frequency.

## Decision

**Option B**, with three constraints the survey found and one demotion Richard did not ask for.

### 1. The format is two labelled lines, not front matter and not a section

Both alternatives were tested and both fail silently, which is the exact defect this record
exists to remove.

**Not YAML front matter.** A delimited block has a way to not exist. Measured: a file with no
front matter at all yields **zero** findings and `kb check` prints `clean`, because every
metadata finding is gated behind `if let Some`; a file that has front matter but omits
`provenance` yields two warnings. **The more broken file gets fewer findings.** A closing
fence typed `--` instead of `---` voided a block containing two hard `E04` errors down to
nothing. And 151 of 211 files carry no front matter today, Steve's 51 knowledge notes among
them, so a YAML design means hand-creating the fragile construct where it is least reviewed.

**Not an `## Keywords` heading either**, and this one is sharper. `store.rs` skips every line
while inside a keyword section, and a section runs until the next heading *of any level*. So a
keyword section placed at the top of a note **silently deletes the note's opening paragraphs
from the full text index**. Run, with the header shape as the only variable: with the section,
`kb route "intro prose searchable" --hybrid` returned *nothing matched, in either scorer*;
with the line form it found the prose. **A line excludes itself. A heading excludes everything
after it.**

So, directly under the title, before the body:

```markdown
**Search for:** `roteamento`, `router`, `quem responde`, `classificador`

**Exists to:** name the one thing this file is for, in a line, because the classifier is
shown this and never the file.
```

`Search for:` keeps its name deliberately: it is the string `store.rs` already excludes from
chunking, and every one of the 114 existing entries already uses it.

Read tolerantly, emit one shape. Case-insensitive label, bold or plain, comma or semicolon,
inline list or bullets, backticks optional, and the Portuguese `Buscar por:` accepted because
half this fleet writes Portuguese. Each of those was a real parse failure before it was a
tolerance.

### 2. A missing section is loud, and that is the whole point

`E02` keeps its sentence and changes its subject. It said *not indexed: no `[[note]]` entry in
MAP.md. A file nobody can find does not exist.* It now says the same thing about the file's
own header. `E03` and `E07` are deleted: one required a map, the other existed only because
the linter and the router read the map with different eyes, and there is no longer a map to
read two ways.

**A file is in the index if and only if it declares keys**, so a protocol or a template can
opt in, and a knowledge note cannot opt out.

### 3. The two scorers stay independent, because that is what fusion rests on

`store.rs` excludes `MAP.md` from the chunk store so the keyword scorer and the full text
scorer do not read the same words. Agreement between two independent signals is what
separates a hit from a guess here, and it stops meaning anything if they share a source.
Moving keys into files would defeat that exclusion. It moves from file level to line level,
which the chunker already implements.

### 4. `has_map` stops being what makes a directory a base

This is the blocker, and it fails closed: `expand_roots` decides a directory is a base by
asking whether it has a map. Remove the maps and **the fleet is not discovered at all**.

**A marker file cannot replace it, and the first version of this record said it could.** That
version proposed `agent.txt`, or an `attach` line, or a `knowledge/` directory. Checked
against the six live bases afterwards, `fleet/profile` has none of the three: no `agent.txt`
because it is not an agent, no `knowledge/` because it holds four files at its root, and no
`attach` line because it is not attached. **The published predicate would have dropped
Richard's own profile out of the fleet**, and `profile/core.md` is resident in every agent, so
the symptom would have been the person quietly disappearing from retrieval rather than an
error. MAP.md is, today, the only file all six bases share.

The predicate is therefore not a marker at all. **A base is an immediate subdirectory of the
fleet root**, which is what `fleet/` already means, plus the manifest's `attach` lines.
`bases_in` already enumerates exactly those children and then filters them by `has_map`; the
filter goes and the enumeration stays. The manifest already carries a `disable` list, so a
directory that should not be a base is named there, which is an opt-out that exists and is
read today rather than a new mechanism.

The general shape, because this is the second time in two days: **a rule invented at a desk
and checked afterwards is a rule that has not been checked.** Both alternatives in section 1
were ruled out by running them. This one was not, and it was wrong.

### 5. `MAP.md` is demoted, not deleted, and this is where Richard's instruction is narrowed

He said the map need not be kept, because a person can read the file itself. That reasoning is
right about people and the map stops being anybody's obligation. But the file is read by two
things that are not people:

- **It is a resident constitution block.** `[map] MAP.md` appears in all four agents'
  `blocks.txt`, worth 2,272 to 6,813 tokens of what each agent knows about itself at wake.
  Steve's map is 27 KB.
- **Its exclusion from the chunk store** is what keeps the scorers independent, and that
  exclusion is by filename.

So: the map stops being required, stops being the index source, and stops being checked. A
base without one works. A base with one keeps it as prose for a reader and as a resident
digest, maintained when somebody feels like it and never because a tool demands it. **Deciding
the `[map]` block's fate is a separate, measurable step**, because dropping ~6,800 resident
tokens from an agent is its own experiment and not a side effect of this one.

## Consequences

- **The index grows from 114 entries to whatever declares keys**, and `SCORE_FLOOR` is an
  absolute 6.0 against IDF sums that inflate with corpus size. The floor must be re-derived
  from the hit and miss ranges `kb eval` prints, exactly as its own doc comment demands. Not
  re-deriving it is how this change quietly breaks abstention.
- **83 of the 98 notes migrate mechanically**: their `Search for:` line already exists and
  only changes file. The remaining 15 need keys written, and 114 purpose lines need writing by
  hand: the map summaries average 524 characters and only 19 of 114 are under 160, so
  extracting the first sentence ships 95 bad ones.
- **The classifier's evidence stops being a path.** Measured on the same dossier: shown
  `steve/.../redacted-stories-algorithm-prompt.md score 15.3 matched: zero`, the classifier
  has to infer the subject from a filename. Shown *about: a copy-paste prompt that turns an
  LLM into an Instagram Algorithm Specialist*, it does not. Carrying the map summary into that
  slot moved owner selection from 9/10 to 10/10 and coverage not at all, and the one
  regression was a Yaron blurb mentioning food pulling a question about cat food. **That is
  the argument for a purpose line written for this job rather than a catalogue blurb reused
  for it.**
- A note is now readable and findable from the same file, which is what Richard asked for and
  the reason the format is visible plain text rather than metadata.

## Revisit trigger

- `kb eval` losing ground on the gold set after the floor is re-derived. The keyword scorer's
  match surface narrows from the whole map entry body to a curated list plus one line, and
  that is the intended trade, but it is a trade and it has a number.
- A base large enough that a person cannot review the keys, at which point the question is not
  the format but whether one base should be several.
- The `[map]` block measurement, whenever it happens, which either keeps the file alive for a
  reason or retires it.
