# LongMemEval, end to end

[LongMemEval](https://github.com/xiaowu0162/longmemeval) (ICLR 2025, MIT) hands each of
500 instances a question and a haystack of timestamped chat sessions, and grades a
free-text answer across five abilities: information extraction, multi-session
reasoning, knowledge updates, temporal reasoning, and abstention.

Ulpia runs it with the product, not a harness that impersonates one: the converter
turns each instance's sessions into a one-agent fleet of markdown memory files, and
then the shipped pipeline runs unmodified through the `kb` library: the walker, both
scorers, the verdict, and the `kb answer` grounding rules. What is scored is the thing
you can clone.

```
# fetch the data (280MB, gitignored, third-party):
curl -L -o data/longmemeval_s_cleaned.json \
  https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json

kb-bench longmem data/longmemeval_s_cleaned.json \
  --answerer ../../tools/answer-claude.cmd \
  --judge judge-claude.cmd \
  --out hypotheses-s.jsonl --workers 6
```

## The honesty rules of this run

- **The keys are mechanical.** A real fleet's `Search for:` lines are authored;
  benchmark ingestion generates them from each session's own frequent surviving words
  and bigrams. That is the weakest honest ingestion, deliberately: the score is a
  floor, and an authored fleet only does better.
- **The local judge is not the official protocol.** The official script judges with
  GPT-4o; ours judges with a Claude model and says so in its own output. The harness
  writes the official `hypotheses-s.jsonl` so anyone can re-judge with the official
  evaluator before comparing against any published number.
- **Abstention is the product's own refusal.** A `Nothing` verdict answers "the
  history does not hold this" without a model call, and the grounding rules order the
  model to say what the passages lack. Nothing special-cases the `_abs` questions on
  the answering side; only the judge knows which they are.

Results with the full configuration header: [RESULTS.md](RESULTS.md)
