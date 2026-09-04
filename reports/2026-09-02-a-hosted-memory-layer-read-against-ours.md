# the hosted product and Ulpia: two memory layers, two constraints

**Date:** 2026-09-02
**Subject:** a hosted consumer memory product for social video on iOS, read on 2026-09-02 against Ulpia's public repository. The vendor is not named here, and the sources are on file.
**Method:** everything about the hosted product comes from pages that were fetched and quoted, plus two unauthenticated requests to their published MCP endpoint, both answered 401. Nobody signed in, nobody installed the app, nobody attempted a write. Everything about Ulpia comes from tracked files in this repository, read at this commit. Nothing under `fleet/` was read into this document.

**The rule this document is written under:** a sentence is EVIDENCE when it was read on a page and is quoted with its source. Everything else is INFERENCE and is labelled. A claim about a closed product that could not be sourced is marked NOT SOURCED and left as a hole rather than filled. Section 8 collects every inference in one place so nobody quotes one back as a fact.

---

## 1. What each one is, and why the comparison is worth making

the hosted product is an iOS app that turns social video into searchable text. You share a Reel, a TikTok or an X post from inside the app you were already using, and their backend fetches the media, transcribes the audio, reads the text on screen, writes a summary, picks a category, computes an embedding, discards the media and keeps the derived text plus the URL. Later you ask a question in plain language and it answers with the posts the answer came from. It is free to install with a weekly allowance, and a paid tier unlocks a hosted MCP connector that gives an AI client four read operations over the same archive. It shipped its 1.0 in August 2026 and reached its fourth patch inside three weeks, and it is one person with a single support address as the entire organisation.

Ulpia is a local first memory layer for a fleet of agents. Markdown files in folders are the source of truth, `.kb/` beside them holds a derived and disposable SQLite index, and a hand written `Search for:` line at the top of each note is what makes it reachable. A question is scored against those keys with idf weighting, fused with BM25 over the chunks by Reciprocal Rank Fusion, and the result carries a verdict: hit, guess, or nothing. It is Apache 2.0, one dependency, no network at runtime, no accounts, and its agent surface is a stdio MCP server with four read tools and deliberately no write tool.

The comparison is worth making because the two products are the same shape and opposite bets. Both take an artefact, derive a text layer from it, index the derived layer, keep a pointer back to the original, and hand the result to a model. Both made the agent surface read only and both said so out loud. And then they part on the one question that decides everything downstream: **who writes the thing the query is matched against.** the hosted product spends model calls so that nobody has to write anything. Ulpia spends a person's thirty terms per note so that no model has to run. Richard's own framing of the difference is right and this document confirms it in detail: he built the app, the summaries, the useful things around the memory; we built the memory and comparatively little around it. What follows is what each of those choices bought and what it cost.

One asymmetry to hold throughout, because it runs in their favour and it cannot be corrected away: their product is closed. Their marketing site is fifteen URLs, their MCP `initialize` returns 401, and their app was not installed. So this study reads their public surface against our source code, and a public surface is chosen while source code merely exists. Where a row says NOT SOURCED, that means we could not see it, not that they do not have it.

---

## 2. The table

| Dimension | Theirs | Ours | Verdict |
|---|---|---|---|
| What the query is compared against | An embedding step exists (EVIDENCE: "embed" is one of six verbs in their privacy policy) and search is sold as meaning based (EVIDENCE, /mcp: "A search by meaning across the items you saved"). That the ranking is cosine over vectors is INFERENCE. Which fields the index covers is NOT SOURCED. | Exact token equality after `index::normalise`, weighted by idf: keys at W_KEYWORD 6.0, phrases at W_PHRASE 10.0 times mean idf, title and stem at 3.0, summary prose at 1.0, fused with BM25 by RRF at RRF_K 60.0 | A different trade |
| Who authors the retrieval handle | Nobody is asked to. The save is two taps (EVIDENCE, support) and the derived fields are machine written (EVIDENCE, privacy). Users can add notes and folders; whether those enter the index is NOT SOURCED | A person, at write time, with no flag to skip it. `WriteError::NoKeys` at write.rs:143; W06 warns under twelve keys and asks for thirty in both languages | Theirs is better for their input; ours is coherent with a lexical ranker |
| Abstention: can it say "I do not have that book" | NOT SOURCED at the retrieval layer, and this is a checked absence: /mcp describes four capabilities and no empty state, and 401 hides the result schema. What is EVIDENCE sits above retrieval, in the terms: results "may be incomplete, inaccurate, or outdated" | A first class field. `confidence_of` returns Nothing when the keyword list is empty, Hit at or above `floor_for(N)` with at least two entries, Guess otherwise | Not comparable on public evidence: ours is measured, theirs is unsourced |
| Explainability of the ranking | Item level provenance, claimed (EVIDENCE, App Store: "shows you the posts the answer came from"). Term level attribution NOT SOURCED, and INFERENCE that a cosine score has no words to show | A fixed EVIDENCE line on every reply: keyword score against the floor that applied, whether both scorers agreed, the margin over the runner up, and the verdict | A different trade |
| Cost per item and per query | A multimodal model per save, plus a scraping vendor, plus an embedding. Visible in product shape (held links, Pro gate) rather than in any published number | Zero models at ingest, zero at query. Warm p50 0.68 ms in process on the demo base; `kb route --json` spawn plus answer p50 184.8 ms on a Windows laptop, 9.6 ms on Linux WSL2 on a 9 entry base | A different trade |
| Capture friction | Two taps from inside the source app, after a one time share sheet setup taught on first launch, plus a mandatory account | `kb write` with an agent, a slug, `--keys` and a body on stdin, or a `.md` dropped into `<agent>/inbox/` and the SessionEnd hook | Theirs is better on capture friction, for an input type we do not accept |
| Where the model sits | Ingest and retrieval both, as a generator (EVIDENCE, privacy: transcribe, describe, summarize, categorize, embed, answer) | Ingest only, and only as a gate that degrades toward writing nothing. `kb_retrieve`'s own description: "no model is involved" | A different trade |
| Provenance of the derived layer | Carried by field name, plus one global disclaimer in the terms. Whether the item view labels maturity is NOT SOURCED | Two orthogonal front matter fields checked by the linter: provenance in {human, agent, external}, stage in {raw, captured, distilled, derived} | A different trade |
| What happens to a query that found nothing | NOT SOURCED. They instrument the app (EVIDENCE, privacy: PostHog, "Session replay is disabled") but no page describes an empty search or a log of one | The question is appended to `kb-misses.txt`, deduplicated, counted, most asked first, with what the base offered back. Gitignored, never committed | A different trade |
| Survival of a lost or stolen machine | The archive is server side and held against the account (EVIDENCE, privacy: "We keep account and archive data while your account is active") | No backup story at all. `fleet/` is its own git repository and whether it was ever pushed is the owner's business | Theirs is better |
| Getting the archive out | No export is named on any public page (checked absence on the website). A portability right by email exists (EVIDENCE, privacy, LGPD) | The files are the export. Markdown in folders; delete `.kb/` and you lost a rebuild, not a fact | Ours is better on published portability; theirs is the side with a legal right written down |
| How many parties see the content | At least four named, and both lists say "including", so it is a floor: Convex, Apify, Google Gemini, OpenAI | Zero to one on the retrieval path, and the user picks it. Noted honestly: `ui.rs` spawns `claude -p` for its chat surface, so the reading room sends messages to Anthropic | A different trade. We spend no exposure because we buy no capability with it |
| Read only agent surface, and how enforced | Declared in a machine readable place: `scopes_supported` declares exactly one read scope and the 401 challenge names the same scope. Whether a write is refused to a valid token is NOT SOURCED | Enforced by there being no write tool, checkable by reading `mcp.rs`. One write disclosed: a refusal appends to `kb-misses.txt` | Theirs is the stronger disclosure; both are declarations, neither is a tested behaviour |
| Physical reach of the connector | One hosted HTTPS endpoint, five client families claimed (EVIDENCE, /mcp). INFERENCE: reachable by clients that cannot spawn a local process | stdio only. Our own README says claude.ai in a browser cannot use it and why. We reach a client driving a local model with the network off | A different trade |
| Path to a first user | The App Store. Free install, one tap sign in, share sheet taught on first launch | `cargo build --release` with a C toolchain, or download a release asset, know the Linux one is musl, verify a checksum, then write an absolute path into a config file | Theirs is better |
| Where the failing number lives | No conditioned number on any public page. Two unconditioned ones: a storefront rating from a two-figure number of reviews, and a lifetime item count | Every figure beside its command, machine and date. README.md:92 publishes the eval failing: "OVERLAPS: no floor tells a hit from a miss on this set" | A different trade, with one narrow point in our favour: we publish the number that embarrasses us |

---

## 3. Where they converge, and whether for the same reason

### Same shape, same reason

**The derived layer is not the artefact, and a pointer home is kept.** Theirs, EVIDENCE (privacy): "Temporary media may be downloaded long enough to extract text or visuals and is then discarded. What we keep is primarily the URL, derived text, metadata, and any thumbnail." Ours, from the root README: "The index is derived and disposable: delete `.kb/` and you have lost a rebuild, not a fact." **The shape matches and the reason does not.** They discard the media because holding it is expensive and legally exposed, which their terms gesture at with "we do not operate a public pirate library or re-host original videos for redistribution." Ours is disposable by decision, ADR-0003: "Files stay the source of truth. Any index is derived from them, rebuildable from scratch, and never authoritative." So the derived layer protects them from a hosting cost and protects us from a lock-in, and only one of the two can be regenerated from what the user still holds.

**The sources come back with the answer, and the generated layer above them is disclaimed.** Theirs, EVIDENCE (App Store): "the hosted product answers, and shows you the posts the answer came from", plus terms: "Always open the original source when accuracy matters." Ours: `kb_route` returns ranked file paths with the words that matched and no file contents; `kb_retrieve` returns passages with heading path and provenance. Same reason in two vocabularies: a generated answer that cannot be checked is worth less than a pointer that can. One asymmetry inside the agreement, and it is INFERENCE about the consumer rather than a sourced claim: a person verifying a Reel thumbnail checks in one glance, so item level citation is probably enough for them, while our caller is a model deciding how much weight to put on a passage, which is why our citation carries a score.

**The agent surface refuses writes, and both say so as a negative rather than leaving it implied.** Theirs, EVIDENCE (/mcp): "It cannot save, edit, delete or share", and EVIDENCE that the claim is also declared in machine readable metadata: `scopes_supported` declares exactly one read scope. Ours, from the `mcp.rs` module header: "A write tool reached by a model is a different security surface and gets built deliberately, not as an afterthought while the retrieval side is still warm." **This is the strongest single agreement in the study**, two independent products reaching the same conclusion about what a model may be handed. Their disclosure is the stronger kind and it is worth saying plainly: a scope name in a discovery document is checkable from outside without trusting marketing copy, while ours is checkable only by reading our source. Correction to an earlier draft that called their promise enforced on the wire: that overstates it. The document DECLARES one scope and the 401 names it. No authenticated request was made and no write was attempted, so whether the resource server refuses a write is NOT SOURCED.

**Acknowledgement is decoupled from derivation.** Theirs, EVIDENCE (support): "A save normally shows up on your Home screen within a few seconds and finishes processing shortly after." Ours, README lines 218 to 224: "A note is served the moment it is on disk. The keyword scorer re-reads your files on every run, so step 3 finds the new note with or without step 2. What step 2 buys is the second scorer." Same reason on both sides: the person must not pay the expensive stage's latency at the moment of the gesture. The difference is which stage is expensive, a model pipeline over media for them, an FTS chunk build for us.

**Back-pressure never destroys the input.** Theirs, EVIDENCE (privacy): "Held links are kept so your content is not lost when you hit Free limits." Ours, the hook's own comment on `--max 3`: "the remainder of the deposit is not lost, only left: the next session end takes the next three." The same rule holds one layer earlier on our side: a session no agent was routed in is not captured, and ADR-0035 records that "the record is kept" for a later owner, with a test asserting it.

**A scored search and an unscored enumeration are separate operations.** Theirs, EVIDENCE (/mcp): "A search by meaning across the items you saved" as one bullet and "A list of them by date, platform, type or folder" plus "The names of your folders" as others. Ours: `kb_fleet` returns the roster with no scoring and its description says why. **The split is EVIDENCE on both sides; the reason is ours alone.** No page of theirs says why the list is separate from the search. What can be said flatly: their capability list has four facets against our one enumeration, and ours is the one that states in the tool description the failure it prevents.

**Both name their sub-processors instead of writing "third parties".** Theirs, EVIDENCE (privacy): "for example Clerk, Convex, PostHog, OpenAI, Google, Apify, Apple, RevenueCat", introduced by "for example", so it is a naming and not a closed set. Ours, in `site/frontend/privacy/index.html`: Cloudflare Pages and Cloudflare D1 are named, with "They are named here rather than left for you to discover." The convergence is the posture, not the scale. Eight names is what a content pipeline costs and one name is what an email list costs.

**Both accept that deletion is not erasure, in writing.** Theirs, EVIDENCE (privacy): "After account deletion, we delete or anonymize personal data except where we must retain records for legal, security, or accounting reasons for a limited period", with named residue at Clerk, Apple and RevenueCat. Ours reaches the same admission from the other direction: ADR-0007 makes a delete a commit that "is visible in a diff, recoverable with `git revert`, and attributable". Neither of us can promise the word gone.

**Both are one named individual with an email address as the whole support organisation, governed by Brazilian law, running on United States infrastructure, and both disclose it.** Theirs: their privacy policy names a single natural person as data controller, with a support address and a two business day commitment. Ours: `hello@ulpia.io`, `security@ulpia.io`, and the privacy notice says "Cloudflare is a United States company operating servers in many countries, so your address may be handled outside the country you are in, and outside Brazil." Naming yourself as controller when you could have interposed a shell is the harder and more honest choice, and it is the same choice we made.

### Same shape, different reason, and the difference matters

**Both hold items that exist and cannot be found by search.** Theirs, EVIDENCE (terms): "If you hit Free save limits, the hosted product may store the URL as a held link without fully processing it until you upgrade or your quota resets." Ours: `index::build` calls `header_of` on every walked file and skips it when the keyword list is empty (index.rs:168-172), so a note with no `Search for:` line is stored, readable by a person, and scores zero on every question. **Theirs is a quota decision that two documents name and give a resume path. Ours is an authoring gap and it is silent.** That comparison goes against us on the axis that matters. Our version even carries a measured size in our own source, index.rs:515-517: "Measured over 8580 keys: 3808 in the keyword bag, 4621 in phrases, and 137 distinct keys dead, of which 21 are the negations above and 116 are not, like `AI slop`, `Gen Z` and `Nova York`", reported only as a W07 warning nobody is required to run.

**Both keep a staging area structurally distinct from the durable library.** Theirs is a billing gate: held links, enumerated as their own content category in the privacy policy. Ours is a quality gate: `promote.rs`'s DEPOSIT constant, `inbox/`, with the module header naming what it exists to avoid, the mem0 audit of "10,134 entries accumulated over 32 days, of which 97.8 percent were judged junk". **Theirs holds material back because the user has not paid. Ours holds material back because nothing has judged it yet.**

**Both keep a record of input they could not process.** Theirs keeps the pointer, ours keeps the question. Same refusal to lose evidence of a failure, measuring two different things: theirs is capture side and refused by a quota, ours is retrieval side and found nothing.

**Both scope the AI surface by default and require a deliberate act to widen it.** Theirs is tenant isolation, EVIDENCE (/mcp): "Your account only, plus the shared folders you are in", which exists because other people's archives are on the same infrastructure. Ours is over-sharing to a model, not to another person: ADR-0034 sets `private = profile/, projects/, records/` and `--all` is "a deliberate act visible in the client's config file". Same mechanism, opposite threat model.

---

## 4. Where they diverge, and what each side optimised for

### 4.1 What the query is compared against

**Theirs.** An embedding step exists and the search is sold as meaning based. Which fields the index covers is thinner than it first looks: EVIDENCE covers visual content ("Images and video: Search by what appears, not just the text") and the landing sells four artefacts per save, but no page says the search runs over all four, so reading the feature list as an index field list confuses marketing with mechanism. Whether a lexical index sits beside the vector one is NOT SOURCED.

**Ours.** Exact token equality after normalisation, idf weighted, fused with BM25 by RRF. The `Search for:` lines are excluded from the text index (`store.rs` `is_keyword_line`, `is_keyword_section`) so one scorer wearing two hats cannot look like two scorers agreeing.

**Constraint.** They optimised for an input nobody labelled and a user who must not be asked to: the save is two taps from inside Instagram, the content is somebody else's speech, and the person recalls it by gist. We optimised for a corpus whose author writes the keys, and for a score that has to be defensible afterwards, because **that same score is the abstention instrument.**

By query shape: their product is AIMED at the query whose words do not appear in the source, which ours structurally cannot reach. How well theirs lands is NOT SOURCED, and no page of theirs names an embedding model or claims cross-language retrieval, so three shipped locales are a distribution fact and not a retrieval property. What is ours to state is our own limit: `index::normalise` matches tokens exactly and `index::suggest` is Dice over character trigrams at SUGGEST_FLOOR 0.65, whose reply text says the comparison "is spelling, not meaning, so it finds a typo or a cognate", so `nunca` never reaches `never`, and ADR-0017 accepted that "the cross lingual gap stays open". **Ours wins the rare exact token**, an identifier, a constant, a file stem, an ADR number, because idf makes a term carried by exactly one entry the heaviest thing in the corpus (`index::idf_unique` is `ln(1+N/2)`, 4.74 at 226 entries). That a dense retriever loses that query to a near neighbour is INFERENCE about vector search in general, not a sourced claim about their product. **Ours also wins the query that must match nothing:** memory.rs records that "ok obrigado" scores 0.00 and abstains.

Verdict: a different trade.

### 4.2 Abstention

**Theirs.** NOT SOURCED at the retrieval layer, and it is a checked absence rather than an assumption. INFERENCE, flagged: a cosine ranking always has a top item, so a refusal has to be an explicit threshold somebody chose. A second INFERENCE in their favour: their answer surface is chat over retrieval, and a model can decline in prose even when the retriever underneath cannot.

**Ours.** The verdict is a first class field computed from the keyword list alone. The floor is not a constant: `floor_for(N) = floor_in_unique_keys x W_KEYWORD x idf_unique(N)`, where `floor_in_unique_keys` is `17.5 / (6 x 4.74) = 0.616`. Computed from the code: **4.1 at four entries, 6.9 at eleven, 17.5 at the 226 it was calibrated on, and about 23.0 at a thousand.** (A repository defect found in this pass: `tools/kb/README.md` line 224 says 26.4 at a thousand. That figure does not survive the arithmetic in memory.rs and index.rs and should be corrected at source.) Below two entries the verdict is never Hit whatever the score, because with one note every key has df 1 and idf can tell nothing apart.

**Measured, with its conditions:** over 50 blind questions on the 11 entry demo, 28 of 30 out of scope questions were not answered confidently, with 2 confident wrong answers left in the record at 33.0 and 24.8, and the in scope split moved from 6 confident / 8 guess / 6 nothing to 12 / 2 / 6 when the floor learned to scale (ADR-0036).

**Constraint.** We optimised for a consumer that is a model: whatever `kb` hands back gets treated as fact, so the mechanism has to carry its own confidence or the model invents one. They optimised for a consumer app where the empty state is a churn event and where the person can always fall back on the original post, which is the fallback their terms name.

**Verdict: not comparable on public evidence.** A measured property of ours cannot be ranked against an unsourced property of theirs. What can be said: ours has an abstention instrument and it is measured, and ADR-0017 measured BGE-M3 scoring 0.496 to 0.659 on correct answers and 0.510 to 0.608 on wrong ones, "overlapping completely", on our corpus, our questions, that model. Two things keep this honest. Abstention is cheap for us and expensive for them: our user owns the corpus and can fix a miss, theirs is paying a subscription and a nothing found screen reads as a broken product. And one anecdotal test of their app would be an anecdote, not a rate, and could not be compared to our 28 of 30 anyway, which is the deterministic layer alone on an 11 entry demo.

### 4.3 Cost per item and per query

**Theirs.** An embedding per item at ingest and, by INFERENCE, one per query, on top of a pipeline that already ran a multimodal model per save, behind a scraping vendor. No per item cost, per query cost or retrieval latency figure is published anywhere we read. On the caps, two first-party sources disagree: terms say "monthly limits", while the App Store says "each week" and release note 1.0.2 says "The free allowance now returns every week instead of every month". The terms read as stale.

**Ours.** Zero models at ingest and zero at query. Numbers, each with its conditions, none comparable to anything of theirs: in process open plus first answer 136.4 ms then warm p50 0.68 ms over 1000 samples, Windows laptop on `examples/demo`, 2026-08-23; spawn `kb route --json` plus open plus answer p50 184.8 ms, min 145.8, p90 252.2, same laptop, release build, 40 samples after 3 warm ups, 2026-08-30; the same spawn and answer p50 9.6 ms of which about 6 ms is process creation, Linux WSL2 on x86-64, a 9 entry base, 40 executions, 2026-08-29, and that last one is somebody else's measurement which we have not reproduced.

**Constraint.** They pay per item because the item is a video that no index of any kind can reach until a model has watched it. We pay nothing per item because the item is text somebody already wrote. **That is a property of the input, not a virtue of either design.**

The honest accounting cuts against the obvious reading: their expensive step is transcription and visual text extraction, not the embedding, so once a multimodal model has watched the video the vector is rounding error on that save. Comparing "an embedding per item" to "a subprocess and a SQLite index" compares the cheap end of their pipeline with the whole of ours. On the per query side: chat messages are metered (EVIDENCE, terms); whether search is metered is NOT SOURCED and the two first-party pages point in different directions. Ours costs a process. On Linux that process costs less than one round trip to anything; on Windows the same spawn is roughly twentyfold more expensive, and the two figures come from different machines and different bases, so the ratio is indicative rather than measured.

Verdict: a different trade.

### 4.4 Who authors the retrieval handle

**Theirs.** Machine derived for the fields the site names, and nothing is asked at save time. Correction worth carrying: users do author text. EVIDENCE (privacy): "You can also organize items into folders, add personal notes", and the App Store has a section headed "YOUR NOTES, NOT JUST A LINK". Whether notes or folder names enter the search index is NOT SOURCED. So the accurate statement is that **no page asks the user to write a retrieval handle**, and some user written text exists whose role in retrieval is unknown.

**Ours.** A person, at write time, with no flag to skip it. `write::Note.keys` is required and `WriteError::NoKeys` is returned when it is empty (write.rs:143); `kb check` warns W06 under twelve keywords and asks for "thirty, in both languages, including the words somebody types from inside the problem". The measured consequence of not doing it is in the root README: a file keyed `eating out, restaurant, poke, salmon` scored zero on "hoje vou sair com meus amigos, to com azia, o que vou comer" and answers at 130.21 once widened to thirty terms.

**Constraint.** They optimised for a capture path that must not interrupt the scroll, on content the user did not write and could not label if asked. We optimised for a lexical scorer that has no other way across the vocabulary gap, on content whose author is present at write time.

**Verdict: theirs is plainly better for their input**, because there is no version of the hosted product where the user types thirty keywords per Reel. Ours is not thereby wrong, because ADR-0016 names what the keys buy: "the keys exist to bridge the gap between how somebody asks and how the file was written. Deriving them from the file guarantees they carry the file's vocabulary, which is the half we already have. `nunca` never appears in a note about limits." Curated keys only earn their price when the scorer is lexical, which makes their choice and ours each coherent with their own ranker.

### 4.5 Where the model sits, and what it may do when unsure

**Theirs.** In the ingest path and the retrieval path both, as a generator. INFERENCE, marked as absence of evidence rather than evidence of absence: no public page describes an outcome where a save produces nothing, because a held link is stored even when processing is refused and a failed save is claimed to retry.

**Ours.** A model is in the ingest path only, and only as a gate. One promoter call per deposit file plus three reviewer lenses per proposal, unanimity to write, and an unreadable reply parsed as a refusal: "An unreadable reply is a rejection, never an approval". ADR-0030 line 100: "Every failure mode of this command degrades toward writing nothing, because it mutates the base." No model in retrieval: `kb_retrieve`'s description says "Ranking fuses a hand written keyword index with full text search; no model is involved."

**Constraint.** They optimised for a save that always yields something searchable, which is coherent for a consumer who tapped Share and will never open an editor. We optimised for a write that can never degrade the base, because the writer is a model and the reader trusts abstention. **What makes their choice defensible and not merely cheaper: the user's tap is a consent signal, an explicit "keep this", that our deposit does not carry.**

Verdict: a different trade.

### 4.6 Explainability of the ranking

**Theirs.** Item level provenance, claimed. Term level attribution NOT SOURCED, and INFERENCE that a cosine score has no words in it to show.

**Ours.** Every `kb_route` and `kb_retrieve` reply carries a fixed line from `mcp.rs fn evidence`, naming the score, the floor that applied to this corpus, whether both scorers ranked the top file, the margin over the runner up, and the verdict. `mcp.rs`'s own doc comment records that we shipped without this and it was a defect: "A model reading this tool could not tell a top score of 188.6 from one of 19.9."

**Constraint.** Their reader is a person who can verify a Reel in one glance, so a thumbnail is a cheaper explanation than any score. Our reader is a model deciding whether to lean on a passage.

**Verdict: a different trade**, downgraded from an earlier "ours is better". Explainability of ranking is only load bearing when the consumer cannot see the item, which makes the axis consumer dependent rather than absolute. They also have an agent consumer, the MCP connector, and what its replies carry is behind the 401, so on the one surface where the comparison would be like for like we have no evidence at all. They bought paraphrase reach with their scorer family; we bought a number.

### 4.7 What happens to a query that found nothing

**Theirs.** NOT SOURCED. INFERENCE, flagged: an empty search is the kind of event product analytics normally captures, which is not the same as a per question log surfaced to somebody who can act on it.

**Ours.** A refusal writes the question to `kb-misses.txt`, deduplicated and counted, most asked first, with an indented `looked like:` line. It is gitignored wherever it lands because, in the module header's words, "A Yaron miss is a health question." Nothing reads it back: `Memory::recall_loss` is the sole caller of `misses::record`, and `misses.rs` exposes no reader.

**Constraint.** Our recall gap is closable by a person typing one alias line, so a worklist of misses is actionable. Theirs is not closable that way at all: nobody improves a vector by writing a synonym.

**Verdict: a different trade, and it is in their favour more than it first looks**, because a miss log is only worth keeping when a human act repairs the miss. What our log buys is the convergence evidence ADR-0018's revisit triggers are written against. What it costs is that the repair is still a person reading a file and typing, and that the log's first hosted deployment kept none of the window's questions: `reports/2026-08-29-first-integration.md` records "The integrator's miss log had two lines, both written earlier on a development machine. None of the six production questions appears."

### 4.8 The trigger, and the cold start of the capture loop

**Theirs.** A server-side asynchronous job per save, INFERENCE from Convex's model (network calls run in actions, not mutations) plus the queue being visible in the product as "held links waiting for processing". No cold start is described: INFERENCE, and it is only that, no public page states a corpus-size dependency.

**Ours.** No queue, by explicit decision, and the trigger is an idle hook rather than a clock. ADR-0035: "No queue of raw turns, no SQLite of transcripts. The transcript already exists on disk and belongs to the harness. Duplicating it into a durable queue with a watermark is the shape a competitor built and this fleet decided not to have." The cold start is real but smaller than an earlier draft said: the one hard gate is `MIN_ENTRIES_TO_ROUTE = 2`, and the floor now scales, so ADR-0035's anecdote about a four key question scoring 11.4 against a floor of 17.5 is from 2026-09-01, the day before the floor started scaling, and reporting it as current behaviour contradicts the source.

**Constraint.** They optimised for input arriving at any moment from a phone that is not running their code, which forces a durable queue and a server. We optimised for a laptop-local binary with no daemon and no second copy of the transcript. ADR-0035 names our failure case: "A session that runs longer than a day without ending. That is the day the clock trigger gets built."

Verdict: a different trade on the trigger. On the cold-start axis alone theirs is the cheaper design, and the axis is small enough that it does not carry a verdict against a system that buys abstention with it.

### 4.9 Second derivation per item

**Theirs.** Two per video, and on the public record it is their sharpest ingest decision. EVIDENCE (App Store): "A full transcript of what was said" and "The text that was on screen, including recipes and step lists" are separate stored fields. INFERENCE: some short-form video puts the ingredient list or the step count on screen and never speaks it, so an audio-only index would miss that content. How large a share is an empirical claim about the world with no source behind it.

**Ours.** One channel, and ADR-0035 names the blind spot: capture writes only the questions the base refused and the agents it routed to, so "A fact the person said in prose and never asked about is not captured." `capture::Session` has exactly two fields.

**Constraint.** They optimised for coverage of one item, spending model calls per save so the searchable text is not a subset of what the item says. We optimised for a deposit where every line is something the system itself measured, so the junk rate ADR-0030 leaves unmeasured can be counted before any generator of candidates is switched on.

Verdict: a different trade, and the row compares unlike units. Two derivations of one video is redundancy over one artefact; two event kinds per session is coverage over a conversation. **The transferable idea is the second independent derivation, not the number two.**

### 4.10 Custody: machine loss, export, exposure, continuity

Four rows that are one trade seen from four sides.

**Machine loss: theirs is better.** Their archive is server side and held against the account. Our README has one sentence on this and it names an operation with no destination: "Backup, sync, and moving to a new machine are all the same operation." Losing the disk loses the base unless the owner independently arranged otherwise.

**Export: ours is better on published portability.** The files are the export. No export feature is named on any public page of theirs, which is a checked absence on the website and not proof about the app's own surface. What they do have that we do not is a written legal right: EVIDENCE (privacy), LGPD portability by email, bounded by "as applicable", with human latency. That is more than nothing and omitting it was unfair.

**Exposure: a different trade.** At least four named parties touch the content of a processed save on their side, and both lists say "including", so the four are a floor. Ours is zero to one on the retrieval path and the user picks it. **On raw exposure count ours is plainly lower, and the two products are not doing the same work.** A lower number of processors for a job that never involves transcribing a video is not a better answer to their question. Stated fairly: we spend no exposure because we buy no capability with it. Noted honestly on our side: `tools/kb/src/ui.rs` spawns `claude -p` for the reading room's chat, so a person using that surface sends their message to Anthropic by default. That is shipped code, not a configuration choice.

**Continuity: a different trade.** Theirs survives the disk and dies with the operator, with liability capped at the greater of fees paid and BRL 100 and no self-hosting or source availability. Ours survives the author (Apache 2.0, published source, markdown in folders) and dies with the disk. Each side's continuity story fails exactly where the other's holds, which is the definition of a different trade.

### 4.11 Distribution: first user, second user, and the shelf

**First user: theirs is better, and it is not close.** Four of their steps happen inside a settings panel; three of ours happen in a terminal. The verdict survives even counting our release-download path, because that path still asks the person to know their platform, know the Linux build is musl, and run a checksum.

**Second user: a different trade.** They have three instruments, all inside the product: a referral with a deadline at the fifth save, shared folders by invite link, and a store surface that recycles users into proof. We have no mechanism. A base is a directory of markdown and ADR-0008 makes copying that folder the documented exit path, so the artefact exists and travels; what does not exist is anything in the software that eases or rewards the transfer. Their entire acquisition machine rests on identity, and ADR-0008 forecloses identity: "Nothing in the code knows what a user is."

**The shelf: theirs is better, narrowly.** An App Store listing is mandatory infrastructure for an iOS app, so merely having a store page is not a decision he made and we skipped. **The credit is for the discretionary half:** the BR listing is "the hosted product: Salvar Vídeos e Posts" with subtitle "Guarde receitas, pergunte à IA", the US listing is "the hosted product: Save Videos & Recipes" with subtitle "Ask your bookmarks anything", same app id. Two positionings of one binary, spending the keyword budget on the mechanic in one market and on the promise in the other, invisible from the marketing site. We have no analogue of that practice.

**Platform coverage: neither better.** They are absent from Android, where a large share of their users are, and the word appears on no page of their site, so the gap is also undisclosed. We are absent from macOS, where a large share of developers building agents are, and README.md:151 names it with the reason. Both are the same shape of gap, a platform nobody on the team runs.

### 4.12 Whether the trust document matches the running code

**Theirs is stale in one place and it is a commercial term:** the terms, dated 28 July 2026, still say "monthly limits" while the product moved to weekly on 22 August.

**Ours is stale in three places, and they are trust boundaries.** (1) `SECURITY.md:21` still states "The privacy gate: anything git does not track must never be served, indexed, or suggested", and ADR-0034, accepted and implemented 2026-09-01, explicitly retires it: "The sentence 'unknown is not public' is retired. What replaces it: undeclared is the folder map." (2) `tools/kb/src/ui.rs:41-42` carries the same retired claim in a module header. (3) `SECURITY.md:23` scopes the local-only promise to "The boot hook and the MCP server" and never mentions `kb ui`, which binds a loopback socket and spawns a cloud model.

**Verdict: theirs is better.** Theirs lags on a price a user discovers in-app anyway; ours lags on the sentence a security researcher reads before deciding what counts as a vulnerability, in three places rather than one.

---

## 5. What we should take, ordered by what it buys us

Each item is written to be turnable into a decision record: mechanism named, honest cost, and the ADR it argues with.

### 5.1 Give unreachable items a named, counted state, the way a held link is named

**What it buys us.** It converts our largest silent failure into a number, and it is the one item here that costs no model, no network and no new dependency. This is the highest ratio of value to risk in the list.

**Mechanism.** `index::build` calls `header_of` on every walked file and continues when the keyword list is empty (index.rs:168-172), so a keyless note is stored, readable and scores zero forever with nothing said at query time. Their equivalent population is announced: a held link is stored and named in two documents, and a failed save "shows why on its row, and you can retry it from the item's menu". The change is to count what `index::build` skipped and print it from `kb index` and beside `gate` in `kb route --json`, so the number reaches whoever is asking rather than sitting behind a linter run. The same subtraction covers the derived lag: `base.rs` walks the files and hands the list to both `index::build` and `Store::sync`, which records every path it chunked, so files-on-disk minus files-with-chunks is a set difference over two lists that already exist. The specific case worth catching: the SessionEnd hook runs `kb capture` and detaches `kb promote` and never runs `kb index`, so the deposit the system writes for itself lands in exactly that window.

**Cost.** A second surface that has to agree with `kb check` W06 and W07 or drift, which is the class of bug index.rs already guards by defining `unreachable_keys` in one place. It also duplicates part of what the per query NOTE in `mcp.rs` already says, so two places can now disagree about one fact. And the number will be embarrassing: 137 distinct keys dead over 8580, 116 of them ordinary terms. That is the point of printing it.

**Argues with.** Nothing. It strengthens ADR-0016 ("A tool that can create an unreachable note has handed you a way to grow a base while making it worse") and makes ADR-0028's rule audible: "A file is in the index if and only if it declares keys." ADR-0003 is why the lag is allowed to exist and why it should be countable rather than narrated.

### 5.2 A reader for the recall loss log, so a miss arrives as a line to paste

**What it buys us.** It closes the one loop in the system that currently ends in a text file nobody opens. The evidence is already collected, deduplicated and ranked by count; only the reader was never built.

**Mechanism.** `kb-misses.txt` is already ranked by count, and `index::suggest` already computes near terms at refusal time. A `kb misses` verb would read the log back and, for each question, print the entries that scored just under this corpus's floor together with the keys they carry, which is the alias line or the `Search for:` term the file header already tells the reader to write. Nothing reads that file today: `misses.rs` exposes `record`, `path_for`, `path_in` and `today`, and no load function.

**Cost.** It is still a human act, it just costs less, so it does not close what our record calls the vocabulary treadmill. And a verb that proposes an alias line sits one step from a verb that writes one, which is the step ADR-0016 and ADR-0030 keep deliberate. **The reader must not gain an `--apply` flag by accident.**

**Argues with.** Nothing decided. The `misses.rs` header already cites ADR-0006 saying the expansion log is "the worklist for improving step 1", and the artefact already instructs the human.

### 5.3 A list-by-facet operation that is not a search

**What it buys us.** It removes a whole class of spurious guess: a filter shaped question that contains no ranking problem currently has to be asked as a question and gets scored against a floor.

**Mechanism.** Their MCP page lists these as separate capabilities: "A search by meaning across the items you saved" and "A list of them by date, platform, type or folder". Whether they split them for the reason we would is not stated. Our four tools are all search or roster, so "what is in Yaron's skills folder" gets scored. The facets already exist on our side: `index::kind_of` computes species from the path, base and folder are on every `Entry`, and ADR-0031 already folds by species.

**Cost.** One more tool in the model's menu, which is not free: `kb_fleet` exists partly because models reach for search when they should not. More seriously, a listing surface is a new way to leak the private layer, and it has to read `private =` off `agent.txt` through the same path `Memory::open` uses (base.rs:69-80) rather than reimplementing the rule, or ADR-0034's single declaration becomes two.

**Argues with.** Nothing. ADR-0034 leaves per consumer scope explicitly unbuilt and says to build it when the second real consumer asks; this is a per operation shape rather than a new scope.

### 5.4 Refuse at the moment of intent and name the next act

**What it buys us.** It is the best single piece of design on their side and it transfers without a price attached. Today our abstention arrives as a bare refusal; theirs arrives with the act attached.

**Mechanism.** They do not stop you at OAuth, where a refusal is unintelligible. The connection succeeds and the check fires inside the assistant, on the first read, where the intent already exists: "The connector asks you to upgrade on its first read." Our analogue is not `MIN_ENTRIES_TO_ROUTE`, which is 2 and which two files clear. It is the scaling floor: ADR-0035 records that "A fleet too small to clear the floor never routes and therefore never captures", and `floor_for(N)` is higher in effective terms on a small corpus. The transferable move is to make the refusal carry the act: name that the base has N entries, that the floor is what it is at N, and what to do next, in the same reply that refused. `memory.rs` already puts `floor` on the `Confidence` struct precisely so every surface reads the threshold that actually applied.

**Cost.** More branches in the refusal path, and a refusal that instructs can instruct wrongly, which is worse than one that says nothing. It is strings and tests rather than architecture.

**Argues with.** Nothing. It extends what `mcp.rs` `nothing_to_search()` and `no_match()` already do.

### 5.5 A meaning tier that runs only on the refusal path, never in the ranking

**What it buys us.** The query shape their product is aimed at and ours structurally cannot reach: a Portuguese question about an English note gets back the English key instead of nothing.

**Mechanism.** The verdict is computed from the keyword list alone in `memory.rs confidence_of`, before anything else runs, so a scorer invoked after that point is structurally incapable of turning a refusal into a hit. **It changes what a refusal says, not what a verdict is.** Today that slot holds `index::suggest`, Dice over character trigrams at SUGGEST_FLOOR 0.65, whose reply text says the comparison "is spelling, not meaning, so it finds a typo or a cognate and never finds a translation". Note what the borrowing rests on: their cross-language reach is INFERENCE about embeddings in general, since no page of theirs names a model or claims it.

**Cost.** It reintroduces a model into a process that has none: a download, a residency cost on a four core laptop with no CUDA, and a latency spike concentrated on the queries that already failed. It has to print its similarity in units that cannot be mistaken for the EVIDENCE line's score, or a model reading the reply treats a suggestion as an answer. **And if it runs off the machine it violates query privacy, so it is local or it is nothing.**

**Argues with.** ADR-0018, and it sits on the edge rather than clearly outside. The record's title is "no model enters the retrieval path", and a suggester after the verdict is not in the retrieval path. But ADR-0018's revisit trigger sets the bar any candidate must clear: "An embedding or reranking model appears that is Apache/MIT licensed, under 1 GB, and demonstrates hit/miss separation on someone else's benchmark", and jina-reranker-v2 was rejected partly for being CC-BY-NC. **This gets a record before it gets a commit.**

### 5.6 Give `kb remember` a `--session` flag, so its proposals reach the deposit

**What it buys us.** A second independent derivation per session, which is the transferable half of their transcript-plus-screen-text decision, and it needs no model.

**Mechanism.** ADR-0035 names this exact gap in its closing line: "What waits for a flag: `kb remember` proposals, which do not know their session." `capture.rs` already has the shape: `note_refused` and `note_routed` each append one tab-delimited line to `.kb/sessions/<id>.events`, and `read()` parses exactly two line kinds through a match with a catch-all arm. A third kind, carrying the claim text, the containment score and the ADD, UPDATE or NOOP verdict, is one more append function and one more match arm, and old records stay readable because the catch-all already ignores unknown kinds. Nothing here reads a transcript and no model is added: remember's judgement is the deterministic containment scorer.

**Cost.** The deposit stops being purely a record of what the system measured about its own failures and starts carrying candidate content, which raises the junk rate the deposit was designed to make measurable. **That is the point and it is also the cost, so it should ship before the first promote runs are counted or after them, never in the middle**, or the number measures two different files.

**Argues with.** ADR-0035's sequencing, mildly and only in appearance. That record defers option B, a model reading the transcript. This is still option A: every line remains something the system computed. But it moves the baseline of the junk-rate measurement both ADR-0030 and ADR-0035 are waiting on.

### 5.7 A low-argument capture door into `inbox/`

**What it buys us.** The share-sheet lesson without the share sheet: a drop lands with no authored keys and the existing promote path authors them.

**Mechanism.** Most of this exists. `inbox/` is already the deposit; `retrieve.rs::layer_of` already labels a deposit passage as short memory at every surface; `promote.rs`'s proposal prompt already instructs a model to write thirty to seventy keys in both languages. What blocks the drop is three things in `write.rs`, all read in source at this commit. First, `keys` is required: line 143 returns `WriteError::NoKeys` on an empty list regardless of folder. Second, `write::note` always writes a MAP.md entry through `place_entry` and returns `WriteError::NoMap` when the base has no MAP.md, which contradicts `capture.rs`'s rule that the router never names a deposit file. Third, `render_note` unconditionally emits a `**Search for:**` line, so a keyless note would carry an empty one. A keys-optional deposit door needs the keys check made conditional on folder, the map write skipped for the deposit, and `render_note` taught to omit the Search line, which is the same shape `kb capture` already produces.

**Cost.** Three. Every casual drop now costs one promoter call plus three reviewer calls, which the hook's own comment prices in minutes, so the base's growth rate moves onto a gate whose refusal rate we have never measured: ADR-0030 measures the junk we did not write and says nothing about the knowledge we did not admit. It makes the deposit the normal way to write, which is exactly the condition under which the unmeasured junk rate stops being a curiosity and becomes the system's quality. And it puts a second writer into the deposit alongside `kb capture`, so the junk-rate number would be measured over a mixture.

**Argues with.** Not ADR-0016, which requires keys because a note with no map entry is unreachable, and a deposit file is already reachable through the text scorer and already labelled short memory. It is a boundary clarification, not a reversal. It does conflict with the README's three-step flow, which teaches writing keys as step one. **One repository inconsistency to fix on the way:** `checks.rs`'s `is_exempt` doc comment still says `inbox/` "is a quarantine and its invisibility is the feature" and that material "becomes findable when somebody promotes it", which `promote.rs`'s own DEPOSIT comment records as never having been true, and which `retrieve.rs` contradicts. Two files give two reasons for one behaviour.

### 5.8 One install command, and the demo before the build in the reading order

**What it buys us.** The gap where they are furthest ahead and where our own record already asked for the fix.

**Mechanism.** Their share sheet works because it hooks a gesture the user already performs and because the configuration cost is paid once. The equivalent gesture for a developer building agents is registering an MCP server from a terminal, and the once-only equivalent of Favorites is `claude mcp add --transport stdio --scope user`, which we document. What we make the person do instead is choose between `kb-linux-x64` and `kb-windows-x64.exe`, know the Linux one is musl, run `sha256sum`, put it on PATH, and write the absolute path into a config file by hand. An install script that reads `uname`, downloads the matching asset, verifies its `.sha256`, and prints the exact `claude mcp add` line with the resolved path filled in collapses six decisions into one paste. Separately and for free: `examples/demo` already runs `kb index`, `kb route`, `kb eval` and `kb answer` with none of the user's own notes, and it currently sits after the cargo build in the README. Reordering costs nothing but the ordering.

**Cost.** A new artifact to maintain on a project whose selling point is one dependency. A curl-to-shell installer for a tool that reads your notes sits against README.md:149, "Building it yourself is the honest default", so the script has to be short enough to read in full, the README has to say read it first, and the build-from-source path has to stay documented above it. It must fail loudly on macOS rather than silently doing nothing, which makes the missing macOS build visible in the one place a new user will look. And demoting build-it-yourself from default to option is a stance change, not a formatting change, so it should produce the ADR rather than skip it.

**Argues with.** Nothing. ADR-0008 asks for exactly this: "The public repository is the product. It has to be installable by a stranger." F-09 is already closed; the installer is named there as genuinely missing, which is a weaker and more honest warrant than closing a finding, and still a warrant.

### 5.9 Fix the trust documents, publish the backup story, print the token cost, and move one sentence

Four smaller items with the same shape: prose that has fallen behind the code, or a number we already half print.

**Trust documents.** Fix `SECURITY.md:21`, `ui.rs:41` and `SECURITY.md:23` today. The mechanism that makes it stick is the one this repository already uses three times, per ADR-0025: "The em dash rule went into `kb check`, the commit rule into `kb commit`, and the boot rule into a hook, each after prose failed." The cheap honest version is to add the trust documents to whatever implementation checklist an accepted record carries; a grep for retired sentences needs a list of retired sentences, which is the same defect one level up.

**Backup.** Publish it as an explicit section in the four-numbered-surprises style the README already uses. The material needs no code: `.gitignore` line 4 records that `fleet/` is its own git repository, so a push to a remote is the whole mechanism. **The honest section has to say that a private remote puts the private layer on somebody else's server**, which is the custody position the product refuses, so the section cannot end in a recommendation. It ends in a named trade.

**Token cost.** `main.rs:1114-1119` already prints, before the first model call, "complete search: reading all {N} files in {M} batch(es), {M+1} model call(s) total." The call count is there; **the remaining gap is tokens, and only tokens.** It must be expressed in tokens rather than currency, because a price table goes stale with every vendor change, and marked as an estimate exactly as the timing already is.

**Say the gate's weakest point where the gate is documented.** Their /mcp page splits revocation into the half the user controls and the half they cannot: "Remove the connector in the AI app and it stops at once. To revoke the key as well, write to us." We have the same shape of gap one document deeper, in ADR-0034:49-52, "The boot hook runs `kb boot --all`. The promotion hook runs `kb promote --all`. Every surface the owner actually uses bypasses the gate." Move one sentence of that next to the private-layer paragraph in `tools/kb/README.md`, where the person configuring the gate is standing. The cost is that someone skimming reads it as "the gate does not work" rather than "the gate is for the model, not for the owner's own hooks", so the wording has to carry the distinction in one line.

---

## 6. What we must not take

### 6.1 An embedding inside the ranking path

**The record.** ADR-0017: "Nineteen of twenty answers are unchanged. The one that changes, changes for the worse." ADR-0018: "The models lost on accuracy, lost on abstention, and lost on latency, on our corpus, at our scale, measured by the system's own code."

**The measurement.** BGE-M3 scored correct answers 0.496 to 0.659 and wrong answers 0.510 to 0.608, "overlapping completely", while the keyword scorer's one wrong answer scored 3.82 against a lowest correct answer of 9.55. Conditions: our corpus, twenty questions written by the agent that tuned the keys, on a Latitude 3420, CPU only, indexing 1,039 chunks in 2,833 seconds.

**Why.** The score is the abstention instrument, and a dense score had no separation to carry a threshold when we measured one. Adopting the mechanism would buy the paraphrase queries named above and spend the property everything else here is built on.

### 6.2 Letting any scorer other than the keyword list set the verdict

**The record.** `memory.rs Memory::ask`, whose doc comment records the earlier version of this mistake: "Before this, `confidence` read the keyword score of *fusion's* pick, which is neither number and was nobody's intention." Plus the pinned case in `Memory::confidence`: "quem e voce?" was ranked by BOTH scorers and was still wrong, at 3.82, which is why agreement is reported and does not gate.

**Why.** This is the subtle version of the same import and it would arrive as a refactor rather than as a decision. Moving the verdict onto a fused or dense list reopens ADR-0017's measured failure without anyone deciding to.

### 6.3 Dropping the floor so a result list is always populated

**The record.** ADR-0036, which re-derived the floor rather than removing it and published both sides.

**The measurement.** Out of scope not answered confidently held at 28 of 30 over 50 blind questions on the 11 entry demo, while in scope confident answers went from 6 to 12, and the two confident wrong answers at 33.0 and 24.8 were kept in the record. Plus `MIN_ENTRIES_TO_ROUTE`, which refuses a hit at any score below two entries because with one note idf can tell nothing apart.

**Why.** This is what a ranker with no threshold gets for free and the one thing this design exists not to have. It would make our numbers look better while making the system worse.

### 6.4 Generating a note's keys from its own body

**The record.** ADR-0016: "the keys exist to bridge the gap between how somebody asks and how the file was written. Deriving them from the file guarantees they carry the file's vocabulary, which is the half we already have. `nunca` never appears in a note about limits." ADR-0028 carries the companion line: "A curated list carries intent; an extracted one carries frequency."

**Why.** It looks like the cheapest fix for our largest cost and it produces the half we already have. **The subtlety that decides which version is allowed:** keys generated at ingest are not a model in the retrieval path, and `promote.rs` already generates them under a three-lens gate, so that version is sanctioned and shipped. What must be refused is removing the gate. The moment machine-generated keys are the only thing making a file reachable, SCORE_FLOOR is calibrated against a key population no reviewer saw. Their design does not have this problem because it has no floor to protect.

### 6.5 Growing the corpus by automatic extraction with no refusal path

**The record.** ADR-0007 refuses automatic extraction of facts from conversation into the durable base, "the 97.8%. Ingestion stays deliberate." ADR-0030 keeps the gate: "Every failure mode of this command degrades toward writing nothing."

**The measurement, and it is not ours:** mem0 issue #4573, a public count of 10,134 entries over 32 days, of which 97.8 percent were judged junk, and ADR-0007's finding that 52.7 percent of that junk was the agent re-extracting its own boot file.

**Why, stated as a sequencing refusal rather than an opposed principle.** ADR-0030 concedes automatic capture is the right trade for users who will never open an editor, and ADR-0035 chose the deterministic option first for an ordering reason, not a taste reason: "A first, B after A has produced enough deposits for the junk rate to be a number." If their pipeline reads every save with a model, and INFERENCE says it does, then they are running our option B without our option A, which is defensible for a consumer whose tap already said keep this. **Ours has no tap.** The honest statement of our position is that we will not switch B on before A has produced a number, and A currently produces too little to count, which is itself a finding rather than a defence.

### 6.6 An ingest mode that always yields a stored artefact

**The record.** ADR-0030 line 100 and its three implementations in `promote.rs`: `ask_model` returns None unless a classifier is configured and the run writes nothing; a reviewer reply with no readable VERDICT line is parsed as a refusal; any one of three lenses refusing is a refusal.

**Why.** Theirs is claimed never to come back empty and that is coherent for them: over quota the URL becomes a held link, and a failure is claimed to retry itself. Their input was chosen by a person tapping Share. Ours was not chosen by anyone, so an empty result is the correct answer more often than not. **What we should take from them is the held-link half, keeping the input, which we already do twice. Not the always-produce half.**

### 6.7 A durable queue of raw turns with a watermark

**The record.** ADR-0035 item 1, unusually blunt about whose shape it is: "No queue of raw turns, no SQLite of transcripts. The transcript already exists on disk and belongs to the harness. Duplicating it into a durable queue with a watermark is the shape a competitor built and this fleet decided not to have." On top of ADR-0003, which makes any index derived and disposable.

**Why.** This is the one place where copying their ingest architecture would cost us a property rather than money: a durable queue is a second authoritative copy of material whose first copy we do not own. Their queue is not a mistake, it is close to mandatory for input arriving from a phone that is not running their code. We have no such input, so we would pay the cost for a capability we do not have.

### 6.8 Automatic promotion of stage or provenance

**The record.** `checks.rs:155-158`: captured is "Kept out of distilled deliberately. A note that arrived through `kb promote` was read by a model and reviewed by a model and by no person." ADR-0007 line 86 says why it is a lint and not a habit: it is "turned into something a linter can check instead of something a reader has to remember."

**Why.** The failure mode is a laundering one: material a model wrote and a model reviewed, presented at the same weight as material a person wrote. Stated without claiming this makes us better than them: their pipeline has no human tier to reach, so it has nothing to falsify, and a per-item maturity ladder would label nothing in a consumer archive. **This is the rule that has to survive if we ever take their cheaper trigger, because a cheaper trigger is exactly what would create volume nobody read.**

### 6.9 A hosted instance, and a hosted retrieval default

**The record.** README.md:525: "Local first is a design position, not a limitation waiting to be lifted: the index lives beside the files, nothing talks to a server, and there is no hosted instance to point at." Backed by ADR-0003: when two options are not equally reversible the reversible one wins unless the irreversible one solves a problem we actually have. Plus ADR-0004 for local-first inference.

**Why.** This is the row where their design is genuinely better on its own dimension and we still cannot take it, because taking it inverts the sentence the product stands on. The moment a hosted instance exists, "your files are the source of truth" becomes "our copy of your files is", and every other custody property in this study is downstream of there being no server. The right answer to the lost laptop is the backup section in 5.9, which the owner controls. Their exposure is not a mistake, it is the price of a capability with no local implementation: nobody transcribes a Reel on a phone at that latency. **We have no such capability to buy.** Noted for honesty: the reading room already crosses this line for chat at `ui.rs:277-291`. What is refused here is making that the default for retrieval, not pretending it never happens.

### 6.10 A write tool over MCP made safe by a scope

**The record.** README.md:515: "There is deliberately no write tool. `kb_remember` measures a claim against what the base already says and proposes ADD, UPDATE or NOOP with its evidence. It writes nothing and decides nothing."

**Why.** Their scope is a real mechanism and it does not transfer. A scope is meaningful because an authorization server sits between the client and the data: their discovery document names `authorization_servers: ["https://clerk.their site"]`. Our MCP server is a stdio process the client already spawned with the user's own file permissions, so there is no party in the middle to enforce anything. A scope on our side would be a string the same process chooses to respect. **Adopting the promise without the mechanism is the failure this repository names repeatedly, a rule that lives in prose.**

### 6.11 Accounts, referral loops, identified analytics, and gated capability

**The records.** ADR-0008's table: "No authentication, no accounts, no tenancy. Nothing in the code knows what a user is." "No telemetry, no phoning home. An agent that reports on its owner is not the thing we are building." And ADR-0008:84: "The paid product is convenience, not capability. Anyone who wants what we built can have it for free by cloning. That is a deliberate position and it is worth saying out loud, because it is the thing that makes the open source release honest rather than a funnel."

**Why, and be fair to them first.** Gating the connector behind Pro is correct for him: it is his highest-intent surface and, INFERENCE, the one whose reads cost him money. It is also mechanically unavailable to us: an Apache 2.0 stdio binary the user compiled has no entitlement check that survives a recompile, so the only way to build the gate is to build the server. Their analytics posture is the honest end of the practice, PostHog with session replay disabled and no "Data Used to Track You" category declared at all. We still refuse it, because our nearest equivalent is `kb-misses.txt`, a verbatim record of what a person could not find, which is the single most sensitive artefact the system produces about its owner. **The cost of refusing is concrete and we should say it: we have no acquisition instrument at all, and INFERENCE from their release notes, two notification releases in three weeks is the signature of steering on retention data we would never possess. We are choosing to fly without that instrument.** The compensating instruments we do have are the miss log and the integration report.

One boundary drawn rather than a blanket no: their 1.0.2 shipped a morning digest, a pile-up nudge and a first-save notice, with "Every one of them is a switch you control in Settings", and shipping the opt-out in the same release is the right order. What we must refuse is the half requiring server-side knowledge of what the owner saved and did not open. A purely local reminder driven by files on the owner's own disk is forbidden by no record we hold.

### 6.12 A macOS binary nobody ran, and deferring numbers behind the install

**The records.** README.md:151: "There is no macOS build: nobody here runs macOS, so a published artifact would be one nothing has ever executed, which is worth less than its absence." Plus the house rule "Never claim something works without running it", and the precedent `.github/workflows/release.yml:66-67`, which already executes the Linux artifact inside `amazonlinux:2023` and `alpine:3` before publishing.

**Why.** This is the specific place where a distribution lens applies pressure to break our own rule. The temptation is not to close a finding, F-09 is already closed; it is to make the macOS gap disappear from a README that currently names it. **The permitted fix is a CI job that executes the artifact on a macOS runner before it is published, the same standard the Linux artifact already meets. The forbidden fix is shipping it because the matrix compiled.** The same rule covers deferring price-shaped numbers: neither the price nor the quota appears anywhere on their public site, which is store convention and defensible, and is still an asymmetry the visitor cannot resolve without installing. We have already decided in public that a number carries its conditions.

---

## 7. What their product does better than ours, said plainly

1. **Capture friction.** Two taps from inside the app the person was already in, with a pre-issued Keychain token so the save happens without leaving Instagram, and the share sheet setup taught on first launch. There is no version of Ulpia where this is beaten on the input type they accept.

2. **The path to a first user.** Free install, one-tap sign in, and the first save in minutes with no terminal, no toolchain and no decision about which artifact to download. Four of their steps are in a settings panel; three of ours are in a terminal.

3. **Survival of a lost, stolen or dead machine.** Their archive is held server side against the account. Ours has no backup story at all, and the README's one sentence names an operation with no destination.

4. **Held links and per-item failure surfacing.** An over-quota save keeps the URL rather than dropping it, both documents say so, and a failed save "shows why on its row, and you can retry it from the item's menu". Our equivalent population, a keyless note, is silent and counted only by a linter nobody is required to run.

5. **The read-only disclosure.** a `scopes_supported` naming exactly one read scope in a discovery document, plus a 401 challenge naming the same scope, is checkable from outside without trusting marketing copy. Ours is checkable only by reading our source. Same decision, better disclosure.

6. **A revocable, narrowable credential.** For a surface exposed to the public internet, a token a user can revoke and an operator can narrow is the stronger mechanism, and their page splits revocation honestly into the half the user controls and the half they must ask for.

7. **Refusing at the moment of intent.** The entitlement check fires inside the assistant on the first read rather than during the OAuth dance, where a refusal would be unintelligible. This is the best single piece of design in their product and the one item in section 5 that transfers with no price attached.

8. **Two derivations per item.** Audio transcript and on-screen text as separate stored fields, so the searchable text is not a subset of what the item says.

9. **Two positionings of one binary.** BR and US listings with different names and subtitles, spending the keyword budget on the mechanic in one market and the promise in the other. We have no analogue of that practice.

10. **Security documentation.** Theirs is thin: encrypted transport, authenticated APIs, restricted access, and an honest "No method of transmission or storage is completely secure." Ours is silent: a grep for "encrypt" across every tracked file outside `fleet/` returns one hit, and it is Let's Encrypt in a deploy doc. **Silent is worse documentation than thin.**

11. **Trust documents that match the code.** Theirs lags in one place and it is a price. Ours lags in three places and they are the sentences a security researcher reads.

12. **Ship cadence made visible.** Five dated releases in three weeks on a public build log is what makes a young product read as alive. Our front page says "Pre-launch. Used daily." with no date on that line.

---

## 8. What we know versus what we inferred

Everything below is INFERENCE, a checked absence, or a number whose conditions forbid a comparison. **None of it may be quoted later as a fact about their product.**

### Inferences about their mechanism

- **That their ranking is cosine similarity over vectors.** EVIDENCE is only that "embed" is one of six verbs in the privacy policy and that /mcp says "A search by meaning". The ranking method is not stated anywhere.
- **That a query costs an embedding and a chat answer costs a model call.** Inferred from the standard shape, not from their pages.
- **That their search crosses languages.** No page names an embedding model or claims cross-language retrieval. Three shipped locales are a distribution fact, not a retrieval property.
- **That a dense retriever loses a rare-exact-token query to a near neighbour.** This is a general claim about vector search, not a sourced claim about the hosted product.
- **That a cosine score has no term-level attribution to offer.** Inferred from the mechanism, not observed.
- **That their embeddings sit in Convex's own vector index.** Inferred from the sub-processor list containing no vector database vendor and from Convex shipping native vector search.
- **That their backend schedules work asynchronously rather than running it inline.** Inferred from Convex's model (network calls run in actions, not mutations) plus "held links waiting for processing".
- **That "Saved in under a second" is enqueue latency rather than end-to-end.** Inferred from an iOS share extension's time budget plus the support page's separate "finishes processing shortly after".
- **That their item record carries platform, type, category, folder and timestamp fields.** Inferred from the /mcp facet list plus marketing fixtures in the site's payload. Those are fixtures, not a production schema.
- **That the notification releases were built for the ingest seam, and that two notification releases in three weeks is the signature of steering on retention data.** Inferred from release-note motive, which is not evidence.
- **That save number one is processed identically to save number ten thousand.** No page states a corpus-size dependency; a stateless per-item pipeline is the economical reading.
- **That the MCP connector is reachable by clients that cannot spawn a local process.** Inferred from it being a remote HTTPS endpoint. The claim that it reaches a phone is struck: nothing fetched supports phone-side connector support in any client.
- **That the five named client families work.** The page asserts them; nobody outside has authenticated.
- **That their exposure to four named processors is the price of a capability with no local implementation.** Reasonable, and still a reading of their motive rather than a sentence they wrote.

### Checked absences, weaker than a quote and stronger than an assumption

- **No abstention or empty-result behaviour is described at their retrieval layer.** /mcp lists four capabilities and says nothing about empty or uncertain results, and `initialize` returns 401 so no result schema is public.
- **No export feature is named on any page of their website.** The app's own in-app surface is not observable from outside, so an undocumented in-app export cannot be ruled out.
- **No log of failed searches is described**, though PostHog is named for product analytics.
- **No price, no quota number, no retention period, no latency figure, no accuracy figure, no failure rate and no ratings denominator appear anywhere on their site.**
- **The word Android appears on no page.** iOS-only is implied and never stated.
- **No per-item statement of maturity or of who judged an item appears on any public page**, but the app's item view is not observable, so this is absence of evidence about a closed surface.
- **No no-training commitment exists.** The strongest sentence is "We instruct providers according to their commercial APIs", which describes an intention about instructions, not a contractual guarantee. They also do not claim a training licence.
- **Their MCP tool names and schemas are not public.** An unauthenticated `initialize` returns 401, so no comparison of tool granularity is possible.
- **No first-party long-form writing about how the product was built exists.** No blog, changelog or docs on their site, and nothing in the places a builder usually writes. So nothing in this study is sourced from the builder's own account of it, only from the product's public surface.

### Claims struck from earlier drafts, recorded so they are not reintroduced

- **"Their read-only promise is enforced on the wire."** It is declared in a discovery document and named in a 401. No write was attempted against a valid token. A declared scope is a strong, auditable claim, not a tested behaviour.
- **A transcript reading "linguine, citrus and anchovy".** The question "What was that pasta with the lemon?" is EVIDENCE from the App Store description. The transcript was invented to make the example land. No fetched page contains the text of any saved item.
- **"Their public pages describe no export."** No export FEATURE is named, which is a checked absence, but an LGPD portability right by email IS named. Omitting it was unfair to them.
- **"Every field the search runs over is machine derived."** Users author notes and folder names; whether those enter the index is NOT SOURCED.
- **"Four named parties touch the content on every save."** Both provider sentences say "including", so four is a floor, and a held link over quota is stored without being processed at all.
- **"Chat is metered while search is not."** The terms say search is "available on Free as described in-product", which is availability and not metering, and the App Store points the other way.
- **"Ours is better on abstention / explainability / provenance / continuity / training / query stream."** Each of those ranked a measured property of ours against an unsourced or differently-constrained property of theirs. They are different trades, and where ours is genuinely stronger the claim has been narrowed to what is sourced.
- **"No model of any kind sits between a question and the ranked files", attributed to ADR-0018.** That sentence is not in ADR-0018 or anywhere in this repository. The rule as actually written is in `mcp.rs`: "Ranking fuses a hand written keyword index with full text search; no model is involved."
- **"A query embedding would be the first thing to open a socket."** `kb serve` opens none, but `ui.rs` binds a TcpListener on 127.0.0.1. The accurate claim is narrower and still decisive: nothing in `kb serve` opens one, and nothing in the retrieval path leaves the machine.
- **"The failing eval is on our front page."** It is at README.md:92 and on the how-it-works docs page. The front page carries one dated `kb route` run and no failing eval, which makes the reach asymmetry worse, not better.
- **"kb write requires seven flags."** Everything except agent, slug, `--keys` and a body has a default. The gap is one required flag and a pipe.
- **"kb-misses.txt has no code that reads it."** `misses.rs:266` parses the existing file to merge counts. The accurate version is that no command surfaces it for review; the only reader is the writer.
- **"Our floor is 26.4 at a thousand entries."** `tools/kb/README.md` line 224 says so and the arithmetic in memory.rs and index.rs gives about 23.0. **That is a defect in our own documentation, found in this pass.**
- **"ADR-0012 fixed the name and the hosting."** ADR-0012 named the system Vesta on richardwollyce.com subdomains; ADR-0019 renamed it Ulpia and bought ulpia.io, and decisions/MAP.md records that it supersedes ADR-0012's name and no-new-domain position.

### Numbers that must not be compared

- **Their headline lifetime item count** is a total processed across all users, with no date. It is not users, not saves per user, and not throughput. Nothing of ours measures the same thing.
- **Their "4.7"** is from 13 ratings on the Brazilian storefront. The US storefront shows no overview because there are not enough ratings.
- **Their "Saved in under a second"** measures the row appearing, not the content being understood, and the landing does not say which. It cannot be placed beside any retrieval latency of ours.
- **Our 28 of 30** is the deterministic layer alone, over 50 blind questions, on an 11 entry demo corpus. One anecdotal test of their app would be an anecdote, not a rate, and could not be compared to it.
- **Our 16/19 top-1** is file selection on a 122-file corpus against a gold set its own author tuned keys against. It is not comparable to any recall@k.
- **Our latency figures** come from three different machines and three different bases, and one of the three is somebody else's measurement we have not reproduced. The Windows-to-Linux spawn ratio is indicative, not measured.
- **Their BRL and USD prices are separate storefront prices**, not conversions of one another.
- **Their version-history dates** were read with a year that conflicts with every other source. The day-and-month ordering is read; the year is unverified and taken as 2026 from context.
- **Their App Store compatibility line lists macOS 12.0+ on M1 and visionOS 1.0+.** For an iOS app that usually means a catalogue flag rather than a built target, and which it is was not verified.
