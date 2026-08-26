# 0033: The text scorer prunes what cannot rank

Date: 2026-08-26. Status: accepted. Owner: Zed.

## The problem, measured before theorized

`Memory::ask` over a 57,486-chunk base (the LongMemEval-V2 enterprise haystack,
221 MB of accessibility trees) costs 150 to 750ms per question, against
sub-millisecond over a 113-entry personal fleet. Profiled with the query
replayed at the SQL level and with instrumented binaries over identical
question sets, the cost decomposes:

- The FTS5 match walk is the cost. Ranking bare rowids with bm25 already costs
  205ms at the median. The expression is an OR of every normalised question
  word, and a word like "list" alone matches 19,530 of the 57,486 chunks, so
  the scorer walks and scores tens of thousands of candidates per question.
- The wide columns are not the cost. `snippet()` and the full chunk text
  looked like the classic FTS5 trap, and the classic fix (rank rowids first,
  fetch the winners) was built and refuted: two phases with MATCH in the
  second cost 524ms against 246ms one-phase on identical questions, because
  the second MATCH walks the doclists again. The refutation lives as a comment
  on the query so nobody rebuilds it.
- The payload is not the cost either: a full retrieve answer is ~37 KB.

## The options

**Keep the full OR expression.** Every term contributes its bm25 share, and a
term in a quarter of the corpus contributes almost nothing: BM25's idf,
log((N - df + 0.5) / (df + 0.5)), sits near FTS5's floor there. Cost: the
longest doclists in the index are walked to move nothing.

**Prune terms with df above N/4 from the text expression, on big bases only.**
The keyword scorer still sees every word; only the FTS expression shrinks.
Costs a document-frequency probe per new term, cached per store and cleared on
sync, because df changes only when the index does.

## The decision, with the trade stated

Prune, above `PRUNE_MIN_CHUNKS` (10,000). Measured on identical 19-question
sets against the unpruned binary: median 371ms against 306ms (the probes cost
more than they save there), p95 1264ms against 1772ms. We trade 65ms nobody
perceives at the median for half a second off the tail that reads as a hang.
Below the threshold nothing changes at all: the demo eval is bit-identical
(file 10/10, 3/3 refusals, ~1.2ms per question) and every personal base
measured so far sits far under 10,000 chunks. When every term would be pruned,
the original expression stands: slow beats none.

Evidence recall on the V2 haystacks after the change: unchanged (checked
against the same instrument that produced the pre-change numbers; see
`benchmarks/longmemeval-v2/README.md`).

## What this is not

Not benchmark tuning: no gold answer was read, the rule is an idf argument
that holds for any corpus, and the benchmark corpus only supplied the scale at
which it starts to matter. The residual 300ms median at 57k chunks is the
match walk itself; a real fix there is query planning work (term-at-a-time
scoring, or an FTS content strategy that shrinks doclists) and is deliberately
not attempted here.
