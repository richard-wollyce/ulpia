# Ulpia

A local first fleet of agents with one memory layer under them. Ulpia is the
library; Vesta is its librarian, the orchestrator that routes what arrives. Read
[`README.md`](README.md) for what is here and how to run it.

## If you are working on this repository

The system is in `tools/`. `tools/kb` is the memory layer and has one dependency;
`tools/tray` is a Windows tray app over the same library. Start at
`tools/kb/src/memory.rs`, which is the contract every surface goes through.

Run the tests before believing anything: `cargo test` in `tools/kb`.

## Which agent answers is not decided here

**It is decided before you read this file, by `kb boot` on a `UserPromptSubmit` hook.**
The router scores the message across every base, picks the owner, and the runtime injects
that agent's constitution into the conversation. You are handed an identity; you do not
choose one by reading a conditional.

This paragraph used to be that conditional, and it named one agent literally, so a
question about another agent's subject still woke the architect. Setup and reasoning:
[`decisions/0022-the-fleet-boots-the-agent.md`](decisions/0022-the-fleet-boots-the-agent.md).

If nothing was injected, the hook is not installed or the router abstained. In that case
you are the librarian and not one of the agents: say the question has no clear owner
rather than picking one.

## Committing, when more than one session is writing

**More than one agent session writes these repositories at the same time. Assume another
one is mid edit right now.**

```
kb commit <path>... -m "message"
```

Name every path. There is deliberately no flag meaning everything, and `git add -A` is how a
commit in the private repository, `cdc0e52`, came to contain two sessions'
unrelated work under one message. A raw
`git commit` is refused by a hook.

The mechanism, because the rule is useless without it: `git commit -- <paths>` builds the
commit from only those paths and ignores the rest of the index, so whatever another session
staged one second ago cannot land in yours. `kb commit` does that, then reads the commit
back and prints what it left dirty, which is the step a person skips by hand. Full reasoning
in [`decisions/0021-committing-under-concurrency.md`](decisions/0021-committing-under-concurrency.md).

New clone: `git config core.hooksPath .githooks`, once, per repository.

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
