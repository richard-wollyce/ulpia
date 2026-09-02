---
provenance: agent
stage: derived
---

# ADR-0035: the session feeds the deposit, and every surface counts its losses

**Search for:** `captura de sessao`, `session capture`, `memoria curta`, `short memory`, `memoria longa`, `long memory`, `deposito`, `deposit`, `inbox`, `dreaming`, `consolidacao`, `consolidation`, `sonho`, `noturno`, `nightly`, `cron`, `agendado`, `scheduled`, `idle`, `ocioso`, `SessionEnd`, `hook`, `kb boot`, `kb ui`, `recall_loss`, `perda de recall`, `kb-misses.txt`, `misses`, `proveniencia`, `provenance`, `agent output`, `saida do modelo`, `transcript`, `transcricao`, `jsonl`, `promote`, `promotor`, `remember`, `fila`, `queue`, `ADR-0030`, `ADR-0031`, `ADR-0034`, `ADR-0035`

**Exists to:** What writes into the short memory at the end of a session, why it writes without a model first, why the trigger stays the idle hook rather than a clock, and the two surfaces that record no recall loss and must.

- **Date:** 2026-09-01
- **Status:** accepted 2026-09-01, implemented the same day
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Builds on:** [[0030-two-promoters-and-the-second-is-not-a-second-opinion]], which names
  session capture as "the other half of the episodic story and its own decision". This is
  that decision. [[0034-git-leaves-the-runtime]] settled that the deposit is served and
  labelled, which is what makes a short memory worth feeding.
- **Reversibility:** reversible. Capture writes files into a folder that `promote` already
  reads; turning it off leaves the deposit as it was.

## Context

Richard's design, in his words: two memories. A short one, fresh and not dense, fed by
every input from the person and every output from the machine. A long one, consolidated,
dense and searchable, the library itself, fed only when something is worth keeping or when
the person says so: "grave os pontos principais desse livro". And a consolidation pass from
time to time, the thing the competitors call dreaming.

Almost all of that exists, and the table says which half does not:

| His term | What exists | State |
|---|---|---|
| Long memory, the library | `knowledge/`, one `Search for:` line per note | exists |
| Short memory, the deposit | `inbox/` in each agent | exists, served and labelled since ADR-0034, **and nothing writes into it** |
| Dreaming | `kb promote`: two promoters, three lenses, unanimity | exists, runs detached on `SessionEnd` |
| The filter | the promoters, with `kb remember` as their measure | exists |
| "Grave os pontos desse livro" | a file dropped in `inbox/`, then `promote` | exists |

So the decision is one thing: **what a session leaves in the deposit when it ends.**

A second, smaller defect sits beside it and belongs to the same record because it is the
same failure. `kb boot` and `kb ui` call `Memory::ask` and never `Memory::recall_loss`.
`boot` is the surface every message passes through on the hook, so a refusal there, which is
the most frequent refusal there is, enters no log. ADR-0034's worklist found the same defect
on four other surfaces and unified them; these two were named and deferred.

## Options for what writes into the deposit

### A. Deterministic capture, no model

At `SessionEnd`, the hook writes one markdown file into the routed agent's `inbox/`, named
by date and session, holding what the session already produced without any model:

- the questions the base refused, which is `kb-misses.txt`'s population for that session,
- the proposals `kb remember --json` produced during the session, queued as the F-04 pattern
  intends: ADD, UPDATE and NOOP with their evidence,
- the agent that was booted and the messages that changed it, which `boot` already tracks
  per session under `.kb/sessions/`.

Cost: zero model calls, and it captures only what passed through `kb`. A fact the person
said in prose and never asked about is not captured. Gain: every line in the file is
something the system measured, so the junk rate that ADR-0030 leaves unmeasured can be read
off `promote`'s output over real deposits before any generator of candidates is switched on.

### B. Model capture

At `SessionEnd`, a small model reads the transcript and writes candidate facts into the
deposit. Captures what the person said in passing. Cost: one call per session, and a junk
rate nobody has measured, which the promoters then have to absorb. Builds on A rather than
replacing it: the deterministic file is still written, and the model's candidates are a
second file beside it with `provenance: agent`.

**Chosen: A first, B after A has produced enough deposits for the junk rate to be a number.**

## Three things that hold either way

1. **No queue of raw turns, no SQLite of transcripts.** The transcript already exists on
   disk and belongs to the harness. Duplicating it into a durable queue with a watermark is
   the shape a competitor built and this fleet decided not to have, for a measured reason:
   nothing here writes in the background, and every index is rebuilt by whoever asks. The
   short memory is markdown in the deposit, like everything else.
2. **Everything captured carries provenance.** An output of the model stored as evidence is
   a fact laundered. ADR-0031 has three species and `write` has `--provenance`; a captured
   file is `agent` and stays `agent` through promotion, so what the model said and what the
   person said never become the same kind of line.
3. **The trigger stays the idle hook, not a clock.** The hook file already records why:
   `Stop` fires while the person is reading, a clock fires whether or not anything happened,
   and `SessionEnd` is the one observable moment a session actually stopped receiving input.
   A nightly run is a fallback for a session that never ends, which today does not happen.
   If it starts happening, add the clock as a second trigger, not a replacement.

## The other half: two surfaces that count nothing

`Memory::recall_loss` is one call, on the contract, and six surfaces make it. `kb boot` and
`kb ui` do not. The fix is one line in each, and it is deferred here rather than done, for a
reason that is specific to `boot`: it runs on every message, under the concurrency ADR-0021
describes, and `recall_loss` writes a file. Two sessions ending a message at the same
instant would both read, merge and rewrite `kb-misses.txt`, and the loser's line is gone.
`misses::record` is read-merge-write with no lock, which was fine while the writers were a
person at a terminal and a serve process, and is not fine for a hook.

So the order is: a lock or an append-only shape for the miss log first, then `boot` and
`ui` call `recall_loss` like everything else. The capture file in option A is written by the
same hook and has the same problem, which is why they are one record.

## What changed, 2026-09-01

In the order the record itself demanded.

1. **The miss log takes a marker while it merges.** `misses::record` holds a `create_new`
   lock file beside the log for the read, merge and write, with a 500 ms wait and a 30 s
   staleness rule, the same shape as `promote::Lock` and smaller. Tested with two threads
   recording twenty times each: both counts exact, marker gone afterwards. A live marker
   is respected and the caller is told; a stale one is stepped over.
2. **`kb boot` and `kb ui` count their losses.** One line each, through `Memory::recall_loss`
   like the other six surfaces, each with a test that drives a refused question through
   the surface and reads the log.
3. **The session's record.** `boot` appends to `.kb/sessions/<id>.events` on every message:
   `refused` with the question and the vocabulary offered, `routed` with the agent. Under
   `.kb/` because it is a working file and losing it costs one session's capture.
4. **`kb capture`.** Reads the record, writes `inbox/<date>-session-<id>.md` in the last
   routed agent's base, `provenance: agent`, `stage: raw`, no `Search for:` line, then
   deletes the record. The deposit is written before the record is deleted, so a crash
   between the two captures again rather than losing the session. A session no agent was
   routed in is not captured, the record is kept, and the reason is printed. The session
   id comes from `--session` or from the hook payload on stdin, the same JSON `boot` reads.
5. **The hook.** `promote-on-idle.sh` runs `kb capture` synchronously before the detached
   `kb promote`, so what this session refused is in front of the promoters this run.
6. **Run, not reasoned.** Two `boot` calls with real hook payloads against a scratch fleet,
   one hit and one refusal; the record held both lines; `capture` with the session id on
   stdin wrote the deposit into the routed agent's inbox; `kb route` over the same fleet
   then surfaced the deposit with `memory: "short"`; a second capture reported nothing to
   capture. 280 tests, from 272.

   **The first run of that flow captured nothing, and the reason is worth keeping.** The
   scratch fleet held one note, so the idf of every key was `ln(1 + 1/2)` and the best
   possible score for a four key question was 11.4 against a floor of 17.5. No message
   routed, no agent owned the session, and `capture` refused with the sentence it was built
   to print. A fleet too small to clear the floor never routes and therefore never captures;
   the refusals still reach `kb-misses.txt`, which does not need an owner. Four notes made
   the floor reachable and the flow ran as written.

What is still option B, and waits for a junk rate: the model's own outputs and the person's
prose. What waits for a flag: `kb remember` proposals, which do not know their session.

## Revisit trigger

- The first measured junk rate from `promote` over deposits that capture wrote. That number
  decides whether option B is switched on and with what `--max`.
- A session that runs longer than a day without ending. That is the day the clock trigger
  gets built.
