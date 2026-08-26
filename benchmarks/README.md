# Benchmarks

Four instruments, each runnable from a clone as far as a clone can go, because a
number a stranger cannot
reproduce is a claim. Every result file states its machine, its commit, its command
and its date; the configuration moves a score as much as the system does, so the
configuration travels with the score.

## Abstention: can it say no

Every retrieval system always returns a rank one, because ranking cannot express
absence. Ulpia's refusal is its differentiating claim, so it is the first thing
measured rather than the first thing advertised.

```
kb-bench abstain examples/demo benchmarks/abstention/questions.tsv
```

The question set was authored blind: the in-scope author saw only a topics list, the
out-of-scope author saw nothing of the corpus at all, and an adversarial pass checked
both for accidental echoes of the corpus keys before anything ran. The provenance
matters because this repository's own answer key was once tuned against its questions
on the day it was graded, and the number it produced did not survive.

Results: [abstention/RESULTS.md](abstention/RESULTS.md)

## Latency: local software against the speed of light

The deterministic pipeline, timed three ways: cold start, warm p50 and p95 in
process, and the TCP connect floor to the hosted competitors' real API endpoints,
measured from this machine with no request sent. That last number is the distance
tax any cloud memory pays before authentication, embedding or inference begin;
vendor-reported server-side latencies sit on top of it, never below it.

```
kb-bench latency examples/demo benchmarks/abstention/questions.tsv ^
  --host mem0=api.mem0.ai --host zep=api.getzep.com --host letta=api.letta.com ^
  --host supermemory=api.supermemory.ai --host synap=synap-cloud-prod.maximem.ai
```

Results: [latency/RESULTS.md](latency/RESULTS.md)

## LongMemEval, the full 500

The public benchmark, run end to end against the product itself: each instance's chat
sessions become a one-agent fleet and the shipped pipeline answers. First run: 49
percent total under the weakest honest ingestion and a non-official local judge, and
**97 percent on abstention**, the ability the benchmark's own paper reports systems
fail hardest and no competitor quotes at all.

Results and honesty rules: [longmemeval/RESULTS.md](longmemeval/RESULTS.md)

## What is deliberately not here

Competitor products are not scored by this harness. Running mem0 or Letta badly and
publishing the number would be the exact failure this fleet documented in a
competitor's benchmark and refuses to repeat: the harness measures Ulpia and a
top-k baseline over the same corpus, states what the vendors claim with their own
citations, and leaves their products to their own harnesses.
