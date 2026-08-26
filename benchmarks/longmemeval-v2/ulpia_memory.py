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
        # The reader's context budget is 200k tokens; serving ~40k chars per
        # file across 12 files stays under half of it and the harness truncates
        # by its own count anyway, so a generous slice costs nothing.
        self.max_chars_per_file = int(memory_params.get("max_chars_per_file", 60000))
        # Per file: its lines, the lowercased whole body, and each line's
        # character offset into that body. Term hits come from `str.find` over
        # the joined body (C speed) mapped back to lines by bisect; the first
        # assembly scanned line by line in Python and paid 700ms per query for
        # it, which the LAFS frontier's first budget point (one second) would
        # have noticed.
        self._file_cache: dict[str, tuple[list[str], str, list[int]]] = {}
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
        self._file_cache.clear()

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

        # Each ranked line is followed by a preview whose breadcrumb names the
        # section the scorer actually hit ("Trajectory X > state 6 >
        # observation"). That anchor is the product's own signal about where in
        # the book the match lives, so the slice opens there first. The first
        # instrumented run served slices around question words alone and lost
        # 78 percent of its evidence misses to exactly this: right file, wrong
        # lines.
        ranked: list[tuple[str, str, set[str]]] = []
        for line in out.splitlines():
            m = ROUTE_LINE.match(line)
            if m:
                ranked.append((m.group(2), m.group(3), set()))
                continue
            if ranked:
                for st in re.findall(r">\s*state (\d+)", line):
                    ranked[-1][2].add(st)

        # The question's own content words name further lines worth opening at.
        terms = [w for w in re.split(r"[^0-9a-zA-Z]+", query) if len(w) >= 4 and w.lower() not in STOP]

        items: list[MemoryContextItem] = []
        for agent, rel_path, states in ranked[: self.top_files]:
            path = self.workspace / agent / rel_path
            key = str(path)
            cached = self._file_cache.get(key)
            if cached is None:
                if not path.is_file():
                    continue
                lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
                joined = "\n".join(lines).lower()
                offsets = [0]
                for l in lines:
                    offsets.append(offsets[-1] + len(l) + 1)
                cached = (lines, joined, offsets)
                self._file_cache[key] = cached
            items.append(
                {
                    "type": "text",
                    "value": f"[{agent}/{rel_path}]\n{self._slice(cached, terms, states)}",
                }
            )
        return items

    def _slice(
        self,
        cached: tuple[list[str], str, list[int]],
        terms: list[str],
        states: set[str],
    ) -> str:
        """Serve the sections the scorer hit, whole and first, then windows
        around the lines the question's own words hit with whatever budget
        remains. Priority matters: the first assembly spent its budget in line
        order, so hundreds of cheap word hits near the top of a file starved
        the anchored section further down, and the instrument caught it as a
        recovered-then-stuck slice-miss class. When nothing lands (a thin
        FTS-only hit), serve the head: goal and first states."""
        lines, joined, offsets = cached
        anchored: set[int] = set()
        if states:
            starts: dict[str, int] = {}
            bounds: list[int] = []
            for i, l in enumerate(lines):
                m = re.match(r"## state (\d+)$", l)
                if m:
                    starts[m.group(1)] = i
                    bounds.append(i)
            bounds.append(len(lines))
            for st in states:
                if st in starts:
                    a = starts[st]
                    b = next(x for x in bounds if x > a)
                    anchored.update(range(a, b))

        import bisect

        windows: set[int] = set()
        for t in {t.lower() for t in terms if t}:
            pos = joined.find(t)
            while pos != -1:
                i = bisect.bisect_right(offsets, pos) - 1
                windows.update(
                    range(max(0, i - self.slice_radius), min(len(lines), i + self.slice_radius + 1))
                )
                pos = joined.find(t, offsets[min(i + 1, len(offsets) - 1)])

        if not anchored and not windows:
            head = "\n".join(lines[: self.slice_radius * 2])
            return head[: self.max_chars_per_file]

        keep: set[int] = set()
        size = 0
        for group in (anchored, windows - anchored):
            for i in sorted(group):
                size += len(lines[i]) + 1
                if size > self.max_chars_per_file:
                    break
                keep.add(i)
            if size > self.max_chars_per_file:
                break

        out: list[str] = []
        prev = -2
        for i in sorted(keep):
            if i != prev + 1:
                out.append("[...]")
            out.append(lines[i])
            prev = i
        return "\n".join(out)
