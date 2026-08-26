"""Ulpia as a LongMemEval-V2 memory backend.

The mechanism, stated up front: `insert` writes each trajectory as one markdown
file in a one-agent fleet, with mechanically extracted `Search for:` keys, the
same weakest-honest-ingestion floor the V1 run used (the key extractor below is
a line-for-line port of `tools/bench/src/longmem.rs::mechanical_keys`). `query`
shells out to the shipped `kb` binary, `kb route --hybrid`, and serves passage
slices from the files the router named. No embedding model, no controller model,
no network: the only model anywhere in this backend is none.

One ingestion choice is not in V1 and is named here so nobody mistakes it for
tuning: the `Search for:` keys are extracted from the trajectory's narrative
(goal, actions, thoughts, URLs) and not from the accessibility trees, because a
tree is mostly UI boilerplate ("button", "link", "textbox") and frequency
ranking over it would bury every key that carries intent. The trees still land
in the file body, where the FTS half of `--hybrid` searches them verbatim.

This file lives in the Ulpia repository and is copied into the official
harness's `memory_modules/` by `setup_harness.py`; it imports only the harness
base class and the standard library.
"""

from __future__ import annotations

import re
import subprocess
import threading
from collections import Counter
from pathlib import Path

from .memory import Memory, MemoryContextItem, register_memory, require

STOP = {
    "the", "and", "for", "that", "with", "this", "you", "your", "have", "has", "had",
    "was", "were", "are", "not", "but", "they", "their", "them", "from", "what",
    "when", "where", "which", "will", "would", "could", "should", "about", "there",
    "been", "being", "into", "over", "also", "just", "like", "some", "more", "can",
    "than", "then", "out", "get", "got", "how", "who", "why", "his", "her", "she",
    "him", "its", "our", "ours", "any", "all", "one", "two", "did", "does", "doing",
    "assistant", "user", "yes", "okay", "sure", "thanks", "thank", "help", "know",
    "want", "need", "make", "made", "really", "very", "much", "many", "here",
}


def mechanical_keys(text: str, cap: int) -> list[str]:
    """The V1 floor's key extractor, ported. Bigrams seen twice fill the first
    third of the cap; unigrams fill the rest unless a kept phrase already
    contains them. Ties break alphabetically so the output is deterministic."""

    def stop(w: str) -> bool:
        return len(w) < 4 or w in STOP

    words = [w for w in re.split(r"[^0-9a-z]+", text.lower()) if w]
    uni = Counter(w for w in words if not stop(w))
    bi = Counter(
        f"{a} {b}" for a, b in zip(words, words[1:]) if not stop(a) and not stop(b)
    )

    ranked = sorted(uni.items(), key=lambda kv: (-kv[1], kv[0]))
    ranked_bi = sorted(
        ((k, n) for k, n in bi.items() if n >= 2), key=lambda kv: (-kv[1], kv[0])
    )

    keys: list[str] = [k for k, _ in ranked_bi[: cap // 3]]
    for k, _ in ranked:
        if len(keys) >= cap:
            break
        if not any(k in have for have in keys):
            keys.append(k)
    return keys


# One `--hybrid` result line: fused score, agent name, path inside the agent,
# then provenance ("keywords #1 + text #1"). Verified against live output; the
# numbered format in the README is the keyword-only scorer's, not this one.
ROUTE_LINE = re.compile(r"^\s{2}([\d.]+)\s+(\S+)\s+(\S+)\s+")


@register_memory
class UlpiaMemory(Memory):
    """Deterministic file-based memory: markdown in, `kb route` out."""

    memory_type = "ulpia"

    def __init__(self, memory_params: dict[str, object]) -> None:
        super().__init__(memory_params)
        self.kb_bin = str(memory_params.get("kb_bin", "kb"))
        self.workspace = Path(
            str(memory_params.get("workspace_dir", "ulpia_workspace"))
        ).resolve()
        self.top_files = int(memory_params.get("top_files", 12))
        self.slice_radius = int(memory_params.get("slice_radius", 40))
        self.max_chars_per_file = int(memory_params.get("max_chars_per_file", 16000))
        self.agent = self.workspace / "trajectories"
        (self.agent / "memory").mkdir(parents=True, exist_ok=True)
        self._index_lock = threading.Lock()
        self._dirty = True
        self._write_agent_shape()

    def _write_agent_shape(self) -> None:
        (self.agent / "agent.txt").write_text(
            "name = Trajectories\nrole = Past agent trajectories, one file per run\n",
            encoding="utf-8",
        )
        (self.agent / "index.md").write_text(
            "# Trajectories\n\n**Search for:** `trajectory`, `past run`, `previous task`,"
            " `earlier session`, `workflow`, `how to`, `steps`\n\n"
            "**Exists to:** Hold past agent trajectories as memory files, one per run\n",
            encoding="utf-8",
        )

    def insert(self, trajectory: dict[str, object]) -> None:
        traj_id = str(trajectory["id"])
        goal = str(trajectory.get("goal", ""))
        outcome = str(trajectory.get("outcome", ""))
        start_url = str(trajectory.get("start_url", ""))
        states = trajectory.get("states") or []

        narrative_parts = [goal, start_url]
        body_parts = [
            f"goal: {goal}\n\noutcome: {outcome}\n\nstart_url: {start_url}\n"
        ]
        for st in states:
            url = str(st.get("url") or "")
            action = str(st.get("action") or "")
            thought = str(st.get("thought") or "")
            tree = str(st.get("accessibility_tree") or "")
            narrative_parts.extend([url, action, thought])
            body_parts.append(
                f"\n## state {st.get('state_index')}\n\n"
                f"url: {url}\n\naction: {action}\n\nthought: {thought}\n\n"
                f"### observation\n\n{tree}\n"
            )

        keys = mechanical_keys(" ".join(narrative_parts), 45)
        keyline = ", ".join(f"`{k}`" for k in keys) or "`trajectory`"
        text = (
            f"# Trajectory {traj_id}\n\n**Search for:** {keyline}\n\n"
            f"**Exists to:** Record the {outcome or 'recorded'} run for: {goal[:200]}\n\n"
            + "".join(body_parts)
        )
        out = self.agent / "memory" / f"{traj_id}.md"
        out.write_text(text, encoding="utf-8", errors="replace")
        self._dirty = True

    def _save_backend(self, output_dir: Path) -> None:
        # The markdown is the memory; the `.kb` index is derived and disposable,
        # so it is not persisted and the load path rebuilds it. Same doctrine as
        # the product: delete `.kb` and you have lost a rebuild, not a fact.
        import shutil

        dest = Path(output_dir) / "ulpia_workspace"
        if dest.resolve() == self.workspace:
            return
        shutil.copytree(
            self.workspace,
            dest,
            ignore=shutil.ignore_patterns(".kb"),
            dirs_exist_ok=True,
        )

    def _load_backend(self, input_dir: Path) -> None:
        self.workspace = (Path(input_dir) / "ulpia_workspace").resolve()
        self.agent = self.workspace / "trajectories"
        require(
            (self.agent / "memory").is_dir(),
            f"saved ulpia workspace missing at {self.workspace}",
        )
        self._dirty = True

    def _kb(self, *args: str) -> str:
        proc = subprocess.run(
            [self.kb_bin, *args, str(self.workspace), "--all"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=600,
        )
        require(
            proc.returncode == 0,
            f"kb {' '.join(args[:1])} failed: {proc.stderr.strip()[:500]}",
        )
        return proc.stdout

    def _ensure_index(self) -> None:
        with self._index_lock:
            if self._dirty:
                self._kb("index")
                self._dirty = False

    def query(
        self,
        query: str,
        query_image: str | None = None,
    ) -> list[MemoryContextItem]:
        # A question image is a modality this backend does not read; the text of
        # the question still routes. Stated, not hidden.
        self._ensure_index()
        out = self._kb("route", query, "--hybrid", "--top", str(self.top_files))

        ranked: list[tuple[str, str]] = []
        for line in out.splitlines():
            m = ROUTE_LINE.match(line)
            if m:
                ranked.append((m.group(2), m.group(3)))

        # Slice around the question's own content words: the router named the
        # book, the question names the lines worth opening it at.
        terms = [w for w in re.split(r"[^0-9a-zA-Z]+", query) if len(w) >= 4 and w.lower() not in STOP]

        items: list[MemoryContextItem] = []
        for agent, rel_path, in ranked[: self.top_files]:
            path = self.workspace / agent / rel_path
            if not path.is_file():
                continue
            body = path.read_text(encoding="utf-8", errors="replace")
            items.append(
                {
                    "type": "text",
                    "value": f"[{agent}/{rel_path}]\n{self._slice(body, terms)}",
                }
            )
        return items

    def _slice(self, body: str, terms: list[str]) -> str:
        """Serve windows around the lines the router's terms actually hit, the
        same shape as the harness's own raw-state slicing. When no term lands
        (an FTS-only hit), serve the head: goal and first states."""
        lines = body.splitlines()
        lowers = [l.lower() for l in lines]
        hits = [
            i
            for i, l in enumerate(lowers)
            if any(t and t.lower() in l for t in terms)
        ]
        if not hits:
            head = "\n".join(lines[: self.slice_radius * 2])
            return head[: self.max_chars_per_file]

        keep: set[int] = set()
        for i in hits:
            keep.update(range(max(0, i - self.slice_radius), min(len(lines), i + self.slice_radius + 1)))

        out: list[str] = []
        prev = -2
        for i in sorted(keep):
            if i != prev + 1:
                out.append("[...]")
            out.append(lines[i])
            prev = i
            if sum(len(l) + 1 for l in out) > self.max_chars_per_file:
                break
        return "\n".join(out)
