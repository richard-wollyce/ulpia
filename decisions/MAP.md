# MAP: the decision records

> The system's architecture decision records: what was decided, when, and the reason it beat the
> alternatives. They live at the repository root rather than inside any agent because a decision
> about the software binds every agent, and reaching them through one agent's folder made no sense.
> Attached to the fleet as a routable base by `attach = decisions` in `fleet.txt`, so a question
> like "why is it called Vesta" finds its answer here without a model.
>
> Numbered, dated, never edited after acceptance: a decision that changes gets a **new** ADR that
> supersedes the old one. Every ADR carries a revisit trigger.

## Current contents

### decisions/


- **[[0019-the-system-is-ulpia]]** 🟢 accepted 2026-08-18. **The system is Ulpia, the librarian
  stays Vesta, and the agents live in `fleet/`.** Named through Steve's process: Wollner's rule kills
  the literal words everyone converges on, a collision search killed six of eight candidates, and the
  survivor is the Bibliotheca Ulpia, Trajan's library, which was Rome's public reading room and its
  record office in one building, knowledge plus decision records, which is this software. Sealed by
  `ulpia.io`. Supersedes ADR-0012's system name and its no-new-domain position; Vesta and its
  reasoning survive untouched. Records the layout that follows: root takes the product name, agents/
  becomes fleet/.
  Search for: `Ulpia`, `ulpia.io`, `name`, `product name`, `system name`, `rename`, `fleet folder`,
  `library metaphor`, `librarian`, `Bibliotheca Ulpia`, `nome do sistema`, `biblioteca`.

- **[[0001-repository-shape]]** ✅ accepted. Zed is built on Yaron's three file split, extended with
  `decisions/`, `records/sessions/`, `fleet/` and a staleness aware evidence ruler. Rejects copying
  Steve's combined boot file (grows without limit, paid on every question) and rejects building a
  code first retrieval system before the manual procedure has proven its shape.
  Search for: `repository shape`, `three file split`, `boot path`, `ADR`, `embeddings`.

- **[[0002-evidence-ruler]]** ✅ accepted 2026-08-13. Why software needs its own ruler rather than
  adopting Yaron's unchanged, and the rule that matters most in it: a model's output, including Zed's
  own, is tier D until confirmed. Resolves how the ruler is applied: tier on the note plus a per claim
  tag only when a claim diverges, recheck on major version or on use and never on a calendar, tier D
  recorded in the discard log only.
  Search for: `evidence ruler`, `tier D`, `model output`, `recheck trigger`, `discard log`.

- **[[0003-knowledge-storage]]** ✅ accepted 2026-08-13. Answers whether the agents should move from markdown
  to a database, Neo4j specifically, as they expand. **Files stay the source of truth and any index is
  derived, rebuildable and disposable.** The argument: the runtime reads files natively so a database
  turns every read into a tool call that can fail or be skipped, git is already the audit trail, a
  graph is bad at storing long prose, the real pain is validation rather than retrieval, and the two
  options are not equally reversible. Separates the knowledge base from the orchestrator GUI's own
  storage, which is genuine database work and gets its own decision.
  Search for: `source of truth`, `derived index`, `Neo4j`, `graph`, `database`, `reversibility`,
  `validator`, `projection`, `GUI storage`.

- **[[0004-local-first-inference]]** ⏳ **proposed.** What it costs to run a local model on this
  machine, and the architecture that follows. The mechanism nobody mentions: **generation is memory
  bandwidth bound and prefill is compute bound**, and on four cores with no GPU the prefill dominates,
  so reading a 21,000 token boot path costs minutes of silence per question and no smaller model fixes
  it. The answer is to retrieve instead of read, and to split the jobs by what each model is good at,
  with speech, routing and extraction local and judgement on a frontier model when online. Carries the
  consequences for how notes must be written, and the four measurements that have to replace its own
  estimates before anything is built.
  Search for: `local model`, `llama.cpp`, `prefill`, `memory bandwidth`, `tokens per second`,
  `quantized`, `role split`, `retrieval`, `prompt cache`, `offline`, `hardware`.

- **[[0005-wake-with-the-constitution]]** ⏳ **proposed.** Answers whether the agents should read their
  whole base once at startup. The technique is real and is called prefix caching, and it costs three
  things: **KV cache RAM at about 144 KB per token**, so three agents holding their full bases would
  need 9 GB; an attention tax paid on every generated token forever, not once at boot; and silent
  recall failure, because a small model with 21,000 tokens in front of it does not know the base, it
  has it nearby. The split: the **constitution** stays resident (identity, rules, thin map, budget of
  4,000 tokens) and the **library** is indexed by code at startup and retrieved per query. Routing
  starts as a keyword lookup against the `Search for:` lines with no model at all.
  Search for: `prefix caching`, `KV cache`, `resident context`, `constitution`, `token budget`,
  `routing`, `retrieval`, `lost in the middle`, `wake up`, `startup`.

- **[[0006-language-architecture]]** ⏳ **proposed.** Whether the fleet should be one language or many,
  for software meant to be used by people in other cultures. Separates the three things that get called
  "the language": the **core** prose, the **keys** the router matches, and the **edge** conversation.
  Same pattern as UTC for time and minor units for money: **normalise at the boundary, one canonical
  representation in the core.** English as canonical, per base rather than per fleet, because the
  jargon is already English and does not translate, which makes mixed language input an advantage
  rather than a problem. The keys become multilingual by a **cascade**: free alias lookup, then local
  query expansion only on a miss, then embeddings only when keywords stop covering it, with the
  expansion log as the worklist for fixing the keys.
  Search for: `language`, `multilingual`, `canonical`, `alias table`, `query expansion`, `embeddings`,
  `cascade`, `i18n`, `jargon`, `normalise at the boundary`.

- **[[0007-memory-architecture]]** ✅ accepted 2026-08-13. The write pipeline, taken from what mem0,
  Letta and Zep do well and refusing what they do badly. **Four named outcomes on every write, ADD,
  UPDATE, DELETE and NOOP**, where NOOP is the branch whose absence produced 52.7% of the junk in the
  mem0 audit. **The agent may delete**, because a delete gate produces hoarding and git makes deletion
  visible, attributable and recoverable, so the constraint is disclosure rather than permission. Then
  **provenance and stage as front matter**, orthogonal on purpose, with the rule that an `agent` claim
  is never promoted to `human` or `external` without a human act. Plus labelled constitution blocks
  ordered by stability so prefix caching survives a project switch, content hash reindexing, and the
  first dependency taken deliberately with the reason recorded.
  Search for: `ADD UPDATE DELETE NOOP`, `NOOP`, `provenance`, `stage`, `write gate`, `delete`,
  `hoarding`, `memory blocks`, `content hash`, `SQLite`, `first dependency`.

- **[[0008-single-user-open-source]]** ✅ accepted 2026-08-13. Build for exactly one self hosted user,
  release it open source, and keep the paid hosted service possible without building any of it now.
  **A is a directory and B is a database**, so files now and a service later is an afternoon, while a
  service now and files later is an export and an apology. Lists what building for one user actually
  means (no accounts, no tenancy, no telemetry, config as a file) and the three things done now that
  keep the service cheap later, each of which is good practice for the single user case anyway. Ends on
  the position that **the paid product is convenience, not capability**, because anyone who wants what
  we built can clone it.
  Search for: `open source`, `self hosted`, `single user`, `multi tenant`, `hosted service`,
  `kb init`, `no telemetry`, `one way door`, `convenience not capability`.

- **[[0009-gui-runtime-boundary]]** ✅ accepted 2026-08-16 for the stack, proposed for the contract.
  Tauri confirmed, but **the boundary is the API contract, not the framework**, which is what makes the
  hosted service a second implementation instead of a rewrite. Records the finding that **no third
  party application can hold a consumer subscription credential**, and the three runtimes that follow
  from it: local, frontier by API key, and driving an agent client the user authenticated themselves.
  Shows that `blocks.txt` pays twice, because the Anthropic API caches on the same prefix rule as the
  local model, cutting the per question cost roughly in half, and turns that into a requirement on the
  contract. Voice deferred with the seam built: typed content parts and one empty trait.
  Search for: `GUI`, `Tauri`, `API contract`, `subscription`, `API key`, `runtime`, `prompt caching`,
  `cost per question`, `keychain`, `voice`, `STT`, `TTS`.

- **[[0010-memory-as-mcp-server]]** 🟡 proposed 2026-08-16. `kb` gains an **MCP server mode**, so any
  MCP capable client can call our routing and memory while the user's own subscription pays for the
  model. **Answers the subscription problem sideways**: the credential never comes near us because it
  never needs to. Names the tool surface, makes `reason` a required parameter on the write tool so
  ADR-0007's disclosure rule is enforced by the schema, and confines the server to the base path it was
  launched with, because tool arguments arrive from a model. Redefines what the GUI is for: local model
  management, the write review loop, provenance made visible, agent switching and voice, not the chat.
  Search for: `MCP`, `MCP server`, `tool surface`, `third party client`, `distribution`, `wedge`,
  `path confinement`, `kb_route`, `kb_retrieve`, `kb_remember`, `kb_write`.

- **[[0011-fleet-layout]]** 🟡 proposed 2026-08-16. **The least reversible decision so far**, because
  every agent created from now on has this shape and the hosted service inherits it as its tenancy
  model. Separates two layers that ADR-0009 collapsed: **what the library accepts** (any path) from
  **where the product creates and looks** (one defined place). The argument that settles it is that an
  orchestrator able to create an agent has to know where to put it, and "anywhere" is not an answer.
  Defines the fleet tree, the agent shape taken from what the three already converged on, `agent.txt`
  kept out of the constitution because `blocks.txt` orders by stability, and **one index per agent**
  because a missing predicate cannot leak a file that is not in the database you opened. The load
  bearing rule is that **no absolute path exists inside the fleet**, which is what makes moving, backup,
  sync and one-tenant-one-directory all the same operation.
  Search for: `fleet`, `fleet root`, `agent shape`, `layout`, `agents/`, `fleet.txt`, `attach`,
  `kb init`, `per agent index`, `tenancy`, `no absolute paths`, `structure`, `repository`,
  `two repositories`, `public repository`, `private`, `gitignored`, `repositorio`, `publico`.

- **[[0012-naming-and-hosting]]** 🟢 accepted 2026-08-17. **The system and its orchestrator are both
  called Vesta**, and everything ships under Richard's own name at `richardwollyce.com`, one
  subdomain per system. Named by Steve from his own base, which is the first time an agent here
  answered a real question with its own distilled material rather than with general model knowledge.
  Two findings drove it: **Wollner's rule** that a mark must be abstract and never literal, which
  kills Router, Hub, Conductor and Nexus on sight, and the discovery that the mythological namespace
  for AI orchestrators is already occupied. Vesta is the Roman deity who had **no statue**, being
  represented by the fire itself, and whose Vestals guarded Rome's wills: the index is the fire, the
  markdown is the wills, which is [[0003-knowledge-storage]] restated as a name. A personal name is
  the umbrella because **a civil name has no third party who can hold prior rights to it**, which
  puts trademark spending after there is something worth defending rather than before. Records three
  accepted costs, including that the brand does not transfer on a sale and that Vesta is one phoneme
  from *besta*.
  Search for: `Vesta`, `name`, `naming`, `brand`, `branding`, `Wollyce`, `richardwollyce.com`,
  `domain`, `subdomain`, `trademark`, `INPI`, `orchestrator name`, `Wollner`.

- **[[0013-retrieval-precedes-classification]]** 🟢 accepted 2026-08-17. **The base is read before the
  model is allowed to decide it was not needed.** Routing is a reflex rather than a decision, because
  it costs microseconds and reads no text, and the model classifies only after seeing what came back.
  Rejects putting a model in front of retrieval on the ground that **whether it needs the base is the
  one decision a model is worst at**: it does not know what it does not know, and a question with a
  good generic answer and a different house answer comes back generic with full confidence. Carries
  the measurement that forced it: twenty real questions went **10 hits, 3 weak, 7 misses** before
  repair and **17, 1, 2** after, with the fused scorer rescuing zero of the seven, so the defect was
  never the ranking. Names the treadmill honestly, since the repair was twelve alias lines and twenty
  keywords matching the exact phrasings tested, and admits the second number is tuned against its own
  test set. **Blocks taking the map out of the resident set** until the expansion step exists.
  Search for: `routing`, `router`, `classification`, `cascade`, `recall`, `recall loss`, `silent
  miss`, `expansion`, `orchestrator`, `Vesta routing`, `who answers`, `general knowledge`, `typo`,
  `roteamento`, `classificar`.

- **[[0014-english-system-any-language-conversation]]** 🟢 accepted 2026-08-17. **The system is
  written in English and the conversation is not.** One language for identifiers, comments, markdown,
  commits and interface copy; whatever the user types stays in the language they typed it. Beat the
  split rule because the cost of a split is not the translation, it is **the boundary**, and a rule
  that needs a judgement per string drifts, which it already had between two decisions four days
  apart. Records what the audit found: `tools/kb` was already clean because its Portuguese is *data
  about Portuguese*, the tray was not, `fn shit` existed, and Steve's quoted creative stays Portuguese
  because the exact wording is the evidence. Private layers deliberately untouched.
  Search for: `language`, `English`, `idioma`, `normalise`, `interface copy`, `UI strings`,
  `translation`, `language audit`, `traduzir`, `one language`.

- **[[0015-expansion-split-by-distance]]** 🟢 accepted 2026-08-17. **Step 2 of the cascade, split by
  what kind of distance a miss actually is.** Orthographic distance, a typo or a cognate, is plain
  software: character trigram overlap against the 849 keyword terms the base already holds, in
  microseconds with no dependency. Semantic distance, a real translation, needs a model, and one is
  already in the loop, so **a miss now returns the candidate vocabulary instead of a dead end** and
  the model expands against words that exist rather than guessing. Rejects building the local model
  version first on a measurement: generation here is 5.55 to 5.88 t/s, so the specified 20 to 40 token
  expansion costs **3.4 to 7.2 seconds against 4.43 ms for the whole of `kb_retrieve`**, paid on the
  path where the question already failed, and it would cost the README's one dependency claim. The
  deciding argument is honesty rather than cost: trigrams can state their own limit exactly and a
  model's expansion cannot. Demonstrated by `ingestao` reaching `ingest a source` with no alias line.
  Carries the wrong turn: requiring every word of a multi word key to align was borrowed from matching
  and is wrong for suggesting, where the reader filters.
  Search for: `expansion`, `step 2`, `suggestion`, `suggest`, `trigram`, `fuzzy`, `typo`, `cognate`,
  `miss`, `nothing matched`, `candidate terms`, `local model`, `expansao`, `sugestao`, `erro de
  digitacao`.

- **[[0016-writing-a-note-includes-its-entry]]** 🟢 accepted 2026-08-17. **A note and the map
  entry that makes it reachable are one write, and there is no flag that does one without the other.**
  Closes the bootstrap: ADR-0007 built the proposing half and nothing ever turned an approved proposal
  into a file, so the base could only grow by hand and did not. Richard named the consequence, that a
  fast retrieval over a corpus nothing fills is optimising the empty case. Rejects letting the linter
  catch the missing entry, because a note with no entry cannot be ranked by the keyword scorer at all
  and "the meantime" is unbounded, and rejects generating keys from the note text, because keys exist
  to bridge how somebody **asks** to how the file was **written** and deriving them from the file
  guarantees the half we already have. Carries the defect the end to end test caught: writing was not
  enough, because `kb` reads `git ls-files` and an untracked note is one the router refuses to serve,
  so the write stages and does not commit.
  Search for: `write`, `kb write`, `escrever`, `gravar`, `new note`, `create a note`, `bootstrap`,
  `empty base`, `staging`, `git add`, `map entry`, `unreachable note`.

- **[[0017-no-dense-scorer-yet]]** 🟢 accepted 2026-08-17. **BGE-M3 measured against twenty real
  questions and not adopted, because fusing it made the system worse.** Head to head it got 13 right
  against roughly 16 for the keyword scorer, embedding note bodies chunked the way `store.rs` chunks
  them. That alone would not disqualify it, since **RRF does not need a second scorer to win, it needs
  it to be wrong about different questions**, so the fusion was computed directly: nineteen of twenty
  answers unchanged, the one that changes moves off the correct file onto a marketing reel, and the
  question the keyword scorer honestly does not answer starts returning a carousel about business
  terminology. **An honest abstention became a confident wrong answer.** Carries the finding that
  outlives it: the keyword scorer's one error scored 3.82 while every correct answer scored 9.55 or
  more, and BGE-M3's correct and wrong answers overlap completely, so a dense scorer that never
  abstains removes the property that separates this from mem0 and Zep. Costs measured: 2.2 GB, and
  2,833 seconds to index 1,039 chunks on this machine.
  Search for: `embedding`, `embeddings`, `dense`, `BGE-M3`, `bge`, `e5`, `vector search`, `semantic
  search`, `fastembed`, `ONNX`, `hybrid retrieval`, `RRF`, `fusion`, `abstention`, `busca semantica`,
  `vetorial`.

- **[[0018-no-model-in-the-retrieval-path]]** 🟢 accepted 2026-08-18. **No model enters the
  retrieval path, and the keyword score floor becomes the abstention mechanism.** The full table:
  keyword 16/19 and the only scorer whose score separates hit from miss; BGE-M3 dense 8 and 13,
  sparse 10 on entries collapsing to **4 on bodies** where the largest file absorbs other agents'
  questions; both cross encoder rerankers **degraded the 16-correct ranking they were handed**, BGE
  v2-m3 to 11 at 2,571 ms per pair, Jina v2 to 14 at 647 ms with a non-commercial licence. A skeptic
  commissioned against the reranker plan predicted zero marginal gain before the numbers existed, and
  the reality was negative. Orders the next code change: carry the keyword score and runner-up margin
  through retrieval instead of discarding them at fusion, and gate on the floor. Instrument is
  `tools/bench`, one Rust command, re-run at 1,000 files.
  Search for: `reranker`, `rerank`, `cross encoder`, `abstention`, `score floor`, `confidence gate`,
  `no model`, `retrieval path`, `kb-bench`, `jina`, `bge-reranker`, `sparse head`, `measurement`,
  `modelo na busca`, `limiar`.


- **[[0020-vesta-routes-to-the-agent]]** 🟡 proposed 2026-08-18. **Vesta chooses which agent
  answers, and the two scorers stop being treated as two attempts at one job.** The table that
  decides it: the keyword scorer alone picks the right file 18/19 and the right agent 16/19, the
  RRF fusion it feeds picks 11/19 and 13/19, and the hardcoded line naming one agent picks 10/19,
  which is the best a fixed choice can do on this set. Mechanism, and it is RRF working as
  designed: fusion rewards agreement, so a file both scorers rank fourth beats a file one scorer
  ranks first, which is right for assembling passages a person reads and wrong for choosing one
  owner. So **the owner and the verdict come from intent, the reading comes from agreement**, both
  from one call. The gate is the score floor alone at 6.0: hits 9.29 to 179.24 against misses at
  0.00, flagging 1/1 of its own misses and demoting 0/18 correct answers. **Carries a refuted
  prediction of mine**, that the scale free runner-up margin should be the primary gate, killed by
  the instrument on its first run because correct answers have margins from 1.00 to 7.00 and a 1.5
  cut threw away twelve of eighteen. Also carries the finding that the README's "about a
  millisecond" is wrong by roughly 9x: routing costs 8.6 to 10.5 ms because `index::route` rebuilds
  the entry list and the document frequency table on every query. Instrument is `kb eval`, in `kb`
  and not in the bench crate, so anyone who can build the tool can re-check its numbers.
  Search for: `routing`, `agent selection`, `which agent`, `coordinator`, `orchestrator`,
  `kb eval`, `gold set`, `evaluation`, `score floor`, `abstention`, `confidence gate`, `margin`,
  `RRF`, `fusion`, `top-1`, `hardcoded`, `boot`, `qual agente`, `roteamento`, `medicao`.

- **[[0021-committing-under-concurrency]]** 🟢 accepted 2026-08-18. **A commit names the paths
  it commits, and proves afterwards that it took nothing else.** Written because more than one
  agent session writes these repositories daily and commit `cdc0e52` already carried two
  sessions' unrelated work under one message, staged by `git add -A` while the other was mid
  write. The damage is not lost work, it is an audit trail that lies. The primitive, verified in
  a scratch repository rather than recalled: **`git commit -- <paths>` builds the commit from only
  those paths and ignores the rest of the index**, so a pathspec closes the race instead of
  guarding it and no lock is needed. Also measured: an untracked path fails a pathspec commit and
  needs `git add` first, a deleted path does not, and index contention is exit 128 with
  `Unable to create ... index.lock`, which is a bounded retry. Ships `kb commit`, which resolves
  the repository from the paths, refuses a list spanning two repositories, refuses an empty list
  because that one affordance is what reintroduces the bug, and **reads the commit back** to
  report anything it absorbed plus every path it left dirty. Plus a tracked `pre-commit` hook that
  refuses raw commits, with a deliberately visible `KB_ALLOW_RAW_COMMIT` escape hatch. Rejected
  per session worktrees, the industrial answer, because isolating the sessions isolates the fleet
  the router is supposed to read across. Does **not** solve two sessions editing one file, and
  says so.
  Search for: `commit`, `git commit`, `concurrency`, `concurrent sessions`, `multiple agents`,
  `git add -A`, `pathspec`, `index.lock`, `pre-commit`, `hook`, `core.hooksPath`, `kb commit`,
  `sweep`, `audit trail`, `worktree`, `lease`, `commitar`, `sessoes simultaneas`.

- **[[0022-the-fleet-boots-the-agent]]** 🟢 accepted 2026-08-18. **The fleet boots the agent,
  the agent does not boot itself.** Richard's framing and it is the decision: a runtime gives us
  a model, and how that model runs in the workspace is our software's call. The old mechanism was
  a static conditional in `CLAUDE.md` naming one agent literally, so a nutrition question still
  woke the architect, and the identity choice sat with whichever model parsed the sentence.
  Replaced by `kb boot` on a **`UserPromptSubmit` hook**, which is the only surface in this
  runtime where our software speaks before the model does: for that event and two others, and no
  others, stdout is injected as the model's context. It routes with `Memory::ask`, emits the
  winning agent's constitution **only when the routed agent changes** (tracked per session, since
  the constitution is ~55 KB), emits the roster and refuses to pick when below ADR-0020's gate,
  reads only what git tracks, and **always exits 0** because exit 2 on this event erases the
  user's message. Rejected an MCP tool as the same failure as prose in a better costume, since a
  tool is still invoked at the model's discretion. Costs **355 ms per message**, isolated by
  experiment: 60 ms process start, 280 ms opening five bases, 5 ms routing, so the router is not
  what makes it slow and a resident server is the fix when it is worth one.
  Search for: `boot`, `hook`, `UserPromptSubmit`, `who am i`, `identity`, `which agent answers`,
  `agent selection`, `constitution injection`, `session`, `plugged in`, `runtime`, `settings.json`,
  `CLAUDE.md`, `orchestration`, `quem responde`, `qual agente`, `roteamento automatico`.

- **[[0023-the-phone-and-the-envelope]]** 🟡 proposed 2026-08-19. **The desk streams over SSE
  carrying the load bearing third of Letta's envelope, and the phone is four decisions, not a
  feature flag.** The envelope decomposed into separable tiers: typed events and sequence numbers
  shipped (sixty lines on the existing HTTP server), while dedup and sync-replay wait for a client
  that can actually lose messages, which a loopback browser cannot. The phone half is an agenda
  rather than a build: a LAN bind is a new exposure class needing per-request tokens (Letta's
  loopback-trusted rule copied whole), an installable PWA requires a secure context that plain LAN
  HTTP never grants (page yes, service worker never), a reachable desk can spend the plan from any
  device holding the token, and phone Wi-Fi is where sync-replay stops being scaffolding. Deferred
  together, on the trigger that Richard asks again.
  Search for: `phone`, `celular`, `PWA`, `websocket`, `SSE`, `envelope`, `event stream`,
  `streaming`, `LAN`, `bind`, `token`, `secure context`, `service worker`, `remote access`,
  `acesso remoto`, `mobile`, `desk transport`.

- **[[0024-the-person-is-one-base]]** 🟢 accepted 2026-08-19. **The person is one base every
  agent reads, and never an agent that answers.** Written after the desk answered "quem sou eu"
  with "the base does not cover it", which was half wrong and the right half was worse: the same
  human was recorded twice, in two languages, in two private folders, while two agents had no
  user block at all. The cost was paid in public the same day, when the router woke the marketing
  agent to answer a question about Richard's own site and CV and that agent could not read the
  file saying personal presence is half his twelve month goal. Richard argued for a global file
  over per-agent scoping and was right, for a mechanism reason: **N copies of a person drift, and
  the gaps are invisible from inside any one agent.** The correction his version needed is that
  **global is about ownership, not residency**, so the person is one base with a small resident
  core and retrieved domain files. `fleet/profile/` has no `agent.txt`, so the router reads it and
  can never elect it, because a person is not an agent. It is tracked, unlike the per-agent
  profile folders, because **Vesta refuses to serve what git does not track, so an untracked base
  is unroutable by construction**. Measured: the architect's resident payload fell from 9,689 to
  2,708 bytes while three agents gained the file. Carries the class of error it exposed, that a
  profile assembled by asking about goals misses the job.
  Search for: `who am i`, `quem sou eu`, `user`, `usuario`, `perfil`, `profile`, `persona`,
  `richard`, `human`, `humano`, `global file`, `arquivo global`, `escopo`, `scope`, `user block`,
  `resident`, `shared base`, `base compartilhada`, `identity`, `identidade`.

- **[[0025-the-shape-is-public-the-person-is-not]]** 🟢 accepted 2026-08-19. **A fleet must
  declare its human, and the shape of that declaration is the part that can be published.**
  Richard's rule, and he named the precedent himself: the agents are his, but how an agent is
  shaped and how they relate is documentable and eventually public, while his own file must never
  transmit what is written about him. So the same split the fleet already makes for agents is made
  for the person. Public: `kb init --person`, the committed `person-skeleton/`, and the structural
  rule. Private: `fleet/profile/`, every word. The part that makes it structural rather than
  advisory is that **`kb init` now writes a `[user]` block into every agent it creates**, so a new
  agent cannot be born not knowing who it works for, which is exactly how Steve came to answer a
  question about Richard's CV without knowing his name. Templates ship **empty with questions in
  them**, because an empty profile is not neutral: an agent that does not know its human gives
  generic answers confidently. Rejected putting the profile inside the agent skeleton, which would
  re-teach the per-agent duplication ADR-0024 removed. **The general form: publish the shape, keep
  the content.** Leaves the licence question open, deliberately.
  Search for: `open source`, `publico`, `public`, `publish`, `licenca`, `license`, `skeleton`,
  `person skeleton`, `kb init`, `privacidade`, `privacy`, `private line`, `shape`, `estrutura`,
  `declarar usuario`, `declare the human`, `template`, `drift test`.

- **[[0026-a-wikilink-stops-at-the-base-edge]]** 🟢 accepted 2026-08-19. **A wikilink resolves
  inside its own base and nowhere else**, in the linter and in the reading room alike. Written
  because moving a file between bases exposed the two disagreeing: `kb check` scoped links to one
  base while the reading room resolved fleet-wide, so the same file was simultaneously fine and
  broken. **Privacy decided it, not tidiness:** a base is where privacy is decided here, the
  decision records reached the public root only after an audit that converted eight files' worth
  of wikilinks whose targets stay private, and fleet-wide resolution would make that audit
  permanent work. Two further arguments: two bases can hold the same stem, so fleet-wide has no
  correct answer only a discovery order; and a base may be opened alone, so a link that resolves
  only when a sibling is mounted is a coincidence of mounting. Crossing a base is now **written
  out as a path**, which rebuilt the stacks' ribbons on an honest source and revealed they had
  been measuring nothing. The linter names the other base in the error, so the rule is teachable
  rather than merely enforced.
  Search for: `wikilink`, `link`, `broken link`, `E01`, `base scope`, `escopo`, `cross-base`,
  `resolucao`, `resolve`, `privacy boundary`, `fronteira`, `ribbon`, `fita`, `stacks`, `Z38`,
  `convention`, `convencao`.

- **[[0027-a-model-decides-who-answers]]** 🟢 accepted 2026-08-19. **Choosing an agent is
  classification, so a model does it.** Written after four sessions of routing failures and
  Richard's demand that the system work independently of what is in the bases. The answer was
  already in this repository: ADR-0013 says *classification is the model's job and lookup is the
  code's job*, and choosing an owner had been built as a sum of IDF weighted keyword scores, then
  patched for three days with stopword lists, aliases, an incumbent margin and a share
  normalisation, most of them measured and removed. **The patching was the symptom.** Retrieval is
  lexical and a keyword index answers it; *who understands this subject* is semantic and no count
  of shared words answers it, which a subject nobody has written about proves: the word **zero**
  in "zero downtime" gave the marketing agent 100% of the field. Retrieval is unchanged; the
  classifier gets a **dossier** of roster plus evidence, never the corpus, so it cannot invent a
  file. Agents now declare `ends =`, because a roster of roles says what each agent does and never
  what none of them does. **Coverage is a first class answer**: covered, adjacent, uncovered, so
  the fleet can say nobody here does this and name the nearest, which is the input to creating an
  agent. Contract is a process, dossier on stdin and verdict on stdout, so any model behind any
  runtime satisfies it. Carries a **built and rejected cascade**: gating the model on the
  deterministic score cut latency from 14s to 1s and routed DevOps to marketing in 971ms, because
  a gate built on a blind signal inherits the blindness. Costs 13 to 16 seconds per message, stated
  and not hidden.
  Search for: `classifier`, `classificador`, `roteamento`, `routing`, `who answers`, `quem
  responde`, `agent selection`, `escolha de agente`, `coverage`, `cobertura`, `devops`, `gap`,
  `novo agente`, `new agent`, `dossier`, `ends`, `edges`, `bordas`, `cascade`, `cascata`,
  `latency`, `latencia`, `model in the loop`, `modelo`.
