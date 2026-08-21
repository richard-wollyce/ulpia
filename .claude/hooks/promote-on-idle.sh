#!/usr/bin/env bash
#
# The trigger ADR-0030 named as unbuilt. Promotion runs when a session goes idle.
#
# ## Why SessionEnd and not Stop
#
# Both events exist. `Stop` fires every time the assistant finishes a turn, which is not
# idleness: the person is usually reading, about to type the next thing. A promotion run on
# `Stop` would start minutes of model calls in the middle of a conversation, dozens of times
# an hour, over a deposit that has not changed since the last one.
#
# `SessionEnd` fires once, when the session actually stops receiving input. That is the
# thing ADR-0030 calls observable, and a clock is not.
#
# ## Why this returns immediately instead of doing the work
#
# `kb promote` is one Sonnet call per deposit file plus three Opus calls per proposal. That
# is minutes. A hook that holds the terminal for minutes on exit is a hook somebody deletes
# within a week, and then the feature is off and nobody remembers switching it off.
#
# So the run is detached and this script exits. `nohup ... &` puts kb in its own process with
# its output on a file rather than on a pipe this shell owns, so nothing waits on it: the
# hook is milliseconds and the run outlives the session that triggered it. The timeout in
# settings.json is 10 seconds, which this never approaches, and which exists only so a
# machine where the spawn itself hangs does not hang the exit with it.
#
# The cost of detaching is that no exit code reaches Claude Code. That is why the log below
# exists, and why it is the only record of a run nobody watched.
#
# ## Why the real thing and not --dry-run
#
# A dry run writes nothing, ever. A trigger that fires a dry run is a scheduled no-op whose
# whole output is a log file nobody opens, and `kb promote` would still be run by hand, which
# is exactly the state ADR-0030 records as the unbuilt one. The gate against a bad write is
# not a person watching this hook. It is inside the command: two promoters with different
# inputs, three lenses, unanimity, and every failure mode degrading toward writing nothing.
# An unreachable reviewer writes nothing. A missing VERDICT line is a refusal. A fleet with
# no `reviewer =` line refuses to start.
#
# What is true is that nobody sees this before it acts, and that is what `--max` answers:
# the run is bounded, so an unattended mistake is a bounded mistake. Everything it writes
# lands at `stage: captured`, which is the word that says no person has read it, and
# `fleet/` is gitignored, so nothing it writes can reach a commit on its own.
#
set -u

ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
# KB_BIN overrides which build runs. The installed release binary is held open by the
# running MCP server and cannot always be replaced, so this is how the hook is exercised
# against a build that is not installed yet. Unset, it is the installed one.
KB="${KB_BIN:-$ROOT/tools/kb/target/release/kb.exe}"
LOG="$ROOT/fleet/kb-promote.log"

# Never fail a session exit. A missing binary or a checkout with no fleet is not an error
# worth showing somebody who is closing their terminal.
[ -x "$KB" ] || exit 0
[ -d "$ROOT/fleet" ] || exit 0

# The log holds proposal slugs and refusal reasons, which are the base's private content,
# so it lives under fleet/ where the gitignore already covers everything. Trimmed rather
# than rotated: the interesting run is the last one.
if [ -f "$LOG" ] && [ "$(wc -c <"$LOG")" -gt 400000 ]; then
  tail -c 100000 "$LOG" >"$LOG.trim" && mv "$LOG.trim" "$LOG"
fi

{
  echo
  echo "=== $(date '+%Y-%m-%d %H:%M:%S')  session ${CLAUDE_SESSION_ID:-unknown} ended"
} >>"$LOG"

# A binary older than these flags does not reject them, it misreads them: `--max` is unknown,
# so the `3` after it stops being a value and becomes a path, and the run dies opening a base
# called 3. Checking first turns a confusing failure into a sentence naming the fix.
if ! "$KB" --help 2>/dev/null | grep -q -- "--lock"; then
  echo "kb promote: this build predates --lock and --max, so the hook did not run." >>"$LOG"
  echo "            Rebuild tools/kb and install it. Nothing was read or written." >>"$LOG"
  exit 0
fi

# --lock, because sessions end together and two runs over one deposit both propose the same
#   note before either has written it. The duplication lens cannot catch that: it is shown
#   what the base holds, and at that moment the base holds neither copy.
#
# --max 3, because nobody is watching. Three is a chosen number and not a measured one, and
#   the honest version of that sentence is in ADR-0030: the self-limiting rule is a junk rate
#   counted from what was written, and it cannot be picked before there are runs to count.
#   Three is small enough that a person reading `git -C fleet diff` at the end of a day is
#   reading three notes, and the remainder of the deposit is not lost, only left: the next
#   session end takes the next three.
#
# --all, matching the boot hook. The deposits and the notes both live in the private layer.
nohup "$KB" promote "$ROOT" --all --lock --max 3 >>"$LOG" 2>&1 &

exit 0
