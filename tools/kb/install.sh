#!/usr/bin/env sh
#
# install: put the binary you just built where everything in this repository looks for it.
#
#   cargo build --release --manifest-path tools/kb/Cargo.toml
#   sh tools/kb/install.sh
#
# ## Why an install step exists at all, instead of everything naming `target/release/`
#
# `tools/kb/target/release/` is a build directory and `tools/kb/bin/` is an install
# directory, and they are two paths on purpose. `.mcp.json` maps `kb serve` for the whole
# life of a session, so whatever path it names is held open by a running process. On
# Windows a running image cannot have its file removed, and cargo installs its artifact by
# remove-then-hardlink, so when those two paths were the same file `cargo build --release`
# compiled a fix and then died on its own final step with
# `failed to remove file ... (os error 5)`. Separating them removes the collision instead
# of recovering from it: cargo writes into `target/`, which nothing maps, and this script
# moves the result into `bin/`, which is the only name the tracked config files carry.
#
# ## The mechanism: rename-aside, not copy-over
#
# Replacing the installed file is exactly the operation Windows refuses while a `kb serve`
# has it mapped. Measured on 2026-09-06 against a live serve:
#
#   cp new kb.exe    ->  "Device or resource busy", exit 1
#   mv kb.exe aside  ->  exit 0, and the serve kept running
#
# A rename succeeds on a mapped file, and the running process keeps executing the image
# under its new name until it exits. So the old file is moved out of the way first and the
# new one is copied into the freed name. Nothing is ever stopped or killed: another
# session's MCP server is not this script's to take away.
#
# ## What this deliberately does not do
#
# No download, no signature check, no version gate, no record of what was installed, no
# rollback. It installs the file cargo just wrote, which is the file you can read the
# source of. The operator-side sibling that does all of that, because a machine running
# this fleet unattended needs to know which commit is actually executing, is
# `fleet/frontinus/tools/kb-install.sh`, and it is private on purpose: its gates, its
# release download and its install record describe one operator's machine, not this tool.
# Do not copy logic between the two. This one is the whole public contract.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SRC_DIR="$HERE/target/release"
DEST_DIR="$HERE/bin"

case "${1:-}" in
  -h|--help)
    cat <<'MSG'
install: copy the release binary from target/release/ into tools/kb/bin/.

  cargo build --release --manifest-path tools/kb/Cargo.toml
  sh tools/kb/install.sh

tools/kb/bin/ is the path .mcp.json and the hooks under .claude/ name, and it is
gitignored, so every clone fills it once from its own build.
MSG
    exit 0
    ;;
esac

# Cargo writes `kb.exe` only on Windows, so the name is discovered rather than assumed:
# hard-coding either spelling breaks the other family.
if [ -f "$SRC_DIR/kb.exe" ]; then
  NAME=kb.exe
elif [ -f "$SRC_DIR/kb" ]; then
  NAME=kb
else
  printf 'install: nothing to install. Build it first:\n\n' >&2
  printf '  cargo build --release --manifest-path tools/kb/Cargo.toml\n\n' >&2
  exit 1
fi

SRC="$SRC_DIR/$NAME"
DEST="$DEST_DIR/$NAME"
mkdir -p "$DEST_DIR"

ASIDE=""
if [ -e "$DEST" ]; then
  ASIDE="$DEST_DIR/$NAME.replaced.$$"
  mv "$DEST" "$ASIDE" || {
    printf 'install: could not move the installed binary aside. Nothing changed.\n' >&2
    exit 1
  }
fi

if ! cp "$SRC" "$DEST"; then
  # A failed install leaves the machine exactly as it found it.
  [ -n "$ASIDE" ] && mv "$ASIDE" "$DEST" 2>/dev/null
  printf 'install: could not copy the new binary into place. The previous one is back.\n' >&2
  exit 1
fi
chmod +x "$DEST"

# The old file is only removable once nothing is executing it. A leftover here is not a
# failure, it is the honest signal that a `kb serve` from an open session is still running
# the previous build and will pick the new one up when that session ends.
if [ -n "$ASIDE" ] && ! rm -f "$ASIDE" 2>/dev/null; then
  printf 'install: a process is still running the previous build, left at %s\n' "$ASIDE"
fi

printf 'installed %s -> %s\n' "$SRC" "$DEST"
"$DEST" --version
