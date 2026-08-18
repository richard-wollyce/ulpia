# Decisions

Architectural decision records. Anything that will still be constraining us in three months and would
otherwise be re-argued from memory.

**Rules:**

- Numbered sequentially, dated, written with the ADR template in the architect agent's base.
- **Never edited after acceptance.** A change of mind gets a new ADR that supersedes the old one, and
  the old one keeps its text so the reasoning stays auditable. What we used to believe, and why we
  stopped, is the most useful part of the record.
- Every ADR carries a **revisit trigger**: the fact that would make us reopen it. A decision with no
  trigger gets defended long after it stopped being right.
- At least two real options, each with cost, failure mode and what it forecloses. "Do nothing" is
  almost always one of them.

**Scope:** this folder holds what cuts across everything, including decisions about this repository and
about the agents. Project specific decisions live in `projects/<project>/decisions/`.

| # | Decision | Status |
|---|---|---|
| [0001](0001-repository-shape.md) | Zed is built on Yaron's three file split, extended for engineering | accepted |
| [0002](0002-evidence-ruler.md) | The evidence ruler for claims about software | accepted |
| [0003](0003-knowledge-storage.md) | Files stay the source of truth, the index is derived | accepted |
| [0004](0004-local-first-inference.md) | Local first inference, and the role split it forces | proposed |
| [0005](0005-wake-with-the-constitution.md) | The agent wakes with its constitution, not with its library | proposed |
| [0006](0006-language-architecture.md) | One canonical language in the core, every language at the edge | proposed |
| [0007](0007-memory-architecture.md) | The memory pipeline, and provenance as a first class field | accepted |
| [0008](0008-single-user-open-source.md) | Build for one self hosted user, keep the hosted service possible | accepted |
| [0009](0009-gui-runtime-boundary.md) | The GUI is a client of a contract, and the runtime is a choice inside it | accepted / proposed |
| [0010](0010-memory-as-mcp-server.md) | The memory system ships as an MCP server, so our GUI stops being the only door | proposed |
| [0011](0011-fleet-layout.md) | The fleet has one shape, and the library still accepts any path | proposed |
| [0012](0012-naming-and-hosting.md) | The system is Vesta, and everything lives under a personal name | accepted |
