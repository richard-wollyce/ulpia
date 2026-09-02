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
                return nothing_to_search();
            }
            return no_match(question, looked_like);
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
                return nothing_to_search();
            }
            return no_match(question, looked_like);
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
fn nothing_to_search() -> String {
    "This base has no knowledge files yet, so nothing could have matched. That is a \
     fact about the base and not about the question.\n\n\
     A base is filled by putting markdown in the agent's folder, giving each file a \
     `Search for:` line of its own near the top, naming the words a real question \
     would use, and running `kb index`. The line lives in the file it describes, not \
     in a list somewhere else, so a file without one is a file nothing can reach and \
     no map can rescue.\n\n\
     kb_fleet works regardless: who this fleet is and which agents exist is read \
     from fleet.txt and each agent.txt, not from the index, so identity is \
     answerable before a single note is written."
        .to_string()
}

fn no_match(question: &str, suggestions: &[String]) -> String {
    let mut out = format!(
        "Nothing matched \"{question}\".\n\n\
         Either the base does not cover it, or its keyword lines do not carry the words \
         a real question uses. Saying so plainly is deliberate: a router that always \
         returns something teaches you to trust a guess."
    );

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
        crate::memory::SCORE_FLOOR,
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
