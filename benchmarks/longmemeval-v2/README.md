# LongMemEval-V2

The successor benchmark: 451 questions over haystacks of web-agent trajectories
(up to 115M tokens), scored on an accuracy-times-latency frontier (LAFS). Two
properties make it our terrain and both are the dataset's own numbers: 128 of
451 questions are abstention checks, the ability no competitor quotes, and
query latency is half the metric. Official harness:
[LongMemEval-V2](https://github.com/xiaowu0162/LongMemEval-V2).

## What ships here

| file | what it is |
|---|---|
| `ulpia_memory.py` | Ulpia as a V2 memory backend: markdown in, `kb route --hybrid` out, no model anywhere inside the backend |
| `setup_harness.py` | clones the official harness untouched and grafts the `ulpia` method in with two anchored patches; refuses to double-apply |
| `evidence_recall.py` | pre-reader instrument: builds the full small-tier haystack per domain and checks whether the served context holds the gold phrases; a ceiling for the reader, not a score |
| `smoke_test.py` | five trajectories, one question, timed; proof of plumbing, not a benchmark |
| `data/`, `harness/`, `runs/`, workspaces | gitignored; `data` is 1.2 GB of trajectory text pulled from the dataset's Hugging Face repository (screenshot archives, 5.9 GB, deliberately not pulled: this backend reads text) |

## The backend, honestly

Ingestion is the same weakest-honest floor as the V1 run: one markdown file per
trajectory, `Search for:` keys extracted mechanically (the extractor is a port
of the V1 harness's `mechanical_keys`, line for line). One choice is new and
named: keys come from the trajectory's narrative (goal, actions, thoughts,
URLs), not from the accessibility trees, because frequency ranking over UI
boilerplate buries intent; the trees still land in the body where the FTS half
of `--hybrid` searches them verbatim. Query shells out to the shipped `kb`
binary and serves line-window slices around the question's own content words
from the files the router named. Question images are a modality this backend
does not read, stated rather than hidden.

## Running it

```
python setup_harness.py
pip install numpy openai openai-agents transformers pillow
PYTHONPATH=harness python smoke_test.py
PYTHONPATH=harness python evidence_recall.py --domain enterprise
```

The full official run additionally needs, per the harness's own protocol:

- a reader endpoint serving Qwen/Qwen3.5-9B (OpenAI-compatible; the reader is
  fixed by the protocol, so no substituting a model we like better),
- `OPENAI_API_KEY` for the GPT-5.2 judge that scores the 156 llm-judged
  questions (the other 295 are matched deterministically by the harness),
- the harness's Python environment (`torch` included, CPU build suffices for
  the processor-based token counting).

Neither key lives in this repository and no agent here handles either one.

## Results, pre-reader

No official numbers yet: the official run needs the fixed Qwen3.5-9B reader and
the GPT-5.2 judge, and neither key lives here. What exists is the pre-reader
instrument, measured 2026-08-26 over the small tier's full haystacks (100
trajectories per domain, 11th Gen i5-1135G7, 16 GB, Windows 11), deterministic
questions only:

| reading | enterprise (141 q) | web (154 q) |
|---|---|---|
| full gold evidence in served context | 124 (88%) | 124 (81%) |
| partial evidence | 2 | 10 |
| no evidence | 15 | 20 |
| query latency p50 / p95 | 745 ms / 1.9 s | 773 ms / 2.9 s |

A ceiling, not a score: it measures whether the served context holds the gold
phrases verbatim, which bounds what any reader can compose from it. The
router's own ceiling sits higher: the ranked top-12 files hold the complete
gold for 94 percent (enterprise) and 90 percent (web) of questions, and the gap
between router and served decomposes, measured by `analyze_misses.py`, into
slice misses (11 and 15), route misses (6 and 9, the real routing gap), and
questions whose gold appears verbatim nowhere in the corpus (1 and 6), which is
the instrument's floor and not the system's. The adapter took five instrumented
rounds to get here (web went 51 to 81 percent); every fix is named in
`ulpia_memory.py`'s comments, none reads the gold. The adapter holds one
`kb serve` child open and speaks MCP to it, so `Memory::open` is paid once per
build; at this corpus scale (about 200 MB per haystack) the remaining query
cost splits into the Rust search itself (150 to 250 ms typical) and the Python
slice assembly (260 to 950 ms), both measured, both improvable, neither above
the LAFS frontier's first budget point of one second. The sub-millisecond
routing figure in `benchmarks/latency/` is a personal-fleet-scale number and is
not this regime, and the reference baselines
for context are the harness's own: simple RAG 51 percent at 0.2 s,
AgentRunbook-R 58.6 percent at 26.9 s, AgentRunbook-C 74.9 percent at 108 s,
each an official accuracy with a reader, which these numbers are not yet.

The 128 abstention questions and 28 gotcha checks are judged rather than
matched, so they wait for the official run.
