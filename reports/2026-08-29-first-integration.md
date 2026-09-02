# First integration: what a hosted consumer found, and what we will change

| | |
|---|---|
| Date | 2026-08-30 |
| Source | An integrator's field report on the first production use of `kb`, 2026-08-29 |
| Binary under test | `kb-v0.1.0`, `x86_64-unknown-linux-musl`, commit `f320903` |
| Consumer shape | A chat agent inside a serverless function: ephemeral instance, read only filesystem, one `execFile` of `kb route --json` per question, base and binary shipped inside the deployment bundle |
| Base under test | 2 agents, 9 index entries, 11 text files, 148 KB |

The report itself is not in this repository and will not be: it names a real deployment, a real
project and real questions asked by real people. What follows is the mechanism half, which is the
half that is ours. ADR-0025.

**What this file is for.** It is the worklist for the next cycle, written before any code changes so
that each item can be turned into a failing test first. Every finding below carries the mechanism in
our own source, with file and line, because a finding reported as a symptom gets fixed as a symptom.

**How each claim here was checked.** Every "mechanism" paragraph was read in the source at the commit
this file was written against. Nothing below has been run yet: the reproductions and the numbers
belong to the integrator's machines, and our own confirmation is by reading. Where I disagree with the
report, the disagreement is marked and the reason is given.

---

## The one sentence

The read half went to production and held: six questions, zero retrieval failures, the verdict gate
obeyed, the error contract used to warn a person on screen. The write half was never exercised and in
the current shape could not have been, and the reason is architectural rather than a matter of taste:
**every write path assumes a writable repository on the same machine as the question.** A hosted
consumer never has one.

The second thing the report exposes is smaller and worse: **the loop that was supposed to repair
recall is not merely blocked by the read only filesystem, it is not wired to the case that matters.**
That is finding F-02 and it is ours alone. The integrator could not have seen it, because from
outside it looks identical to the filesystem problem.

---

## The findings

Numbers are ours. The integrator's numbers are given beside them so the two documents can be read
together.

| | Finding | Theirs | Severity | Kind |
|---|---|---|---|---|
| F-01 | The gate lives outside the JSON, so a caller cannot tell "not covered" from "found and refused" | U-01 | high | contract |
| F-02 | The recall loss log is not written for the losses it exists to count, and each surface defines the loss differently | new | high | correctness |
| F-03 | A miss has no way off an ephemeral machine | U-02 | high | capability |
| F-04 | `kb remember` has no machine readable output, so the one write side piece a hosted agent could call is unreachable | U-08 | high | contract |
| F-05 | `guess` means abstention to the evaluator and delivery to the consumer | U-03 | medium | measurement |
| F-06 | `suggestions` is empty in the case it was built for | U-04 | medium | correctness |
| F-07 | The index error names a symptom and not the fix | U-05 | low | ergonomics |
| F-08 | Process start costs more than the search, and there is no batch door | U-07 | low | performance |
| F-09 | No macOS binary and no platform resolving installer | U-06, corrected | low | adoption |
| F-10 | Our own documented spawn cost is off by more than ten times on the platform deployments use | new | low | documentation |

---

### F-01 (high) The gate lives outside the JSON

**Symptom.** `kb route --json` returns `verdict: "nothing"` together with a `results` array that
contains the correct file, sometimes in position one. In the integrator's paraphrase set this happened
four times out of ten. One of them was the definition of the metric their whole integration exists to
serve.

**Mechanism.** `Memory::ask` runs two scorers and fuses them for the reading list, then derives the
verdict from the keyword ranking alone: `memory.rs:613` builds `found` out of `fuse(&keyword, &text)`,
and `memory.rs:634` returns `Verdict::Nothing` the moment the keyword hit list is empty. The text
scorer contributes to `agreement` at `memory.rs:663` and to nothing else. So "the keyword scorer found
no term" and "the base does not cover this" are the same output, and `results` carries the difference
that the verdict throws away.

This is not a bug in the gate. ADR-0018 put the verdict on the keyword ranking deliberately and
`MIN_MARGIN` records the measurement that kept it there. The defect is that the JSON does not say
which question the verdict answered.

**Why it hurts an integrator specifically.** The rule that a `nothing` verdict must never reach the
model is written in the prose of `--help` and in `tools/kb/README.md`, not in the payload. A caller
that reads `results.length > 0` and serves what it finds is serving passages we consider not found. A
caller that trusts the verdict discards passages we did in fact find. Both readings are reasonable and
we ship no field that settles it.

**The change.** Say it in the payload. Three parts, and the first one is the contract:

1. A boolean that states whether `results` passed the gate, for example `served: false`, so no caller
   has to re-derive our rule from a string.
2. A named state for "the text scorer ranked it and the keyword scorer did not", distinct from
   "nothing ranked at all in either scorer". Today `verdict: "nothing"` covers both and they call for
   opposite handling: the first is a keys problem in the base, the second is a coverage problem.
3. The floor the verdict was measured against, so a caller can disagree with it without guessing its
   value. `SCORE_FLOOR` is already public in the type and absent from the wire.

**Tests to write first.**

- A base whose note is reachable by the text scorer only: `route --json` returns a verdict that is not
  `hit`, a non-empty `results`, and the new field saying the results did not pass the gate.
- A question no scorer ranks: the same fields, with `results` empty and the new state distinct from
  the one above.
- A clean hit: the gate field says served, and the existing fields do not change. This one exists to
  catch the payload being reshaped under callers who already parse it.
- The floor value in the payload equals `memory::SCORE_FLOOR`, asserted against the constant rather
  than a literal, so the two cannot drift.

**Where.** `route_as_json` at `main.rs:793` prints straight to stdout, so there is nothing a test can
hold. Split it into a function returning `json::Value` and a caller that prints. That split is the
first commit and it is behaviour preserving.

---

### F-02 (high) The recall loss log is not written for the losses it exists to count

**Symptom.** The integrator's miss log had two lines, both written earlier on a development machine.
None of the six production questions appears, and none of the four "found but gated" paraphrases
appears either. They attributed this to the read only filesystem. That explains part of it. It does
not explain the rest.

**Mechanism.** The miss is recorded on empty results, not on a `nothing` verdict, and "empty" means a
different list on each surface:

- `main.rs:796`, `route --json`: `if answer.found.is_empty()`. `found` is the fused list, so any text
  scorer match at all suppresses the record. Every "found but gated" case, which is exactly the class
  F-01 is about, is invisible here on any filesystem, writable or not.
- `mcp.rs:343`, `kb_retrieve`: the same fused test, the same hole.
- `main.rs:717` and `mcp.rs:308`, the terminal `route` and `kb_route`: `hits.is_empty()` over the
  keyword list, which does coincide with a `nothing` verdict.
- `main.rs:1176`, `kb answer`: a fourth definition, found while fixing the other three. It refuses,
  prints the same apology and offers the same vocabulary, and then calls `suggest` without
  `record_miss`. A question that reached the answerer and was refused left no trace at all, and that
  is the surface where somebody most clearly wanted an answer.

So `kb-misses.txt` counts a different population depending on which door the question came through,
and the door a deployment uses is the one with the hole. `misses.rs` opens by calling itself the
measurement both live revisit triggers are written against. It cannot be that while it is defined
four ways.

**Why it hurts.** This is the loop that repairs F-01. A question that the keyword line failed to catch
is supposed to land in the log, earn an alias or a `Search for:` term, and stop failing. The log is
silent in precisely the case where the base already contains the answer and the keys are wrong, which
is the cheapest possible fix and the one we most want reported.

**The change.** Record the miss on the verdict, not on the list length, and define it in one place on
`Memory` so no surface can hold its own opinion. A `nothing` verdict is a recall loss. A `guess` is
arguably one too, and that is a decision to take with a number rather than by feel, so the first
version records `nothing` and the `guess` case is a separate question left open below.

**Tests to write first.**

- A question the text scorer ranks and the keyword scorer does not, against a writable base: the miss
  file exists afterwards and contains the question. This test fails today.
- The same question through `kb route --json` and through the MCP retrieve tool records the same
  thing. One test over both surfaces, because the defect is that they disagree.
- An empty base records nothing, which `memory.rs:438` already promises and must keep.

---

### F-03 (high) A miss has no way off an ephemeral machine

**Symptom.** On a read only filesystem the write fails, one line goes to stderr, and `route` returns
success. In a serverless log, stderr from a child process is where information goes to die.

**Mechanism.** `misses::record` at `misses.rs:68` writes to `path_in(root)`, which is
`fleet root/kb-misses.txt`, chosen deliberately in that module's header so that deleting the index
loses a rebuild and never loses evidence. `memory.rs:441` picks `self.opened.first()` as that root.
The failure path at `misses.rs:103` prints and continues, which is the right call for the query and
the wrong end state for the evidence. There is no environment variable and no return value: the
function signature has nowhere to put a miss it could not store.

**The change.** Two doors, and they are not alternatives:

1. Return the miss to the caller in the payload, for example a `miss` object beside `suggestions`,
   so an integrator can persist it wherever their stack already writes. This is the one that works in
   a bundle with no writable path at all.
2. Honour an override for the log location, so an operator who does have a writable path, `/tmp` on
   most function runtimes, can point at it without changing the base layout.

Door 1 is the contract change and it is the one to build first. Door 2 is a convenience that costs a
few lines and one test.

**Tests to write first.**

- With the base writable, `route --json` on a missed question returns the miss in the payload and also
  writes the file. Both, because the payload is an addition and not a replacement.
- With the log path pointed elsewhere, the file appears at the override and not beside the base.
- With writing impossible, the query still succeeds, the payload still carries the miss, and the exit
  code is unchanged. Simulating an unwritable path portably is the awkward part: putting a regular
  file where the directory is expected fails on both Windows and Linux and is the vector to use.

---

### F-04 (high) `kb remember` cannot be called by a program

**Symptom.** The integrator hand tested the piece that answers the exact question their agent needed,
"is this worth storing", and got three correct classifications with readable reasons. They could not
wire it up. `--json` is accepted and ignored.

**Mechanism.** `main.rs:158` parses `--json` once for the whole process, and the `remember` arm at
`main.rs:272` calls `cmd_remember(claim, &paths, all)`. The flag never reaches it. `cmd_remember` at
`main.rs:1673` prints prose only, and on an open failure it prints to stderr and exits 1 with an empty
stdout, which is the shape `route` was already fixed away from at `main.rs:689`.

The classifier underneath is fine and is not what changes: `remember::assess` at `remember.rs:70`
already returns `Outcome`, `reason` and ranked `Evidence`. This is a missing serialisation, not a
missing feature.

**Why it matters more than its size.** It converts the write half from something that needs a
developer's machine into something a hosted agent can participate in: it proposes, the caller queues
the proposal, and a machine with the repository applies it later with `kb write`. The judgement, which
is the part that is hard and the part we have, runs where the conversation is.

**The change.** `kb remember --json`, emitting outcome, reason and evidence, with the same error
object contract `route` uses. Then document the proposal shape as something a caller may store and
replay, because a payload nobody is told to keep is a payload nobody keeps.

**Tests to write first.**

- ADD, UPDATE and NOOP each serialise with the outcome label, the reason string and the evidence
  array, over the fixtures that already exist in `remember.rs`.
- Containment values round through `score()` exactly as `route` does, so two commands do not print
  numbers differently.
- An unopenable base prints one JSON object with `error` on stdout and exits non zero.
- The usage text names `--json` on the `remember` line. Cheap, and it is the flag being silently
  swallowed that cost the integrator the time.

---

### F-05 (medium) `guess` means two things

**Symptom.** `kb eval` reports abstention on four of four for a question set where one of those four
returned `guess` with a real score and served two files. A reasonable consumer, theirs, treats `guess`
as "serve it with a warning". So the same run supports "it stayed quiet" and "it answered", and both
readings quote the same number.

**Mechanism.** `eval.rs:457` counts an abstention as `verdict != Verdict::Hit`, which folds `guess`
into `nothing`. The type says the opposite at `memory.rs:112`: `Guess` documents itself as a warning
and never a filter, and the passages are returned on purpose.

**The change.** Report the two separately in the eval summary, because they are two different safety
properties. "Refused outright" and "hedged and served anyway" are not interchangeable when the number
is used to decide whether the thing is safe to put in front of people.

**Tests to write first.**

- A gold row expecting abstention, answered with `guess`: it counts in the hedged column and not in
  the refused column. This fails today.
- The same row answered with `nothing` counts as refused.
- The two columns sum to the abstention denominator, which is the arithmetic the front page will
  eventually quote.

---

### F-06 (medium) `suggestions` is empty exactly when it is needed

**Symptom.** In every `nothing` case measured, `suggestions` came back `[]`, including cases where the
base plainly holds neighbouring vocabulary.

**Mechanism.** Same line as F-02: `main.rs:796` computes suggestions only when the fused list is
empty. A `nothing` verdict with text results present skips the call entirely. Where the list really is
empty, `index::suggest` at `index.rs:640` is a trigram overlap over indexed keys, which reaches a typo
or a cognate and never a translation, and it says so. On a 9 entry base asked in different words,
trigram overlap has little to work with.

So there are two separate causes wearing one symptom, and the fix order matters: wire the call to the
verdict first, then judge whether the suggester itself is too strict. Measuring the suggester before
it is being called at all would measure the wrong thing.

**Tests to write first.**

- A `nothing` verdict with non-empty results returns non-empty `suggestions` when the base holds a key
  that overlaps the question. Fails today for the wiring reason.
- The honesty property stays: a question with no overlap at all still returns `[]` rather than the
  nearest thing in the base. A suggester that always answers is a suggester nobody can trust.

---

### F-07 (low) The index error names a symptom, not the fix

**Symptom.** `{"error":"cannot open the index: Permission denied (os error 13)"}` when `.kb/` is
missing from a bundle on a read only filesystem. The integrator passes our error text through to the
person on screen, which is what a good integrator does and what we should write for.

**Mechanism.** `Store::open` at `store.rs:271` calls `create_dir_all` on the index's parent before
opening, so a missing `.kb/` on a read only filesystem fails as a permission error while the real
cause is that the index was never built. `memory.rs:285` then wraps it as `cannot open the index: {e}`.
Permission is the true proximate error and the misleading one.

**The change.** When the index file does not exist, say that first and name `kb index`, keeping the
underlying error after it. The `error` field itself is right and should not change: the integrator
specifically credits it, and `skipped` beside it, for letting them tell "the base does not cover this"
from "the base did not ship".

**Test to write first.** A base where the index path cannot be created reports an error naming
`kb index`, with the underlying cause still in the string.

---

### F-08 (low) The process costs more than the search

**Symptom.** Their measurement, on their machine: about 6 ms to spawn against 2.4 ms of retrieval,
9.6 ms p50 wall clock end to end. Irrelevant next to a model call, and a real tax on anyone who calls
in a loop.

**Mechanism.** No defect. `kb serve` exists and speaks MCP over stdio, which does not fit a stateless
HTTP function, so a serverless caller has spawn and nothing else.

**The change.** Documentation first: state the spawn cost next to the retrieval cost so nobody
discovers it in a loop. A batch mode, many questions in one invocation and one JSON line per answer,
is the cheap version of a server and is worth a decision rather than a reflex. Not scheduled here.

---

### F-09 (low) Platforms. Their U-06, and the report is wrong about it

**The correction.** The report says only a linux-x64 binary is published. Our release workflow at the
`kb-v0.1.0` tag builds and publishes a Windows binary as well, `kb-windows-x64.exe` with its sha256:
`.github/workflows/release.yml:90` and `:109`, present in the tagged revision, checked with
`git show kb-v0.1.0`. What is genuinely missing is a **macOS** binary and any installer that resolves
the platform for you.

Unverified: whether the published release page actually carries the Windows asset. That needs a
network fetch, which I did not do. The workflow says it should.

**Why their experience still counts.** They develop on Windows, their build gate is wrapped in a
script that skips silently off Linux, and their measurements needed WSL. A published Windows binary
that an integrator does not know exists is, from where they sit, the same as no binary. That is a
discoverability defect in our README and it is the actionable half.

---

### F-10 (low) Our documented spawn cost is wrong for the platform that matters

**Mechanism.** `tools/kb/README.md:374` tells an integrator that spawning `kb route --json` per
request "pays the 136 ms every time", citing `benchmarks/latency/RESULTS.md`, which measures a cold
open in process on Windows over the 15 file demo corpus. The field measurement is 9.6 ms p50 for the
whole spawn and answer on Linux over a 148 KB base. Our number is not wrong, it is answering a
different question on a different platform, and the sentence built on it gives advice that is off by
more than ten times in the direction that makes people build a server they do not need.

**The change.** State the platform and the corpus beside the number, and add the Linux spawn figure
once we have measured it ourselves. Their number is theirs until we reproduce it, which is why this is
a documentation item and not a benchmark result.

---

## What is working and is not up for redesign

Listed because the next person to touch retrieval will be tempted by all of it.

- **No model in the retrieval path.** Zero failures over the production window, and no way to cite a
  file that does not exist. ADR-0018.
- **`skipped` in the JSON.** The integrator uses it to tell "the base does not cover this" from "no
  base was searched" and shows a warning to the person on screen. Named as the best API decision in
  the set.
- **Read only filesystems answer correctly and do not try to reindex.** This is what makes a bundle
  viable at all, and F-03 must not break it.
- **The `remember` judgement.** Three claims, three correct classifications, reasons a person can
  read. F-04 makes it reachable and changes none of it.
- **A gate that stays quiet.** On the four questions the gold set says to refuse, nothing invented was
  pushed. F-05 is about how we count that, not about the behaviour.

---

## Order of work

Each step is a failing test first, then the change, then the run.

1. **F-01 split. Done, 2026-08-30.** `route_payload` returns a `json::Value` and `route_as_json`
   prints it. No behaviour change: 233 tests pass and the CLI still emits one line that the release
   workflow's `jq` check accepts. Three characterization tests came with it, in `main.rs`, and two of
   them pin defects on purpose: the gated result set that looks like an uncovered question, and the
   empty `suggestions` beside it. Step 2 is an edit to those two tests, which is the point of writing
   them now. The gated case now reproduces locally in a fixture, so F-01 is no longer only a field
   report.
2. **F-01 and F-06 payload. Done, 2026-08-30.** `route --json` carries
   `gate: { served, ranked_by_text_only, floor }`. Three facts rather than one enum: `served` states
   our own rule instead of leaving it to be re-derived from prose, `ranked_by_text_only` names the
   mechanism and not a diagnosis, and `floor` is what `keyword_score` was measured against. The
   suggestion branch moved from `results.is_empty()` to the verdict, which is F-06 and which carried
   the miss recording with it, so the first half of F-03's motivating case is closed on this surface.
   Seven tests in `main.rs`, 237 in the suite, and the state is visible from the real CLI:
   `kb route "blast radius" examples/demo --all --json` returns `nothing` with one result and
   `ranked_by_text_only: true`. README and `--help` updated to say branch on `gate.served`, never on
   the length of `results`.
3. **F-02. Done, 2026-08-30.** `Memory::recall_loss(question, confidence)` decides, records and hands
   back the vocabulary, in one call. Six surfaces now ask it and none of them holds an opinion:
   `route --json`, the terminal `route` in both modes, `kb answer`, and both MCP tools.
   `record_miss` is private, and `SUGGEST_LIMIT` moved onto the contract from the two places that had
   their own. A refusal is the loss and a `guess` is not, pinned by a test so that changing it is
   deliberate.

   **A predicate would not have been enough**, and that is the reusable part. Surfaces would have kept
   pairing it with their own `suggest` and their own write, which is three chances to differ. The
   whole path moved, so there is nothing left at a call site to get wrong.

   **Running it caught what the tests did not.** The first fix put the call inside
   `print_suggestions`, which the hybrid terminal path calls only when the fused list is empty, so a
   refusal over passages it went on to print still recorded nothing: the same defect, one level down.
   Driving four surfaces over one question against `examples/demo` showed a count of 1, 1, 2, 3 where
   it should have been 1, 2, 3, 4. Two more tests, then the decision was hoisted out of every printing
   branch. It now reads 1, 2, 3, 4, and a hit adds nothing. 245 tests pass.
4. **F-03. Done, 2026-08-30.** `Memory::recall_loss` returns a `RecallLoss` instead of a word list:
   question, `looked_like`, date, the path it wrote to, and `error` when it could not. `route --json`
   carries it as `miss`, self contained so a caller with nowhere to write can persist it whole, and
   `null` when the answer was served. `misses::record` returns its failure instead of only printing
   it, so the reason reaches the caller rather than the stderr of a child process. `KB_MISSES_PATH`
   names a file and moves the log; an empty value is treated as unset, because a platform that
   exported the name and chose no value has not chosen a path.

   **The environment read is one line and is deliberately not unit tested.** `path_for` takes the
   override as an argument and is tested pure; setting a process wide variable in a test that runs in
   parallel with every other test in the binary is a race, and the neighbouring tests write miss logs
   of their own. The one line that reads the environment was verified by running the binary:
   `KB_MISSES_PATH=<scratch>/losses.txt kb route ... --json` wrote there, wrote nothing beside the
   base, and reported that path in `miss.log`.

   **The unwritable case was run, not reasoned about.** Pointing the log at a path that cannot be
   written, on 2026-08-30: `verdict: "nothing"`, one result, `gate.ranked_by_text_only: true`, exit 0,
   and `miss.recorded: false` with `miss.error` carrying `os error 5`. That is the deployment shape
   the report described, with the caller now holding the only copy that will exist. Not run against a
   genuinely read only mount, which is a different thing and is marked as such in the README.
   251 tests pass.
5. **F-04. Done, 2026-08-30.** `kb remember --json` emits claim, proposal, reason, ranked evidence
   with containment through the same rounding every other number uses, and `notice`, which carries the
   caveat the terminal prints so a model reading this through another surface is told what a person is
   told. The flag reaches the command now: it was parsed process wide and dropped on this one arm.

   `open_error_as_json` is shared with `route`, because the two disagreed about failure: `route`
   printed a parseable object and `remember` printed nothing at all on stdout, so a program calling one
   got a failure it could read and a program calling the other got exit 1 and silence.

   The payload builder takes the `Assessment` and not the memory, so all three outcomes are pinned
   without arguing with the classifier about which one a fixture produces. What `assess` decides is
   tested beside `assess`.

   **All three ran, against `examples/demo`, on 2026-08-30**: NOOP at containment 1.000, UPDATE at
   0.625 naming the three words the claim adds, ADD at 0.250. The error object came back on stdout
   with exit 1 for a base that does not exist. 257 tests pass.

   The README documents the queue-and-apply pattern the report asked for: ask at the moment the fact
   appears, store the object, and on a machine with the repository act on it. `UPDATE` names the file
   to edit in `evidence[0]`, `ADD` becomes a `kb write`, `NOOP` is dropped.
6. **F-05. Done, 2026-08-30.** `Summary` carries three columns where it carried one:
   `abstention_refused` (verdict `nothing`), `abstention_hedged` (verdict `guess`, served with a
   warning) and `abstention_answered` (called a hit, which is the outright failure). A test asserts
   they sum back to the denominator, because arithmetic that does not close is how a column goes
   missing without anybody noticing. The printed line reads
   `of 3 question(s) the set says to decline: refused 3, hedged 0, answered 0`.

   **Demonstrated on a gold set built to produce the confusion**, since the demo set refuses all three
   of its abstain rows and would have looked identical either way. Marking a question the router hedges
   on as an abstain row: `of 2 question(s) the set says to decline: refused 1, hedged 1, answered 0`.
   The old code printed `abstained on 2/2` for that, which is the number the report objected to.

   One thing fixed in passing, one line above, because it prints in the same block: a collapsed line
   continuation had left its indentation inside a string, so the AGENT keyword line read
   `the fallback          when no classifier answers`. Cosmetic, and it is output somebody reads.
7. **F-07. Done, 2026-08-30.** `OpenError::Store` carries the path and whether an index was there at
   all, asked before the open because opening is what creates it. Two messages: a missing index names
   the cause, then `kb index`, then the deployment half of it, and keeps the underlying error in
   brackets; an index that exists and will not open gets the reason and none of that advice, because
   building one is not the fix and sending somebody there is the same defect wearing the opposite
   sign. Both branches have a test and both were read out of the real binary:

   `cannot open the index at <path>: nothing has been indexed here yet. Run \`kb index\` on the base,
   and make sure the .kb/ directory it writes reaches the machine that answers the question.
   (os error 183)`

   The remedy is deliberately not put in `cmd_index`'s own error path, which opens the same store:
   telling somebody to run `kb index` while they are running `kb index` is worse than saying nothing.

   **Three mangled strings fixed along the way, two of them pre-existing.** A collapsed line
   continuation leaves its indentation inside the literal, so the text prints with a run of spaces in
   the middle of a sentence. It had happened to `checks.rs`'s W06 message and to the eval line about
   excluded questions, both of which a person reads. Not in scope for F-07 and named here rather than
   changed silently. 261 tests pass.
8. **F-09 and F-10. Done, 2026-08-30.**

   **F-09.** The root README already named both published binaries; `tools/kb/README.md`, which is the
   file carrying every word of deployment guidance, did not, and said only "the binary lands in
   `target/release/kb.exe`". So the integrator read the deployment section and never learned a Windows
   binary is published. Both files now name the two artifacts and state that there is no macOS build,
   with the reason: nobody here runs macOS, so a published artifact would be one nothing has ever
   executed, which is worth less than its absence.

   **F-10, measured.** Spawn, open and answer, release build, `examples/demo`, 40 samples after 3
   warm-ups, on the same Windows laptop the 136.4 ms figure came from:

   ```
   kb route "como faco rollback sem downtime" examples/demo --all --hybrid --json --top 4
   p50 184.8 ms   min 145.8   p90 252.2   max 308.7
   ```

   Timed from Python with `subprocess.run` and output to `DEVNULL`, which is the shape the integrator's
   Node `execFile` has. Measured first through PowerShell's `Measure-Command`, which reported p50
   298 ms, and discarded: that harness spends more time on its own pipeline than on the process it is
   timing, and a benchmark that measures its own instrument is not a benchmark.

   So the sentence the README used to carry, "spawning per request pays the 136 ms every time", was
   wrong twice. On Windows the real figure is 185 ms, not 136. On Linux, where deployments run, the
   integrator measured 9.6 ms for the whole thing, twentyfold less, and the advice built on the old
   number pushed people toward a long-lived process they do not need. Both READMEs now carry the three
   numbers in a table with their platform, corpus and date, and say which one was not run here.

   **The Linux figure is still theirs.** WSL is installed on this machine and has no Rust toolchain,
   and installing one is a change to somebody's machine that is not mine to make unasked. Stated in the
   README as not reproduced rather than quietly adopted.

F-08 and the macOS binary are not scheduled. Both are real and neither blocks anybody today.

## What is not decided here

- Whether a `guess` verdict is a recall loss. It changes what `kb-misses.txt` counts, and the two
  revisit triggers written against that file are the reason to settle it with a measurement rather
  than in this paragraph.
- Whether the new "text found it, keys did not" state is a third verdict or a field beside the
  existing three. A third verdict breaks every caller that matches on the label, so the field is the
  reversible option and is the one to build unless the eval says otherwise.
- Batch mode. It is a decision, not a task.
