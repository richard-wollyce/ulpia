# kb

A linter for file based knowledge bases. Zero dependencies, one binary.

[ADR-0003](../../decisions/0003-knowledge-storage.md) decided that the markdown files stay the source
of truth and that any index is **derived** from them. This is the first derived thing. It does not
store anything, it reads the files and reports what the conventions promise while nothing was
checking.

It knows the three agents by shape, not by configuration:

| Agent | Map file   | Knowledge folder | Keyword line   |
|-------|------------|------------------|----------------|
| Zed   | `MAP.md`   | `knowledge/`     | `Search for:`  |
| Steve | `MAP.md`   | `knowledge/`     | `Search for:`  |
| Yaron | `MAPA.md`  | `conhecimento/`  | `Buscar por:`  |

## Use

```
kb check [path]... [--strict] [--all]
kb index [path]... [--all]
kb route <question> [path]... [--top N]
```

```
kb check ../../ ../../../steve ../../../yaron
kb route "quanto de proteina por refeicao" ../../ ../../../steve ../../../yaron
```

`check` reports what is broken. `index` emits the derived index as JSON. `route` answers which agent
and which files a question should open, by scoring it against the `Search for:` lines, with **no model
involved**: instant, free, explainable, and incapable of inventing a file that does not exist. It
prints which words matched, so a bad ranking can be diagnosed instead of guessed at, and it says
plainly when nothing matched rather than returning a confident guess.

### How route scores

Each query word is weighted by **inverse document frequency**, so a word that appears in most entries
carries almost no information about which file to open, which is arithmetic rather than opinion. A
multi word keyword found whole in the question scores highest. Hand written keywords outrank the title
and the file name, which outrank the entry prose.

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

**Zero dependencies is a choice, not a limitation.** Directory walking, front matter reading and the
wikilink parser are about two hundred lines of `std`. The cost of a regex crate here is a supply chain,
a compile time and a version to track, against parsing a bracket pair. The north star's efficiency
clause is not rhetorical: it decides real calls, and this is one of them.

**Two bugs found by running it on real bases**, both worth remembering:

1. **Case insensitive filesystems lie.** Asking Windows whether `INDEX.md` exists returns true when the
   file is really `index.md`, so Yaron's operating instructions were detected as its map, the map
   lookup then failed, and every map check was skipped **without a word**. Silent failure is the worst
   possible outcome for a checker. Names are now matched against what was actually collected from disk,
   case sensitively, and there is a regression test.
2. **A checker that misreads the convention manufactures work.** Counting every list item that opens
   with a wikilink produced 20 warnings demanding keyword lines for things that were not entries at
   all: indented sub items inside an entry, and cross references in a connections section. The rule is
   now exact, `- **[[name]]**` at the start of a line, and both shapes have tests.

Both were caught in the first ten minutes of real use, which is the argument for pointing a tool at a
real base before believing it.
