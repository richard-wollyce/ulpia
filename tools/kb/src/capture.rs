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
//! - **The deposit**, `inbox/<date>-session-<id>.md` in the base of the agent that owned
//!   the conversation when the question was asked, written by `kb capture` at session end
//!   and then the record is deleted. One session can leave more than one deposit, because
//!   one session can pass through more than one agent.
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

/// One line of the record, in the order it was appended.
///
/// **The order is the ownership**, which the first version of this module threw away. It
/// parsed straight into two vectors, so a session that opened with one agent and closed
/// with another lost the only fact that says which agent each refused question belonged
/// to. The deposit then went to whoever was last, and a question the architect could not
/// answer was filed in the nutritionist's inbox, where nobody who could act on it would
/// look. Found by reading our own source against somebody else's product, 2026-09-02.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Refused { question: String, looked_like: Vec<String> },
    Routed(String),
}

/// What a session's record holds, read back.
#[derive(Debug, Default, PartialEq)]
pub struct Session {
    /// Every line, in file order. The derived views below are folds over this.
    pub events: Vec<Event>,
}

impl Session {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Questions the base could not answer, in order, with what it offered instead.
    pub fn refused(&self) -> Vec<(String, Vec<String>)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Refused { question, looked_like } => {
                    Some((question.clone(), looked_like.clone()))
                }
                Event::Routed(_) => None,
            })
            .collect()
    }

    /// Agents routed to, in order, one entry per switch. A switch is a change, so the
    /// same agent twice in a row is one entry.
    pub fn routed(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in &self.events {
            if let Event::Routed(agent) = e {
                if out.last().map(String::as_str) != Some(agent.as_str()) {
                    out.push(agent.clone());
                }
            }
        }
        out
    }

    /// The refusals each agent owns, in file order, keyed by agent name.
    ///
    /// **The rule, in two clauses: a refusal belongs to whoever was holding the
    /// conversation when it arrived, and to the first agent who takes it if nobody was.**
    ///
    /// A third clause was tried and removed the same hour, and the removal is the useful
    /// part. It said that a routing immediately after a refusal is the same message, on
    /// the reasoning that `boot::brief` records the refusal at boot.rs:187 and the routing
    /// at boot.rs:357. The code order is right and the inference from it is wrong: driving
    /// four messages through `kb boot` showed that a message is routed OR refused and
    /// almost never both, because a `nothing` verdict with no classifier names no owner.
    /// So the record alternates, every refusal is followed by the NEXT message's routing,
    /// and the clause fired every time. It handed a question asked while the architect
    /// held the conversation to the nutritionist, which is the exact defect this function
    /// exists to fix, reintroduced by the fix.
    ///
    /// **What the two clauses get wrong, named rather than hidden:** a message that is
    /// refused AND routed, which happens when a classifier names an owner for a question
    /// the keyword scorer could not reach, is attributed to the previous holder instead of
    /// to the agent the classifier named. Telling those apart needs a message boundary in
    /// the record, which the format does not carry. It is one more field on both writers
    /// and it is not built, because the case needs a configured classifier and the wrong
    /// answer it gives is the previous agent in the same conversation rather than a
    /// stranger.
    ///
    /// A session with refusals and no routing at all yields nothing here, and the caller
    /// keeps the record rather than inventing an owner.
    pub fn by_owner(&self) -> Vec<(String, Vec<(String, Vec<String>)>)> {
        let mut out: Vec<(String, Vec<(String, Vec<String>)>)> = Vec::new();
        for (i, event) in self.events.iter().enumerate() {
            let Event::Refused { question, looked_like } = event else { continue };
            let in_force = || {
                self.events[..i].iter().rev().find_map(|e| match e {
                    Event::Routed(a) => Some(a.clone()),
                    _ => None,
                })
            };
            let taken_later = || {
                self.events[i..].iter().find_map(|e| match e {
                    Event::Routed(a) => Some(a.clone()),
                    _ => None,
                })
            };
            let Some(owner) = in_force().or_else(taken_later) else { continue };
            match out.iter_mut().find(|(a, _)| a == &owner) {
                Some((_, questions)) => questions.push((question.clone(), looked_like.clone())),
                None => out.push((owner, vec![(question.clone(), looked_like.clone())])),
            }
        }
        out
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
                out.events.push(Event::Refused { question, looked_like });
            }
            Some("routed") => {
                if let Some(agent) = parts.next().map(str::trim).filter(|a| !a.is_empty()) {
                    out.events.push(Event::Routed(agent.to_string()));
                }
            }
            _ => {}
        }
    }
    out
}

/// What `capture` did, said rather than implied. A hook nobody watches has to leave a
/// sentence behind or the feature is off and no one remembers switching it off.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The deposit was written here.
    Written(PathBuf),
    /// Nothing was written, and this is why.
    Nothing(String),
}

/// Turns a session's record into one deposit per agent that owned part of it, then
/// deletes the record.
///
/// **One deposit per owner, not one for whoever was last.** A session passes through more
/// than one agent whenever the subject changes, which is the case `boot` exists to
/// handle, and every refusal in it belongs to the agent holding the conversation at the
/// time. See [`Session::by_owner`] for the rule. Filing them all with the last agent put
/// evidence in a base that never saw the question.
///
/// Every deposit is written before the record is deleted, so a crash in the middle leaves
/// a record that captures again rather than a session that vanished. A base that cannot
/// be written is reported and does not stop the others: one missing agent must not cost
/// the whole session.
pub fn write_deposit(root: &Path, session: &str, today: &str) -> Result<Vec<Outcome>, String> {
    let record = read(root, session);
    if record.is_empty() {
        return Ok(vec![Outcome::Nothing("the session produced nothing to capture".into())]);
    }

    let mut owned = record.by_owner();
    if owned.is_empty() {
        // No routing anywhere in the record. The session's own marker is the last
        // fallback, and without that there is no owner and inventing one would put a
        // session's questions in a base that never saw them.
        match crate::boot::last_agent_of(root, session) {
            Some(agent) => owned.push((agent, record.refused())),
            None => {
                return Ok(vec![Outcome::Nothing(
                    "no agent was routed in this session, so the deposit has no owner".into(),
                )]);
            }
        }
    }

    let routed = record.routed();
    let mut out = Vec::new();
    for (agent, questions) in &owned {
        let base = crate::write::agent_root(root, agent);
        if !base.is_dir() {
            out.push(Outcome::Nothing(format!(
                "the routed agent {agent} has no base at {}",
                base.display()
            )));
            continue;
        }
        let inbox = base.join(crate::promote::DEPOSIT);
        if let Err(e) = std::fs::create_dir_all(&inbox) {
            out.push(Outcome::Nothing(format!("cannot create {}: {e}", inbox.display())));
            continue;
        }
        let path = inbox.join(format!("{today}-session-{}.md", safe_session(session)));
        match std::fs::write(&path, render(session, today, questions, &routed)) {
            Ok(()) => out.push(Outcome::Written(path)),
            Err(e) => out.push(Outcome::Nothing(format!("cannot write {}: {e}", path.display()))),
        }
    }

    // Only when at least one deposit landed. A record consumed after writing nothing is a
    // session lost to a directory that did not exist.
    if out.iter().any(|o| matches!(o, Outcome::Written(_))) {
        let _ = std::fs::remove_file(events_file(root, session));
    }
    Ok(out)
}

/// The deposit's text. No `Search for:` line on purpose: this is the short memory, and a
/// keyword line would make the router name a raw session as an answer.
fn render(
    session: &str,
    today: &str,
    refused: &[(String, Vec<String>)],
    routed: &[String],
) -> String {
    let mut out = String::new();
    out.push_str("---\nprovenance: agent\nstage: raw\n---\n\n");
    out.push_str(&format!("# Session {}, {today}\n\n", safe_session(session)));
    out.push_str(
        "What this session produced without a model, written by `kb capture` when it ended. \
         Nothing here has been judged. `kb promote` reads it and decides what, if anything, \
         enters the library.\n\n",
    );

    if !refused.is_empty() {
        out.push_str("## Questions the base could not answer\n\n");
        out.push_str(
            "Each one is a recall loss: either the base does not cover it, or the note that \
             does is missing the words this question used. The vocabulary after each is what \
             the base offered back, spelling only.\n\n",
        );
        for (question, looked_like) in refused {
            out.push_str(&format!("- {question}\n"));
            if !looked_like.is_empty() {
                out.push_str(&format!("  looked like: {}\n", looked_like.join(", ")));
            }
        }
        out.push('\n');
    }

    if !routed.is_empty() {
        // The whole path, in every deposit, because an agent reading its own inbox needs
        // to know the question arrived in a conversation that also belonged to somebody
        // else. That is context, not noise: it is what says whether to promote or to
        // leave the question for the agent who actually covers it.
        out.push_str("## Where the conversation went\n\n");
        out.push_str(&format!("{}\n", routed.join(", then ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second base beside the one `scratch` makes, for the tests about a session that
    /// passed through more than one agent.
    fn make_agent(root: &Path, name: &str) {
        let dir = root.join("fleet").join(name);
        std::fs::create_dir_all(dir.join("knowledge")).expect("mkdir");
        std::fs::write(dir.join("agent.txt"), format!("name = {name}\n")).expect("agent");
    }

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
            got.refused(),
            vec![
                ("qual a taxa de juros".to_string(), vec!["taxa de cambio".to_string()]),
                ("outra".to_string(), vec![]),
            ],
            "a tab inside a question became a space and the line survived"
        );
        assert_eq!(got.routed(), vec!["zed".to_string(), "yaron".to_string()], "same agent twice is one");
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
        assert_eq!(out.len(), 1, "one agent, one deposit: {out:?}");
        let Outcome::Written(path) = out[0].clone() else { panic!("expected a deposit, got {out:?}") };
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
            vec![Outcome::Nothing("the session produced nothing to capture".into())]
        );
    }

    /// A session nobody was routed in has no owner, and a deposit with no owner is a
    /// question filed in a base that never saw it. Said, not guessed.
    /// **The defect this test was written for, found by reading our own source against
    /// somebody else's product.** `read` parsed the append-ordered events file into two
    /// separate vectors and threw the interleaving away, and `write_deposit` took
    /// `routed.last()`. So a session that opened with one agent and closed with another
    /// filed every question the first one refused into the second one's inbox, where
    /// nobody who could act on it would ever look. A deposit in the wrong base is worse
    /// than no deposit: it is evidence filed against a base that never saw the question.
    #[test]
    fn each_refusal_is_deposited_with_the_agent_that_owned_the_conversation() {
        let root = scratch("split");
        make_agent(&root, "yaron");
        // The shape a real session produces, taken from four messages driven through
        // `kb boot` on 2026-09-02: a message is routed OR refused, rarely both, because a
        // `nothing` verdict with no classifier names no owner. So the record alternates,
        // and each refusal belongs to the agent above it.
        note_routed(&root, "s-split", "zed");
        note_refused(&root, "s-split", "como faco rollback", &[]);
        note_routed(&root, "s-split", "yaron");
        note_refused(&root, "s-split", "quanto de proteina por quilo", &[]);

        let out = write_deposit(&root, "s-split", "2026-09-02").expect("captures");
        let written: Vec<&PathBuf> = out
            .iter()
            .filter_map(|o| match o {
                Outcome::Written(p) => Some(p),
                Outcome::Nothing(_) => None,
            })
            .collect();
        assert_eq!(written.len(), 2, "one deposit per owner, not one for the last: {out:?}");

        let zed = std::fs::read_to_string(
            root.join("fleet").join("zed").join("inbox").join("2026-09-02-session-s-split.md"),
        )
        .expect("zed got a deposit");
        let yaron = std::fs::read_to_string(
            root.join("fleet").join("yaron").join("inbox").join("2026-09-02-session-s-split.md"),
        )
        .expect("yaron got a deposit");

        assert!(zed.contains("como faco rollback"), "{zed}");
        assert!(!zed.contains("proteina"), "yaron's question is not zed's: {zed}");
        assert!(yaron.contains("quanto de proteina por quilo"), "{yaron}");
        assert!(!yaron.contains("rollback"), "zed's question is not yaron's: {yaron}");
    }

    /// **The case the two clause rule gets wrong, pinned so nobody discovers it as a
    /// surprise.** A classifier can name an owner for a message the keyword scorer
    /// refused, and then the refusal and the routing are one message. The rule cannot see
    /// that, because the record carries no message boundary, so the question goes to the
    /// agent who was holding the conversation rather than to the one just named. The test
    /// asserts the wrong answer on purpose: it is the documentation of a limit, and the
    /// day the record carries a message number this test is what has to change.
    #[test]
    fn a_refusal_the_classifier_routed_goes_to_the_previous_holder_and_that_is_the_known_limit() {
        let root = scratch("adjacent");
        make_agent(&root, "yaron");
        note_routed(&root, "s-adj", "zed");
        note_refused(&root, "s-adj", "quantos gramas de proteina", &[]);
        note_routed(&root, "s-adj", "yaron");

        let owners = read(&root, "s-adj").by_owner();
        assert_eq!(owners.len(), 1, "one refusal, one owner: {owners:?}");
        assert_eq!(
            owners[0].0, "zed",
            "the agent in force, not the one the classifier named. See `by_owner` for why              telling them apart needs a message boundary the record does not carry"
        );
    }

    /// A session opens with a refusal, and only afterwards does an agent take it. The
    /// question belongs to whoever took the conversation, because that is who can act on
    /// it. `boot::brief` records the refusal before it records the routing for one
    /// message, so this is the ordinary first message of a session and not an edge case.
    #[test]
    fn a_refusal_before_any_routing_belongs_to_the_agent_that_took_the_conversation() {
        let root = scratch("before");
        note_refused(&root, "s-first", "em quanto tempo o investimento volta", &[]);
        note_routed(&root, "s-first", "zed");

        let out = write_deposit(&root, "s-first", "2026-09-02").expect("captures");
        assert_eq!(out.len(), 1, "{out:?}");
        let text = std::fs::read_to_string(
            root.join("fleet").join("zed").join("inbox").join("2026-09-02-session-s-first.md"),
        )
        .expect("zed got it");
        assert!(text.contains("em quanto tempo o investimento volta"), "{text}");
    }

    #[test]
    fn a_session_with_no_routed_agent_is_not_captured_and_says_why() {
        let root = scratch("ownerless");
        note_refused(&root, "s2", "pergunta sem dono", &[]);
        let out = write_deposit(&root, "s2", "2026-09-01").expect("no error, a reason");
        assert!(
            matches!(out.as_slice(), [Outcome::Nothing(why)] if why.contains("no agent was routed")),
            "{out:?}"
        );
        assert!(events_file(&root, "s2").exists(), "and the record is kept for a later owner");
    }

    #[test]
    fn a_session_that_produced_nothing_writes_nothing() {
        let root = scratch("empty");
        assert_eq!(
            write_deposit(&root, "s3", "2026-09-01").expect("fine"),
            vec![Outcome::Nothing("the session produced nothing to capture".into())]
        );
        assert!(!root.join("fleet").join("zed").join("inbox").exists(), "no empty deposit, no empty folder");
    }
}
