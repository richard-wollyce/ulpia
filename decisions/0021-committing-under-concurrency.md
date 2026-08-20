---
provenance: agent
stage: derived
---

# ADR-0021: a commit names the paths it commits, and proves afterwards that it took nothing else

**Search for:** `commit`, `commitar`, `git commit`, `kb commit`, `git add`, `git add -A`, `pathspec`, `caminhos`, `paths`, `staging`, `git index`, `indice do git`, `index.lock`, `lock`, `contention`, `retry`, `concorrencia`, `concurrency`, `sessoes paralelas`, `parallel sessions`, `paralelismo`, `multiplos agentes`, `multiple agents`, `race`, `clobber`, `sobrescrever`, `conflito`, `conflict`, `cdc0e52`, `commit message`, `mensagem de commit`, `git log`, `historico`, `auditoria`, `audit trail`, `rastreabilidade`, `pre-commit`, `pre-commit hook`, `hook de commit`, `githooks`, `core.hooksPath`, `git config`, `KB_ALLOW_RAW_COMMIT`, `raw commit`, `escape hatch`, `guard`, `guarda`, `policy`, `worktree`, `isolamento`, `merge`, `lease`, `claim`, `untracked`, `exit 128`, `nested repository`, `repositorio aninhado`, `kb check`, `versionamento`, `git`, `git show`, `git status`, `dirty`, `erro ao commitar`, `arquivos staged`, `unstaged`, `adr-0021`

**Exists to:** How to commit safely when several agent sessions write the same repositories at once, and why kb commit and the pre-commit hook exist.

- **Date:** 2026-08-18
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. One subcommand, one hook, one paragraph in the boot files.

## Context

More than one agent session now writes these repositories at the same time, every day.
That stopped being hypothetical during the session that produced ADR-0020: a parallel
session created `fleet/aldo`, edited the roster and the README, and committed `cdc0e52`,
which **also contains an unrelated backlog edit from this session** because it staged with
`git add -A` while the other was mid write.

**Nothing was lost, and that is the trap.** The damage is not destroyed work, it is a
commit whose message describes half of what it contains. `git log` is the audit trail this
whole system leans on: [[0003-knowledge-storage]] keeps markdown in git precisely so that
who changed what stays answerable, and a commit that lies about its own contents removes
that property silently. Six months later nobody can tell which half of `cdc0e52` was
which without reading the diff.

Richard asked for a rule and for the best practice implemented, on the grounds that this
is now the normal working condition rather than an incident.

## What was measured first

Three git behaviours, tested on 2026-08-18 rather than recalled, because the design rests
entirely on the first one:

| Behaviour | Result |
|---|---|
| `git commit -- <paths>` with another session's file staged | Commits **only** the named paths. The other session's file stays staged and uncommitted |
| `git commit -- <path>` on an **untracked** file | Fails: `pathspec did not match any file(s) known to git`. Needs `git add -- <path>` first |
| `git commit -- <path>` on a **deleted** file | Works, no special case |
| A commit while `.git/index.lock` is held | Exit 128, `Unable to create ... index.lock` |

The first row is the whole thing. **A pathspec on the commit closes the dangerous window
instead of guarding it.** The window in `git add` then `git commit` is that a bare commit
takes the entire index, including whatever another session staged in between; a pathspec
commit never consults the rest of the index at all. That is why no lock is needed for the
failure that actually happened.

## Options

### Option A: write the rule down and rely on it

"Never `git add -A`, never `git commit -a`, always name your paths" in `CLAUDE.md`.

- Cost: nothing.
- Failure mode, and it is disqualifying on its own evidence: **this repository already
  knows what happens to rules that live only in prose.** the quality protocol says a standard that
  lives only in someone's head gets negotiated away, and the em dash rule was moved into
  `kb check` for exactly that reason. The session that produced `cdc0e52` was following a
  `CLAUDE.md` that already said to be careful.
- Forecloses: nothing.

### Option B: the rule, plus a subcommand that implements it, plus a hook that requires it

`kb commit <path>... -m <message>`, and a tracked `pre-commit` hook that refuses a raw
`git commit` unless it came through that subcommand or the operator opted out explicitly.

- Cost: one module, one hook, one config line per repository.
- Failure mode: the hook is a policy gate and not a smart detector. It cannot tell an
  intended multi-area commit from a sweep, because only the person committing knows which
  paths were theirs, so it does not try. It makes the deliberate path the default path.
- Forecloses: nothing. A lease layer remains buildable on top.

### Option C: one git worktree per session

The industrial answer. No shared index exists, so none of this can happen.

- Cost: disk, which is F6 in the private backlog and the binding constraint on this machine, times
  one `target/` and one `.kb/` per session. Plus a merge for every session.
- Failure mode, and it is the one that kills it: **it breaks the product.** The fleet is
  built so every agent's base is readable at once. `kb route` spans all of them, Vesta's
  whole job is answering across them, and Zed is granted autonomy to edit Steve and Yaron.
  Sessions in separate worktrees cannot see each other's current state, so the router
  would be answering from a stale copy of the fleet. Isolating the sessions isolates the
  thing they are supposed to share.
- Forecloses: the live cross agent reading that [[0011-fleet-layout]] exists to provide.

## Decision

**Option B.**

1. **`kb commit <path>... -m <message>`.** It resolves the repository from the paths rather
   than the working directory, refuses paths spanning two repositories (real here, since
   `fleet/` is a separate repository nested inside the public one), `git add -- <paths>`
   then `git commit -- <paths>`, and retries a bounded six times on `index.lock`
   contention.
2. **It reads the commit back and proves it.** The file list is taken from
   `git show --name-only HEAD`, not assumed. Anything in it that was not named is reported
   as an error, and every path that was dirty before and is still dirty after is printed as
   evidence that the other session's work was left alone. **This is the step a person skips
   by hand**, and it is the only reason the tool is worth more than the rule.
3. **There is deliberately no flag meaning everything.** An empty path list is an error
   with a sentence explaining why, because that single affordance is what reintroduces the
   bug.
4. **A tracked `pre-commit` hook**, enabled with `git config core.hooksPath .githooks` in
   each repository, refuses a raw `git commit` and prints the two ways forward. The escape
   hatch is `KB_ALLOW_RAW_COMMIT=1`, deliberately visible: a guard with no way past it gets
   disabled entirely and then guards nothing.

**What this does not solve, stated plainly.** Two sessions editing the same file still
clobber each other, and no git technique fixes that, because the race is at the filesystem
before git sees anything. A claim or lease layer would, and it is not built: it costs every
agent a protocol step, it needs expiry handling for crashed sessions, and the failure it
prevents has not happened once yet. The trigger for building it is in the revisit section
rather than in a guess about how likely it is.

## Consequences

- Every agent gets this by having `kb`, which is one binary all of them already use. That
  answers Richard's requirement that each agent in each session has the capability.
- **Raw `git commit` stops working in both repositories** until the operator opts out by
  name. That is intended and it will be briefly annoying.
- A commit message can now be trusted to describe its own contents, which is the property
  `cdc0e52` cost us.
- The hook lives in `.githooks/` and is tracked, so it travels with a clone. It still needs
  one `git config core.hooksPath .githooks` per clone, because git will not let a
  repository configure its own hook path from tracked content, and that restriction is a
  security feature rather than an oversight.
- Two copies of the hook now exist, one per repository. That is duplication and it is
  accepted at two; see the revisit trigger.

## Revisit trigger

- **A third repository**, at which point copying the hook by hand is the wrong shape and
  `kb guard <repo>` should write it and set the config in one command.
- **The first time two sessions clobber each other inside one file.** That is the failure
  this record deliberately does not solve, and one real occurrence is the evidence that
  buys the lease layer.
- **The first time somebody sets `KB_ALLOW_RAW_COMMIT` habitually.** A guard that is always
  bypassed is worse than none, because it produces the appearance of a control. If that
  happens the guard is wrong and should be replaced, not tightened.
- Sessions ever running on different machines against a shared remote, where the failure
  becomes a push race and none of this applies.

## Notes

The three git behaviours in the table were verified in a scratch repository on 2026-08-18
and each has a test in `tools/kb/src/commit.rs` that fails if git ever changes its mind.
The `cdc0e52` example is in the private repository, so it is named here by hash and its
contents are not quoted.
