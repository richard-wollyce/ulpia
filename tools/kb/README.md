# kb

A linter, indexer and router for file based knowledge bases. One dependency, one binary.

[ADR-0003](../../decisions/0003-knowledge-storage.md) decided that the markdown files stay the source
of truth and that any index is **derived** from them. This is the first derived thing. It does not
store anything, it reads the files and reports what the conventions promise while nothing was
checking.

It knows agents by shape, not by configuration: any base under `fleet/` with tracked
markdown is served, `kb init` generates one, and every file declares its own
`Search for:` keyword line. The examples below use a three-agent fleet named
`zed`, `steve` and `yaron`, which is also the worked example the rest of this
repository uses.

## Use

```
kb check [path]... [--strict] [--all]
kb index [path]... [--json] [--all]
kb route <question> [path]... [--top N] [--hybrid] [--json]
```

```
kb check fleet/zed fleet/steve fleet/yaron
kb index fleet/zed fleet/steve fleet/yaron
kb route "quem decide qual agente responde" fleet/zed fleet/steve fleet/yaron --hybrid
```

`check` reports what is broken. `index` emits the derived index as JSON. `route` answers which agent
and which files a question should open, by scoring it against the `Search for:` lines, with **no model
involved**: instant, free, explainable, and incapable of inventing a file that does not exist. It
prints which words matched, so a bad ranking can be diagnosed instead of guessed at, and it says
plainly when nothing matched rather than returning a confident guess.

### `kb route --json`: the same answer, for a program

```
kb route "how do I roll back" fleet/zed --json
```

One line of JSON on stdout, and it is **not a serialisation of the terminal output**. The terminal has
two modes because a person reading a ranked list and a person reading passages want different things.
A program wants both plus the verdict, in one call, which is what the contract computes in one pass
anyway: the owner and the verdict from the keyword ranking, the reading from fusion. So `--json`
always fuses and `--hybrid` adds nothing on top of it.

```json
{
  "question": "how do I roll back",
  "verdict": "hit",
  "confidence": { "agreement": 2, "keyword_score": 24.7, "margin": 2.13 },
  "agent": { "name": "zed", "score": 0.0328, "files": 1, "margin": null,
             "contenders": 1, "totals": [{ "agent": "zed", "score": 0.0328 }] },
  "keyword_top": "zed/knowledge/deploy-checklist.md",
  "indexed": { "entries": 11, "agents": 3, "aliases": 0 },
  "skipped": [],
  "index_was_rebuilt": false,
  "suggestions": [],
  "results": [{ "base": "zed", "path": "knowledge/deploy-checklist.md",
                "title": "...", "purpose": "...", "score": 0.032787,
                "keyword_score": 24.7, "why": ["keywords #1", "text #1"],
                "matched": ["rollback"],
                "passages": [{ "heading_path": "...", "text": "...", "excerpt": "...",
                               "provenance": "human", "stage": null }] }]
}
```

Four things in there will bite a caller that assumes otherwise, so they are stated rather than
discovered:

- **`verdict` is `hit`, `guess` or `nothing`**, measured against a keyword floor of 17.5 that is
  calibrated in the open (see `SCORE_FLOOR` in `memory.rs`). A small base produces `guess` often, and
  `guess` means show it and say it is weak, not hide it.
- **`skipped` names bases that were left out** because git could not be consulted there. A caller
  reading stdout alone would otherwise see an empty `results` and conclude the base does not cover the
  question, when in fact nothing was searched. See the deployment section below, where this is the
  default outcome rather than an edge case.
- **`agent.margin` is `null` when only one agent scored.** That is JSON's encoding of infinity, and it
  means maximum confidence, not a missing field.
- **Scores are rounded to six decimals.** The keyword score is an `f32`, so widening it to `f64` would
  print seventeen digits of precision it never had.

Errors go to stdout as `{"question": ..., "error": ...}` with exit code 1, and to stderr as the same
sentence a person would read.

### The index

`kb index` builds a SQLite file at `<base>/.kb/index.db`, **one per base, and there is no flag to
point it anywhere else**. That is a decision rather than an omission: a shared index defaulting to the
working directory meant which database you got depended on where you were standing, and it cost three
separate incidents in one week. To index a different base, name that base. To try a different index
over the same content, copy the content, which is exactly as annoying as it should be.

**It holds nothing that cannot be recomputed from the markdown**, which is what keeps ADR-0003
literally true: delete it and the next `kb index` rebuilds it.

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

### `kb blocks`: what the agent wakes up knowing, and what it costs

```
kb blocks .            # the report
kb blocks . --emit     # the assembled resident constitution
```

An optional `blocks.txt` at the base root declares the constitution as labelled blocks, **in stability
order, most stable first**, each marked `resident` or `on-demand`.

The order is a mechanism rather than a preference. Prefix caching reuses the KV state of a prompt only
up to the first token that differs, so a change invalidates its own block and everything after it. Put
a frequently changing block early and every switch pays to recompute the stable ones behind it, for
nothing.

The report prints exactly that asymmetry:

```
  #   block      mode       files     bytes   ~tokens  cumulative
  1   identity   resident       3     23643      5911        5911
  2   user       resident       1      9595      2399        8310
  3   map        resident       1     25617      6404       14714

  cost of changing a block, in tokens that have to be prefilled again:
    identity     14714
    user          8803
    map           6404
```

**The measurement it produced immediately:** the map is 44% of Zed's resident set and **69% of
Steve's**, after the map was already cut from 99 KB to 24 KB. It is the largest resident block and the
one that changes most often, which is the worst combination available.

It is still resident only because the agent routes by reading it. **The moment `kb route` is wired
into the loop, the map becomes on-demand and the resident set drops by 46% across the fleet**, because
routing is the map's whole job and it will be happening outside the model by then.

### `kb remember`: the write side

```
kb remember "the fts5 tokenizer is unicode61 with remove_diacritics 2" fleet/zed
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

### `kb::memory::Memory`: the contract

`kb` is a library as well as a binary. **Start at `Memory`**: three verbs over a set of bases, and the
one place an answer is computed.

```rust
let memory = kb::memory::Memory::open(&[Path::new("../../")], false, Path::new(".kb/index.db"))?;

memory.route("who decides which agent answers", 5);     // which files to open
memory.retrieve("who decides which agent answers", 5);  // the passages themselves
memory.remember("the index is rebuilt on demand");      // ADD / UPDATE / NOOP, writes nothing
```

The `serve` subcommand wraps it in MCP for other people's runtimes, the Tauri GUI links it and calls it
directly, and a hosted service later would wrap it in HTTP. Three surfaces, one pipeline, no way for
them to answer differently.

**Reaching past it into the modules means rebuilding the pipeline**, which is how two of the bugs
listed at the bottom of this file happened: alias expansion reaching one scorer and not the other, and
the two oversampling factors drifting apart. `mcp.rs` did assemble it by hand for one commit, and that
is exactly why it does not any more.

`Memory::open` refuses a base whose privacy is unknowable, meaning git could not be consulted, unless
the caller explicitly asked for the private layer. Unknown is not public.

#### A fleet root, accepted and never required

A path may be a base, or a **fleet root**: a directory that is not itself a base but whose immediate
children are. Both work.

```
kb serve C:\fleets            # finds every base under it
kb serve C:\fleets\zed        # just the one
```

Requiring an arrangement would be an assumption about the user's filesystem, which ADR-0008 forbids in
as many words. Accepting one makes a tidy layout a convenience rather than a shape imposed on anyone,
so grouping agents under a parent is optional tidying instead of a migration.

A directory holding a map of its own is a base and is never expanded, even when it contains bases:
expanding it would silently drop the parent's own notes.

### `kb serve`: the base as an MCP server

```
kb serve [path]... [--top N] [--all]
```

Speaks the Model Context Protocol over stdio, so any MCP client can search the base: Claude Code,
Claude Desktop, or our own GUI driving a local model. **The server does not know which one is calling
and does not care.** That is the reason retrieval lives here rather than in the GUI: with a cloud model
the passages travel in the prompt, with a local model nothing leaves the machine, and both read the
same code.

Three tools, all read only:

| Tool | Returns |
|---|---|
| `kb_route` | Ranked file paths with the words that matched. Cheap, no file contents |
| `kb_retrieve` | The passages themselves, with heading path and provenance |
| `kb_remember` | The ADD / UPDATE / NOOP proposal and its evidence. **Writes nothing** |

There is no write tool yet, deliberately. A write reached by a model is a different security surface
and gets built on purpose rather than while the retrieval side is still warm.

**The private layer stays out unless asked.** `profile/`, `projects/` and `records/` are gitignored
because they are private, and what a tool returns travels to whatever model is reasoning, so the
default is what git tracks. `--all` includes them and is a deliberate act visible in the client's
config file.

**It refuses to start when git cannot be consulted.** Then every file's privacy is unknown, and
unknown is not public. Either the base is a git repository or `--all` says you meant it.

Registering it with Claude Code, project scope, from `.mcp.json` at the repository root:

```json
{
  "mcpServers": {
    "zed-memory": {
      "type": "stdio",
      "command": "${CLAUDE_PROJECT_DIR}/tools/kb/target/release/kb.exe",
      "args": ["serve", "${CLAUDE_PROJECT_DIR}"]
    }
  }
}
```

For all three agents at once, user scope, which stays out of git:

```
claude mcp add --transport stdio --scope user fleet-memory -- \
  C:\fleets\zed\tools\kb\target\release\kb.exe serve \
  C:\fleets\zed C:\fleets\steve C:\fleets\yaron
```

The `--` is mandatory: everything after it is passed to the server untouched.

**claude.ai in a browser cannot use this**, and that is documented rather than a limitation of ours: a
web page cannot spawn a local process, and Anthropic's custom connectors dial out from their
infrastructure rather than from your machine, so a `localhost` URL resolves to their servers. Claude
Code and Claude Desktop both run locally and both work.

#### What the protocol demanded, and what it cost

**Dual-era.** Revision 2026-07-28 removed the `initialize` handshake and moved the version into
per-request `_meta`; 2025-11-25 and earlier require the handshake. The spec names an implementation
speaking both "dual-era", so the server answers `initialize` when asked, never requires it, and echoes
back whatever version the client named rather than asserting one. Both paths are tested against the
real binary.

**stdout belongs to the protocol.** Verbatim from the spec: "The server MUST NOT write anything to its
stdout that is not a valid MCP message." Every `cmd_*` in `kb` prints to stdout, so the serve path
sends every diagnostic to stderr instead, which the spec explicitly allows and which clients are told
not to read as failure.

**The JSON is hand written**, in `json.rs`, keeping the one dependency. What makes a JSON parser hard
is escapes: `\"`, `\\`, `\uXXXX` and the surrogate pairs that carry anything above U+FFFF. Accents are
not hard and never were, because Rust strings are UTF-8. The tempting shortcut of folding accents
before parsing was considered and rejected: it would destroy the framing, since a quote inside a string
arrives as `\"`, and on the `remember` path the claim becomes a file, so folding would corrupt the base
permanently and silently. Diacritic folding already happens where it belongs, in the FTS5 tokenizer,
applied to search terms after parsing and never to text on its way to disk.

Two bounds exist because the input arrives from another process: nesting is capped, so `[[[[[...` is an
error rather than a stack overflow, and output is always one line, so a passage containing a newline
cannot split one message into two and desync the stream for good.

### Running it somewhere that is not your machine

`kb` is local first and that is a design position, not a limitation waiting to be lifted. It reads the
markdown on every run and the index lives beside the files. Nothing here talks to a server, and there
is no hosted instance to point at: the base has to be on the same filesystem as the binary.

That works on a server as well as on a laptop, and four things about it will surprise anyone who
deploys it without reading this. Each one says what it stands on, because three of the four are cheap
to assume and expensive to be wrong about.

**1. Git decides what is public, so a deployment without git serves nothing.** *Run.*

The privacy filter asks `git ls-files` from inside each base. No `.git` in the bundle, or no `git` on
the host, and the answer is "unknown", which is not "public": every base is left out with a notice on
stderr, and the reply is an empty result set. Reproduced by copying `examples/demo` outside any
repository and routing against it. Two ways through, and they are not equal:

- **Deploy a base that is entirely publishable and pass `--all`.** The privacy boundary moves from
  "git says tracked" to "this bundle contains nothing private", which is a real guarantee only if you
  built the bundle that way. Recommended, because it is checkable at build time.
- **Ship `.git` and the git binary.** Keeps the original guarantee and pays for it in bundle size and
  in a runtime dependency most serverless images do not have. Unverified here.

Either way, read `skipped` in `kb route --json`. It is the field that separates "the base does not
cover this" from "no base was searched".

**2. The filesystem is usually read only, and that is survivable.** *Read the source. Not run against a
read-only filesystem.*

The index is built before the deploy, by `kb index`, and only read at runtime. The one file written on
a query is `kb-misses.txt`, the recall loss log, and a failed write there prints on stderr and does
not fail the query. So a read-only deployment answers correctly and loses the miss log. Writes to the
base itself, `kb write` and `kb promote`, do not belong on that machine at all.

**3. Every process start pays the cold open.** *Measured on a laptop, not on a deployment.*

From `benchmarks/latency`: 136.4 ms to open the fleet and answer the first question, then p50 0.68 ms
warm over 1000 samples. Spawning `kb route --json` per request pays the 136 ms every time. Keeping one
`kb serve` process alive and speaking MCP to it pays it once per process. Start with the spawn,
because it is four lines of code, and move to the long-lived process when the number starts mattering.

**4. Match the libc, or link none.** *Run, in CI, on 2026-08-28.*

A Linux binary built against a modern glibc will not start on the older glibc that serverless images
carry, because glibc symbol references are versioned. `.github/workflows/release.yml` builds
`x86_64-unknown-linux-musl` instead. `file` reports the result as `static-pie linked, stripped`, and
the same artifact then runs `kb check --all examples/demo` inside `amazonlinux:2023` and inside
`alpine:3`, clean in both, before anything is published.

**That check is the whole point of the file and it runs on every release**, because the failure it
catches is invisible until the deploy: a glibc build goes green in CI, uploads, and dies on the host
with a missing symbol version. A portability claim nothing executes is a rumour.

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

- **It does not check whether a note is any good.** That is the bar, a
  protocol in the private layer, and it is not automatable.
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
package fixed it. Recorded as F4 in the fleet backlog, in the private layer.

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
