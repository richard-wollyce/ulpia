"""Graft the `ulpia` method into the official LongMemEval-V2 harness.

The harness stays theirs: this clones it unmodified, copies `ulpia_memory.py`
into `memory_modules/`, and applies two one-line-sized patches, the method name
in `METHODS` and a branch in `build_memory_config`. Nothing in the evaluation
path is touched, because a benchmark run where the harness was edited is a
benchmark run nobody should believe. Re-running this script is safe: every
patch checks whether it already applied and refuses to double-apply.

Usage: python setup_harness.py  (from this directory)
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
HARNESS = HERE / "harness"
REPO_URL = "https://github.com/xiaowu0162/LongMemEval-V2"


def patch(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        print(f"  ok {label} (ja aplicado)")
        return
    if old not in text:
        sys.exit(f"PATCH FALHOU, ancora nao encontrada: {label}")
    if text.count(old) != 1:
        sys.exit(f"PATCH FALHOU, ancora ambigua: {label}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    print(f"  ok {label}")


def main() -> None:
    if not HARNESS.exists():
        subprocess.run(
            ["git", "clone", "--depth", "1", REPO_URL, str(HARNESS)], check=True
        )
    print("harness presente")

    shutil.copy2(HERE / "ulpia_memory.py", HARNESS / "memory_modules" / "ulpia_memory.py")
    print("  ok ulpia_memory.py copiado")

    patch(
        HARNESS / "memory_modules" / "memory.py",
        "from .rag import RagMemory  # noqa: E402,F401",
        "from .rag import RagMemory  # noqa: E402,F401\n"
        "from .ulpia_memory import UlpiaMemory  # noqa: E402,F401",
        "registro em memory.py",
    )

    run_eval = HARNESS / "evaluation" / "run_eval.py"
    patch(
        run_eval,
        '    "no_retrieval",\n',
        '    "no_retrieval",\n    "ulpia",\n',
        "METHODS",
    )
    patch(
        run_eval,
        '    if args.method == "no_retrieval":\n'
        '        return {"memory_type": "no_retrieval", "memory_params": {}}\n',
        '    if args.method == "no_retrieval":\n'
        '        return {"memory_type": "no_retrieval", "memory_params": {}}\n'
        '    if args.method == "ulpia":\n'
        "        return {\n"
        '            "memory_type": "ulpia",\n'
        '            "memory_params": {\n'
        '                "kb_bin": os.getenv("ULPIA_KB_BIN", "kb"),\n'
        '                "workspace_dir": str(Path(args.output_dir) / "ulpia_workspace"),\n'
        "            },\n"
        "        }\n",
        "build_memory_config",
    )
    print("pronto: --method ulpia disponivel no run_eval.py")


if __name__ == "__main__":
    main()
