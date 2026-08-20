# ADR-0009: the GUI is a client of a contract, and the runtime is a choice inside it

**Search for:** `GUI`, `Tauri`, `API contract`, `subscription`, `API key`, `runtime`, `prompt caching`

- **Date:** 2026-08-16
- **Status:** accepted for the stack, proposed for the contract
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** the framework is reversible. The contract is the part worth getting right, because
  everything written against it inherits its shape.

## Context

Richard chose Tauri, asked for both a local model and a frontier model as selectable options, and
deferred voice to a second phase with the seam left in place.

He then asked the question that actually decides the architecture: **can the app use his Claude
subscription, or only an API key?** That is not a billing detail. It determines whether our software
is the thing that talks to the model, or the thing that talks to something else that talks to the
model, and those are different products.

## What was found about the subscription

**Verified, read in the Anthropic API documentation this session:** credentials for the SDKs and the
`ant` CLI resolve in a fixed order, `ANTHROPIC_API_KEY`, then `ANTHROPIC_AUTH_TOKEN`, then an OAuth
profile created by `ant auth login`, then workload identity federation. All of those are **API account**
credentials, metered per token. The documentation also notes that Claude Code carries **its own login
credential, separate from the `ant auth login` profile**, and warns about the conflict between them.

**Not verified this session, stated from knowledge and not tested:** that Claude Code's own login is
what authenticates against a Pro or Max subscription. I did not run it and did not read the terms today.

The load bearing conclusion holds either way:

> **There is no mechanism by which a third party application holds a consumer subscription credential
> and spends its quota.** Subscriptions authenticate first party clients. An application that wanted to
> spend one would have to impersonate a client or lift a session cookie, which is against the terms and
> breaks the first time authentication changes.

So the subscription is not something our app can hold. It is something a client the user already owns
can hold, and that reframes the design rather than blocking it.

## Decision

**The GUI is not a program with a model inside it. It is a client of an API contract, and the runtime
is one field in that contract.**

Tauri is confirmed as the stack: Rust backend, TypeScript frontend, system WebView, `kb` linked as a
library instead of shelled out to and parsed. It produces an installer, which is the only distribution
that works for someone who will not open a terminal. It is roughly 10 MB against Electron's 150, on a
machine where the model already wants 4 to 5 GB and memory bandwidth is the measured bottleneck.

But **the boundary is the API, not the framework**, and that is the part that makes [[0008-single-user-open-source]]
a design rather than a sentence. One frontend, one contract, three implementations of it: the Tauri
backend locally today, a server later for the hosted service, and nothing in the frontend that knows
which one it is talking to.

### Three runtimes, and what each one costs

| Runtime | Who holds the credential | Who pays | What we give up |
|---|---|---|---|
| **Local** (llama.cpp, Qwen3.5 4B) | Nobody. There is no credential | Electricity | Capability. Measured at ~5.9 tokens/s generation on this machine |
| **Frontier by API key** | Our app, in the OS keychain | Per token, metered, unbounded | Nothing architecturally. It is the only path where our code calls Anthropic |
| **Third party agent client** | The user's own client, which we never see | The user's subscription, flat | The loop. We hand a prompt to someone else's harness and get text back |

The third row is the answer to his question. If the Tauri backend spawns a locally installed agent
client that the user authenticated themselves, the credential never enters our process, and the
subscription pays. We are not reselling inference, we are driving a program the user already runs.
What we lose is control of the agent loop, which for a memory system is a smaller loss than it sounds:
retrieval happens before the prompt and the write proposal happens after the answer, so both stay ours.

### What prompt caching does to the API key path, and why blocks.txt pays twice

`blocks.txt` orders the constitution by stability, most stable first, because prefix caching reuses the
KV state of a prompt only up to the first differing token. That was written for the local model. **The
same file, unchanged, is what makes the frontier path affordable**, because the Anthropic API caches on
exactly the same prefix rule.

Measured resident set for Zed is 14,714 tokens, from `kb blocks`. At Opus 5 rates, $5 per million in
and $25 out, with a question and its retrieved chunks at roughly 3,000 tokens and an answer at 1,500:

| | Uncached | Constitution cached |
|---|---|---|
| Constitution | $0.0736 | $0.0074 |
| Question and chunks | $0.0150 | $0.0150 |
| Answer | $0.0375 | $0.0375 |
| **Per question** | **$0.126** | **$0.060** |

A little over half, from a design decision already made for another reason. The cache write costs
$0.092 once. Sonnet 5 lands near $0.036 per question cached, Haiku 4.5 near $0.012, which is the
frontier end of the same cascade the local model sits at the bottom of.

**The contract must therefore keep the constitution stable and put everything volatile after it.**
Retrieved chunks, the question, the session state, all after the breakpoint. That is a requirement on
the contract, not an optimisation to add later, because getting it backwards costs the full price on
every single question and produces no error to notice.

### The voice seam, built now, implemented later

Two things, both free today:

1. The contract's message content is a **list of typed parts** from the first version,
   `[{type: "text" | "audio" | "image", ...}]`, even while only `text` is implemented. Adding a variant
   to a list is additive. Changing a string field into a list later is a break in every client.
2. STT and TTS sit behind one trait in the Rust backend with a single no-op implementation.

That is the whole bed. It costs one type definition and one empty trait.

### What we deliberately do not build

No account system, no proxy that holds anyone's token, no abstraction over model providers written
before there are two of them in use, and no server. Per [[0008-single-user-open-source]], the honest way
to keep the hosted service possible is to keep the local version clean.

## The contract, decided 2026-08-16

The contract is **a Rust API first**, `kb::memory::Memory`: three verbs over a set of bases, and the
one place an answer is computed. MCP, the GUI and any future HTTP service are wrappers over it, so they
cannot answer differently.

That is not stylistic. `mcp.rs` was rebuilding the pipeline itself, and this codebase has already
shipped two bugs of exactly that shape: alias expansion reaching one scorer and not the other, and the
two oversampling factors drifting apart. A second caller assembling the pipeline is a second chance at
both.

### The GUI links the library rather than speaking MCP to itself

Richard pushed back on the first recommendation, which was for the GUI to be an MCP client, and he was
right. **Latency was measured and is not the criterion:**

| | median | p95 | reply |
|---|---|---|---|
| `ping`, the protocol floor | **0.031 ms** | 0.077 ms | 44 B |
| `kb_retrieve` | 4.43 ms | 5.65 ms | 14.8 KB |

The protocol costs 31 microseconds, including serialising and moving 14.8 KB. That is 0.7% of a
retrieval, and against the measured local model (~5.9 tokens/s) the same question costs around 100
seconds of inference: a ratio near one to three million.

What actually decides it is three things latency hid:

1. **Process lifecycle is where the bugs live.** An MCP client must spawn, monitor, restart and reap a
   subprocess. On Windows that means orphaned children when the parent dies, zombies and handle leaks.
   A library call has none of it.
2. **The divergence risk was overstated.** Both paths call `retrieve::fuse`, so retrieval cannot
   diverge. Only formatting and argument defaults can.
3. **The local model probably should not use tool calling at all.** A 4B model calling tools reliably
   is a stretch, so the local path retrieves first and puts passages in the prompt. The GUI's two
   runtimes therefore differ from each other regardless, which is most of what "one code path" was
   supposed to buy.

An MCP **client** gets written when there is a third party server actually worth using, for that
purpose. Until then it is complexity bought on speculation, which the project's quality bar protocol names as the
same failure as a lazy shortcut pointed the other way.

### Which runtime answers, decided 2026-08-16

Richard's policy, and it is the right default for a reason worth stating: **the most capable model
first, the local one as the fallback, and the user told when it happens.**

| Condition | Runtime |
|---|---|
| Network up, API answering | Frontier. The default |
| No network, or the API is failing | Local, **and a notification says so** |
| User chose local only | Local, always. An option, never the default |

The notification is the load bearing part. A system that silently degrades to a weaker model produces
worse answers with no visible cause, and the user concludes the product is bad rather than that it is
offline. Falling back is fine; falling back quietly is not.

Note this inverts the usual privacy-first framing, deliberately. The local model exists here for
capability continuity rather than as the preferred path, and the user who wants the privacy property
opts into it explicitly. That is Richard's call and it is coherent: he already accepted, in
[[0010-memory-as-mcp-server]], that passages travel when a cloud model reasons.

### A fleet root is accepted, never required

A path handed to `Memory::open` may be a base or a **fleet root**: a directory that is not itself a
base but whose immediate children are. Both work, and neither is privileged.

Requiring an arrangement would be an assumption about the user's filesystem, which
[[0008-single-user-open-source]] forbids in as many words: the base is addressed by path, never
assumed. Accepting one means a tidy layout is a convenience the user may adopt rather than a shape we
impose, and it means moving the three agent folders under one parent is optional tidying instead of a
migration. Verified by pointing the server once at the machine's Desktop directory, which is already a fleet
root by this definition, and watching it find all three agents.

## Consequences

- **The frontend never learns which runtime answered.** That is what makes the hosted service a second
  implementation instead of a rewrite, and it is also what makes the local and frontier switch a
  dropdown rather than a fork in the code.
- **A frontier API key lives in the OS keychain and nowhere else**, per the standing security constraint.
  Never in a file, tracked or gitignored, never in the base.
- **The third party client path means we depend on something we do not ship.** If it is not installed
  or not logged in, that runtime is simply unavailable and the app has to say so plainly rather than
  fail into a confusing error.
- **Cost becomes visible to the user, because it is theirs.** The API key path should show tokens and
  money per question. A metered path that hides its meter is how people get surprised.

## Revisit trigger

- Anthropic publishing an actual mechanism for a third party application to consume a subscription,
  which would collapse rows two and three of the runtime table into one.
- The first non Anthropic frontier provider being used in earnest, which is when a provider abstraction
  stops being speculative complexity and starts being justified.
- Voice moving from phase two to now, which is when the seam gets tested rather than assumed.
