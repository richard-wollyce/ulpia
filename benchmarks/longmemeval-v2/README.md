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

## Results

None yet. Numbers land here the way every RESULTS file in `benchmarks/` lands:
exact command, commit, machine, date, and the caveat that keeps them honest.
