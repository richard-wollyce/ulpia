//! The answering surface: a model reads what retrieval found, and only that.
//!
//! ## Where this sits, and where it deliberately does not
//!
//! ADR-0018 keeps models out of the retrieval path, and this module does not touch that
//! line: retrieval runs first, deterministic, and produces its ranked passages and its
//! verdict. What this adds is the step AFTER the verdict, for callers who want prose
//! instead of a reading list: a model receives the question, the passages, and the
//! gate's own evidence, and writes an answer grounded in them. Same process contract as
//! the classifier and the promoters (ADR-0027): a command from the manifest, prompt on
//! stdin, text on stdout, and when the command is absent or fails the caller gets the
//! reading list it would have gotten anyway. The fleet never stops answering because a
//! model was missing.
//!
//! ## The refusal carries through, which is the whole point
//!
//! The abstention benchmark measured the deterministic layer refusing 28 of 30
//! out-of-scope questions. An answering surface that papered over that with fluent
//! prose would spend the system's one differentiating property. So: a `Nothing` verdict
//! never reaches the model at all, and the prompt orders the model to say plainly when
//! the passages do not hold the answer, with the evidence line in front of it so a low
//! score arrives labelled. The instruction is not a vibe, it is what makes LongMemEval's
//! abstention split measurable end to end.

use crate::memory::{Answer, Confidence, Verdict, SCORE_FLOOR};
use crate::retrieve::Retrieved;

/// How many passages the model reads. Few and whole beats many and clipped: the chunker
/// already bounds a passage, and five whole ones fit any modern context with room for
/// the answer.
const PASSAGES: usize = 5;

/// The prompt, assembled from retrieval's output and nothing else.
///
/// The model is told what the librarian knows: which files answered, how confidently,
/// and what the pages actually say. It is not given the fleet, the question's history,
/// or a license to know things; the grounding rule is stated as a hard instruction and
/// the caller prints the sources itself, so a fabricated citation has nowhere to hide.
pub fn prompt(question: &str, answer: &Answer) -> String {
    let mut out = String::new();
    out.push_str(
        "You answer ONE question from a personal knowledge library, using ONLY the \
         passages below. Hard rules:\n\
         - Every claim in your answer must be supported by a passage. No outside \
           knowledge, no filling gaps, however obvious the gap.\n\
         - If the passages do not hold the answer, or hold only part of it, say so \
           plainly and say what is missing. \"The library does not hold this\" is a \
           correct and complete answer.\n\
         - Cite the file path after the claim it supports, in parentheses.\n\
         - Answer in the language the question was asked in. Be brief: the reader \
           asked a question, not for a report.\n\n",
    );

    out.push_str(&format!(
        "WHAT RETRIEVAL THINKS. Top keyword score {:.1} against a floor of {:.1}; \
         verdict: {}. Below the floor, treat every passage as a lead, not an answer.\n\n",
        answer.confidence.keyword_score,
        SCORE_FLOOR,
        match answer.confidence.verdict {
            Verdict::Hit => "something here matches",
            Verdict::Guess => "a guess; the match may be a coincidence of vocabulary",
            Verdict::Nothing => "nothing matched",
        }
    ));

    out.push_str("THE PASSAGES:\n");
    for f in answer.found.iter().take(PASSAGES) {
        for p in f.passages.iter().take(2) {
            out.push_str(&format!(
                "\n--- {}/{} ({})\n{}\n",
                f.base,
                f.path,
                if p.heading_path.is_empty() { "top" } else { &p.heading_path },
                p.text.trim()
            ));
        }
        if f.passages.is_empty() && !f.purpose.is_empty() {
            // A keyword-only hit carries no chunk; its purpose line is still evidence
            // of what the file is for, and the model should ask for the file rather
            // than invent its contents.
            out.push_str(&format!(
                "\n--- {}/{} (no passage retrieved; the file exists to: {})\n",
                f.base, f.path, f.purpose
            ));
        }
    }

    out.push_str(&format!("\nTHE QUESTION:\n{}\n", question.replace('\n', " ")));
    out
}

/// The line every caller prints under a model answer, so the citations can be checked
/// against what retrieval actually served rather than taken on the model's word.
pub fn sources_line(answer: &Answer) -> String {
    let mut out = String::from("sources served:");
    for f in answer.found.iter().take(PASSAGES) {
        out.push_str(&format!(" {}/{}", f.base, f.path));
    }
    out
}

/// Whether the question should reach a model at all.
///
/// `Nothing` short-circuits: there are no passages to ground an answer in, and sending
/// a model to answer from nothing is how fluent fabrication happens. The caller prints
/// the same refusal `kb route` prints, with the suggestion list, and spends zero model
/// calls doing it.
pub fn worth_asking(confidence: &Confidence, found: &[Retrieved]) -> bool {
    confidence.verdict != Verdict::Nothing && !found.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::AgentChoice;

    fn answer_with(found: Vec<Retrieved>, verdict: Verdict, score: f32) -> Answer {
        let _: Option<AgentChoice> = None;
        Answer {
            found,
            confidence: Confidence { verdict, agreement: 1, keyword_score: score, margin: 1.0 },
            agent: None,
            keyword_top: None,
        }
    }

    fn hit(base: &str, path: &str, text: &str) -> Retrieved {
        Retrieved {
            base: base.into(),
            path: path.into(),
            title: String::new(),
            purpose: String::new(),
            score: 1.0,
            keyword_score: 20.0,
            why: vec!["keywords #1".into()],
            matched: vec![],
            passages: vec![crate::retrieve::Passage {
                heading_path: "H".into(),
                text: text.into(),
                excerpt: String::new(),
                provenance: None,
                stage: None,
            }],
        }
    }

    #[test]
    fn the_refusal_instruction_is_in_every_prompt() {
        // The abstention property must survive the answering surface, and it survives
        // as an instruction the model cannot miss plus a verdict line in front of it.
        let a = answer_with(vec![hit("zed", "knowledge/x.md", "body")], Verdict::Guess, 9.0);
        let p = prompt("qualquer coisa", &a);
        assert!(p.contains("The library does not hold this"));
        assert!(p.contains("a guess; the match may be a coincidence"));
        assert!(p.contains("treat every passage as a lead"));
    }

    #[test]
    fn nothing_never_reaches_a_model() {
        let a = answer_with(vec![], Verdict::Nothing, 0.0);
        assert!(!worth_asking(&a.confidence, &a.found), "no passages, no model call");
    }

    #[test]
    fn the_prompt_carries_only_what_retrieval_served() {
        let a = answer_with(
            vec![hit("yaron", "knowledge/protein-basics.md", "1.6 to 2.2 g per kg")],
            Verdict::Hit,
            60.0,
        );
        let p = prompt("how much protein", &a);
        assert!(p.contains("yaron/knowledge/protein-basics.md"));
        assert!(p.contains("1.6 to 2.2 g per kg"));
        assert!(
            sources_line(&a).contains("yaron/knowledge/protein-basics.md"),
            "the caller can check citations against what was served"
        );
    }
}
