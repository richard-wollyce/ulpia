---
provenance: agent
stage: derived
---

# ADR-0043: local reader offline optimization and 4-bit AWQ distillation profile

**Search for:** `local reader`, `leitor local`, `offline`, `sem internet`, `AWQ`, `activation aware weight quantization`, `quantizacao 4 bit`, `4-bit`, `Gemma-2`, `Gemma-2-2B`, `Qwen 2.5`, `Qwen 2.5-3B`, `destilacao`, `distillation`, `estudante`, `student model`, `limite de memoria`, `memory budget`, `2GB RAM`, `Ulpia Tray`, `harness invariant`, `zero API cost`, `sem custo de API`, `antigravity`, `llama.cpp`, `sliding window attention`, `hybrid attention`, `prefill`, `largura de banda`, `ADR-0004`

**Exists to:** record the model profile, quantization format, and memory budget for running Ulpia offline in standalone UI mode (Ulpia Tray), while binding the zero-API invariant when running inside an external agent harness.

- **Date:** 2026-09-14
- **Status:** accepted
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible in configuration. Changing model weights or quant format is a file swap, not an architectural migration.

## Context

ADR-0004 measured the physical limits of local inference on this hardware (Dell Latitude 3420, Intel i5-1135G7 Tiger Lake, 4 cores / 8 threads, single-channel DDR4 memory yielding ~16 GB/s bandwidth, integrated Iris Xe iGPU). The finding that governs everything is that **generation is memory bandwidth bound and prefill is compute bound**. When the boot path or retrieved context is large, prefill costs seconds or minutes of silence; when weights must be fetched from single-channel system RAM for every generated token, generation throughput is strictly capped by byte throughput.

Towards AI's *Building LLMs for Production* (2nd Edition, Chapters XI and XII) documents the state of the art in serving compact models under tight resource constraints:
1. Teacher-student distillation preserves frontier task performance in sub-4B student architectures.
2. Activation-aware Weight Quantization (AWQ) outperforms naive Round-To-Nearest (RTN) and GPTQ at low bitwidths by protecting the 1% most salient weight channels based on activation magnitudes rather than static weight sizes.
3. Attention patterns with sliding window or linear hybrid states cap the KV cache size, preventing memory ballooning during multi-turn exchanges.

Ulpia operates in two distinct operational environments, and conflating them was causing wasted API calls and runtime confusion:
1. **The External Agent Harness environment** (e.g. Antigravity, Claude Code, Cursor): The developer is already paired with an advanced frontier model within the IDE harness. Routing, promotion, review, and answer synthesis are executed directly by this harness model. No external cloud APIs should ever be called, and no background local LLM process should compete for CPU cores or memory.
2. **The Standalone UI environment** (Ulpia Tray, standalone desktop app, or web viewer): The system runs without an external harness. When offline or disconnected from cloud providers, a local quantized student model must handle classification, query routing, and passage answering.

## The Memory Envelope: Under 2 GB RAM

For standalone Ulpia Tray mode, inference cannot monopolize the machine. The laptop has 15.7 GB total RAM shared dynamically with the Intel Iris Xe GPU. The operating system, developer tools, and Ulpia's SQLite FTS5 database already consume memory.

The standalone local reader is allocated a **strict budget ceiling of 2,000 MB (2 GB) resident memory**, subdivided as:
- Model weights: 1,200 MB to 1,600 MB (at 4-bit quantization).
- KV cache and compute scratchpad: 200 MB to 350 MB (capped at 2,048 context tokens).
- FTS5 index and process overhead: 50 MB to 100 MB.

A 7B or 8B parameter model at Q4 requires 4.5 GB to 5.0 GB just for weights, violating this envelope and slowing generation down to 5 to 7 tokens per second on single-channel RAM. Models above 3.5B are therefore excluded from standalone local reader mode.

## Why 4-Bit AWQ Beats RTN and GPTQ

Traditional integer quantization formats exhibit severe trade-offs at 4-bit:
- **Round-To-Nearest (RTN)**: Quantizes all weights uniformly. At 4-bit, critical outlier weights that govern attention routing are truncated, resulting in syntax degradation, repetitive loops, and hallucinated markdown citations.
- **GPTQ**: Uses second-order Taylor expansion (Hessian matrix) to compensate for quantization error. While effective on 13B+ models, on sub-4B models GPTQ often suffers from error accumulation across layers, leading to degraded perplexity on structured tasks.
- **Activation-aware Weight Quantization (AWQ)**: Identifies that not all weights are equally important: protecting the top 0.5% to 1% of channels that correspond to large activation magnitudes eliminates almost all perplexity degradation. Only salient channels are kept at higher precision or scaled appropriately, while the remaining 99% are packed into 4-bit integers.

On Intel Tiger Lake and Iris Xe (via Vulkan or OpenCL in llama.cpp/candle), 4-bit AWQ cuts memory bus traffic by ~70% compared to 16-bit float, pushing generation from ~5 t/s up to 15-22 t/s on small contexts.

## Student Distillation Model Selection

Two student models meet the accuracy, memory, and architectural criteria:

### 1. Primary Profile: Google Gemma-2 2B (Instruct / Distilled)
- **Parameter count**: 2.6 billion parameters.
- **Size at 4-bit (AWQ / Q4_K_M)**: ~1.5 GB.
- **Architecture**: Distilled from Gemma-2 27B teacher; uses interleaved local sliding-window attention (4,096 tokens) and global attention. Sliding window attention bounds the KV cache footprint to a deterministic ceiling.
- **Role**: Primary standalone local reader for document Q&A (`kb answer`) and note promotion review (`kb panel`).

### 2. Alternate / Secondary Profile: Qwen 2.5 3B (Instruct)
- **Parameter count**: 3.09 billion parameters.
- **Size at 4-bit (AWQ / Q4_K_M)**: ~1.8 GB.
- **Architecture**: Dense multi-query attention with strong multilingual and structured JSON capabilities.
- **Role**: High-precision routing and classification fallback (`kb boot` and `classify`) when Portuguese and technical jargon density is high.

## The Two-Track Separation Invariant

To ensure no ambiguity between environments, the system establishes an invariant:

1. **Harness Track (Zero API / Zero Local Background Daemon)**:
   - When running inside Antigravity or an agent harness, `fleet.txt` keeps `classifier =`, `promoter =`, `reviewer =`, and `answerer =` commented out or pointed to internal harness commands.
   - The harness model handles all prompt evaluations and synthesis.
   - Zero API tokens are billed to external cloud endpoints (Claude, OpenAI, Gemini API).
   - Zero local LLM reader daemons run in the background, leaving all 4 CPU cores free for compilation and indexing.

2. **Standalone UI Track (Offline Reader)**:
   - When launched via Ulpia Tray without an external harness, the application boots the local reader runner pointing to the 4-bit AWQ student weights (`gemma-2-2b-it-awq` or `qwen-2.5-3b-awq`).
   - Pure local inference through llama.cpp / candle runner on CPU/iGPU.
   - Full privacy: no telemetry, no network calls, fully operable in air-gapped environments.

## Consequences

- Standalone offline users receive sub-second retrieval from `tools/kb` FTS5 index combined with 15-20 t/s generation within a 1.8 GB RAM footprint.
- In-harness developers incur zero extra API fees and experience instantaneous responses from the resident assistant.
- Model profiles and parameters are committed to `fleet.txt` as documentation and configuration references.

## Revisit Trigger

- Upgrading the laptop to dual-channel RAM (adding a second DDR4 stick), which doubles memory bandwidth and would make a 7B/8B model viable at 12-15 t/s.
- Arrival of sub-2B reasoning models (e.g. distilled deepseek/qwen small reasoning models) with native chain-of-thought capabilities fitting under 1.2 GB RAM.
- Release of direct NPU hardware support on the host machine.
