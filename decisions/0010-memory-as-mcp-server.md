# ADR-0010: the memory system ships as an MCP server, so our GUI stops being the only door

**Search for:** `MCP`, `MCP server`, `tool surface`, `third party client`, `distribution`, `wedge`

- **Date:** 2026-08-16
- **Status:** proposed
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** adding the server is reversible. **The tool surface it publishes is close to a one
  way door**, because other people's clients bind to the names and shapes, and renaming a tool later
  breaks configurations we cannot see.

## Context

Richard asked whether someone could use Claude or ChatGPT as the interface while running our backend,
our routing and our memory. A third party's runtime with our memory.

The question was asked as an aside about the GUI. It is the most consequential question in the whole
GUI conversation, because the answer changes what the GUI is for.

## Decision

**`kb` gains an MCP server mode. The memory system becomes something any MCP capable client can call,
and the GUI becomes the good door rather than the only door.**

The Model Context Protocol is how an agent client discovers and calls tools that live outside it. A
client launches the server as a local process, reads its tool list, and calls the tools during a
conversation. Nothing is hosted, nothing is proxied, and no credential crosses a boundary.

**Verified:** MCP is a documented, current protocol and the Anthropic API has a first party connector
for remote MCP servers. **Not verified this session:** the exact current MCP support in each competing
desktop client. Claude Code and Claude Desktop are the ones I would build against first, and the
protocol being open is what makes the rest a matter of time rather than a matter of permission.

### The tool surface

`kb` already is these functions. The server is a thin process over them, not new logic.

| Tool | Maps to | Returns |
|---|---|---|
| `kb_route(question)` | `kb route` | Ranked files, with the words that matched, so a bad ranking is diagnosable |
| `kb_retrieve(question)` | `kb route --hybrid`, **plus the chunk text, which it does not return yet** | The passages themselves, with `heading_path` and provenance |
| `kb_remember(claim)` | `kb remember` | The proposal, ADD, UPDATE or NOOP, with the containment evidence. It writes nothing |
| `kb_write(claim, path, reason)` | new | Performs the write and commits, with `reason` in the commit message |

Two constraints on that surface, both structural rather than advisory:

1. **`reason` is a required parameter on `kb_write`.** [[0007-memory-architecture]] made the constraint
   on writing and deleting **disclosure rather than permission**. A required schema field is disclosure
   enforced by the protocol, so it cannot be skipped by an agent in a hurry.
2. **The server operates only on the base path it was launched with, never on a path from a tool call.**
   Tool arguments arrive from a model, which means they can arrive from anything the model read. A
   server that accepts a destination path from its caller is a file write primitive for whatever text
   the agent happened to ingest.

### Why this matters more than it looks

**It answers the subscription problem sideways.** [[0009-gui-runtime-boundary]] establishes that no
third party application can spend a user's subscription. An MCP server does not need to: the user's own
client makes the model call, on their own subscription, and simply calls our tools along the way. The
credential never comes near us because it never needs to.

**It is the distribution wedge, and it is far smaller than an app.** Asking someone to install an
application is a large ask. Asking them to add a few lines to a config file so their existing agent can
search their own notes is a small one. The open source release from [[0008-single-user-open-source]]
becomes something people try in ten minutes instead of something they evaluate.

**It forces the tool surface to be honest.** A CLI can be forgiving because the person driving it wrote
the base. A tool surface is called by a model that has never seen the folder, so every ambiguity in
naming, every unexplained ranking, every silent empty result becomes a visible failure. That pressure
is good for the design and there is no way to get it without exposing the surface.

### What the GUI is for, once this exists

Not the chat. The chat is the part MCP already gives away. What the GUI adds is everything MCP is the
wrong shape for:

- **Managing the local model.** Downloading, quantisation choice, offloading, the measurements.
- **The write review loop.** `kb remember` produces a proposal; seeing it as a diff and approving it is
  a UI problem, and it is the moment where the base's quality is actually decided.
- **Provenance and history made visible.** Who wrote this, when, from what, and the git history behind
  it. This is the answer to "why does the agent believe that", and a terminal is a poor place for it.
- **Agent switching**, and later voice.

That list is shorter and sharper than "a chat app with memory", and it is a better product for it.

### The alternative considered

**Build only the GUI and let the memory stay behind it.** Cheaper now, and it keeps every user inside
our application, which is the shape a company would choose if the goal were lock in. It is rejected for
the reason in [[0008-single-user-open-source]]: the paid product is convenience, not capability. A
memory system that only works inside our app is one we would be defending rather than improving.

## What was verified before building, and what it changed

Three findings, in descending order of how much they changed the plan.

### 1. The private layer was already leaking, and it was reproduced

The gate lived on the file walk only. `Base::discover(root, all)` filtered `base.files`;
`Store::search` had no filter of any kind, and the `files` table had no column recording which flag
built it. `fuse` made it worse by using `or_insert_with`, an insert rather than a lookup, so a private
file the keyword scorer had correctly excluded was **put back** by the text side.

Reproduced on this machine, printing paths only and never content:

```
kb index ../../ --db <scratch> --all
kb route "..." ../../ --hybrid --db <scratch>     # no --all
  -> profile/<a private file>
kb remember "..." ../../ --db <scratch>           # no --all
  -> profile/<a private file>
  -> records/sessions/<a private file>
```

**One `--all` at index time poisoned the database permanently.** Every later query returned private
files whether or not it asked for them, and nothing in the file recorded that it had happened. The
live `.kb/index.db` happened to be clean, which was sequence luck rather than design.

Fixed by making the flag travel with the file into the index: `MdFile.tracked: Option<bool>`, a
`tracked` column on `files`, and `Store::search(terms, limit, scope)` where `Scope` has no default and
must be named. `NULL` is a third state meaning git could not be asked, and it is not folded into
either answer, because a base outside a git repository has no private layer to protect and folding
`NULL` into "private" would make the router silently return nothing.

An index written before the column existed **cannot** say which of its rows came from an `--all` run,
so opening one empties it and says so. Emptying it silently would have the router answer "nothing
matched", which reads as "the base does not cover this" and is a more expensive wrong lesson than an
error.

Four regression tests, and they are the first tests in the project that touch the database at all.

**This is why the server's default is opt-in and not a preference.** The leak proves the failure mode
is real rather than theoretical, and it was on exactly the surface about to be handed to a cloud model.

### 2. The protocol removed its handshake, and stdout belongs to it

The current revision is **2026-07-28**, which is "stateless": the `initialize` / `notifications/initialized`
handshake is gone, replaced by per-request `_meta` carrying `io.modelcontextprotocol/protocolVersion`.
The spec names the two eras, **modern** (2026-07-28 and later) and **legacy** (2025-11-25 and earlier,
with the handshake), and names an implementation speaking both: **dual-era**. We build dual-era, so the
server does not depend on which revision a given client speaks.

Framing is newline-delimited JSON, not LSP `Content-Length`. And, verbatim: *"The server MUST NOT write
anything to its `stdout` that is not a valid MCP message."* Every `cmd_*` in `kb` prints to stdout
today, so the serve path needs its diagnostics on stderr or it corrupts its own protocol. That is the
most likely first bug and it is now written down.

### 3. claude.ai in a browser cannot reach a local server

Verbatim from Anthropic's support documentation: *"Local MCP servers configured in Claude Desktop via
`claude_desktop_config.json` are a separate mechanism and do use your local network, but those aren't
available in Cowork or claude.ai."* And the reason: *"the connection to your MCP server originates from
Anthropic's servers, not from your machine's network interface."*

So the reachable surface is **Claude Code and Claude Desktop**, both of which run on the user's machine
and can spawn a subprocess. A browser cannot, by design. The MCP tunnels feature does not rescue this:
tunnels are not available as connectors in claude.ai.

### What a tool returns still travels

Confirmed verbatim in Anthropic's own documentation: *"Tool inputs and outputs still flow to Anthropic's
control plane (where Claude runs) so the model can see results."* Retention on consumer plans is
documented as five years with the training setting on and thirty days with it off.

**The store is local; what a tool returns is not.** That asymmetry is the reason the private layer is
opt-in, and it is the honest sentence to put in the README rather than "everything stays local".

## Decisions taken

| Question | Decision | Why |
|---|---|---|
| Private layer in the server | **Out by default, explicit opt-in** via a launch flag visible in the client's config | Turning it on becomes an act with a record, rather than a silent behaviour |
| JSON parsing | **Hand written**, keeping the one dependency | Richard's call, against the ADR-0007 standard |
| Pre-parse accent and quote stripping | **Not implemented**, deliberately | It solves nothing a correct parser needs (`\"`, `\\`, `\uXXXX`, surrogates are the hard part) and destroys the framing, since a quote inside a JSON string arrives as `\"`. On the `remember` path the claim becomes a file, so folding accents would corrupt the base permanently and silently. The same normalisation already exists one layer down and in the right place: FTS5 indexes with `remove_diacritics 2`, applied to search terms after parsing, never to text on its way to disk |

## Consequences

- **The tool names and their shapes become public surface.** They deserve the same care as an ADR and
  should not be renamed casually. This is the part of the decision that is hard to reverse.
- **`kb_retrieve` requires work `kb` has not done**: `route` currently returns files, and a model
  calling over MCP needs the passages. That is a small, concrete next task rather than a design problem.
- **A write tool over MCP is a real security surface.** The path confinement above is not optional, and
  it should have a test that tries to escape the base.
- **We inherit a compatibility obligation to clients we do not ship.** When a client changes its MCP
  behaviour, that becomes our bug report to answer.

## Revisit trigger

- The first outside user configuring the server, which turns the tool surface from a design into a
  contract with someone.
- Any tool that turns out to need a path argument from the caller, which would put the confinement rule
  under real pressure and deserves its own decision rather than a quiet exception.
