//! The instrument that keeps the router's thresholds honest.
//!
//! ADR-0018 closed with "re-measure at 1,000 files with one command", and Z23 ships a
//! score floor calibrated on a single wrong answer. Both of those are promises that
//! only mean something if the measurement is cheap enough to actually re-run, so this
//! lives in `kb` rather than in `tools/bench`: the bench crate exists to audition
//! models and carries a fastembed dependency tree to do it, while this needs nothing
//! that is not already here. Anyone who can build `kb` can re-check its numbers.
//!
//! **It grades three separate things, and they fail independently.**
//!
//! 1. *File level.* Did the right file come back first? The original question.
//! 2. *Agent level.* Did the right agent's base win? This is the question a router
//!    that picks who answers has to get right, and nothing measured it before.
//! 3. *Abstention.* When the router was wrong, did it say so? A router that is right
//!    80% of the time and silent about the other 20% is worse to build on than one
//!    that is right 70% of the time and flags the rest, because the first one has to
//!    be double checked everywhere and the second one only sometimes.
//!
//! The agent level number is reported against the **strongest possible hardcoded
//! baseline**: always answering with whichever agent owns the most questions in the
//! gold set. Comparing against a weaker fixed choice would flatter the router by
//! picking a bad opponent, and the honest comparison is the point of the exercise.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use crate::memory::{AgentChoice, Confidence, Memory, Verdict};

/// One graded question.
pub struct Row {
    pub question: String,
    /// Acceptable answers as `agent/relative`. Empty means the correct behaviour is
    /// to abstain, which is a gradeable outcome and not a missing row.
    pub answers: Vec<String>,
    pub top: Option<String>,
    /// The keyword scorer's own top-1, measured beside the fused one because the two
    /// disagree and ADR-0018 recorded the keyword side as the stronger of them. A
    /// summary that reports only the path the code happens to call would hide a
    /// regression in whichever path it did not measure.
    pub keyword_top: Option<String>,
    pub agent: Option<AgentChoice>,
    /// The same choice folded from the keyword ranking instead of the fused one.
    pub keyword_agent: Option<AgentChoice>,
    pub confidence: Confidence,
    pub micros: u128,
    pub keyword_micros: u128,
    /// Bases that are allowed to be an answer's owner. Empty means "not filtered",
    /// which is what the unit tests want.
    pub routable: Vec<String>,
}

impl Row {
    /// True when the gold set says to abstain.
    pub fn expects_abstention(&self) -> bool {
        self.answers.is_empty()
    }

    pub fn file_hit(&self) -> bool {
        match &self.top {
            Some(path) => self.answers.iter().any(|a| a == path),
            None => false,
        }
    }

    pub fn keyword_hit(&self) -> bool {
        match &self.keyword_top {
            Some(path) => self.answers.iter().any(|a| a == path),
            None => false,
        }
    }

    /// The agents any acceptable answer belongs to. A question answerable from two
    /// bases has two correct agents and picking either is right, which matters here:
    /// three questions in the current set are answerable from the architect's notes
    /// or from the decision records, and grading them against one arbitrary choice
    /// would manufacture failures.
    /// Bases that can answer, as opposed to bases that can be read. Set from the fleet
    /// before grading, because an attached base like the decision records is a shared
    /// library every agent reads and none of them *is*.
    pub fn set_routable(&mut self, routable: &[String]) {
        self.routable = routable.to_vec();
    }

    /// True when no acceptable answer lives in a base that could answer.
    ///
    /// Six of the nineteen questions are answered only from the attached decision
    /// records. There is no correct agent to grade against for those, so they are
    /// excluded from the agent denominator and **counted out loud** rather than being
    /// silently scored as misses, which would understate the router by a third.
    pub fn has_no_routable_owner(&self) -> bool {
        !self.answers.is_empty() && self.correct_agents().is_empty()
    }

    pub fn correct_agents(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for a in &self.answers {
            if let Some((agent, _)) = a.split_once('/') {
                let routable = self.routable.is_empty()
                    || self.routable.iter().any(|r| r.eq_ignore_ascii_case(agent));
                if routable && !out.contains(&agent) {
                    out.push(agent);
                }
            }
        }
        out
    }

    pub fn agent_hit(&self) -> bool {
        match &self.agent {
            Some(c) => self.correct_agents().contains(&c.agent.as_str()),
            None => false,
        }
    }

    pub fn keyword_agent_hit(&self) -> bool {
        match &self.keyword_agent {
            Some(c) => self.correct_agents().contains(&c.agent.as_str()),
            None => false,
        }
    }
}

/// Reads `question<TAB>path[|path...]`, where a lone `-` means abstain.
///
/// Comments and blank lines are skipped rather than parsed leniently, because a gold
/// file that silently drops a malformed row reports a smaller denominator and a
/// better score, which is the one failure mode a measuring instrument must not have.
pub fn read_gold(path: &Path) -> Result<Vec<(String, Vec<String>)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((question, answers)) = line.split_once('\t') else {
            return Err(format!("{}:{}: no tab, so this row has no answer", path.display(), n + 1));
        };
        let answers = if answers.trim() == "-" {
            Vec::new()
        } else {
            answers.split('|').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()
        };
        out.push((question.trim().to_string(), answers));
    }
    if out.is_empty() {
        return Err(format!("{} has no gradeable rows", path.display()));
    }
    Ok(out)
}

/// Every gold answer that names a file the fleet does not have.
///
/// **This check exists because the alternative already happened.** The decision
/// records moved out of the architect agent into a base of their own, twelve gold
/// answers kept pointing at the old location, and nothing noticed. Running the set in
/// that state would have scored twelve correct answers as misses and produced a
/// confident, precise, entirely wrong verdict on the router.
pub fn stale_answers(rows: &[(String, Vec<String>)], memory: &Memory) -> Vec<String> {
    let mut stale = Vec::new();
    for (_, answers) in rows {
        for answer in answers {
            let Some((agent, rel)) = answer.split_once('/') else {
                stale.push(format!("{answer} (not in agent/path form)"));
                continue;
            };
            let known = memory
                .agents
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case(agent))
                .map(|a| a.root.join(rel).exists())
                .unwrap_or(false);
            if !known && !stale.contains(answer) {
                stale.push(answer.clone());
            }
        }
    }
    stale
}

pub fn run(memory: &Memory, rows: &[(String, Vec<String>)], top: usize) -> Vec<Row> {
    let routable: Vec<String> =
        memory.agents.iter().filter(|a| a.routable).map(|a| a.name.clone()).collect();
    let _ = &routable;
    rows.iter()
        .map(|(question, answers)| {
            // The whole call, timed as a caller would pay for it. `ask` does one
            // query expansion and both scorers; measuring the pieces separately and
            // adding them up would price a pipeline nobody runs.
            let started = Instant::now();
            let answer = memory.ask(question, top);
            let micros = started.elapsed().as_micros();

            // The keyword half on its own, because dropping full text is a real
            // option to price: it is the cheaper router and, at agent level, the
            // more accurate one.
            let kw_started = Instant::now();
            let keyword = memory.route(question, top);
            let keyword_agent = memory.choose_agent_by_keyword(&keyword);
            let keyword_micros = kw_started.elapsed().as_micros();

            Row {
                question: question.clone(),
                answers: answers.clone(),
                top: answer.found.first().map(|f| format!("{}/{}", f.base, f.path)),
                keyword_top: answer.keyword_top.clone(),
                agent: memory.choose_agent(&answer.found),
                keyword_agent,
                confidence: answer.confidence,
                micros,
                keyword_micros,
                routable: memory
                    .agents
                    .iter()
                    .filter(|a| a.routable)
                    .map(|a| a.name.clone())
                    .collect(),
            }
        })
        .collect()
}

/// The agent that answers the most questions in the gold set, which is the best a
/// fixed choice can possibly do. Ties break alphabetically so the number is stable
/// across runs rather than depending on hash order.
pub fn best_fixed_agent(rows: &[Row]) -> Option<(String, usize)> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows.iter().filter(|r| !r.expects_abstention() && !r.has_no_routable_owner()) {
        for agent in row.correct_agents() {
            *counts.entry(agent).or_insert(0) += 1;
        }
    }
    counts.into_iter().max_by_key(|(name, n)| (*n, std::cmp::Reverse(*name))).map(|(n, c)| (n.to_string(), c))
}

pub struct Summary {
    pub answerable: usize,
    pub file_hits: usize,
    pub keyword_hits: usize,
    pub agent_hits: usize,
    pub keyword_agent_hits: usize,
    /// The agent-level denominator: answerable questions that have an owner at all.
    pub ownable: usize,
    pub baseline_agent: String,
    pub baseline_hits: usize,
    /// Questions the router got wrong and also flagged as a guess. The number that
    /// decides whether the gate is worth having.
    pub misses_flagged: usize,
    pub misses_total: usize,
    /// Correct answers the gate demoted to a guess. The cost side of the same trade,
    /// and the revisit trigger for `MIN_MARGIN` is the first one of these.
    pub hits_demoted: usize,
    /// Denominator for `hits_demoted`: keyword hits, since that is what the gate judges.
    pub gate_denominator: usize,
    pub abstention_correct: bool,
    pub abstention_expected: bool,
    pub hit_scores: Vec<f32>,
    pub miss_scores: Vec<f32>,
    pub total_micros: u128,
    pub keyword_micros: u128,
}

pub fn summarise(rows: &[Row]) -> Summary {
    let answerable: Vec<&Row> = rows.iter().filter(|r| !r.expects_abstention()).collect();
    let ownable: Vec<&&Row> =
        answerable.iter().filter(|r| !r.has_no_routable_owner()).collect();
    let (baseline_agent, baseline_hits) =
        best_fixed_agent(rows).unwrap_or_else(|| ("none".into(), 0));

    let mut hit_scores = Vec::new();
    let mut miss_scores = Vec::new();
    let (mut misses_flagged, mut hits_demoted) = (0, 0);

    // Bucketed by the keyword pick, not the fused one, because that is the ranking
    // the gate now judges. Bucketing by a different list than the one being scored is
    // how the first version of this measurement reported OVERLAPS for a floor that
    // was never being applied to those results.
    for row in &answerable {
        if row.keyword_hit() {
            hit_scores.push(row.confidence.keyword_score);
            if row.confidence.verdict != Verdict::Hit {
                hits_demoted += 1;
            }
        } else {
            miss_scores.push(row.confidence.keyword_score);
            if row.confidence.verdict != Verdict::Hit {
                misses_flagged += 1;
            }
        }
    }

    let abstention = rows.iter().find(|r| r.expects_abstention());

    Summary {
        answerable: answerable.len(),
        file_hits: answerable.iter().filter(|r| r.file_hit()).count(),
        keyword_hits: answerable.iter().filter(|r| r.keyword_hit()).count(),
        agent_hits: ownable.iter().filter(|r| r.agent_hit()).count(),
        keyword_agent_hits: ownable.iter().filter(|r| r.keyword_agent_hit()).count(),
        ownable: ownable.len(),
        baseline_agent,
        baseline_hits,
        misses_flagged,
        misses_total: miss_scores.len(),
        hits_demoted,
        gate_denominator: hit_scores.len(),
        abstention_correct: abstention
            .map(|r| r.confidence.verdict != Verdict::Hit)
            .unwrap_or(false),
        abstention_expected: abstention.is_some(),
        hit_scores,
        miss_scores,
        total_micros: rows.iter().map(|r| r.micros).sum(),
        keyword_micros: rows.iter().map(|r| r.keyword_micros).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(question: &str, answers: &[&str], top: Option<&str>, agent: Option<&str>) -> Row {
        Row {
            question: question.into(),
            answers: answers.iter().map(|a| a.to_string()).collect(),
            top: top.map(|t| t.to_string()),
            keyword_top: top.map(|t| t.to_string()),
            agent: agent.map(|a| AgentChoice {
                agent: a.into(),
                score: 1.0,
                files: 1,
                margin: 2.0,
                contenders: 2,
                totals: Vec::new(),
            }),
            keyword_agent: agent.map(|a| AgentChoice {
                agent: a.into(),
                score: 1.0,
                files: 1,
                margin: 2.0,
                contenders: 2,
                totals: Vec::new(),
            }),
            confidence: Confidence {
                verdict: Verdict::Hit,
                agreement: 2,
                keyword_score: 10.0,
                margin: 2.0,
            },
            micros: 100,
            keyword_micros: 40,
            routable: Vec::new(),
        }
    }

    #[test]
    fn any_acceptable_answer_counts_as_a_file_hit() {
        let r = row("q", &["zed/a.md", "decisions/b.md"], Some("decisions/b.md"), None);
        assert!(r.file_hit(), "the gold set lists alternatives, not one blessed path");
    }

    /// Three questions in the real set are answerable from either base. Grading them
    /// against one arbitrary agent would invent failures the router did not commit.
    #[test]
    fn a_question_answerable_from_two_bases_accepts_either_agent() {
        let r = row("q", &["zed/a.md", "decisions/b.md"], None, Some("decisions"));
        assert_eq!(r.correct_agents(), vec!["zed", "decisions"]);
        assert!(r.agent_hit());
    }

    #[test]
    fn an_abstain_row_is_graded_not_skipped() {
        let mut r = row("q", &[], None, None);
        r.confidence.verdict = Verdict::Nothing;
        assert!(r.expects_abstention());
        let s = summarise(&[r]);
        assert_eq!(s.answerable, 0, "it is not part of the accuracy denominator");
        assert!(s.abstention_expected && s.abstention_correct, "but it is still graded");
    }

    /// The baseline has to be the strongest fixed choice available, not a convenient
    /// one, or the comparison the whole command exists for is rigged.
    #[test]
    fn the_baseline_is_the_agent_owning_the_most_questions() {
        let rows = vec![
            row("a", &["zed/x.md"], None, None),
            row("b", &["zed/y.md"], None, None),
            row("c", &["yaron/z.md"], None, None),
        ];
        let (agent, hits) = best_fixed_agent(&rows).expect("a baseline exists");
        assert_eq!(agent, "zed");
        assert_eq!(hits, 2);
    }

    #[test]
    fn a_gold_row_without_a_tab_is_an_error_and_not_a_skipped_line() {
        let dir = std::env::temp_dir().join(format!("kb-eval-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let p = dir.join("bad.tsv");
        std::fs::write(&p, "# comment\nquestion with no answer\n").expect("write");
        assert!(read_gold(&p).is_err(), "a silently dropped row shrinks the denominator");
    }

    #[test]
    fn comments_and_blank_lines_are_not_rows() {
        let dir = std::env::temp_dir().join(format!("kb-eval-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let p = dir.join("good.tsv");
        std::fs::write(&p, "# header\n\nq1\tzed/a.md|zed/b.md\nq2\t-\n").expect("write");
        let rows = read_gold(&p).expect("parses");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1, vec!["zed/a.md", "zed/b.md"]);
        assert!(rows[1].1.is_empty(), "a dash is abstain, not an answer named dash");
    }
}
