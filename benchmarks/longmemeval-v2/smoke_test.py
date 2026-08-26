"""Insert five real trajectories, route one real question, time the query.

Not a benchmark: a proof that the plumbing holds before any number is quoted.
Run from this directory with PYTHONPATH=harness.
"""

import json
import time
from pathlib import Path

from memory_modules.memory import build_memory

HERE = Path(__file__).resolve().parent
DATA = HERE / "data"

questions = [json.loads(l) for l in open(DATA / "questions.jsonl", encoding="utf-8")]
haystacks = json.load(open(DATA / "lme_v2_small.json", encoding="utf-8"))

q = next(
    x
    for x in questions
    if x["domain"] == "enterprise" and x["eval_function"].startswith("norm_phrase_set")
)
wanted = set(haystacks[q["id"]][:5])
print(f"pergunta: {q['question'][:120]}...")
print(f"gold: {q['answer']}")

trajs = []
with open(DATA / "trajectories.jsonl", encoding="utf-8") as fh:
    for line in fh:
        t = json.loads(line)
        if t["id"] in wanted:
            trajs.append(t)
            if len(trajs) == len(wanted):
                break
print(f"trajetorias achadas: {len(trajs)}")

mem = build_memory(
    {
        "memory_type": "ulpia",
        "memory_params": {
            "kb_bin": str(HERE.parent.parent / "tools" / "kb" / "target" / "release" / "kb.exe"),
            "workspace_dir": str(HERE / "smoke_workspace"),
        },
    }
)

t0 = time.perf_counter()
for t in trajs:
    mem.insert(t)
print(f"insert de {len(trajs)}: {time.perf_counter() - t0:.2f}s")

t0 = time.perf_counter()
items = mem.query(q["question"])
first_query = time.perf_counter() - t0
t0 = time.perf_counter()
items2 = mem.query(q["question"])
warm_query = time.perf_counter() - t0

print(f"query fria (inclui o build do indice): {first_query:.2f}s")
print(f"query quente: {warm_query * 1000:.0f}ms")
print(f"itens servidos: {len(items)}")
for it in items[:3]:
    print("  ---", it["value"][:200].replace("\n", " | "))
total_chars = sum(len(i["value"]) for i in items)
print(f"total de caracteres servidos: {total_chars}")
