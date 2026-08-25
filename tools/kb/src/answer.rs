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

/// The three table sizes, because one default lied by omission on aggregation.
///
/// LongMemEval's multi-session split measured it: with the answer surface reading five
/// files, questions whose answer is crumbs across a dozen sessions scored 18 percent,
/// not because retrieval ranked wrong files but because most of the right ones never
/// reached the table. A personal fleet's common question has one owner and wants the
/// small fast table; an aggregation question wants a bigger one, and an exhaustive one
/// wants the whole base read. Three modes, chosen by the caller, never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The default: top files, whole passages, one model call. The librarian's answer.
    Fast,
    /// The bigger table: up to twelve files, one call. For questions whose evidence
    /// spreads across several files but still fits one reading.
    Expanded,
    /// Every file in the base, read in batches (map), then composed (reduce). The
    /// detective's answer, and the caller is warned it costs one model call per batch
    /// plus one, with the estimate printed before anything runs.
    Complete,
}

impl Mode {
    pub fn files(self) -> usize {
        match self {
            Mode::Fast => 5,
            Mode::Expanded => 12,
            Mode::Complete => usize::MAX,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Fast => "fast",
            Mode::Expanded => "expanded",
            Mode::Complete => "complete",
        }
    }
}

/// Files per map batch in complete mode. Sized so a batch of chunked notes stays well
/// inside any model's context with room to answer; the estimate the caller prints is
/// derived from this.
pub const BATCH: usize = 10;

/// The prompt, assembled from retrieval's output and nothing else.
///
/// The model is told what the librarian knows: which files answered, how confidently,
/// and what the pages actually say. It is not given the fleet, the question's history,
/// or a license to know things; the grounding rule is stated as a hard instruction and
/// the caller prints the sources itself, so a fabricated citation has nowhere to hide.
pub fn prompt(question: &str, answer: &Answer, mode: Mode) -> String {
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
    for f in answer.found.iter().take(mode.files()) {
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
pub fn sources_line(answer: &Answer, mode: Mode) -> String {
    let mut out = String::from("sources served:");
    for f in answer.found.iter().take(mode.files()) {
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

/// Complete mode: the whole base, read for real.
///
/// Two stages. **Map**: every markdown file the fleet serves, in batches of [`BATCH`],
/// each batch handed to the model with one job: list the facts relevant to the
/// question, one line each, citing the file after each fact, or the word NONE. The
/// question travels with every batch, so relevance is judged against it, not guessed.
/// **Reduce**: the surviving fact lines, composed into an answer under the same
/// grounding rules as every other mode. Facts arrive pre-cited, so the reduce step
/// inherits its citations instead of inventing them.
///
/// This is the aggregation answer the fast table cannot give: "how many times did X
/// happen" is crumbs across many files, and a top-k table starves it by construction.
pub struct CompletePlan {
    pub files: Vec<(String, std::path::PathBuf)>,
    pub batches: usize,
}

/// What complete mode is about to cost, computed before anything runs, so every
/// surface can warn: the UI puts it on screen, and a CLI or MCP caller gets it as the
/// first line of output, because the model reading that output deserves the same
/// warning a person gets.
pub fn complete_plan(memory: &crate::memory::Memory) -> CompletePlan {
    let mut files = Vec::new();
    for agent in &memory.agents {
        if let Ok(base) = crate::base::Base::discover(&agent.root, true) {
            for f in &base.files {
                let (keys, _) = crate::index::header_of(&f.text);
                if !keys.is_empty() {
                    files.push((format!("{}/{}", agent.name, f.rel), agent.root.join(&f.rel)));
                }
            }
        }
    }
    let batches = files.len().div_ceil(BATCH);
    CompletePlan { files, batches }
}

/// One map batch's prompt.
pub fn map_prompt(question: &str, batch: &[(String, String)]) -> String {
    let mut out = String::from(
        "Extract facts relevant to ONE question from the files below. Rules:\n\
         - One fact per line, followed by the file path in parentheses.\n\
         - Only what the files literally state. No inference across files, no outside \
           knowledge.\n\
         - If nothing in these files bears on the question, answer exactly: NONE\n\n",
    );
    for (name, text) in batch {
        out.push_str(&format!("--- {name}\n{text}\n\n"));
    }
    out.push_str(&format!("THE QUESTION:\n{question}\n"));
    out
}

/// The reduce prompt over the collected fact lines.
pub fn reduce_prompt(question: &str, facts: &str) -> String {
    format!(
        "Answer ONE question using ONLY the fact lines below, which were extracted from \
         a personal knowledge library and carry their source file in parentheses. Hard \
         rules:\n\
         - Every claim cites its file, carried over from the fact line.\n\
         - Aggregate honestly: if the question asks how many or which ones, count and \
           list from the facts, and say if the facts look incomplete.\n\
         - If the facts do not hold the answer, say so plainly. \"The library does not \
           hold this\" is a correct and complete answer.\n\
         - Answer in the language of the question, briefly.\n\n\
         THE FACTS:\n{facts}\n\nTHE QUESTION:\n{question}\n"
    )
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
    fn the_map_stage_carries_the_question_and_demands_citations() {
        let p = map_prompt(
            "quantas vezes fui ao medico",
            &[("history/memory/001.md".into(), "fui ao medico em marco".into())],
        );
        assert!(p.contains("THE QUESTION"));
        assert!(p.contains("history/memory/001.md"));
        assert!(p.contains("NONE"), "an empty batch has an explicit empty answer");
    }

    #[test]
    fn the_reduce_stage_keeps_the_refusal_and_the_honest_count() {
        let p = reduce_prompt("how many visits", "visited in march (a.md)");
        assert!(p.contains("The library does not\n         hold this")
            || p.contains("The library does not hold this"));
        assert!(p.contains("say if the facts look incomplete"));
    }

    #[test]
    fn the_refusal_instruction_is_in_every_prompt() {
        // The abstention property must survive the answering surface, and it survives
        // as an instruction the model cannot miss plus a verdict line in front of it.
        let a = answer_with(vec![hit("zed", "knowledge/x.md", "body")], Verdict::Guess, 9.0);
        let p = prompt("qualquer coisa", &a, Mode::Fast);
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
        let p = prompt("how much protein", &a, Mode::Fast);
        assert!(p.contains("yaron/knowledge/protein-basics.md"));
        assert!(p.contains("1.6 to 2.2 g per kg"));
        assert!(
            sources_line(&a, Mode::Fast).contains("yaron/knowledge/protein-basics.md"),
            "the caller can check citations against what was served"
        );
    }
}
