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
//! What was deliberately not stolen: their WebSocket envelope. It is the best engineered
//! piece of their stack (event_seq, idempotency_key, sync replay) and it earns its
//! complexity only when messages stream from a live model loop. This page inspects; it
//! does not chat. The envelope gets copied the day a model loop lives behind this server,
//! and not one day earlier, per the-bar's rule on complexity bought before the use case.
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

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

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
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let _ = handle(&mut stream, &mut state);
    }
    Ok(())
}

fn handle(stream: &mut TcpStream, state: &mut State) -> std::io::Result<()> {
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
        ("GET", "/api/graph") => respond_json(stream, api_graph(state)),
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

/// The bases as a graph: files are nodes, `[[wikilinks]]` are edges.
///
/// A link resolves by stem, same base first, then anywhere in the fleet, which is the
/// order a reader would try. One that resolves nowhere is still emitted, flagged
/// broken, because the graph showing a dangling reference is `kb check`'s E01 made
/// visible instead of a lint line nobody reads.
fn api_graph(state: &State) -> Value {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for (name, base) in &state.bases {
        for file in &base.files {
            // The map's own links are structural: it points at every file it catalogs,
            // by construction, so its edges would turn every base into a star and say
            // nothing. The map stays as a node; only its edges are dropped. Same for
            // template folders, whose [[links]] are placeholders per checks.rs.
            let is_map = base.map.as_deref() == Some(file.rel.as_str());
            let mut n = Value::obj();
            n.set("id", Value::Str(format!("{}/{}", name, file.rel)));
            n.set("base", Value::Str(name.clone()));
            n.set("rel", Value::Str(file.rel.clone()));
            n.set("stem", Value::Str(file.stem.clone()));
            n.set("bytes", Value::Num(file.text.len() as f64));
            nodes.push(n);

            if is_map {
                continue;
            }
            for (_, target) in checks::wikilinks(&file.text) {
                let resolved = resolve(state, name, &target);
                let mut e = Value::obj();
                e.set("from", Value::Str(format!("{}/{}", name, file.rel)));
                match resolved {
                    Some(to) => {
                        e.set("to", Value::Str(to));
                        e.set("broken", Value::Bool(false));
                    }
                    None => {
                        e.set("to", Value::Str(format!("{}#{}", name, target)));
                        e.set("broken", Value::Bool(true));
                    }
                }
                edges.push(e);
            }
        }
    }

    let mut out = Value::obj();
    out.set("nodes", Value::Arr(nodes));
    out.set("edges", Value::Arr(edges));
    out
}

fn resolve(state: &State, home: &str, stem: &str) -> Option<String> {
    let hit = |name: &str, base: &Base| -> Option<String> {
        base.files
            .iter()
            .find(|f| f.stem.eq_ignore_ascii_case(stem))
            .map(|f| format!("{}/{}", name, f.rel))
    };
    if let Some((name, base)) = state.bases.iter().find(|(n, _)| n == home) {
        if let Some(found) = hit(name, base) {
            return Some(found);
        }
    }
    for (name, base) in &state.bases {
        if name != home {
            if let Some(found) = hit(name, base) {
                return Some(found);
            }
        }
    }
    None
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
/// tracked-only rule, or nothing is served. Traversal is unrepresentable rather
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

    fn scratch_state() -> State {
        let dir = std::env::temp_dir()
            .join("kb-ui-tests")
            .join(format!("{}", std::process::id()));
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

    /// The security property the whole endpoint rests on: a path that discovery did
    /// not produce is not served, however it is spelled. There is no traversal to
    /// block because there is no path resolution at all.
    #[test]
    fn a_file_outside_the_discovered_set_is_not_served_however_spelled() {
        let state = scratch_state();
        assert!(file_text(&state, "probe", "knowledge/real-note.md").is_some());
        assert!(file_text(&state, "probe", "../../../etc/passwd").is_none());
        assert!(file_text(&state, "probe", "..\\..\\secrets.txt").is_none());
        assert!(file_text(&state, "probe", "C:/Windows/win.ini").is_none());
        assert!(file_text(&state, "nosuch", "knowledge/real-note.md").is_none());
    }

    /// A dangling wikilink is a node the graph shows as broken, not a crash and not
    /// an omission: the graph is E01 made visible.
    #[test]
    fn the_graph_carries_broken_links_flagged_rather_than_dropped() {
        let state = scratch_state();
        let graph = api_graph(&state);
        let edges = match graph.get("edges") {
            Some(Value::Arr(e)) => e,
            _ => panic!("edges"),
        };
        assert_eq!(
            edges.len(),
            1,
            "one wikilink in the scratch base: the note's. The map's structural link              to the note is deliberately not an edge"
        );
        assert_eq!(edges[0].get("broken"), Some(&Value::Bool(true)));
    }

    #[test]
    fn the_fleet_projection_carries_role_and_routability() {
        let state = scratch_state();
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
