# Implementation Plan: 5 Production RAG and Agent Architecture Improvements

Implement five advanced architectural capabilities inspired by Towards AI's *Building LLMs for Production* (2nd Edition) in Ulpia's `tools/kb` engine, executing each feature one-by-one with strict Test-Driven Development (TDD).

---

## Invariants and Principles

- **Strict TDD**: Red-green-refactor cycles using Rust unit tests (`cargo test --manifest-path tools/kb/Cargo.toml`).
- **Zero-API Invariant in Harness**: When running inside an agent harness (e.g. Antigravity), all synthesis, scoring, and reviews are handled by the model in the loop or deterministic local Rust logic. No external API tokens or billing are triggered.
- **Rule W03 Compliance**: ASCII hyphens only (`-`). No unicode em dashes or en dashes in any documentation or code comments.
- **Repository Health**: All 470+ unit tests remain green; `kb check fleet/zed fleet/person --strict` remains completely clean.

---

## 5 Improvements Breakdown

### 1. Small-to-Big Sentence/Section Windowing for `kb answer`
- **Objective**: Decouple search target granularity from context assembly. While retrieval matches exact heading or sentence chunks, `kb answer` expands the matched chunk to its enclosing section boundary (heading + surrounding paragraphs) instead of dumping whole files or providing isolated, out-of-context sentence fragments.
- **Files**:
  - `tools/kb/src/store.rs`
  - `tools/kb/src/answer.rs`
- **TDD Steps**:
  1. Write unit tests in `answer.rs` demonstrating section window expansion around matched passages and capping to boundary limits.
  2. Implement window expansion logic.
  3. Verify clean execution with `cargo test`.

---

### 2. Constitutional Self-Critique Protocols in `kb panel` Ledgers
- **Objective**: Enforce that agent objections in multi-agent review panels (`kb panel`) adhere strictly to constitutional rubrics (mechanism-based, architectural invariants, evidence-backed) rather than vague stylistic preference.
- **Files**:
  - `tools/kb/src/panel.rs`
- **TDD Steps**:
  1. Write unit tests verifying rejection of vague objections and admission of mechanism-backed constitutional objections.
  2. Implement constitutional validator and ledger classification.
  3. Verify with `cargo test`.

---

### 3. Sub-Question Query Decomposition for Vesta Fleet Routing (`kb boot`)
- **Objective**: Decompose compound or multi-intent questions into atomic sub-queries before routing to fleet agents, seating co-owners or convening panels when multiple domains are invoked.
- **Files**:
  - `tools/kb/src/boot.rs`
- **TDD Steps**:
  1. Write unit tests for query decomposition on compound clauses.
  2. Integrate decomposition with fleet routing dispatch.
  3. Verify with `cargo test`.

---

### 4. Offline RAGAS-Style Triad Grading in `kb eval`
- **Objective**: Offline mathematical grading of generation quality: Faithfulness (grounding in served passages), Answer Relevance, and Context Relevancy, including strict abstention scoring.
- **Files**:
  - `tools/kb/src/eval.rs`
- **TDD Steps**:
  1. Write unit tests evaluating synthetic ground-truth and abstention test cases.
  2. Implement offline RAGAS triad metrics.
  3. Verify with `cargo test`.

---

### 5. Local Reader Offline Optimization and 4-Bit AWQ/Gemma-2 Distillation Profile
- **Objective**: Define and document local low-memory runtime profiles (<2GB RAM) for standalone UI execution (Gemma-2 2B/9B, Qwen 2.5 3B with 4-bit AWQ) while keeping harness execution zero-cost.
- **Files**:
  - `decisions/0043-local-reader-awq-distillation.md`
  - `fleet.txt`
- **TDD Steps**:
  1. Create ADR 0043 documenting quantization, memory budgets, and student distillation.
  2. Update fleet configuration documentation.
  3. Verify `kb check` and repo integrity.

