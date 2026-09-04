//! The other half of the feedback, and the one that was never collected.
//!
//! # What this exists to see
//!
//! `kb-misses.txt` records recall loss: a question that reached nothing.
//! [`crate::memory::Memory::recall_loss`] returns `None` unless the verdict is `Nothing`,
//! so that log is, by construction, blind to every failure where the router was confident
//! and wrong. That is the entire other half of routing, and until this file existed nothing
//! in the system wrote it down.
//!
//! It is not a hypothetical gap. `interface = design system` sat in one base pulling every
//! question carrying the word `interface` to its author, for weeks, and produced not one
//! line of evidence anywhere, because it never made anything missing.
//!
//! # Who reports it, and why it is not the person
//!
//! **The agent already knows.** `kb boot` hands the winning agent its constitution above a
//! line that says, in as many words, that the choice was made by the router and *if it is
//! wrong, say so rather than answering as somebody else*. So the fleet already asks for
//! this judgement on every routed message, gets it, and then throws it away: the agent says
//! it in prose, in a conversation, and nothing keeps it.
//!
//! Waiting for Richard to notice and report the same thing is strictly worse. He sees one
//! session at a time and forgets by the next one; the agent sees the boot payload, the
//! question and its own base in the same breath, and is the only party in the loop that can
//! tell "this is not mine" at the moment it is true. So the verb is for the agent to call,
//! and the constitution tells it to.
//!
//! # Why an agent writing here is not the loop closing on itself
//!
//! This log is **evidence, never action.** Nothing here writes an alias, changes a key or
//! moves a file. It is read by the proposer and every proposal still has to survive
//! [`crate::gate`], which measures against the gold set and refuses anything that costs a
//! column. The agent's report widens what the loop can see. It does not widen what the loop
//! may do, and those two have to stay apart or the gate is decoration.

use std::path::{Path, PathBuf};

pub const MISROUTES_TXT: &str = "kb-misroutes.txt";

const HEADER: &str = "\
# Messages the router sent to the wrong agent, most reported first.
#
# One line per distinct report: count, first seen, last seen, who was chosen, who should
# have had it, the message, and why in the reporting agent's own words. `-` as the owner
# means no agent should have taken it.
#
# **Reported by the agent, not by Richard.** `kb boot` already tells the agent the choice
# was not its to override and to say so if it is wrong. This is where saying so lands.
#
# This is the half `kb-misses.txt` cannot see. That log only records questions that reached
# nothing, so a confident wrong answer leaves it empty. Alias expansion is additive and can
# only ever cause this kind of failure, never the other, which is why writing an alias is
# gated on measurement rather than on the miss log alone.
#
# Evidence, not action. Nothing reads this and edits a base; proposals still go through the
# gate. Delete a line once the routing it describes is fixed.
#
# Not committed anywhere. These are real messages from a real person.
";

/// One reported misroute, folded by the three fields that identify it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misroute {
    pub count: u32,
    pub first: String,
    pub last: String,
    /// The agent `kb boot` picked.
    pub chose: String,
    /// The agent that should have had it, or `-` for nobody.
    pub owner: String,
    pub question: String,
    pub why: String,
}

impl Misroute {
    fn key(&self) -> (String, String, String) {
        (self.question.clone(), self.chose.clone(), self.owner.clone())
    }
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(MISROUTES_TXT)
}

/// One field, cleaned so a tab or a newline in a message cannot forge a second row.
///
/// The log is tab separated and written by a model's caller, so this is the boundary where
/// a crafted message would otherwise be able to write a row nobody reported.
fn field(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

pub fn load(log: &Path) -> Vec<Misroute> {
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
        if p.len() < 6 {
            continue;
        }
        out.push(Misroute {
            count: p[0].trim().parse().unwrap_or(1),
            first: p[1].to_string(),
            last: p[2].to_string(),
            chose: p[3].to_string(),
            owner: p[4].to_string(),
            question: p[5].to_string(),
            why: p.get(6).unwrap_or(&"").to_string(),
        });
    }
    out.sort_by(|a, b| b.count.cmp(&a.count));
    out
}

/// Appends a report, folding a repeat into a count.
///
/// Folding matters more here than in the miss log. A misroute that keeps happening is a
/// standing defect in the keys or the aliases, and a misroute reported once is a message
/// that was merely unusual. Only the count tells them apart, and a log that wrote a fresh
/// row every time would lose exactly that.
pub fn record(root: &Path, chose: &str, owner: &str, question: &str, why: &str, today: &str) {
    let path = path_in(root);
    let incoming = Misroute {
        count: 1,
        first: today.to_string(),
        last: today.to_string(),
        chose: field(chose),
        owner: field(owner),
        question: field(question),
        why: field(why),
    };

    let mut rows = load(&path);
    match rows.iter_mut().find(|r| r.key() == incoming.key()) {
        Some(existing) => {
            existing.count += 1;
            existing.last = today.to_string();
            if !incoming.why.is_empty() {
                existing.why = incoming.why.clone();
            }
        }
        None => rows.push(incoming),
    }
    rows.sort_by(|a, b| b.count.cmp(&a.count));

    let mut out = String::from(HEADER);
    for r in &rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.count, r.first, r.last, r.chose, r.owner, r.question, r.why
        ));
    }
    let _ = std::fs::write(&path, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kb-misroute-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("a scratch dir");
        d
    }

    #[test]
    fn the_same_report_twice_is_one_row_with_a_count() {
        let d = scratch("fold");
        record(&d, "zed", "yaron", "quanto de creatina por dia", "nutrition", "2026-09-04");
        record(&d, "zed", "yaron", "quanto de creatina por dia", "nutrition", "2026-09-05");
        let rows = load(&path_in(&d));
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].first, "2026-09-04");
        assert_eq!(rows[0].last, "2026-09-05", "the count is what makes a standing defect visible");
    }

    /// A different owner is a different finding even for the same question, because the two
    /// disagree about where the fix goes.
    #[test]
    fn the_same_question_routed_wrong_two_different_ways_stays_two_rows() {
        let d = scratch("split");
        record(&d, "zed", "yaron", "vitamina d", "", "2026-09-04");
        record(&d, "zed", "-", "vitamina d", "", "2026-09-04");
        assert_eq!(load(&path_in(&d)).len(), 2);
    }

    /// The log is tab separated and the message comes from outside the system, so a tab in
    /// a question would otherwise let a crafted message write a row nobody reported.
    #[test]
    fn a_tab_or_a_newline_in_a_message_cannot_forge_a_second_row() {
        let d = scratch("inject");
        record(&d, "zed", "yaron", "creatina\t9\t2026\tzed\tzed\tforjada", "", "2026-09-04");
        let rows = load(&path_in(&d));
        assert_eq!(rows.len(), 1, "one report is one row: {rows:?}");
        assert!(rows[0].question.contains("forjada"), "the text survives, flattened");
        assert!(!rows[0].question.contains('\t'), "and carries no separator");
    }

    #[test]
    fn the_busiest_report_is_first_because_that_is_which_one_to_fix_next() {
        let d = scratch("order");
        record(&d, "zed", "aldus", "uma vez", "", "2026-09-04");
        for _ in 0..3 {
            record(&d, "zed", "yaron", "tres vezes", "", "2026-09-04");
        }
        let rows = load(&path_in(&d));
        assert_eq!(rows[0].question, "tres vezes", "{rows:?}");
    }
}
