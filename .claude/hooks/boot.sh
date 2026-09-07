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
# exercised against a build that is not installed yet.
#
# ## Why the installed binary is not the file cargo wrote
#
# `tools/kb/bin/` is the installed location and `tools/kb/target/release/` is a build
# directory. They were the same path until 2026-09-06, and on Windows that is a defect and
# not a shortcut: a running image cannot have its file removed, cargo installs its artifact
# by remove-then-hardlink, and `.mcp.json` kept a `kb serve` mapped on that exact name for
# the life of a session. So `cargo build --release` compiled the fix in 004f29f and then
# died on its final step with `failed to remove file ... Acesso negado (os error 5)`. For
# 24 minutes the repository held a fix for silent data loss in `kb ingest` and every caller
# of the installed binary still had the defect, and nothing could say so. Separating the two
# paths removes the collision instead of recovering from it.
#
# ## Why this hook still falls back to the build directory and `.mcp.json` does not
#
# What causes the collision is not reading the build directory, it is holding it open. The
# holder in the incident was the persistent `kb serve`, which lives as long as a session, so
# `.mcp.json` names `bin/` alone and can never map a build artifact again. This hook runs
# `kb boot` and exits in milliseconds, so the fallback below maps the build directory for
# milliseconds, and only while `bin/` is still empty. Dropping the fallback instead would
# mean a clone that ran `cargo build --release` and nothing else gets a dead hook on every
# prompt, which is the exact defect the header above records this file as fixing, arriving
# from the other side. `tools/kb/install.sh` is tracked and fills `bin/` in one line, so that
# state is now a step somebody has not taken yet rather than a step nobody was given; the
# fallback stays because the cost of keeping it is a branch on a file that is not there, and
# the cost of dropping it is a silent dead hook for whoever built and read no further.
#
# Unset, the name is resolved per platform rather than assumed. Cargo writes `kb.exe` only on
# Windows, so hard-coding either spelling breaks the other family. The `.exe` is tested first
# in each pair because MSYS resolves an extensionless `kb` to `kb.exe` inside `[ -x ]`, so
# testing the bare name first on Windows takes a branch on a file that is not there. On Linux
# and macOS the `.exe` test simply fails and the bare name is reached.
if [ -n "${KB_BIN:-}" ]; then
  KB="$KB_BIN"
elif [ -x "$ROOT/tools/kb/bin/kb.exe" ]; then
  KB="$ROOT/tools/kb/bin/kb.exe"
elif [ -x "$ROOT/tools/kb/bin/kb" ]; then
  KB="$ROOT/tools/kb/bin/kb"
elif [ -x "$ROOT/tools/kb/target/release/kb.exe" ]; then
  KB="$ROOT/tools/kb/target/release/kb.exe"
else
  KB="$ROOT/tools/kb/target/release/kb"
fi

# The drift finding left by the last commit that touched `tools/kb/`, when it is still true.
# Written by `.githooks/post-commit` and only read here, never computed here: this hook runs
# on every message that gets typed, and `kb-install.sh --check` costs a process spawn, a
# sha256 over the whole binary and up to four git calls. Both tests below are shell builtins,
# so the common path where there is nothing to report spawns nothing at all.
#
# `-nt` is the staleness guard and it is what makes the file trustworthy rather than just
# available. A report written before the installed binary was last replaced was answered by
# that install: it describes a file that is no longer there, so it stays quiet instead of
# nagging about drift that is already fixed. When `$KB` does not exist at all, `-nt` against
# a missing file is true, which is the correct reading, because no installed binary is the
# loudest drift there is.
DRIFT="$ROOT/fleet/kb-drift.txt"
if [ -s "$DRIFT" ] && [ "$DRIFT" -nt "$KB" ]; then
  cat "$DRIFT"
  echo
fi

# A checkout with no build and a checkout with no fleet are both ordinary states for somebody
# who has just cloned this repository to read it. Neither is an error.
[ -x "$KB" ] || exit 0
[ -d "$ROOT/fleet" ] || exit 0

# `--all` includes the private layer, which is correct here and only here: this hook runs for
# the fleet's own owner on their own machine. It is the wrong flag for any consumer other
# people talk to, and the README says so.
exec "$KB" boot "$ROOT" --top 5 --all
