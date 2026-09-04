---
provenance: agent
stage: derived
---

# ADR-0022: the fleet boots the agent, the agent does not boot itself

**Search for:** `boot`, `kb boot`, `bootar`, `inicializacao`, `startup`, `hook`, `gancho`, `UserPromptSubmit`, `SessionStart`, `UserPromptExpansion`, `settings.json`, `claude/settings.json`, `stdin`, `stdout`, `exit 0`, `exit 2`, `fail open`, `context injection`, `injecao de contexto`, `constituicao`, `constitution`, `agent identity`, `identidade do agente`, `identidade da sessao`, `agente ativo`, `roster`, `dono`, `owner`, `latencia`, `latency`, `lentidao`, `355 ms`, `280 ms`, `60 ms`, `sqlite`, `Base::discover`, `resident server`, `servidor residente`, `kb serve`, `mcp`, `kb_boot`, `mcp tool`, `model discretion`, `claude.md`, `55 kb`, `session id`, `kb/sessions`, `re-route`, `topic switch`, `troca de assunto`, `tracked`, `camada privada`, `private layer`, `runtime`, `vendor hook`, `fallback`, `instalar hook`, `hook falhou`, `sem constituicao`, `adr-0022`

**Exists to:** How a session is told which agent it is: the UserPromptSubmit hook running kb boot, the 355 ms it costs, and why the model is not asked to decide.

- **Date:** 2026-08-18
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. One hook registration and one subcommand.

## Context

[[0020-vesta-routes-to-the-agent]] decided that routing to an agent beats a fixed choice by
six questions in nineteen, and deliberately left the boot mechanism open.

Richard closed it, and the framing is the decision: **a runtime gives us a model, and how
that model runs in the workspace is our software's call, not the model's.** The comparison
he drew is to any system you authenticate into and are then handed tools by. You do not
hand the integration a document and ask it to work out what it is.

Measured against that, the old mechanism fails on principle rather than on accuracy. The
public `CLAUDE.md` carried a static conditional saying to read the architect's file and
follow it instead. It names one agent literally, never reads the message, and leaves the
identity decision to whichever model happened to parse the sentence. That is the model
deciding, dressed as configuration.

## Options

### Option A: `CLAUDE.md` tells the model to call the router

Keep the file as the entry point, replace the hardcoded name with an instruction to run
`kb route` first and adopt the winner.

- Cost: nothing to build.
- Failure mode: it is still the model choosing whether to obey, on every message, forever.
  An instruction that has to be followed a thousand times gets followed nine hundred. This
  repository has already paid for the difference between a rule in prose and a rule in
  code, twice: the em dash rule moved into `kb check`, and the commit rule into `kb commit`
  after a rule in prose failed to prevent `cdc0e52`.
- Forecloses: nothing.

### Option B: the runtime runs the router before the model sees the message

A `UserPromptSubmit` hook runs `kb boot`, which routes and prints; the runtime injects that
output as context.

- Cost: one subcommand, one hook registration, and **355 ms of latency on every message.**
- Failure mode: the hook belongs to somebody else's runtime and its payload can change on a
  version bump. Mitigated by failing open: every error path prints nothing and exits 0, so
  a broken router degrades to today's behaviour instead of eating the message.
- Forecloses: nothing.

### Option C: an MCP tool the model calls

`kb serve` already runs. Add a `kb_boot` tool.

- Cost: nothing, the transport exists.
- Failure mode: **the same one as Option A wearing a better costume.** A tool is still
  invoked at the model's discretion. MCP has no way for a server to push identity into a
  turn the model did not initiate.
- Forecloses: nothing.

## Decision

**Option B.** The mechanism, verified against the hook reference on 2026-08-18: for
`UserPromptSubmit`, `UserPromptExpansion` and `SessionStart`, and for no other event, the
runtime adds the command's stdout to the model's context. Exit 0 means the context is
added. That is the only surface in this runtime where our software speaks before the model
does, and it is exactly the plug Richard described.

`kb boot` reads the payload on stdin, routes with `Memory::ask`, and:

1. **Above the ADR-0020 gate**, emits the winning agent's assembled constitution and the
   files to open, with a line stating the identity was decided by the fleet and is not the
   model's to override.
2. **Below the gate**, emits the roster and says no owner was found. It does not pick.
   A router that always picks is the failure ADR-0013 spent a day measuring.
3. **Emits the constitution only when the routed agent changes**, tracked per session id
   under `.kb/sessions/`. It is roughly 55 KB; injecting it every turn would make the
   router the most expensive thing in the loop. This is also how Option B's named failure
   mode in ADR-0020, a conversation changing domain at message three, gets handled: it
   re-routes and re-boots for the price of one file read.

**It always exits 0.** On this event exit 2 blocks the prompt and erases it, and exit 1
shows the user a hook error. Neither is acceptable for a routing step that failed: the
message belongs to the user and must reach the model whatever the router thinks.

**It reads only what git tracks.** The private layer stays out of the injected context by
the same mechanism that keeps it out of the MCP server, rather than by a second rule that
could disagree.

## Consequences

- **355 ms is added to every message**, measured on the release binary. Isolated by
  experiment rather than guessed: bare process start is about 60 ms, opening the five bases
  is about 280 ms, and the routing itself is about 5 ms. So the cost is `Base::discover`
  shelling out to git once per base plus opening five SQLite stores, and **Z29's O(entries)
  router is not what makes this slow.** The fix, when it is worth doing, is a resident
  server rather than a faster router.
- The identity a session runs under is now auditable: it came from a scored decision over
  the bases, not from a sentence somebody wrote.
- `CLAUDE.md` stops naming an agent. It now says who decides and what to do when nothing
  was injected.
- A second runtime that has no equivalent hook falls back to reading `CLAUDE.md`, which is
  why that file still explains the situation instead of being emptied.

## Revisit trigger

- **The hook payload changing shape.** It is another vendor's contract. `kb boot` fails
  open, so the symptom will be silence rather than an error: the check is that a session
  stops being told who it is.
- **355 ms becoming annoying**, which is the trigger for making `kb serve` hold the open
  bases and having the hook talk to it instead of opening them again per message.
- **The router picking wrong in real use**, as opposed to on the gold set that Zed wrote.
  ADR-0020 already carries that caveat and this makes it load bearing: a routing error now
  changes who is answering rather than which file gets read first.
- A second model or runtime in the fleet, where "the software decides how the model runs"
  needs a transport that is not one vendor's hook.

## Notes

The hook is registered in `.claude/settings.json`, which is tracked, so it travels with the
repository. The private repository needs no registration of its own: the workspace is the
public root and the router reaches every base under it.

**That sentence was half the story until 2026-09-04, and the missing half was a defect in
every clone.** The registration travelled and the thing it pointed at did not: the command
named `tools/kb/target/release/kb.exe`, and `target/` is gitignored, with zero files under
`tools/kb/target` in the repository. So a cold clone ran a `UserPromptSubmit` hook whose
command did not exist, on every prompt, from the first message. The same line ended in
`.exe`, which is wrong on every Linux and macOS clone even after a successful build, since
cargo writes that suffix only on Windows.

Found by a `kb panel` round on this very record, in Cicero's objection, and verified with
`git check-ignore` and `git ls-files` before anything was changed. The fix is
`.claude/hooks/boot.sh`: it resolves the binary per platform, honours `KB_BIN`, and exits 0
with no output when there is no build or no fleet. Silence is the correct failure here and
the repository already knew it, in `promote-on-idle.sh`, which has guarded exactly this way
since it was written. That guard had simply never been applied to the hook that runs most
often. What a clone gets now is what this document already promised it would get when the
hook is absent: no injection, and a session that says the question has no clear owner.
