//! The write side: measuring a claim against what the base already says.
//!
//! ADR-0007 gives every write four possible outcomes, ADD, UPDATE, DELETE and NOOP.
//! **This module decides none of them.** It measures, it shows the evidence, and it
//! proposes. The agent decides.
//!
//! That split is the whole point. mem0 lets a similarity score decide, and a
//! similarity score cannot tell "the user changed their mind" from "two things are
//! true in different contexts", so it deletes live facts in silence. A number can say
//! how much two texts overlap. It cannot say whether the older one is now wrong.
//! Pretending otherwise is how a memory layer starts lying confidently.

use crate::index::normalise;
use crate::store::Hit;
use std::collections::HashSet;

/// Containment above this and the claim adds no words the base does not already
/// have, in one chunk. That is what "already known" can mean without judgement.
const NOOP_AT: f64 = 0.90;

/// Above this the claim substantially overlaps something already written, which is
/// what a changed fact looks like: the same sentence with a different number.
const UPDATE_AT: f64 = 0.55;

#[derive(PartialEq, Debug)]
pub enum Outcome {
    Noop,
    Update,
    Add,
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Noop => "NOOP",
            Outcome::Update => "UPDATE",
            Outcome::Add => "ADD",
        }
    }
}

pub struct Evidence {
    pub base: String,
    pub path: String,
    pub heading_path: String,
    pub excerpt: String,
    pub containment: f64,
    pub shared: Vec<String>,
    pub missing: Vec<String>,
}

pub struct Assessment {
    pub outcome: Outcome,
    pub reason: String,
    pub evidence: Vec<Evidence>,
}

/// The fraction of the claim's words that already appear in the chunk.
///
/// Containment rather than Jaccard, deliberately. Jaccard divides by the union, so a
/// short claim sitting inside a long section scores low no matter how completely the
/// section already covers it, which is the opposite of the question being asked. The
/// question is "does the base already say this", not "are these two texts similar".
fn containment(claim: &HashSet<String>, chunk: &HashSet<String>) -> f64 {
    if claim.is_empty() {
        return 0.0;
    }
    let shared = claim.intersection(chunk).count();
    shared as f64 / claim.len() as f64
}

pub fn assess(claim: &str, terms: &[String], hits: &[Hit]) -> Assessment {
    let claim_set: HashSet<String> = terms.iter().cloned().collect();

    let mut evidence: Vec<Evidence> = hits
        .iter()
        .map(|hit| {
            let chunk_set: HashSet<String> = normalise(&hit.text).into_iter().collect();
            let mut shared: Vec<String> =
                claim_set.intersection(&chunk_set).cloned().collect();
            let mut missing: Vec<String> =
                claim_set.difference(&chunk_set).cloned().collect();
            shared.sort();
            missing.sort();

            Evidence {
                base: hit.base.clone(),
                path: hit.path.clone(),
                heading_path: hit.heading_path.clone(),
                excerpt: hit.excerpt.clone(),
                containment: containment(&claim_set, &chunk_set),
                shared,
                missing,
            }
        })
        .collect();

    evidence.sort_by(|a, b| {
        b.containment
            .partial_cmp(&a.containment)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    evidence.truncate(5);

    let best = evidence.first();
    let top = best.map(|e| e.containment).unwrap_or(0.0);

    let (outcome, reason) = match best {
        None => (
            Outcome::Add,
            "nothing in the base matched any word of this claim.".to_string(),
        ),
        Some(e) if top >= NOOP_AT => (
            Outcome::Noop,
            format!(
                "{} of {} words are already present in one chunk. The base appears to \
                 say this already, so writing it again would be the duplicate that \
                 grows a base without adding to it.",
                e.shared.len(),
                e.shared.len() + e.missing.len()
            ),
        ),
        Some(e) if top >= UPDATE_AT => (
            Outcome::Update,
            format!(
                "{} of {} words already appear in one chunk, and the claim adds: {}. \
                 An overlap this size with a small difference is usually the same fact \
                 with a new value, which is an UPDATE rather than an ADD.",
                e.shared.len(),
                e.shared.len() + e.missing.len(),
                if e.missing.is_empty() { "nothing".to_string() } else { e.missing.join(", ") }
            ),
        ),
        Some(_) => (
            Outcome::Add,
            "the closest thing in the base shares only part of this claim, which \
             usually means related material rather than the same fact."
                .to_string(),
        ),
    };

    let _ = claim;
    Assessment { outcome, reason, evidence }
}

/// What this tool refuses to do, printed with every assessment.
///
/// Not decoration. A tool that proposes three outcomes and stays silent about the
/// fourth invites the reader to assume the fourth does not exist.
pub const DISCLAIMER: &str = "\
kb measured overlap. Whether the older text is now wrong is judgement, and so is
where a new claim belongs. DELETE is not proposed here at all: it needs the claim
to be false rather than absent, and no count of shared words can tell those apart.
The decision and its reason belong in the commit message.";

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(text: &str) -> Hit {
        Hit {
            base: "yaron".into(),
            path: "protocols/safety-and-limits.md".into(),
            heading_path: "Safety > Limits".into(),
            excerpt: text.chars().take(40).collect(),
            text: text.into(),
            provenance: None,
            stage: None,
        }
    }

    #[test]
    fn an_identical_claim_is_a_noop() {
        let text = "The calorie floor for men is 1500 kcal per day";
        let terms = normalise(text);
        let a = assess(text, &terms, &[hit(text)]);
        assert_eq!(a.outcome, Outcome::Noop);
    }

    #[test]
    fn the_same_fact_with_a_new_number_is_an_update() {
        let existing = "Calorie floor for men is 1500 kcal, below it micronutrients get hard";
        let claim = "the calorie floor for men is 1600 kcal";
        let terms = normalise(claim);
        let a = assess(claim, &terms, &[hit(existing)]);
        assert_eq!(a.outcome, Outcome::Update);
        assert!(a.evidence[0].missing.contains(&"1600".to_string()));
        assert!(a.evidence[0].shared.contains(&"calorie".to_string()));
    }

    #[test]
    fn an_unrelated_claim_is_an_add() {
        let existing = "Calorie floor for men is 1500 kcal";
        let claim = "Vulkan prefill runs at 72 tokens per second on the Iris Xe";
        let terms = normalise(claim);
        let a = assess(claim, &terms, &[hit(existing)]);
        assert_eq!(a.outcome, Outcome::Add);
    }

    #[test]
    fn nothing_found_is_an_add() {
        let claim = "something entirely new";
        let terms = normalise(claim);
        let a = assess(claim, &terms, &[]);
        assert_eq!(a.outcome, Outcome::Add);
        assert!(a.evidence.is_empty());
    }

    #[test]
    fn a_short_claim_inside_a_long_chunk_is_still_covered() {
        // The reason for containment over Jaccard: a long section that already says
        // the thing must not score low just because it says much else besides.
        let long = format!(
            "The calorie floor for men is 1500 kcal. {}",
            "Unrelated surrounding prose about training volume and sleep. ".repeat(20)
        );
        let claim = "calorie floor men 1500 kcal";
        let terms = normalise(claim);
        let a = assess(claim, &terms, &[hit(&long)]);
        assert_eq!(a.outcome, Outcome::Noop, "containment must ignore chunk length");
    }

    #[test]
    fn the_best_evidence_is_first() {
        let weak = hit("calorie counting is not for everyone");
        let strong = hit("the calorie floor for men is 1500 kcal");
        let claim = "calorie floor men 1500 kcal";
        let terms = normalise(claim);
        let a = assess(claim, &terms, &[weak, strong]);
        assert!(a.evidence[0].containment > a.evidence[1].containment);
    }
}
