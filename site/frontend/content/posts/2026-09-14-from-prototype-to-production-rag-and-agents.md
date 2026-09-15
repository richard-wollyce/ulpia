---
title: From Prototype to Production: 5 Architectural Upgrades in Ulpia's RAG & Multi-Agent Engine
date: 2026-09-14
description: Five architectural upgrades turning naive RAG into a production engine: small-to-big section windowing, constitutional critique ledgers, cross-domain query decomposition, offline triad grading, and 4-bit AWQ student distillation under 2 GB RAM.
lang: en
---

When people build their first Retrieval-Augmented Generation (RAG) system or multi-agent pipeline, the demo looks effortless. You take a collection of markdown files, slice them into arbitrary 256-token chunks, compute cosine similarity against an embedding endpoint, and feed the top three chunks into a large language model. In a prototype, it feels like magic.

In production, that naive architecture breaks down immediately.

Over the past few months of developing Ulpia - our local-first AI memory and multi-agent coordination engine written in Rust - we ran directly into the hard operational limits of naive retrieval and unconstrained agent loops. Chunks split across crucial logical clauses. Multi-agent review panels degenerated into subjective bikeshedding. Compound questions broke single-domain routers. Continuous evaluation incurred massive cloud API bills. And running local models consumed all available system RAM, starving developer tools.

To solve these problems, we engineered five architectural upgrades into Ulpia's core engine (`tools/kb`). Here is what broke in production, what replaced it, the mechanics of how it works in code, and what it cost.

## The Prototype Trap: Why Naive Retrieval Fails

Naive vector retrieval fails in production because of two opposing failure modes:

1. **The Isolated Snippet Problem**: When a document is chopped into arbitrary 256-token or 512-token chunks, semantic boundaries are ignored. A critical rule like *"if the transaction flag is false, do not write to disk"* gets split right after the comma. Chunk A contains the condition; Chunk B contains the action. When a query matches terms in Chunk B, the retriever serves Chunk B in isolation. The model never sees the negative constraint, loses the antecedent, and generates an answer that directly contradicts the source text.
2. **The Full Document Bloat Problem**: The instinctual fix for snippet fragmentation is to abandon chunking and feed entire files into large context windows. But full-document stuffing introduces severe regressions: prefill latency explodes, attention degrades across long contexts (the "lost in the middle" phenomenon), KV cache allocation balloons, and if you are using cloud endpoints, token costs multiply by orders of magnitude. On a local developer machine, processing a 50 KB context for a single question can lock up CPU cores for seconds.

Ulpia does not rely on a model to do retrieval. Retrieval is handled deterministically by SQLite FTS5 with custom BM25 ranking, scoring titles, file names, headings, and keyword density in microseconds. But bridging the gap between fast retrieval and coherent generation required rethinking how context is assembled, how agents critique each other, how queries are routed, how answers are evaluated, and how models are quantized.

## Upgrade 1: Small-to-Big Sentence and Section Windowing in `kb answer`

The first upgrade decouples search target granularity from context assembly.

To keep keyword matching sensitive and accurate, the search index needs small, focused targets. But to generate an accurate answer, the reading model needs complete paragraphs with their governing headers.

In `tools/kb/src/answer.rs`, we implemented Small-to-Big section windowing via `assemble_passages`. When FTS5 retrieves matching passages, `assemble_passages` does not merely concatenate raw chunks. Instead, it groups chunks by their source file (`captured_from`) and hierarchical heading path (`heading_path`), merges contiguous chunks, and bounds the context to a strict ceiling:

```rust
pub const MAX_SECTION_WINDOW: usize = 1800;

pub fn assemble_passages(
    passages: &[crate::retrieve::Passage],
    max_passages: usize,
) -> Vec<crate::retrieve::Passage> {
    let mut out: Vec<crate::retrieve::Passage> = Vec::new();

    for p in passages.iter().take(max_passages) {
        if let Some(existing) = out.iter_mut().find(|e| {
            e.heading_path == p.heading_path && e.captured_from == p.captured_from
        }) {
            existing.text = merge_chunk_text(&existing.text, &p.text);
        } else {
            out.push(p.clone());
        }
    }

    for p in &mut out {
        if p.text.len() > MAX_SECTION_WINDOW {
            p.text = cap_window_cleanly(&p.text, MAX_SECTION_WINDOW);
        }
    }

    out
}
```

The merging function `merge_chunk_text` inspects the overlap between chunks:

```rust
fn merge_chunk_text(first: &str, second: &str) -> String {
    let first = first.trim();
    let second = second.trim();

    if first.contains(second) { return first.to_string(); }
    if second.contains(first) { return second.to_string(); }

    let min_len = 10;
    let max_overlap = first.len().min(second.len());
    let mut overlap_size = 0;

    for len in (min_len..=max_overlap).rev() {
        if first.ends_with(&second[..len]) {
            overlap_size = len;
            break;
        }
    }

    if overlap_size > 0 {
        format!("{}\n{}", first, &second[overlap_size..].trim_start())
    } else {
        format!("{}\n\n{}", first, second)
    }
}
```

If the combined section exceeds `MAX_SECTION_WINDOW` (1,800 characters, or approximately 450 tokens), `cap_window_cleanly` truncates cleanly at sentence boundaries (`. `, `\n`, `!`, `?`) rather than cutting mid-token.

The result: the reader model receives unbroken paragraphs with intact logical conditions and headers. Prefill latency stays deterministic and flat, avoiding both out-of-context hallucinations and multi-megabyte prompt bloat.

## Upgrade 2: Constitutional Self-Critique Protocols in `kb panel`

Ulpia coordinates specialised multi-agent fleets. When an agent proposes new knowledge or updates an architectural decision, it must pass review by a panel of peer agents (`kb panel`).

Early implementations suffered from a common multi-agent trap: subjective bikeshedding. Unconstrained models acting as reviewers would raise vague objections:
- *"I don't like the tone of this explanation."*
- *"Feels a bit wordy, simplify it."*
- *"This design feels weird."*

If any objection could block knowledge promotion, panels stalled indefinitely over stylistic preferences.

To eliminate bikeshedding while preserving rigorous technical oversight, we implemented Constitutional Self-Critique Protocols in `tools/kb/src/panel.rs`. Objections are classified into two strictly typed categories:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectionCategory {
    /// Grounded in explicit mechanisms, invariants, rules, or failure modes.
    /// Eligible to be marked blocking.
    Constitutional,
    /// Constructive improvement or suggestion without invariant violation.
    /// Non-blocking.
    Advisory,
}
```

The validator `validate_objection_constitutional` inspects each reviewer's objection against two criteria:
1. **Subjective Filter**: Vague complaints without a stated mechanism (e.g. phrases matching `"feels wrong"`, `"i don't like the tone"`, `"not a fan"`) are rejected outright unless they explicitly identify a causal violation (`"because..."` or `"violates..."`).
2. **Constitutional Grounding**: To be classified as `Constitutional`, an objection must cite an explicit mechanism, architectural rule, or failure mode. The validator checks for concrete invariant markers (`invariant`, `violates`, `failure mode`, `race condition`, `deadlock`, `memory leak`, `use-after-free`, `overflow`, `rule`, `adr-`, `w03`, `citation`, `latency`, `security`, `contract`, `regression`, `panic`) or clauses matching the agent's explicit constitution file.

```rust
let category = validate_objection_constitutional("", text)
    .map_err(Error::UnconstitutionalBlocking)?;

if category != ObjectionCategory::Constitutional {
    return Err(Error::UnconstitutionalBlocking(format!(
        "objection '{text}' is advisory; blocking objections must be constitutional"
    )));
}
```

If an objection is helpful but does not cite an invariant violation, it is recorded in the panel ledger as `Advisory`. The authoring agent can inspect it as non-blocking guidance, but it cannot prevent knowledge promotion. Only `Constitutional` objections have the authority to block.

## Upgrade 3: Sub-Question Query Decomposition in Vesta Fleet Routing

In production, human and agent prompts are rarely atomic. Users ask compound questions that cross domain boundaries:
- *"How is data routed between agents and what are our memory retention policies?"*
- *"Como funciona o pipeline FTS5 e qual e o limite de memoria do leitor local?"*

Under a standard single-agent routing model, compound queries produce two failure states:
- **Domain Dominance**: The router picks the agent with the highest single keyword score, completely ignoring the second half of the question.
- **Confidence Floor Collapse**: Because the inquiry is divided across two separate topics, the keyword score for each individual agent falls below the confidence floor, causing the system to refuse with a false negative guess.

In `tools/kb/src/boot.rs`, we added sub-question query decomposition to the Vesta fleet boot path. Before routing, `decompose_query` analyzes the input:

```rust
pub fn decompose_query(prompt: &str) -> Vec<String> {
    let clean = prompt.trim();
    if clean.is_empty() { return Vec::new(); }

    let mut clauses: Vec<String> = Vec::new();
    for line in clean.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        for chunk in line.split(&['?', ';', '!'][..]) {
            let chunk = chunk.trim();
            if !chunk.is_empty() { clauses.push(chunk.to_string()); }
        }
    }

    let conjunctions = [
        " e como ", " e qual ", " e quanto ", " e onde ", " e por que ",
        " alem disso ", " além disso ",
        " and how ", " and what ", " and why ", " and where ", " and when ",
        " furthermore ", " moreover ",
    ];

    // Recursively splits clauses on cross-domain conjunctions...
```

Each atomic sub-question is routed independently against the fleet. If distinct agents achieve a confident `Hit` on different sub-questions, Vesta does not pick one at random. Instead, it assigns primary ownership to the first responding agent and automatically convenes the other domain owners into a collaborative multi-agent panel:

```rust
if sub_routed.len() > 1 {
    let primary = chosen.clone().unwrap_or_else(|| sub_routed[0].0.clone());
    for (sub_agent, sub_q) in &sub_routed {
        if !sub_agent.eq_ignore_ascii_case(&primary)
            && !panel.iter().any(|r| r.agent.eq_ignore_ascii_case(sub_agent))
        {
            panel.push(crate::classify::Reviewer {
                agent: sub_agent.clone(),
                why: format!("cross-domain sub-question: {sub_q}"),
            });
        }
    }
}
```

Vesta records the decomposition directly in the session trace:
```
VESTA: decomposed compound inquiry across 2 domains:
  - [arch]: "How is data routed between agents"
  - [policy]: "what are our memory retention policies"
```

Compound questions are answered completely, with each domain specialist contributing grounded context from its own knowledge base.

## Upgrade 4: Offline RAGAS-Style Triad Grading in Pure Rust (`kb eval --triad`)

Evaluating RAG performance in production is notorious for high latency and runaway costs. Standard evaluation frameworks (such as RAGAS or TruLens) rely on LLM-as-a-judge: for every question evaluated, three separate prompts are dispatched to GPT-4 to score faithfulness, relevance, and context quality.

Running an evaluation suite of 150 benchmark questions against an external model requires 450 API calls, costs dollars per run, takes over a minute, and suffers from non-deterministic grading variance. That makes it impossible to run evaluation inside a tight CI test loop.

In `tools/kb/src/eval.rs`, we implemented an offline, deterministic RAGAS-style triad grading engine in pure Rust:

```rust
pub struct TriadScore {
    /// Ratio of answer content claims supported by served context (0.0 to 1.0).
    pub faithfulness: f64,
    /// Ratio of question terms/intents addressed by the answer (0.0 to 1.0).
    pub answer_relevance: f64,
    /// Ratio of context sentences bearing directly on the question (0.0 to 1.0).
    pub context_relevancy: f64,
}

impl TriadScore {
    pub fn composite(&self) -> f64 {
        ((self.faithfulness + self.answer_relevance + self.context_relevancy) / 3.0 * 100.0).round() / 100.0
    }
}
```

The function `evaluate_triad` tokenizes text into lowercase content terms, strips language-specific stopwords, and calculates the three triad dimensions mathematically:

1. **Faithfulness**: Computes the proportion of content tokens in the generated answer that are grounded within the served context passages. If an answer introduces unsupported factual claims, faithfulness drops.
2. **Answer Relevance**: Measures the proportion of key question tokens addressed in the answer, penalizing evasion and topic drift.
3. **Context Relevancy**: Scans each sentence in the retrieved context to verify that it carries direct semantic overlap with the inquiry, detecting context window pollution.
4. **Honest Abstention Handling**: Crucially, if the repository does not contain the answer and the model honestly abstains (*"the library does not hold this"* or *"não possui"*), it is awarded a perfect 1.0 score for faithfulness and relevance rather than being penalized as a zero-overlap failure.

Running `kb eval --triad` executes in under 5 milliseconds on a single CPU core. Zero external API calls, zero token billing, and 100% deterministic regression testing on every commit.

## Upgrade 5: The 4-Bit AWQ Distillation Profile under 2 GB RAM (ADR-0043)

Our hardware reality governs our deployment architecture. Ulpia is built to run locally on standard developer laptops (tested on an Intel Core i5-1135G7 Tiger Lake CPU with single-channel DDR4 memory yielding ~16 GB/s bandwidth and integrated Iris Xe iGPU).

On this hardware, physical law dictates that **generation is memory bandwidth bound and prefill is compute bound**. When weights must be fetched from single-channel system RAM for every generated token, generation throughput is strictly capped by byte throughput.

To make standalone local execution viable without competing with developer tools, ADR-0043 establishes two strict invariants:

### 1. The Two-Track Separation Invariant

Ulpia operates in two distinct operational environments:
- **Track 1: External Agent Harness** (e.g. Antigravity, Claude Code, Cursor). The developer is already paired with an active frontier model in their editor. All synthesis, routing, and reviews are performed directly by this resident model. Zero external cloud API calls are made, and zero background local LLM daemons run. All four CPU cores and memory remain completely free.
- **Track 2: Standalone Ulpia Tray Mode**. When launched as a standalone UI or tray application without an external harness, an offline quantized student model handles document Q&A (`kb answer`) and note promotion review (`kb panel`).

### 2. The Strict 2 GB Resident Memory Envelope

For standalone Ulpia Tray mode, the local reader is allocated a hard resident memory ceiling of 2,000 MB:
- **Model weights**: 1,200 MB to 1,600 MB (4-bit quantization).
- **KV cache and compute scratchpad**: 200 MB to 350 MB (capped at 2,048 context tokens).
- **FTS5 index and process overhead**: 50 MB to 100 MB.

A 7B or 8B model requires 4.5 GB to 5.0 GB even at 4-bit, which violates this envelope and slows generation down to 5 to 7 tokens per second on single-channel RAM.

### Why 4-Bit AWQ Beats RTN and GPTQ

Standard 4-bit quantization methods exhibit severe flaws on sub-4B models:
- **Round-To-Nearest (RTN)**: Truncates outlier weights uniformly. At 4-bit, this causes severe syntax degradation, repetitive loops, and hallucinated citations.
- **GPTQ**: Uses second-order Hessian error compensation. On small student models, GPTQ suffers from layer-by-layer error accumulation, degrading structured reasoning.
- **Activation-aware Weight Quantization (AWQ)**: Recognizes that not all weights are equally important. By protecting the top 0.5% to 1% of salient weight channels based on actual activation magnitudes, AWQ eliminates almost all perplexity loss while packing the remaining 99% of weights into 4-bit integers.

On Intel Tiger Lake and Iris Xe, 4-bit AWQ reduces memory bus traffic by ~70%, increasing generation throughput from ~5 tokens/sec up to 15-22 tokens/sec.

### Student Distillation Models

We selected two compact student models:
1. **Google Gemma-2 2B (Instruct / Distilled)**: Distilled from the Gemma-2 27B teacher. Its interleaved local sliding-window attention (4,096 tokens) bounds the KV cache footprint to a deterministic ceiling. Quantized at 4-bit, it fits into ~1.5 GB of RAM. It serves as our primary standalone reader for Q&A and panel reviews.
2. **Qwen 2.5 3B (Instruct)**: Dense multi-query attention with strong multilingual and structured JSON capabilities. Quantized at 4-bit, it fits into ~1.8 GB of RAM and serves as our high-precision routing and classification fallback.

## Production Is a Discipline of Constraints

The difference between a prototype RAG system and a production engine is not the size of the vector database or the parameter count of the cloud model. It is the discipline of system boundaries:
- Windowing context cleanly at paragraph boundaries instead of dumping fragmented snippets or whole files.
- Restricting agent panel objections to mechanism-grounded constitutional invariants instead of subjective style nitpicks.
- Decomposing multi-intent questions into atomic sub-queries before routing to specialised domain agents.
- Grading retrieval and generation quality deterministically in pure Rust in milliseconds without external API costs.
- Enforcing strict 2 GB memory envelopes and 4-bit AWQ distillation profiles so local inference runs fast on standard developer laptops.

All five upgrades are committed, verified across more than 470 unit tests, and documented in our architectural decision records. Ulpia is open source under the Apache 2.0 license at [github.com/richard-wollyce/ulpia](https://github.com/richard-wollyce/ulpia).
