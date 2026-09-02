---
provenance: agent
stage: derived
---

# ADR-0036: the floor scales with the corpus, and a corpus of one has no floor

**Search for:** `piso`, `floor`, `SCORE_FLOOR`, `floor_for`, `idf`, `idf_unique`, `inverse document frequency`, `frequencia inversa de documento`, `raridade`, `rarity`, `escala`, `scale`, `tamanho do corpus`, `corpus size`, `N entradas`, `entries`, `df`, `document frequency`, `W_KEYWORD`, `peso da chave`, `chave unica`, `unique key`, `calibracao`, `calibration`, `FLOOR_CALIBRATED_AT`, `MIN_ENTRIES_TO_ROUTE`, `minimo de entradas`, `base pequena`, `small base`, `base grande`, `big base`, `mil arquivos`, `thousand files`, `varredura`, `sweep`, `kb eval`, `kb-bench abstain`, `abstencao`, `abstention`, `demoted`, `rebaixado`, `guess`, `chute`, `hit`, `ADR-0036`

**Exists to:** Why the confidence floor stopped being one number, what each term in the formula means, the three instruments the change was measured on before it was accepted, and why a fleet of one entry never routes.

- **Date:** 2026-09-02
- **Status:** accepted, implemented and measured the same day
- **Scope:** system. Every surface that gates on the floor.
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. The constant `SCORE_FLOOR` still exists and still equals what it
  did; the scaling is one function and one comparison, and removing them restores the fixed
  floor exactly.

## The terms, before the formula

Richard's question, verbatim: *K significa o quê? Knowledge base?* No. It was the letter
I picked for a constant in a chat message, and a name that has to be asked about is a bad
name. Every symbol below is spelled out, and the code uses the spelled out names.

| Term | Meaning | Where it lives |
|---|---|---|
| **entry** | One note the router can find: a markdown file with a `Search for:` line. A fleet's size, for everything below, is its number of entries | `Memory::entry_count` |
| **N** | The number of entries in the fleet being asked | same |
| **key** | One term on a note's `Search for:` line | `index::Entry::keywords` |
| **df**, document frequency | How many entries carry a given key. A key on one note has df 1 | `index::route`, computed per question |
| **idf**, inverse document frequency | The weight of a key: `ln(1 + N / (1 + df))`. Rare keys weigh more; a key on every note weighs almost nothing. This is the standard arithmetic for "how much does this word tell me about which file" | `index::idf` |
| **idf_unique(N)** | idf with df 1: the weight of a key that exactly one note carries, at this fleet size. The heaviest a single key can be. `ln(1 + N/2)` | `index::idf_unique` |
| **W_KEYWORD** | What one matched key is worth before its idf: 6.0. A question matching a key scores `W_KEYWORD × idf(key)` for it | `index::W_KEYWORD` |
| **keyword score** | The sum of those over every key the question matched, plus smaller terms for title and phrase matches. The number `kb route` prints beside each file | `Confidence::keyword_score` |
| **SCORE_FLOOR** | 17.5. The score a top result had to reach to be called a `hit`. Measured on Richard's fleet, twice moved by measurement | `memory::SCORE_FLOOR` |
| **FLOOR_CALIBRATED_AT** | 226. The number of entries that fleet had when 17.5 was the right floor for it. The two constants are one fact | `memory::FLOOR_CALIBRATED_AT` |
| **floor_in_unique_keys** | `SCORE_FLOOR / (W_KEYWORD × idf_unique(226))` = 17.5 / (6 × 4.74) = **0.616**. The floor read as a fraction of what one unique key scores on the calibration fleet: "a result has to score at least 62% of a single word found in exactly one note" | `memory::floor_in_unique_keys` |
| **floor_for(N)** | `floor_in_unique_keys × W_KEYWORD × idf_unique(N)`. The same 62% of a unique key, re-expressed in the idf of a fleet of N entries. Equals 17.5 at N = 226, by construction | `memory::floor_for` |
| **MIN_ENTRIES_TO_ROUTE** | 2. Below it the verdict is never `hit` | `memory::MIN_ENTRIES_TO_ROUTE` |

So the floor is not a new number. It is the old number, stated in the unit it was
secretly always in, and carried to other fleet sizes in that unit.

## The problem, with the arithmetic that shows it

A matched key is worth `6 × idf`, and idf grows with N because rarity needs a corpus to be
rare in. So a fixed floor means a different number of keys at every size:

| N | one unique key scores | a key in 5% of entries scores | 17.5 means |
|---|---|---|---|
| 4 | 6.6 | 6.4 | three unique keys |
| 11 | 11.2 | 8.4 | two unique keys |
| 226 (calibration) | 28.4 | 10.7 | one unique key |
| 1000 | 37.3 | **18.2** | half a key: **a word in fifty files clears it alone** |

The fixed floor got harder as the base shrank and easier as it grew, in exactly the two
directions that hurt. Below, the small side measured; the large side is arithmetic, because
no fleet of a thousand entries exists here yet.

**Measured on the demo corpus with the fixed floor, before any change:**

```
 N   floor  own q  hit  guess  nothing  wrong file   abstain: refused hedged answered
 1    17.5      0    0      0        0           0                  3      0        0
 2    17.5      1    0      1        0           0                  3      0        0
 3    17.5      2    0      2        0           0                  3      0        0
 4    17.5      2    0      2        0           0                  3      0        0
 5    17.5      4    4      0        0           0                  3      0        0
 ...
11    17.5     10    8      2        0           0                  3      0        0
```

`own q` is the gold questions whose file survives at that size. From two to four entries
**not one of them reached `hit`**, every one a `guess`, on the right file. A fleet that small
could not route at all. At the full demo, two correct answers were still demoted.

## Options

### A. Keep the fixed floor

Cost: the table above. Every fleet smaller than five entries is unusable for routing, and
every fleet much larger than the calibration one confidently answers on common words. The
integrator's base of nine entries sat in the zone where a paraphrase scored 1.7 and was a
guess. Rejected.

### B. Scale the floor, calibrated neutral

Express the floor as a fraction of one unique key, calibrate that fraction so the
calibration fleet's floor is 17.5 to the last decimal, and apply the fraction everywhere.
Cost: the constant that `kb eval` has moved twice by measurement gains a second constant,
the size it was measured at, and the two have to be re-derived together. Gain: the meaning
of the floor in keys stops depending on N, which the first test pins at four sizes.

### C. Scale the floor and refuse to route below a minimum size

B, plus: with one entry every key has df 1, so every key weighs the same and idf can tell
nothing apart. Any shared word clears a floor built from that. Below the minimum the
verdict is at most `guess`, served with its warning, and `boot` hands over no identity.

**Chosen: C.** B alone hands the degenerate case a `hit` for the wrong reason.

## The minimum, and why it is two and not five

Two is the structural minimum: the first size at which a word in both notes weighs less
than a word in one, which is the first size at which idf is a ruler. The sweep after the
change shows every gold question hitting the right file from two entries up, no wrong file
at any size, every refusal holding at every size. A larger minimum would refuse bases that
are measured to route correctly, and there is no measurement supporting one. Richard's
instinct was a larger number; the data does not give it, and the constant carries a revisit
trigger instead of a guess.

## The measurements, before and after

Three instruments, same binary either side of the change, 2026-09-02.

**Richard's fleet, 226 entries, 33 questions.** Identical line for line: FILE 11/24,
routes 20/24, keyword 21/24, refused 3, hedged 0, answered 6 of 9, hit scores 33.15 to
145.95, miss scores 7.98 to 93.60. Neutral by construction, and checked rather than assumed.

**The demo, 11 entries, 13 questions.**

| | fixed floor 17.5 | scaled floor 6.9 |
|---|---|---|
| FILE keyword | 10/10 | 10/10 |
| AGENT routes, classifier included | 8/10 | **10/10** |
| correct answers demoted to guess | 2/10 | **0/10** |
| refused of 3 | 3 | 3 |

**The abstention benchmark, 50 blind questions over the demo.**

| | fixed floor | scaled floor |
|---|---|---|
| out-of-scope not answered confidently | 28/30 | 28/30 |
| confident wrong answers | 2, at 33.0 and 24.8 | the same 2 |
| in-scope confident / guess / nothing | 6 / 8 / 6 | **12 / 2 / 6** |

The refusal claim, which is the product's differentiating one, did not move. The two
baits that got through score above either floor and are the classifier's job, as the
results file already says. The in-scope side doubled its confident answers, which is the
guess column that "unturned phrasing costs a small base" being paid for by the floor
rather than by the phrasing.

**The sweep, after.** Every size from two up: every own question a `hit`, zero wrong file,
three of three refusals. At one entry there is no own question in the gold and the rule
says `guess` regardless.

## What changed

- `index::W_KEYWORD` is public and `index::idf_unique` exists, so the floor can be stated
  in the scorer's own units.
- `memory::FLOOR_CALIBRATED_AT`, `floor_in_unique_keys`, `floor_for`, `MIN_ENTRIES_TO_ROUTE`,
  `Memory::floor`, `Memory::enough_to_route`. `SCORE_FLOOR` stays and is the anchor.
- `Confidence` carries `floor`: the threshold that applied to this verdict, for this
  corpus. Every surface that prints "against a floor of" reads it from there: `route --json`
  (`gate.floor`), `kb answer`'s prompt, `kb boot`'s notice, the MCP evidence line, the
  classifier's dossier, the promoter's dossier, the reading room, and `kb eval`.
- `kb-bench abstain` gates on the same rule and prints both the corpus's floor and the
  calibration it derives from.
- Tests: the floor equals the measured number at the calibration size and the meaning in
  keys is invariant across four sizes; a word in 5% of a thousand entries is not a hit on
  its own; one entry never routes and two can; the verdict carries its floor. The one
  fixture that relied on one unique key being a guess was refit to a key every note
  carries, which is the shape that is a guess at four entries.
- 284 tests, from 280.

## Revisit trigger

- The first fleet that misroutes at two to four entries. That is the number `MIN_ENTRIES_TO_ROUTE`
  is waiting for.
- The first fleet near a thousand entries, where the large side of the table above stops
  being arithmetic. `kb eval` on it is the instrument, and `gate.floor` in every JSON reply
  says what applied.
- `SCORE_FLOOR` moving again by measurement. When it does, `FLOOR_CALIBRATED_AT` moves with
  it or the re-derivation did not happen.
