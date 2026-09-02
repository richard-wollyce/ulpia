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
    root.join(".kb").join("sessions").join(safe_session(session))
}

/// The session id, made safe to be a file name.
///
/// The id comes from outside, so it is not allowed to be a path. Anything that is not
/// plainly alphanumeric becomes an underscore, which makes traversal impossible rather
/// than unlikely. Public because `capture` names its files by the same id and must not
/// grow a second opinion about what is safe.
pub fn safe_session(session: &str) -> String {
    session
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

pub fn last_agent_of(root: &Path, session: &str) -> Option<String> {
    last_agent(root, session)
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

/// Removes one kind of envelope, `open` to `close`, wherever it appears.
///
/// An unterminated block is treated as running to the end of the text: a truncated
/// envelope is still an envelope, and the alternative is to keep half a machine message
/// and score it as if a person had typed it.
fn strip_blocks(text: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(i) = rest.find(open) else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..i]);
        let after = &rest[i + open.len()..];
        let Some(j) = after.find(close) else { return out };
        rest = &after[j + close.len()..];
    }
}

/// The runtime's own envelopes, taken out before the prompt is judged to be a question.
///
/// **Stripping, not matching.** A substring test for the notification marker would also
/// silence a real message that quotes a notification in order to ask about it, which is
/// how this defect was reported in the first place. What survives the envelopes is what
/// the person actually typed, and if nothing survives then nothing was asked.
pub fn without_machine_blocks(prompt: &str) -> String {
    let s = strip_blocks(prompt, "<system-reminder>", "</system-reminder>");
    strip_blocks(&s, "<task-notification>", "</task-notification>")
}

/// Routes one message and produces what the runtime should inject.
pub fn brief(memory: &Memory, root: &Path, req: &Request, top: usize) -> Briefing {
    // An empty prompt is a session opening rather than a question. Routing it would rank
    // files against nothing and pick whichever base is largest.
    if req.prompt.trim().is_empty() {
        return Briefing { agent: None, switched: false, text: roster(memory) };
    }

    // **Machine text is not a question.** The runtime submits background task
    // notifications on this same hook, in the same field, so without this the router
    // ranked files against "a background command has completed" and paid a classifier
    // subprocess to decide who owns it. Observed over three consecutive notifications it
    // answered zed, then zed with a caveat, then nobody: three different answers to a
    // question nobody asked.
    //
    // **Silence, not the roster.** The last routed agent's constitution is still in the
    // conversation, so emitting nothing leaves whoever was working still working, which
    // is the only correct response to a notification. It also leaves the session's
    // remembered agent untouched, because nothing below this line runs. The roster above
    // is for a session opening, which is a different event with a different right answer.
    let asked = without_machine_blocks(&req.prompt);
    if asked.trim().is_empty() {
        return Briefing { agent: None, switched: false, text: String::new() };
    }

    // Everything downstream scores `asked` rather than the raw prompt, so an envelope
    // appended to a real question cannot contribute its vocabulary to the ranking.
    let answer = memory.ask(&asked, top);

    // **This surface counted nothing, and it is the one every message passes through.**
    // Six other surfaces asked the contract whether the question was a recall loss;
    // this one asked `ask` and moved on, so the most frequent refusal there is entered
    // no log. It was deferred until the log could take two writers at once, because a
    // hook runs under the concurrency ADR-0021 describes; `misses::record` now holds a
    // marker while it merges. The loss also goes into the session's own record, which
    // is what `kb capture` turns into a deposit at session end. ADR-0035.
    if let Some(loss) = memory.recall_loss(&asked, &answer.confidence) {
        crate::capture::note_refused(root, &req.session, &loss.question, &loss.looked_like);
    }

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
        &crate::classify::dossier(memory, &asked, &answer.found, answer.confidence),
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
        // **No agent owns it, and that has two meanings the fleet used to run together.**
        //
        // A base with no `agent.txt` is read by the router and can never be chosen as the
        // one who answers. Until now, evidence landing there produced the same sentence as
        // evidence landing nowhere: *the base does not appear to cover it*. Those are
        // different states. `general/` holds what the fleet knows without any agent owning
        // it, and `person/` holds the human; when the answer is in one of them it is not a
        // gap, it is Vesta's own.
        //
        // Richard's framing, and it is why this branch exists: *deveria existir uma base de
        // conhecimento geral, que e a base em que a propria Vesta responde por conta propria
        // ao inves de rotear para algum agente.*
        let routable: Vec<&str> = memory
            .agents
            .iter()
            .filter(|a| a.routable)
            .map(|a| a.name.as_str())
            .collect();
        let mine: Vec<String> = answer
            .found
            .iter()
            .filter(|f| !routable.iter().any(|r| r.eq_ignore_ascii_case(&f.base)))
            .take(top)
            .map(|f| format!("  {}/{}", f.base, f.path))
            .collect();

        if !mine.is_empty() && answer.confidence.verdict != Verdict::Nothing {
            return Briefing {
                agent: None,
                switched: false,
                text: format!(
                    "VESTA: no agent owns this and it does not need one. The library holds \
                     it in a base that everyone reads and nobody answers for, so this one \
                     is mine.\n\n\
                     Answer as the fleet's librarian, from these, and say plainly if they \
                     do not actually cover the question:\n{}\n",
                    mine.join("\n")
                ),
            };
        }

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
    // Into the session's record as well, so the deposit `kb capture` writes at session
    // end lands with whoever had the conversation last and can say where it went.
    crate::capture::note_routed(root, &req.session, &agent);

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

/// The names that could actually answer, which is not every base that was opened.
///
/// **It listed every base, including the ones that can never be chosen.** This is printed
/// when routing found no owner, precisely so the reader can pick one, and it was offering
/// `person` and `general`: two bases with no `agent.txt`, read by everyone and answering for
/// nobody. `Memory::roster`, the list the classifier may choose from, has always filtered;
/// only the sentence a person reads did not.
///
/// A menu that lists dishes the kitchen does not make is worse than a short menu.
fn roster(memory: &Memory) -> String {
    let mut out = String::from("The fleet:\n");
    for a in memory.agents.iter().filter(|a| a.routable) {
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

    /// The defect this guard exists for: the runtime submits background task
    /// notifications on the same hook and in the same field as a question, so the router
    /// was ranking files against machine text and paying a classifier subprocess to
    /// decide who owns it.
    #[test]
    fn a_notification_leaves_nothing_to_ask() {
        let notification = "<system-reminder>\n[SYSTEM NOTIFICATION - NOT USER INPUT]\n\
             <task-notification>\n<task-id>abc</task-id>\n<status>completed</status>\n\
             </task-notification>\n</system-reminder>";
        assert!(without_machine_blocks(notification).trim().is_empty());
    }

    /// Why this strips rather than matching a marker. A person quoting a notification in
    /// order to ask about it is asking a question, and a substring test would have
    /// silenced the very message that reported this defect.
    #[test]
    fn a_question_that_quotes_a_notification_survives() {
        let asked = without_machine_blocks(
            "<system-reminder>[SYSTEM NOTIFICATION - NOT USER INPUT]</system-reminder>\n\
             porque o roteador recebe isso?",
        );
        assert!(!asked.trim().is_empty());
        assert!(asked.contains("porque o roteador recebe isso?"));
        assert!(!asked.contains("SYSTEM NOTIFICATION"));
    }

    /// An envelope appended to a real question must not lend its vocabulary to the
    /// ranking. The question is what gets scored, and nothing else.
    #[test]
    fn an_envelope_never_reaches_the_ranking() {
        let asked = without_machine_blocks(
            "quanto de proteina<system-reminder>cwd, git status, background task\
             </system-reminder>",
        );
        assert_eq!(asked.trim(), "quanto de proteina");
    }

    /// A truncated envelope is still an envelope. Keeping the half that arrived would
    /// score machine text as though a person had typed it.
    #[test]
    fn an_unterminated_envelope_is_dropped_to_the_end() {
        assert_eq!(without_machine_blocks("<system-reminder>cortado ao meio").trim(), "");
    }

    /// The two events are different and have different right answers: a session opening
    /// gets the roster so the model knows who exists, a notification gets silence so
    /// whoever was already working stays working.
    #[test]
    fn a_notification_is_not_the_same_event_as_an_empty_prompt() {
        assert!(without_machine_blocks("").trim().is_empty());
        assert!(without_machine_blocks("<task-notification>x</task-notification>")
            .trim()
            .is_empty());
    }

    /// The session id arrives from outside the program and is used to build a path.
    #[test]
    fn a_session_id_cannot_escape_the_sessions_directory() {
        let root = Path::new("C:/fleet");
        let evil = session_file(root, "../../../../etc/passwd");
        assert!(evil.starts_with("C:/fleet/.kb/sessions"), "stays inside: {}", evil.display());
        assert!(!evil.to_string_lossy().contains(".."));
    }

    /// **The surface every message passes through counted no recall loss.** ADR-0035.
    /// A refused question through `brief` now lands in the miss log like every other
    /// surface, and in the session's own record, which is what becomes the deposit.
    #[test]
    fn a_refused_message_is_counted_and_goes_into_the_sessions_record() {
        let root = std::env::temp_dir().join(format!("kb-boot-loss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let agent = root.join("fleet").join("probe");
        std::fs::create_dir_all(agent.join("knowledge")).expect("scratch");
        std::fs::write(agent.join("agent.txt"), "name = Probe\nrole = testing\n").expect("agent");
        std::fs::write(
            agent.join("knowledge").join("zebra.md"),
            "# Zebra\n\n**Search for:** `zebra`, `quagga`\n\n**Exists to:** hold one animal\n",
        )
        .expect("note");
        let memory = Memory::open(&[root.as_path()], true).expect("opens");

        let req = Request {
            prompt: "qual a taxa de juros do trimestre".into(),
            session: "s-loss".into(),
            cwd: None,
        };
        let _ = brief(&memory, &root, &req, 5);

        let log = std::fs::read_to_string(crate::misses::path_in(&root)).expect("the loss was counted");
        assert!(log.contains("qual a taxa de juros do trimestre"), "{log}");

        let record = crate::capture::read(&root, "s-loss");
        assert_eq!(record.refused.len(), 1, "and it is in the session's record: {record:?}");
        assert_eq!(record.refused[0].0, "qual a taxa de juros do trimestre");
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
