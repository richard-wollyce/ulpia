# Contributing

Ulpia is one person's fleet made public, so contributions are welcome and the bar is
the repository's own: name the mechanism, mark what is unverified, and never claim
something works without running it. Small, argued changes land; large unexplained
ones do not.

## Sign your commits (DCO)

Every commit must carry a Developer Certificate of Origin sign-off:

```
git commit -s -m "your message"
```

which appends `Signed-off-by: Your Name <you@example.com>`, your statement of the
[Developer Certificate of Origin](https://developercertificate.org/) that you have
the right to submit the work under this repository's licence.

Why this exists, stated plainly rather than as boilerplate: the project is Apache 2.0
and intends to stay open, and the sign-off is what keeps the licensing history of
every line auditable. A pull request without sign-offs will be asked to add them
before review, not after.

## The ground rules

- **Run it.** A change to `tools/kb` comes with `cargo test` green and, where the
  change touches routing, the demo eval run and quoted:
  `kb eval examples/demo/gold.tsv examples/demo`.
- **No em dashes.** House style, enforced by the linter (`kb check` reports W03).
- **Nothing under `fleet/` ever lands in a commit.** It is gitignored private
  knowledge; a PR that adds an exception to that rule will be declined without
  ceremony.
- **Numbers carry their method.** A performance claim names the machine, the set and
  the command, or it stays out of the docs.
- **Security reports go to the door**, not the issue tracker: see
  [SECURITY.md](SECURITY.md).

## Where to start

`tools/kb/src/memory.rs` is the contract every surface goes through, and the
decision records under `decisions/` are the why behind almost everything you will
want to ask about. Read the record before proposing to reverse it; several of them
document ideas that were built, measured and removed.
