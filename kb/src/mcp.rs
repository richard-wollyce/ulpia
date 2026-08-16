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
//! proposes; nothing here edits a file. A write tool reached by a model is a
//! different security surface and gets built deliberately, not as an afterthought
//! while the retrieval side is still warm.

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

    // Everything the server knows about the base comes through Memory, including the
    // refusal when git could not be consulted. Rebuilding the pipeline here is how a
    // second caller ends up expanding aliases for one scorer and not the other, which
    // has already happened once in this codebase.
    let memory = match Memory::open(&refs, all) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kb serve: {e}");
            return ExitCode::from(1);
        }
    };

    if memory.index_was_rebuilt {
        eprintln!("kb serve: an index predated the tracked column and was emptied.");
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
                     ranked file paths with the words that matched, so a bad ranking can be \
                     diagnosed rather than guessed at. Cheap and does not return file contents. \
                     Use it to decide what to read; use kb_retrieve when you want the text.",
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
        if hits.is_empty() {
            return no_match(question);
        }

        let mut out = format!("Files to open for: {question}\n\n");
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
        let found = self.memory.retrieve(question, top);
        if found.is_empty() {
            return no_match(question);
        }

        let mut out = format!("Passages for: {question}\n\n");

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
                "index is the strongest signal available without a model, and there is none ",
                "here. Treat these as leads, and consider that the base may not cover the ",
                "question at all.

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

fn no_match(question: &str) -> String {
    format!(
        "Nothing matched \"{question}\".\n\n\
         Either the base does not cover it, or its keyword lines do not carry the words \
         a real question uses. Saying so plainly is deliberate: a router that always \
         returns something teaches you to trust a guess."
    )
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
}
