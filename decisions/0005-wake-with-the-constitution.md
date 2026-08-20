# ADR-0005: the agent wakes with its constitution, not with its library

**Search for:** `lines with no model at all`

- **Date:** 2026-08-13
- **Status:** proposed
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible, and cheap to reverse while nothing is built

## Context

Richard's proposal, 2026-08-13: maybe read the whole base **once**, at startup. Turn the system on,
the orchestrator wakes the agents, each one reads its entire codebase and understands how it acts and
what knowledge it has. Then an input arrives, the orchestrator routes it to the right agent
intelligently, and **that agent does not have to read the whole base again**, it works by reference.

The instinct is right and the technique is real. It is called **prefix caching**: prefill produces a
key value cache, and if the beginning of the prompt is byte identical on the next call, the runtime
reuses that cache instead of recomputing it. Read once, reuse many. That is exactly the shape of the
idea.

It also has three costs, and the first one is measured in gigabytes on a machine with 15.7 GB shared
with the GPU and Windows.

## Cost 1: the KV cache is RAM, and it scales with what you keep resident

Bytes per token in the cache: `2 (K and V) × layers × kv_heads × head_dim × bytes_per_element`.

For a 4B class model with grouped query attention, roughly 36 layers, 8 KV heads, head dim 128, at
f16, that is about **144 KB per token**. An 8B class model lands near 128 KB per token, so the number
barely moves with model size: **what you keep resident costs the same whichever model holds it.**

| What stays resident | Tokens | KV at f16 | KV at q8 |
|---|---|---|---|
| Steve's whole boot path today | ~21,000 | 3.0 GB | 1.5 GB |
| Zed's whole boot path today | ~9,000 | 1.3 GB | 0.65 GB |
| A 4,000 token constitution | 4,000 | 0.58 GB | 0.29 GB |

Now multiply by three agents awake at once, which is the whole point of the orchestrator:

- Three full bases resident: **9 GB of cache** on top of the model, on a machine with 15.7 GB where
  Windows already holds 5 to 6. It does not fit.
- Three constitutions resident: **0.87 GB at q8.** It fits with room to spare.

These are calculated from published attention shapes, tier C, and the real numbers come from the
measurement plan in [[0004-local-first-inference]].

## Cost 2: a resident context taxes every token, not just the first

Prefix caching removes the cost of **recomputing** the prompt. It does not remove the cost of
**attending** to it. Every generated token attends over the whole resident context, so a model holding
21,000 tokens generates meaningfully slower than the same model holding 2,000, for every token of
every answer, forever.

So a big resident context is not paid once at startup. It is paid on every word the agent ever says.

## Cost 3: a small model with a full base in context does not actually know the base

Long context recall degrades, and it degrades hardest in small models and hardest in the middle of the
window. Putting 21,000 tokens of knowledge base in front of a 4B model does not give you an agent that
knows the base. It gives you an agent that has it nearby and finds some of it, unpredictably, with no
signal about which parts it missed.

This is the cost that matters most, because the other two are just performance. This one is
correctness, and it fails silently.

## The reframe

**The agent does not need to read the base in order to know what is in it.** An index knows what is in
it. Reading is what you do to the two or three files that turn out to matter.

That splits Richard's "wake up and read everything" into two things that were tangled together, and
both survive:

- **"Understand how I act"**: identity, method, rules, the bar, the limits, and a thin map of what
  exists. Small, stable, needed on literally every call. **This is what stays resident.**
- **"Know all the knowledge I have"**: the notes themselves. Large, growing, and only ever needed a few
  files at a time. **This gets indexed by code at startup and retrieved per query.**

The second half is the part Richard's proposal already had right: reading the whole base at startup is
correct, it is just **code** that should do the reading, not the model. Walking every file, parsing its
front matter and keywords and links, and building an index takes milliseconds, costs no RAM at
inference time, taxes no token, and cannot forget the middle of the document. `tools/kb` already walks
the files. Emitting the index is the next step it was always going to take.

**And the digest of what the base contains already exists: it is `MAP.md`.** Written by hand, by a
model with judgement, at a moment when there was time to think. It is a far better startup summary than
anything a 4B model would produce by skimming everything at boot. It just has to be thin enough to stay
resident, which is exactly the S1b work already in the backlog. That finding stopped being hygiene the
moment we decided to run locally.

## Decision

**Startup, once per session:**

1. `kb index` walks the files and refreshes the derived index. Code, not a model. Milliseconds.
2. Each agent's **constitution** is loaded as a cached prefix: boot file, operating instructions, and
   the thin map. KV cache quantized to q8.

**Per query:**

1. **Route** to the agent and to candidate files.
2. **Retrieve** the two to four files or sections that matter.
3. **Answer** from constitution (cached, free) plus retrieved chunks plus the question. Only the new
   part gets prefilled.

**On routing, and this is the part worth arguing about: start with no model at all.**

The `Search for:` line on every map entry is already a keyword index, written deliberately, by hand,
by whoever knew the file best. Scoring a query against those lines is a lookup: deterministic, instant,
free, debuggable, and it never hallucinates a file that does not exist. Put the small model in only
where the lookup is genuinely ambiguous, and log every time that happens, because that log is the list
of keyword lines that need fixing.

**Do not put a language model where a lookup table works.** It is the efficiency clause of the north
star applied to our own house, and the version with the model in it is slower, less predictable and
harder to debug for no gain we can name.

## The consequence that changes how we write from now on

**The boot path now has a price in gigabytes, and it is paid for the whole session.**

Proposed budget: **4,000 tokens per agent constitution**, roughly 16 KB. That is 0.29 GB of q8 cache
per agent and 0.87 GB for three awake at once.

> **Corrected 2026-08-13 by measurement.** The 144 KB per token this budget was built on assumed a
> dense attention model. The model we actually run costs **32 KB per token** by arithmetic and about
> 48 KB observed, because three of every four layers use linear attention whose state does not grow
> with context. Zed's real 8,259 token constitution therefore costs about 0.26 GB, not 1.19 GB.
>
> **So the budget is restated in memory rather than tokens: 1 GB of KV cache across the whole awake
> fleet.** A token count was the wrong unit, because the same text costs four times more or less
> depending on the model's attention architecture. See `local-inference-latitude-3420`.
>
> The budget still binds. It is just no longer the tightest constraint, and generation speed is.

Today Zed's is about 36 KB and Steve's about 85 KB, so both are over and both have obvious fat: Steve's
map still does routing and summarising at once (S1b), and Zed's `index.md` is generous with prose.

This is also a check `kb` can enforce, and it should: a constitution over budget is now a measurable
regression rather than a matter of taste.

## Consequences

- The retrieval layer becomes the load bearing piece. What it fails to surface does not exist as far
  as the answer is concerned, which is a real risk and the honest cost of this decision.
- A missing `Search for:` line stops being untidy and becomes a file the local model cannot reach.
  `kb` already reports it as W02.
- Notes must be answerable on their own, as recorded in [[0004-local-first-inference]].
- Editing a file mid session invalidates the cached prefix from that point on, so the constitution has
  to be stable during a session. Knowledge notes can change freely, since they are retrieved and not
  resident.

## Confirmed by measurement, 2026-08-13

The central bet was tested the same day, on the real constitution, and it held.

| Request | Tokens prefilled | Prefill time |
|---|---|---|
| First question of the session | 8,259 | 129,232 ms |
| Second question, same prefix | 518 | 10,664 ms |
| Third question, same prefix | 517 | 10,889 ms |

**12.1 times less wall clock**, because 94% of the prompt was reused rather than recomputed. Prefix
caching is not a theory here, it is a measured 129 seconds paid once instead of per question.

The attention tax on resident context was also real and modest: generation runs about 5.0 t/s at 2k of
context and about 4.3 t/s at 8k, roughly 15% slower. Worth knowing, not worth redesigning around.

## Revisit trigger

- The measured KV cost per token coming in far from 144 KB, which would move the budget.
- Routing by keyword lookup missing the right file often enough to be the limiting factor, which is
  when the small model earns its place.
- A machine with more RAM or a dedicated GPU, which changes the resident budget by an order of
  magnitude and makes most of this moot.
