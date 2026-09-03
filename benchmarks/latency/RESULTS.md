# Latency: local software against the speed of light

| | |
|---|---|
| Date | 2026-08-23 |
| Commit | 35322d2 |
| Machine | 11th Gen Intel i5-1135G7, 16 GB, Windows 11, release build, home connection in Brazil |
| Command | `kb-bench latency examples/demo benchmarks/abstention/questions.tsv --host mem0=api.mem0.ai --host zep=api.getzep.com --host letta=api.letta.com --host supermemory=api.supermemory.ai --host synap=synap-cloud-prod.maximem.ai`, from the repo root |
| Corpus | `examples/demo`, 15 files |

## Measured here

```
LOCAL    cold start, open plus first question: 136.4 ms
         warm, 1000 samples over 50 questions: p50 0.68 ms, p95 1.16 ms, max 1.70 ms
```

The warm number is the whole deterministic pipeline per question: both scorers,
fusion, the species fold and the confidence gate, in process. No model, no network,
no cache warming tricks; the index is the program. The cold number is everything the
first question ever pays, including opening the fleet.

## Re-run 2026-09-02, both versions side by side

The number above was published on 2026-08-23 and had never been re-measured. It is now
1.7x off, and this section says how much of that is the code and how much is the room.

**Method, because a single latency run on a laptop is worth nothing.** Nine runs of the
current binary while a browser and several agent sessions were open gave p50 between
1.53 ms and 4.17 ms: the same binary, the same corpus, a 2.7x spread from load alone. So
this comparison was taken with the machine quiet, and the two versions were **alternated
within one command**, four rounds each, every measured run preceded by an unmeasured one.
The August side is commit `c0d2662`, the first commit that carries `kb-bench latency`;
the current side is `a87212c`. Both indexes were read back with sqlite and hold the same
11 files and 11 chunks, so the corpus is not the variable.

| | cold open | warm p50 | warm p95 |
|---|---|---|---|
| `c0d2662`, 2026-08-23 code | 136.8, 182.9, 203.4, 186.2 ms | 0.84, 0.87, 0.84, 0.79 ms | 1.66, 1.68, 1.66, 1.57 ms |
| `a87212c`, 2026-09-02 code | 14.2, 10.8, 9.5, 12.7 ms | 1.14, 1.03, 1.17, 1.18 ms | 2.17, 2.00, 2.15, 2.38 ms |
| median, then against then | **184.6 to 11.8 ms, 15.6x faster** | **0.84 to 1.16 ms, 1.38x slower** | **1.66 to 2.16 ms, 1.30x slower** |

**The cold open got 15.6x cheaper, and the cause is named.** Opening a fleet used to shell
out to `git ls-files` once per base to decide what was private. ADR-0034 took git out of the
runtime and replaced it with a `private =` line read off disk. That is the whole 173 ms.

**The warm median got 1.38x more expensive, and the cause is not named here.** The candidates
are the term pruning of ADR-0033, which trades median for tail and whose probes are pure cost
on a corpus this small, and the corpus-scaled floor of ADR-0036, which computes a logarithm
per query. Neither was isolated, and this file will not guess at which: what is measured is
the total.

**How much is the machine.** The August code measures 0.84 ms here today against the 0.68 ms
it published on 2026-08-23. The same code, the same laptop, nine days apart. So of the 1.7x
drift a reader sees between the published figure and the current one, roughly 1.24x is the
room and 1.38x is the code.

**One thing this run found by accident.** The first attempt ran both binaries against a copy
of `examples/demo` outside any repository, and the August binary reported p50 0.01 ms. That
was not speed. It was `git ls-files` returning nothing outside a repository, so the base
indexed zero files and answered instantly with nothing. The measurement is void and the
observation is kept: before ADR-0034, a base outside a git repository silently held no
searchable content.

## What the vendors say about themselves

Their own words, from their own pages, each quote re-fetchable at its URL. None of
these are our measurements and none should be read as one.

| vendor | their claim | what it measures | source |
|---|---|---|---|
| mem0 | search p50 0.148 s, p95 0.200 s | their search phase alone, their harness, LOCOMO | arxiv.org/abs/2504.19413 |
| Zep | "Graph search now returns results in 150ms (P95)"; context retrieval 200ms (P95) | internal server-side, self-measured | blog.getzep.com/scaling-agent-memory-zep-30x |
| Supermemory | "sub-300ms recall" | their hosted recall, no percentile stated | supermemory.ai |
| Letta | no retrieval latency number published | the honest cell is the absence | docs.letta.com |

Read with their own caveat, which Supermemory's blog states better than we could:
claimed latency often measures only one step of the pipeline. Every number above is
server-side or harness-side; a caller adds the network below.

## The distance tax, measured from this machine

TCP connect to their real API hostnames, 12 samples each, median, no request sent:

```
mem0         api.mem0.ai                  213.5 ms
zep          api.getzep.com                 5.3 ms
letta        api.letta.com                 30.0 ms
supermemory  api.supermemory.ai            29.7 ms
synap        synap-cloud-prod.maximem.ai   24.2 ms
```

synap appears in this table and not in the claims table above it, because synap
publishes no comparable latency figure; the connect floor is our measurement and
stands on its own.

**Read this table carefully, because it can mislead in both directions.** Four of the
five hosts resolve to CDN edges (Cloudflare ranges), so the connect lands on a server
near this machine and the number understates the real round trip to their
application, which still sits behind the edge. mem0's hostname resolves to origin
infrastructure directly, which is why its floor shows the actual distance. In every
case the floor is a lower bound on what a caller pays before authentication, TLS
session setup, request, embedding and inference, and it is a bound their server-side
claims sit on top of, never below.

## The comparison that is fair

Using each vendor's own published number against ours, at the current figures from the
re-run above: their fastest self-reported retrieval (Zep, 150 ms P95, server-side, before
any network) is roughly **69x** our measured warm P95 of 2.16 ms end-to-end on this
machine. mem0's own paper's search p50 of 148 ms is roughly **128x** our p50 of 1.16 ms.
Against the August figures the same two ratios read 130x and 220x, and the ratios moved
because our own number moved, not because theirs did. The asymmetry is structural,
not an optimization: their pipelines embed, traverse and rank on a server across a
network; this one reads an index that lives beside the files it indexes. What they
buy for that price is capability this system deliberately does not have: automatic
ingestion at scale. The price of ours is written in the abstention results one
directory over.
