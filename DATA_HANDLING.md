# Data Handling, Privacy & Air-Gap Guarantees

Ulpia is designed from the ground up as a **local-first, sovereign AI memory engine**. This document defines what data Ulpia touches, where it is stored, how privacy boundaries are enforced, and the fundamental security difference between running local models versus cloud providers.

---

## 1. The Two Operational Modes: Local Sovereignty vs Cloud Providers

The security and privacy of an AI memory layer depends entirely on the model reading it. Ulpia distinguishes between two operational modes:

```
MODE A: Local / Sovereign (Air-Gapped)
[User Prompt] ──▶ [Ulpia FTS5 Local Search] ──▶ [Local Student LLM] ──▶ [Local Answer]
       └────────────── 100% On-Device / Zero Network Calls ──────────────┘

MODE B: External Cloud Provider
[User Prompt] ──▶ [Ulpia FTS5 Local Search] ──▶ [Cloud API (HTTPS)] ──▶ [Model Answer]
                                                      │
                                                      ▼
                                       Third-Party Cloud Servers
                                       (Provider Retention & Terms Apply)
```

### Mode A: Local / Air-Gapped Mode (Full Sovereignty & GDPR Compliance)
When Ulpia is used offline with a local model (such as Google Gemma-2 2B or Qwen 2.5 3B via 4-bit AWQ / GGUF, or an in-harness local model):
- **100% On-Device**: Prompts, retrieved knowledge passages, and generated answers never leave the physical machine.
- **Zero Third-Party Risk**: No external server ever receives or logs your code, internal documentation, or personal notes.
- **Air-Gapped Operation**: Ulpia operates completely without an internet connection. The SQLite search index, BM25 scorers, and small-to-big section windowing require zero network requests.
- **Enterprise & GDPR Ready**: Satisfies strict European Union data residency, confidentiality, and air-gapped security audit requirements.

### Mode B: External Cloud Provider Mode (Third-Party Disclosure)
If you or your enterprise choose to connect Ulpia to cloud-hosted models (such as Anthropic Claude, OpenAI Codex, or Google Gemini/Vertex API):
- **Local Retrieval, External Synthesis**: Ulpia's indexing, scoring, and passage retrieval remain entirely local and free of token billing. However, once passages are assembled into a prompt and dispatched to an external API endpoint, **those passages travel over HTTPS to third-party infrastructure**.
- **Shared Responsibility & External Vulnerability**: Third-party servers operate outside your control and outside Ulpia's security perimeter. Data sent to cloud endpoints is governed exclusively by the third-party provider's privacy policy, enterprise data processing agreements (DPA), and data retention terms.
- **Vulnerability Warning**: If your knowledge base contains confidential intellectual property, proprietary credentials, or personal data, connecting cloud models subjects that data to the risk of external vendor exposure or logging. For sensitive environments, Mode A (Local / Air-Gapped) is mandatory.

---

## 2. What Ulpia Stores and Where It Lives

Ulpia does not use remote databases or hosted vector clusters. Everything lives inside the repository working tree:

| Data Type | Storage Location | Format | Lifecycle & Retention |
| :--- | :--- | :--- | :--- |
| **Knowledge Base** | `fleet/<agent>/knowledge/` | Plain Markdown (`.md`) | Version-controlled source of truth. |
| **Search Index** | `.kb/index.db` | SQLite 3 (in-process FTS5) | Disposable derived cache; can be rebuilt in milliseconds via `kb index`. |
| **Session Continuity** | `.kb/sessions/*.handoff.md` | Plain Markdown | Task baton passing between agent sessions. |
| **Lifecycle Events** | `.kb/sessions/*.events` | Tab-separated log | Session-scoped unmapped questions and routing events. |
| **Miss / Gap Log** | `kb-misses.txt` | Plain text log | Records unanswered questions to guide knowledge authoring. |

### The Private Layer (`private_layer` / ADR-0034)
Ulpia provides a cryptographic-grade distinction between public and private knowledge:
- Directories named `profile/`, `projects/`, and `records/` are classified as the **private layer** by default.
- Files in the private layer are **never indexed into public search tables** and are never returned to queries unless the local operator explicitly supplies the `--all` flag.
- Private files are excluded from git-tracked public releases.

---

## 3. Zero Telemetry and Zero Analytics Guarantee

Ulpia contains:
- **Zero background daemons**: No resident HTTP servers listening on ports.
- **Zero telemetry**: No analytics pings, no usage metrics, and no crash beacons.
- **Zero phone-home**: The `kb` binary never contacts any update server or license validator.
- **Open and Auditable**: The entire engine is open-source under the Apache 2.0 license.

---

## 4. Portability, Migration and Backups

A frequent question from developers and system administrators:
> *"If I migrate the `ulpia` folder from Windows to Linux or macOS, or restore a backup, will Git or the memory engine break?"*

**No. Ulpia and Git are 100% portable across machines and operating systems.**

### Why Migration Does Not Break
1. **Relative Paths**: All references in Git, SQLite tables, and fleet manifests (`fleet.txt`, `agent.txt`, `MAP.md`) use strictly relative paths (e.g. `fleet/zed/knowledge/foo.md`) with standardized forward slashes. Ulpia never hardcodes absolute filesystem paths (like `C:\Users\...` or `/home/...`).
2. **Git Root Anchoring**: Project identity is determined by Git repository roots (`git rev-parse --show-toplevel` and remote tracking URLs), never by arbitrary folder names. Renaming the parent folder or moving it between directories has zero impact on project resolution.
3. **Disposable Derived Cache**: The SQLite search index (`.kb/index.db`) is completely derived from Markdown files. If you migrate across operating systems:
   - You can simply delete `.kb/` or run `kb index --all`.
   - Ulpia will regenerate the entire index in under 100 milliseconds.
   - Even if you copy `.kb/index.db` directly, SQLite database files are binary-portable and endian-neutral across Windows, macOS, and Linux.
4. **Universal Line Endings**: Ulpia's text ingestion and parsers seamlessly strip carriage returns (`\r\n` vs `\n`), so migrating between Windows and Unix systems produces zero parsing discrepancies.

### Clean Migration Steps
To move your Ulpia knowledge base to a new machine:
```bash
# Method 1: Via Git (Recommended)
git push origin main
# On your new machine (Linux or MacBook):
git clone <repo-url>
kb index --all

# Method 2: Direct Folder Copy (Archive or Rsync)
rsync -avz ./ulpia/ user@new-machine:~/ulpia/
# On new machine:
kb index --all
```
Your knowledge, agent rosters, and history will immediately resume functioning without manual reconfiguration.
