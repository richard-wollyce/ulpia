# CopilotKit has three protocols. We have one, and the one we are missing is A2A.

A study for Richard, 2026-09-02. His question, in his words: CopilotKit has three
different protocols, AG-UI, MCP and A2A, and that made him think, we do not have A2A, do
we? Should we?

Every claim below is marked. **Read** means a sentence I read in a specification, a
source file or a page, and it carries its location. **Inferred** means I concluded it. A
protocol claim I did not read in the specification is inferred, however confident it
sounds. Section 6 collects the split.

---

## 1. The answer

**Do we have A2A? No. We have never had it, and we have never declined it either.**

Read: a grep for `A2A`, `Agent2Agent`, `AG-UI`, `AGUI` and `CopilotKit` across
`decisions/` and `tools/kb/src/` returns zero hits. Thirty-eight decision records and
none of them mentions the protocol. So the honest description is not "we looked at A2A
and said no". It is "A2A has never come up". That distinction matters, because a study
that reports absence as refusal credits the project with a judgement it never made.

What we have instead, precisely:

- **MCP, adopted, as a server.** Read: `kb serve` speaks JSON-RPC 2.0 over stdio and
  exposes five tools, `kb_route`, `kb_retrieve`, `kb_remember`, `kb_fleet`, `kb_list`
  (`tools/kb/src/mcp.rs:207-296`). It echoes the client's own `protocolVersion` rather
  than asserting one (`mcp.rs:183-205`).
- **A hand-rolled SSE envelope for the reading room**, built against Letta's WebSocket
  design as prior art, not against AG-UI. Read: ADR-0023 takes two of Letta's four named
  mechanisms, typed events and `seq`, "the third of the envelope that is load bearing
  everywhere" (`decisions/0023-the-phone-and-the-envelope.md:20-62`).
- **No agent-to-agent anything.** Read: grep for `handoff`, `hand off`, `delegate` and
  `transfer` across `tools/kb/src/` returns zero hits. `classify::Verdict.owner` is a
  single `Option<String>` (`classify.rs:55-57`), so exactly one agent owns a message and
  a second owner is unrepresentable.

**Should we? No. Not now, and the reason is not cost, it is that we have no counterparty.**

The mechanism, because a recommendation without it does not ship. A2A's premise is stated
in its own specification, section 1.2: agents collaborate "without needing to share their
internal thoughts, plans, or tool implementations", and section 1 says the point is to
cooperate "without needing access to each other's internal state, memory, or tools".
Ulpia's premise is the exact inverse. Read: `Memory::open` loops every base and calls
`entries.extend(built.entries)`, concatenating the whole fleet into one `Vec`
(`memory.rs:569-650`); `Memory::ask` runs `index::route` and `search_all` over that single
`self.entries` (`memory.rs:1012-1032`). There is one index and every agent reads all of
it. A2A is a protocol for talking to something you cannot read. We can read everything.

That is not a slogan, it is the design decision that already answered this question once.
Read: ADR-0024 records the router waking an agent to answer a question about Richard's own
website while "two folders away a file recorded a twelve month professional goal that was
precisely the marketing agent's business" (`decisions/0024-the-person-is-one-base.md:37`).
An agent needed what another agent held. The fix was not "let it ask". The fix was one
base every agent reads, and it was measured as a net saving: Zed's payload went from
9,689 bytes to 2,708, about 1,700 tokens off his resident set, while three other agents
gained the file entirely (`0024:101-102`).

**The recommendation, in one line:** take the one piece of A2A that has independent
evidence of value, the manifest, and take it without the protocol. Three small changes,
none of which touches the dependency count, the network position or the accounts row.
Section 5 has them. The observable trigger that would reopen the question is in section 5
too, and it is not a feeling: the first agent in the fleet whose base is not on this
filesystem.

---

## 2. What CopilotKit is, and why three protocols is their answer

CopilotKit is not an agent framework. It is the presentation layer that sits in front of
somebody else's agent.

Read: the GitHub repository describes itself as "The Frontend Stack for Agents &
Generative UI. React, Angular, Mobile, Slack, and more. Makers of the AG-UI Protocol".
The README says "What started as a React library is now the horizontal layer between your
agents and your users: the same agent can power your web app, your mobile app, and your
team's Slack or Microsoft Teams workspace." The docs state the three layers plainly:
"CopilotKit is a three-layer stack, frontend, runtime, agent, connected by the open AG-UI
event protocol. The runtime lives in your own application server, so the only thing
between your UI and your agent is a wire format you can inspect."
(`https://docs.copilotkit.ai/concepts/architecture`)

Read: their own docs are explicit that their built-in agent is a fallback rather than the
point. "`BuiltInAgent` is CopilotKit's *own* agent, it calls the model directly.
Registering it as `default` means chat talks to it, not to any agent you already wrote. It
replaces your agent rather than connecting to it."

**Inferred, and this is most of the answer to Richard's question: their constraint is
that they do not own the agent.** A company whose product is the UI for an agent it did
not write has to speak whatever that agent speaks. If the customer's agent runs on
LangGraph, they need a LangGraph bridge. If it calls MCP servers, they must render MCP
tool calls. If the customer bought two agents from two vendors that talk over A2A, they
must terminate A2A and translate it. So three protocols is not a claim about how the
world is layered. It is a bill of materials for a translation business.

The package names give it away. Read: their A2A support is `@ag-ui/a2a-middleware` and
their MCP Apps support is `@ag-ui/mcp-apps-middleware`. Both live in the AG-UI namespace,
not the upstream one. Read, on the direction of the dependency: "CopilotKit uses a
middleware to expose A2A agents to an AG-UI compatible coordinator. This allows you to
expose your A2A agents to your users through the AG-UI protocol."
(`https://docs.copilotkit.ai/agentic-protocols/a2a`)

**Be fair to them on two counts.** First, they integrate rather than compete. Read: their
own AG-UI README lists A2A under "Agent Interaction Protocols" as Supported, with
CopilotKit docs and the integration marked "Partnership", and lists MCP Apps under
"Generative UI" as Supported with CopilotKit docs, even though MCP Apps occupies AG-UI's
own slot. That is not the behaviour of somebody defending a tier. Second, they are not a
toy: read, the repository was created 2023-06-19, has 37,167 stars, 214 contributors,
1,444 releases and a commit from the day I checked (GitHub API, 2026-09-02), and
TechCrunch reported a $27M Series A on 2026-05-05 with Deutsche Telekom, Docusign, Cisco
and S&P Global named as customers.

**Our constraint is the opposite of theirs, and naming that difference is the answer.**
We own every agent in the fleet. We wrote every base. Every agent's knowledge is on this
filesystem, readable by one process, in one index. CopilotKit needs three protocols
because it stands between parties that cannot read each other. Ulpia has no such gap to
bridge, because Ulpia is the thing on the far side of everybody's boundary, for one
person.

One correction worth carrying, in fairness to the diagram: the three-layer framing appears
in CopilotKit's docs and in AG-UI's non-normative README ("MCP gives agents tools / A2A
allows agents to communicate with other agents / AG-UI brings agents into user-facing
applications"). It appears in **none** of the three normative specifications. Read: MCP
revision 2026-07-28 (33 files, 1,065,582 bytes) contains "AG-UI" zero times, "A2A" zero
times, "Agent2Agent" zero times. The AG-UI draft specification (27 files, 289,058 bytes)
contains "MCP" zero times and "A2A" zero times. The A2A 1.0.0 specification (156,828
bytes) mentions MCP exactly five times, all between lines 3606 and 3616, inside Appendix
B. Two conditions on that silence, because it reads more dramatically than it is: 726,080
of MCP's 1,065,582 bytes are `schema.mdx`, an auto-generated type dump, so 48 percent of
that corpus is machine-written listing rather than prose; and the silence is normative
only, since AG-UI's non-normative README names both neighbours and ships integrations for
both. **Inferred:** the honest reading is narrow. The layer relationship is documented
nowhere that binds an implementer, so a client speaking all three gets no conformance
obligation from any of them about how they meet.

---

## 3. What each protocol actually is

### MCP, revision 2026-07-28

An agent reaching outward to capability it does not own: tools, resources and prompts,
mounted by a host it trusts. Read, on the topology: "Servers should not be able to read
the whole conversation, nor 'see into' other servers... Cross-server interactions are
controlled by the host", with "each client having a 1:1 relationship with a particular
server" (`architecture/index.mdx:71, 101-106`). Read, on direction, which is the
load-bearing rule: "A binding MUST deliver client-sent requests and notifications to the
server, and server-sent responses and notifications to the client. No other message
direction exists: per the message patterns, servers do not initiate JSON-RPC requests and
clients do not send JSON-RPC responses" (`basic/transports/index.mdx:32-36`). And the
breaking change that hardened it: "Servers MUST send server-to-client requests (such as
`roots/list`, `sampling/createMessage`, or `elicitation/create`) using the MRTR pattern.
The previous pattern of server-initiated requests is no longer supported. This is a
breaking change." **Inferred:** a protocol where one side structurally cannot originate a
request is not a peer protocol.

### A2A, 1.0.0

Peer delegation between agents that stay opaque to each other. Read, section 1: agents
"discover each other's capabilities... securely exchange information to achieve user goals
without needing access to each other's internal state, memory, or tools." The unit of work
is a Task with an id, nine `TaskState` values (UNSPECIFIED, SUBMITTED, WORKING, COMPLETED,
FAILED, CANCELED, INPUT_REQUIRED, REJECTED, AUTH_REQUIRED), an artifacts list and a message
history. Discovery is hostless: "A2A Servers MUST make an Agent Card available" (8.1) at
the IANA-registered well-known suffix `agent-card.json`. Three normative bindings, JSON-RPC,
gRPC and HTTP+JSON/REST, and section 7.1: "Production deployments MUST use encrypted
communication (HTTPS for HTTP-based bindings, TLS for gRPC)."

### AG-UI, draft

The agent streaming itself to whatever renders it. Read: "AG-UI is an open protocol that
standardizes how agents talk to user-facing applications: one request in, one ordered
stream of typed events out, carrying everything the user sees of the agent"
(`docs/spec/draft/index.mdx`). Thirty-one event types in eight families, only the run
lifecycle mandatory for a producer, a consumer MUST accept all of them. Note the version
condition: the whole normative specification is served under `/spec/draft`, every page
imports a `DraftBanner`, and the deprecated `THINKING_*` events say "Will be removed in
1.0.0". As of today, AG-UI 1.0 has not shipped. Note also the drift: the front-page copy
still says "16 standardized event types" while the draft spec says thirty-one. The
specification outranks the page about it.

### The overlap, named

**The MCP/A2A seam is real and mutually declared.** Read, A2A Appendix B: "A2A and MCP are
complementary protocols designed for different aspects of agentic systems... Think of MCP
as the 'how-to' for an agent to use a specific capability or access a resource... It's
about how agents partner or delegate work." MCP never returns the compliment in text, but
its message-direction rule forecloses A2A's job structurally.

**The seam moved in 2026-07-28, in both directions at once, and reporting one direction is
how both the pitch and the takedown get written.** Inward: the same revision deprecated
Roots, Sampling and Logging. Read, the lifecycle policy: "new implementations SHOULD NOT
adopt it, and existing implementations SHOULD migrate before the feature's earliest
removal", with Sampling's migration path given in the registry table as "Integrate directly
with LLM provider APIs" and earliest removal "First revision released on or after
2027-07-28". Outward, same revision or its extensions: MCP Tasks absorbed the common core
of A2A's state machine, and MCP Apps absorbed interactive HTML in the chat. **Inferred:**
MCP's edge moved inward on three features and outward on two extensions, and nobody redrew
the diagram.

**The sharpest single overlap, and it breaks the tidy story.** All three protocols specify
"the work stopped to ask a human", and there are four wire shapes, not three, because MCP
carries two. Read, MCP MRTR: the server returns `InputRequiredResult` and "The JSON-RPC id
MUST be different between the initial request and the retry, as they are independent
requests". Read, MCP Tasks: the task moves to `input_required`, stays alive, and "The
client responds via `tasks/update`, no second connection or unsolicited server-to-client
messages required". Read, AG-UI: "A producer MUST NOT report an interrupted run as
success: the interrupt outcome is the only conforming way to end a run that stopped to
ask." Read, A2A: `TASK_STATE_INPUT_REQUIRED` is annotated "This is an interrupted state"
and the task persists. **Inferred:** if one protocol needs two incompatible answers to one
question in one revision, that question is not a layer boundary. It is a hard design
problem nobody has settled, MCP included.

Sourcing caveat on Tasks, and it matters under the house rule: every Tasks quote here comes
from `modelcontextprotocol.io/extensions/tasks/overview`, a first-party documentation page,
not from the normative extension specification in `github.com/modelcontextprotocol/ext-tasks`,
which was not fetched. This claim rests on documentation, not on a specification.

**And the framing's real overstatement: the stack has no floor.** Nothing in these three
specifications defines agent memory, cross-session state, routing, or which agent should
answer a given message. A2A refuses it by principle, MCP by architecture, AG-UI by scope.
That gap is exactly Ulpia's subject, and it is where LangGraph, CrewAI, the Agents SDK and
every in-house orchestrator live. None of them is a wire protocol, and neither is our
`UserPromptSubmit` hook.

---

## 4. What Ulpia already has that is A2A shaped, and what it structurally cannot do

### Shaped like A2A, under other names

**An agent card, minus the endpoint and minus the schema.** Read: `agent.txt` declares
`name`, `role` and `ends` (`fleet.rs:56-78`), and `kb_fleet` serves the fleet's name and
role plus every member, explicitly labelled as read from manifests rather than from the
index (`fleet.rs:98-125`, `mcp.rs:304`).

**A capability declaration with a typed reply over a provider-independent transport.**
Read, ADR-0027 decision point 5: "The contract is a process, not a provider: dossier on
stdin, verdict on stdout. Any model behind any runtime satisfies it, including a local
one... `kb` gains no dependency and no network code." The reply is four labelled lines and
`classify::parse` "refuses a name that is not on the roster" (`classify.rs:374-395`).

**A first-class decline that is stronger than anything A2A expresses.** Read, ADR-0027
decision point 4: "Coverage is a first class answer. covered, adjacent, uncovered. Adjacent
names the nearest agent and says plainly that answering from there is a stretch." And the
consequence at `:135`: "The fleet can now say nobody here covers this, which is the input
to deciding whether an agent should exist. That answer did not exist in any previous
version."

**A producer and an independent reviewer exchanging an artefact across a process boundary,
where the independence is enforced by the type.** Read, ADR-0030's table at `:58`: promoter
one reads "the deposit, `inbox/`" and never sees "the base"; promoter two reads "the
proposal, plus what the base already holds" and never sees "promoter one's reasoning". Read
`promote.rs:81-95`: "There is no `reasoning` field and that absence is the mechanism",
pinned by the test `a_proposal_carries_no_place_to_put_reasoning` at `:920`.

**A work-claim lease under concurrency.** Read: `Lock::take` uses `create_new` (O_EXCL on
Unix, CREATE_NEW on Windows) at `<fleet_root>/.kb-promote.lock`, stale after 3600 seconds,
and reports when it took one over (`promote.rs:239-326`).

**And the transport half of an A2A server, already in the binary, hand written, with no
dependency.** Read: `mcp.rs` sets `"jsonrpc":"2.0"`, carries the id back byte identical
(`:684-703`) and defines -32700/-32600/-32601/-32602 (`:53-56`); `json.rs` is a hand
written parser and serialiser; `ui.rs` runs an HTTP/1.1 server over `std::net::TcpListener`
(`:105`) that writes `HTTP/1.1 200 OK\r\nContent-Type: text/event-stream` (`:246`). A2A
section 9.1 asks for exactly that combination: "Protocol: JSON-RPC 2.0 over HTTP(S)...
Streaming: Server-Sent Events (`text/event-stream`)". **Inferred:** the first objection
people reach for, that a one-dependency local binary cannot speak HTTP and JSON-RPC, is
false here. Put the transport cost at roughly zero so the argument happens where the real
costs are, which is custody and scope.

### What it structurally cannot do

**An agent has no address.** Read: its identity is a directory name (`memory.rs:1550-1555`)
and its only handle is a filesystem path (`fleet.rs:38-42`). Nothing opens a channel from
one agent to another.

**Exactly one owner per message.** Read: `classify::Verdict.owner` is `Option<String>`
(`classify.rs:53-63`), `boot::remember_agent` writes one name into one session file
(`boot.rs:101-107`), `Briefing.agent` is `Option<String>` (`boot.rs:112`), and
`capture`'s owner is `routed.last()` with a single-name fallback (`capture.rs:139-144`).
A question needing two agents can be routed to one or refused. It cannot be split.

**No agent can initiate toward another.** Correcting a claim I would otherwise have made:
Ulpia **does** run unattended work. Read, ADR-0035: a SessionEnd hook,
`.claude/hooks/promote-on-idle.sh`, runs `kb capture` synchronously and detaches
`kb promote`, writing a markdown file into the routed agent's `inbox/`. So "nothing runs
outside the user's turn" is false. What is true is narrower and is the property that
actually rules out A2A: no agent can address another one.

**Nothing is reachable from another machine.** Read, README:525-527: "Local first is a
design position, not a limitation waiting to be lifted: the index lives beside the files,
nothing talks to a server, and there is no hosted instance to point at. The base has to be
on the same filesystem as the binary." `kb serve` is stdio only (`mcp.rs:104-131`);
`ui.rs:36-43` says it "Binds 127.0.0.1 and nothing else... a non-loopback bind is refused
at argument time rather than tokened".

**And the most useful thing the system does is not reachable by any caller.** Read:
`classify::run` has exactly two callers, `boot.rs:202` and `eval.rs:330`. `kb route --json`
emits `agent` from the deterministic keyword fold, not from the classifier
(`main.rs:1088-1116`). `kb_route` over MCP emits ranked files and an evidence line with no
owner anywhere in the output (`mcp.rs:328-364`). **Inferred:** the model-decided owner with
a first-class "nobody here does this" verdict, which is the most distinctive thing in the
codebase, exists only inside the hook. That gap is not caused by refusing A2A and would not
be closed by adopting it.

### The one that is a bug rather than a boundary

Read: `fleet::Card.ends` is parsed at `fleet.rs:70`, written into the classifier dossier as
`    stops: {ends}` at `classify.rs:133` under the comment "The edge matters more than the
role for the judgement being asked for", and taken by `promote::proposal_prompt` at
`promote.rs:378`. It appears nowhere in `Description::to_text`, which prints FLEET, ROLE,
AGENTS, then per member name, root and role, and stops (`fleet.rs:98-125`). `kb_fleet`
returns exactly that text (`mcp.rs:304`).

So two internal model prompts get the edges and every remote reader of the roster does not.
That reproduces exactly the failure the field was created to prevent, quoted from its own
doc comment: "**A roster of roles tells a reader what each agent does and never what none
of them does**, which is exactly the judgement the classifier exists to make."

---

## 5. The recommendation

### Do now, all three cheap, none touching the network position

**1. Print the edges on `kb_fleet`.** One line in `fleet::Description::to_text`, the one
function out of three that drops `ends`. **Mechanism:** the classifier and the promoter
already receive the edges, so withholding them from a remote MCP caller is one function out
of step with two, which makes it an oversight rather than a policy. Independent support
that this is the piece of A2A worth having: read, API Evangelist's census, "Providers are
reaching for the Agent Card not because they intend to speak A2A to other agents, but
because it is the only widely-known machine-readable way to say here is my agent surface,
here is what it can do, here is where to authenticate. They want the manifest. The protocol
is incidental."

**2. Put the owner and the coverage verdict on `kb_route`'s MCP output.** **Mechanism:**
the fleet's most distinctive answer, a model-decided owner with a first-class Uncovered
verdict, currently exists only on the hook path and is invisible to the one client that
already exists. This is the cheapest interoperability win available and it is entirely
inside our own code.

**3. Fix the deposit misdelivery, and price it correctly.** Read: `capture::read` parses the
append-ordered `.events` file into two separate vectors, `refused: Vec<(String, Vec<String>)>`
and `routed: Vec<String>` (`capture.rs:84-136`), throwing away the interleaving between a
refused question and the agent current when it was refused. `write_deposit` takes
`record.routed.last()` under the comment "Whoever had the conversation last owns what it
left" (`:139-144`). **Consequence:** a session that opened with the architect and closed
with the nutritionist deposits every refusal into the nutritionist's inbox. **Correction to
the cheap version of this fix:** it is not a partition of a `Vec` inside one function.
`Session` and `capture::read` both have to change to carry the owning agent per refusal. The
ordering exists on disk and not in the type. That is still one process, one filesystem, no
network and no peer.

### Worth doing when it stops being free

**4. Deliver the scope refusals instead of discarding them.** Measured on
`kb-rejections.txt` today, lens column only, no refusal text read: 181 distinct refused
proposals, of which 19 were refused by the Scope lens over 28 refusal events, against 143
duplication (199 events) and 19 contradiction (23 events). Scope is 10.5 percent of distinct
refusals and 11.2 percent of refusal events. Read, the Scope lens prompt: "Reject if it is
out of scope, and name the agent it belongs to if one does." So the system detects material
belonging to another agent, names the recipient, and throws it away. **This cuts both ways
and both halves belong in the report.** The delivery demand is real and non-zero rather than
theoretical. It is also small, and it is dwarfed by duplication at 143, which is the shared
index doing its job. The fix is a call to `write::note` into the named agent's inbox, in the
same process under the same lock. Any A2A framing of 19 proposals is a wire protocol wrapped
around a function call.

**5. The cross-base citation, when ADR-0026's own trigger fires.** Read, ADR-0026 decision
point 3: the reading room's ribbons "showed zero cross-base marks on the real fleet before,
because the audit had already converted those links away, and they show a real one now." One
written path on the real fleet is the honest size of the person's cross-agent workload. Read,
its revisit trigger, which already names the proportionate fix, a checkable `[[base:note]]`
syntax: "Worth building the first time a written path goes stale without anything noticing."

### Do not do

**Do not build an A2A server.** Five specific costs, in order of how hard they are to
reverse.

- **TLS is the only requirement `std` cannot cover.** Read, A2A 7.1: "Production deployments
  **MUST** use encrypted communication". Read, the normative proto on `AgentInterface.url`:
  "For HTTP-based transports, must be a valid absolute HTTPS URL in production." Priced
  honestly, and smaller than the objection usually gets stated: `cargo tree --edges normal`
  in `tools/kb` today prints ten crates including `kb` itself, so nine dependencies, all
  under `rusqlite`. `rustls` 0.23.43 declares five required normal dependencies plus a crypto
  provider. So the declared count goes from one to two and the tree from nine to roughly
  twenty. Not a sevenfold increase and not a new build class, since read, README:137-138, a
  fresh clone already "needs a C toolchain (MSVC Build Tools on Windows)". On ADR-0007's own
  reasoning, "Writing a B-tree and a full text index by hand to avoid a dependency would be
  the same mistake in the opposite direction", importing `rustls` would be defensible. The
  argument against it is the ratio against the reachable audience, not the crate count.
- **The reverse-proxy escape does not dissolve the objection, it relocates it.** Terminating
  TLS in a proxy and letting `kb` speak plain HTTP on loopback is what AWS AgentCore does.
  But ADR-0007's phrasing is "`rusqlite` with the bundled feature compiles SQLite in, so
  there is still no system package to install and nothing to run", and a reverse proxy is
  precisely a system package to install and a thing to run. On ADR-0007's own words the proxy
  breaks the constraint.
- **Authentication contradicts a recorded decision rather than costing work.** Read, A2A 7.4:
  the server "**MUST** authenticate every incoming request". Read, 13.1, which is stronger and
  closes the cheap escape: "Servers **MUST** implement authorization checks on every A2A
  Protocol Operations request" and "implementations **MUST** scope results to the caller's
  authorized access boundaries". Read, ADR-0008's decision table: "No authentication, no
  accounts, no tenancy | Nothing in the code knows what a user is." **Be fair about the size
  of what is being refused:** A2A defines no identity of its own, and section 7.3 puts
  credential acquisition out of band, so a single API key checked against a value in
  `fleet.txt` satisfies 7.4 as written. The objection that survives is not effort. It is that
  a product whose code has never known what a caller is would now hold exactly one credential,
  and that credential is the seam every later account system grows from. Label that as a slope
  argument, not a technical cost.
- **The card collides with the privacy split, and the specification concedes it.** Read,
  ADR-0025 puts "Every base under `fleet/`" in the Private column and the person's profile
  there as "`fleet/profile/`, every word of it". Read, A2A's own discovery guide: "Agent Cards
  include sensitive information, such as: URLs for internal or restricted agents. Descriptions
  of sensitive skills." Its mitigations are authenticated extended cards, mTLS, network
  restrictions and registry selective disclosure, none of which this project has. For a
  corporate agent a skills list is a datasheet. For a fleet containing a person's own base it
  is a readable index of what its owner keeps agents for, served unauthenticated at a
  well-known path by design.
- **The Task model is the largest genuine build and there is nowhere to put it.** Read, A2A
  3.1.4: List Tasks "MUST use cursor-based pagination", "MUST be sorted by last update time in
  descending order", with "appropriate authorization scoping". Nothing in Ulpia persists a unit
  of work: `.kb/sessions/<id>` holds one agent name, the promote lock holds a pid aged by
  mtime. A durable store with ids, a nine-state machine, cursor pagination and per-caller
  scoping is the first mutable server state in a product whose position is that files are the
  source of truth. That is a custody change, not a feature.

**And say plainly what A2A does not violate**, because pretending otherwise would be the
weaker argument. Read, ADR-0009: the contract is "a Rust API first, `kb::memory::Memory`...
MCP, the GUI and any future HTTP service are wrappers over it, so they cannot answer
differently." An A2A server is the anticipated shape of a future HTTP service. Nothing about
it is architecturally excluded. The conflicts are three specific rows, all amendable by a new
ADR.

### The trigger, observable rather than felt

**Reopen this the first time the fleet contains an agent Richard does not own and whose base
is not on this filesystem.** That is observable in one line: a `fleet.txt` entry, or an
attached base, that names a host rather than a path. Two secondary triggers, each on its own
sufficient:

- **The scope-refusal count crosses duplication**, or the fix in item 4 ships and the
  delivered notes are still going to the wrong agent. That would mean cross-agent traffic is
  the dominant mode rather than a tenth of it.
- **A public census shows conformant Agent Cards in the hundreds rather than the tens.** The
  baseline, with its conditions: read, API Evangelist, 2026-07-29, probing 22,341 deduplicated
  hosts from the APIs.io catalog, of which "20,185 answered". "Sixty-five providers serve a
  card", stated as "0.29 percent of the reachable web surface of the entire API industry"
  against the full 22,341, which is 0.32 percent against the answering set. "Only 10 pass every
  structural check in the A2A 1.0.0 AgentCard object", 41 fail a hard structural requirement,
  40 declare no `protocolVersion`, and "Fifteen of the 65, twenty-three percent, are still
  sitting at `/.well-known/agent.json`", the pre-0.3 path a conformant client never reads.
  **Condition, and it is not a formality:** the population is the public API web surface, not
  a census of the internet, and A2A's defenders place production use inside enterprise
  boundaries this method cannot see. Strong evidence about public agent-to-agent discovery,
  no evidence at all about intra-enterprise deployment.

**Be fair to the protocol while the answer is no.** A2A has been a Linux Foundation project
since Google's 2025 donation, not only since it moved to the Agentic AI Foundation on
2026-08-17. AWS ships first-party support: `agentcore create --protocol A2A`, cards at
`/.well-known/agent-card.json`. There is a v1.0.1 of 2026-05-28. And on the one benchmark I
found, read, arXiv 2603.22823, 30 queries by 5 runs by 3 architectures, 450 executions: for
complex queries A2A finished in 45.1 s against MCP's 51.8 s and consumed 11,318 tokens against
MCP's 34,959, a 3.1x difference, at 39 percent lower cost. Conditions the paper states itself,
and they are load bearing: one model (Claude Sonnet), one domain, and **the A2A agents ran on
localhost**, so the one place A2A wins is the place its network cost was zeroed out. It also
took 1,530 lines across 15 files against MCP's 706 across 11.

The last argument in A2A's favour is our own number, and it deserves to be stated rather than
buried. Read, ADR-0027:116-129: the classifier adds "9 to 15 seconds... to every message", and
the autopsy: "This line read 13 to 16 until 2026-08-20 and the real figure was 43 to 48, over
a hook budget of 30", caused by the classifier inheriting a working directory holding "11,510
files and 2.5 GB of build output". Read, `classify.rs:455-462`, same dossier, same model, same
flags, varying only the working directory: fleet root 47.4 s wall of which 12.8 s API, empty
directory 11.5 s wall of which 7.1 s API. An eleven second answer is exactly the case A2A's
return-a-Task-then-stream-updates was built for. **Inferred:** if an A2A server were ever built
here, the task model would be earning its keep rather than being protocol tax. That is the
strongest single point on the other side, and it still does not carry, because the same
latency budget is the reason not to add a hop: one agent-to-agent hop inside the
`UserPromptSubmit` hook is a second constitution assembly plus a second model call of the same
order against a 30 s budget that has already been blown once by a cost nobody designed.

---

## 6. What we read against what we inferred

### Read, quoted from a specification or from source

- MCP's message-direction rule, its host-mediated isolation, and the MRTR breaking change.
- A2A sections 1, 1.2, 7.1, 7.3, 7.4, 8.1, 8.2, 13.1, Appendix B, and the nine `TaskState`
  values from `specification/a2a.proto`.
- AG-UI's scope paragraph, its thirty-one event types in eight families, and its draft status.
- The zero-mention counts across all three corpora, with byte totals.
- Every Ulpia claim with a file and line: `memory.rs:569-650` and `:1012-1032` for the single
  concatenated index; `classify.rs:55-57`, `:123-136`, `:374-395`, `:455-462`;
  `fleet.rs:22-36`, `:56-78`, `:98-125`; `mcp.rs:53-56`, `:104-131`, `:207-296`, `:304`,
  `:328-364`, `:684-703`; `boot.rs:101-112`, `:202`, `:375`; `capture.rs:84-136`, `:139-144`;
  `promote.rs:81-95`, `:148-155`, `:239-326`, `:378`, `:920`; `ui.rs:36-43`, `:105`, `:246`;
  `main.rs:1088-1116`; `base.rs:79-110`, `:195-200`.
- ADR-0007, 0008, 0009, 0020, 0022, 0023, 0024, 0025, 0026, 0027, 0030, 0035, quoted.
- The rejection census, run today over `kb-rejections.txt`, lens column only: 181 distinct,
  19 scope over 28 events, 143 duplication over 199, 19 contradiction over 23.
- `cargo tree --edges normal` in `tools/kb`, run today: ten crates, nine dependencies.
- The API Evangelist census figures and the arXiv benchmark figures, with their stated
  conditions.
- CopilotKit's architecture page, the AG-UI README's support tables, and the repository
  metadata from the GitHub API.

### Inferred, my conclusion and not a sentence anybody wrote

- That CopilotKit's three protocols follow from not owning the agent, and that Ulpia's
  constraint is the inverse. Nobody at CopilotKit says this; it is my reading of their package
  namespaces and their own "it replaces your agent rather than connecting to it" warning.
- That the four interrupt shapes mean the question is a hard design problem rather than a
  layer boundary.
- That MCP's edge moved inward on three deprecations and outward on two extensions, and that
  neither direction alone is an honest summary.
- That opacity is A2A's premise rather than a side effect, and that a fleet built on one shared
  index has nothing for that premise to attach to.
- That `rustls` doubles the declared dependency count and roughly doubles the tree. I read
  crates.io's dependency list for 0.23.43 and counted; I did not build it.
- That an added hop costs "a second model call of the same order". Nothing in the record
  measures a second hop, so this is deliberately weaker than saying it doubles latency.
- That `ends` missing from `to_text` is an oversight rather than a policy, on the grounds that
  two of three consumers already receive it.
- That the reverse-proxy option breaks ADR-0007's constraint rather than dissolving it.
- That `std` cannot supply TLS. This sits on top of two quotes rather than being one.

### Not verified, and worth saying so

- The MCP Tasks quotes come from a first-party documentation page, not from the normative
  extension specification in `ext-tasks`, which was not fetched. Under the rule that a
  specification outranks a page about it, that claim rests on documentation.
- The "31 event types" figure was reconciled against the TypeScript enum (36 members, 5
  deprecated) but the eight families and named events are what I verified in the spec prose.
- No first-party product documentation was read for Salesforce, SAP, IBM or Cisco A2A
  deployments. Treat those as claimed, not verified.
- No independent measurement exists of intra-enterprise A2A deployment. Every deployment
  report I could reach is qualitative. The Redis piece states it plainly and I agree: "we
  don't have enough independent data to say how common production A2A is."

### One stale line found while checking, unrelated to the question

`README.md:498` says "Four read-only tools over stdio: `kb_route`, `kb_retrieve`,
`kb_remember`, `kb_fleet`." There are five, `kb_list` is missing, and "read-only" is the
overstatement `mcp.rs:28-34` already corrects: a refusal in `kb_route` or `kb_retrieve` calls
`Memory::recall_loss`, which appends to `kb-misses.txt`. Worth a separate fix; it is not part
of this recommendation.
