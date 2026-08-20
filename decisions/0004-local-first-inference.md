# ADR-0004: local first inference, and the role split it forces

**Search for:** `local first`, `local LLM`, `modelo local`, `rodar sem internet`, `sem internet`, `offline`, `llama.cpp`, `Ollama`, `Qwen`, `Qwen3.5-4B`, `whisper.cpp`, `Piper`, `speech to text`, `text to speech`, `TTS`, `STT`, `prefill`, `prefill lento`, `tokens per second`, `tokens por segundo`, `memory bandwidth`, `largura de banda`, `quantization`, `quantizacao`, `Q4`, `hybrid attention`, `linear attention`, `dual channel`, `single channel`, `pente de memoria`, `upgrade de RAM`, `memoria RAM`, `quanta RAM`, `Dell Latitude 3420`, `i5-1135G7`, `Tiger Lake`, `Iris Xe`, `iGPU`, `GPU integrada`, `sem placa de video`, `sem CUDA`, `espaco em disco`, `disk space`, `4B`, `7B`, `8B`, `14B`, `escolher modelo`, `modelo pequeno`, `frontier model`, `modelo de fronteira`, `role split`, `roteamento local`, `extraction`, `drafting`, `chunk size`, `prompt cache`, `cache de prompt`, `benchmark`, `medicao de desempenho`, `lentidao`, `muito lento`, `demora para responder`, `notebook fraco`, `roda no meu notebook`, `generation speed`, `velocidade de geracao`

**Exists to:** What a local model actually costs on this laptop, and which jobs go local versus frontier.

- **Date:** 2026-08-13
- **Status:** proposed. The numbers in it are calculated, not measured, and the first action is to
  replace them with measurements.
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** reversible. Nothing here is built yet, which is the point of writing it first.

## Context

Richard stated the goal on 2026-08-13: local first, so that with no internet we still have the best of
both worlds, our own context and documents in cheap local files, plus a **local LLM** running on top of
the agent producing inputs and outputs that are efficient and intelligent enough. He is explicit that a
local Qwen or Kimi does not compare to a frontier model, and that the bar is the **minimum viable good
answer**, not parity.

That bar is the right one. The hardware, measured today, decides what it costs to reach.

## The machine

| | |
|---|---|
| Model | Dell Latitude 3420 |
| CPU | Intel i5-1135G7, Tiger Lake, 4 cores and 8 threads |
| RAM | 15.7 GB, **shared with the GPU** |
| GPU | Intel Iris Xe, integrated, no dedicated memory, no CUDA |
| Disk | 25.4 GB free of 237 GB |

No dedicated GPU means inference runs on the CPU, and that has a specific consequence most advice
about local models never mentions.

## The mechanism that decides everything here

Local inference has two phases with **different bottlenecks**, and they behave nothing alike.

**Generation is memory bandwidth bound.** Every token generated requires reading the active model
weights out of RAM. So tokens per second is roughly memory bandwidth divided by model size. Tiger Lake
dual channel gives something like 50 to 68 GB/s theoretical, call it 35 to 45 GB/s real.

| Model at Q4 | Size | Estimated generation |
|---|---|---|
| 1.5B | ~1.0 GB | 25 to 35 t/s |
| 3 to 4B | ~2.5 GB | 9 to 15 t/s |
| 7 to 8B | ~4.7 GB | 5 to 8 t/s |
| 14B | ~9 GB | 2 to 4 t/s, and it will not fit alongside a working system |

**Prefill is compute bound**, and this is the part that hurts. Before the first output token, the model
has to process the entire prompt. On four cores, expect roughly 20 to 60 tokens per second for a 7B.

**Now apply that to our own architecture, which is the finding:**

| Base | Mandatory reading path | Tokens, roughly | Prefill at 40 t/s |
|---|---|---|---|
| Zed | 36 KB | ~9,000 | about 4 minutes |
| Steve | 85 KB | ~21,000 | about 9 minutes |

**Minutes of silence before the first word, on every question.** Our design reads a fixed set of files
on every query, and that design was written for a model where prefill is nearly free. On this machine
it is the dominant cost, and no choice of model fixes it, because a smaller model prefills faster but
still reads the same 21,000 tokens.

All of the above is **tier C: calculated from bandwidth and core count, not measured.** It is good
enough to decide the shape and not good enough to quote. The first action below turns it into tier A.

## Options

### A. Same architecture, smaller model

Swap the frontier model for a local one and change nothing else. Costs minutes per question and
produces worse answers. This is what "run a local LLM" usually means in practice and it is why people
try it once and go back.

### B. Same model everywhere, but retrieve instead of read

Stop reading the whole map on every question. Pull the two or three files that matter, from a derived
index, and prefill 2,000 tokens instead of 21,000. Prefill drops from minutes to seconds.

Cost: the retrieval has to be good, because what it fails to retrieve simply does not exist as far as
the answer is concerned. Today the map is read whole precisely because that is the crude version of
retrieval that cannot miss.

### C. Split the roles by what each model is actually good at

The local model does the high frequency, low context, low judgement work. The frontier model does deep
reasoning, when there is a connection.

| Job | Where | Why |
|---|---|---|
| Speech to text | Local, whisper.cpp class | Real time on CPU, no judgement needed |
| Text to speech | Local, Piper class | Tiny and fast on CPU |
| Routing: which agent, which files | Local, 1.5 to 4B | A classification, not an essay. Small context, and it is the query that runs on every single input |
| Extraction: pull fields, tag, summarise one file | Local, 3 to 8B | Bounded context, checkable output |
| Drafting a first pass | Local, 7 to 8B | Cheap, and a human or a stronger model reviews it |
| Architecture, design, judgement, the bar | Frontier when online | Judgement is the thing that does not degrade gracefully |
| Everything, offline | Local, degraded and honest about it | Working worse beats not working |

## Decision

**B and C together, in that order.** They are not alternatives, they are the same fix seen from two
sides: make the context small, then give the small context to the model that fits the job.

**Consequence for the knowledge base, and this matters now because we are about to start filling it:**

- **A note has to be answerable on its own.** If understanding one note requires reading four others,
  retrieval has to pull five files and prefill goes back up. Self contained beats cross referenced when
  the reader has a context budget.
- **The `Search for:` line stops being documentation and becomes the retrieval key.** It is what the
  router matches against. A missing keyword line is now a file that cannot be found by the local model
  at all, which is exactly what `kb` check W02 already enforces.
- **A stable prefix is worth real time.** llama.cpp and friends can cache the KV state of a prompt
  prefix. If the boot path is stable and small, it gets processed once instead of once per question.
  That is a strong argument for keeping the boot path both short and unchanging, and against stuffing
  more into `index.md` over time.
- **Chunk size should be picked from the measurement**, not from a blog post's 512 tokens.

## The first action, before anything is built

Measure instead of arguing. On this machine, with our own files as the prompt:

1. Install llama.cpp from a prebuilt release, about 50 MB. Chosen over Ollama for one reason that
   matters here: explicit control over prompt caching and slots, which is the bottleneck.
2. Pull one small model and one mid model, Q4, roughly 2.5 GB and 4.7 GB.
3. Measure four numbers: prefill tokens per second, generation tokens per second, memory at rest, and
   what a warm prompt cache actually saves on the second question.
4. Record them in `knowledge/reference/`, with the hardware and the date, as tier A.

Those four numbers decide the model, the chunk size, and whether the router runs local or not. Every
other decision here is downstream of them, and none of it should be argued further until they exist.

## Measured, 2026-08-13, same day

Done. Full results in `local-inference-latitude-3420`, tier A. **Two of the estimates above were
wrong and one of them was wrong for an interesting reason.**

| Claim above | Estimated | Measured |
|---|---|---|
| Prefill on CPU | 20 to 60 t/s | **19.55 t/s**, at the very bottom |
| Prefill on the iGPU | not considered | **72.70 t/s**, 3.7x the CPU |
| Generation, 4B at Q4 | 9 to 15 t/s | **5.88 t/s** |
| KV cache per token | 144 KB | **32 KB by arithmetic, ~48 KB observed** |

**Why generation was overestimated:** the bandwidth assumption of 35 to 45 GB/s assumed dual channel
memory. The machine has one stick in a two slot board, so it runs single channel and achieves about
16 GB/s. A second matching stick is the cheapest available upgrade and should nearly double generation.

**Why the KV cost was overestimated, and this one generalises:** `Qwen3.5-4B` is a hybrid attention
model. Only one layer in four keeps a KV cache that grows with context; the rest use linear attention
with a fixed size state. **The cost of a resident context is set by the attention architecture, not by
the parameter count.**

**What the measurement did not overturn:** the shape of the decision. Prefill still dominates, a
21,000 token boot path still costs minutes, and reading the whole base per query is still not viable.
The iGPU makes it 3.7 times less bad, not viable.

## Consequences

- Disk is the binding constraint, not RAM. 25.4 GB free supports two or three quantized models and
  their KV caches, not a lab. Model choice is a commitment, not an experiment we can rerun ten times.
- We take on a retrieval layer, which is the natural growth of `tools/kb` and was already predicted by
  [[0003-knowledge-storage]].
- The agents keep working in text and in files, so nothing about the base changes shape. The local
  model is a consumer of the base, not an owner of it.

## Revisit trigger

- The measurements coming back materially better or worse than the estimates above.
- A dedicated GPU entering the picture, which changes every number here by an order of magnitude.
- Retrieval quality proving to be the limit rather than prefill, which would move the work from
  performance to relevance.
