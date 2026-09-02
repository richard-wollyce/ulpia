//! The reading room: a local web interface over the same Memory every surface uses.
//!
//! ADR-0009 drew the boundary this module lives inside: a GUI links the library and
//! calls it directly, it never grows its own pipeline. Everything here is a thin
//! projection of `Memory`, `Base`, `blocks` and `checks` into JSON, plus one embedded
//! HTML page. No answer is computed in this file that the CLI could not already give,
//! which is what keeps the two from ever disagreeing.
//!
//! ## What was stolen, and from where
//!
//! The shape of the page follows the first hand read of Letta's ADE recorded in
//! `letta-architecture`: conversation-shaped inspection in the middle, configuration to
//! one side, and **"what the agent actually has in its head" always visible**. Their
//! memory Palace renders memory files as a graph with references as edges; ours renders
//! the bases with `[[wikilinks]]` as edges, which we can do honestly with a parse pass
//! because the files are the truth (ADR-0003) and the links already exist. Their context
//! viewer shows the compiled prompt; ours shows what `kb boot` would inject, which is
//! stronger, because our injection is deterministic and theirs is approximated.
//!
//! The desk (chat) rides the same mechanism as every session: it spawns the Claude Code
//! CLI headless in the fleet root, and the `UserPromptSubmit` hook boots the agent there
//! exactly as it does here, so a chat message is routed by Vesta with zero code in this
//! file deciding anything. The CLI's `--output-format stream-json` is already a typed
//! message union, which is the half of Letta's wire design worth having. **The prompt
//! travels on the child's stdin, never as an argument**: on Windows a `.cmd` shim is
//! launched through cmd.exe, whose argument quoting is exploitable with attacker text
//! (the BatBadBut class), and stdin has no quoting to exploit.
//!
//! Their full WebSocket envelope (event_seq, idempotency_key, sync replay) is adopted in
//! one third: events carry a sequence number. The rest earns its complexity only with a
//! second client on a lossy transport, which is the phone, which is an ADR of its own:
//! it needs a non-loopback bind, a token, and an answer to PWAs requiring a secure
//! context off localhost. Deferred deliberately, not forgotten.
//!
//! ## Security posture, in one paragraph
//!
//! Binds 127.0.0.1 and nothing else, taking the loopback-trusted half of Letta's
//! "auth is layered by exposure" rule; a non-loopback bind is refused at argument time
//! rather than tokened, because this page has no business leaving the machine. File
//! reads are allowlisted, not sanitised: `/api/file` serves a path only if it is one of
//! the exact relative paths discovery already produced, so traversal is not "blocked",
//! it is unrepresentable. The private layer stays out the same way it does everywhere
//! else: `Base::discover` narrowed to what git tracks unless `--all` said otherwise.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::base::Base;
use crate::blocks;
use crate::checks;
use crate::fleet;
use crate::json::Value;
use crate::memory::{Memory, Verdict};

/// 114 AD is the consecration of the Bibliotheca Ulpia, and a fixed default beats a
/// random one: the address is typeable from memory and never printed into a config.
pub const DEFAULT_PORT: u16 = 4114;

/// Everything the server holds between requests.
///
/// Held resident on purpose: opening the bases costs about 280 ms on this machine
/// (measured for ADR-0022), and a page that stutters on every click teaches the user
/// to stop clicking. The cost is staleness, which is what `/api/reload` is for, and
/// the reload is explicit because a silent auto-refresh would make two consecutive
/// readings of the same page unexplainable when a file changed between them.
struct State {
    memory: Memory,
    /// One discovery per agent, kept beside the Memory so the graph and the file
    /// endpoint see exactly the set of files retrieval sees.
    bases: Vec<(String, Base)>,
    all: bool,
    roots: Vec<std::path::PathBuf>,
}

impl State {
    fn open(paths: &[&Path], all: bool) -> Result<State, String> {
        let memory = Memory::open(paths, all).map_err(|e| e.to_string())?;
        let mut bases = Vec::new();
        for agent in &memory.agents {
            let base = Base::discover(&agent.root, all).map_err(|e| e.to_string())?;
            bases.push((agent.name.clone(), base));
        }
        Ok(State {
            memory,
            bases,
            all,
            roots: paths.iter().map(|p| p.to_path_buf()).collect(),
        })
    }

    fn reload(&mut self) -> Result<(), String> {
        let refs: Vec<&Path> = self.roots.iter().map(|p| p.as_path()).collect();
        *self = State::open(&refs, self.all)?;
        Ok(())
    }
}

pub fn serve(paths: &[&str], all: bool, port: u16) -> Result<(), String> {
    let given: Vec<&Path> = paths.iter().map(Path::new).collect();
    let mut state = State::open(&given, all)?;

    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("cannot bind 127.0.0.1:{port}: {e}"))?;

    eprintln!("kb ui: http://127.0.0.1:{port}/  ({} agents, {} entries{})",
        state.memory.agents.len(),
        state.memory.entry_count(),
        if all { ", private layer INCLUDED" } else { "" });

    // Sequential on purpose. One local user, and a single thread means the State
    // needs no lock and a reload cannot race a read half way through.
    // Sequential for everything that reads State: one local user, no lock, and a
    // reload cannot race a read. The one exception is the desk: a chat turn takes as
    // long as the model takes, and holding every other panel hostage to it teaches
    // the user not to chat. Chat requests get their socket and a thread, and touch
    // no State at all: the child process and the boot hook do the routing, which is
    // ADR-0022 doing the work.
    let chats: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let root: PathBuf =
        state.roots.first().cloned().unwrap_or_else(|| PathBuf::from("."));

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let _ = handle(&mut stream, &mut state, &chats, &root);
    }
    Ok(())
}

fn handle(
    stream: &mut TcpStream,
    state: &mut State,
    chats: &Arc<Mutex<HashMap<String, String>>>,
    root: &Path,
) -> std::io::Result<()> {
    // Enough for a request line and headers; bodies are not read because no endpoint
    // takes one. A larger request is somebody else's protocol.
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let mut lines = request.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };

    match (method, path) {
        ("GET", "/") => respond(stream, 200, "text/html; charset=utf-8", PAGE),
        ("GET", "/api/fleet") => respond_json(stream, api_fleet(state)),
        ("GET", "/api/route") => {
            let q = param(query, "q").unwrap_or_default();
            respond_json(stream, api_route(state, &q))
        }
        ("GET", "/api/stacks") => respond_json(stream, api_stacks(state)),
        ("GET", "/font/400") => respond_bytes(stream, "font/woff2", FONT_400),
        ("GET", "/font/400i") => respond_bytes(stream, "font/woff2", FONT_400I),
        ("GET", "/font/600") => respond_bytes(stream, "font/woff2", FONT_600),
        ("GET", "/api/chat") => {
            let msg = param(query, "m").unwrap_or_default();
            let chat = param(query, "chat").unwrap_or_else(|| "desk".into());
            let resume = chats.lock().ok().and_then(|m| m.get(&chat).cloned());
            let chats = Arc::clone(chats);
            let root = root.to_path_buf();
            let stream = stream.try_clone()?;
            std::thread::spawn(move || {
                let _ = chat_stream(stream, &root, &chat, &msg, resume, &chats);
            });
            Ok(())
        }
        ("GET", "/api/blocks") => respond_json(stream, api_blocks(state)),
        ("GET", "/api/check") => respond_json(stream, api_check(state)),
        ("GET", "/api/file") => {
            let base = param(query, "base").unwrap_or_default();
            let rel = param(query, "rel").unwrap_or_default();
            match file_text(state, &base, &rel) {
                Some(text) => respond(stream, 200, "text/plain; charset=utf-8", &text),
                None => respond(stream, 404, "text/plain", "not a file this fleet serves"),
            }
        }
        ("POST", "/api/reload") => match state.reload() {
            Ok(()) => respond(stream, 200, "application/json", "{\"ok\":true}"),
            Err(e) => respond(stream, 500, "text/plain", &e),
        },
        _ => respond(stream, 404, "text/plain", "no such page"),
    }
}

fn respond(stream: &mut TcpStream, code: u16, ctype: &str, body: &str) -> std::io::Result<()> {
    let status = match code {
        200 => "200 OK",
        404 => "404 Not Found",
        _ => "500 Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())
}

fn respond_json(stream: &mut TcpStream, v: Value) -> std::io::Result<()> {
    respond(stream, 200, "application/json", &v.to_string())
}

fn respond_bytes(stream: &mut TcpStream, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nCache-Control: max-age=31536000, immutable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)
}

// ---------------------------------------------------------------------------
// The desk
// ---------------------------------------------------------------------------

/// One chat turn: spawn the runtime headless, translate its typed stream onto SSE.
///
/// The child is `claude -p` with the fleet root as its working directory, so the
/// `UserPromptSubmit` hook fires inside it and Vesta routes the message exactly as it
/// routes this session's. This function forwards; it decides nothing.
///
/// SSE rather than the WebSocket envelope, deliberately: one client on loopback needs
/// ordering (the `seq` field) and nothing else the envelope sells. The body has no
/// Content-Length and ends when the socket closes, which HTTP/1.1 permits and
/// EventSource accepts.
fn chat_stream(
    mut stream: TcpStream,
    root: &Path,
    chat: &str,
    message: &str,
    resume: Option<String>,
    chats: &Arc<Mutex<HashMap<String, String>>>,
) -> std::io::Result<()> {
    stream.set_read_timeout(None).ok();
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
    )?;

    let mut seq: u64 = 0;
    // stream-json does not emit UserPromptSubmit hook events (verified 2026-08-19:
    // SessionStart hooks appear, the boot hook does not, although it demonstrably
    // runs, since it writes the routed agent under .kb/sessions/<session-id>). So
    // the routing line is read from the hook's own record on disk, once the first
    // real event proves the hook has already run.
    let mut sid: Option<String> = None;
    let mut routed_sent = false;
    let mut send = |stream: &mut TcpStream, kind: &str, text: &str| -> std::io::Result<()> {
        let mut v = Value::obj();
        v.set("seq", Value::Num(seq as f64));
        v.set("kind", Value::Str(kind.into()));
        v.set("text", Value::Str(text.into()));
        seq += 1;
        stream.write_all(format!("data: {}\n\n", v.to_string()).as_bytes())
    };

    if message.trim().is_empty() {
        return send(&mut stream, "error", "an empty message routes nowhere");
    }

    // `claude` on Windows is an npm shim, so the real target of Command::new is a
    // .cmd, which CreateProcess hands to cmd.exe. Every argument below is a fixed
    // string for exactly that reason; the message goes in on stdin, where there is
    // no quoting layer to exploit (the BatBadBut class of bug).
    let spawn_with = |program: &str| {
        let mut cmd = crate::base::quiet(program);
        cmd.arg("-p").arg("--output-format").arg("stream-json").arg("--verbose");
        if let Some(sid) = &resume {
            // The session id came from the child's own earlier output, validated on
            // capture below, never from the browser.
            cmd.arg("--resume").arg(sid);
        }
        cmd.current_dir(root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd.spawn()
    };

    // The shim name differs across installs; the bare name covers the unix shape.
    let mut child = match spawn_with("claude.cmd").or_else(|_| spawn_with("claude")) {
        Ok(c) => c,
        Err(e) => {
            return send(
                &mut stream,
                "error",
                &format!("the runtime did not start: {e}. Is Claude Code on PATH?"),
            );
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(message.as_bytes());
        // Dropped here, which closes the pipe; -p treats EOF as end of prompt.
    }

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return send(&mut stream, "error", "the runtime gave no output stream");
    };

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = crate::json::parse(&line) else { continue };
        let kind = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        let sent = match kind {
            "system" => {
                let sub = event.get("subtype").and_then(|t| t.as_str()).unwrap_or("");
                if sub == "init" {
                    if let Some(sid_str) = event.get("session_id").and_then(|s| s.as_str()) {
                        // Constrained to the shape a session id has, so a hostile
                        // child could not plant a flag through the resume path.
                        if !sid_str.is_empty()
                            && sid_str.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                        {
                            sid = Some(sid_str.to_string());
                            if let Ok(mut m) = chats.lock() {
                                m.insert(chat.to_string(), sid_str.to_string());
                            }
                        }
                    }
                    send(&mut stream, "session", "")
                } else if sub == "hook_response" {
                    // The boot hook's own stdout, which is Vesta saying who answers.
                    // Surfacing it makes the desk show the routing decision the same
                    // way this session's transcript shows it.
                    let out = event.get("stdout").and_then(|s| s.as_str()).unwrap_or("");
                    match out.lines().find(|l| l.starts_with("VESTA:")) {
                        Some(first) => send(&mut stream, "routed", first),
                        None => Ok(()),
                    }
                } else {
                    Ok(())
                }
            }
            "assistant" => {
                if !routed_sent {
                    routed_sent = true;
                    // The boot hook ran before the model spoke; its record is the
                    // truth about who is answering, per ADR-0022.
                    if let Some(sid) = &sid {
                        if let Ok(agent) =
                            std::fs::read_to_string(root.join(".kb/sessions").join(sid))
                        {
                            let agent = agent.trim();
                            if !agent.is_empty() {
                                let _ = send(
                                    &mut stream,
                                    "routed",
                                    &format!("VESTA: answering as {agent}"),
                                );
                            }
                        }
                    }
                }
                let is_error = event.get("error").and_then(|e| e.as_str()).is_some();
                let mut wrote = Ok(());
                if let Some(Value::Arr(content)) =
                    event.get("message").and_then(|m| m.get("content"))
                {
                    for item in content {
                        match item.get("type").and_then(|t| t.as_str()) {
                            Some("text") => {
                                let text =
                                    item.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                wrote = send(
                                    &mut stream,
                                    if is_error { "error" } else { "assistant" },
                                    text,
                                );
                            }
                            Some("tool_use") => {
                                let name =
                                    item.get("name").and_then(|t| t.as_str()).unwrap_or("tool");
                                wrote = send(&mut stream, "tool", name);
                            }
                            _ => {}
                        }
                    }
                }
                wrote
            }
            "result" => send(&mut stream, "done", ""),
            _ => Ok(()),
        };

        // A dead socket means the reader left, so the child has nobody to talk to.
        if sent.is_err() {
            let _ = child.kill();
            return Ok(());
        }
    }

    let _ = child.wait();
    Ok(())
}

const FONT_400: &[u8] = include_bytes!("fonts/eb-garamond-latin-400-normal.woff2");
const FONT_400I: &[u8] = include_bytes!("fonts/eb-garamond-latin-400-italic.woff2");
const FONT_600: &[u8] = include_bytes!("fonts/eb-garamond-latin-600-normal.woff2");

/// One query parameter, percent-decoded, `+` as space.
///
/// Hand rolled like the JSON parser and for the same reason: the alternative is a
/// dependency, and the grammar is one page of RFC.
fn param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == name {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => {
                let hex = |b: u8| -> Option<u8> {
                    match b {
                        b'0'..=b'9' => Some(b - b'0'),
                        b'a'..=b'f' => Some(b - b'a' + 10),
                        b'A'..=b'F' => Some(b - b'A' + 10),
                        _ => None,
                    }
                };
                // A bare or malformed escape passes through as a literal percent,
                // because a decoder that errors on user input turns a typo in the
                // ask box into a failed request.
                if i + 2 < bytes.len() {
                    if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                        out.push(h * 16 + l);
                        i += 3;
                        continue;
                    }
                }
                out.push(b'%');
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// The projections
// ---------------------------------------------------------------------------

fn api_fleet(state: &State) -> Value {
    let mut agents = Vec::new();
    for agent in &state.memory.agents {
        let card = fleet::card(&agent.root, "agent.txt", &agent.name);
        let files = state
            .bases
            .iter()
            .find(|(n, _)| n == &agent.name)
            .map(|(_, b)| b.files.len())
            .unwrap_or(0);
        let mut v = Value::obj();
        v.set("name", Value::Str(card.name));
        v.set("role", Value::Str(card.role.unwrap_or_default()));
        v.set("routable", Value::Bool(agent.routable));
        v.set("files", Value::Num(files as f64));
        agents.push(v);
    }
    let mut out = Value::obj();
    out.set("agents", Value::Arr(agents));
    out.set("entries", Value::Num(state.memory.entry_count() as f64));
    out.set("aliases", Value::Num(state.memory.alias_count() as f64));
    out.set("private", Value::Bool(state.all));
    out
}

fn api_route(state: &State, question: &str) -> Value {
    let mut out = Value::obj();
    out.set("question", Value::Str(question.to_string()));
    if question.trim().is_empty() {
        out.set("verdict", Value::Str("empty".into()));
        return out;
    }

    let answer = state.memory.ask(question, 5);
    // The reading room asked and never counted. ADR-0035, same line as every other surface.
    let _ = state.memory.recall_loss(question, &answer.confidence);

    out.set(
        "verdict",
        Value::Str(
            match answer.confidence.verdict {
                Verdict::Hit => "hit",
                Verdict::Guess => "guess",
                Verdict::Nothing => "nothing",
            }
            .into(),
        ),
    );
    out.set("score", Value::Num(answer.confidence.keyword_score as f64));
    out.set("floor", Value::Num(crate::memory::SCORE_FLOOR as f64));
    out.set("margin", Value::Num(answer.confidence.margin as f64));

    if let Some(choice) = &answer.agent {
        let mut a = Value::obj();
        a.set("name", Value::Str(choice.agent.clone()));
        a.set("score", Value::Num(choice.score));
        a.set("files", Value::Num(choice.files as f64));
        a.set("contenders", Value::Num(choice.contenders as f64));
        let mut totals = Vec::new();
        for (name, weight) in &choice.totals {
            let mut t = Value::obj();
            t.set("name", Value::Str(name.clone()));
            t.set("score", Value::Num(*weight));
            totals.push(t);
        }
        a.set("totals", Value::Arr(totals));
        out.set("agent", a);
    }

    let mut found = Vec::new();
    for f in &answer.found {
        let mut v = Value::obj();
        v.set("base", Value::Str(f.base.clone()));
        v.set("rel", Value::Str(f.path.clone()));
        v.set("title", Value::Str(f.title.clone()));
        v.set("fused", Value::Num(f.score));
        v.set("keyword", Value::Num(f.keyword_score as f64));
        v.set("why", Value::Str(f.why.join(" + ")));
        v.set("matched", Value::Str(f.matched.join(", ")));
        if let Some(p) = f.passages.first() {
            v.set("heading", Value::Str(p.heading_path.clone()));
            v.set("excerpt", Value::Str(p.excerpt.clone()));
        }
        found.push(v);
    }
    out.set("found", Value::Arr(found));
    out
}

/// The bases as the stacks: one shelf per agent, files as book spines.
///
/// This replaces the force graph, on Richard's direction: the fleet's own language is
/// the library (ADR-0019), so the visualization is shelves and books rather than nodes
/// and springs. The graph's information survives whole, in book form:
///
/// - An **edge out** is the book's `cites` count, listed when the book is opened.
/// - An **edge in from another agent's base** becomes a **ribbon** on the spine, in
///   that agent's color: the visible mark that more than one agent works this
///   document. That was the question the graph answered with crossing lines; a
///   bookmark left in a borrowed book answers it in the library's own grammar.
/// - A **broken link** is listed on the open book as a citation to a missing work,
///   which is E01 said the way a reader would say it.
///
/// Shelf order is the fleet's order; the record office (an attached, non-routable
/// base like `decisions/`) shelves beside the agents, since the Bibliotheca Ulpia
/// was reading room and record office in one building, which is the whole name.
fn api_stacks(state: &State) -> Value {
    // Backlinks first: who cites whom, resolved once across the fleet.
    // target id -> list of (citing base, citing rel)
    let mut cited_by: HashMap<String, Vec<(String, String)>> = HashMap::new();
    // source id -> (resolved targets, broken stems)
    let mut cites: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();

    for (name, base) in &state.bases {
        for file in &base.files {
            let is_map = base.map.as_deref() == Some(file.rel.as_str());
            if is_map {
                // The catalog cites everything it catalogs, by construction. Recording
                // that would put every agent's ribbon on every book and say nothing.
                continue;
            }
            let id = format!("{}/{}", name, file.rel);
            let entry = cites.entry(id.clone()).or_default();

            // Wikilinks: inside the base, or broken. Same rule as the linter.
            for (_, target) in checks::wikilinks(&file.text) {
                match resolve(state, name, &target) {
                    Some(to) => {
                        if !entry.0.contains(&to) {
                            entry.0.push(to.clone());
                            cited_by
                                .entry(to)
                                .or_default()
                                .push((name.clone(), file.rel.clone()));
                        }
                    }
                    None => {
                        if !entry.1.contains(&target) {
                            entry.1.push(target);
                        }
                    }
                }
            }

            // Written-out paths: the only way to point at another base, and therefore
            // the only thing a ribbon can honestly be made of.
            for to in cross_base_refs(state, name, &file.text) {
                if !entry.0.contains(&to) {
                    entry.0.push(to.clone());
                    cited_by
                        .entry(to)
                        .or_default()
                        .push((name.clone(), file.rel.clone()));
                }
            }
        }
    }

    let mut shelves = Vec::new();
    for (name, base) in &state.bases {
        let routable = state
            .memory
            .agents
            .iter()
            .find(|a| &a.name == name)
            .map(|a| a.routable)
            .unwrap_or(false);

        let mut books = Vec::new();
        for file in &base.files {
            let id = format!("{}/{}", name, file.rel);
            let (out_links, broken) = cites.get(&id).cloned().unwrap_or_default();

            // A ribbon per *other* base that cites this book, one each however many
            // of its files do: the ribbon says "that agent works here", not how often.
            let mut ribbons: Vec<String> = Vec::new();
            for (citer, _) in cited_by.get(&id).map(|v| v.as_slice()).unwrap_or(&[]) {
                if citer != name && !ribbons.contains(citer) {
                    ribbons.push(citer.clone());
                }
            }

            let mut b = Value::obj();
            b.set("rel", Value::Str(file.rel.clone()));
            b.set("stem", Value::Str(file.stem.clone()));
            // The book's own first heading is its title. The file is already in
            // memory, and a note that opens with anything else has no better name
            // than its stem, which the front already shows.
            let title = file
                .text
                .lines()
                .find(|l| l.starts_with("# "))
                .map(|l| l[2..].trim().to_string())
                .unwrap_or_default();
            b.set("title", Value::Str(title));
            b.set("bytes", Value::Num(file.text.len() as f64));
            b.set("folder", Value::Str(
                file.rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default(),
            ));
            b.set("cites", Value::Arr(out_links.into_iter().map(Value::Str).collect()));
            b.set("broken", Value::Arr(broken.into_iter().map(Value::Str).collect()));
            b.set(
                "cited_by",
                Value::Arr(
                    cited_by
                        .get(&id)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[])
                        .iter()
                        .map(|(citer, rel)| Value::Str(format!("{}/{}", citer, rel)))
                        .collect(),
                ),
            );
            b.set("ribbons", Value::Arr(ribbons.into_iter().map(Value::Str).collect()));
            books.push(b);
        }

        let mut shelf = Value::obj();
        shelf.set("agent", Value::Str(name.clone()));
        shelf.set("routable", Value::Bool(routable));
        shelf.set("books", Value::Arr(books));
        shelves.push(shelf);
    }

    let mut out = Value::obj();
    out.set("shelves", Value::Arr(shelves));
    out
}

/// A `[[wikilink]]` resolves **inside its own base and nowhere else**, which is the same
/// rule `kb check` enforces. Z38 was the two disagreeing: this function used to try the
/// home base and then the whole fleet, so the reading room drew edges the linter called
/// broken, and moving one file between bases produced both behaviours at once.
///
/// **Base scope wins, and the deciding argument is privacy rather than tidiness.** A base
/// is a privacy boundary: the fleet repository is gitignored by the public one, and
/// `yaron/profile/` is gitignored inside the fleet. A link that silently resolves across
/// that boundary is how a reference to private material lands in a publishable file with
/// nobody noticing, and the publication audit already had to undo exactly that by hand.
///
/// Two more reasons it would be wrong the other way. A base is addressed by path and may
/// be opened alone (ADR-0008), so a link that only resolves when a sibling happens to be
/// mounted is not a link, it is a coincidence of mounting. And two agents may hold the
/// same stem, which they do, so fleet-wide resolution has no correct answer, only a
/// discovery order.
///
/// Crossing a base is therefore written out in full, as a path, which is what the
/// publication audit produced and what [`cross_base_refs`] reads.
fn resolve(state: &State, home: &str, stem: &str) -> Option<String> {
    let (name, base) = state.bases.iter().find(|(n, _)| n == home)?;
    base.files
        .iter()
        .find(|f| f.stem.eq_ignore_ascii_case(stem))
        .map(|f| format!("{}/{}", name, f.rel))
}

/// Explicit references to another base, written as `<base>/<path>.md` in the prose.
///
/// This is what a cross-base citation looks like now that wikilinks stop at the base
/// edge, so it is what a ribbon has to be built from. It is also the more honest signal:
/// the author wrote the whole path, meaning they knew they were pointing outside.
///
/// A reference is only counted when the named base actually holds that file, so a path that
/// merely looks like one does not manufacture a ribbon.
///
/// **This guard was written around a name collision that no longer exists.** The shared
/// person base was called `profile`, so every `profile/...md` written inside Yaron's own
/// private `profile/` folder was a candidate false ribbon, and the example this comment used
/// to cite (`yaron/profile/profile.md`) has since moved to the shared base as `body.md`.
/// ADR-0029 renamed the base to `person`, and the hazard dissolved with the name. The guard
/// stays because it is generic: it protects any future base whose name repeats inside an
/// agent, which nothing in the tool prevents.
fn cross_base_refs(state: &State, home: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (name, base) in &state.bases {
        if name == home {
            continue;
        }
        let needle = format!("{name}/");
        let mut from = 0usize;
        while let Some(hit) = text[from..].find(&needle) {
            let start = from + hit;
            from = start + needle.len();

            // A path is a whole word: `person/x.md` counts, `../person/x.md` and
            // `yaron/person/x.md` do not, because those name something else.
            let preceded_by_path_char = start > 0
                && text[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c == '/' || c == '.' || c.is_alphanumeric() || c == '-');
            if preceded_by_path_char {
                continue;
            }

            let rest = &text[from..];
            let end = rest.find(".md").map(|i| i + 3);
            let Some(end) = end else { continue };
            let rel = &rest[..end - 3];
            if rel.is_empty() || rel.contains(char::is_whitespace) || rel.contains("..") {
                continue;
            }
            let rel = format!("{rel}.md");
            if base.files.iter().any(|f| f.rel == rel) {
                let id = format!("{name}/{rel}");
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

fn api_blocks(state: &State) -> Value {
    let mut agents = Vec::new();
    for agent in &state.memory.agents {
        let Some(blocks) = blocks::read(&agent.root) else { continue };
        let mut list = Vec::new();
        for b in &blocks {
            let mut v = Value::obj();
            v.set("name", Value::Str(b.name.clone()));
            v.set(
                "mode",
                Value::Str(format!("{:?}", b.mode).to_lowercase()),
            );
            v.set("bytes", Value::Num(b.bytes as f64));
            v.set("files", Value::Arr(b.files.iter().map(|f| Value::Str(f.clone())).collect()));
            v.set(
                "missing",
                Value::Arr(b.missing.iter().map(|f| Value::Str(f.clone())).collect()),
            );
            list.push(v);
        }
        let mut costs = Vec::new();
        for (name, cost) in blocks::invalidation_cost(&blocks) {
            let mut c = Value::obj();
            c.set("name", Value::Str(name));
            c.set("cost", Value::Num(cost as f64));
            costs.push(c);
        }
        let mut a = Value::obj();
        a.set("agent", Value::Str(agent.name.clone()));
        a.set("blocks", Value::Arr(list));
        a.set("invalidation", Value::Arr(costs));
        agents.push(a);
    }
    let mut out = Value::obj();
    out.set("agents", Value::Arr(agents));
    out
}

fn api_check(state: &State) -> Value {
    let mut all = Vec::new();
    for (name, base) in &state.bases {
        for finding in checks::run(base) {
            let mut v = Value::obj();
            v.set("base", Value::Str(name.clone()));
            let level = match finding.level {
                checks::Level::Error => "error",
                checks::Level::Warning => "warning",
            };
            v.set("level", Value::Str(level.to_string()));
            v.set("code", Value::Str(finding.code.to_string()));
            v.set("file", Value::Str(finding.file.clone()));
            v.set("line", Value::Num(finding.line as f64));
            v.set("message", Value::Str(finding.message.clone()));
            all.push(v);
        }
    }
    let mut out = Value::obj();
    out.set("findings", Value::Arr(all));
    out
}

/// A file's text, only if discovery already produced exactly this (base, rel) pair.
///
/// This is an allowlist, not a sanitiser. There is no path arithmetic to get wrong:
/// the rel string either equals one that `Base::discover` returned, respecting the
/// declared private layer, or nothing is served. Traversal is unrepresentable rather
/// than rejected.
fn file_text(state: &State, base: &str, rel: &str) -> Option<String> {
    let (_, b) = state.bases.iter().find(|(n, _)| n.eq_ignore_ascii_case(base))?;
    b.files.iter().find(|f| f.rel == rel).map(|f| f.text.clone())
}

const PAGE: &str = include_str!("ui.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_handles_the_shapes_a_browser_sends() {
        assert_eq!(percent_decode("quanto+de+proteina"), "quanto de proteina");
        assert_eq!(percent_decode("caf%C3%A9"), "café");
        assert_eq!(percent_decode("a%2Fb%3Fc"), "a/b?c");
        assert_eq!(percent_decode("100%"), "100%", "a bare percent is not an escape");
    }

    #[test]
    fn param_finds_its_key_and_only_its_key() {
        assert_eq!(param("q=hello+world&x=1", "q").as_deref(), Some("hello world"));
        assert_eq!(param("q=hello", "x"), None);
        assert_eq!(param("", "q"), None);
    }

    /// Named per test, because tests in one binary run in parallel and a shared
    /// scratch directory is a race: this exact helper without the name parameter
    /// failed intermittently in the full run and passed alone.
    fn scratch_state(name: &str) -> State {
        let dir = std::env::temp_dir()
            .join("kb-ui-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let agent = dir.join("fleet").join("probe");
        std::fs::create_dir_all(agent.join("knowledge")).expect("mkdir");
        std::fs::write(
            agent.join("MAP.md"),
            "# MAP\n\n- **[[real-note]]** a note\n  Search for: `probe`\n",
        )
        .expect("map");
        std::fs::write(
            agent.join("knowledge").join("real-note.md"),
            "# Real\n\nSee [[missing-note]].\n",
        )
        .expect("note");
        std::fs::write(agent.join("agent.txt"), "name = Probe\nrole = testing\n").expect("agent");
        State::open(&[dir.as_path()], true).expect("opens")
    }

    /// The reading room asked and never counted, ADR-0035. Same one line as every
    /// other surface, tested because the last time a surface was "obviously" covered it
    /// was found by running four of them over one question and reading 1, 1, 2, 3.
    #[test]
    fn a_refused_question_in_the_reading_room_is_counted() {
        let state = scratch_state("loss");
        let root = state.bases.first().map(|(_, b)| b.root.clone()).expect("one base");
        let fleet = root.parent().and_then(|p| p.parent()).expect("the scratch root");
        let _ = api_route(&state, "qual a taxa de juros do trimestre");
        let log = std::fs::read_to_string(crate::misses::path_in(fleet)).expect("counted");
        assert!(log.contains("qual a taxa de juros do trimestre"), "{log}");
    }

    /// The security property the whole endpoint rests on: a path that discovery did
    /// not produce is not served, however it is spelled. There is no traversal to
    /// block because there is no path resolution at all.
    #[test]
    fn a_file_outside_the_discovered_set_is_not_served_however_spelled() {
        let state = scratch_state("allowlist");
        assert!(file_text(&state, "probe", "knowledge/real-note.md").is_some());
        assert!(file_text(&state, "probe", "../../../etc/passwd").is_none());
        assert!(file_text(&state, "probe", "..\\..\\secrets.txt").is_none());
        assert!(file_text(&state, "probe", "C:/Windows/win.ini").is_none());
        assert!(file_text(&state, "nosuch", "knowledge/real-note.md").is_none());
    }

    /// A dangling wikilink is listed on the open book as a citation of a missing
    /// work, not dropped: the stacks are E01 made visible, exactly as the graph was.
    #[test]
    fn a_book_lists_its_broken_citations_rather_than_hiding_them() {
        let state = scratch_state("broken");
        let stacks = api_stacks(&state);
        let shelves = match stacks.get("shelves") {
            Some(Value::Arr(v)) => v,
            _ => panic!("shelves"),
        };
        assert_eq!(shelves.len(), 1);
        let books = match shelves[0].get("books") {
            Some(Value::Arr(v)) => v,
            _ => panic!("books"),
        };
        let note = books
            .iter()
            .find(|b| b.get("stem") == Some(&Value::Str("real-note".into())))
            .expect("the note shelves");
        let broken = match note.get("broken") {
            Some(Value::Arr(v)) => v,
            _ => panic!("broken"),
        };
        assert_eq!(broken, &vec![Value::Str("missing-note".into())]);
    }

    /// Z38, decided and applied: a wikilink stops at the base edge, in the reading
    /// room exactly as in the linter. The version before this resolved fleet-wide, so
    /// the two disagreed about the same file.
    #[test]
    fn a_wikilink_does_not_reach_into_another_base() {
        let dir = std::env::temp_dir()
            .join("kb-ui-scope")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for agent in ["alfa", "beta"] {
            let root = dir.join("fleet").join(agent);
            std::fs::create_dir_all(root.join("knowledge")).expect("mkdir");
            std::fs::write(root.join("agent.txt"), "name = X
role = t
").expect("a");
            std::fs::write(root.join("MAP.md"), "# MAP

- **[[a]]** x
  Search for: `x`
")
                .expect("map");
        }
        std::fs::write(dir.join("fleet/alfa/knowledge/only-in-alfa.md"), "# A

x
")
            .expect("w");
        // beta points at a stem that exists only in alfa, two ways: as a wikilink,
        // which must break, and as a written path, which must count.
        std::fs::write(
            dir.join("fleet/beta/knowledge/pointer.md"),
            "# B

See [[only-in-alfa]] and alfa/knowledge/only-in-alfa.md.
",
        )
        .expect("w");

        let state = State::open(&[dir.as_path()], true).expect("opens");
        assert_eq!(
            resolve(&state, "beta", "only-in-alfa"),
            None,
            "a wikilink must not reach across the base edge"
        );
        assert_eq!(
            resolve(&state, "alfa", "only-in-alfa").as_deref(),
            Some("alfa/knowledge/only-in-alfa.md"),
            "and must still resolve at home"
        );
        assert_eq!(
            cross_base_refs(&state, "beta", "See alfa/knowledge/only-in-alfa.md here"),
            vec!["alfa/knowledge/only-in-alfa.md"],
            "a written path is the way across, so it is what a ribbon counts"
        );
        assert!(
            cross_base_refs(&state, "beta", "see ../alfa/knowledge/only-in-alfa.md").is_empty(),
            "a relative path out of the tree is not a fleet reference"
        );
        assert!(
            cross_base_refs(&state, "alfa", "alfa/knowledge/only-in-alfa.md").is_empty(),
            "a base citing itself is not a cross-base reference"
        );
    }

    /// The multi-agent mark: a ribbon appears only for a citation arriving from
    /// another base, because a ribbon says "another agent works here" and a base
    /// citing its own book says nothing of the kind. Since Z38 the citation has to
    /// be a written path, because a wikilink stops at the base edge.
    #[test]
    fn a_ribbon_marks_a_citation_from_another_base_and_only_that() {
        let dir = std::env::temp_dir()
            .join("kb-ui-ribbon")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for agent in ["prosa", "verso"] {
            let root = dir.join("fleet").join(agent);
            std::fs::create_dir_all(root.join("knowledge")).expect("mkdir");
            std::fs::write(root.join("agent.txt"), "name = X\nrole = t\n").expect("a");
            std::fs::write(
                root.join("MAP.md"),
                "# MAP\n\n- **[[a]]** x\n  Search for: `x`\n",
            )
            .expect("map");
        }
        // prosa owns the book; verso cites a stem that only prosa has.
        std::fs::write(
            dir.join("fleet/prosa/knowledge/only-prosa-has-this.md"),
            "# The book\n\nno citations out\n",
        )
        .expect("w");
        std::fs::write(
            dir.join("fleet/verso/knowledge/versos-note.md"),
            "# Verso\n\nSee prosa/knowledge/only-prosa-has-this.md.\n",
        )
        .expect("w");

        let state = State::open(&[dir.as_path()], true).expect("opens");
        let stacks = api_stacks(&state);
        let shelves = match stacks.get("shelves") {
            Some(Value::Arr(v)) => v,
            _ => panic!("shelves"),
        };
        let mut ribboned = None;
        for shelf in shelves {
            if let Some(Value::Arr(books)) = shelf.get("books") {
                for b in books {
                    if b.get("stem") == Some(&Value::Str("only-prosa-has-this".into())) {
                        ribboned = Some(b.clone());
                    }
                }
            }
        }
        let book = ribboned.expect("the cited book shelves");
        assert_eq!(
            book.get("ribbons"),
            Some(&Value::Arr(vec![Value::Str("verso".into())])),
            "one ribbon, in the citing agent's name"
        );
    }

    #[test]
    fn the_fleet_projection_carries_role_and_routability() {
        let state = scratch_state("fleetcard");
        let fleet = api_fleet(&state);
        let agents = match fleet.get("agents") {
            Some(Value::Arr(a)) => a,
            _ => panic!("agents"),
        };
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].get("role"), Some(&Value::Str("testing".into())));
        assert_eq!(agents[0].get("routable"), Some(&Value::Bool(true)));
    }
}
