"""Classify every evidence miss by the mechanism that lost it.

Four classes, checked in order, because each has a different fix and lumping
them is how a wrong theory survives:

  slice-miss    the router named a file that holds the gold, the served slice
                just did not include those lines (fix: slicing, cheap)
  route-miss    some file in the corpus holds the gold verbatim, none of the
                ranked files do (fix: routing, the real gap)
  not-verbatim  no file in the corpus holds the gold phrase at all; the answer
                is a paraphrase or a judgment, and this instrument's verbatim
                proxy simply cannot see it (fix: none; the proxy's own floor)

Also times the two halves of a query, the `kb route` subprocess and the
Python slicing, because the p50 went from 155ms at 5 trajectories to 1.3s at
100 and a number without a mechanism is a rumor.

Usage: PYTHONPATH=harness python analyze_misses.py --domain web
"""

import argparse
import json
import re
import subprocess
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
DATA = HERE / "data"
KB = str(HERE.parent.parent / "tools" / "kb" / "target" / "release" / "kb.exe")

ROUTE_LINE = re.compile(r"^\s{2}([\d.]+)\s+(\S+)\s+(\S+)\s+")


def norm(s: str) -> str:
    return re.sub(r"[\s ]+", " ", s.replace("-", " ").lower()).strip()


def golds_of(q: dict) -> list[str]:
    if q["eval_function"].startswith("mc_choice"):
        return [q["answer"]]
    return [g for g in re.split("[,;]", q["answer"]) if g.strip()]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", choices=["enterprise", "web"], required=True)
    args = ap.parse_args()

    ws = HERE / f"workspace_{args.domain}_small"
    mem_dir = ws / "trajectories" / "memory"
    corpus = {p.name: norm(p.read_text(encoding="utf-8", errors="replace")) for p in mem_dir.glob("*.md")}
    print(f"corpus: {len(corpus)} arquivos")

    questions = [
        q
        for q in (json.loads(l) for l in open(DATA / "questions.jsonl", encoding="utf-8"))
        if q["domain"] == args.domain
        and (q["eval_function"].startswith("norm_phrase") or q["eval_function"].startswith("mc_choice"))
    ]

    import sys
    sys.path.insert(0, str(HERE / "harness"))
    from memory_modules.memory import build_memory

    mem = build_memory(
        {
            "memory_type": "ulpia",
            "memory_params": {"kb_bin": KB, "workspace_dir": str(ws)},
        }
    )

    counts = {"served": 0, "slice-miss": 0, "route-miss": 0, "not-verbatim": 0}
    route_ms, slice_ms = [], []
    for q in questions:
        golds = [norm(g) for g in golds_of(q)]

        t0 = time.perf_counter()
        out = subprocess.run(
            [KB, "route", q["question"], str(ws), "--hybrid", "--top", "12", "--all"],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
        ).stdout
        route_ms.append((time.perf_counter() - t0) * 1000)

        ranked = [m.group(3).split("/")[-1] for m in map(ROUTE_LINE.match, out.splitlines()) if m]

        t0 = time.perf_counter()
        items = mem.query(q["question"])
        slice_ms.append((time.perf_counter() - t0) * 1000 - route_ms[-1])
        served = norm(" ".join(i["value"] for i in items))

        missing = [g for g in golds if g not in served]
        if not missing:
            counts["served"] += 1
            continue
        in_ranked = any(all(g in corpus.get(f, "") for g in golds) for f in ranked)
        in_corpus = any(all(g in body for g in golds) for body in corpus.values())
        if in_ranked:
            counts["slice-miss"] += 1
        elif in_corpus:
            counts["route-miss"] += 1
        else:
            counts["not-verbatim"] += 1

    print(f"\n{args.domain}:")
    for k, v in counts.items():
        print(f"  {k:12s} {v}/{len(questions)}")
    route_ms.sort(); slice_ms.sort()
    mid = len(route_ms) // 2
    print(f"  rota p50 {route_ms[mid]:.0f}ms | fatiamento+leitura p50 {max(0, slice_ms[mid]):.0f}ms")


if __name__ == "__main__":
    main()
