# kb

A linter, indexer and router for file based knowledge bases. One dependency, one binary.

[ADR-0003](../../decisions/0003-knowledge-storage.md) decided that the markdown files stay the source
of truth and that any index is **derived** from them. This is the first derived thing. It does not
store anything, it reads the files and reports what the conventions promise while nothing was
checking.

It knows the three agents by shape, not by configuration:

| Agent | Map file   | Knowledge folder | Keyword line   |
|-------|------------|------------------|----------------|
| Zed   | `MAP.md`   | `knowledge/`     | `Search for:`  |
| Steve | `MAP.md`   | `knowledge/`     | `Search for:`  |
| Yaron | `MAP.md`   | `knowledge/`    | `Search for:`  |

## Use

```
kb check [path]... [--strict] [--all]
kb index [path]... [--db FILE] [--json] [--all]
kb route <question> [path]... [--top N] [--hybrid] [--db FILE]
```

```
kb check ../../ ../../../steve ../../../yaron
kb index ../../ ../../../steve ../../../yaron
kb route "por que o poke e caro em proteina" ../../ ../../../steve ../../../yaron --hybrid
```

`check` reports what is broken. `index` emits the derived index as JSON. `route` answers which agent
and which files a question should open, by scoring it against the `Search for:` lines, with **no model
involved**: instant, free, explainable, and incapable of inventing a file that does not exist. It
prints which words matched, so a bad ranking can be diagnosed instead of guessed at, and it says
plainly when nothing matched rather than returning a confident guess.

### The index

`kb index` builds a SQLite file, `.kb/index.db` by default. **It holds nothing that cannot be
recomputed from the markdown**, which is what keeps ADR-0003 literally true: delete it and the next
`kb index` rebuilds it.

Two objects, and you can open both with any `sqlite3` binary:

```sql
files  (base, path, hash, provenance, stage)         -- one row per file
chunks (base, path, heading_path, text)              -- FTS5, tokenize unicode61 remove_diacritics 2
```

**Chunking splits at heading boundaries**, then splits any section over ~1800 characters into windows
with 300 characters of overlap, cutting at a blank line when there is one. Heading boundaries are free
structure: the note template already forces named sections, so a chunk means something on its own
without a parser. The heading path travels with the chunk, `Title > Section > Subsection`, because it
carries context the chunk itself rarely repeats.

**Content hashing decides what gets reindexed.** Unchanged file, skipped. Changed file, its chunks
replaced. Deleted file, its rows removed. Reindexing 111 unchanged files is instant, which is what lets
this run on every startup instead of being a chore.

The hash is `std`'s SipHash, deliberately not cryptographic: the question is "did this change since
last time", and the adversary is a text editor.

### How route scores

Each query word is weighted by **inverse document frequency**, so a word that appears in most entries
carries almost no information about which file to open, which is arithmetic rather than opinion. A
multi word keyword found whole in the question scores highest. Hand written keywords outrank the title
and the file name, which outrank the entry prose.

With `--hybrid`, that keyword ranking is fused with BM25 full text search over the chunks, using
**Reciprocal Rank Fusion**: each list contributes `1 / (60 + rank)` to every document it ranks, and the
sums are compared. RRF uses position and ignores the raw scores on purpose, because a BM25 value and a
keyword score live in different numeric universes and normalising them into a weighted sum means
inventing a conversion factor and then tuning it per corpus. Ranks need no conversion.

**The two scorers are kept independent, and that is a design constraint rather than an implementation
detail.** RRF reads agreement between lists as strong evidence, and agreement is only evidence when the
lists are looking at different things. So the `Search for:` lines and the map file itself are excluded
from the text index: they are the keyword scorer's corpus, and indexing them twice would make one
scorer wearing two hats look like two scorers agreeing.

A file contributes to the fusion **once**, at the rank of its best chunk. Scoring every matching chunk
turns the ranking into "which file has the most matching pieces", which is a different question and
usually the wrong one.

### `kb remember`: the write side

```
kb remember "the calorie floor for men is 1600 kcal" ../../../yaron
```

Measures a claim against what the base already says and **proposes** one of ADD, UPDATE or NOOP, with
the evidence that produced the proposal: the closest chunks, how much of the claim each already
contains, and which words are shared and which are new.

**It decides nothing.** That is the design, not a limitation. mem0 lets a similarity score decide, and
a similarity score cannot tell "the user changed their mind" from "two things are true in different
contexts", which is why it deletes live facts in silence. A number can say how much two texts overlap.
It cannot say whether the older one is now wrong.

The measure is **containment**, the fraction of the claim's words already present in one chunk, rather
than Jaccard. Jaccard divides by the union, so a short claim sitting inside a long section scores low
however completely the section already covers it, which is the opposite of the question being asked.

| Containment | Proposal | Reading |
|---|---|---|
| ≥ 0.90 | **NOOP** | The base already says this. Writing it again is the duplicate that grows a base without adding to it |
| ≥ 0.55 | **UPDATE** | Substantial overlap with a small difference, which is what the same fact with a new value looks like |
| below | **ADD** | Related material at most, not the same fact |

**DELETE is never proposed**, and the tool says so on every run. Deleting needs the claim to be false
rather than absent, and no count of shared words can tell those apart. Per ADR-0007 the agent may
delete on its own; the constraint is that the reason goes in the commit message.

A useful side effect, found on the first real run: asking about the calorie floor returned containment
1.00 from **two** files, which is the base telling you a fact is written down twice.

### The alias table

An optional `kb-aliases.txt` at the base root, one `alias = canonical` per line, `#` for comments.
Expansion is **additive**: the original words always survive, so a wrong alias can add noise and can
never remove signal.

It exists because a question can be entirely correct and still match nothing. Translation is the loud
case: an English base asked in Portuguese. The quiet case is the same shape and more common, a
Portuguese base asked in Portuguese where the file is keyed `sono` and the person said `dormir`. The
table is really about **the distance between how someone asks and how a file was indexed**.

Only add a line after a real question missed. It is a record of misses, not a dictionary, and a
dictionary is what makes it unmaintainable.

- `--strict` counts warnings toward the exit code, which is what a commit hook wants.
- `--all` includes files git does not track. By default only tracked files are checked, because the
  private layer is gitignored by design, it is nobody's to publish, and linting it buries the findings
  that matter under noise from files we would never edit.

Exit code is 1 when there are errors, or when `--strict` and there are warnings. Everything else is 0.

## Checks

| Code | Level | What it catches |
|------|-------|-----------------|
| E01  | error | A `[[link]]` with no file behind it |
| E02  | error | A note in the knowledge folder with no entry in the map. A file nobody can find does not exist |
| E03  | error | No map file at the root |
| W01  | warn  | A `[[link]]` matching more than one file, so it is ambiguous |
| W02  | warn  | A map entry with no `Search for:` line, so grep cannot route to it |
| W03  | warn  | An em dash or en dash, which house style forbids |
| W04  | warn  | A note declaring a source with no `evidence_tier` or `valid_for` |
| W05  | warn  | A note with no `provenance` or no `stage`, so who wrote it is unknown |
| E04  | error | `provenance` or `stage` carries a value outside the legal set |

Links inside fenced blocks and inline code are ignored, because a base that documents its own link
convention writes `[[file-name]]` in backticks and those are examples, not references. The
`templates/` folder is exempt from link checks for the same reason: its links are placeholders.

## What it deliberately does not do

- **It does not check whether a note is any good.** That is [the bar](../../protocols/the-bar.md), and
  it is not automatable.
- **It does not fix anything.** It reports. Applying the fix is a decision.
- **It does not index the private layer** unless asked with `--all`.
- **It has no notion of staleness yet.** `valid_for` is required on sourced notes but nothing compares
  it to reality. That needs to know what is installed, which is the next honest step, not a guess.

## Build

```
cargo test
cargo build --release
```

The binary lands in `target/release/kb.exe`.

Historical note, because it cost an hour and the mechanism generalises: this used to require calling
the rustup toolchain by absolute path. A Chocolatey Rust package with an incomplete MinGW environment
was shadowing rustup, so every build compiled and then failed to link. A PATH reorder could not fix it,
because Windows composes the machine PATH before the user PATH, and cargo resolves `rustc` through
PATH, so even calling the correct `cargo.exe` directly still picked up the wrong compiler. Removing the
package fixed it. See F4 in [the fleet backlog](../../fleet/backlog.md).

## Design notes

**One dependency, and the stance behind it changed on purpose.** For a linter that matches brackets,
zero dependencies was right: a regex crate would have bought a supply chain against parsing a bracket
pair. It stopped being right at the index. SQLite brings FTS5 with BM25 built in, holds the chunks and
their metadata in the same file, and stays readable with any `sqlite3` binary, which is the property
the whole design rests on. Writing a B-tree and a full text index by hand to avoid it would be the same
mistake pointed the other way. `rusqlite` with `bundled` compiles SQLite in, so there is still no
system package and nothing to run.

**Five bugs found by running it on real bases**, all worth remembering:

1. **Case insensitive filesystems lie.** Asking Windows whether `INDEX.md` exists returns true when the
   file is really `index.md`, so Yaron's operating instructions were detected as its map, the map
   lookup then failed, and every map check was skipped **without a word**. Silent failure is the worst
   possible outcome for a checker. Names are now matched against what was actually collected from disk,
   case sensitively, and there is a regression test.
2. **A checker that misreads the convention manufactures work.** Counting every list item that opens
   with a wikilink produced 20 warnings demanding keyword lines for things that were not entries at
   all: indented sub items inside an entry, and cross references in a connections section. The rule is
   now exact, `- **[[name]]**` at the start of a line, and both shapes have tests.

3. **A guard applied after normalisation is not a guard.** The front matter parser trimmed a key and
   then tested the trimmed key for leading whitespace, so the check for nested keys could never fail
   and shipped broken until its own test caught it.
4. **The alias expansion reached one scorer and not the other.** A Portuguese question routed correctly
   by keyword and matched zero chunks by text, because the English term was substituted on one side
   only. Both scorers now receive the same expanded terms from one call.
5. **RRF ranks documents, and the text list ranks chunks.** Scoring every matching chunk made a long
   file accumulate one contribution per section, so the ranking quietly became "which file has the most
   matching pieces". The safety protocol, which defines the calorie floor in a single table row, lost
   to a longer file that mentioned it three times in passing.

All five were caught in the first minutes of real use, which is the argument for pointing a tool at a
real base before believing it. Three of the five were **silent**: they produced a plausible answer
rather than an error, which is the class of bug a test suite finds and a demo never does.
