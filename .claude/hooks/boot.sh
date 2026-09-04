#!/usr/bin/env bash
#
# The routing hook, wrapped so a clone that has not built anything is not punished for it.
#
# ## The defect this exists to fix
#
# `.claude/settings.json` is tracked, so a cold clone gets its hooks. Its `UserPromptSubmit`
# command pointed straight at `tools/kb/target/release/kb.exe`, and `target/` is gitignored
# (`.gitignore:21`), with zero files under `tools/kb/target` in the repository. So every
# clone of the public repository ran a hook whose command did not exist, **on every single
# prompt**, from the first message onward. The second half of the same bug: the path ends in
# `.exe`, which is wrong on every Linux and macOS clone even after they have built the tool.
#
# Found by a `kb panel` round on 2026-09-04, in Cicero's objection, and verified against
# `git check-ignore` and `git ls-files` before this was written.
#
# ## Why a wrapper rather than a smarter command string
#
# `settings.json` holds one command line and no logic. Anything conditional has to live in a
# file, and `.claude/hooks/promote-on-idle.sh` established both the pattern and the rule this
# file follows: **never fail somebody's session over a missing binary.** That hook guards with
# `[ -x "$KB" ] || exit 0` because a missing build is not an error worth showing somebody who
# is closing a terminal. This one fires on every prompt instead of once at exit, so the same
# discipline matters more here, and it was the one place it had not been applied.
#
# Exit 0 with no output is the correct silent failure: the runtime injects nothing and the
# conversation proceeds with no agent routed, which is exactly what `CLAUDE.md` already tells
# a session to expect when the hook is not installed.
set -u

ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

# KB_BIN overrides which build runs, matching promote-on-idle.sh. It is how the hook gets
# exercised against a build that is not installed yet, which is not hypothetical: the
# installed binary is routinely held open by a running MCP server and cannot be replaced.
#
# Unset, the name is resolved per platform rather than assumed. Cargo writes `kb.exe` only on
# Windows, so hard-coding either spelling breaks the other family.
if [ -n "${KB_BIN:-}" ]; then
  KB="$KB_BIN"
elif [ -x "$ROOT/tools/kb/target/release/kb" ]; then
  KB="$ROOT/tools/kb/target/release/kb"
else
  KB="$ROOT/tools/kb/target/release/kb.exe"
fi

# A checkout with no build and a checkout with no fleet are both ordinary states for somebody
# who has just cloned this repository to read it. Neither is an error.
[ -x "$KB" ] || exit 0
[ -d "$ROOT/fleet" ] || exit 0

# `--all` includes the private layer, which is correct here and only here: this hook runs for
# the fleet's own owner on their own machine. It is the wrong flag for any consumer other
# people talk to, and the README says so.
exec "$KB" boot "$ROOT" --top 5 --all
