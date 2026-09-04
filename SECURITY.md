# Security

Ulpia handles a person's private knowledge on their own machine, so a vulnerability
here is a privacy problem before it is anything else. The disclosure door has existed
since day one, per decision record 0019; this file is the sign on it.

## Reporting a vulnerability

Write to **security@ulpia.io**. Do not open a public issue for anything that could
expose a user's private layer, and do not attach a real fleet's content to a report;
a minimal reproduction against the generated `agent-skeleton` is enough.

You will get an acknowledgement within a few days. Ulpia is maintained by one person
with a fleet of agents, so the honest promise is a prompt first reply and a fix
prioritized by blast radius, not a corporate SLA.

## What counts

Anything that moves private data across the lines the design draws:

- The privacy gate: a base declares its own private layer, as a `private =` line in
  `agent.txt`, defaulting to `profile/`, `projects/` and `records/`. What that line
  covers must never be served, indexed, or suggested unless the caller passed `--all`.
  A path that dodges that rule is a vulnerability even when nothing sensitive was
  behind it on your machine.

  **This rule changed on 2026-09-01 and this file said the old one until 2026-09-03.**
  It used to read "anything git does not track must never be served", which is what
  the code did until [ADR-0034](decisions/0034-git-leaves-the-runtime.md) took git out
  of the runtime. Anybody who read this file and went looking for a hole would have
  tested a gate that no longer exists. The rule to test is the declaration, read off
  the base itself, with no git anywhere in the path.
- The boot hook and the MCP server run locally and send nothing anywhere. Anything
  that makes them do otherwise is a vulnerability.
- The promotion pipeline must not write without its reviewer. A way to make it do so
  is a vulnerability, not a feature request.
