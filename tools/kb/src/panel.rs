//! The objection round, promoted from one agent's document to a verb any agent can run.
//!
//! # What this is a promotion of
//!
//! `running-the-objection-round.md` in the scriptwriter's base describes a review that
//! works: **one owner accountable for the piece, a panel that returns named objections
//! rather than rewrites, the panel chosen by what the piece claims, one blocking objection
//! per reviewer that the owner may not refuse, and every refusal written down.** It was
//! written for one agent and one artifact, a video script, and it is not about either.
//!
//! Its own text says what it runs on: *the mechanism this fleet already has, rather than
//! the version that would need a coordinator agent that does not exist.* That constraint
//! survives here. **Nothing in this module coordinates anything.** It assembles what a
//! session needs to run the round, and it keeps the accounting the round produces. The
//! session drives; a subagent per reviewer is the fleet's existing mechanism and this
//! writes down which ones to boot, what each costs, and what each said.
//!
//! # Why a verb and not a longer document
//!
//! Three things a document cannot do, and each one is a failure that happened.
//!
//! **A cost table in prose goes stale.** The skill's table was measured on 2026-09-03 and
//! two of its five numbers were already wrong on 2026-09-04, because two agents gained
//! files. [`cost`] reads `blocks.txt` off disk through the same [`crate::blocks::Block::tokens`]
//! that `kb blocks` prints, so the two cannot disagree. It also counts the artifact, which
//! the prose table did not: every reviewer reads the piece as well as its own constitution,
//! and the honest number is the sum.
//!
//! **A ledger kept in a conversation dies with the conversation.** The whole correction to
//! single ownership is that a refusal is auditable after the piece underperforms. That
//! requires the refusal to outlive the session, which is the same argument
//! [`crate::misroute`] makes about a misroute the agent noticed and nothing kept.
//!
//! **"Did not answer" and "found nothing" are different facts and collapse into each other
//! by default.** Section 5b of the skill was written the day the protocol first stalled on
//! a reviewer who never returned. Here they are two states that no code path merges, so a
//! silent reviewer can never be read as an endorsement.
//!
//! # What it refuses to do
//!
//! It calls no model, spawns no reviewer and schedules nothing. It has no opinion on
//! whether an objection is good. It cannot close a round that has a blocking objection in
//! it, and that refusal is the one piece of judgement the owner does not hold.

use std::path::{Path, PathBuf};

use crate::blocks;

pub const ROUNDS_TXT: &str = "kb-rounds.txt";

const HEADER: &str = "\
# Objection rounds: who was asked about which artifact, and what came back.
#
# One row per fact. `seq 0` is the reviewer itself, and its state says whether that
# reviewer has answered at all. `seq 1` and up are that reviewer's objections, each
# carrying whether it was taken, refused or escalated, and why.
#
# States:
#   asked         booted, no answer yet. NOT the same as `nothing`, and nothing here
#                 merges them: a reviewer who never came back is not an endorsement
#   nothing       answered, and found nothing wrong from inside its own domain
#   not-returned  asked, deadline passed, decided without it. The reason is on the row
#   objection     raised, not yet accounted for
#   taken         the owner changed the piece
#   refused       the owner did not, and the reason is on the row. This is the price of
#                 single ownership and it is paid in writing
#   escalated     a blocking objection, which the owner may not refuse. It goes to the
#                 person, not to the owner's judgement
#
# Written by `kb panel`. Evidence, never action: nothing reads this file and edits a
# piece, an agent or a base.
#
# Not committed anywhere. A round names real work in progress.
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Asked,
    Nothing,
    NotReturned,
    Objection,
    Taken,
    Refused,
    Escalated,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Asked => "asked",
            State::Nothing => "nothing",
            State::NotReturned => "not-returned",
            State::Objection => "objection",
            State::Taken => "taken",
            State::Refused => "refused",
            State::Escalated => "escalated",
        }
    }

    fn parse(s: &str) -> Option<State> {
        Some(match s {
            "asked" => State::Asked,
            "nothing" => State::Nothing,
            "not-returned" => State::NotReturned,
            "objection" => State::Objection,
            "taken" => State::Taken,
            "refused" => State::Refused,
            "escalated" => State::Escalated,
            _ => return None,
        })
    }
}

/// How an owner accounted for one objection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Taken,
    Refused,
    Escalated,
}

/// What a reviewer came back with, or did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The strongest thing wrong with the piece from inside this reviewer's domain.
    Objection { text: String, blocking: bool },
    /// Read it, found nothing. On the record, because a reviewer who never objects is a
    /// reviewer who is not being read.
    Nothing,
    /// Did not come back inside the window. `why` carries what was decided instead.
    NotReturned { why: String },
}

/// One row of `kb-rounds.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub artifact: String,
    pub owner: String,
    pub reviewer: String,
    /// 0 is the reviewer's own status; 1 and up are its objections, in the order raised.
    pub seq: u32,
    pub state: State,
    pub opened: String,
    pub updated: String,
    pub blocking: bool,
    pub text: String,
    pub why: String,
}

impl Row {
    fn key(&self) -> (String, String, u32) {
        (self.artifact.clone(), self.reviewer.clone(), self.seq)
    }
}

#[derive(Debug)]
pub enum Error {
    /// A reviewer with no `blocks.txt` cannot be booted as itself, and a subagent handed
    /// no constitution answers as the base model wearing a name. The whole value of the
    /// protocol is that an objection comes from inside a domain, so this refuses rather
    /// than degrading into a panel of one model talking to itself.
    CannotBoot(String, PathBuf),
    NoSuchAgent(String),
    OwnerOnPanel(String),
    NoReviewers,
    NoRound(String),
    NoSuchObjection(String, u32),
    RefusingToRefuseBlocking(u32),
    /// Blocking is bounded to one per reviewer so that blocking stays expensive enough to
    /// mean something. A reviewer who can block everything has a veto, and a veto is not
    /// what the owner agreed to when they took accountability for the piece.
    AlreadyBlockedOnce(String, u32),
    AlreadyAccounted(u32, State),
    NotOnPanel(String, String),
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CannotBoot(agent, root) => write!(
                f,
                "{agent} has no blocks.txt at {}, so it cannot be booted as itself. A \
                 subagent handed no constitution answers as the base model wearing a \
                 name, which is a reviewer in costume. Write blocks.txt for {agent}, or \
                 leave it off the panel.",
                root.display()
            ),
            Error::NoSuchAgent(a) => write!(
                f,
                "no agent named '{a}' in this fleet. `kb fleet` lists who is here, with \
                 what each one owns and where each one stops."
            ),
            Error::OwnerOnPanel(a) => write!(
                f,
                "{a} owns this piece and cannot review it. An objection from the owner is \
                 a revision, not an objection, and putting it in the ledger would make \
                 the round look reviewed by somebody who was never outside it."
            ),
            Error::NoReviewers => write!(
                f,
                "a round needs at least one reviewer. Run without --reviewer to see who \
                 the artifact's own words reach and what each one would cost."
            ),
            Error::NoRound(a) => write!(
                f,
                "no round is open on '{a}'. Open one with --owner and --reviewer before \
                 recording an answer against it."
            ),
            Error::NoSuchObjection(a, n) => write!(
                f,
                "'{a}' has no objection {n}. `kb panel {a} --ledger` numbers them."
            ),
            Error::RefusingToRefuseBlocking(n) => write!(
                f,
                "objection {n} was marked blocking, and a blocking objection cannot be \
                 refused by the owner. That is the entire correction to the cost of \
                 single ownership: it is bounded to one per reviewer so that blocking \
                 stays expensive, and in exchange it leaves the owner's judgement. Take \
                 it, or escalate it to the person with --escalated."
            ),
            Error::AlreadyBlockedOnce(agent, n) => write!(
                f,
                "{agent} already marked objection {n} blocking, and blocking is bounded to \
                 one per reviewer. That bound is what keeps blocking expensive enough to \
                 mean something: a reviewer who can block everything holds a veto, which \
                 is not what the owner agreed to. Raise this one as an ordinary objection, \
                 or say which of the two is the one that cannot be refused."
            ),
            Error::AlreadyAccounted(n, s) => write!(
                f,
                "objection {n} is already {}. The ledger is append-once per objection so \
                 that a refusal cannot be quietly rewritten into a taken after the piece \
                 underperformed.",
                s.as_str()
            ),
            Error::NotOnPanel(agent, artifact) => write!(
                f,
                "{agent} is not on the panel for '{artifact}'. An answer from an agent \
                 nobody asked is not a review; add it to the panel first, which costs its \
                 constitution and says so."
            ),
            Error::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(ROUNDS_TXT)
}

/// The artifact path as a stable key.
///
/// A round is keyed by the path a person typed, and a person types `./site/index.html` on
/// one line and `site\index.html` on the next. Without this they are two rounds on one
/// file and the ledger silently splits.
pub fn key_of(artifact: &str) -> String {
    let s = artifact.trim().replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// One field, flattened so a tab or a newline cannot forge a second row.
///
/// Objection text is written by a model reporting what another model said, so this is the
/// boundary where a crafted objection would otherwise be able to write rows nobody raised.
/// Same reasoning, same treatment, as [`crate::misroute`].
fn field(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

pub fn load(log: &Path) -> Vec<Row> {
    let text = match std::fs::read_to_string(log) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 8 {
            continue;
        }
        let Some(state) = State::parse(p[4]) else { continue };
        out.push(Row {
            artifact: p[0].to_string(),
            owner: p[1].to_string(),
            reviewer: p[2].to_string(),
            seq: p[3].trim().parse().unwrap_or(0),
            state,
            opened: p[5].to_string(),
            updated: p[6].to_string(),
            blocking: p[7].trim() == "blocking",
            text: p.get(8).unwrap_or(&"").to_string(),
            why: p.get(9).unwrap_or(&"").to_string(),
        });
    }
    out
}

fn save(log: &Path, rows: &[Row]) -> Result<(), Error> {
    let mut out = String::from(HEADER);
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.artifact,
            r.owner,
            r.reviewer,
            r.seq,
            r.state.as_str(),
            r.opened,
            r.updated,
            if r.blocking { "blocking" } else { "-" },
            r.text,
            r.why
        ));
    }
    std::fs::write(log, out).map_err(|e| Error::Io(log.to_path_buf(), e))
}

// ---------------------------------------------------------------------------
// Booting the panel
// ---------------------------------------------------------------------------

/// One reviewer, assembled and priced.
pub struct Booted {
    pub agent: String,
    /// The resident constitution, written where a subagent can be pointed at it.
    pub path: PathBuf,
    /// What that constitution costs before the reviewer has read a line of the piece.
    pub tokens: usize,
}

/// What a round costs, counted the way it is actually paid.
pub struct Cost {
    pub boot: usize,
    /// The artifact, once per reviewer, because every one of them reads it.
    pub reading: usize,
    pub reviewers: usize,
}

impl Cost {
    pub fn total(&self) -> usize {
        self.boot + self.reading
    }
}

/// Assembles each reviewer's resident constitution and writes it where a subagent can be
/// handed it.
///
/// **`--emit` by another name, and deliberately so.** `kb blocks <base> --emit` already
/// assembles exactly what an agent boots with, deterministically. Using the router here
/// instead would be the wrong instrument: the router elects an owner from a question, and
/// in a review the panel is already known, so election can only get it wrong. This calls
/// the same [`crate::blocks::assemble`] rather than growing a second path to the same
/// text.
pub fn boot(agent: &str, agent_root: &Path, out_dir: &Path) -> Result<Booted, Error> {
    let Some(read) = blocks::read(agent_root) else {
        return Err(Error::CannotBoot(agent.to_string(), agent_root.to_path_buf()));
    };

    let text = blocks::assemble(agent_root, &read);
    let tokens: usize = read
        .iter()
        .filter(|b| b.mode == blocks::Mode::Resident)
        .map(|b| b.tokens())
        .sum();

    std::fs::create_dir_all(out_dir).map_err(|e| Error::Io(out_dir.to_path_buf(), e))?;
    let path = out_dir.join(format!("{}-constitution.txt", agent.to_lowercase()));
    std::fs::write(&path, text).map_err(|e| Error::Io(path.clone(), e))?;

    Ok(Booted { agent: agent.to_string(), path, tokens })
}

/// What the panel costs, boot and reading counted apart.
///
/// **The prose version of this counted only the constitutions**, and said so honestly:
/// *before anybody has read a line of the draft.* That is the number nobody pays. A
/// reviewer reads the piece too, once each, so a long artifact reviewed by four agents
/// costs four times its own length on top of four constitutions, and the panel that looks
/// cheap on a one page piece is not the same panel on a twelve page one.
pub fn cost(booted: &[Booted], artifact_bytes: usize) -> Cost {
    Cost {
        boot: booted.iter().map(|b| b.tokens).sum(),
        reading: blocks::tokens(artifact_bytes) * booted.len(),
        reviewers: booted.len(),
    }
}

// ---------------------------------------------------------------------------
// The ledger
// ---------------------------------------------------------------------------

/// Opens a round: one `asked` row per reviewer.
///
/// Re-running it with the same panel changes nothing, which is what makes the boot step
/// safe to repeat when a constitution needs regenerating after an agent's base changed.
/// A reviewer added later joins the same round rather than starting a second one, because
/// the audit trail a person wants is every objection ever raised against the piece, not
/// every sitting.
pub fn open(
    root: &Path,
    artifact: &str,
    owner: &str,
    reviewers: &[String],
    today: &str,
) -> Result<Vec<String>, Error> {
    if reviewers.is_empty() {
        return Err(Error::NoReviewers);
    }
    let artifact = key_of(artifact);
    let owner = field(owner).to_lowercase();

    let log = path_in(root);
    let mut rows = load(&log);
    let mut added = Vec::new();

    for reviewer in reviewers {
        let reviewer = field(reviewer).to_lowercase();
        if reviewer == owner {
            return Err(Error::OwnerOnPanel(reviewer));
        }
        let already = rows
            .iter()
            .any(|r| r.artifact == artifact && r.reviewer == reviewer && r.seq == 0);
        if already {
            continue;
        }
        rows.push(Row {
            artifact: artifact.clone(),
            owner: owner.clone(),
            reviewer: reviewer.clone(),
            seq: 0,
            state: State::Asked,
            opened: today.to_string(),
            updated: today.to_string(),
            blocking: false,
            text: String::new(),
            why: String::new(),
        });
        added.push(reviewer);
    }

    save(&log, &rows)?;
    Ok(added)
}

/// Records what one reviewer came back with, and returns the objection's **ledger number**
/// when it raised one.
///
/// **The number returned is the round-wide ordinal, not the row's `seq`.** Those two
/// diverge the moment a second reviewer objects, and the first real run of this command
/// caught it: three reviewers each objected once, three rows carried `seq 1`, and the
/// command told the caller to resolve objection 1 three times over. `--resolve` numbers
/// across the round because the person reading the ledger counts down a single table, so
/// this counts the same way. The `seq` stays per reviewer because it is the row's key.
///
/// **`Nothing` and `NotReturned` are separate arms and no branch below merges them.** The
/// protocol stalled once on a reviewer that never answered, and the rule written that day
/// says the ledger row reads *not returned*, never *no objection*, because collapsing them
/// is how a silent reviewer becomes a fake endorsement.
pub fn record(
    root: &Path,
    artifact: &str,
    reviewer: &str,
    answer: &Answer,
    today: &str,
) -> Result<Option<u32>, Error> {
    let artifact = key_of(artifact);
    let reviewer = field(reviewer).to_lowercase();

    let log = path_in(root);
    let mut rows = load(&log);

    let Some(seat) = rows
        .iter()
        .find(|r| r.artifact == artifact && r.reviewer == reviewer && r.seq == 0)
        .cloned()
    else {
        let any = rows.iter().any(|r| r.artifact == artifact);
        return Err(match any {
            true => Error::NotOnPanel(reviewer, artifact),
            false => Error::NoRound(artifact),
        });
    };

    match answer {
        Answer::Nothing | Answer::NotReturned { .. } => {
            let (state, why) = match answer {
                Answer::Nothing => (State::Nothing, String::new()),
                Answer::NotReturned { why } => (State::NotReturned, field(why)),
                Answer::Objection { .. } => unreachable!(),
            };
            if let Some(row) = rows
                .iter_mut()
                .find(|r| r.artifact == artifact && r.reviewer == reviewer && r.seq == 0)
            {
                row.state = state;
                row.updated = today.to_string();
                row.why = why;
            }
            save(&log, &rows)?;
            Ok(None)
        }
        Answer::Objection { text, blocking } => {
            if *blocking {
                if let Some(prior) = rows.iter().find(|r| {
                    r.artifact == artifact && r.reviewer == reviewer && r.blocking && r.seq > 0
                }) {
                    return Err(Error::AlreadyBlockedOnce(reviewer, prior.seq));
                }
            }
            let seq = rows
                .iter()
                .filter(|r| r.artifact == artifact && r.reviewer == reviewer)
                .map(|r| r.seq)
                .max()
                .unwrap_or(0)
                + 1;
            rows.push(Row {
                artifact: artifact.clone(),
                owner: seat.owner.clone(),
                reviewer: reviewer.clone(),
                seq,
                state: State::Objection,
                opened: today.to_string(),
                updated: today.to_string(),
                blocking: *blocking,
                text: field(text),
                why: String::new(),
            });
            // The seat itself stops being `asked` the moment its holder speaks. Left
            // alone, a reviewer with three objections would still read as never having
            // answered, which is the one thing this file exists to state correctly.
            if let Some(row) = rows
                .iter_mut()
                .find(|r| r.artifact == artifact && r.reviewer == reviewer && r.seq == 0)
            {
                if row.state == State::Asked {
                    row.state = State::Objection;
                }
                row.updated = today.to_string();
            }
            save(&log, &rows)?;
            // Computed after the write, off the same list the ledger prints, so the number
            // the caller is handed and the number `--resolve` takes cannot be two answers.
            let n = numbered_objections(&rows, &artifact)
                .iter()
                .position(|r| r.reviewer == reviewer && r.seq == seq)
                .map(|i| i as u32 + 1);
            Ok(n)
        }
    }
}

/// The owner accounting for one objection.
///
/// **Refusing a blocking objection is not a permitted outcome and this is where that is
/// enforced.** Everything else in the protocol is the owner's judgement, on purpose; a
/// blocking objection is the one valve on it, and a valve that a rule asks nicely for is
/// not a valve.
pub fn resolve(
    root: &Path,
    artifact: &str,
    seq_of: u32,
    reviewer: Option<&str>,
    outcome: Outcome,
    why: &str,
    today: &str,
) -> Result<Row, Error> {
    let artifact = key_of(artifact);
    let log = path_in(root);
    let mut rows = load(&log);

    if !rows.iter().any(|r| r.artifact == artifact) {
        return Err(Error::NoRound(artifact));
    }

    let numbered = numbered_objections(&rows, &artifact);
    let Some((index, _)) = numbered.iter().enumerate().find(|(i, r)| {
        *i as u32 + 1 == seq_of
            && reviewer.is_none_or(|w| r.reviewer == field(w).to_lowercase())
    }) else {
        return Err(Error::NoSuchObjection(artifact, seq_of));
    };
    let target = numbered[index].key();

    let current = rows.iter().find(|r| r.key() == target).expect("just found").state;
    if current != State::Objection {
        return Err(Error::AlreadyAccounted(seq_of, current));
    }
    let blocking = rows.iter().find(|r| r.key() == target).expect("just found").blocking;
    if blocking && outcome == Outcome::Refused {
        return Err(Error::RefusingToRefuseBlocking(seq_of));
    }

    let row = rows.iter_mut().find(|r| r.key() == target).expect("just found");
    row.state = match outcome {
        Outcome::Taken => State::Taken,
        Outcome::Refused => State::Refused,
        Outcome::Escalated => State::Escalated,
    };
    row.why = field(why);
    row.updated = today.to_string();
    let done = row.clone();

    save(&log, &rows)?;
    Ok(done)
}

/// Every objection on one artifact, in the order raised, which is the order the ledger
/// numbers them in.
fn numbered_objections<'a>(rows: &'a [Row], artifact: &str) -> Vec<&'a Row> {
    rows.iter().filter(|r| r.artifact == artifact && r.seq > 0).collect()
}

/// One artifact's round, read back.
pub struct Ledger<'a> {
    pub artifact: String,
    pub owner: String,
    /// The panel, in the order it was seated.
    pub seats: Vec<&'a Row>,
    /// The objections, in the order raised. Position plus one is the number the ledger
    /// prints and `--resolve` takes.
    pub objections: Vec<&'a Row>,
}

pub fn ledger<'a>(rows: &'a [Row], artifact: &str) -> Ledger<'a> {
    let artifact = key_of(artifact);
    let seats: Vec<&Row> =
        rows.iter().filter(|r| r.artifact == artifact && r.seq == 0).collect();
    let owner = seats.first().map(|r| r.owner.clone()).unwrap_or_default();
    Ledger { objections: numbered_objections(rows, &artifact), artifact, owner, seats }
}

impl Ledger<'_> {
    pub fn exists(&self) -> bool {
        !self.seats.is_empty()
    }

    /// Objections nobody has accounted for. A round with any of these is not finished,
    /// whatever was said in chat.
    pub fn unaccounted(&self) -> Vec<(u32, &Row)> {
        self.objections
            .iter()
            .enumerate()
            .filter(|(_, r)| r.state == State::Objection)
            .map(|(i, r)| (i as u32 + 1, *r))
            .collect()
    }

    /// Blocking objections that have not gone to the person yet.
    pub fn blocking_open(&self) -> Vec<(u32, &Row)> {
        self.objections
            .iter()
            .enumerate()
            .filter(|(_, r)| r.blocking && r.state != State::Escalated && r.state != State::Taken)
            .map(|(i, r)| (i as u32 + 1, *r))
            .collect()
    }

    /// Reviewers that were booted and have said nothing at all. Never reported as having
    /// found nothing: those are different facts.
    pub fn silent(&self) -> Vec<&Row> {
        self.seats.iter().filter(|r| r.state == State::Asked).copied().collect()
    }

    /// A round is closed when every objection is accounted for, no blocking objection is
    /// still sitting with the owner, and no reviewer is still out.
    pub fn closed(&self) -> bool {
        self.exists()
            && self.unaccounted().is_empty()
            && self.blocking_open().is_empty()
            && self.silent().is_empty()
    }

    /// The ledger as the markdown table that travels with the piece.
    ///
    /// **This is the deliverable half, not a working note.** The protocol's own rule is
    /// that a piece arriving without its ledger has not been through the round, so the
    /// table has to be something a person pastes rather than something they retype from a
    /// terminal report.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "| # | Reviewer | Objection | Taken or refused | Why |\n|---|---|---|---|---|\n"
        ));
        for (i, r) in self.objections.iter().enumerate() {
            let marker = if r.blocking { " **(blocking)**" } else { "" };
            out.push_str(&format!(
                "| {} | {} | {}{} | {} | {} |\n",
                i + 1,
                r.reviewer,
                r.text,
                marker,
                r.state.as_str(),
                r.why
            ));
        }
        for r in &self.seats {
            match r.state {
                State::Nothing => out.push_str(&format!(
                    "| - | {} | found nothing from inside its own domain | - | - |\n",
                    r.reviewer
                )),
                State::NotReturned => out.push_str(&format!(
                    "| - | {} | **not returned** | - | {} |\n",
                    r.reviewer, r.why
                )),
                State::Asked => out.push_str(&format!(
                    "| - | {} | **asked, still out** | - | - |\n",
                    r.reviewer
                )),
                _ => {}
            }
        }
        out
    }
}

/// Every artifact with a round on it, in the order first opened.
pub fn artifacts(rows: &[Row]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in rows {
        if !out.iter().any(|a| a == &r.artifact) {
            out.push(r.artifact.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kb-panel-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("a scratch dir");
        d
    }

    fn agent(root: &Path, name: &str, chars: usize) -> PathBuf {
        let dir = root.join("fleet").join(name);
        std::fs::create_dir_all(&dir).expect("dirs");
        std::fs::write(dir.join("blocks.txt"), "[identity]\nindex.md\n").expect("manifest");
        std::fs::write(dir.join("index.md"), "x".repeat(chars)).expect("body");
        dir
    }

    fn objection(text: &str) -> Answer {
        Answer::Objection { text: text.to_string(), blocking: false }
    }

    fn blocking(text: &str) -> Answer {
        Answer::Objection { text: text.to_string(), blocking: true }
    }

    #[test]
    fn a_reviewer_is_assembled_from_the_same_blocks_kb_blocks_prints() {
        let root = scratch("boot");
        let dir = agent(&root, "apelles", 400);
        let out = root.join(".kb").join("panel");

        let booted = boot("apelles", &dir, &out).expect("booted");
        assert_eq!(booted.tokens, 100, "400 characters at 4 per token");
        let text = std::fs::read_to_string(&booted.path).expect("written");
        assert!(text.contains("block: identity"), "the marker the assembler writes: {text:.80}");
    }

    /// A panel of reviewers with no constitution is a panel of one model talking to
    /// itself, so this refuses rather than producing a round that looks reviewed.
    #[test]
    fn an_agent_with_no_blocks_manifest_cannot_be_seated() {
        let root = scratch("noblocks");
        let dir = root.join("fleet").join("nobody");
        std::fs::create_dir_all(&dir).expect("dirs");
        assert!(matches!(
            boot("nobody", &dir, &root.join("out")),
            Err(Error::CannotBoot(_, _))
        ));
    }

    /// The prose table this replaces counted constitutions only, and said so: *before
    /// anybody has read a line of the draft*. Nobody pays that number.
    #[test]
    fn the_cost_counts_the_artifact_once_per_reviewer_and_not_once() {
        let root = scratch("cost");
        let out = root.join("out");
        let a = boot("a", &agent(&root, "a", 400), &out).expect("a");
        let b = boot("b", &agent(&root, "b", 800), &out).expect("b");

        let c = cost(&[a, b], 4_000);
        assert_eq!(c.boot, 300);
        assert_eq!(c.reading, 2_000, "1000 tokens of artifact, read by each of the two");
        assert_eq!(c.total(), 2_300);
    }

    #[test]
    fn opening_a_round_twice_with_the_same_panel_changes_nothing() {
        let root = scratch("idempotent");
        let panel = vec!["apelles".to_string(), "steve".to_string()];
        open(&root, "site/index.html", "goldoni", &panel, "2026-09-04").expect("first");
        let again = open(&root, "site/index.html", "goldoni", &panel, "2026-09-05").expect("second");
        assert!(again.is_empty(), "nothing new was seated");
        assert_eq!(load(&path_in(&root)).len(), 2);
    }

    /// The same file written two ways is one round, or the ledger silently splits and
    /// each half looks complete.
    #[test]
    fn the_artifact_path_is_normalised_so_one_file_is_one_round() {
        let root = scratch("key");
        open(&root, "./site/index.html", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        open(&root, "site\\index.html", "goldoni", &["steve".into()], "2026-09-04").expect("open");
        let rows = load(&path_in(&root));
        assert_eq!(artifacts(&rows), vec!["site/index.html".to_string()], "{rows:?}");
    }

    #[test]
    fn the_owner_cannot_sit_on_the_panel_for_their_own_piece() {
        let root = scratch("owner");
        assert!(matches!(
            open(&root, "a.md", "goldoni", &["goldoni".into()], "2026-09-04"),
            Err(Error::OwnerOnPanel(_))
        ));
    }

    #[test]
    fn an_answer_from_an_agent_nobody_asked_is_not_a_review() {
        let root = scratch("uninvited");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        assert!(matches!(
            record(&root, "a.md", "steve", &objection("hm"), "2026-09-04"),
            Err(Error::NotOnPanel(_, _))
        ));
    }

    /// The rule written the day the protocol first stalled: the row reads "not returned",
    /// never "no objection", because collapsing them is how a silent reviewer becomes a
    /// fake endorsement.
    #[test]
    fn a_reviewer_who_never_answered_is_not_a_reviewer_who_found_nothing() {
        let root = scratch("silence");
        let panel = vec!["zed".to_string(), "steve".to_string(), "apelles".to_string()];
        open(&root, "a.md", "goldoni", &panel, "2026-09-04").expect("open");
        record(&root, "a.md", "steve", &Answer::Nothing, "2026-09-04").expect("nothing");
        record(
            &root,
            "a.md",
            "apelles",
            &Answer::NotReturned { why: "deadline, decided on the 08-20 run".into() },
            "2026-09-04",
        )
        .expect("not returned");

        let rows = load(&path_in(&root));
        let l = ledger(&rows, "a.md");
        let table = l.to_markdown();

        assert!(table.contains("not returned"), "{table}");
        assert!(table.contains("found nothing"), "{table}");
        assert_eq!(l.silent().len(), 1, "zed is still out and says so");
        assert_eq!(l.silent()[0].reviewer, "zed");
        assert!(!l.closed(), "a round with a reviewer still out is not closed");
    }

    #[test]
    fn an_objection_is_numbered_and_the_round_stays_open_until_it_is_accounted_for() {
        let root = scratch("account");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        let raised = objection("minute six states a latency the base does not produce");
        let n = record(&root, "a.md", "zed", &raised, "2026-09-04")
            .expect("recorded")
            .expect("numbered");
        assert_eq!(n, 1);

        let rows = load(&path_in(&root));
        assert!(!ledger(&rows, "a.md").closed());
        assert_eq!(ledger(&rows, "a.md").unaccounted().len(), 1);

        resolve(&root, "a.md", 1, None, Outcome::Refused, "measured again on 09-04", "2026-09-04")
            .expect("refused");
        let rows = load(&path_in(&root));
        assert!(ledger(&rows, "a.md").closed(), "{rows:?}");
        assert!(ledger(&rows, "a.md").to_markdown().contains("refused"));
    }

    /// The one piece of judgement the owner does not hold. A rule that asks nicely is not
    /// a valve.
    #[test]
    fn a_blocking_objection_cannot_be_refused_and_can_be_escalated() {
        let root = scratch("blocking");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        record(&root, "a.md", "zed", &blocking("the number is false"), "2026-09-04").expect("raised");

        assert!(matches!(
            resolve(&root, "a.md", 1, None, Outcome::Refused, "I disagree", "2026-09-04"),
            Err(Error::RefusingToRefuseBlocking(1))
        ));

        let rows = load(&path_in(&root));
        assert_eq!(ledger(&rows, "a.md").blocking_open().len(), 1);

        resolve(&root, "a.md", 1, None, Outcome::Escalated, "to Richard", "2026-09-04")
            .expect("escalated");
        let rows = load(&path_in(&root));
        assert!(ledger(&rows, "a.md").blocking_open().is_empty());
        assert!(ledger(&rows, "a.md").closed());
    }

    /// The first real run of this command caught this: three reviewers objected once each,
    /// all three rows carried `seq 1`, and the command said "objection 1" three times while
    /// `--resolve` numbered them 1, 2 and 3. Two places computing the same number.
    #[test]
    fn the_number_reported_is_the_number_resolve_takes() {
        let root = scratch("ordinal");
        let panel = vec!["zed".to_string(), "steve".to_string(), "apelles".to_string()];
        open(&root, "a.md", "goldoni", &panel, "2026-09-04").expect("open");

        let a = record(&root, "a.md", "zed", &objection("first"), "2026-09-04").expect("a");
        let b = record(&root, "a.md", "steve", &objection("second"), "2026-09-04").expect("b");
        let c = record(&root, "a.md", "apelles", &objection("third"), "2026-09-04").expect("c");
        assert_eq!((a, b, c), (Some(1), Some(2), Some(3)), "one table, one numbering");

        resolve(&root, "a.md", 2, None, Outcome::Taken, "fixed", "2026-09-04").expect("second");
        let rows = load(&path_in(&root));
        let taken: Vec<&Row> = rows.iter().filter(|r| r.state == State::Taken).collect();
        assert_eq!(taken[0].text, "second", "the number the caller was handed picked its own row");
    }

    /// A reviewer who can block everything holds a veto, and a veto is not what the owner
    /// agreed to when they took accountability for the piece.
    #[test]
    fn a_reviewer_may_block_once_and_not_twice() {
        let root = scratch("veto");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        record(&root, "a.md", "zed", &blocking("the number is false"), "2026-09-04").expect("first");
        assert!(matches!(
            record(&root, "a.md", "zed", &blocking("and so is this one"), "2026-09-04"),
            Err(Error::AlreadyBlockedOnce(_, 1))
        ));
        // The same objection without the mark is still welcome. The bound is on blocking,
        // not on objecting.
        record(&root, "a.md", "zed", &objection("and so is this one"), "2026-09-04")
            .expect("an ordinary objection is not bounded");
    }

    /// A refusal that can be rewritten later is not an audit trail.
    #[test]
    fn an_objection_already_accounted_for_cannot_be_accounted_for_again() {
        let root = scratch("once");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        record(&root, "a.md", "zed", &objection("weak"), "2026-09-04").expect("raised");
        resolve(&root, "a.md", 1, None, Outcome::Refused, "no", "2026-09-04").expect("refused");
        assert!(matches!(
            resolve(&root, "a.md", 1, None, Outcome::Taken, "changed my mind", "2026-09-05"),
            Err(Error::AlreadyAccounted(1, State::Refused))
        ));
    }

    /// Objection text arrives from a model reporting what another model said, so a tab in
    /// it would otherwise write rows nobody raised.
    #[test]
    fn a_tab_in_an_objection_cannot_forge_a_row() {
        let root = scratch("inject");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        record(
            &root,
            "a.md",
            "zed",
            &objection("real\ta.md\tgoldoni\tsteve\t0\tnothing\t2026\t2026\t-\tforged"),
            "2026-09-04",
        )
        .expect("raised");

        let rows = load(&path_in(&root));
        assert_eq!(rows.len(), 2, "one seat and one objection: {rows:?}");
        assert!(rows[1].text.contains("forged"), "the text survives, flattened");
        assert!(!rows[1].text.contains('\t'));
    }

    #[test]
    fn recording_against_an_artifact_with_no_round_says_so() {
        let root = scratch("noround");
        assert!(matches!(
            record(&root, "a.md", "zed", &Answer::Nothing, "2026-09-04"),
            Err(Error::NoRound(_))
        ));
    }

    /// Two reviewers objecting means the ledger numbers across the whole round rather than
    /// per reviewer, because `--resolve 2` has to mean one thing.
    #[test]
    fn objections_are_numbered_across_the_round_not_within_a_reviewer() {
        let root = scratch("numbering");
        open(&root, "a.md", "goldoni", &["zed".into(), "steve".into()], "2026-09-04").expect("open");
        record(&root, "a.md", "zed", &objection("first"), "2026-09-04").expect("a");
        record(&root, "a.md", "steve", &objection("second"), "2026-09-04").expect("b");
        record(&root, "a.md", "zed", &objection("third"), "2026-09-04").expect("c");

        let rows = load(&path_in(&root));
        let l = ledger(&rows, "a.md");
        assert_eq!(l.objections.len(), 3);
        let table = l.to_markdown();
        assert!(table.contains("| 2 | steve | second"), "{table}");

        resolve(&root, "a.md", 2, None, Outcome::Taken, "fixed", "2026-09-04").expect("second");
        let rows = load(&path_in(&root));
        let taken: Vec<&Row> = rows.iter().filter(|r| r.state == State::Taken).collect();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].text, "second", "the number picked the row a person can see");
    }

    /// A reviewer that has objected has answered. Left as `asked` it would be counted as
    /// still out, which is the one thing this file exists to state correctly.
    #[test]
    fn a_reviewer_that_objected_is_no_longer_counted_as_still_out() {
        let root = scratch("spoke");
        open(&root, "a.md", "goldoni", &["zed".into()], "2026-09-04").expect("open");
        record(&root, "a.md", "zed", &objection("hm"), "2026-09-04").expect("raised");
        let rows = load(&path_in(&root));
        assert!(ledger(&rows, "a.md").silent().is_empty(), "{rows:?}");
    }
}
