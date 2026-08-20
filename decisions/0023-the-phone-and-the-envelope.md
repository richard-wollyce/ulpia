---
provenance: agent
stage: derived
---

# ADR-0023: the desk streams over SSE with sequence numbers, and the phone is a decision, not a feature flag

**Search for:** `phone`, `celular`, `PWA`, `websocket`, `SSE`, `envelope`, `event stream`

- **Date:** 2026-08-19
- **Status:** proposed
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. The transport is one function in `ui.rs`.

## Context

The reading room grew a chat (the desk), which needed a streaming transport, and Richard
asked two things at once: whether Letta's WebSocket envelope, recorded in
[[0020-vesta-routes-to-the-agent]]'s follow-up read as the best engineered piece of their
stack, should be implemented here, and whether his phone could reach the fleet on his
laptop, ideally as an installable PWA.

The envelope is four mechanisms, and they are separable: **typed events** (a
`message_type` discriminator per event), **ordering** (`event_seq`), **deduplication**
(`idempotency_key`), and **recovery** (`sync` replays current state after transport
loss, with fan-out to multiple subscribed clients).

## Options for the transport today

### Option A: the full envelope over WebSocket, now

- Cost: a WS handshake and framing implementation in a zero-dependency binary (the
  hand-rolled HTTP server does not speak WS), plus server-side event journals per
  conversation for `sync` to replay, plus key tracking for dedup.
- Failure mode: complexity bought before the client that needs it exists. One browser on
  loopback cannot lose the transport without losing the server, cannot receive
  duplicates from a connection it did not retry, and has nobody to fan out to.
- Forecloses: nothing, but it is scaffolding for an audience of zero.

### Option B: SSE, carrying the third of the envelope that is load bearing everywhere

Typed events and sequence numbers, over `text/event-stream` on the existing HTTP server.

- Cost: about sixty lines, no framing, no journal.
- Failure mode: a dropped stream mid-turn loses the rest of that turn's events, and the
  client re-asks. Acceptable on loopback, where the stream and the server share a fate.
- Forecloses: nothing. EventSource reconnects natively, and the `seq` field is the hook
  the remaining two thirds attach to later.

### Option C: no stream, poll for the finished answer

- Cost: none.
- Failure mode: a chat that says nothing for the whole of a model turn reads as broken,
  and the routing line (Vesta saying who answers) is worth showing the moment it exists.

## Decision, first half

**Option B, shipped.** Typed events (`session`, `routed`, `tool`, `assistant`, `error`,
`done`) with `seq` on every one. The typed union is the envelope's real lesson and it
cost nothing; the reliability machinery waits for a client that can actually lose
messages.

## The phone, stated honestly rather than started

The phone is not a transport feature. It is four decisions, and two of them are not
engineering:

1. **The bind.** `kb ui` binds 127.0.0.1 and refuses otherwise, deliberately. A LAN bind
   is a new exposure class: every device on the network can reach a server that reads
   the fleet and spawns the runtime. This is the moment Letta's rule applies whole:
   loopback is trusted, any other bind requires a token presented on every request.
   Their shape (a capability token in a file, or short-lived tokens minted from a shared
   secret) is the right one to copy.
2. **The secure context.** An installable PWA requires HTTPS everywhere except
   localhost. `http://192.168.x.x:4114` gets a page, never a service worker, never an
   install prompt. The honest options are: a plain browser page over LAN HTTP with a
   token (works today, installs never), a self-signed certificate the phone is told to
   trust (works, ugly, breaks silently on rotation), or a real hostname with a real
   certificate, which drags DNS into a local-first product.
3. **What the phone may do.** Reading the stacks is one risk; the desk spawning model
   sessions from any device that found the token is another. The write gates hold
   either way, but a phone that can start conversations is a phone that can spend
   Richard's plan.
4. **The reliability tier.** This is where the envelope's other two thirds become load
   bearing: a phone on Wi-Fi drops streams as a matter of course, so `sync` replay and
   idempotency keys stop being scaffolding and start being the feature. The `seq` field
   shipped today is their anchor.

**Decision, second half: deferred, all four together.** Not because it is hard, but
because each is a real choice Richard should make looking at it, not inherit from a
transport patch. The revisit trigger is him wanting it enough to sit through those four
decisions in one session.

## Consequences

- The desk works today on loopback with typed, ordered events, and its transport needs
  no change for the phone: SSE survives the trip; what changes is bind, auth, and
  recovery.
- The envelope is documented as three separable tiers rather than one artifact, which
  is what makes the deferral cheap instead of a rewrite.

## Revisit trigger

Richard asks for the phone again. That is the whole trigger: the demand is the decision
session, and this record is its agenda.
