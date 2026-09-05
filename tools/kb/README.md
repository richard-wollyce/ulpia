# kb

A linter, indexer and router for file based knowledge bases. One dependency, one binary.

[ADR-0003](../../decisions/0003-knowledge-storage.md) decided that the markdown files stay the source
of truth and that any index is **derived** from them. This is the first derived thing. It does not
store anything, it reads the files and reports what the conventions promise while nothing was
checking.

It knows agents by shape, not by configuration: any directory with markdown in it is a
base and is served, `kb init` generates one in the full shape, and every file declares
its own `Search for:` keyword line. No repository is needed and none is consulted. The examples below use a three-agent fleet named
`zed`, `steve` and `yaron`, which is also the worked example the rest of this
repository uses.

## Use

```
kb check [path]... [--strict] [--all]
kb index [path]... [--json] [--all]
kb list [path]... [--base B] [--folder F] [--kind K] [--stage S] [--provenance P] [--json] [--all]
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

## What is in the box, and the pain each piece answers

Twenty-two verbs. Each exists because something went wrong without it, and the table says what.
A verb whose pain you do not have is a verb you do not need to learn.

| Verb | The pain | What it does | What it never does |
|---|---|---|---|
| `route` | "Which of my notes answers this?" is a question people answer by grepping and models answer by guessing | Scores the question against every note's `Search for:` line and its text, names the owner agent, returns the passages and a verdict: `hit`, `guess` or `nothing` | Calls a model. Invents a file. Returns a rank one for a question the base does not cover |
| `answer` | Retrieval hands you passages; you still have to read them | Runs `route`, then hands only what was served to the model named in `fleet.txt`, which must cite it or say the passages do not hold the answer | Reaches the model on a `nothing` verdict. Lets the model see anything retrieval did not serve |
| `remember` | "Is this worth writing down, or do I already have it?" | Measures a claim against the base and proposes ADD, UPDATE or NOOP, with the overlapping passage as evidence | Writes. Decides. Proposes DELETE, because absence and falsehood look the same to a word count |
| `write` | A note without its keyword line is a note nothing can find, and people forget the line | Writes the note with its `Search for:` header and its map entry in one step, and refuses without keys | Writes half: a failed map entry deletes the note again |
| `promote` | Raw material piles up in an inbox and nobody distils it | Two promoters, three questions, unanimity: the first proposes notes without seeing the base, the second decides without seeing the first's reasoning. Writes at stage `captured` | Writes on a split decision. Starts over another run's lock when `--lock` is given |
| `check` | A broken link, a missing keyword line, an em dash: each one is invisible until it costs an hour | Lints every base: E01 broken link, E02 not indexed, W06 thin keywords, and the house rules | Fixes anything. Touches a file |
| `index` | Full text search needs an index, and an index that drifts from the files lies | Builds one SQLite file per base from the markdown, content hashed so unchanged files cost nothing, and counts what it could build no entry for: the files no question can reach, and the ones exempt by name | Holds anything that cannot be rebuilt from the files. Runs in the background |
| `list` | A filter question has no ranking problem in it, so scoring it against a floor answers a guess | Lists the files a base holds, narrowed by base, folder, species, stage or provenance, with no score and no verdict | Rank anything. Take a question. Hide a file because no question can reach it |
| `eval` | "Does retrieval work?" answered by feel | Grades the router against a gold file of questions and expected answers, including questions it is supposed to refuse, and prints hit, guess and refusal rates | Grades a gold file that names files the fleet does not have |
| `boot` | Every session starts as nobody, and picking an agent by reading a conditional woke the wrong one | On a hook, routes the incoming message across the fleet, picks the owner, and injects that agent's constitution before the model reads anything. When the work lands in more than one domain it names the panel too, and hands over the `kb panel` command that opens the round | Picks an agent when no base covers the message. It says so instead. Convenes a panel from arithmetic, which was measured and cannot separate the cases |
| `blocks` | A constitution is several files, and assembling them by hand drifts | Assembles the resident blocks in order and reports what is missing | Invents a block that is not on disk |
| `fleet` | "Who is in this fleet and what does each one do?" | Reads `fleet.txt` and every `agent.txt`, and prints the roster | Reads the index. Identity is never derived from retrieval |
| `init` | An agent created by hand is missing the one file that makes it findable | Generates a base with the shape the router needs, or a person base with the questions a fleet must answer about its human | Writes a word about the person. The skeleton is empty on purpose |
| `commit` | Two sessions writing one repository, and `git add -A` sweeping a stranger's work into your message | Commits exactly the paths named, then reads the commit back and prints what it left dirty | Offers a flag meaning everything |
| `serve` | Other people's runtimes need the same answers, not a port of the pipeline | Speaks MCP over stdio: `kb_route`, `kb_retrieve`, `kb_remember`, `kb_fleet`, `kb_list`, all through the same `Memory` the CLI uses | Writes to stdout anything that is not protocol. Serves a base the caller did not name |
| `ui` | Reading a base through a terminal is reading a library through a keyhole | A local reading room over the same contract: shelves, books, broken citations shown rather than hidden | Serves a file discovery did not produce, however the path is spelled |
| `capture` | A session ends and everything it could not answer ends with it | Turns the session's record, appended by `boot` on every message, into one raw file in the last routed agent's `inbox/`: the refused questions with the vocabulary offered back, and where the conversation went. Then `promote` reads it | Runs a model. Writes a `Search for:` line, so the router never names a raw session as an answer. Captures a session no agent was routed in |
| `panel` | A piece gets reviewed by whoever is in the room, and the objection that killed it is remembered by nobody | Boots a named panel from each agent's own `blocks.txt`, prices the round before it is spent, and keeps a ledger where every objection is taken, refused with a reason, or escalated | Call a model. Choose the panel for you. Let a blocking objection be refused, or a reviewer's silence be recorded as agreement |
| `misses` | The log records what was asked and could not be answered, and stops there. The file that nearly held the answer, and the key it was missing, is the half nobody can look up | Reads `kb-misses.txt` back, most asked first, and beside each question names the files today's index nearly caught it with, the keys each of those files declares, and the path it read | Write anything. It proposes the alias line and a person adds it |
| `misroute` | The router hands a message to the wrong agent, the agent answers anyway or says so in chat, and the evidence dies with the conversation | Records that a named agent was handed a message a different agent owns, in the owner's own words, so a routing fault becomes a countable fact instead of an anecdote | Edit a base. Decide who was right. It is evidence, and `kb misses --apply` is the only thing that proposes a fix from it |
| `misroutes` | One misroute is an anecdote and thirty are a map, but only if something reads them back | Reads `kb-misroutes.txt`, most reported first, so a pair of bases that keep being confused for each other is visible as a pattern rather than remembered as a grievance | Write anything. Rank by anything but how often the same fault was filed |
| `abstentions` | The router finding that nobody owns a subject is the fleet's most useful failure, and it was the only one that left no trace: the miss log fires on retrieval, and a misroute needs an agent to file it, and an abstention has neither | Records every coverage abstention keyed on the classifier's subject rather than the message, with its reason and the bases that scored anyway, then reads them back with what each would score today | Record a refusal that is not a coverage judgement. Key on the message, which would leave every gap at count one forever |

### How the pieces make two memories

The verbs above are one system, and the shape of it is two memories with a filter between.

| | Where it lives | What feeds it | What reads it |
|---|---|---|---|
| **Short memory**, fresh and unjudged | `inbox/` in each agent, plus `kb-misses.txt`, the questions the base could not answer | Files a person drops, and `capture` at session end: what the session refused and where it went, without a model | `promote`, `misses`, and every question, labelled short |
| **The filter** | `promote`, with `remember` as its measure | The short memory | Nobody. It writes or it refuses |
| **Long memory**, the library | `knowledge/` in each agent, with a `Search for:` line on every note | `write`, `promote`, and a person editing markdown | `route`, `answer`, `boot`, `serve`, `ui`, every question anybody asks |

Both memories are searched, and that is the decision rather than a gap. The router routes to the
library: a note without a `Search for:` line has no index entry, and the short memory has none by
design. The text scorer reaches both, so a raw drop in `inbox/` can surface as a passage, and when it
does it arrives **labelled**: `memory: "short"` in the JSON, `[SHORT MEMORY: recent, not distilled]`
on the passage header the model reads, `[short memory]` on the terminal line. The rule travels with
the label: a model may use it, and if it does it says the claim comes from short memory. Hiding the
deposit would lose real facts; serving it unmarked would let a raw drop read as settled knowledge;
serving it marked leaves the decision where it belongs, with whoever is reading, made consciously.
[ADR-0034](../../decisions/0034-git-leaves-the-runtime.md). A fact enters the library by one of three
doors, a person writing it, `write` writing it on their behalf, or `promote` admitting it unanimously,
and each door leaves provenance on the note saying which.

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
  "gate": { "served": true, "ranked_by_text_only": false, "floor": 17.5 },
  "confidence": { "agreement": 2, "keyword_score": 24.7, "margin": 2.13 },
  "agent": { "name": "zed", "score": 0.0328, "files": 1, "margin": null,
             "contenders": 1, "totals": [{ "agent": "zed", "score": 0.0328 }] },
  "keyword_top": "zed/knowledge/deploy-checklist.md",
  "indexed": { "entries": 11, "agents": 3, "aliases": 0 },
  "unreachable": { "files": 0, "paths": [], "unindexed": 0, "unindexed_paths": [] },
  "skipped": [],
  "index_was_rebuilt": false,
  "suggestions": [],
  "miss": null,
  "results": [{ "base": "zed", "path": "knowledge/deploy-checklist.md",
                "title": "...", "purpose": "...", "score": 0.032787,
                "keyword_score": 24.7, "why": ["keywords #1", "text #1"],
                "matched": ["rollback"],
                "passages": [{ "heading_path": "...", "text": "...", "excerpt": "...",
                               "provenance": "human", "stage": null }] }]
}
```

Several things in there will bite a caller that assumes otherwise, so they are stated rather than
discovered:

- **`verdict` is `hit`, `guess` or `nothing`**, measured against a keyword floor of 17.5 that is
  calibrated in the open (see `SCORE_FLOOR` in `memory.rs`). A small base produces `guess` often, and
  `guess` means show it and say it is weak, not hide it.
- **Branch on `gate.served`, never on `results.length`.** A refused answer still carries its
  candidates, because the verdict comes from the keyword scorer and the results come from both, so
  `verdict: "nothing"` with a full array is an ordinary outcome rather than a contradiction.
  `gate.ranked_by_text_only` separates the two refusals a caller has to handle differently: `true`
  means the text scorer ranked files the keyword lines missed, which is a base whose `Search for:`
  terms need a word, and `false` with an empty `results` means nothing ranked anywhere. `gate.floor`
  is what `confidence.keyword_score` was measured against, so the gate can be argued with rather than
  guessed at. This field exists because the first integrator to parse this output had to reconstruct
  the rule from a paragraph of `--help`, and either reading of it loses answers.
- **`skipped` names bases that were left out.** Empty since ADR-0034: the one reason a base used to
  be left out, git not answering for its privacy, no longer exists. The field stays because callers
  read it, and it is where a future reason to leave a base out will be named. A caller reading stdout
  alone should still check it rather than conclude from an empty `results` that nothing covers the
  question.
- **`unreachable` says what the fleet holds and cannot reach, in two counts because they are two
  problems wanting opposite work.** `files` is an authoring problem: a note nobody wrote a
  `Search for:` line for is on disk, it opens, it reads fine, and it scores zero on every question
  ever asked, and the only place that was ever reported is `kb check` E02, which nobody is required
  to run. `unindexed` is a `kb index` that has not run since those files were written, which is the
  window the SessionEnd hook opens every time: it captures, it detaches a promotion, and it never
  indexes. `paths` and `unindexed_paths` carry at most eight each while the counts beside them stay
  exact, so a caller can tell a short list from a small problem.
- **`results[].memory` is `short` or `long`.** `short` is the deposit, `inbox/`: recent, unjudged,
  not yet in the library, served on purpose and labelled on purpose so a model leaning on it does so
  knowing what it holds. A caller that wants only settled knowledge filters on `long`.
- **`miss` is the recall loss, handed back rather than only logged.** `null` when the answer was
  served. On a refusal it carries the question, the words the base does know, the date, the path the
  log was written to, and `recorded`, which is `false` when the write failed and `error` says why.
  The log lives beside the fleet by design, so a deployment with a read only filesystem cannot keep
  one: persist this object where your own stack already writes, or set `KB_MISSES_PATH` to a file in
  the one writable directory a function runtime gives you. Without it, the recall loss log on the
  surface with the most real questions in it stays empty, which is what the first deployment measured.
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

### The floor, term by term

The verdict is a comparison: the top file's keyword score against a floor. Both sides are made of the
same pieces, so here they are, named, because a threshold nobody can read is a threshold nobody can
argue with.

| Term | What it is |
|---|---|
| **N** | How many entries the fleet has. An entry is a note with a `Search for:` line |
| **df** | Document frequency: how many entries carry a given key |
| **idf** | `ln(1 + N / (1 + df))`. The weight of a key. A key on one note in a thousand weighs 6.2; a key on every note weighs almost nothing. Standard arithmetic for "how much does this word say about which file" |
| **idf_unique(N)** | idf for a key exactly one note carries: `ln(1 + N/2)`. The heaviest a single key can be at this size. 0.41 at one entry, 1.10 at four, 1.87 at eleven, 4.74 at 226 |
| **W_KEYWORD** | 6.0. One matched key is worth `6 × idf` |
| **keyword score** | The sum over every key the question matched, plus smaller terms for title and whole phrase matches. Printed beside each file by `kb route` |
| **SCORE_FLOOR** | 17.5, measured on a fleet of **226** entries and moved twice by `kb eval` |
| **floor_for(N)** | `0.616 × 6 × idf_unique(N)`, where 0.616 is `17.5 / (6 × idf_unique(226))`. The same floor, restated as "62% of what one unique key scores here", and carried to every other size in that unit. It is 17.5 at 226 entries to the last decimal, 6.9 at eleven, 4.1 at four, 26.4 at a thousand |

**Why it scales.** idf grows with N, because rarity needs a corpus to be rare in. A fixed 17.5 therefore
meant three unique keys on a base of four entries, one on the base it was measured on, and half a key
on a thousand: a word in fifty files cleared it alone. Measured on the demo before the change, a base
of two to four entries got not one `hit` on its own gold questions. After: every size from two up
routes every gold question to the right file, the three refusals hold at every size, and the abstention
benchmark's refusal figure did not move while its in-scope confident answers doubled. The whole record,
with the tables, is [ADR-0036](../../decisions/0036-the-floor-scales-with-the-corpus.md).

**A fleet of one entry never routes.** With one note every key has df 1, so every key weighs the same
and idf can tell nothing apart. Below `MIN_ENTRIES_TO_ROUTE`, two, the verdict is at most `guess`. Two
is the first size at which a word in both notes weighs less than a word in one, which is the first size
at which there is a ruler.

`gate.floor` in `kb route --json`, and every surface that says "against a floor of", carry the floor
that applied to that fleet, never the calibration constant.

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

### `kb panel`: an objection round any agent can run

```
kb panel <piece> --owner goldoni                                  # who could review it, and what each costs
kb panel <piece> --owner goldoni --reviewer apelles --reviewer zed # open the round
kb panel <piece> --from zed --objection "minute six states a latency the base does not produce" --blocking
kb panel <piece> --from apelles --nothing
kb panel <piece> --resolve 1 --escalated --why "to the person, it is blocking"
kb panel <piece> --ledger
```

A review with more than one reviewer has one failure mode that matters and it is not
disagreement: it is **nobody owning the piece.** So this implements one shape and refuses the
other. One agent is accountable, the panel returns named objections rather than rewrites, and
the owner accounts for every one of them in writing.

**Why not everyone arguing to convergence.** Every reviewer has to be booted with its own
constitution before it can judge anything, and `kb blocks` says what that costs. One owner pays
it once per revision cycle. A round table pays it again on every exchange, and convergence has
no bounded number of exchanges, so the cost is unbounded in exactly the case where the
disagreement is real. **The protocol that costs the most is the one that runs when the argument
is hardest.**

The number is read off disk rather than written down, because a cost table in prose goes stale:

```
  reviewer       ~boot   constitution
  apelles         6885   .kb/panel/apelles-constitution.txt
  steve          12832   .kb/panel/steve-constitution.txt
  cicero          5576   .kb/panel/cicero-constitution.txt

  boot              25293
  reading            5613   1871 tokens of artifact, read once by each of 3
  ------------------------
  total             30906 tokens, before a word of the review is written
```

**That estimate is the floor and the real number is about six times it.** Measured on this machine on
2026-09-04, that exact panel run through three real subagents on that exact artifact cost 60,010,
67,537 and 78,408 tokens, 205,955 in total, roughly 69,000 per reviewer. The gap is the reviewer's own
harness: its system prompt, its tool schemas and the turns it takes. The command counts the documents
because those are the part it can read off disk; the rest is charged by whatever runs the subagent.

**The reading line is the half a prose estimate leaves out.** Every reviewer reads the piece as
well as its own constitution, so a long artifact reviewed by four agents costs four times its own
length on top of four constitutions.

**It never calls a model and it coordinates nothing.** It assembles each reviewer's constitution
through the same [`blocks::assemble`](src/blocks.rs) that `kb blocks --emit` uses, writes it where
a subagent can be pointed at it, and prints the exact instruction to hand that subagent and the
exact commands that record what comes back. The session drives. There is no scheduler, no shared
state, and no agent talking to another agent unprompted.

**The panel is decided by the router, not by whoever runs the command.** `kb boot` already asks a
model who owns each message, with the roster, every agent's role and every agent's edge in front of
it. That is the same material the panel question needs, so the verdict gained one line: `REVIEWERS`,
up to three names with what each is being asked to check. When it comes back non-empty the boot
briefing tells the owner who has to object, prices the panel from those agents' own `blocks.txt`, and
prints the `kb panel` command that opens the round. Running `kb panel` by hand still works and is how
a round opens for a piece nobody asked a question about.

**The arithmetic was the obvious way to decide this and it was measured out.** `margin` and
`contenders` are computed on every message and look like the signal: a close race across several
bases reads as a question that spans two domains. Against this fleet's own 49 question gold set on
2026-09-04, every question with exactly one correct owner had between 2 and 10 contenders, median 4,
and a margin cut of 1.5 fires on 25% of them, 2.0 on 40%, 3.0 on 78%. No cut separates a two domain
question from a one domain question with a shared vocabulary, which is the second time that shape has
been measured here; see `MIN_MARGIN`. So with no classifier configured there is no panel, and that
is a decision rather than an omission: **a panel this fleet failed to convene costs one review nobody
had, and a panel it convened wrongly costs about 206,000 tokens and three agents' attention.** When
the instrument cannot tell, the cheap error is the one to make.

Four rules are enforced rather than asked for, and each one is a way a review quietly becomes
theatre:

- **The owner cannot sit on the panel.** An objection from the owner is a revision.
- **An agent with no `blocks.txt` cannot be seated.** A subagent handed no constitution answers as
  the base model wearing a name, and the whole value of the round is that an objection comes from
  inside a domain.
- **A blocking objection cannot be refused**, only taken or escalated to the person. Everything
  else is the owner's judgement on purpose; this is the one valve on it, and a valve a rule asks
  nicely for is not a valve. It is bounded to one per reviewer so that blocking stays expensive.
- **A reviewer who never answered is `not-returned`, never `nothing`.** Those are different facts
  and no code path merges them, because collapsing them is how a silent reviewer becomes a fake
  endorsement.

`--ledger` prints the markdown table that travels with the piece, and **exits 1 while the round is
open**, so a release step can ask whether a piece has been through its round without parsing prose.
The log is `kb-rounds.txt` at the fleet root, one row per fact, gitignored for the same reason the
miss log is: it holds work in progress and a second agent's criticism of it.

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

#### `--json`: the proposal, for an agent that cannot write

```
kb remember "the rollback path is written down before the deploy" fleet/zed --json
```

```json
{
  "claim": "the rollback path is written down before the deploy",
  "proposal": "NOOP",
  "reason": "5 of 5 words are already present in one chunk. ...",
  "evidence": [{ "base": "zed", "path": "knowledge/deploy-checklist.md",
                 "heading_path": "...", "excerpt": "...", "containment": 1.0,
                 "shared": ["rollback", "..."], "missing": [] }],
  "notice": "kb measured overlap. Whether the older text is now wrong is judgement ..."
}
```

**This is the write side's one reachable half, and reaching it is the point.** `write`, `promote` and
the inbox all assume a repository on disk with permission to write, which a hosted agent never has:
its filesystem is read only and its instance is gone a second later. `remember` assumes none of that.
It is deterministic, needs no model, and writes nothing, so it can run at the moment the fact appears,
in the conversation, on the machine that has none of the repository.

So the pattern for a hosted consumer is: **ask at the moment, queue the proposal, apply it later.**

1. Call `kb remember <claim> --json` when something in the conversation looks like it should be kept.
2. Store the object. It is self contained and stable, so it can be queued, versioned and reviewed.
3. On a machine that has the repository, act on it. `ADD` means `kb write <agent> <slug> --keys ...`
   with the claim as the body. `UPDATE` means edit `evidence[0].base/evidence[0].path`, which is the
   passage the overlap was measured against. `NOOP` means drop it: the base already says this.

`notice` carries the same caveat the terminal prints, so a model reading this through another surface
is told what a person is told: overlap is measured, judgement is not, and DELETE is never proposed.

Errors take the same shape `route` uses, `{"claim": ..., "error": ...}` on stdout with exit 1, because
a program calling one command and a program calling the other should not have to learn two ways to
fail.

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

`Memory::open` reads each base's private layer off the base itself: a `private =` line in
`agent.txt`, or `profile/`, `projects/` and `records/` when there is none. Those folders are served
only with `--all`. Nothing outside the directory is consulted, so there is no such thing as a base
whose privacy is unknown, and a folder with a note in it is served the moment it exists. ADR-0034.

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

Five tools, all read only:

| Tool | Returns |
|---|---|
| `kb_route` | Ranked file paths with the words that matched. Cheap, no file contents |
| `kb_retrieve` | The passages themselves, with heading path, provenance and the short memory label |
| `kb_remember` | The ADD / UPDATE / NOOP proposal and its evidence. **Writes nothing** |
| `kb_fleet` | The roster: the fleet's name and role, and every agent with theirs. Read from the manifests, never from the index |
| `kb_list` | The files themselves, narrowed by facet. **Not a search and it takes no question**: no score, no floor, no verdict, because nothing was ranked |

There is no write tool yet, deliberately. A write reached by a model is a different security surface
and gets built on purpose rather than while the retrieval side is still warm.

**The private layer stays out unless asked.** `profile/`, `projects/` and `records/` are the
declared private layer of every base that declares nothing else, and what a tool returns travels to
whatever model is reasoning, so the default leaves them out. `--all` includes them and is a
deliberate act visible in the client's config file. Nothing is asked of git, and a folder with notes
in it is served as it stands (ADR-0034). The deposit, `inbox/`, is served in both scopes and every
passage from it says `[short memory: ...]` in the tool's text.

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

**1. A bundle needs no `.git`, and `--all` means what it says.** *Run, on 2026-09-01: `examples/demo`
copied outside any repository, indexed and routed without `--all`, three agents served.*

This used to be the first thing a deployment hit: `kb` asked `git ls-files` what it may serve, a
bundle has no `.git`, and every base was left out with an empty result set. The first integration
passed `--all` to get past it, which is the wrong reason to pass it. ADR-0034 removed the question.
The private layer is a declaration in each base, `profile/`, `projects/` and `records/` unless
`agent.txt` says otherwise, and it is read off the files you shipped. So: ship the base, ship the
`.kb/` index you built, and pass `--all` only if the consumer is meant to see the private layer. A
hosted agent for other people almost never is.

**2. The filesystem is usually read only, and that is survivable, but the recall loss log is not
free.** *Run, on 2026-08-30, against a log path that could not be written. Not run against a read-only
mount.*

The index is built before the deploy, by `kb index`, and only read at runtime. The one file written on
a query is `kb-misses.txt`, the recall loss log, and a failed write there prints on stderr, returns
the reason in `miss.error`, and does not fail the query. Writes to the base itself, `kb write` and
`kb promote`, do not belong on that machine at all.

**Losing that log is worse than it sounds, so do one of these two things.** It is the only record of
what the base failed to answer, and the first deployment of this ran for a window with six real
questions and kept none of them, while the log in the repository held two lines written earlier on a
laptop. Either read `miss` out of `kb route --json` and store it where your stack already writes, or
export `KB_MISSES_PATH=/tmp/kb-misses.txt` and accept that it lives as long as the instance does. The
first survives the instance; the second costs one line of configuration.

**3. Every process start pays the cold open, and what that costs depends on the operating system far
more than on the base.** *Two of the three numbers below were run here. The one that matters most for
a deployment was not, and is marked.*

| what | where | number |
|---|---|---|
| open the fleet and answer the first question, in process | Windows laptop, `examples/demo`, 2026-09-02 | 11.8 ms, then warm p50 1.16 ms over 1000 samples. The same run on the 2026-08-23 code measured 184.6 ms cold and 0.84 ms warm; ADR-0034 taking git out of the runtime is the cold difference |
| spawn `kb route --json`, open, answer | the same laptop, release build, 40 samples after 3 warm-ups, 2026-08-30 | p50 **184.8 ms**, min 145.8, p90 252.2 |
| the same spawn and answer | Linux, WSL2 on x86-64, a 9 entry base, 40 executions, 2026-08-29 | p50 **9.6 ms**, of which about 6 ms is process creation |

**The last row is somebody else's measurement and we have not reproduced it.** It comes from the first
integration of this into a serverless function, and it is in the table because it is the only figure
here taken on the operating system a deployment actually runs.

The gap between the second row and the third is about twentyfold, and it is the reason this section
used to give bad advice: it quoted the 136 ms as what "spawning per request pays every time", which is
a Windows number generalised to Linux, where the whole spawn and answer costs less than the rounding.
**So: on Linux, do not engineer around the spawn.** Four lines of `execFile` per request, and the
process costs less than one round trip to anything. On Windows, and in a loop on any platform, measure
before you decide: `kb serve` speaks MCP over stdio and pays the open once per process, which does not
fit a stateless HTTP handler and does fit a long-lived worker.

**4. Match the libc, or link none.** *Run, in CI, on 2026-08-28.*

A Linux binary built against a modern glibc will not start on the older glibc that serverless images
carry, because glibc symbol references are versioned. `.github/workflows/release.yml` builds
`x86_64-unknown-linux-musl` instead. `file` reports the result as `static-pie linked, stripped`, and
the same artifact then runs `kb check --all examples/demo` inside `amazonlinux:2023` and inside
`alpine:3`, clean in both, before anything is published.

**That check is the whole point of the file and it runs on every release**, because the failure it
catches is invisible until the deploy: a glibc build goes green in CI, uploads, and dies on the host
with a missing symbol version. A portability claim nothing executes is a rumour.

### What survives a lost machine

**`kb` does not copy your files anywhere, and it is not going to.** Sync is conflict resolution,
causality, clock skew and delete propagation, none of which is a knowledge base problem, and building
it here would produce a worse Syncthing. Use rsync, restic, robocopy, Syncthing, or whatever you
already trust.

What no copier can answer for you is the other two thirds of the question, and those are the parts
this tool knows: **which files must survive, and whether the copy you made is still a base.** Both
are decided in [ADR-0037](../../decisions/0037-what-survives-a-lost-machine.md).

**The set, stated so a script can follow it.** Take the fleet root and everything under it, then
subtract exactly three patterns:

| Subtract | Why it is not in the set |
|---|---|
| `.kb/`, one per base, at any depth | The derived index. Delete it and you have lost a rebuild, not a fact. `kb index` regenerates it from the markdown beside it |
| `.kb-promote.lock`, at the fleet root | The running-now marker. It survives a crash on purpose so the next run can see that one died, which is a fact about that machine and a lie on any other |
| `kb-misses.txt.lock`, beside the log | The same thing one file over: the marker held while the recall loss log merges |

**Everything else is in**, and these five are the ones people leave out:

- **`profile/`, `projects/`, `records/`**, plus anything a base names in its own `private =` line,
  plus the whole of `person/`. The private layer. It is gitignored by design, and **nothing anywhere
  else holds a copy of it.**
- **`kb-misses.txt`**, or wherever `KB_MISSES_PATH` points. Every line is a real question the base
  could not answer, counted. No rebuild produces it and no reindex recovers it.
- **`kb-rejections.txt`**. The same species: what the promoter refused, and why. A repeated refusal
  is a gap in the base, and nothing else records it.
- **`kb-misroutes.txt` and `kb-abstentions.txt`**, the two routing logs. One holds a message an
  agent said was not its own, the other a subject the router found no owner for at all. Both were
  missing from this list, which is the failure the list is about: a file nobody thinks of as data
  is exactly the file a restore drops.
- **`kb-aliases.txt`** at each base root. Small enough to skip for being small. Every line in it was
  paid for by a real question that missed.

**So do not use a git push as your backup.** The ignore files this project ships are a publication
rule, and a correct one: the private layer is gitignored because it is nobody's to publish. Read
backwards it is not a backup rule, it is the exact complement of one. A push carries the notes and
leaves behind the four things above.

**Then verify it, because a backup nobody restored is not a backup.** A checksum proves the bytes
arrived. It does not prove the copy is a base, and that is a stronger claim you can check in two
commands:

```
kb check --all  /path/to/restored/copy
kb eval  /path/to/restored/copy/gold.tsv /path/to/restored/copy --all
```

`kb check` opens the copy as a fleet and resolves every `[[link]]` against what is actually there, so
a file that did not arrive is an E01 and a file truncated past its header is an E02. `kb eval` then
asks the gold set's real questions of the copy and grades where they land, which exercises the index
build, the scorer, the fold and the confidence gate against known answers. **`--all` on both lines is
load bearing**: without it the private layer is never opened, and the private layer is the part git
did not have.

*Run, on 2026-09-02: `examples/demo` copied to a scratch path outside any repository with every
`.kb/` left behind, 18 files. `kb check --all` clean on all three bases, private layer included.
`kb eval` graded FILE 10/10, AGENT 10/10, and held all three refusals.*

**What the exclusion is worth, measured on this fleet the same day.** Dropping `.kb/` takes the copy
from 35.3 MB to 24.0 MB, which is 32 percent and not the order of magnitude it looks like: the index
is 4.3 times the markdown it derives from, and most of a fleet by weight is inbox payload rather than
notes. What the exclusion really buys is that you never copy an 11 MB binary file that changes
whenever a note does. Restoring it costs one command: 290 files and 2,720 chunks reindexed in **1.97
seconds**.

**Three destinations, and the trade is yours to make.**

1. **Another disk on the same desk.** No third party. Survives the failure that actually happens,
   which is one disk dying. Does not survive theft, fire, or anything that walks a mounted volume.
2. **Another machine you own, over the LAN.** No third party, and it survives a dead machine, but
   only if the two sit in different places. Two boxes on one desk share a building, a power supply
   and a burglar.
3. **A cloud bucket, encrypted before it leaves.** The only one that survives the building, and the
   only one where **the encryption is not a setting.** `profile/`, `projects/` and `records/` are in
   the set by the rule above. Uploading them unencrypted puts a person's profile, projects and
   records in plaintext on somebody else's server, which is the custody position this whole tool
   exists to refuse. Encrypted, the provider holds bytes it cannot read, and that is a different
   relationship.

Only the third survives the building. Only the third involves a third party. **There is no
destination that does both**, and which failure you should insure against depends on what your
`profile/` holds, which this tool does not know. So this section names the trade and stops there.

**`kb backup --list` and `kb backup --verify` are the shape ADR-0037 chose for the two answerable
halves**, printing the paths one per line for piping into any of the tools above, and opening a copy
to run the check. **Neither is built yet.** Until they are, the rule is the table above and the
verification is the two commands above, which is what the verbs will run.

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

`kb misses` is how you find the ones worth a line. It reads `kb-misses.txt` back, most asked first,
and beside each question names the files today's index nearly caught it with and the keys each of
those files declares. A file the text scorer reached at keyword score `0.00` is the whole finding:
the words are in the note and not on its `Search for:` line, so the fix is a line here or another
key rather than another note. It proposes and never applies, and there is deliberately no flag that
does: a machine appending to this file on evidence it produced itself closes that loop with nobody
in it.

- `--strict` counts warnings toward the exit code, which is what a commit hook wants.
- `--all` includes files git does not track. By default only tracked files are checked, because the
  private layer is gitignored by design, it is nobody's to publish, and linting it buries the findings
  that matter under noise from files we would never edit.

Exit code is 1 when there are errors, or when `--strict` and there are warnings. Everything else is 0.

## Checks

| Code | Level | What it catches |
|------|-------|-----------------|
| E01  | error | A `[[link]]` with no file behind it |
| E02  | error | A file with no `Search for:` line, so the router builds no entry for it. A file nobody can find does not exist |
| E04  | error | `provenance` or `stage` carries a value outside the legal set |
| W01  | warn  | A `[[link]]` matching more than one file, so it is ambiguous |
| W02  | warn  | A map entry with no `Search for:` line, where a map exists |
| W03  | warn  | An em dash or en dash, which house style forbids |
| W04  | warn  | A note declaring a source with no `evidence_tier` or `valid_for` |
| W05  | warn  | A note with no `provenance` or no `stage`, so who wrote it is unknown |
| W06  | warn  | A `Search for:` line too short to be found by a real question |
| W07  | warn  | A key that reaches neither the keyword nor the phrase index, because it collapses to nothing after stopwords |
| W08  | warn  | A `.gitignore` is here and misses a folder the base declares private |

E03, no map file, is gone with the reason it existed: the index walks files since ADR-0028 and a map
is a reading list for people, so a base without one indexes perfectly well.

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

**There are prebuilt binaries, and this file used not to say so.** Tagged releases carry
`kb-linux-x64`, statically linked against musl so it depends on nothing outside the file, and
`kb-windows-x64.exe`, each with a `.sha256` beside it. Both are built and checked by
`.github/workflows/release.yml`. The first integrator to deploy this developed on Windows, wrapped the
build gate in a script that skips silently off Linux, and did their measuring inside WSL, because
nothing they read told them a Windows binary was published. A binary nobody can find is a binary
nobody has.

**There is no macOS build.** Not an oversight being hidden: nobody here runs macOS, so a published
artifact would be one nothing has ever executed, which is worth less than its absence. Build from
source there, or open an issue and it gets added to the matrix.

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
