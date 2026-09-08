---
provenance: agent
stage: derived
---

# ADR-0042: a source is an object with an opaque key, and a citation is a pointer to it

**Search for:** `fonte`, `source`, `fontes`, `sources`, `citacao`, `citation`, `citar`, `cite`, `bibliografia`, `bibliography`, `referencias`, `references`, `lista de referencias`, `reference list`, `chave opaca`, `opaque key`, `chave estavel`, `stable key`, `identidade da fonte`, `source identity`, `src:`, `ponteiro de citacao`, `citation pointer`, `sources/`, `registro de fonte`, `source record`, `kb source add`, `kb sources`, `CSL JSON`, `csljson`, `Zotero`, `Better BibTeX`, `chave de formula`, `formula key`, `alfabeto sem zero`, `transcription safe alphabet`, `aniversario`, `birthday bound`, `colisao de chave`, `key collision`, `dez caracteres`, `ten characters`, `tipo de fonte`, `source type`, `retrieved_on`, `retrieval_status`, `data de acesso`, `access date`, `registro de catalogo`, `metadata only`, `fonte nao lida`, `source never opened`, `dc:replaces`, `substitui`, `supersede`, `correcao de fonte`, `source correction`, `E05`, `E06`, `E08`, `E09`, `W04`, `W09`, `regra opcional`, `opt in rule`, `evidence_tier`, `valid_for`, `nota sem tier`, `ungraded note`, `paragrafo de fontes em prosa`, `prose sources paragraph`, `Maslow 1965`, `Cidade Editora 2005`, `placeholder de template`, `template placeholder`, `projecao`, `projection`, `copia embutida`, `embedded copy`, `ADR-0003`, `ADR-0026`, `ADR-0029`

**Exists to:** record that a source becomes a first class object with an opaque ten character
key, that a citation is a pointer at it and the bibliography a checked projection of the
pointers, and that `kb check` gains the failures that make a reference list unable to disagree
with the body.

- **Date:** 2026-09-08
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. The records are plain text files under `sources/`, exportable
  as CSL JSON from the first commit, and the SQLite table is derived. Nothing on disk in any
  existing base changes and no note is rewritten. Removing this means deleting a directory and
  five check codes.

## Context

The fleet read 63 sources in one week into one base and shipped four citation defects that a
proofreader caught by hand. `Maslow (1965)` printed twice in a body with Maslow in no reference
entry. Lax and Sebenius dated 1996 in one paragraph and 1986 in another. `LAUREANO ... Cidade:
Editora, 2005`, a template placeholder that reached a tracked file. A body citing ten authors
against a list of fourteen.

**Every one of those is a divergence between restatements, not a mistake about a fact.** A
source was written down in three unconnected places: a free prose `Sources, read <date>:`
paragraph at the foot of a note, a row in an inbox `SOURCES.md` ledger, and a `captured_from`
string in front matter. Nothing joined them and nothing could compare them, so "the body and
the reference list agree" was a property maintained by attention.

Two measurements taken on 2026-09-08, before anything was built:

1. **204 knowledge notes across the fleet, 16 carrying `evidence_tier`, 17 carrying a front
   matter `source`, 11 carrying `captured_from`.** Cosimo's base is 73 notes with zero of each,
   and 29 of them carry a prose sources paragraph.
2. **`kb check --all` reported 0 W04 findings across the whole fleet.** W04 is the check that
   demands `evidence_tier` and `valid_for`, and its condition was `has("source") || has("type")`
   in front matter. Those are keys a writer adds or does not, so the evidence ruler graded only
   the notes whose author had already decided to be graded. All 63 sources of that week went
   ungraded and the linter said the base was clean.

`kb ingest`'s own doc comment asked for the fix a level below where it could apply it: *"the
deposit name is not an identity [...] two documents can leave notes carrying one
`captured_from` string, and afterwards nothing says which note came from which. That wants the
deposit name to carry something derived from the content."*

Zotero was read against its source for the mechanism (`utilities.js`, `relations.js`,
`editorInstance.js`, `userdata.sql`, GitHub `main`, fetched 2026-09-08). **Nothing was run**, so
none of what follows about Zotero is tier A. What it has and this did not is one thing: a source
is an object with an opaque key, and every mention of it is a pointer at the object rather than
a restatement of it. The bibliography is a projection of the pointers, so the two cannot
disagree. That is a structural impossibility, not a discipline.

## Options

### Option A: build the source object inside `kb`

- Cost: a new module, a directory of records per base, five check codes, and a fleet of 29 prose
  paragraphs that keep working and are not yet pointers.
- Failure mode: the records are a second thing to keep, and a base that mints a source and never
  cites it accumulates orphans. That is what E06 is for, and it is why E06 is an error.
- What it forecloses: nothing. The records export as CSL JSON, which is the shape Zotero,
  Pandoc and every processor already read, so option B stays available at the price of an
  importer.

### Option B: use Zotero as the source store and have `kb` read it

- Local HTTP API at `http://localhost:23119/api/`, `format=csljson`, and the `Note Markdown`
  translator already emits `zotero://select/library/items/<KEY>` into markdown. Everything
  needed exists.
- Cost: a GUI application must be running for `kb check` to pass. An XUL and Firefox-ESR runtime
  becomes a dependency of a binary that has one dependency on purpose. The sources leave git.
- Failure mode: the memory stops being readable without a vendor's runtime, which fails test two
  of the north star and contradicts the line the product is sold on. This is the option that
  should stay refused for a stated reason rather than by reflex.

### Option C: keep the prose paragraph and add a linter that parses it

- Cost: a parser for free prose in two languages, and it would be wrong often enough to be
  turned off.
- Failure mode: it can compare a paragraph against nothing, because there is no second copy that
  is authoritative. It would catch a malformed paragraph and never catch a wrong year, which is
  the defect that actually shipped.

## Decision

**Option A.** Steal the identity mechanism, not the application, and emit CSL JSON from the
first commit because it costs nothing today and keeps option B reachable.

### 1. The key: ten characters, opaque, minted once

`23456789ABCDEFGHIJKLMNPQRSTUVWXYZ`, 33 characters, `0`, `1` and `O` removed. Zotero's alphabet,
taken for the reason it exists: a key gets read aloud and copied off a screen, and those three
have a confusable partner.

**The length is arithmetic and not a copy of Zotero's eight.** A key is minted without
coordination, because bases do not see each other, private bases are not readable from outside,
and ADR-0037 assumes the whole thing can be restored onto a machine that has never met the
others. So uniqueness comes from the draw and the number that decides the length is the birthday
bound `p ~ n^2 / 2N` over the lifetime population.

The measured scale: the busiest week this fleet has had read 63 sources, against 204 knowledge
notes total. Sustained at three times that week, forever, one person's fleet reaches about 1e5
sources in fifty years. 1e6 is the paranoid bound, ten times wrong.

| length | N = 33^len | p at 1e5 | p at 1e6 |
|---|---|---|---|
| 8 | 1.41e12 | 1 in 280 | 1 in 2.8 |
| 10 | 1.53e15 | 1 in 306,000 | 1 in 3,060 |
| 12 | 1.67e18 | 1 in 3.3e8 | 1 in 3.3e6 |

Eight is enough in Zotero and not here, because Zotero enforces `UNIQUE (libraryID, key)` at
insert against a library it can see all of. Twelve buys three orders of magnitude and spends them
on a token nobody can hold in one glance, which undoes the reason for the alphabet. **Ten reads
as two groups of five and survives being wrong about the scale by a factor of ten.**

The draw is `RandomState`, seeded from the operating system's generator, with modulo bias
rejected rather than tolerated. The clock was refused for a measurable reason: Windows' default
timer granularity is milliseconds at best and often about 15 ms, so two sessions minting inside
one tick would collide.

**Never derived from content, path, title, author or a formula.** Better BibTeX's
`auth.lower + shorttitle + year` is human-readable-key ergonomics for LaTeX, and it changes the
day somebody fixes a misspelled author, which is the identity bug wearing a better face.

### 2. The record: one file per source, in `sources/<KEY>.txt`

`key = value` lines, the shape `agent.txt` and `kb-aliases.txt` already use. Six types, `book`,
`chapter`, `article`, `report`, `webpage`, `recording`, and not forty: forty exists to feed style
processors, which is out of scope, and CSL is the export for anything that needs one.

Required on every record: `type`, `title`, `retrieved_on`, `retrieval_status`. Optional and typed:
`author` (repeatable), `year`, `container`, `volume`, `pages`, `publisher`, `url`, `doi`, `isbn`,
`lang`, `replaces`, `note`.

**`retrieval_status` is the field that answers `Cidade: Editora, 2005`.** Four values, `full`,
`partial`, `metadata`, `unreachable`, and one has to be chosen out loud. `metadata` is the word
for "I saw the catalogue entry and never opened it", which the fleet has already needed once, for
Wahba and Bridwell 1976. The honest boundary, stated rather than glossed: **a typed field removes
the template, it does not remove lying.** A record is minted by `kb source add` from arguments,
so there is nothing to copy a placeholder out of, and somebody can still type a wrong publisher.
What it removes is the case where nobody was ever asked.

Three shape decisions, all deliberate:

- **One file per source, not one ledger.** More than one session writes these repositories at
  once (ADR-0021), and two sessions appending to one ledger conflict on every busy day.
- **`.txt` and not `.md`.** A record is structured data with no prose. As markdown it would join
  the chunk index, be graded for a keyword line it has no use for, and be reported by the house
  style check for an em dash inside somebody else's book title.
- **The path is derived from the key, never the reverse.** This repository has paid for
  path-keyed identity twice.

### 3. The citation: `[src:KEY]`, single brackets

**Not `[[src:KEY]]`, and the choice is not cosmetic.** Double brackets are this repository's
wikilink and ADR-0026 gives that syntax one meaning: a pointer to a note in this base, resolved
by file stem, refused across a base edge. Overloading it would make one syntax mean two things
and force the linter, the router, Obsidian and a person to each learn the exception. ADR-0029 is
the record of what that costs when it is allowed once. Single brackets carry no meaning here
today, `[1]` is already what a citation marker looks like, and `[text](url)` cannot collide with
a token that must open with `src:`.

A citation resolves inside its own base and nowhere else, which is ADR-0026's rule applied to the
new pointer for the same privacy reason.

### 4. The embedded copy lives in one bibliography line, and it is checked

Zotero puts a full CSL JSON copy of the item inside the note's citation span so the note survives
detachment from the library. That is the answer to "the video went private", and it is the part
that is easiest to get wrong: a copy a person maintains is a fourth restatement, which is the bug
this ADR exists to remove.

**So the copy is a rendering, and `kb check` compares it byte for byte.** A note that cites
`[src:KEY]` anywhere carries exactly one line of the shape:

```
- [src:K7M2QX4BTF] Maslow, A. H. "A Theory of Human Motivation". Psychological Review 50,
  370-396. 1943. https://psychclassics.yorku.ca/Maslow/motivation.htm. read in full, 2026-09-06.
```

`kb source add` prints that line when it mints the record, so the copy is a paste and never an
invention. **The rendering is a canonical form and not a citation style**, deliberately: every
title is quoted, including a book's, which no style would do, because equality has to be
decidable by a machine and readable by a person. Style is what CSL JSON is for.

That is the difference between a projection and a restatement, in one sentence: **a restatement
can drift and nothing notices; a projection cannot drift without failing a check.**

### 5. `kb check` gains three errors and one warning

| code | fires on | why that level |
|---|---|---|
| E05 | a `[src:KEY]` with no record behind it, or a malformed key | a claim with no source is not a claim |
| E06 | a source record no note cites | a record nothing points at is in no bibliography |
| E08 | a cited key with no bibliography line, or one that drifted from the record | without it, item 4 reintroduces the bug item 1 removes |
| W09 | a citation of a source another record supersedes | citing what was actually read is legitimate |
| E09 | a file under `sources/` that is not a record | a dropped record blames the note for a defect in the record |

Three errors and not the two the design called for. **E08 is load-bearing**: an embedded copy
nothing verifies is exactly the free prose paragraph it replaces, in a new place.

E06 is an error rather than a warning because a warning accumulates into a list nobody
reconciles, which is what the `SOURCES.md` ledgers already are.

### 6. `dc:replaces`: a correction supersedes, and the old key still resolves

A record carries `replaces = <OLDKEY>`. **One direction only**: a back pointer on the old record
would be a second fact free to disagree with the first, which is the failure this whole ADR
removes. The replaced record is found by scanning, and every record is loaded anyway.

Three consequences, and together they are the whole feature:

- **The old key keeps resolving.** The old record is never deleted, so no note breaks.
- **A superseded record is exempt from E06.** Without that exemption, correcting a source would
  fail the check the moment the last note moved to the new key, which is the opposite of what a
  correction should cost.
- **A note still citing the old key gets W09, naming the replacement.** A warning, because an
  error would force somebody to rewrite the history of their own reading.

This is what "Maslow 1965 is wrong, 1943 is right" should have produced, instead of two notes and
a prose sentence joining them.

### 7. W04 stops being opt in

The condition becomes a fact about the body: **the note cites something.** A `[src:KEY]` pointer,
or the hand written `Sources, ...:` paragraph that predates pointers. A note that cites a source
is making a sourced claim by definition, and that is the population the evidence ruler was
written for.

**Measured, before and after, on the fleet's fifteen bases with `--all`:**

| code | before | after |
|---|---|---|
| W04 | 0 | 58 |
| E01, E02, W03, W07, W08 | 43, 2, 43, 27, 4 | identical |

58 findings on **29 notes**, two each, and all 29 are the Cosimo notes that cite in prose. Nothing
else in the fleet's output moved.

**The stronger rule was considered and refused: every note in the knowledge folder.** It fires on
188 of 204 notes on the day it lands, which turns `kb check` into a wall people learn to scroll
past. This repository already recorded that failure once, in the reason E03 was removed.

### 8. CSL JSON from day one, and it is an export

`kb sources --json` emits a CSL JSON array with `accessed` carrying `retrieved_on`, which is the
field CSL has for exactly the thing the prose paragraphs were retyping by hand. `retrieval_status`
and `replaces` travel under `custom`, which is where CSL passes fields through untouched.

**Nothing reads CSL back**, because an importer is a translator and that is out of scope.

### 9. The SQLite `sources` table is derived and takes no migration

One row per record, rebuilt wholesale on each `sync`. `store.rs` wipes the index when a column is
added to `files`, because `sync` skips a file whose hash it already knows and a new column would
stay NULL on every note already indexed. **A table rewritten in full on every sync has no stale
row to migrate around**, so it gets `CREATE TABLE IF NOT EXISTS` and nothing else. It answers
"have we read this already", which nothing could be asked before.

## Consequences

- **`kb check` on the fleet goes from 8 errors and 8 warnings to 8 errors and 66 warnings.** The
  58 new warnings are the notes that were always making sourced claims and were never graded.
- **The 29 prose paragraphs are not retrofitted here, and that is deliberate.** A schema change
  and a content migration in one commit is how nobody can tell which half broke. The migration
  path is stated below.
- **A new writing convention that nothing enforces yet on old notes.** A note written from today
  cites by pointer; a note written last week cites in prose and now warns. Both work.
- **`sources/` is public.** It is not in the private default, because a bibliography is the part
  of a base that is safest to publish. A base that declares it private is honoured: `kb check`
  skips the family and `kb sources` refuses to list, or E06 would report every source as cited by
  nobody.
- **A citation inside the private layer still counts.** `kb check` scans the declared private
  folders for keys alone, never for paths, so a source cited only from `records/` is not reported
  as an orphan and no private path can reach a finding.
- **`kb write`'s dash refusal now has an exemption**, for bibliography lines only. House style
  governs our prose; a book's real title belongs to somebody else, and rewriting it to satisfy us
  would corrupt the one string this whole model exists to keep single.
- **Tests: 405 to 436**, all green, `cargo test` in `tools/kb`.

### What this deliberately does not build

Stated so the next reader does not think it was missed: no style engine and no CSL processor, no
browser connector and no translators, no sync, no blob storage of snapshots. `kb ingest`'s destroy
rule is a decided ADR and stands. No formula key.

### The migration path for the 29 prose paragraphs

Not run here. Per note, in this order, and it is reversible at every step:

1. `kb source add <base> --type ... --retrieval-status ...` once per source in the paragraph.
   `retrieval_status` is the field to think about: several of those 29 paragraphs already say
   "metadata only, paywalled, not read" in prose, and that sentence has a value now.
2. Replace the prose mention in the body with `[src:KEY]`.
3. Replace the paragraph with one `- [src:KEY] ...` line per source, pasted from what
   `kb source add` printed.
4. Add `evidence_tier` and `valid_for` to the front matter, which W04 is now asking for.
5. `kb check` goes clean on that note, and E06 stops firing for those records.

A note that is half converted fails E08 or E06 loudly, which is the property that makes doing them
one at a time safe.

## Revisit trigger

**A key collision, or a second person's fleet needing to share a key namespace with this one.**
The arithmetic above assumes one fleet minting without coordination. Two fleets pooling their
records multiply `n`, and at 1e7 the ten character key is at 1 in 30, which is the point where the
length is wrong rather than merely lucky.

The second trigger is **somebody needing a formatted bibliography for a journal or a university**.
That is the moment a CSL processor stops being out of scope, and the record set is already in the
shape one reads, which is what buying that door early was for.

The third is **E06 being turned off or worked around**. If a real workflow keeps producing sources
that legitimately nothing cites, the rule is wrong and the honest fix is a `retrieval_status` value
for "read and rejected" rather than a downgrade to a warning nobody reads.
