---
provenance: agent
stage: derived
---

# ADR-0041: the keys live in one place, and every door to a model filters

**Search for:** `keys`, `chaves`, `linha de palavras chave`, `keyword line`, `linha search for`, `entrada do mapa`, `map entry`, `MAP.md`, `mapa`, `catalogo`, `catalogue`, `lista de leitura`, `reading list`, `duas copias`, `two copies`, `copia duplicada`, `duplicate copy`, `deriva`, `drift`, `W02`, `E02`, `kb write`, `kb init`, `kb check`, `render_entry`, `map_md`, `PERSON_MAP`, `agent-skeleton`, `person-skeleton`, `esqueleto`, `skeleton`, `criar base`, `create a base`, `padrao para novas bases`, `standard for new bases`, `strip_keyword_lines`, `blocks::assemble`, `map_prompt`, `kb answer --complete`, `modo completo`, `complete mode`, `filtro no prompt`, `filter at the prompt`, `tokens residentes`, `resident tokens`, `is_orientation`, `is_exempt`, `header_of`, `mapa se indexa`, `map indexes itself`, `catalogo como resposta`, `catalogue as an answer`, `person/MAP.md`, `gate::propose`, `review_prompt`, `excecao declarada`, `stated exception`, `ADR-0016`, `ADR-0028`

**Exists to:** record that a note's keys live in the note and nowhere else, that every path
by which base text reaches a model filters those lines at the door, and that an orientation
file is never indexed whatever it happens to carry.

- **Date:** 2026-09-07
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible in code, one-way in content only if somebody deletes the
  lines the fifteen existing maps still carry. Nothing here deletes them. `render_entry` can
  emit the line again in one commit, and W02 can come back with it.

## Context

Commit `e5df2b5` filtered `Search for:` lines out of the resident constitution at assembly.
It cut fleet resident tokens from 127,231 to 87,055, and it deliberately changed nothing on
disk, because the lines are what `kb check` validates and what an agent edits by hand.

Filtering at the door left three things open, and Richard's instruction was to close them and
to standardise the shape so a base created tomorrow is not born in the form the filter has to
clean up: *pode podar onde tem desperdicio e ja padroniza no sistema para as proximas
geracoes/criacoes seguir esse padrao otimizado.*

Three facts, each measured on 2026-09-07 against the fleet's fifteen bases:

1. **`kb init` generates a map that tells the writer something false.** The generated
   `MAP.md` said each entry gets a `Search for:` line *because that line is what the router
   matches against*. It has not been true since ADR-0028 moved the keys into each note's own
   header: `index::header_of` reads the note, `store::sync` skips the map entirely, and no
   scorer has read a map entry in four months. Every base anyone creates inherited the
   sentence.
2. **`kb write` writes the key list twice**, once into the note's header where the router
   reads it and once into the map entry where nothing does.
3. **`person/MAP.md` was in the index as a note.** It has no `##` heading anywhere, so
   `header_of`'s bound never fired, it read the first entry's keyword line, and the map ranked
   alongside the file it points at:

   ```
   kb route "quem e o usuario" . --all --top 5
     1.  28.26  person/MAP.md      the reading list
     2.  28.26  person/core.md     the file it points at
   ```

   `index::is_exempt` already named `map.md` an orientation file. It did not prevent this,
   because `build` consulted it only after `header_of` came back empty: it classified files
   nobody could reach and never stopped a file from being reached.

## Options

### Option A: keep generating the keyword line in map entries, and keep filtering it

- Cost: nothing today. `blocks::assemble` already removes them from every prompt, W02 keeps
  grading them, a person greps `MAP.md` and finds them.
- Failure mode: **two copies of one list with nothing keeping them in sync.** The router reads
  the note; a person greps the map; the first hand edit to a note's keys makes them disagree
  and no tool says so. The token argument was already collected by the filter, so what is left
  is a correctness cost paid forever to keep a copy nobody reads.
- It also keeps the filter load-bearing on disk shape: every future surface that renders a map
  has to remember to call `strip_keyword_lines`.

### Option B: keep the copy and add a check that the two agree

- Cost: a new check, plus a migration to make the 391 existing lines agree with their notes.
- Failure mode: it institutionalises the duplication. Paying maintenance to keep a copy
  synchronised is only worth it when the copy has a reader, and after `e5df2b5` this one has
  none in any prompt.

### Option C: new maps stop carrying keyword lines; existing maps are untouched

- Cost: `W02` has to go with the line it grades, because a linter that warns about every entry
  its own `kb write` produces is a linter people turn off. Someone who greps `MAP.md`
  specifically loses the hit and gets the note instead.
- Failure mode: maps become mixed, old entries with the line and new ones without. Mitigated
  by the generated preamble, which now states the rule, and by the filter, which goes on
  removing the old ones from every prompt.

## Decision

**Option C**, plus the two mechanical fixes it depends on.

### 1. The keys live in the note. The map entry is a reading list line

`kb write` writes `**Search for:**` into the note's header and writes the entry as
`- **[[slug]]** one line saying what the file is.` and nothing more. `kb init`'s generated
`MAP.md` and `PERSON_MAP` say so in the preamble instead of repeating the old claim, and the
published `agent-skeleton/` and `person-skeleton/` are regenerated from those templates by the
drift tests that already guard them.

**Nothing on disk in any existing base changes.** The fifteen maps keep their 391 lines,
`blocks::assemble` keeps filtering them out of every prompt, and no migration is required or
performed.

### 2. W02 is retired, and E02 is the check that survives

W02 warned that a map entry carried no `Search for:` line, *so grep cannot route to it*. That
stopped being true with ADR-0028. Measured before removing it: **0 findings across all fifteen
bases**, because every existing entry has the line. It graded 391 lines and reported nothing,
which is what a check on a redundant copy looks like. E02 asks the reachability question of the
file that decides it, so a note nothing can reach is still an error and is reported once.

ADR-0016's principle is untouched: a note and its map entry still arrive in one act. Only what
the entry holds moved.

### 3. An orientation file is never indexed, asked before the file is parsed

`index::is_orientation` splits out of `is_exempt` the half that means *the router builds no
entry for this file whatever it carries*, and `build` asks it before `header_of` runs. It has
two arms: the conventional names, and the base's own declared catalogue from `Base::map`, which
covers a base whose reading list is `MAPA.md` or `INDEX.md` and which the name list alone did
not know.

**The fix is in the mechanism and not in the file.** Giving `person/MAP.md` a `##` heading
would have closed this instance and left the next map written without one waiting.
`header_of`'s doc claimed the bound excluded a map *by construction*; the construction was a
formatting convention that fourteen of fifteen maps happened to follow. Asking what the file is
before parsing how it is punctuated makes the answer independent of the punctuation.

**The text scorer already worked this way**, which is why this is a correction rather than a
new rule: `store::sync` has always skipped `Base::map`, so no map has ever been chunked and no
map has ever come back as a passage. The two scorers disagreed about one file class and only
the keyword side leaked it.

**What this deliberately does not reach**, stated so the next reader does not think it was
missed: `README.md`, `what-goes-here.md` and `MOVED.md` stay in the full text index, 56 chunks
across the fleet. A map's body is pointers to files that are themselves indexed, so returning
it returns a worse copy of something already reachable. A folder legend's body is content
nothing else carries, and excluding it would lose real answers to buy tidiness.

### 4. Every door by which base text reaches a model filters, with two stated exceptions

The full inventory, walked rather than assumed:

| Path | Carried keyword lines | Now |
|---|---|---|
| `blocks::assemble`, the resident constitution: `kb boot`, `kb blocks --emit`, `kb panel` | yes | filtered since `e5df2b5` |
| Retrieved passages: `kb answer` fast and expanded, `kb serve`'s `kb_retrieve`, `kb route --json`, `promote::evidence_for`, the reading room | **no** | unchanged |
| `kb answer --complete`, which reads whole files off disk | yes | filtered, in `answer::map_prompt` |
| `kb boot`'s briefing, `kb fleet`, `kb list`, `classify`'s dossier | no, they carry paths and `Exists to:` lines | unchanged |
| `promote::proposal_prompt`, which embeds a deposit file | no, a deposit file carries no keys by design | unchanged |

**The second row corrects `e5df2b5`'s own commit message and a doc comment it left behind**,
both of which said a retrieved passage still arrives with its keyword line. It does not.
`store::chunk` drops those lines before a chunk is stored, for a different reason with the same
effect: the keyword scorer and the text scorer must not read the same words, or their agreement
stops being evidence. Counted over the fleet's fifteen indexes, **0 of 4,357 stored chunks
carry one**. The seam named in that commit message did not exist, and it was named without
looking.

**The seam that does exist is complete mode**, the one path that bypasses the chunker. Over the
330 files `complete_plan` selects: 3,641,773 bytes read from disk, of which **310,215 are
keyword lines and their continuations, 8.5%**, about 77,500 tokens per full read across 33
model calls. It is filtered in `answer::map_prompt`, at the point the file becomes a prompt,
which is the seam `blocks::assemble` occupies, and by the same function, so there is one
definition of what a keyword line is and not a fourth.

**Two stated exceptions, where a keyword line is the subject and not overhead:**

- `gate::propose` shows a model the keys each near-miss file declares. Its job is to propose an
  alias from a query term to a term a file already carries, and it cannot do that blind.
- `promote::review_prompt` shows the reviewer the proposed note's own keys, because judging the
  keys is half of what the reviewer is for.

Neither reads a file's prose, so neither passes through the filter, and both would be broken by
a rule with no exception. A rule with a named exception is worth more than a rule that quietly
is not universal.

## Consequences

- **The index loses exactly one entry on this fleet**, 330 to 329, and it is `person/MAP.md`.
  `kb route "quem e o usuario"` now returns `person/core.md` alone, at 30.65 rather than 28.26:
  one fewer document carries `usuario`, so its idf rose. Every other route is unchanged.
- **`kb answer --complete` costs about 8.5% less per full read**, and the saving scales with
  the corpus rather than being a one-off.
- **A map written from here on is not greppable for keys.** That is the accepted cost.
  `grep -rn "Search for:"` over a base still finds every key list, once each, in the notes,
  which is also where the router looks.
- **Maps become mixed** until somebody chooses to trim an old one. Nothing requires it and no
  tool will complain either way.
- **A third test for a keyword line stays where it is, named rather than unified.**
  `store::is_keyword_line` drops any line containing the string, which is wider than
  `index::labelled`, and the two differ on **43 lines out of 632 across the fleet**: mostly
  map preambles that `sync` never chunks, plus a few sentences in real notes that are
  therefore absent from the full text index. Tightening it changes what the text scorer can
  find, which needs the fleet reindexed and `kb eval` rerun on both sides. It is documented at
  the function with the measurement, and left for a change that can be measured on its own.

- **`index::keywords_in` and `checks::map_entries` are deleted**, along with `MapEntry`. Both
  were the pre-ADR-0028 map-driven walk: `keywords_in` had no caller but its own tests, and
  `map_entries` lost its last one when W02 went. `MapEntry.body` carried the doc comment
  *"which is what the index reads for keywords"*, which had been false for four months.

## Revisit trigger

**A person or an agent reaches for a map's `Search for:` line and finds it missing, twice.**
That is the evidence that the grep argument was real and that the entry needs the keys back, at
which point Option B, a check that the two copies agree, becomes the right answer rather than
the expensive one.

The second trigger is a new surface that hands base text to a model without going through
`blocks::assemble`, `retrieve` or `answer::map_prompt`. The inventory above is a snapshot of
the doors, and a fourth door is a reason to re-walk it rather than to assume it is covered.
