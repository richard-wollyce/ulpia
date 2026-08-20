//! The fleet deciding who answers, instead of a file asking the model to decide.
//!
//! **What this replaces.** The public `CLAUDE.md` carried a static conditional: when
//! `fleet/zed/` is here, read its `CLAUDE.md` and follow it instead. That names one agent
//! literally, never reads the question, and puts the choice in the hands of whichever
//! model happened to read the file. Ask it about protein and you still get the architect.
//!
//! Richard's correction, and it is the right one: a model should not read a file to learn
//! who it is. It should be **plugged into the software**, the way an integration is handed
//! its tools after it authenticates, and the software decides how it runs.
//!
//! ## The mechanism, which is the whole point
//!
//! The runtime exposes a `UserPromptSubmit` hook. It runs a command **before the model
//! sees the message**, hands it the prompt as JSON on stdin, and injects whatever the
//! command prints to stdout into the model's context. Confirmed against the hook
//! reference on 2026-08-18: stdout is added as context for `UserPromptSubmit`,
//! `UserPromptExpansion` and `SessionStart` specifically, exit 0 means the context is
//! added, and the timeout for this event is 30 seconds.
//!
//! So the direction of control inverts. The router runs first, unconditionally, and the
//! model receives an identity it did not choose. That is the difference between reading a
//! rule and being handed one, and it is why this lives in a hook rather than in prose.
//!
//! ## Why the constitution is not injected every message
//!
//! It is roughly 55 KB. Emitting it on every turn would spend the context window on
//! repetition and would make the router the most expensive thing in the loop.
//!
//! `UserPromptSubmit` fires per message, so this **emits the constitution only when the
//! routed agent changes**, tracked per session id under `.kb/sessions/`. That is ADR-0020's
//! Option B implemented honestly: Vesta routes, hands off, and leaves, and the only reason
//! it comes back is that the conversation changed domain, which was Option B's named
//! failure mode. Handling it costs one file read.
//!
//! ## What it does when it does not know
//!
//! It says so and emits the roster instead of picking. A router that always picks is the
//! failure ADR-0013 spent a day measuring: silently answering from the wrong base is worse
//! than admitting the question is not covered. The gate from ADR-0020 decides, and below
//! it nothing is handed over as identity.

use std::path::{Path, PathBuf};

use crate::json;
use crate::memory::{Memory, Verdict};

/// What the runtime handed us on stdin.
pub struct Request {
    pub prompt: String,
    pub session: String,
    pub cwd: Option<PathBuf>,
}

/// Parses the hook payload.
///
/// Every field is optional on purpose. A hook that panics on an unexpected payload takes
/// the user's message down with it, and the contract belongs to somebody else's runtime:
/// it can gain fields, rename them, or hand us something else entirely on a version bump.
/// A boot step that fails closed and silent is strictly better than one that fails loud
/// and blocks the conversation.
pub fn parse_request(stdin: &str) -> Option<Request> {
    let v = json::parse(stdin).ok()?;
    let prompt = v.get("prompt").and_then(|p| p.as_str()).unwrap_or("").to_string();
    let session =
        v.get("session_id").and_then(|s| s.as_str()).unwrap_or("unknown").to_string();
    let cwd = v.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from);
    Some(Request { prompt, session, cwd })
}

/// Where the last routed agent for a session is remembered.
///
/// Under `.kb/`, which is already derived, already gitignored, and already declared
/// disposable by ADR-0003. Losing it costs one extra constitution injection, which is the
/// correct blast radius for a cache.
fn session_file(root: &Path, session: &str) -> PathBuf {
    // The session id comes from outside, so it is not allowed to be a path. Anything that
    // is not plainly alphanumeric becomes an underscore, which makes traversal
    // impossible rather than unlikely.
    let safe: String = session
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    root.join(".kb").join("sessions").join(safe)
}

fn last_agent(root: &Path, session: &str) -> Option<String> {
    std::fs::read_to_string(session_file(root, session)).ok().map(|s| s.trim().to_string())
}

fn remember_agent(root: &Path, session: &str, agent: &str) {
    let path = session_file(root, session);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, agent);
}

/// What the model is handed, before it has read anything.
pub struct Briefing {
    /// The agent the fleet chose, if it was confident enough to choose one.
    pub agent: Option<String>,
    /// True when the constitution is included, which happens on the first message of a
    /// session and whenever the routed agent changes under the conversation.
    pub switched: bool,
    pub text: String,
}

/// Routes one message and produces what the runtime should inject.
pub fn brief(memory: &Memory, root: &Path, req: &Request, top: usize) -> Briefing {
    // An empty prompt is a session opening rather than a question. Routing it would rank
    // files against nothing and pick whichever base is largest.
    if req.prompt.trim().is_empty() {
        return Briefing { agent: None, switched: false, text: roster(memory) };
    }

    let answer = memory.ask(&req.prompt, top);

    // **Retrieval is code; choosing who answers is a judgement.** ADR-0013 said so and
    // this is where it finally holds: the deterministic sum is now the fallback, and a
    // model reads the roster and the evidence when one is configured. It sees a dossier
    // of roughly three hundred tokens and never the corpus, so it cannot invent a file,
    // and its answer is a name from a list it was handed.
    let classifier = memory.classifier();
    let roster_names = memory.roster();

    // **The cascade was built, measured and rejected. See `classify::is_uncontested`.**
    // The classifier is consulted on every message, because the one signal a cascade
    // could gate on is the deterministic score, and the deterministic score does not
    // know when it is wrong. That is the whole reason this module exists.
    let verdict = crate::classify::run(
        &classifier,
        root,
        &crate::classify::dossier(memory, &req.prompt, &answer.found, answer.confidence),
        &roster_names,
    )
    .or_else(|| {
        // No classifier, or it could not be reached. The deterministic choice stands,
        // gated by the floor exactly as before, so the fleet keeps routing when a model
        // is unavailable rather than stopping. Which of the two happened is carried
        // forward, because a configured classifier that is not answering is a fault and
        // a fleet without one is not.
        let why = match classifier {
            crate::classify::Classifier::None => crate::classify::FellBack::NotConfigured,
            _ => crate::classify::FellBack::DidNotAnswer,
        };
        match answer.confidence.verdict {
            Verdict::Hit => crate::classify::fallback(answer.agent.clone(), why),
            _ => None,
        }
    });

    // A subject nobody owns is reported, not routed. This is the state arithmetic could
    // never express: a score of zero says "no match" and never says "no one here does
    // this kind of work".
    if let Some(v) = &verdict {
        if let Some(note) = crate::classify::coverage_note(v) {
            let nearest = v
                .owner
                .as_ref()
                .map(|o| format!(
                    "\n\nIf you answer anyway, answer as {o} and say plainly that this is \
                     outside the fleet's covered ground.\n"
                ))
                .unwrap_or_default();
            return Briefing {
                agent: None,
                switched: false,
                text: format!("{note}{nearest}\n{}", roster(memory)),
            };
        }
    }

    // **Say it out loud when a configured classifier is not answering.** The fallback is
    // good enough that nothing looks broken: routing continues, an agent is named, and
    // the only symptom is worse choices. That is how a stale binary ran for a day with
    // the classifier compiled out of it while the surface reported it working. Degraded
    // and silent is the combination that costs the most to find.
    let degraded = verdict
        .as_ref()
        .is_some_and(|v| v.reason == crate::classify::FellBack::DidNotAnswer.reason());

    let chosen = verdict.as_ref().and_then(|v| v.owner.clone());

    let Some(agent) = chosen else {
        return Briefing {
            agent: None,
            switched: false,
            text: format!(
                "VESTA: routed this message and found no owner (top keyword score {:.1}, \
                 floor {:.1}).\n\n\
                 Do not assume an agent. Either ask which one this belongs to, or answer \
                 as the fleet's librarian and say the base does not appear to cover it.\n\n{}",
                answer.confidence.keyword_score,
                crate::memory::SCORE_FLOOR,
                roster(memory)
            ),
        };
    };

    let previous = last_agent(root, &req.session);

    // **Stickiness was built here and removed on 2026-08-19, measured both times.**
    //
    // It existed because "yes, run it and measure" routed to the nutrition agent at
    // 28.44 on the word "out" from a recipes file. The real cause was the stopword
    // list, not the absence of an incumbent: completing the closed classes dropped
    // "ok obrigado", "isso ai", "pode fazer" and "continua" to a score of **zero**,
    // where the confidence floor already handles them and never changes the agent.
    //
    // What the incumbent rule did instead was freeze a wrong answer. Richard's
    // session pinned to the marketing agent and stayed there through four messages
    // about routing and interface design, because a challenger had to double the
    // incumbent to take the conversation. The hook said "still steve" while he asked
    // about the router. **A mechanism that needs the first answer to be right is not
    // a correction mechanism.**
    //
    // Same shape as MIN_MARGIN and the corpus-share normalisation: reasoned into
    // existence, then removed once the instrument had an opinion. The floor decides,
    // and a message with nothing in it changes nothing.

    let switched = previous.as_deref() != Some(agent.as_str());
    remember_agent(root, &req.session, &agent);

    let files: Vec<String> = answer
        .found
        .iter()
        .take(top)
        .map(|f| format!("  {}/{}", f.base, f.path))
        .collect();

    let mut text = String::new();
    if switched {
        let agent_root = memory
            .agents
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(&agent))
            .map(|a| a.root.clone());

        text.push_str(&format!(
            "VESTA: this message routes to {agent}. You are {agent} for as long as that \
             holds. This was decided by the fleet's router before you saw the message, \
             not by a file you read, so it is not yours to override: if it is wrong, say \
             so rather than answering as somebody else.\n\n"
        ));

        if let Some(agent_root) = agent_root {
            if let Some(blocks) = crate::blocks::read(&agent_root) {
                text.push_str(&crate::blocks::assemble(&agent_root, &blocks));
                text.push_str("\n\n");
            }
        }
        if let Some(prev) = previous {
            text.push_str(&format!(
                "(the conversation was with {prev} until this message)\n\n"
            ));
        }
    } else {
        text.push_str(&format!("VESTA: still {agent}.\n\n"));
    }

    if degraded {
        text.push_str(
            "VESTA: the configured classifier did not answer, so this was routed by \
             keyword score alone. Routing is degraded, not stopped. Say so if the choice \
             looks wrong.\n\n",
        );
    }

    if files.is_empty() {
        text.push_str("No file ranked for this message.\n");
    } else {
        text.push_str("Open these before answering:\n");
        text.push_str(&files.join("\n"));
        text.push('\n');
    }

    Briefing { agent: Some(agent), switched, text }
}

fn roster(memory: &Memory) -> String {
    let mut out = String::from("The fleet:\n");
    for a in &memory.agents {
        out.push_str(&format!("  {}\n", a.name));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_and_session_come_off_the_hook_payload() {
        let req = parse_request(
            r#"{"session_id":"abc-123","prompt":"quanto de proteina","cwd":"C:/x","hook_event_name":"UserPromptSubmit"}"#,
        )
        .expect("parses");
        assert_eq!(req.prompt, "quanto de proteina");
        assert_eq!(req.session, "abc-123");
    }

    /// The runtime's payload belongs to somebody else and can change under us. Missing
    /// fields must degrade, never panic, because a panicking hook takes the user's
    /// message with it.
    #[test]
    fn a_payload_missing_everything_still_parses() {
        let req = parse_request("{}").expect("parses");
        assert_eq!(req.prompt, "");
        assert_eq!(req.session, "unknown");
        assert!(req.cwd.is_none());
    }

    #[test]
    fn a_payload_that_is_not_json_is_not_a_panic() {
        assert!(parse_request("not json at all").is_none());
    }

    /// The session id arrives from outside the program and is used to build a path.
    #[test]
    fn a_session_id_cannot_escape_the_sessions_directory() {
        let root = Path::new("C:/fleet");
        let evil = session_file(root, "../../../../etc/passwd");
        assert!(evil.starts_with("C:/fleet/.kb/sessions"), "stays inside: {}", evil.display());
        assert!(!evil.to_string_lossy().contains(".."));
    }

    #[test]
    fn the_same_agent_twice_in_a_row_is_not_a_switch() {
        let dir = std::env::temp_dir().join(format!("kb-boot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");

        assert_eq!(last_agent(&dir, "s1"), None, "a fresh session remembers nothing");
        remember_agent(&dir, "s1", "zed");
        assert_eq!(last_agent(&dir, "s1").as_deref(), Some("zed"));
        remember_agent(&dir, "s1", "yaron");
        assert_eq!(last_agent(&dir, "s1").as_deref(), Some("yaron"), "a switch is recorded");
    }
}
