---
title: "Why In-Process Memory Beats Resident Daemons: Lessons from the AI Memory Frontier"
date: 2026-09-16
description: Exploring the architectural tradeoffs in AI agent memory: why in-process SQLite FTS5 beats resident server daemons, how filesystem boundaries prevent multi-tenant leaks, and what is coming next in Ulpia.
lang: en
---

When software engineers set out to solve the long-term memory problem for AI coding agents, the default architectural reflex is almost universal: build a resident background daemon.

You create a persistent HTTP or stdio server listening on `127.0.0.1:49374`. You add a local SQLite database, register a suite of Model Context Protocol (MCP) tools, hook lifecycle events from your favourite CLI agents (Claude Code, Cursor, Codex, OpenCode), and perhaps wrap everything in Docker or a systemd service. On paper, it feels like sensible client-server engineering.

In real-world developer workflows, however, persistent daemons introduce subtle failure modes that compound quickly.

Over the past week, watching the wider agent memory ecosystem evolve - including discussions and issue reports across community projects like `ai-memory` - has brought into sharp focus just how many problems disappear when you step back and make a different architectural choice: **in-process, file-backed, deterministic memory with zero resident daemons**.

Here is what we have learned from the AI memory frontier, why Ulpia's in-process engine avoids common production traps, and the improvements we are rolling out next.

---

## 1. The Daemon Trap: Port Contention and Flaky Tests

When your memory system runs as a background daemon, running tests requires spawning actual server child processes, allocating TCP ports, and synchronizing on network listeners.

Under concurrent execution (such as running test suites across multiple cores or running automated CI on macOS), daemons quickly become flaky:
- Default ports like `127.0.0.1:49374` collide when multiple test runners execute in parallel.
- Dynamically assigned ports (`127.0.0.1:0`) require polling mechanisms and startup budgets that slow down test execution.
- Handling process interrupts (`SIGINT`, `SIGTERM`) across different operating systems produces orphaned socket listeners and zombie processes.

In community projects, developers frequently report test suites that fail when run in parallel (`110 passed, 8 failed`) but pass when isolated to a single thread (`--test-threads=1`).

**How Ulpia does it**:
Ulpia's core engine (`tools/kb`) is an in-process Rust library and self-contained binary. When an agent boots or queries memory, SQLite FTS5 runs directly inside the host process. There are no listening sockets, no HTTP ports, and no resident daemons consuming background RAM. 

Because there is zero port contention, Ulpia's entire test suite of 484 unit and integration tests executes concurrently across all CPU cores in approximately 3.2 seconds. Clean, deterministic, and impossible to flake on port conflicts.

---

## 2. The Tenancy Trap: Directory Basenames vs Git Roots

Another common temptation in agent memory servers is adding multi-user authentication (such as OIDC or human login tokens) without building strict resource authorization.

In shared server setups, a critical vulnerability emerges when project identities are derived from superficial filesystem folder names (like directory `basename`). If developer Alice checks out a private repository into a directory called `api/` and records confidential contract terms, and developer Bob checks out an entirely unrelated project also named `api/`, both projects collide on the same database identifier. Because the server lacks granular per-project Access Control Lists (ACLs), Bob can execute a search query and retrieve Alice's confidential notes verbatim.

**How Ulpia does it**:
Ulpia does not rely on a shared multi-tenant central daemon. Instead, it respects the developer's local filesystem and Git topology:
- **Repository-anchored identity**: Project scopes are determined by Git repository roots (`git rev-parse --show-toplevel` and remote tracking URLs), never by arbitrary folder names.
- **Explicit private layers**: In Ulpia's architecture (defined in ADR-0034 and `base.rs`), sensitive directories such as `profile/`, `projects/`, and `records/` are governed by a strict `private_layer` declaration. Private notes are never indexed into the public search table and are never served unless the local human operator explicitly supplies the `--all` flag.

By keeping memory local and tied to filesystem permissions, you eliminate an entire class of cross-tenant data leaks.

---

## 3. The Token Tax: Pure BM25 vs Remote Embedding Bills

Many agent memory tools treat vector embeddings as a mandatory prerequisite. Every session end, prompt, and tool call is sent to a remote embedding API (OpenAI `text-embedding-3`, Cohere, or Gemini) to generate high-dimensional vectors.

This approach creates three operational bottlenecks:
1. **Network latency**: Every memory write and lookup requires round-trip HTTPS calls.
2. **Fragile dependencies**: When API quotas expire or cloud providers experience minor downtime, agent memory stops working.
3. **Continuous costs**: Developers frequently seek workarounds, such as attempting to piggyback on GitHub Copilot subscription tokens, simply to avoid paying for embedding APIs.

**How Ulpia does it**:
Ulpia's retrieval engine uses SQLite FTS5 with custom BM25 scoring and Small-to-Big section windowing. It operates directly over plain Markdown files with zero external API calls:
- File names, headings, keyword tags, and prose density are scored in microseconds.
- Contiguous passage hits are merged up to a strict window (1,800 characters / ~450 tokens), giving the reading model complete paragraph context without context bloat.
- It is 100% offline and air-gapped out of the box. Zero API keys, zero token fees, zero cloud dependencies.

---

## 4. MCP Schema Fragility: Flat Types vs Recursive Defs

The Model Context Protocol (MCP) has become the standard way coding agents interact with tools. However, different LLM frontends parse JSON schemas with varying levels of strictness:
- Google Gemini and Vertex AI enforce strict dialect expectations.
- Anthropic Claude Code handles flexible schemas.
- Strict constrained-decoding runtimes (such as Moonshot/Kimi) fail with HTTP 400 (`infinite recursion without termination condition`) when a tool schema uses nested `$defs` or complex `oneOf` unions without a top-level `type: string`.

In the wider ecosystem, developers have had to invent dialect flags like `--flavor gemini` or `--flavor moonshot` to stop their MCP tools from crashing strict parsers.

**How Ulpia does it**:
In `tools/kb/src/mcp.rs`, Ulpia constructs tool definitions with a clean, flat schema generator:

```rust
fn tool(name: &str, description: &str, args: Vec<(&str, &str, &str, bool)>) -> Value {
    let mut props = Value::obj();
    let mut required = Vec::new();
    for (arg, ty, desc, req) in args {
        let mut p = Value::obj();
        p.set("type", ty.into());
        p.set("description", desc.into());
        props.set(arg, p);
        if req {
            required.push(Value::Str(arg.to_string()));
        }
    }
    let mut schema = Value::obj();
    schema.set("type", "object".into());
    schema.set("properties", props);
    schema.set("required", Value::Arr(required));
    // ...
}
```

Every parameter explicitly declares its primitive type (`string`, `integer`, `boolean`), the root is always declared as `type: object`, and recursive definitions are completely avoided. As a result, Ulpia's MCP server works out of the box with Vertex, Gemini, Claude, and Moonshot without requiring schema dialect negotiation.

---

## What We Are Improving Next

Learning from the broader community's experiences also highlights opportunities to make Ulpia even better. Here are the three enhancements currently in development:

### 1. Formalizing `DATA_HANDLING.md` (Enterprise Air-Gap Readiness)
Enterprise and corporate security teams (particularly in Europe and Germany) increasingly require formal documentation before allowing developer tools onto employee workstations. We are documenting Ulpia's privacy guarantees in a dedicated `DATA_HANDLING.md`:
- Explicit confirmation that zero data ever leaves the local machine.
- Confirmation of zero background telemetry, zero usage tracking, and zero hidden analytics.
- Full verification of offline, air-gapped installation and operation.

### 2. Baton Claim Semantics in `kb handoff`
Currently, `kb handoff` formats the state of a task (goals, decisions, open questions, and next steps) so that another agent session can resume where the previous one left off. To make multi-agent collaboration even more robust, we are introducing explicit claim lifecycle states (`Pending` -> `Claimed` -> `Completed`), preventing two parallel agent sessions from attempting to work the same handoff simultaneously.

### 3. Sanitized Assistant Turn Capture in `kb capture`
Ulpia's `kb capture` currently records unmapped questions, routing decisions, and abstentions to detect knowledge gaps. By extending capture hooks to optionally record sanitized final assistant conclusions, our multi-session consolidation tool (`kb consolidate`) will be able to synthesize not only what questions were missed, but which architectural solutions proved successful over time.

---

## Conclusion: Boring Infrastructure is Reliable Infrastructure

Memory for AI agents does not need to be an elaborate network service or a fragile cloud pipeline. When you build on top of plain Markdown files, local SQLite FTS5, and in-process execution, your memory layer becomes as unexciting and dependable as `git`.

No background daemons to keep alive, no network ports to fight over, no token bills to monitor, and no risk of cross-tenant data leakage. Just fast, dependable, local-first memory that works whenever your terminal opens.
