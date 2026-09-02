---
title: The confidence floor now scales with your corpus
date: 2026-09-02
description: Why one threshold cannot serve a base of ten notes and a base of a thousand, every term in the formula spelled out, and the three measurements the change had to pass before it shipped.
lang: en
---

Vesta refuses to answer when nothing in your library matches well enough. That refusal is
a comparison: the best file's keyword score against a floor. Until today the floor was one
number, 17.5, measured on one fleet. This post is about why one number cannot be right for
every size of library, what replaced it, and what each piece of the arithmetic means, so you
can argue with the threshold instead of trusting it.

## The pieces, named

| Term | What it is |
|---|---|
| **entry** | A note the router can find: a markdown file with a `Search for:` line |
| **N** | How many entries your fleet has |
| **key** | One term on a note's `Search for:` line |
| **df** | Document frequency: how many entries carry a given key |
| **idf** | Inverse document frequency, `ln(1 + N / (1 + df))`. The weight of a key. A key found in one note out of a thousand weighs 6.2; a key on every note weighs almost nothing. This is the standard arithmetic for "how much does this word tell me about which file to open" |
| **idf_unique(N)** | idf for a key that exactly one note carries: `ln(1 + N/2)`. The heaviest a single key can be at that size. 0.41 with one entry, 1.10 with four, 1.87 with eleven, 4.74 with 226 |
| **keyword score** | Each key the question matches is worth `6 × idf`, and the score is the sum. It is the number `kb route` prints beside every file |
| **floor** | What the top score has to reach for the verdict to be `hit`. Below it the router still answers, labelled a `guess`; at zero it says `nothing` |

## Why one number was wrong twice

idf grows with N, because rarity needs a corpus to be rare in. So a fixed floor means a
different number of keys at every size:

| N | one unique key scores | a key in 5% of entries scores | what 17.5 meant |
|---|---|---|---|
| 4 | 6.6 | 6.4 | three unique keys |
| 11 | 11.2 | 8.4 | two |
| 226 | 28.4 | 10.7 | one |
| 1000 | 37.3 | 18.2 | half a key: a word in fifty files cleared it alone |

The floor got harder as the base shrank and easier as it grew, in the two directions that
hurt. Measured on the demo fleet that ships in the repository, before any change: with two,
three or four entries, not one of its own gold questions reached `hit`. Every one was a
`guess`, on the right file. A library that small could not route at all.

## What replaced it

The floor is not a new number. It is the old number, stated in the unit it was always
secretly in. 17.5 on a fleet of 226 entries is `17.5 / (6 × 4.74)` = 0.616 of what one
unique key scores there. So:

```
floor(N) = 0.616 × 6 × idf_unique(N)
```

Read it as "a result has to score at least 62% of what a single word found in exactly one
note would score in this library". At 226 entries that is 17.5 to the last decimal, by
construction. At eleven it is 6.9. At four it is 4.1. At a thousand it is 26.4, which puts
the word in fifty files back under it.

One more rule, because scaling fixes the ruler and does not conjure one. **A fleet of one
entry never routes.** With one note every key has df 1, so every key weighs the same and idf
can tell nothing apart; any shared word would clear a floor built from that. Below two
entries the verdict is at most `guess`. Two is the first size at which a word in both notes
weighs less than a word in one, which is the first size at which there is a ruler.

## What it had to pass

Three instruments, same binary either side of the change, run on 2 September 2026.

**The fleet the floor was measured on, 226 entries, 33 questions.** Identical line for line.
Neutral where it was calibrated, and checked rather than assumed.

**The demo fleet, 11 entries, 13 questions.** Routing went from 8 of 10 to 10 of 10. The two
correct answers the old floor demoted to guesses are hits. All three refusals still refuse.

**The blind abstention benchmark, 50 questions over the same demo.** The number that matters
did not move: 28 of 30 out-of-scope questions were still not answered confidently, and the
two that got through are the same two medical baits, scoring above either floor. On the
in-scope side, confident answers went from 6 to 12, out of the guess column.

**A sweep from one entry to eleven.** From two up: every gold question a `hit` on the right
file, no wrong file at any size, every refusal holding.

Every JSON reply carries the floor that actually applied to your fleet in `gate.floor`, and
every surface that says "against a floor of" reads the same number. The full record, with
every table and the revisit triggers, is
[ADR-0036](https://github.com/richard-wollyce/ulpia/blob/main/decisions/0036-the-floor-scales-with-the-corpus.md).
