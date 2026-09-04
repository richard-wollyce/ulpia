---
provenance: agent
stage: derived
---

# ADR-0037: we do not build sync, we name what must survive and prove the copy still answers

**Search for:** `backup`, `backups`, `copia de seguranca`, `restaurar`, `restore`, `restauracao`, `sobreviver`, `survive`, `sync`, `sincronizacao`, `sincronizar`, `Syncthing`, `rsync`, `restic`, `robocopy`, `tar`, `perdi o notebook`, `lost laptop`, `notebook roubado`, `stolen laptop`, `disco morreu`, `dead disk`, `maquina`, `machine`, `migrar de maquina`, `trocar de computador`, `kb backup`, `backup --list`, `backup --verify`, `kb-misses.txt`, `kb-rejections.txt`, `.kb`, `indice derivado`, `derived index`, `descartavel`, `disposable`, `.kb-promote.lock`, `lixo de runtime`, `runtime debris`, `nuvem`, `cloud`, `bucket`, `S3`, `criptografia`, `encryption`, `criptografar antes de enviar`, `client side encryption`, `terceiro`, `third party`, `custodia`, `custody`, `disco em rede`, `disco externo`, `LAN`, `another disk`, `fora do predio`, `predio`, `gitignore`, `camada privada`, `private layer`, `profile`, `projects`, `records`, `ADR-0003`, `ADR-0034`, `ADR-0037`, `estudo comparativo`, `comparative study`, `verificacao`, `verification`, `kb check`, `kb eval`, `gold set`, `conjunto ouro`

**Exists to:** Why `kb` will never copy your files anywhere, what the exact set of files that must survive a lost machine is, how you prove a copy of it is still a working base, and the three destinations with the one that cannot be taken without encryption.

- **Date:** 2026-09-02
- **Status:** accepted. **The verb is not built.** See the note at the end.
- **Scope:** system. Binds `kb` and everyone who runs a fleet on a disk they own.
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. Nothing here changes a format, a schema or a path. Two read only
  verbs are added and either can be deleted without a migration. What is expensive to reverse is the
  position, not the code: once the README says we do not do this, saying later that we do is a
  product change and not a patch.

## Context

[ADR-0034](0034-git-leaves-the-runtime.md) took git out of the runtime. That was right for every
reason listed there, and it had a consequence nobody wrote down at the time: **git was also the only
backup story this system had, and it was never a good one.** "Anybody who wants their fleet versioned
still has `kb commit`" is a sentence about versioning. It is not a sentence about what happens when
the disk dies.

Two things then arrived on the same day.

The first is the [hosted memory layer study](../reports/2026-09-02-a-hosted-memory-layer-read-against-ours.md). The subject is a hosted archive
held against an account, so a stolen phone loses nothing. Our row in that table reads, verbatim: "No
backup story at all. `fleet/` is its own git repository and whether it was ever pushed is the owner's
business." The study's summary is blunter: "Ours has no backup story at all, and the README's one
sentence names an operation with no destination." **This is one of exactly three rows in that study
where their design is ahead of ours, and it is the only one of the three we can close without
building a server.**

The second is that sentence. `README.md:342` says: "Backup, sync, and moving to a new machine are all
the same operation." It is true and it is useless. It names an equivalence and no destination, no
file list and no way to tell whether the copy works. A reader takes it as reassurance and it is not
one.

**Richard's decision, taken before this record and recorded here rather than argued: we do not build
sync.** Sync is a distributed systems problem. It is conflict resolution, causality, clock skew,
partial writes and deletion propagation, all of which have to be right before any of it is worth
anything, and none of which is about knowledge bases. We would ship a worse Syncthing, and Syncthing
is free.

What is left after that subtraction is the part only this tool can answer, and it is two questions no
general purpose copier can answer for us:

1. **Which files must survive**, given that a third of what sits in a fleet is derived and a small
   part of it is debris.
2. **Whether a copy is still a base**, which is not a file count and not a checksum.

Everything below is those two.

## What git as a backup actually copies, and why it is the wrong half

This has to be stated because it is the mistake a careful person makes. The fleet is a git
repository, so the intuitive backup is a push to a private remote. Here is what that copies.

*Checked on this machine, 2026-09-02, with `git check-ignore` inside `fleet/`:* `records/` is
ignored, `projects/` is ignored, `.kb/` is ignored. The published `agent-skeleton/.gitignore` that
`kb init` writes ignores `profile/`, `records/`, `projects/` and `.kb/` by name, and the root
`.gitignore` ignores `kb-misses.txt`, `kb-rejections.txt` and `.kb-promote.lock`.

**So a push backs up the complement of the private layer.** That is not a bug in the ignore file.
The ignore file is a publication rule and it is a correct one: those folders are gitignored because
they are nobody's to publish. But a publication rule is not a backup rule read backwards, and reusing
it as one loses precisely the files that cannot be recovered from anywhere else.

The failure modes are asymmetric, and they run the opposite way from the ones the skeleton's ignore
file was written against. That file says, about publication: "forgetting to ignore puts somebody
else's material in the history permanently, while forgetting to un-ignore only means a note of ours
is not backed up." For a backup the asymmetry flips. Forgetting to include loses a file forever.
Forgetting to exclude costs disk. **So the publication list is a deny list with a few exceptions, and
the backup list has to be an allow list with a few subtractions.** Same principle, prefer the failure
you can see, applied in the direction the failure actually runs.

## ONE: what must survive

Stated as a rule a program can follow, because a rule a person has to interpret is a rule that loses
a folder.

> **Take the fleet root and everything under it, then subtract exactly three patterns:**

| Subtract | Where | Why it is not in the set |
|---|---|---|
| `.kb/` | one per base, at any depth | The derived index. Disposable by [ADR-0003](0003-knowledge-storage.md): "delete `.kb/` and you have lost a rebuild, not a fact." `kb index` regenerates it from the markdown beside it |
| `.kb-promote.lock` | fleet root | Runtime debris. `promote.rs:238` calls it the running-now marker. It survives a crash on purpose so the next run can see that one died, which is a fact about **this** machine and a lie on any other |
| `kb-misses.txt.lock` | beside the log, wherever the log is | The same thing one file over. `misses.rs` appends `.lock` to the log path while one writer holds it, and treats it as stale after 30 seconds. Restoring it restores a claim that a writer which does not exist is mid write |

**Everything else is in.** In particular these four, which are the ones a person copying by intuition
leaves out:

- **`profile/`, `projects/`, `records/`** in every base, plus any folder a base names in its own
  `private =` line, plus the whole of `person/`. This is the private layer of ADR-0034. It is
  gitignored by design, it is the part of the base that is about a person rather than about a
  subject, and **nothing anywhere else holds a copy of it.**
- **`kb-misses.txt`**, at the fleet root or wherever `KB_MISSES_PATH` points. `misses.rs` states the
  test in its own header: "Deleting the index should lose a rebuild; deleting this loses evidence
  that cannot be recomputed." Every line is a real question the base could not answer, counted. No
  rebuild produces it, no reindex recovers it, and it is the only instrument that says whether the
  design converges.
- **`kb-rejections.txt`**, at the fleet root. The same species. It records what the promoter refused
  and why, and [ADR-0030](0030-two-promoters-and-the-second-is-not-a-second-opinion.md) counts
  refusals because a repeated refusal is a gap in the base. Derivable from nothing.
- **`kb-aliases.txt`**, at each base root. Small, and easy to skip for being small. It is a record of
  misses rather than a dictionary, so every line in it was paid for by a question that failed once.

**The three line subtraction is the whole rule**, and it has the property an enumerated include list
does not: a file `kb` starts writing tomorrow is in the backup by default, and a new derived artefact
costs one line. The list of things to remember is three long and it is written here.

If the fleet is a git repository, `.git/` travels with it under this rule. That is the owner's call
rather than a requirement: it costs 7.0 MB here and buys the history.

### What the subtraction is worth, measured

*Run on this machine, 2026-09-02, on the real fleet and on `examples/demo`.*

| | files | bytes |
|---|---|---|
| the fleet directory, everything | 1,720 | 35,256,155 |
| of which `.kb/` indexes | 13 | 11,218,944 |
| of which `.git/`, the fleet repository and two nested ones | | 6,987,836 |
| everything that is neither | 479 | 17,049,375 |
| the markdown alone, which is what the index derives from | 290 | 2,626,559 |

**The index is 4.3 times the markdown it is built from**, and on `examples/demo`, where SQLite's page
overhead dominates a tiny corpus, it is 12 times: 98,304 bytes of index over 8,182 bytes of source.

**The claim this record started from, that excluding the index shrinks the backup by an order of
magnitude, does not survive the measurement, and it is corrected here rather than repeated.** Dropping
`.kb/` takes this fleet from 35.3 MB to 24.0 MB, which is 32 percent and not a factor of ten. The
order of magnitude is real between the index and the markdown it derives from, and that is not the
ratio anyone cares about, because most of a fleet by weight is inbox payload rather than notes. The
direction of the rule is unchanged and the honest number is 32 percent.

**What the exclusion actually buys is the rebuild, not the disk.** Copied the fleet to a scratch
path, deleted every `.kb/`, ran `kb index --all`: 290 files, 2,720 chunks, 13 indexes, **1.97
seconds**. That is the entire cost of leaving the index out, and it is paid once on restore instead
of on every incremental copy of an 11 MB binary file that changes whenever a note does.

## TWO: how you know it worked

**A backup nobody restored is not a backup.** It is a directory with a hopeful name. Every general
purpose tool verifies its own copy, and a checksum proves the bytes arrived. It does not prove that
what arrived is a base, because a base is not a pile of bytes, it is a thing that answers questions.

We can prove the stronger property, and cheaply, because both instruments already ship:

```
kb check --all <restored-copy>
kb eval  <restored-copy>/<gold>.tsv <restored-copy> --all
```

**The mechanism, which is why this is worth more than a checksum.** `kb check` opens the copy as a
fleet, walks every file and resolves every `[[link]]` against what is actually there: a file that did
not arrive is an E01 in whatever links to it, and a file that arrived truncated past its header is an
E02, "no `Search for:` line, so the router builds no entry for it." `kb eval` then asks the gold
set's real questions of the restored copy and grades where they land. **The grading is the test a
file count cannot fail and a broken base cannot pass**: it exercises the index build, the keyword
scorer, the fold and the confidence gate against known answers.

`--all` is load bearing on both lines. Without it the private layer is not opened, and the private
layer is exactly the part git did not have, which is the part most likely to be missing.

*Run, 2026-09-02, debug binary built the same day.* Copied 18 files out of `examples/demo`, leaving
every `.kb/` behind, into a scratch path outside any repository, then ran both verbs against the copy:

```
kb check --all   3 bases, "private layer included", clean, clean, clean
kb eval          FILE  fused 10/10, keyword 10/10
                 AGENT routes 10/10, keyword 10/10
                 GATE  demoted 0/10, refused 3 of 3
```

**The first attempt at that run is the reason the verb is worth having, so it is recorded.** It used
the release binary sitting in `target/release`, built 2026-08-26, and graded AGENT 8/10 with two
correct answers demoted to a guess. Nothing was wrong with the restore. That binary predates
[ADR-0036](0036-the-floor-scales-with-the-corpus.md) and carries the fixed floor of 17.5 instead of
the scaled one. A verification whose result depends on which binary the operator happened to have on
their PATH is a verification that will one day report a disaster that is not there, and a verb that
runs both checks itself is where that gets pinned down.

A base with no gold set can run the first line and not the second, and that is worth saying rather
than papering over: `kb check` proves the copy is structurally whole, `kb eval` proves it still
answers, and only the second is the claim worth making.

## The three destinations

The private layer decides which of these are acceptable, and it decides differently for the third
than for the first two.

### 1. Another disk on the same desk

An external drive, or a second internal one. **No third party sees anything.** It survives the
failure that actually happens most, which is one disk dying. It does not survive theft, fire, flood,
or anything that walks a mounted volume and encrypts what it finds, because at the moment of the copy
the second disk is a folder on the same machine.

### 2. Another machine you own, over the LAN

A NAS, a second desktop, a small box with a disk in it. **No third party sees anything**, and it
survives a dead machine. It survives a dead machine **only if the two sit in different places**,
which is the part that gets assumed and is usually false: two boxes on one desk share a building, a
power supply, a burglar and a flood. A second machine in the same room buys one more disk and one
more motherboard, which is a real gain and is not off site.

### 3. A cloud bucket, encrypted before it leaves

Object storage, or any hosted destination. **It is the only one of the three that survives the
building**, which is the failure the other two are structurally unable to cover.

**It is also the only one where the encryption is not optional, and that has to be stated plainly.**
By rule ONE, `profile/`, `projects/` and `records/` are in the backup set. Those are the folders
ADR-0034 named as the private layer, and they are in the set for the exact reason they are gitignored:
nothing else holds a copy. Uploading that set without client side encryption puts a person's profile,
their projects and their records in plaintext on somebody else's server. That is not a weaker version
of local first. **It is the custody position this product exists to refuse**, and the hosted memory layer study
names it in as many words: "The moment a hosted instance exists, 'your files are the source of truth'
becomes 'our copy of your files is', and every other custody property in this study is downstream of
there being no server."

Encrypt before it leaves and the third party holds ciphertext, which is a different relationship: the
provider holds bytes they cannot read, the same as any other opaque blob. That is acceptable. The
unencrypted version of the same destination is not, and the difference between the two is one flag a
tired person skips at midnight.

### How the third one actually works, because "encrypted" is not a mechanism

Richard chose this destination on 2026-09-02 and asked the question the phrase hides: does the person
bring their own account and their own key, and what does the tool do. Yes, and almost nothing, and
both halves of that are the design.

**Two secrets, and they do different jobs.** The encryption key protects the contents and never leaves
the machine. The cloud credential protects access to the bucket and belongs to the person's own
account. Neither is ever written to a file by us: they arrive in the environment or from whatever
password manager the person already keeps, per the fleet rule that a real credential never goes into
any file, tracked or not. **Losing the encryption key makes the backup unreadable forever.** That is
the guarantee and the failure mode in one sentence, so the key goes somewhere that is not the machine
being backed up, which is the one place it is guaranteed to be lost with.

The size decides the shape. Measured on this fleet on 2026-09-02: 40 MB total, of which 14 MB is the
derived index that rule ONE excludes, leaving **26 MB, of which 2.5 MB is the markdown across 290
files**. At that size deduplication and incremental transfer buy nothing, so a full encrypted snapshot
every time is not a compromise, it is the simpler correct answer.

```
# once. The private half of this key does not stay on this machine.
age-keygen -o ulpia-backup.key

# every backup
kb backup --list | tar -cz -T - | age -r age1... > ulpia-2026-09-02.tar.gz.age
rclone copy ulpia-2026-09-02.tar.gz.age b2:their-bucket/ulpia/
```

**Public key encryption, and the property is not incidental.** The machine encrypts with the public
half and cannot decrypt with it, so **a compromised machine cannot read its own backup history**. A
symmetric passphrase kept on the same disk buys none of that.

```
# restoring, which is the only proof that any of the above worked
rclone copy b2:their-bucket/ulpia/ulpia-2026-09-02.tar.gz.age .
age -d -i ulpia-backup.key ulpia-2026-09-02.tar.gz.age | tar -xz -C /tmp/restored
kb backup --verify /tmp/restored
```

Anyone wanting history rather than dated snapshots feeds the same list to `restic`, which brings
snapshots, pruning and its own integrity check. The list is ours either way, which is the point of
rule ONE: the person chooses the transport and the destination, and neither choice changes what has
to be in the tar.

**What the provider still sees, stated because "encrypted" invites the reader to think nothing.**
Contents and file names travel as ciphertext. The size of each object and the time it appeared do
not. A provider learns that roughly 26 MB arrived at 03:00 on a Tuesday and that it changes at some
rhythm. That is a small residual and it is a real one, and it is the price of the only destination
that survives the building.

**What this costs in money is not the interesting number.** 26 MB at Backblaze B2 pricing is under a
cent a year. The cost of this destination is remembering where the private key is, and that cost is
not paid in money and cannot be paid by the tool.

### The trade, named, because this section does not end in a recommendation

Only the third destination survives the building. Only the third involves a third party. **No
destination on this list does both, and no arrangement of the three removes the choice.** Which one
is right depends on what a particular person's `profile/` and `records/` hold and on which failure
they actually expect, and the tool knows neither of those things. A record that recommended one would
be recommending it to a reader whose threat model it has never seen.

What this record does say is narrower, and it is a requirement rather than a preference: **if the
destination is the third one, the encryption is not a setting.**

## Options

### A. `kb backup --to <dest>`, a verb that copies

`kb` reads the set and writes it to a destination. Cost: **we become the owner of a copier.** The
first issue is "it does not do incremental", the second is "it is slow to a NAS", the third is "it
does not resume a broken transfer", the fourth is a checksum comparison, and every one of those is a
solved feature of rsync, robocopy, restic and Syncthing. Every one of them is also correct to ask
for, which is what makes the trajectory a trap rather than a slippery slope argument. Failure mode: a
copier that runs on a schedule and notices the destination changed is one conflict resolution
decision away from being the sync engine this record opened by refusing. It forecloses nothing
technically. It forecloses the position, which is the expensive part. Rejected.

### B. Build sync

What the hosted memory layer study's row is really asking for, and a hosted archive is how they answer it. Cost:
conflict resolution, causality, clock skew, partial writes, delete propagation and a wire protocol,
none of which is a knowledge base problem and all of which has to be right before any of it is worth
anything. Failure mode: a merge that silently keeps the wrong side of a note, which is the one
failure a memory layer cannot have. **Rejected by Richard directly: we would ship a worse
Syncthing.**

### C. Do nothing

The status quo, and it deserves a fair statement. The files are already portable, no absolute path
exists anywhere inside a fleet, and a person who knows what they are doing runs restic tonight and is
fine. Cost: the README keeps naming an operation with no destination, the private layer keeps being
the part a git push does not carry, and **this stays the row where a two person iOS app is ahead of
us.** The failure mode is the only one that matters here and it is silent until the day it is total.
Rejected, and named as the option the study put its finger on.

### D. Name the set, verify the copy, own neither the transport nor the destination

**Chosen.**

## The decision

Two read only verbs. Neither copies a byte.

```
kb backup --list [path]...      prints the paths that must survive, one per line
kb backup --verify <dir>        opens the copy and runs the check
```

**`--list` prints paths and nothing else**, one per line, no header, no counts, no colour, because
the output is an argument to somebody else's tool and every one of those already takes a file list on
stdin:

```
kb backup --list | rsync --files-from=- / /mnt/backup/
kb backup --list | restic backup --files-from -
kb backup --list | tar -cz -T - -f fleet.tgz
```

**`--verify <dir>` opens the copy as a fleet and runs `kb check --all` over it, then `kb eval` with
the gold set when the base has one**, and exits non zero when the copy does not grade. It exists so
the operator does not have to remember two command lines, the `--all` on each of them, or which
binary they ran it with. Every one of those is a way a verification quietly stops verifying.

**The mechanism, and why the split falls exactly here:** the destination and the transport are where
every product in this space differs from every other, where the existing tools are good, and where
none of our knowledge applies. The set and the verdict are where all of our knowledge is and where
nobody else's tool can help, because no copier knows that `.kb/` rebuilds in two seconds and that
`kb-misses.txt` never comes back. **We ship the half only we can answer and we pipe into the half
everybody else already solved.**

## Consequences

- **What gets easier.** The backup question has a written answer that is a file list rather than an
  equivalence, and the answer is checkable. A restore can be graded instead of hoped over.
- **What gets harder.** The person still has to choose and run a copier. This does not make us
  competitive with an archive held against an account, and the study's row does not flip: it goes
  from "no backup story at all" to "a documented one you operate yourself", which is a smaller claim
  and a true one.
- **What we now maintain.** Three subtraction patterns that have to stay correct as `kb` grows. The
  cost of them being wrong is a lost file, so any new writable artefact beside a fleet has to be
  classified as source, evidence or debris at the moment it is created, and this record is where that
  classification lives.
- **What this makes irreversible.** Nothing in the code. In the prose: once `tools/kb/README.md` says
  we do not do transports, adding one later is a stance reversal that needs its own record.
- **What it does not cover.** Encryption. `--list` prints paths, nothing here encrypts anything, and
  the requirement stated for the third destination is a requirement on the operator's tool. If that
  turns out to be the step people skip, it is a finding, and it belongs in the trigger below rather
  than in a feature nobody has asked for.

## Revisit trigger

- **The first restore that comes back missing something the rule did not list.** That is the only
  evidence the subtraction is wrong, and the file they name goes into the table in ONE the same day.
  Nothing else counts as evidence, including an argument from somebody's intuition, this record's
  author included.
- **A fourth writable artefact beside a fleet that is neither index nor lock.** Three patterns is a
  rule a person holds in their head. Six is a config file, and at six this belongs in the manifest as
  a declaration rather than in the binary as a constant.
- **`--verify` ever needing to write into the copy to verify it.** It is read only by design. If a
  future check needs a scratch index, the verb builds it somewhere else or this record gets
  redecided, because a verification that mutates the artefact it verifies is not one.
- **A fleet where `kb index` takes longer than restoring the index would have.** 1.97 seconds for 290
  files here. The exclusion stops paying somewhere, and `kb index`'s own output is the instrument
  that says where.
- **Anybody shipping a hosted instance of this**, at which point the record is about a different
  product, and what changed is ADR-0034's custody argument rather than this one.

## Notes

- **The verb is not built.** This record is the decision and nothing else. `kb backup --list` and
  `kb backup --verify` do not exist in the binary as of 2026-09-02 and `kb --help` does not name
  them. Everything in TWO was run by hand with `kb check` and `kb eval`, which is what the verb will
  call, and that is why the run is quoted rather than described.
- The hand run found the rough edge `--verify` removes: `kb eval --all <dir>` fails with "cannot read
  \<dir\>: Acesso negado (os error 5)", because `eval`'s first positional is the gold file and a
  directory in that position is read as one. Correct behaviour today, and one more thing standing
  between an operator and a verified restore.
- Measurements: the fleet sizes and the index rebuild timing were taken on this machine on 2026-09-02
  with `find` and the release binary. The restore verification used the debug binary built the same
  day, after `cargo test` passed at 327 tests. Nothing in this record is estimated.
- [The hosted memory layer study](../reports/2026-09-02-a-hosted-memory-layer-read-against-ours.md), section 5.9, asked for this and named
  its shape: "The honest section has to say that a private remote puts the private layer on somebody
  else's server, which is the custody position the product refuses, so the section cannot end in a
  recommendation. It ends in a named trade."
