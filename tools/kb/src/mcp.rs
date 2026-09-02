//! An MCP server over stdio, so any client that speaks the protocol can use the
//! base: Claude Code, Claude Desktop, or our own GUI driving a local model.
//!
//! The server does not know which of those is calling and does not care. That is the
//! point of putting retrieval here rather than in the GUI: with a cloud model the
//! passages travel in the prompt, with a local model nothing leaves the machine, and
//! both read the same code.
//!
//! Three rules come from the transport and each one is a bug if forgotten:
//!
//! 1. **stdout belongs to the protocol.** The spec is explicit: "The server MUST NOT
//!    write anything to its stdout that is not a valid MCP message." Every
//!    diagnostic here goes to stderr, which the spec explicitly allows and which
//!    clients are told not to read as failure.
//! 2. **One message per line, no embedded newlines.** `json::Value::to_string`
//!    guarantees it and has a test.
//! 3. **Dual-era.** Revision 2026-07-28 removed the `initialize` handshake and moved
//!    the version into per-request `_meta`; 2025-11-25 and earlier require the
//!    handshake. The spec names an implementation that speaks both "dual-era", so
//!    this answers `initialize` when asked and never requires it, and it echoes back
//!    whatever version the client named rather than asserting one.
//!
//! What it deliberately does not have yet is a write tool. `kb_remember` measures and
//! proposes; no tool here writes to the base. A write tool reached by a model is a
//! different security surface and gets built deliberately, not as an afterthought
//! while the retrieval side is still warm.
//!
//! One file is written, and saying "nothing is written" hid it: a refusal in
//! `kb_route` or `kb_retrieve` calls `Memory::recall_loss`, which appends the question
//! and the terms offered back to `kb-misses.txt` beside the fleet manifest. That is a
//! log of what could not be answered, not knowledge, and it is the record ADR-0016's
//! successor reads to find out which gaps are real. **Which questions count is decided
//! on the contract and not here**: these two tools used to decide it themselves, from
//! the length of two different lists, so one base had two definitions of one number.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::json::{self, Value};
use crate::memory::Memory;
use crate::remember;

const SERVER_NAME: &str = "kb";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// What we answer with when a legacy client does not name a version. The oldest
/// widely deployed revision is the safest default: naming a newer one at a client
/// that does not know it is how a handshake fails.
const FALLBACK_PROTOCOL: &str = "2025-06-18";

// JSON-RPC error codes, from the spec.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

struct Server {
    memory: Memory,
    top: usize,
}

pub fn serve(paths: &[&str], all: bool, top: usize) -> ExitCode {
    let owned: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let refs: Vec<&Path> = owned.iter().map(|p| p.as_path()).collect();

    // Everything the server knows about the base comes through Memory, including which
    // files are private: the declaration is read off each base by `Memory::open`, and
    // nothing here is consulted about it. Rebuilding the pipeline here is how a second
    // caller ends up expanding aliases for one scorer and not the other, which has
    // already happened once in this codebase.
    let memory = match Memory::open(&refs, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb serve: {e}");
            return ExitCode::from(1);
        }
    };

    if memory.index_was_rebuilt {
        eprintln!("kb serve: an index predated the private column (ADR-0034) and was emptied.");
        eprintln!("    Run `kb index` before relying on retrieval.");
    }

    for agent in &memory.agents {
        eprintln!(
            "kb serve: {} ({}){}",
            agent.name,
            agent.root.display(),
            if all { "  (private layer INCLUDED)" } else { "" }
        );
    }
    eprintln!(
        "kb serve: {} map entries, {} aliases, scope {:?}. Ready on stdio.",
        memory.entry_count(),
        memory.alias_count(),
        memory.scope()
    );

    Server { memory, top }.run()
}

impl Server {
    fn run(self) -> ExitCode {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("kb serve: stdin closed: {e}");
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }

            if let Some(reply) = self.handle(&line) {
                // One line, always. `to_string` cannot emit a raw newline.
                if writeln!(stdout, "{reply}").is_err() || stdout.flush().is_err() {
                    eprintln!("kb serve: stdout closed");
                    return ExitCode::from(1);
                }
            }
        }

        // Stdin closing is the documented graceful shutdown signal, and the spec asks
        // servers to honour it so the client does not have to kill the process.
        ExitCode::SUCCESS
    }

    /// Returns the line to write back, or None for a notification, which by
    /// definition gets no response.
    fn handle(&self, line: &str) -> Option<String> {
        let msg = match json::parse(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("kb serve: unparseable message: {e}");
                return Some(error_reply(Value::Null, PARSE_ERROR, &format!("parse error: {e}")));
            }
        };

        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = match msg.get("method").and_then(|m| m.as_str()) {
            Some(m) => m.to_string(),
            None => {
                return Some(error_reply(id, INVALID_REQUEST, "no method"));
            }
        };

        // A notification has no id and must never be answered, not even with an
        // error. `notifications/initialized` is the legacy handshake's second half
        // and there is nothing to do with it.
        let is_notification = msg.get("id").is_none();
        if is_notification {
            eprintln!("kb serve: notification {method}");
            return None;
        }

        let params = msg.get("params").cloned().unwrap_or_else(Value::obj);

        let result = match method.as_str() {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(Value::obj()),
            "tools/list" => Ok(self.tools_list()),
            "tools/call" => self.tools_call(&params),
            other => {
                eprintln!("kb serve: unknown method {other}");
                return Some(error_reply(id, METHOD_NOT_FOUND, &format!("unknown method: {other}")));
            }
        };

        match result {
            Ok(value) => Some(ok_reply(id, value)),
            Err(message) => Some(error_reply(id, INVALID_PARAMS, &message)),
        }
    }

    /// The legacy handshake. Modern clients never send it, which is why nothing here
    /// depends on it having happened.
    fn initialize(&self, params: &Value) -> Value {
        // Echo the client's version rather than asserting ours. Naming a revision the
        // client does not know is how a handshake fails, and we support the tools
        // surface identically across every revision that has one.
        let version = params
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or(FALLBACK_PROTOCOL)
            .to_string();

        let mut caps = Value::obj();
        caps.set("tools", Value::obj());

        let mut info = Value::obj();
        info.set("name", SERVER_NAME.into());
        info.set("version", SERVER_VERSION.into());

        let mut out = Value::obj();
        out.set("protocolVersion", version.into());
        out.set("capabilities", caps);
        out.set("serverInfo", info);
        out
    }

    fn tools_list(&self) -> Value {
        let mut out = Value::obj();
        out.set(
            "tools",
            Value::Arr(vec![
                tool(
                    "kb_route",
                    "Ask which files in the knowledge base a question should open. Returns \
                     ranked file paths with the words that matched and an EVIDENCE line \
                     carrying the score, the floor and the verdict, so a bad ranking can be \
                     diagnosed rather than guessed at. It returns no file contents, which is \
                     the difference from kb_retrieve; it is not cheaper, because the evidence \
                     costs the same second pass over the corpus. Use it to decide what to \
                     read; use kb_retrieve when you want the text. \
                     When nothing matches it offers back terms the base does know that look \
                     like the words you used, which is a spelling comparison and not a \
                     semantic one: it reaches a typo or a cognate and never reaches a \
                     translation. Ask again with those terms, or with the canonical ones you \
                     expect, before concluding the base does not cover the subject.",
                    vec![
                        ("question", "string", "The question, in any language. An alias table maps common terms.", true),
                        ("top", "integer", "How many files to return. Default 5.", false),
                    ],
                ),
                tool(
                    "kb_retrieve",
                    "Search the knowledge base and return the matching passages themselves, \
                     with their heading path and provenance. This is the tool to use when you \
                     need to quote or reason over what the base actually says. Ranking fuses a \
                     hand written keyword index with full text search; no model is involved.",
                    vec![
                        ("question", "string", "The question, in any language.", true),
                        ("top", "integer", "How many files to return passages from. Default 5.", false),
                    ],
                ),
                tool(
                    "kb_remember",
                    "Measure a claim against what the base already says, and propose ADD, \
                     UPDATE or NOOP with the evidence behind the proposal. It writes nothing \
                     and decides nothing: a containment score can say how much two texts \
                     overlap, and cannot say whether the older one is now wrong. DELETE is \
                     never proposed. Use it before adding anything, to find out whether the \
                     base already covers it.",
                    vec![("claim", "string", "The claim to measure, as one sentence.", true)],
                ),
                tool(
                    "kb_fleet",
                    "Look up the fleet's own name and roster: the fleet's name and role, and \
                     every agent in it with that agent's role and directory. This is a read \
                     of fleet.txt and each agent.txt, not a search, so nothing here is \
                     ranked and nothing here is a passage from a knowledge file. Call it \
                     when the question is about who you are, what agents exist, or which \
                     agent covers a subject, and then answer in your own words: these are \
                     facts to answer with, not an answer. Do not use kb_route or \
                     kb_retrieve for this, because the fleet's name is not written in any \
                     knowledge file and searching for it returns whichever agent happens to \
                     write about identity.",
                    vec![],
                ),
                tool(
                    "kb_list",
                    "List the files the knowledge base holds, narrowed by facet. This is \
                        NOT a search and it takes no question: nothing here is ranked, \
                        nothing carries a score and nothing carries a verdict, because there \
                        is no ranking problem in a filter. Call it when the question is \
                        about what exists rather than about what answers something: which \
                        notes are still raw, what is in one agent's tools folder, what \
                        landed in the deposit. Asking that of kb_route or kb_retrieve scores \
                        it against a floor and comes back as a guess, which is the confusion \
                        this tool exists to end. Every argument is optional, and an omitted \
                        facet means DO NOT FILTER ON                      THIS rather than \
                        any particular value. Facets combine with AND. Every file is listed, \
                        including ones no question can reach because they carry no Search \
                        for line, which is the whole deposit and every README: a filter is a \
                        question about what the library holds, not about what a search can find.",
                    vec![
                        ("base", "string", "One agent, by its directory name. Omit for every open base.", false),
                        ("folder", "string", "A directory under the base, and everything below it: knowledge, knowledge/systems, inbox.", false),
                        ("kind", "string", "The species, read from the folder: memory, skills or tools.", false),
                        ("stage", "string", "raw, captured, distilled or derived, from the file's own front matter.", false),
                        ("provenance", "string", "human, agent or external, from the file's own front matter.", false),
                        ("limit", "integer", "How many rows to return. Default 50, and the count of what was cut is always reported.", false),
                    ],
                ),
            ]),
        );
        out
    }

    fn tools_call(&self, params: &Value) -> Result<Value, String> {
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or("tools/call needs a name")?;
        let args = params.get("arguments").cloned().unwrap_or_else(Value::obj);

        let text = match name {
            "kb_fleet" => self.memory.describe().to_text(),
            "kb_route" => {
                let q = string_arg(&args, "question")?;
                let top = args.get("top").and_then(|t| t.as_usize()).unwrap_or(self.top);
                self.route(&q, top)
            }
            "kb_retrieve" => {
                let q = string_arg(&args, "question")?;
                let top = args.get("top").and_then(|t| t.as_usize()).unwrap_or(self.top);
                self.retrieve(&q, top)
            }
            "kb_remember" => {
                let claim = string_arg(&args, "claim")?;
                self.remember(&claim)
            }
            "kb_list" => self.list(&args)?,
            other => return Err(format!("unknown tool: {other}")),
        };

        Ok(tool_text(&text))
    }

    // -- the tools themselves ------------------------------------------------

    fn route(&self, question: &str, top: usize) -> String {
        let hits = self.memory.route(question, top);
        // The ranked list stays the keyword scorer's, because ADR-0018 measured fusion
        // to be the wrong rule for picking a winner. The confidence costs a second pass
        // over the corpus and is taken anyway: this tool is called once per question by
        // a person or a model, never in a loop, and a ranked list with no evidence
        // beside it is the whole defect being fixed here.
        let confidence = self.memory.ask(question, top).confidence;

        // The recall loss is decided on the contract, from the verdict, and asked
        // unconditionally: this surface used to decide it here, from the length of the
        // keyword list, while `retrieve` below decided it from the fused one. Two tools
        // over one base, counting different populations into one file.
        let loss = self.memory.recall_loss(question, &confidence);
        let looked_like: &[String] = loss.as_ref().map_or(&[], |m| &m.looked_like);
        if hits.is_empty() {
            if self.memory.is_empty() {
                return nothing_to_search(self.memory.unreachable().len());
            }
            return no_match(question, looked_like, &self.memory.shortfall(&confidence));
        }

        let mut out = format!("Files to open for: {question}\n\n");
        out.push_str(&evidence(&confidence));
        for (i, hit) in hits.iter().enumerate() {
            out.push_str(&format!(
                "{}. {}/{}  ({})\n   matched: {}\n   {}\n",
                i + 1,
                hit.entry.base,
                hit.entry.rel,
                hit.entry.title,
                hit.matched.join(", "),
                hit.entry.summary.replace('\n', " ").trim(),
            ));
        }
        out
    }

    fn retrieve(&self, question: &str, top: usize) -> String {
        // `Memory::retrieve` is `Memory::ask` minus the confidence and the agent, over
        // the same expansion and the same two scorers. Asking for the whole answer here
        // is the same work and carries the evidence back.
        let answer = self.memory.ask(question, top);
        let found = answer.found;

        // Same call, same decision, and this is the surface where the old one had the
        // hole: a question the text scorer answered and the gate refused arrived here
        // with a full `found`, was served to the model as passages, and was recorded
        // nowhere.
        let loss = self.memory.recall_loss(question, &answer.confidence);
        let looked_like: &[String] = loss.as_ref().map_or(&[], |m| &m.looked_like);
        if found.is_empty() {
            if self.memory.is_empty() {
                return nothing_to_search(self.memory.unreachable().len());
            }
            return no_match(question, looked_like, &self.memory.shortfall(&answer.confidence));
        }

        let mut out = format!("Passages for: {question}\n\n");

        out.push_str(&evidence(&answer.confidence));

        // Every file ranked by keywords and none by text means the full text index
        // does not cover these files, which usually means it is stale. Without this
        // line a caller sees five files and no passages and concludes the base is
        // thin, which is the wrong conclusion and an invisible one. Found by pointing
        // a benchmark at the wrong index file and believing the result.
        if self.memory.no_agreement(&found) {
            out.push_str(
                concat!(
                "NOTE: only one of the two scorers ranked any of these, so this is a guess ",
                "rather than an answer. Agreement between the keyword index and the full text ",
                "index is observed here rather than assumed, and there is none of it. Read it ",
                "for what it is: agreement says a file is on topic and says nothing about ",
                "whether the base covers the topic, so it is reported and never gates a ",
                "verdict. It is also worth less than the word suggests, because the text side ",
                "is merged round robin per agent, so a file that is its own agent's best match ",
                "is admitted without competing against the others. Treat these as leads, and ",
                "consider that the base may not cover the question at all.

",
            )
            );
        }

        if self.memory.looks_stale(&found) {
            out.push_str(
                "NOTE: the keyword index ranked these files but the full text index has no \
                 chunks for any of them, which usually means the index is stale or was built \
                 over different bases. Run `kb index` over the same paths this server was \
                 started with. The rankings below are still meaningful; the missing passages \
                 are not evidence that the files are empty.\n\n",
            );
        }

        for f in &found {
            out.push_str(&format!("## {}/{}", f.base, f.path));
            if !f.title.is_empty() {
                out.push_str(&format!("  ({})", f.title));
            }
            if f.layer == crate::retrieve::Layer::Short {
                out.push_str("  [short memory: recent, not distilled, not yet in the library]");
            }
            out.push_str(&format!("\n   ranked by: {}\n", f.why.join(" + ")));

            if f.passages.is_empty() {
                out.push_str("   (ranked by keywords only; open the file for its text)\n\n");
                continue;
            }
            for p in &f.passages {
                let provenance = match (&p.provenance, &p.stage) {
                    (Some(pr), Some(st)) => format!("  [{pr}/{st}]"),
                    (Some(pr), None) => format!("  [{pr}]"),
                    _ => String::new(),
                };
                out.push_str(&format!("\n### {}{}\n{}\n", p.heading_path, provenance, p.text.trim()));
            }
            out.push('\n');
        }
        out
    }

    /// The filter surface. It states no privacy rule of its own: `serve` opened the
    /// memory with the scope it was given, and `Memory::list` reads that scope back. The
    /// module header already claims that property for every other tool here, and this is
    /// the fifth one to keep it.
    ///
    /// **A bad facet is an error and not an empty list.** A JSON-RPC error naming the
    /// legal set tells a model it made a typo; zero rows tells it the base holds none of
    /// those, which is the wrong lesson and the one it will act on.
    fn list(&self, args: &Value) -> Result<String, String> {
        let filter = crate::list::Filter::parse(
            opt_str_arg(args, "base").as_deref(),
            opt_str_arg(args, "folder").as_deref(),
            opt_str_arg(args, "kind").as_deref(),
            opt_str_arg(args, "stage").as_deref(),
            opt_str_arg(args, "provenance").as_deref(),
        )
        .map_err(|e| e.to_string())?;

        let rows = self.memory.list(&filter).map_err(|e| e.to_string())?;
        if rows.is_empty() {
            return Ok("No file in the open bases carries every facet you named. That is a fact \
                about the filter and about what is on disk, not a ranking: nothing was scored and nothing was refused."
                .to_string());
        }

        let limit = args
            .get("limit")
            .and_then(|v| v.as_usize())
            .unwrap_or(crate::list::MCP_LIMIT)
            .max(1);
        let shown = rows.len().min(limit);
        let mut out = format!("{} files. Nothing here is ranked.

", rows.len());
        out.push_str(&crate::list::to_text(&rows[..shown]));
        if shown < rows.len() {
            // The count stays exact and only the list is shortened, the same rule
            // `Memory::PATHS_SHOWN` follows: a caller has to be able to tell a narrow
            // filter from a truncated one, and a bare cut destroys exactly that.
            out.push_str(&format!(
                "
{} more not shown. Narrow the facets, or raise limit.
",
                rows.len() - shown
            ));
        }
        Ok(out)
    }

    fn remember(&self, claim: &str) -> String {
        let a = self.memory.remember(claim);
        let mut out =
            format!("claim: {claim}\nproposal: {}\nreason: {}\n", a.outcome.label(), a.reason);

        if a.evidence.is_empty() {
            out.push_str("\nNothing in the base overlaps this claim.\n");
        } else {
            out.push_str("\nevidence, closest first:\n");
            for e in a.evidence.iter().take(5) {
                out.push_str(&format!(
                    "\n  {:.2} contained  {}/{}  {}\n    shared: {}\n    new: {}\n    {}\n",
                    e.containment,
                    e.base,
                    e.path,
                    e.heading_path,
                    if e.shared.is_empty() { "-".into() } else { e.shared.join(", ") },
                    if e.missing.is_empty() { "-".into() } else { e.missing.join(", ") },
                    e.excerpt.replace('\n', " ").trim(),
                ));
            }
        }

        out.push_str("\n---\n");
        out.push_str(remember::DISCLAIMER);
        out.push_str(
            "\n\nThis tool wrote nothing. Applying the proposal is a separate, deliberate act.",
        );
        out
    }
}

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

/// The answer when there is nothing to search, which is not the answer when
/// something was searched and missed.
///
/// A fresh `kb init` has a full constitution and an empty library, so this is the
/// reply a first question gets. It has to say the library is empty and what fills
/// it, because the alternative, measured on 2026-08-17, is a new user being told
/// that their phrasing did not match keyword lines that do not exist.
///
/// It also names the tool that **does** work on an empty base, since identity is a
/// lookup over `fleet.txt` and `agent.txt` and never needed the index at all.
/// `unreachable` is [`crate::memory::Memory::unreachable`]'s count, taken by the caller
/// because this function has no memory to ask. **It changes what the reply is telling
/// somebody to do.** A base can hold markdown a person already wrote and still be empty
/// here, because a file with no `Search for:` line builds no entry; telling that reader to
/// start putting markdown in the folder is telling them to redo work they have done.
fn nothing_to_search(unreachable: usize) -> String {
    let mut out = "This base has no knowledge files yet, so nothing could have matched. That is a \
     fact about the base and not about the question.\n\n\
     A base is filled by putting markdown in the agent's folder, giving each file a \
     `Search for:` line of its own near the top, naming the words a real question \
     would use, and running `kb index`. The line lives in the file it describes, not \
     in a list somewhere else, so a file without one is a file nothing can reach and \
     no map can rescue."
        .to_string();

    // **Beside the paragraph that explains the line, not after the closing note.** The
    // terminal's copy of this reply orders it this way, and two surfaces printing the same
    // three paragraphs in two orders is the drift this whole change is against.
    if unreachable > 0 {
        let (are, they, them) = match unreachable {
            1 => ("file is", "it is", "it"),
            _ => ("files are", "they are", "them"),
        };
        out.push_str(&format!(
            // **The open bases, not this base**, because `Memory::unreachable` folds
            // every agent the server opened and a server started on a fleet root holds
            // more than one. The sentence said "this base" and was false for exactly the
            // deployment shape `kb serve` is built for.
            "\n\n{unreachable} markdown {are} already in the open bases without that \
             line: {they} on disk, a person can read {them}, and no question reaches \
             {them}. `kb check` names each one."
        ));
    }

    out.push_str(
        "\n\nkb_fleet works regardless: who this fleet is and which agents exist is read \
         from fleet.txt and each agent.txt, not from the index, so identity is \
         answerable before a single note is written.",
    );
    out
}

/// `state` is the refusal's own circumstances, and it goes in **before** the suggestion
/// block on purpose: this function returns early when there is nothing to suggest, so a
/// paragraph appended at the end reaches only the callers who least need it. One paragraph
/// rather than a bulleted list because the reader is a model with a context window, and the
/// sentences are [`crate::memory::Shortfall::lines`]'s so that the terminal, this reply and
/// the boot briefing cannot come to mean three different things again.
fn no_match(question: &str, suggestions: &[String], state: &crate::memory::Shortfall) -> String {
    let mut out = format!(
        "Nothing matched \"{question}\".\n\n\
         Either the base does not cover it, or its keyword lines do not carry the words \
         a real question uses. Saying so plainly is deliberate: a router that always \
         returns something teaches you to trust a guess."
    );

    let said = state.lines();
    if !said.is_empty() {
        out.push_str(&format!("\n\n{}", said.join(" ")));
    }

    if suggestions.is_empty() {
        return out;
    }

    // The candidate space, so the caller expands against what exists rather than
    // guessing. Trigrams reach a typo or a cognate and never reach a translation,
    // so the reply says which kind of help this is and leaves the other kind to
    // whoever is reading it.
    out.push_str(&format!(
        "\n\nThe base does know these, and they look like words you used: {}.\n\n\
         That comparison is spelling, not meaning, so it finds a typo or a cognate and \
         never finds a translation. If the question was asked in one language and the \
         base was written in another, rewrite it with the terms above or with the \
         canonical ones you expect, and ask again.",
        suggestions.join(", ")
    ));
    out
}

/// An optional string argument. Absent, null, or whitespace all mean the same thing here
/// and it is not an error: an omitted facet does not filter, so there is nothing to
/// refuse. Separate from `string_arg`, which refuses exactly those cases, because a
/// required question and an optional facet want opposite answers to one shape.
fn opt_str_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn string_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("missing or empty argument: {key}"))
}

fn tool(name: &str, description: &str, args: Vec<(&str, &str, &str, bool)>) -> Value {
    let mut props = Value::obj();
    let mut required = Vec::new();
    for (arg, ty, desc, req) in args {
        let mut p = Value::obj();
        p.set("type", ty.into());
        p.set("description", desc.into());
        props.set(arg, p);
        if req {
            required.push(Value::Str(arg.to_string()));
        }
    }

    let mut schema = Value::obj();
    schema.set("type", "object".into());
    schema.set("properties", props);
    schema.set("required", Value::Arr(required));

    let mut t = Value::obj();
    t.set("name", name.into());
    t.set("description", description.into());
    // camelCase, as the spec writes it. snake_case here is silently ignored by some
    // clients, which shows up as a tool that exists and never accepts an argument.
    t.set("inputSchema", schema);
    t
}

/// A tools/call result. Content is a list of typed parts, which is also the shape
/// ADR-0009 commits the GUI's contract to, so audio and images are additive later.
fn tool_text(text: &str) -> Value {
    let mut part = Value::obj();
    part.set("type", "text".into());
    part.set("text", text.into());

    let mut out = Value::obj();
    out.set("content", Value::Arr(vec![part]));
    out.set("isError", false.into());
    out
}

fn ok_reply(id: Value, result: Value) -> String {
    let mut m = Value::obj();
    m.set("jsonrpc", "2.0".into());
    m.set("id", id);
    m.set("result", result);
    m.to_string()
}

fn error_reply(id: Value, code: i64, message: &str) -> String {
    let mut e = Value::obj();
    e.set("code", Value::Num(code as f64));
    e.set("message", message.into());

    let mut m = Value::obj();
    m.set("jsonrpc", "2.0".into());
    m.set("id", id);
    m.set("error", e);
    m.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A JSON-RPC id has to come back byte identical or the client cannot match the
    /// response to its request. An integer id re-emitted as `3.0` is a hang.
    #[test]
    fn a_reply_carries_the_id_back_unchanged() {
        assert!(ok_reply(Value::Num(3.0), Value::obj()).contains(r#""id":3"#));
        assert!(ok_reply(Value::Str("req-1".into()), Value::obj()).contains(r#""id":"req-1""#));
    }

    #[test]
    fn every_reply_is_exactly_one_line() {
        let r = ok_reply(Value::Num(1.0), tool_text("first\nsecond\nthird"));
        assert!(!r.contains('\n'), "a passage with newlines must not split the message");
        let parsed = json::parse(&r).expect("a reply must parse");
        let text = parsed.get("result").unwrap().get("content").unwrap();
        match text {
            Value::Arr(items) => {
                assert_eq!(items[0].get("text").unwrap().as_str(), Some("first\nsecond\nthird"));
            }
            _ => panic!("content must be a list"),
        }
    }

    #[test]
    fn a_tool_schema_uses_the_camel_case_key_the_spec_writes() {
        let t = tool("kb_route", "d", vec![("question", "string", "q", true)]);
        assert!(t.get("inputSchema").is_some(), "input_schema is silently ignored by clients");
        let req = t.get("inputSchema").unwrap().get("required").unwrap();
        assert_eq!(req, &Value::Arr(vec![Value::Str("question".into())]));
    }

    #[test]
    fn an_optional_argument_is_not_marked_required() {
        let t = tool(
            "kb_route",
            "d",
            vec![("question", "string", "q", true), ("top", "integer", "n", false)],
        );
        let req = t.get("inputSchema").unwrap().get("required").unwrap();
        assert_eq!(req, &Value::Arr(vec![Value::Str("question".into())]));
    }

    #[test]
    fn an_error_reply_is_well_formed_json_rpc() {
        let r = error_reply(Value::Num(7.0), METHOD_NOT_FOUND, "unknown method: nope");
        let v = json::parse(&r).expect("parse");
        assert_eq!(v.get("jsonrpc").unwrap().as_str(), Some("2.0"));
        assert_eq!(v.get("error").unwrap().get("code").unwrap().as_f64(), Some(-32601.0));
        assert!(v.get("result").is_none(), "a reply carries result or error, never both");
    }

    #[test]
    fn a_missing_argument_is_named_in_the_error() {
        let args = Value::obj();
        let err = string_arg(&args, "question").unwrap_err();
        assert!(err.contains("question"));
        let mut blank = Value::obj();
        blank.set("question", "   ".into());
        assert!(string_arg(&blank, "question").is_err(), "whitespace is not a question");
    }

    /// Accents have to survive the whole round trip, because the alias table exists
    /// precisely so a Portuguese question can reach an English base.
    #[test]
    fn a_question_with_accents_survives_the_wire() {
        let mut args = Value::obj();
        args.set("question", "por que o poke é caro em proteína?".into());
        assert_eq!(
            string_arg(&args, "question").unwrap(),
            "por que o poke é caro em proteína?"
        );
    }

    /// A base with one note the text scorer can reach and the keyword scorer cannot:
    /// its keys, its title and its purpose are about an animal, its body is about
    /// deploys. Indexed, because `retrieve` reads chunks and `Memory::open` builds no
    /// index of its own.
    ///
    /// Written out again here rather than shared with `main.rs`, which has the same
    /// fixture: the binary and the library are two crates, and a `#[cfg(test)]` helper
    /// in one is not compiled into the other. Duplicating fifteen lines of fixture is
    /// the cheaper of the two honest options.
    /// **The refusal has to carry the next act, on the surface a model actually queries.**
    ///
    /// `served_fleet` is a one entry base, so this is the small fleet case exactly: a
    /// question with no matching term is refused, and today the reply says only that
    /// nothing matched and that the keys may be wrong. On a fleet of one entry the keys
    /// are not what refused it and never could have been, and the reader is sent to edit
    /// a `Search for:` line that would not have helped.
    ///
    /// The second half of this test is the one that catches a careless insertion:
    /// `no_match` returns early when there are no suggestions, so a paragraph appended
    /// after that return reaches nobody, and a paragraph inserted before the suggestion
    /// block must not swallow it.
    #[test]
    fn a_miss_on_a_small_fleet_carries_the_next_act_and_not_just_the_refusal() {
        let (_, server) = served_fleet("shortfall-small");
        let out = server.route("qual a taxa de juros do trimestre", 4);

        assert!(out.contains("Nothing matched"), "the refusal is still the first thing: {out}");
        assert!(out.contains('1'), "and it names the size it refused from: {out}");
        assert!(
            out.contains(&crate::memory::MIN_ENTRIES_TO_ROUTE.to_string()),
            "and the threshold that size fails: {out}"
        );
        // A second question, because the first has nothing that looks like it and takes
        // `no_match`'s early return. A misspelling reaches the trigram suggestions, so this
        // call carries both paragraphs and pins their order: the state was inserted before
        // that return and did not swallow what comes after it.
        let typo = server.route("zebr listrada", 4);
        assert!(typo.contains("look like words you used"), "the suggestions survive: {typo}");
        assert!(typo.contains("vocabulary miss"), "and the state is there too: {typo}");
        assert!(
            typo.find("vocabulary miss") < typo.find("look like words you used"),
            "state first, suggestions last: {typo}"
        );
    }

    /// **The same blame-the-question defect, one level down.**
    ///
    /// The empty base reply tells a first time reader to give each file a `Search for:`
    /// line. On a base whose markdown is already written without one, that is advice about
    /// files they have already got, and the reply says nothing about them. The count is
    /// the difference between "start writing" and "seven files are one line short".
    #[test]
    fn an_empty_base_names_the_files_that_declare_no_keys() {
        let root = std::env::temp_dir()
            .join("kb-mcp-miss-tests")
            .join(format!("{}-shortfall-empty", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let agent = root.join("probe");
        std::fs::create_dir_all(agent.join("knowledge")).expect("mkdir");
        std::fs::write(agent.join("MAP.md"), "# MAP\n").expect("map");
        std::fs::write(
            agent.join("knowledge").join("keyless.md"),
            "# Keyless\n\nwritten before anybody knew about the line.\n",
        )
        .expect("note");

        let memory = Memory::open(&[agent.as_path()], true).expect("opens");
        assert!(memory.is_empty(), "no keys means no entries, which is the fixture");
        let server = Server { memory, top: 4 };

        let out = server.route("qual a taxa de juros do trimestre", 4);
        assert!(out.contains("no knowledge files yet"), "still the empty base reply: {out}");
        assert!(out.contains('1'), "and it counts the file already on disk: {out}");
        assert!(out.contains("markdown file is"), "one file, singular: {out}");
        // Beside the paragraph that explains the line, not after the closing note, which
        // is the order the terminal copy of this reply already prints.
        assert!(
            out.find("markdown file is") < out.find("kb_fleet works regardless"),
            "the count comes before the closing note: {out}"
        );
    }

    fn served_fleet(name: &str) -> (std::path::PathBuf, Server) {
        let root = std::env::temp_dir()
            .join("kb-mcp-miss-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let agent = root.join("probe");
        std::fs::create_dir_all(agent.join("knowledge")).expect("mkdir");
        std::fs::write(agent.join("MAP.md"), "# MAP\n").expect("map");
        std::fs::write(
            agent.join("knowledge").join("striped.md"),
            "# Zebra\n\n**Search for:** `zebra`, `quagga`\n\n\
             **Exists to:** hold one striped animal\n\n\
             ## Body\n\nA rollback without downtime keeps the previous release serving.\n",
        )
        .expect("note");

        let base = crate::base::Base::discover(&agent, true).expect("discover");
        let mut db = crate::store::Store::open(&crate::memory::index_path(&agent)).expect("index");
        db.sync(&base, "probe").expect("sync");
        drop(db);

        let memory = Memory::open(&[agent.as_path()], true).expect("opens");
        (agent, Server { memory, top: 4 })
    }

    /// F-02, on the surface where it actually bit. `retrieve` decided what to record
    /// by asking whether the fused list was empty, so a question the text scorer
    /// answered and the gate refused was served to the model as passages and recorded
    /// as nothing at all. That is the loss the log most needs, because the base holds
    /// the answer and only its keys are wrong, which is the cheapest fix there is.
    #[test]
    fn a_refusal_over_a_full_result_set_is_recorded_by_this_surface_too() {
        let (root, server) = served_fleet("retrieve");
        let log = crate::misses::path_in(&root);

        let out = server.retrieve("rollback sem downtime", 4);
        assert!(out.contains("Passages for"), "the text scorer found it: {out}");

        let written = std::fs::read_to_string(&log).expect("the refusal was recorded");
        assert!(written.contains("rollback sem downtime"), "{written}");
    }

    /// And the same question through the other tool records the same thing. The two
    /// used to test different lists, so which door a question came through decided
    /// whether it counted.
    #[test]
    fn both_tools_agree_about_what_a_recall_loss_is() {
        let (root, server) = served_fleet("both");
        let log = crate::misses::path_in(&root);

        server.route("rollback sem downtime", 4);
        let after_route = std::fs::read_to_string(&log).expect("route recorded it");
        assert!(after_route.contains("rollback sem downtime"), "{after_route}");

        server.retrieve("rollback sem downtime", 4);
        let after_both = std::fs::read_to_string(&log).expect("retrieve recorded it too");
        assert!(
            after_both.contains("2    "),
            "one question, counted twice, not two lines: {after_both}"
        );
    }

    /// The label reaches the one surface a model actually queries. A raw drop in the
    /// deposit is served, and the model is told what it is holding.
    #[test]
    fn a_passage_from_the_deposit_is_served_with_its_label_on() {
        let (root, server) = served_fleet("short-memory");
        std::fs::create_dir_all(root.join("inbox")).expect("mkdir");
        std::fs::write(
            root.join("inbox").join("dropped.md"),
            "# Dropped\n\nthe quagga population doubled last spring\n",
        )
        .expect("drop");
        // The fixture indexed before the drop existed; index it the way `kb index` would.
        let base = crate::base::Base::discover(&root, true).expect("discover");
        let mut db = crate::store::Store::open(&crate::memory::index_path(&root)).expect("index");
        db.sync(&base, "probe").expect("sync");
        drop(db);
        let server = Server { memory: Memory::open(&[root.as_path()], true).expect("opens"), top: server.top };

        let out = server.retrieve("quagga population", 4);
        assert!(out.contains("inbox/dropped.md"), "the deposit is served: {out}");
        assert!(out.contains("[short memory: recent, not distilled"), "and labelled: {out}");
    }

    /// The filter tool is offered, and it cannot be reached with a ranking question.
    ///
    /// **`required` is an empty array and `question` is not a property**, which is the
    /// confusion the tool exists to end: the other four all score a question against a
    /// floor, so a filter shaped ask with no ranking problem in it came back as a
    /// guess. A tool that accepted a question here would be a fifth scorer with no
    /// floor, which is worse than the four.
    ///
    /// The four existing tools are asserted present in the same test, because a tool
    /// added by editing an array is a tool that can displace one.
    #[test]
    fn the_list_tool_is_offered_and_requires_no_question() {
        let (_, server) = served_fleet("tools-list");
        let listed = server.tools_list();
        let tools = match listed.get("tools") {
            Some(Value::Arr(a)) => a.clone(),
            other => panic!("tools is not an array: {other:?}"),
        };
        let names: Vec<&str> = tools.iter().filter_map(|t| t.get("name")?.as_str()).collect();
        for existing in ["kb_route", "kb_retrieve", "kb_remember", "kb_fleet"] {
            assert!(names.contains(&existing), "nothing was displaced: {names:?}");
        }
        assert!(names.contains(&"kb_list"), "{names:?}");

        let t = tools
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("kb_list"))
            .expect("present");
        let schema = t.get("inputSchema").expect("camelCase, as the spec writes it");
        assert_eq!(
            schema.get("required"),
            Some(&Value::Arr(Vec::new())),
            "every facet is optional: an omitted one does not filter"
        );
        assert!(
            schema.get("properties").and_then(|p| p.get("question")).is_none(),
            "there is no ranking question to ask this tool"
        );
    }

    /// This surface states no privacy rule of its own, which is what the module header
    /// already claims for every other tool: the declaration is read off each base by
    /// `Memory::open`, and nothing here is consulted about it. `.mcp.json` runs
    /// `kb serve .` with no `--all`, so the live server is Public scope and this is the
    /// arm that matters in production.
    #[test]
    fn the_list_tool_inherits_the_servers_scope_and_states_none_of_its_own() {
        let (root, _) = served_fleet("list-scope");
        std::fs::create_dir_all(root.join("profile")).expect("mkdir");
        std::fs::write(root.join("profile").join("me.md"), "# Me
").expect("private note");

        let public =
            Server { memory: Memory::open(&[root.as_path()], false).expect("opens"), top: 4 };
        let out = public.list(&Value::obj()).expect("no facet named is no filter, never an error");
        assert!(out.contains("knowledge/striped.md"), "the public shelf is listed: {out}");
        assert!(!out.contains("profile/me.md"), "and the private layer is not: {out}");

        let all = Server { memory: Memory::open(&[root.as_path()], true).expect("opens"), top: 4 };
        let out = all.list(&Value::obj()).expect("lists");
        assert!(out.contains("profile/me.md"), "--all is the deliberate act: {out}");
    }

    /// A served answer is not a loss, on this surface as on every other.
    #[test]
    fn a_question_the_base_answers_is_not_recorded() {
        let (root, server) = served_fleet("answered");
        server.retrieve("zebra", 4);
        assert!(
            !crate::misses::path_in(&root).exists(),
            "the keyword scorer ranked it, so nothing was lost"
        );
    }
}

/// The gate's evidence, in the same words every other surface uses.
///
/// **This surface used to emit none of it.** `kb ui` reports verdict, score, floor and
/// margin; `kb route` and `kb boot` report them too; `kb serve`, the one surface a model
/// actually queries, reported two prose NOTE blocks and no number at all. A model reading
/// this tool could not tell a top score of 188.6 from one of 19.9, so "the base does not
/// cover this" and "here are five files" arrived looking the same. Confidence that does
/// not reach the caller is confidence the caller cannot act on.
///
/// Agreement is reported and does not gate, for the reason recorded in
/// `Memory::confidence_of`: the text side is merged round robin per agent, so a file that
/// is its own agent's best match is admitted without competing against the others.
fn evidence(c: &crate::memory::Confidence) -> String {
    format!(
        "EVIDENCE: keyword score {:.1} against a floor of {:.1}; {} of the two independent \
         scorers ranked the top file; it leads the runner-up by {:.2}x. Verdict: {}.\n\n",
        c.keyword_score,
        c.floor,
        match c.agreement {
            2 => "both",
            1 => "only one",
            _ => "neither",
        },
        c.margin,
        match c.verdict {
            crate::memory::Verdict::Hit => "something here matches",
            crate::memory::Verdict::Guess =>
                "a guess, too weak or too close to the runner-up to tell from a coincidence \
                 of vocabulary",
            crate::memory::Verdict::Nothing => "nothing matched",
        }
    )
}
