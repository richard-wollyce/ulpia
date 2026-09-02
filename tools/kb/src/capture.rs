//! What a session leaves in the short memory when it ends. ADR-0035.
//!
//! [ADR-0030](../../../decisions/0030-two-promoters-and-the-second-is-not-a-second-opinion.md)
//! built the filter between the deposit and the library and then said, in as many words,
//! that nothing writes into the deposit: files arrive by hand. This is the half it named
//! as unbuilt, in its cheapest honest shape: **no model, only what the session already
//! measured.** Every line the deposit receives is something `kb` itself produced during the
//! session, so the junk rate that ADR-0030 leaves unmeasured can be read off `promote`'s
//! output over real deposits before anything that generates candidates is switched on.
//!
//! Two files, and the split is the design:
//!
//! - **The session record**, `.kb/sessions/<id>.events`, appended to by `kb boot` on every
//!   message: the questions the base refused and the agents it routed to. Under `.kb/`,
//!   which is derived and disposable by ADR-0003, because it is a working file and losing
//!   it costs one session's capture. One writer per session, sequential, because a session
//!   is one conversation and the hook runs once per message; the miss log is the file two
//!   sessions share, and that one holds a marker while it merges.
//! - **The deposit**, `inbox/<date>-session-<id>.md` in the last routed agent's base, written
//!   once by `kb capture` at session end from the record, and then the record is deleted.
//!   Markdown, `provenance: agent`, `stage: raw`, no `Search for:` line: the router never
//!   names it, the text scorer reaches it, and every passage from it is labelled short
//!   memory at every surface. `kb promote` reads it and decides.
//!
//! What is deliberately absent, and named so it is not mistaken for forgotten: the model's
//! own outputs, the person's prose, and `kb remember` proposals. The first two need a model
//! to read the transcript, which is ADR-0035's option B and waits for a junk rate. The third
//! needs `remember` to know which session it is in, which is a flag it does not have yet.

use std::path::{Path, PathBuf};

use crate::boot::safe_session;

/// The file a session appends to while it runs.
pub fn events_file(root: &Path, session: &str) -> PathBuf {
    root.join(".kb").join("sessions").join(format!("{}.events", safe_session(session)))
}

fn append(root: &Path, session: &str, line: &str) {
    let path = events_file(root, session);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Best effort, like the session's agent marker beside it: a boot hook that fails the
    // user's message over a bookkeeping file is worse than a session that captures less.
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{line}");
    }
}

/// One field per tab, so the line survives a question with commas in it. A tab inside a
/// question becomes a space, which loses nothing a reader needs.
fn field(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

/// A question the base refused, with the vocabulary it offered back.
pub fn note_refused(root: &Path, session: &str, question: &str, looked_like: &[String]) {
    append(root, session, &format!("refused\t{}\t{}", field(question), field(&looked_like.join(", "))));
}

/// An agent the session was routed to. Recorded on every switch, so the deposit can say
/// where the conversation went and land with whoever had it last.
pub fn note_routed(root: &Path, session: &str, agent: &str) {
    append(root, session, &format!("routed\t{}", field(agent)));
}

/// What a session's record holds, read back.
#[derive(Debug, Default, PartialEq)]
pub struct Session {
    /// Questions the base could not answer, in order, with what it offered instead.
    pub refused: Vec<(String, Vec<String>)>,
    /// Agents routed to, in order, one entry per switch.
    pub routed: Vec<String>,
}

impl Session {
    pub fn is_empty(&self) -> bool {
        self.refused.is_empty() && self.routed.is_empty()
    }
}

pub fn read(root: &Path, session: &str) -> Session {
    let text = std::fs::read_to_string(events_file(root, session)).unwrap_or_default();
    let mut out = Session::default();
    for line in text.lines() {
        let mut parts = line.split('\t');
        match parts.next() {
            Some("refused") => {
                let question = parts.next().unwrap_or("").trim().to_string();
                if question.is_empty() {
                    continue;
                }
                let looked_like: Vec<String> = parts
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(|w| w.trim().to_string())
                    .filter(|w| !w.is_empty())
                    .collect();
                out.refused.push((question, looked_like));
            }
            Some("routed") => {
                if let Some(agent) = parts.next().map(str::trim).filter(|a| !a.is_empty()) {
                    // A switch is a change. The same agent twice in a row is one entry.
                    if out.routed.last().map(String::as_str) != Some(agent) {
                        out.routed.push(agent.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// What `capture` did, said rather than implied. A hook nobody watches has to leave a
/// sentence behind or the feature is off and no one remembers switching it off.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// The deposit was written here.
    Written(PathBuf),
    /// Nothing was written, and this is why.
    Nothing(String),
}

/// Turns a session's record into a deposit in the last routed agent's inbox, then
/// deletes the record. The deposit is written first, so a crash between the two leaves a
/// record that captures again rather than a session that vanished.
pub fn write_deposit(root: &Path, session: &str, today: &str) -> Result<Outcome, String> {
    let record = read(root, session);
    if record.is_empty() {
        return Ok(Outcome::Nothing("the session produced nothing to capture".into()));
    }

    // Whoever had the conversation last owns what it left. Without an owner there is no
    // deposit to write into, and inventing one would put a session's questions in a base
    // that never saw them.
    let owner = record
        .routed
        .last()
        .cloned()
        .or_else(|| crate::boot::last_agent_of(root, session));
    let Some(agent) = owner else {
        return Ok(Outcome::Nothing(
            "no agent was routed in this session, so the deposit has no owner".into(),
        ));
    };

    let base = crate::write::agent_root(root, &agent);
    if !base.is_dir() {
        return Err(format!("the routed agent {agent} has no base at {}", base.display()));
    }
    let inbox = base.join(crate::promote::DEPOSIT);
    std::fs::create_dir_all(&inbox).map_err(|e| format!("cannot create {}: {e}", inbox.display()))?;
    let path = inbox.join(format!("{today}-session-{}.md", safe_session(session)));

    std::fs::write(&path, render(session, today, &record))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let _ = std::fs::remove_file(events_file(root, session));
    Ok(Outcome::Written(path))
}

/// The deposit's text. No `Search for:` line on purpose: this is the short memory, and a
/// keyword line would make the router name a raw session as an answer.
fn render(session: &str, today: &str, record: &Session) -> String {
    let mut out = String::new();
    out.push_str("---\nprovenance: agent\nstage: raw\n---\n\n");
    out.push_str(&format!("# Session {}, {today}\n\n", safe_session(session)));
    out.push_str(
        "What this session produced without a model, written by `kb capture` when it ended. \
         Nothing here has been judged. `kb promote` reads it and decides what, if anything, \
         enters the library.\n\n",
    );

    if !record.refused.is_empty() {
        out.push_str("## Questions the base could not answer\n\n");
        out.push_str(
            "Each one is a recall loss: either the base does not cover it, or the note that \
             does is missing the words this question used. The vocabulary after each is what \
             the base offered back, spelling only.\n\n",
        );
        for (question, looked_like) in &record.refused {
            out.push_str(&format!("- {question}\n"));
            if !looked_like.is_empty() {
                out.push_str(&format!("  looked like: {}\n", looked_like.join(", ")));
            }
        }
        out.push('\n');
    }

    if !record.routed.is_empty() {
        out.push_str("## Where the conversation went\n\n");
        out.push_str(&format!("{}\n", record.routed.join(", then ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-capture-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fleet").join("zed").join("knowledge")).expect("scratch");
        std::fs::write(dir.join("fleet").join("zed").join("agent.txt"), "name = Zed\n").expect("agent");
        dir
    }

    #[test]
    fn a_session_record_round_trips_and_collapses_repeated_routes() {
        let root = scratch("roundtrip");
        note_routed(&root, "s1", "zed");
        note_refused(&root, "s1", "qual a taxa\tde juros", &["taxa de cambio".into()]);
        note_routed(&root, "s1", "zed");
        note_routed(&root, "s1", "yaron");
        note_refused(&root, "s1", "outra", &[]);

        let got = read(&root, "s1");
        assert_eq!(
            got.refused,
            vec![
                ("qual a taxa de juros".to_string(), vec!["taxa de cambio".to_string()]),
                ("outra".to_string(), vec![]),
            ],
            "a tab inside a question became a space and the line survived"
        );
        assert_eq!(got.routed, vec!["zed".to_string(), "yaron".to_string()], "same agent twice is one");
    }

    /// The whole path: a session refuses two questions and was routed to zed, the
    /// deposit lands in zed's inbox with both, raw and unkeyed, and the record is gone
    /// so a second capture finds nothing.
    #[test]
    fn a_session_becomes_a_deposit_in_the_last_routed_agents_inbox_once() {
        let root = scratch("deposit");
        note_routed(&root, "abc-123", "zed");
        note_refused(&root, "abc-123", "em quanto tempo o investimento volta", &["voltar atras".into()]);
        note_refused(&root, "abc-123", "qual o cpf do cliente", &[]);

        let out = write_deposit(&root, "abc-123", "2026-09-01").expect("captures");
        let Outcome::Written(path) = out else { panic!("expected a deposit, got {out:?}") };
        assert_eq!(path, root.join("fleet").join("zed").join("inbox").join("2026-09-01-session-abc-123.md"));

        let text = std::fs::read_to_string(&path).expect("written");
        assert!(text.starts_with("---\nprovenance: agent\nstage: raw\n---\n"), "{text}");
        assert!(text.contains("- em quanto tempo o investimento volta\n  looked like: voltar atras"), "{text}");
        assert!(text.contains("- qual o cpf do cliente"), "{text}");
        assert!(!text.contains("Search for"), "the short memory carries no keys: {text}");
        assert!(text.contains("## Where the conversation went\n\nzed"), "{text}");

        assert!(!events_file(&root, "abc-123").exists(), "the record is consumed");
        assert_eq!(
            write_deposit(&root, "abc-123", "2026-09-01").expect("second run"),
            Outcome::Nothing("the session produced nothing to capture".into())
        );
    }

    /// A session nobody was routed in has no owner, and a deposit with no owner is a
    /// question filed in a base that never saw it. Said, not guessed.
    #[test]
    fn a_session_with_no_routed_agent_is_not_captured_and_says_why() {
        let root = scratch("ownerless");
        note_refused(&root, "s2", "pergunta sem dono", &[]);
        let out = write_deposit(&root, "s2", "2026-09-01").expect("no error, a reason");
        assert!(matches!(out, Outcome::Nothing(ref why) if why.contains("no agent was routed")), "{out:?}");
        assert!(events_file(&root, "s2").exists(), "and the record is kept for a later owner");
    }

    #[test]
    fn a_session_that_produced_nothing_writes_nothing() {
        let root = scratch("empty");
        assert_eq!(
            write_deposit(&root, "s3", "2026-09-01").expect("fine"),
            Outcome::Nothing("the session produced nothing to capture".into())
        );
        assert!(!root.join("fleet").join("zed").join("inbox").exists(), "no empty deposit, no empty folder");
    }
}
