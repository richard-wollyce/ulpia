# The ten findings, one paragraph each

A digest of what was done to the findings in
[`2026-08-29-first-integration.md`](2026-08-29-first-integration.md), written to be read in one
sitting. That file carries the mechanism, the evidence and the tests for each one; this is the state.

| | |
|---|---|
| Worked | 2026-08-30 |
| Closed | 9 of 10. F-08 was never scheduled and is still open |
| Tests | 230 before, 261 after. Every change was a failing test first |
| Files | `main.rs`, `memory.rs`, `mcp.rs`, `misses.rs`, `eval.rs`, `checks.rs`, both READMEs |
| Not run | No genuinely read only mount, no Linux measurement of our own. Both marked where they are claimed |

---

## F-01 (high, done) The gate now speaks in the payload

The verdict answered "did the keyword scorer rank anything" and `results` answered "did either of
them", so a refusal over real candidates and a subject the base does not cover arrived identically:
`verdict: "nothing"` with an array beside it. `route --json` now carries
`gate: { served, ranked_by_text_only, floor }`. Three facts rather than one enum, because each is
separately checkable and none needs versioning the day a fourth state appears: `served` states our own
rule instead of leaving a caller to re-derive it from a paragraph of `--help`, `ranked_by_text_only`
names the mechanism and not a diagnosis, and `floor` is what `keyword_score` was measured against so
the gate can be argued with. Getting there needed `route_as_json` split into a `route_payload` that
returns a `json::Value`, because while it printed there was nothing a test could hold, which is why
the first report of this contract's shape came back from somebody's production.

## F-02 (high, done) One definition of a recall loss, where there were four

`kb-misses.txt` is the file both live revisit triggers are measured against, and six surfaces decided
its contents four different ways: two tested the length of the keyword list, two the fused list, and
`kb answer` suggested vocabulary without ever recording anything. The surface a deployment uses had
the hole in it, so the loss most worth having, a question the text scorer answered and the gate
refused, went unrecorded everywhere it mattered. `Memory::recall_loss(question, confidence)` now
decides, records and hands back the vocabulary in one call, `record_miss` is private, and
`SUGGEST_LIMIT` moved onto the contract from the two places that had their own. A predicate would not
have been enough: surfaces would have kept pairing it with their own `suggest` and their own write,
which is three more chances to differ.

## F-03 (high, done) The loss travels off the machine that could not keep it

The log lives beside the fleet, which is right on a machine somebody owns and impossible on a hosted
one, and a failed write reached the caller as one line on the stderr of a child process while `route`
exited 0. `recall_loss` now returns a `RecallLoss` with the question, the vocabulary, the date, the
path it wrote to and `error` when it could not, and `route --json` carries it as `miss`, self
contained so a caller with nowhere to write can persist it whole. `KB_MISSES_PATH` names a file and
moves the log, with an empty value treated as unset. The environment read is one line and deliberately
has no unit test: setting a process wide variable in a test that runs beside every other test in the
binary is a race, and the neighbours write miss logs of their own, so the branch is tested pure and
the line was verified by running the binary.

## F-04 (high, done) The write side's one reachable half became reachable

`--json` is parsed once for the whole process and the `remember` arm dropped it, so the flag was
accepted, ignored, and answered with terminal prose. The integrator measured by hand the one piece
their agent needed, the judgement about whether a fact is worth storing, and could not wire it up.
`kb remember --json` now emits claim, proposal, reason and ranked evidence, with containment through
the same rounding every other number uses, plus `notice` carrying the caveat the terminal prints so a
model reading this through another surface is told what a person is told. The payload builder takes
the `Assessment` and not the memory, so all three outcomes are pinned without arguing with the
classifier about which one a fixture produces. A divergence surfaced while doing it: `route` printed a
parseable error object and `remember` printed nothing at all on stdout, so `open_error_as_json` is now
shared.

## F-05 (medium, done) Refusing and hedging stopped sharing a number

`kb eval` counted anything that was not a `Hit` as an abstention, so a `guess` that served two files
scored as silence, and one run supported both "it stayed quiet" and "it answered" while quoting the
same figure. The summary now carries `abstention_refused`, `abstention_hedged` and
`abstention_answered`, with a test that they sum back to the denominator, because arithmetic that does
not close is how a column goes missing unnoticed. The demo gold set refuses all three of its abstain
rows and would have looked identical either way, so the split was demonstrated on a set built to
produce the confusion: `refused 1, hedged 1, answered 0`, where the old code printed `abstained on
2/2`.

## F-06 (medium, done) The suggestions arrive when they are needed

`suggestions` was computed only when the fused list was empty, so the one case it was built for, a
refusal the person on screen has to recover from, was the case where it returned nothing. The
condition moved from the list length to the verdict, which is the same line F-02 fixed and the reason
the two landed together. The honesty property was kept and has its own test: a base with no
orthographic neighbour of any question word still offers nothing rather than the nearest thing it
holds, because trigram overlap measures spelling and never meaning, and a suggester that always
answers is one nobody can use.

## F-07 (low, done) The index error names the cause before the symptom

A bundle that left `.kb/` behind got `cannot open the index: Permission denied (os error 13)`, and
permission is the true proximate cause and the misleading one, because `Store::open` creates the
index's parent before opening. `OpenError::Store` now carries the path and whether an index was there
at all, asked before the open because opening is what creates it. A missing index names the cause,
then `kb index`, then the deployment half of it, keeping the underlying error in brackets; an index
that exists and will not open gets the reason and none of that advice, because building one is not the
fix. The remedy is deliberately absent from `cmd_index`'s own error path: telling somebody to run
`kb index` while they are running `kb index` is worse than saying nothing.

## F-08 (low, NOT DONE) The process still costs more than the search

About 6 ms to spawn against 2.4 ms of retrieval on the integrator's machine. There is no defect here
and nothing was changed: `kb serve` exists and speaks MCP over stdio, which does not fit a stateless
HTTP handler, so a serverless caller has spawn and nothing else. A batch mode, many questions in one
invocation and one JSON line per answer, is the cheap version of a server and is worth a decision
rather than a reflex. The documentation half of it was answered by F-10. **This is the one finding
still open, and it blocks nobody today.**

## F-09 (low, done) The report was wrong about the binaries, and the real gap was next to it

The report said only a linux-x64 binary is published. The release workflow at the `kb-v0.1.0` tag
builds and publishes `kb-windows-x64.exe` with its sha256, which `git show` confirms, and the root
README already documented both. What was actually missing: `tools/kb/README.md`, the file carrying
every word of deployment guidance, said only "the binary lands in `target/release/kb.exe`", so an
integrator could read the whole deployment section and never learn a Windows binary exists. Both files
now name the two artifacts and state that there is no macOS build, with the reason rather than the
silence: nobody here runs macOS, and a published artifact nothing has ever executed is worth less than
its absence.

## F-10 (low, done) Our own performance advice was wrong twice

The README told integrators that spawning per request "pays the 136 ms every time", citing a figure
that is an in-process cold open measured on a Windows laptop over the 15 file demo corpus. Measured
here on 2026-08-30, release build, same corpus, 40 samples after 3 warm-ups: the real spawn, open and
answer is **p50 184.8 ms** on Windows, not 136. On Linux, where deployments run, the integrator
measured **9.6 ms** for the whole thing, twentyfold less, so the advice built on the old number pushed
people toward a long-lived process they do not need. Both READMEs now carry the three numbers in a
table with platform, corpus and date. The Linux figure stays theirs: WSL here has no Rust toolchain,
and installing one is a change to somebody's machine that was not asked for, so the README says the
row was not reproduced rather than quietly adopting it.

---

## What a consumer of `kb route --json` has to know

Two fields are new and one branch is now wrong to write.

- **`gate`** is the field to branch on. `gate.served`, never `results.length`. A refused answer still
  carries its candidates, so `verdict: "nothing"` with a full array is an ordinary outcome.
- **`miss`** is `null` on a served answer and carries the whole recall loss on a refusal, including
  `recorded` and `error` when the log could not be written.
- Nothing was removed or renamed. Every field that existed before still exists and means the same
  thing, which the first test in the set pins by name.

## What running found that reading did not

Three defects in this batch were absent from the field report and from my own reading of the source,
and all three turned up by executing something.

- **`kb answer` never recorded a recall loss at all.** A fourth definition, found while unifying the
  other three.
- **The hybrid terminal path kept swallowing the record after F-02 was "done".** The fix had been put
  inside `print_suggestions`, which that path calls only when the fused list is empty, so the same
  defect survived one level down. Three tests passed over it. Driving four surfaces over one question
  and reading counts of 1, 1, 2, 3 where 1, 2, 3, 4 was owed is what caught it.
- **A benchmark that measured its own instrument.** The first timing of F-10 went through PowerShell's
  `Measure-Command` and reported p50 298 ms; that harness spends more time on its own pipeline than on
  the process it is timing. Re-run through a harness shaped like the integrator's `execFile`, the
  answer was 185 ms.

## Two decisions left open on purpose

- **Whether a `guess` is a recall loss.** Today it is not: it was served, with a warning, and a
  question that reached the caller is not one the base failed to reach. Moving that line changes what
  `kb-misses.txt` counts, and both live revisit triggers are measured against that file, so it should
  move on a measurement. A test pins the current answer so changing it has to be deliberate.
- **Batch mode, F-08.** A decision, not a task.
