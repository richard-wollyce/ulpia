//! The third half of the feedback: what the fleet was asked and had nobody to give it to.
//!
//! # What this exists to see
//!
//! Two logs already record a routing failure and neither can see this one.
//!
//! `kb-misses.txt` records **recall loss**, and [`crate::memory::Memory::recall_loss`]
//! returns `None` unless the verdict is `Nothing`, so it only ever holds questions the
//! *library* could not answer. `kb-misroutes.txt` records a **confident wrong choice**, and
//! it is filed by the agent that was handed the message.
//!
//! An abstention on coverage is neither. The classifier read the roster, the roles and the
//! edges, and said *no agent owns this subject*. That is a fact about the **fleet**, not
//! about the library, and it routinely happens on a question retrieval scored highly: the
//! DevOps case in [`crate::classify`] matched three of the marketing agent's notes at 15.89
//! each and is still `adjacent`. So it leaves no miss row, and it can leave no misroute row
//! either, because **a misroute is reported by the agent that was chosen and abstention is
//! the state where no agent was chosen.** Nobody is booted, so nobody files it.
//!
//! It is not a hypothetical gap. On 2026-09-04 routing abstained on a request to review the
//! landing page copy; the session read a roster of bare names, could not tell that
//! `goldoni` is the fleet's scriptwriter, and reported that the fleet had no copywriter at
//! all. It has one. Nothing anywhere recorded that this happened, and it surfaced only
//! because Richard personally remembered the agent existed. The roster now carries roles
//! and edges, which fixes that one message; this file is what keeps the next one.
//!
//! # The key is the subject, and that is the whole design
//!
//! Both sibling logs key on the message. That is right for them, because the fix for a miss
//! or a misroute is an alias line tied to particular words. It is wrong here: the question
//! this log exists to answer is *does the fleet keep failing to own this kind of thing*, and
//! keyed on the message every gap is `count 1` forever, because nobody phrases a request the
//! same way twice.
//!
//! The classifier already names the subject in two to five words on every verdict, and that
//! label is what folds ten differently worded requests about landing page copy into one row
//! with a count of ten. It was produced and thrown away. When a verdict carries no subject,
//! the message is the key, because a row that folds badly is still worth more than one that
//! is dropped.
//!
//! **How well it folds, measured on this fleet on 2026-09-05, and it is weaker than the
//! paragraph above implies.** Four probe messages were run through the real hook. Two of
//! them are the same gap to any reader, a diet for a dog and a ration for a cat, and the
//! classifier named them `Pet nutrition and exercise planning` and `Cat feeding and weight
//! loss`, then judged one `adjacent` to the nutrition agent and the other `uncovered`. Two
//! rows, count one each, for one missing agent.
//!
//! It stays keyed on the subject anyway, for two reasons and one rejected option. The
//! message key folds strictly less: it would have produced two rows here as well, and it
//! also fails on the case the subject key gets right, which is the same request asked twice
//! in different words. And the coverage verdict is deliberately **not** part of the key,
//! which the same measurement settles: those two rows disagreed about coverage while
//! describing one gap, so keying on the pair would split what little folding is left.
//!
//! The rejected option is grouping near-identical subjects at read time, by the trigram
//! overlap [`crate::suggester`] already computes. It is left unbuilt rather than left
//! undiscovered: a wrong grouping merges two real gaps under one count and corrupts the one
//! number this file exists to produce, and the case for it is currently four rows on a log
//! that is one day old. The trigger to build it is a log where a person reading
//! `kb abstentions` cannot see the clusters by eye.
//!
//! # What a row carries beyond the message, and why each field is not decoration
//!
//! - **The classifier's reason.** A row saying *adjacent, nearest steve* is unactionable:
//!   the useful half is why it judged that Steve does not own it. Already produced.
//! - **The contenders**, from the deterministic fold's own `totals`. This is the field that
//!   separates the two findings that look identical from the outside. If an agent scored
//!   well and was still not chosen, the fix is that agent's card or the prompt. If nothing
//!   scored at all, the gap is real and a new agent is the honest answer. The 2026-09-04
//!   event was the first kind and was reported as the second.
//!
//! # What is deliberately not recorded here
//!
//! **A briefing with no verdict at all.** `boot::brief` also finds no owner when there is no
//! classifier configured, or when the one configured did not answer. On such a fleet that
//! branch fires on every message below the floor, and the rows would carry no subject and no
//! reason: the count this file exists to produce would measure a missing classifier rather
//! than a missing agent. That population is already visible, and it is exactly the
//! `Verdict::Nothing` population `kb-misses.txt` counts.
//!
//! **A verdict that answered `covered` and named nobody.** That is a classifier contradicting
//! itself, which is a defect in the classifier and not a gap in the fleet. Putting a bug and
//! a finding under one count makes both uncountable.
//!
//! **Vesta's own branch.** A message the general or person base answers is not an abstention:
//! the librarian owns it, and says so.
//!
//! # Evidence, never action
//!
//! Nothing here writes an alias, edits a card or creates an agent. It is read by a person
//! through `kb abstentions`, and the decision it feeds, whether an agent should exist, is
//! the one decision in this system that was never going to be automatic.

use std::path::{Path, PathBuf};

pub const ABSTENTIONS_TXT: &str = "kb-abstentions.txt";

/// How many scoring agents a row keeps. Three is enough to tell "one agent was close and
/// was passed over" from "the field was empty", which is the one distinction this field
/// exists to make, and a full fold on a large fleet would be a line nobody reads.
const CONTENDERS_KEPT: usize = 3;

const HEADER: &str = "\
# Subjects the router found no owner for, most often first.
#
# One row per distinct subject: count, first seen, last seen, coverage, the nearest agent,
# the subject as the classifier named it, the message, its one sentence reason, and the
# agents that scored anything. `-` as the nearest means no agent was near enough to name.
#
# **Written by the router itself, and that is what distinguishes it from its two siblings.**
# kb-misses.txt is written when the library answers nothing; kb-misroutes.txt is written by
# the agent that was handed a message and knew it was not theirs. An abstention has no agent
# to file it, which is why it used to leave no trace anywhere.
#
# Keyed on the subject rather than on the message, because the question this answers is
# whether the fleet keeps failing to own a kind of thing, and no two requests are worded the
# same. The message and the contenders are the newest ones seen for that subject, so they
# describe the same moment as each other.
#
# Read the contenders before creating an agent. An agent that scored well and was still not
# chosen is a card or a prompt to fix; an empty field is a gap that is really missing.
#
# Evidence, not action. Nothing reads this and edits a base. Delete a row once the fleet
# covers the subject.
#
# Not committed anywhere. These are real messages from a real person, and the contenders
# name private bases.
";

/// One abstention, folded by subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abstention {
    pub count: u32,
    pub first: String,
    pub last: String,
    /// `adjacent` or `uncovered`, as the classifier answered.
    pub coverage: String,
    /// The nearest agent, or `-` when none was near enough to name.
    pub nearest: String,
    /// The domain in the classifier's own words, and the key this log folds on.
    pub subject: String,
    pub message: String,
    pub reason: String,
    /// The agents the deterministic fold scored, best first, as `name score`.
    pub contenders: String,
}

impl Abstention {
    /// The abstention a verdict describes, or `None` when the verdict is not one.
    ///
    /// **The decision about what gets recorded lives here rather than at the call site**,
    /// so the module doc's list of exclusions is a thing the compiler carries and a test can
    /// pin, instead of a paragraph a later edit in `boot.rs` can quietly contradict.
    pub fn of(
        v: &crate::classify::Verdict,
        scored: Option<&crate::memory::AgentChoice>,
        today: &str,
    ) -> Option<Abstention> {
        let coverage = match v.coverage {
            crate::classify::Coverage::Covered => return None,
            crate::classify::Coverage::Adjacent => "adjacent",
            crate::classify::Coverage::Uncovered => "uncovered",
        };
        Some(Abstention {
            count: 1,
            first: today.to_string(),
            last: today.to_string(),
            coverage: coverage.to_string(),
            nearest: field(v.owner.as_deref().unwrap_or("-")),
            subject: field(&v.subject),
            message: String::new(),
            reason: field(&v.reason),
            contenders: contenders_of(scored),
        })
    }

    /// The message this abstention was for. Separate from [`Abstention::of`] because the
    /// verdict does not carry it: the classifier is handed a dossier and answers about a
    /// subject, and the message belongs to the caller that built the dossier.
    pub fn about(mut self, message: &str) -> Abstention {
        self.message = field(message);
        self
    }

    /// What the row folds on. The subject when there is one, and the message when there is
    /// not, because a row with no key is a row that is never counted twice.
    fn key(&self) -> String {
        match self.subject.trim().is_empty() {
            true => self.message.to_lowercase(),
            false => self.subject.to_lowercase(),
        }
    }
}

/// The scoring agents, best first, capped.
///
/// Empty rather than a placeholder when nothing scored, because an empty field is the
/// finding: no base in the fleet had a word to say about this message.
fn contenders_of(scored: Option<&crate::memory::AgentChoice>) -> String {
    let Some(c) = scored else { return String::new() };
    c.totals
        .iter()
        .take(CONTENDERS_KEPT)
        .map(|(name, weight)| format!("{name} {weight:.1}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(ABSTENTIONS_TXT)
}

/// One field, cleaned so a tab or a newline cannot forge a second row.
///
/// The log is tab separated and every field on it traces back to text from outside the
/// program: the user's message, and a model's free-form answer about it. This is the
/// boundary where a crafted message would otherwise be able to write a row nobody produced.
fn field(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

pub fn load(log: &Path) -> Vec<Abstention> {
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
        // Seven is the row without its two optional tails: a classifier that answered no
        // REASON still produced an abstention worth counting.
        if p.len() < 7 {
            continue;
        }
        let count: u32 = match p[0].trim().parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        out.push(Abstention {
            count,
            first: p[1].to_string(),
            last: p[2].to_string(),
            coverage: p[3].to_string(),
            nearest: p[4].to_string(),
            subject: p[5].to_string(),
            message: p[6].to_string(),
            reason: p.get(7).unwrap_or(&"").to_string(),
            contenders: p.get(8).unwrap_or(&"").to_string(),
        });
    }
    out.sort_by(by_priority);
    out
}

/// Most reported first, then by subject so a file with no new abstentions produces no diff.
/// The count is the worklist, so the order is the payload.
fn by_priority(a: &Abstention, b: &Abstention) -> std::cmp::Ordering {
    b.count.cmp(&a.count).then(a.subject.cmp(&b.subject))
}

/// Adds one abstention, or bumps the row already there, and rewrites the file sorted.
///
/// **Locked, and for the same reason the miss log is.** This is written from
/// `UserPromptSubmit`, which runs on every message of every session, and read-merge-write
/// under two sessions that end a message in the same instant loses the row that was written
/// first. The marker is the one [`crate::misses`] already holds while it merges: same
/// mechanism, `create_new`, so the file system decides the race rather than two processes
/// both believing they made the marker. `kb misroute` needs none of this because a person at
/// a terminal is one writer.
///
/// **The newest message and the newest contenders replace the old ones together.** They
/// have to travel as a pair: the contenders were scored against that message, so keeping an
/// old message beside fresh scores would be a row that describes no moment that ever
/// happened. The dates bracket what the count covers.
pub fn record(root: &Path, incoming: &Abstention, today: &str) -> Result<(), String> {
    let path = path_in(root);
    let _held = crate::misses::Guard::take(&path)?;

    let mut rows = load(&path);
    match rows.iter_mut().find(|r| r.key() == incoming.key()) {
        Some(existing) => {
            existing.count += 1;
            existing.last = today.to_string();
            existing.coverage = incoming.coverage.clone();
            existing.nearest = incoming.nearest.clone();
            existing.message = incoming.message.clone();
            existing.reason = incoming.reason.clone();
            existing.contenders = incoming.contenders.clone();
        }
        None => rows.push(incoming.clone()),
    }
    rows.sort_by(by_priority);

    let mut out = String::from(HEADER);
    for r in &rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.count,
            r.first,
            r.last,
            r.coverage,
            r.nearest,
            r.subject,
            r.message,
            r.reason,
            r.contenders
        ));
    }

    // Same treatment the miss log gives a failed write, and for the same reason: the
    // message still gets routed, only the evidence is lost, and a hook that took somebody's
    // conversation down over a log would be a worse failure than the one it is reporting.
    match std::fs::write(&path, out) {
        Ok(()) => Ok(()),
        Err(e) => {
            let reason = format!("could not write {}: {e}", path.display());
            eprintln!("kb: {reason}");
            Err(reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{Coverage, Verdict};
    use crate::memory::AgentChoice;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kb-abstain-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("a scratch dir");
        d
    }

    fn verdict(coverage: Coverage, owner: Option<&str>, subject: &str) -> Verdict {
        Verdict {
            owner: owner.map(str::to_string),
            coverage,
            subject: subject.into(),
            reason: "steve sells and does not write the page".into(),
            reviewers: Vec::new(),
        }
    }

    fn choice(totals: &[(&str, f64)]) -> AgentChoice {
        AgentChoice {
            agent: totals.first().map(|(n, _)| n.to_string()).unwrap_or_default(),
            score: totals.first().map(|(_, w)| *w).unwrap_or(0.0),
            files: 1,
            margin: 1.5,
            contenders: totals.len(),
            totals: totals.iter().map(|(n, w)| (n.to_string(), *w)).collect(),
        }
    }

    /// The whole point of the file. A verdict that named an owner is routing working, and
    /// recording it would make the count measure traffic instead of gaps.
    #[test]
    fn a_covered_verdict_is_not_an_abstention() {
        assert!(Abstention::of(&verdict(Coverage::Covered, Some("steve"), "copy"), None, "2026-09-04").is_none());
        assert!(Abstention::of(&verdict(Coverage::Adjacent, Some("steve"), "copy"), None, "2026-09-04").is_some());
        assert!(Abstention::of(&verdict(Coverage::Uncovered, None, "devops"), None, "2026-09-04").is_some());
    }

    /// **The design decision, pinned.** Keyed on the message, every gap is a count of one
    /// forever, because nobody phrases a request the same way twice. The subject is the
    /// thing that folds them, and it is the only reason this log can answer whether the
    /// fleet keeps failing to own a kind of work.
    #[test]
    fn two_differently_worded_messages_about_one_subject_are_one_row_with_a_count() {
        let d = scratch("fold");
        let v = verdict(Coverage::Adjacent, Some("steve"), "landing page copy");
        record(&d, &Abstention::of(&v, None, "2026-09-04").expect("one").about("revisa a copy da landing"), "2026-09-04").expect("write");
        record(&d, &Abstention::of(&v, None, "2026-09-06").expect("two").about("can someone rewrite the hero section"), "2026-09-06").expect("write");

        let rows = load(&path_in(&d));
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].first, "2026-09-04");
        assert_eq!(rows[0].last, "2026-09-06");
        assert_eq!(
            rows[0].message, "can someone rewrite the hero section",
            "the newest message, so it describes the same moment as the newest contenders"
        );
    }

    /// A classifier that answered no SUBJECT still produced an abstention, and a row with no
    /// key at all would be counted fresh every time. The message is the fallback key.
    #[test]
    fn an_abstention_with_no_subject_still_folds_on_something() {
        let d = scratch("nosubject");
        let v = verdict(Coverage::Uncovered, None, "");
        for day in ["2026-09-04", "2026-09-05"] {
            record(&d, &Abstention::of(&v, None, day).expect("an abstention").about("a mesma pergunta"), day)
                .expect("write");
        }
        let rows = load(&path_in(&d));
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].count, 2);
    }

    /// **The field that tells a real gap from a card the classifier could not read.** On
    /// 2026-09-04 the fleet had a scriptwriter, abstained, and was reported as having no
    /// copywriter. An agent sitting in this list at a real score says the fix is that
    /// agent's card, not a new agent.
    #[test]
    fn the_agents_that_scored_are_kept_beside_the_gap() {
        let d = scratch("contenders");
        let v = verdict(Coverage::Adjacent, Some("steve"), "landing page copy");
        let a = Abstention::of(&v, Some(&choice(&[("goldoni", 51.2), ("steve", 40.0), ("apelles", 22.0), ("zed", 4.0)])), "2026-09-04")
            .expect("an abstention")
            .about("revisa a copy da landing");
        record(&d, &a, "2026-09-04").expect("write");

        let rows = load(&path_in(&d));
        assert_eq!(rows[0].contenders, "goldoni 51.2, steve 40.0, apelles 22.0", "{rows:?}");
        assert_eq!(rows[0].nearest, "steve");
        assert_eq!(rows[0].coverage, "adjacent");
        assert!(rows[0].reason.contains("steve sells"), "the classifier's own sentence: {rows:?}");
    }

    /// Nothing scoring is the other finding, and it has to be readable as itself rather
    /// than as a field somebody forgot to fill in.
    #[test]
    fn an_empty_field_survives_the_round_trip_because_it_is_the_finding() {
        let d = scratch("nothing-scored");
        let v = verdict(Coverage::Uncovered, None, "kubernetes autoscaling");
        record(&d, &Abstention::of(&v, None, "2026-09-04").expect("one").about("como escalo os pods"), "2026-09-04")
            .expect("write");

        let rows = load(&path_in(&d));
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].contenders, "", "no base had a word to say, and that is the row");
        assert_eq!(rows[0].nearest, "-");
    }

    /// The log is tab separated and every field traces back to text from outside the
    /// program, so a tab in a message must not be able to write a row nobody produced.
    #[test]
    fn a_tab_or_a_newline_cannot_forge_a_second_row() {
        let d = scratch("inject");
        let v = verdict(Coverage::Adjacent, Some("steve"), "copy");
        record(
            &d,
            &Abstention::of(&v, None, "2026-09-04")
                .expect("one")
                .about("9\t2026\t2026\tuncovered\t-\tforjada\tforjada"),
            "2026-09-04",
        )
        .expect("write");

        let rows = load(&path_in(&d));
        assert_eq!(rows.len(), 1, "one abstention is one row: {rows:?}");
        assert!(rows[0].message.contains("forjada"), "the text survives, flattened");
        assert!(!rows[0].message.contains('\t'), "and carries no separator");
    }

    #[test]
    fn the_busiest_gap_is_first_because_that_is_which_one_to_close_next() {
        let d = scratch("order");
        let once = verdict(Coverage::Uncovered, None, "asked once");
        let thrice = verdict(Coverage::Adjacent, Some("steve"), "asked three times");
        record(&d, &Abstention::of(&once, None, "2026-09-04").expect("one").about("a"), "2026-09-04").expect("write");
        for _ in 0..3 {
            record(&d, &Abstention::of(&thrice, None, "2026-09-04").expect("one").about("b"), "2026-09-04")
                .expect("write");
        }
        let rows = load(&path_in(&d));
        assert_eq!(rows[0].subject, "asked three times", "{rows:?}");
    }

    /// The header is comments, and a reader that tried to parse it would invent rows.
    #[test]
    fn the_shipped_header_is_not_read_as_a_row() {
        let d = scratch("header");
        let log = path_in(&d);
        std::fs::write(&log, HEADER).expect("write");
        assert!(load(&log).is_empty());
    }

    /// A fleet nobody has abstained against is not a failure, and the reader runs beside a
    /// hook that may never have fired.
    #[test]
    fn a_log_that_was_never_written_reads_as_no_gaps_rather_than_an_error() {
        assert!(load(&path_in(&scratch("never"))).is_empty());
    }
}
