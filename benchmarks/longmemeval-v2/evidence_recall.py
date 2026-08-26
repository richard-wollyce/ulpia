"""Pre-reader instrument: does the served context hold the gold evidence?

No reader model exists on this machine yet, so this measures the half of the
pipeline that is ours: build the full small-tier haystack per domain, run every
deterministically-evaluated question through `UlpiaMemory.query`, and check
whether each gold phrase appears verbatim (case-insensitive, hyphens
normalised) in the served text. That is a ceiling for the reader, not a score:
evidence present does not mean the reader will compose it, and evidence absent
means it cannot. The abstention and llm-judged questions are skipped because
their gold is a judgment, not a phrase.

Usage: PYTHONPATH=harness python evidence_recall.py [--domain enterprise|web]
"""

import argparse
import json
import re
import sys
import time
from pathlib import Path

from memory_modules.memory import build_memory

HERE = Path(__file__).resolve().parent
DATA = HERE / "data"


def norm(s: str) -> str:
    return re.sub(r"[\s ]+", " ", s.replace("-", " ").lower()).strip()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", choices=["enterprise", "web"], required=True)
    ap.add_argument("--limit", type=int, default=None)
    args = ap.parse_args()

    questions = [json.loads(l) for l in open(DATA / "questions.jsonl", encoding="utf-8")]
    haystacks = json.load(open(DATA / "lme_v2_small.json", encoding="utf-8"))

    qs = [
        q
        for q in questions
        if q["domain"] == args.domain
        and (q["eval_function"].startswith("norm_phrase") or q["eval_function"].startswith("mc_choice"))
    ]
    if args.limit:
        qs = qs[: args.limit]
    ids = set(haystacks[qs[0]["id"]])
    print(f"{args.domain}: {len(qs)} perguntas determinsticas, haystack de {len(ids)} trajetorias")

    ws = HERE / f"workspace_{args.domain}_small"
    mem = build_memory(
        {
            "memory_type": "ulpia",
            "memory_params": {
                "kb_bin": str(HERE.parent.parent / "tools" / "kb" / "target" / "release" / "kb.exe"),
                "workspace_dir": str(ws),
            },
        }
    )

    existing = {p.stem for p in (ws / "trajectories" / "memory").glob("*.md")}
    todo = ids - existing
    if todo:
        t0 = time.perf_counter()
        n = 0
        with open(DATA / "trajectories.jsonl", encoding="utf-8") as fh:
            for line in fh:
                t = json.loads(line)
                if t["id"] in todo:
                    mem.insert(t)
                    n += 1
                    if n == len(todo):
                        break
        print(f"insert de {n}: {time.perf_counter() - t0:.1f}s")
    else:
        print("workspace ja construido")

    full = 0
    partial = 0
    none_at_all = 0
    lat = []
    for i, q in enumerate(qs):
        t0 = time.perf_counter()
        items = mem.query(q["question"])
        lat.append(time.perf_counter() - t0)
        served = norm(" ".join(it["value"] for it in items))
        if q["eval_function"].startswith("mc_choice"):
            golds = [q["answer"]]
        else:
            seps = "[,;]"
            golds = [g for g in re.split(seps, q["answer"]) if g.strip()]
        hit = sum(1 for g in golds if norm(g) in served)
        if hit == len(golds):
            full += 1
        elif hit > 0:
            partial += 1
        else:
            none_at_all += 1
        sys.stdout.write(
            f"  [{i + 1}/{len(qs)}] {q['id']} {hit}/{len(golds)} golds no contexto\n"
        )
    lat.sort()
    p50 = lat[len(lat) // 2]
    p95 = lat[int(len(lat) * 0.95)]
    print(f"\n{args.domain}, evidencia servida (teto do leitor, nao um score):")
    print(f"  toda a evidencia presente : {full}/{len(qs)}")
    print(f"  evidencia parcial         : {partial}/{len(qs)}")
    print(f"  nenhuma evidencia         : {none_at_all}/{len(qs)}")
    print(f"  latencia de query p50 {p50 * 1000:.0f}ms, p95 {p95 * 1000:.0f}ms")


if __name__ == "__main__":
    main()
