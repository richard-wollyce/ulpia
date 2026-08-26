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

Using each vendor's own published number against ours: their fastest self-reported
retrieval (Zep, 150 ms P95, server-side, before any network) is roughly **130x** our
measured warm P95 of 1.16 ms end-to-end on this machine. mem0's own paper's search
p50 of 148 ms is roughly **220x** our p50 of 0.68 ms. The asymmetry is structural,
not an optimization: their pipelines embed, traverse and rank on a server across a
network; this one reads an index that lives beside the files it indexes. What they
buy for that price is capability this system deliberately does not have: automatic
ingestion at scale. The price of ours is written in the abstention results one
directory over.
