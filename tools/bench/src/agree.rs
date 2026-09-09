//! Judge validation: two independent judges over the same hypotheses, and the three
//! numbers that make a benchmark score auditable instead of merely stated.
//!
//! **Why this file exists.** `benchmarks/longmemeval/RESULTS.md` declares its judge as
//! non-official and validates it against nothing, which means every score in it rests on
//! one unvalidated grader. Graphify publishes 90.6% agreement at Cohen's kappa 0.81
//! between its judge and a second independent one, and that is the whole difference
//! between a number and an auditable number.
//!
//! **Raw agreement alone is the trap, and it is why kappa is here.** On a benchmark where
//! one label dominates, two judges that agree 90% of the time can be agreeing almost
//! entirely by accident: if both say "correct" 90% of the time at random, chance
//! agreement is already 0.82. Cohen's kappa subtracts exactly that, and the subtraction
//! is what the disagreement report then makes readable.

use std::collections::BTreeMap;

/// One judge's verdict on one question.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub question_id: String,
    pub question_type: String,
    /// Whether the question is one of LongMemEval's unanswerable `_abs` instances, which
    /// are judged under a different rubric and are the ability this system exists for.
    pub abstention: bool,
    pub correct: bool,
}

/// The 2x2 table of two judges over the questions both of them graded.
///
/// **Named by judge and verdict rather than by `a`, `b`, `c`, `d`**, because a confusion
/// matrix whose cells are positional is a matrix somebody will read transposed.
#[derive(Debug, Default, PartialEq)]
pub struct Table {
    pub both_correct: usize,
    pub both_wrong: usize,
    /// The primary judge said correct and the second said wrong.
    pub primary_only: usize,
    /// The second judge said correct and the primary said wrong.
    pub second_only: usize,
}

impl Table {
    pub fn n(&self) -> usize {
        self.both_correct + self.both_wrong + self.primary_only + self.second_only
    }

    /// The share of questions the two judges labelled the same way.
    pub fn agreement(&self) -> f64 {
        match self.n() {
            0 => 0.0,
            n => (self.both_correct + self.both_wrong) as f64 / n as f64,
        }
    }

    /// Cohen's kappa: agreement above what the two judges' own answer rates would produce
    /// by chance.
    ///
    /// `(p0 - pe) / (1 - pe)`, where `pe` is the sum over labels of the product of the two
    /// judges' marginal rates for that label.
    ///
    /// **`None` when it is undefined, which is the case that matters.** If both judges give
    /// every question the same single label, `pe` is 1 and the formula divides by zero.
    /// That is not perfect agreement worth reporting as 1.0; it is a table with no
    /// variance, where kappa has nothing to measure. Returning a number there is how an
    /// implementation with no test publishes 1.0 for a judge pair that never disagreed
    /// because it never had the chance to.
    pub fn kappa(&self) -> Option<f64> {
        let n = self.n();
        if n == 0 {
            return None;
        }
        let n = n as f64;
        let p0 = self.agreement();

        let primary_correct = (self.both_correct + self.primary_only) as f64 / n;
        let second_correct = (self.both_correct + self.second_only) as f64 / n;
        let pe = primary_correct * second_correct
            + (1.0 - primary_correct) * (1.0 - second_correct);

        // Exactly 1.0 only when a marginal is degenerate, so the comparison is against
        // the float rather than against an epsilon somebody would have to justify.
        if (1.0 - pe).abs() < f64::EPSILON {
            return None;
        }
        Some((p0 - pe) / (1.0 - pe))
    }
}

/// Pairs two label sets by `question_id` and builds the table.
///
/// **Questions only one judge graded are dropped and counted, never guessed at.** A judge
/// call that failed to parse is missing evidence, and scoring it as a disagreement would
/// make an unreliable transport look like an unreliable judge.
pub fn pair(primary: &[Label], second: &[Label]) -> (Table, Vec<String>) {
    let by_id: BTreeMap<&str, &Label> =
        second.iter().map(|l| (l.question_id.as_str(), l)).collect();

    let mut table = Table::default();
    let mut unpaired = Vec::new();

    for p in primary {
        match by_id.get(p.question_id.as_str()) {
            Some(s) => match (p.correct, s.correct) {
                (true, true) => table.both_correct += 1,
                (false, false) => table.both_wrong += 1,
                (true, false) => table.primary_only += 1,
                (false, true) => table.second_only += 1,
            },
            None => unpaired.push(p.question_id.clone()),
        }
    }

    let seen: BTreeMap<&str, ()> = primary.iter().map(|l| (l.question_id.as_str(), ())).collect();
    for s in second {
        if !seen.contains_key(s.question_id.as_str()) {
            unpaired.push(s.question_id.clone());
        }
    }
    unpaired.sort();
    unpaired.dedup();

    (table, unpaired)
}

/// Every question the two judges labelled differently, in id order.
///
/// **The list is the deliverable, not a debugging aid.** A kappa is one number and it
/// cannot say whether the disagreements cluster on one ability. If they all sit on
/// `_abs`, the abstention score, which is the number this benchmark leads with, is the
/// one resting on the shakiest grading.
pub fn disagreements<'a>(primary: &'a [Label], second: &[Label]) -> Vec<&'a Label> {
    let by_id: BTreeMap<&str, &Label> =
        second.iter().map(|l| (l.question_id.as_str(), l)).collect();
    let mut out: Vec<&Label> = primary
        .iter()
        .filter(|p| {
            by_id
                .get(p.question_id.as_str())
                .is_some_and(|s| s.correct != p.correct)
        })
        .collect();
    out.sort_by(|a, b| a.question_id.cmp(&b.question_id));
    out
}

/// How the disagreements fall across the abilities, so a cluster is visible.
///
/// Returns `ability -> (disagreements, questions both judges graded)`.
pub fn by_ability(primary: &[Label], second: &[Label]) -> BTreeMap<String, (usize, usize)> {
    let by_id: BTreeMap<&str, &Label> =
        second.iter().map(|l| (l.question_id.as_str(), l)).collect();
    let mut out: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for p in primary {
        let Some(s) = by_id.get(p.question_id.as_str()) else { continue };
        // Abstention is a question_id suffix in the dataset, not a question_type, so a
        // report keyed on type alone would scatter the one ability that matters most.
        let key = if p.abstention { "abstention".to_string() } else { p.question_type.clone() };
        let e = out.entry(key).or_default();
        e.1 += 1;
        if s.correct != p.correct {
            e.0 += 1;
        }
    }
    out
}

/// Landis and Koch's bands, named so a kappa is read rather than admired.
///
/// They are a convention and not a law, which is why the number is always printed next
/// to the word.
pub fn strength(kappa: f64) -> &'static str {
    match kappa {
        k if k < 0.0 => "worse than chance",
        k if k < 0.20 => "slight",
        k if k < 0.40 => "fair",
        k if k < 0.60 => "moderate",
        k if k < 0.80 => "substantial",
        _ => "almost perfect",
    }
}

/// Reads a labels file written by `kb-bench judge`.
///
/// **A row whose verdict is `null` is dropped, not defaulted.** That is the judge saying
/// it produced no parseable answer, and turning it into `false` would manufacture a
/// disagreement out of a failed call, which is the one error that makes a validation run
/// worse than no validation run.
pub fn load(path: &std::path::Path) -> Result<(Vec<Label>, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    let mut unparsed = 0usize;

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let Some(id) = v["question_id"].as_str() else { continue };
        match v["correct"].as_bool() {
            Some(correct) => out.push(Label {
                question_id: id.to_string(),
                question_type: v["question_type"].as_str().unwrap_or("unknown").to_string(),
                abstention: v["abstention"].as_bool().unwrap_or(id.ends_with("_abs")),
                correct,
            }),
            None => unparsed += 1,
        }
    }
    Ok((out, unparsed))
}

/// The validation report: the table, the two numbers, the split, and the disagreements.
///
/// **Written as markdown to stdout rather than to a file** so that the run that produced
/// it is the thing that publishes it, and a report cannot silently be a stale file from
/// an earlier judge.
pub fn report(
    primary_name: &str,
    second_name: &str,
    primary: &[Label],
    second: &[Label],
    unparsed: (usize, usize),
) -> String {
    let (table, unpaired) = pair(primary, second);
    let mut out = String::new();

    out.push_str(&format!(
        "# Judge validation: {primary_name} against {second_name}\n\n\
         | | |\n|---|---|\n\
         | questions graded by both | {} |\n\
         | raw agreement | {:.1}% |\n",
        table.n(),
        table.agreement() * 100.0
    ));
    match table.kappa() {
        Some(k) => out.push_str(&format!("| Cohen's kappa | {k:.3} ({}) |\n", strength(k))),
        None => out.push_str(
            "| Cohen's kappa | undefined: one judge used a single label, so there is no \
             variance to correct for |\n",
        ),
    }
    out.push_str(&format!(
        "| both said correct | {} |\n| both said wrong | {} |\n\
         | {primary_name} only | {} |\n| {second_name} only | {} |\n",
        table.both_correct, table.both_wrong, table.primary_only, table.second_only
    ));
    if unparsed.0 + unparsed.1 > 0 {
        out.push_str(&format!(
            "| replies that parsed as neither yes nor no | {} and {} |\n",
            unparsed.0, unparsed.1
        ));
    }
    if !unpaired.is_empty() {
        out.push_str(&format!("| graded by only one judge, dropped | {} |\n", unpaired.len()));
    }

    out.push_str("\n## Where the disagreements fall\n\n| ability | disagreements | graded |\n|---|---|---|\n");
    for (ability, (d, n)) in by_ability(primary, second) {
        out.push_str(&format!("| {ability} | {d} | {n} |\n"));
    }

    let d = disagreements(primary, second);
    out.push_str(&format!("\n## The {} disagreement(s)\n\n", d.len()));
    if d.is_empty() {
        out.push_str("None.\n");
    } else {
        out.push_str("| question_id | ability | ");
        out.push_str(primary_name);
        out.push_str(" |\n|---|---|---|\n");
        for l in d {
            let ability = if l.abstention { "abstention" } else { &l.question_type };
            out.push_str(&format!(
                "| `{}` | {ability} | {} |\n",
                l.question_id,
                if l.correct { "correct" } else { "wrong" }
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn a_null_verdict_is_dropped_and_counted_rather_than_read_as_wrong() {
        let dir = std::env::temp_dir().join(format!("kb-agree-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("labels.jsonl");
        std::fs::write(
            &p,
            "{\"question_id\":\"q1\",\"question_type\":\"multi-session\",\"abstention\":false,\"correct\":true}\n\
             {\"question_id\":\"q2\",\"question_type\":\"multi-session\",\"abstention\":false,\"correct\":null}\n",
        )
        .unwrap();

        let (labels, unparsed) = load(&p).expect("labels");
        assert_eq!(labels.len(), 1, "the unparsed verdict is not a `wrong`");
        assert_eq!(unparsed, 1);
        assert_eq!(labels[0].question_id, "q1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_report_carries_the_numbers_and_names_every_disagreement() {
        let a = vec![
            label("q1_abs", "multi-session", true),
            label("q2", "temporal-reasoning", true),
            label("q3", "temporal-reasoning", false),
        ];
        let b = vec![
            label("q1_abs", "multi-session", false),
            label("q2", "temporal-reasoning", true),
            label("q3", "temporal-reasoning", false),
        ];
        let r = report("haiku", "sonnet", &a, &b, (0, 0));

        assert!(r.contains("66.7%"), "raw agreement is in the report: {r}");
        assert!(r.contains("Cohen's kappa"), "kappa is in the report");
        assert!(r.contains("`q1_abs`"), "the disagreeing question is named: {r}");
        assert!(!r.contains("`q2`"), "an agreed question is not listed as a disagreement");
        assert!(r.contains("| abstention | 1 | 1 |"), "the split names abstention: {r}");
    }

    #[test]
    fn a_report_over_a_degenerate_table_says_so_instead_of_printing_a_number() {
        let a = vec![label("q1", "multi-session", true), label("q2", "multi-session", true)];
        let b = a.clone();
        let r = report("haiku", "sonnet", &a, &b, (0, 0));
        assert!(r.contains("undefined"), "no kappa is printed for a one-label table: {r}");
        assert!(r.contains("100.0%"), "raw agreement is still reported");
    }

    fn label(id: &str, qtype: &str, correct: bool) -> Label {
        Label {
            question_id: id.to_string(),
            question_type: qtype.to_string(),
            abstention: id.ends_with("_abs"),
            correct,
        }
    }

    #[test]
    fn kappa_matches_the_worked_example() {
        // 20 both correct, 15 both wrong, 5 primary only, 10 second only, n = 50.
        // p0 = 35/50 = 0.70. Primary says correct 25/50 = 0.50, second 30/50 = 0.60.
        // pe = 0.50*0.60 + 0.50*0.40 = 0.50. kappa = (0.70-0.50)/(1-0.50) = 0.40.
        let t = Table { both_correct: 20, both_wrong: 15, primary_only: 5, second_only: 10 };
        assert_eq!(t.n(), 50);
        assert!((t.agreement() - 0.70).abs() < 1e-12, "{}", t.agreement());
        assert!((t.kappa().unwrap() - 0.40).abs() < 1e-12, "{:?}", t.kappa());
    }

    #[test]
    fn perfect_agreement_with_both_labels_present_is_one() {
        let t = Table { both_correct: 30, both_wrong: 20, primary_only: 0, second_only: 0 };
        assert!((t.kappa().unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn agreement_that_is_entirely_chance_is_zero() {
        // Both judges say correct half the time and their verdicts are independent, so
        // the table is 25/25/25/25: p0 = 0.5, pe = 0.5, kappa = 0.
        let t = Table { both_correct: 25, both_wrong: 25, primary_only: 25, second_only: 25 };
        assert!((t.agreement() - 0.5).abs() < 1e-12);
        assert!(t.kappa().unwrap().abs() < 1e-12, "{:?}", t.kappa());
    }

    #[test]
    fn a_table_with_no_variance_has_no_kappa() {
        // The trap: both judges say correct to everything. Raw agreement is 100% and
        // kappa is 0/0. An implementation with no test publishes 1.0 here.
        let t = Table { both_correct: 200, both_wrong: 0, primary_only: 0, second_only: 0 };
        assert_eq!(t.agreement(), 1.0);
        assert_eq!(t.kappa(), None, "kappa is undefined when a marginal is degenerate");

        let empty = Table::default();
        assert_eq!(empty.kappa(), None);
    }

    #[test]
    fn high_agreement_on_a_lopsided_benchmark_can_still_be_a_weak_kappa() {
        // The reason raw agreement is not enough, as a number. 90% agreement, but both
        // judges say correct about 90% of the time, so chance alone explains most of it.
        let t = Table { both_correct: 85, both_wrong: 5, primary_only: 5, second_only: 5 };
        assert!((t.agreement() - 0.90).abs() < 1e-12);
        // pe = 0.9*0.9 + 0.1*0.1 = 0.82, so kappa = (0.90-0.82)/0.18 = 0.444.
        let k = t.kappa().unwrap();
        assert!((k - 0.4444).abs() < 1e-3, "{k}");
        assert_eq!(strength(k), "moderate", "90% agreement is not substantial agreement");
    }

    #[test]
    fn kappa_can_go_below_zero() {
        let t = Table { both_correct: 5, both_wrong: 5, primary_only: 45, second_only: 45 };
        let k = t.kappa().unwrap();
        assert!(k < 0.0, "systematic disagreement is worse than chance: {k}");
        assert_eq!(strength(k), "worse than chance");
    }

    #[test]
    fn pairing_is_by_id_and_not_by_position() {
        let a = vec![label("q1", "multi-session", true), label("q2", "multi-session", false)];
        let b = vec![label("q2", "multi-session", false), label("q1", "multi-session", true)];
        let (t, unpaired) = pair(&a, &b);
        assert_eq!(t, Table { both_correct: 1, both_wrong: 1, primary_only: 0, second_only: 0 });
        assert!(unpaired.is_empty());
    }

    #[test]
    fn a_question_only_one_judge_graded_is_dropped_and_named() {
        let a = vec![label("q1", "temporal-reasoning", true), label("q2", "temporal-reasoning", true)];
        let b = vec![label("q1", "temporal-reasoning", false), label("q3", "temporal-reasoning", true)];
        let (t, unpaired) = pair(&a, &b);
        assert_eq!(t.n(), 1, "only q1 was graded by both");
        assert_eq!(t.primary_only, 1);
        assert_eq!(unpaired, vec!["q2".to_string(), "q3".to_string()]);
    }

    #[test]
    fn the_disagreements_are_listed_in_id_order() {
        let a = vec![
            label("q3", "knowledge-update", true),
            label("q1", "multi-session", true),
            label("q2", "multi-session", false),
        ];
        let b = vec![
            label("q1", "multi-session", false),
            label("q2", "multi-session", false),
            label("q3", "knowledge-update", false),
        ];
        let d = disagreements(&a, &b);
        assert_eq!(
            d.iter().map(|l| l.question_id.as_str()).collect::<Vec<_>>(),
            vec!["q1", "q3"]
        );
    }

    #[test]
    fn the_split_by_ability_puts_abstention_in_its_own_row() {
        // The dataset marks unanswerable questions by an id suffix, not by type, so a
        // report keyed on type alone would hide the ability the benchmark leads with.
        let a = vec![
            label("q1_abs", "multi-session", true),
            label("q2_abs", "multi-session", true),
            label("q3", "multi-session", true),
        ];
        let b = vec![
            label("q1_abs", "multi-session", false),
            label("q2_abs", "multi-session", true),
            label("q3", "multi-session", true),
        ];
        let split = by_ability(&a, &b);
        assert_eq!(split.get("abstention"), Some(&(1, 2)));
        assert_eq!(split.get("multi-session"), Some(&(0, 1)));
    }
}
