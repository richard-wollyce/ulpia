# Ulpia

A local first fleet of agents with one memory layer under them. Ulpia is the
library; Vesta is its librarian, the orchestrator that routes what arrives. Read
[`README.md`](README.md) for what is here and how to run it.

## If you are working on this repository

The system is in `tools/`. `tools/kb` is the memory layer and has one dependency;
`tools/tray` is a Windows tray app over the same library. Start at
`tools/kb/src/memory.rs`, which is the contract every surface goes through.

Run the tests before believing anything: `cargo test` in `tools/kb`.

## If an architect agent is present

`fleet/` is a separate, private repository and may not be here at all. When it is, and
it holds `fleet/zed/`, **read [`fleet/zed/CLAUDE.md`](fleet/zed/CLAUDE.md) first and
follow it instead of this file.** That agent carries the operating instructions, the
quality bar, the autonomy limits and the design record, and none of that is in here.

## Rules that hold either way

- **Name the mechanism.** A recommendation without the reason it works does not ship.
- **Two options and their consequences**, or it is a preference, not a decision.
- **Mark what is unverified.** Ran it, read the source, read the docs, or guessing. Say
  which.
- **Never claim something works without running it.** If it was not run, say it was not
  run.
- **Nothing under `fleet/` is ever committed here.** It is gitignored and it is
  somebody's private knowledge. Do not add exceptions to that rule.
- **No em dashes.** Not in chat, not in files, not in code comments, not in commit
  messages.
