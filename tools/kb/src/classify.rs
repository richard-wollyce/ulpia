//! Who should answer, decided by a model reading the evidence, not by arithmetic.
//!
//! **This is ADR-0013's own rule finally applied to the right question.** That record
//! says, in its own words, *classification is the model's job and lookup is the code's
//! job*, and it names the failure it was written against: an earlier version that
//! *classified questions and answered with strings we had written, which is code doing a
//! model's job badly*. Choosing an agent is classification. It was implemented as a sum of
//! IDF weighted keyword scores, and three days were then spent patching that sum with
//! stopword lists, alias files, an incumbent margin and a corpus share normalisation, each
//! measured and most of them removed.
//!
//! **The reason arithmetic cannot do this job, stated once.** Retrieval and routing ask
//! different questions. *Which file contains this* is lexical, and a keyword index answers
//! it exactly. *Who understands this subject* is semantic, and no count of shared words
//! answers it at all. The proof is a domain nobody has written about: ask about DevOps in a
//! fleet with no DevOps agent and every base scores zero or noise, while a reader who knows
//! only that Zed does *software architecture and building* and Steve does *marketing* can
//! place it immediately, and can also say the thing that matters most, which is that
//! **nobody here really covers it**.
//!
//! ## The split this file preserves
//!
//! Retrieval stays exactly as it was: deterministic, no model, reproducible, and the only
//! thing that reads the corpus. What reaches this file is a **dossier**: the roster, the
//! evidence retrieval found, and the question. Roughly three hundred tokens, and never the
//! base itself. So the classifier is cheap, and it cannot invent a file, because it never
//! sees the corpus and its answer is a name from a list it was given.
//!
//! ## Why the classifier is a command and not a provider
//!
//! Richard's requirement is that the system works whichever client is in front of it. So
//! the contract is a process: **dossier on stdin, verdict on stdout.** Any model behind any
//! runtime satisfies it, including a local one, and `kb` gains no dependency and no network
//! code. When no classifier is configured or the command fails, routing falls back to the
//! deterministic choice, so **the fleet never stops answering because a model was
//! unavailable.**

use std::io::Write;
use std::path::Path;

use crate::memory::{AgentChoice, Memory};
use crate::retrieve::Retrieved;

/// How the classifier is reached, from `classifier = ...` in the fleet manifest.
#[derive(Debug, Clone, PartialEq)]
pub enum Classifier {
    /// No line in the manifest: the deterministic choice stands alone.
    None,
    /// A command that reads the dossier on stdin and writes the verdict on stdout.
    Command(String),
}

/// What the classifier concluded, and the reason a caller can show a person.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The agent that should answer, when one should.
    pub owner: Option<String>,
    pub coverage: Coverage,
    /// The subject as the classifier named it. This is what a person reads when the
    /// fleet says nobody covers something, and what a new agent would be created for.
    pub subject: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// An agent's domain covers this, and the base has material.
    Covered,
    /// No agent owns the subject; the named one is merely the nearest. **This is the
    /// state the whole file exists for**, because it is the one arithmetic cannot
    /// report: a score of zero says "no match" and never says "no one here does this".
    Adjacent,
    /// Nobody, and nothing near enough to name.
    Uncovered,
}

impl Coverage {
    fn parse(s: &str) -> Coverage {
        match s.trim().to_ascii_lowercase().as_str() {
            "covered" => Coverage::Covered,
            "adjacent" => Coverage::Adjacent,
            _ => Coverage::Uncovered,
        }
    }
}

/// The prompt, assembled from the roster and the evidence.
///
/// Deliberately small and deliberately closed: the classifier is told the only names it
/// may answer with, so an invented agent is a parse failure rather than a routing error.
pub fn dossier(memory: &Memory, question: &str, found: &[Retrieved]) -> String {
    let mut out = String::new();

    out.push_str(
        "You are Vesta, the librarian of a fleet of agents. Decide who should answer one \
         message. Answer in the exact format at the end and write nothing else.\n\n\
         THE FLEET, and these names are the only ones you may choose:\n",
    );
    for agent in memory.agents.iter().filter(|a| a.routable) {
        let card = crate::fleet::card(&agent.root, "agent.txt", &agent.name);
        out.push_str(&format!(
            "  {}\n    does:  {}\n",
            agent.name.to_lowercase(),
            card.role.unwrap_or_else(|| "not declared".into())
        ));
        // The edge matters more than the role for the judgement being asked for. A
        // list of roles says what each agent does; only the edges say what none of
        // them does, and that is the answer arithmetic can never give.
        if let Some(ends) = card.ends {
            out.push_str(&format!("    stops: {ends}\n"));
        }
    }

    out.push_str(
        "\nWHAT THE LIBRARY FOUND for this message. The search is lexical, so a high score \
         means shared words and not shared meaning, and an empty list means nobody has \
         written about this yet, which is information rather than an error:\n",
    );
    if found.is_empty() {
        out.push_str("  (nothing matched)\n");
    } else {
        for f in found.iter().take(5) {
            out.push_str(&format!(
                "  {}/{}  score {:.1}  matched: {}\n",
                f.base,
                f.path,
                f.keyword_score,
                if f.matched.is_empty() { "text only".into() } else { f.matched.join(", ") }
            ));
        }
    }

    out.push_str(&format!("\nTHE MESSAGE:\n  {}\n", question.replace('\n', " ")));

    out.push_str(
        "\nJudge by what each agent does and where it stops, never by the scores. The \
         scores say which files share words with the message; they never say who \
         understands the subject. A message may belong to an agent whose base scored \
         nothing, and a high score in a base whose domain does not fit is a coincidence \
         of vocabulary.\n\n\
         COVERAGE is the field that matters and the bar for `covered` is high. Use it \
         only when the subject is plainly part of what that agent does, as its own two \
         lines describe it. If the subject merely sits near an agent's work, or needs \
         knowledge that agent has no reason to hold, answer `adjacent` and name that \
         agent as the nearest. When torn between covered and adjacent, answer adjacent.\n\n\
         Worked example, with a fleet of a marketer, a nutritionist, an architect who \
         builds software and stops before running it, and an interface designer, asked \
         about Kubernetes autoscaling: the answer is `adjacent`, owner the architect, \
         because operating systems in production is a different craft from designing and \
         building them. Answering `covered` there hides a real gap behind a confident \
         name, and the gap is the useful part: it tells the person a new agent may be \
         worth creating.\n\n\
         Reply exactly:\n\
         OWNER: <name from the list, or none>\n\
         COVERAGE: <covered|adjacent|uncovered>\n\
         SUBJECT: <two to five words naming the domain this message belongs to>\n\
         REASON: <one sentence>\n",
    );
    out
}

/// Parses the four lines, tolerantly, and refuses a name that is not on the roster.
///
/// **A model naming an agent that does not exist is the one failure that must not pass**,
/// because every surface downstream treats the owner as real. Unknown name resolves to no
/// owner, which the caller already knows how to present.
pub fn parse(reply: &str, roster: &[String]) -> Option<Verdict> {
    let field = |key: &str| -> Option<String> {
        reply.lines().find_map(|l| {
            // Models decorate. Strip the emphasis before looking for the key, and
            // again after taking the value, so `**OWNER:** Zed` reads the same as
            // `OWNER: zed`. Found by the test that asserts exactly that.
            let l = l.trim().trim_start_matches(['*', '-', '#', ' ']);
            let rest = l.strip_prefix(key)?;
            let rest = rest.trim_start_matches(['*', ' ']);
            let rest = rest.strip_prefix(':').unwrap_or(rest);
            Some(rest.trim().trim_matches('*').trim().to_string())
        })
    };

    let coverage = Coverage::parse(&field("COVERAGE").unwrap_or_default());
    let raw_owner = field("OWNER").unwrap_or_default();
    let owner = roster
        .iter()
        .find(|r| r.eq_ignore_ascii_case(raw_owner.trim()))
        .cloned();

    // A reply with no recognisable field at all is not a verdict, it is noise.
    if field("COVERAGE").is_none() && field("OWNER").is_none() {
        return None;
    }

    Some(Verdict {
        owner,
        coverage,
        subject: field("SUBJECT").unwrap_or_default(),
        reason: field("REASON").unwrap_or_default(),
    })
}

/// **Built, measured and rejected on 2026-08-19.** Kept, with its number, because the
/// idea is obvious enough that somebody will propose it again.
///
/// The cascade was going to be ADR-0013's own prescription applied to cost: route always,
/// and *spend intelligence only where the free mechanism admitted it failed*. Asking a
/// model about every message costs 13 to 16 seconds on this machine; gating on "one agent
/// holds the field" dropped the common case to about one second, measured.
///
/// It also broke the one case the classifier exists for. Asked *como faco deploy com zero
/// downtime e monitoramento de infra*, the word **zero** matched three of Steve's research
/// notes at 15.89 each. Steve was therefore the only agent scoring, held 100% of the
/// field, cleared the floor, and the gate let the arithmetic answer alone: DevOps routed
/// to marketing in 971 ms.
///
/// **The mechanism, and it is why no threshold rescues this:** a cascade can only gate on
/// the deterministic score, and the deterministic score does not know when it is wrong.
/// One agent alone in the field does not distinguish "plainly theirs" from "a coincidence
/// of vocabulary", and those two are the same number. A gate built on a blind signal
/// inherits the blindness.
///
/// So the classifier runs on every message and the latency is paid honestly. If it must
/// come down, the answer is a faster classifier (a local model, a resident process), not a
/// cheaper decision about when to think.
const UNCONTESTED: f64 = 0.70;

/// Whether the deterministic choice dominates its field. **No longer consulted**; see the
/// constant above for the measurement that took it out of the path. Kept so the rejected
/// idea has a testable definition rather than only a paragraph.
pub fn is_uncontested(choice: Option<&AgentChoice>, verdict: crate::memory::Verdict) -> bool {
    if verdict != crate::memory::Verdict::Hit {
        return false;
    }
    let Some(c) = choice else { return false };
    let total: f64 = c.totals.iter().map(|(_, w)| *w).sum();
    if total <= 0.0 {
        return false;
    }
    c.score / total >= UNCONTESTED
}

/// Runs the classifier and returns its verdict, or None when it cannot be reached.
///
/// Every failure path returns None rather than an error, because the caller's fallback is
/// the deterministic choice and a fleet that stops routing when a model is unavailable is
/// worse than one that routes the old way.
pub fn run(classifier: &Classifier, root: &Path, dossier: &str, roster: &[String]) -> Option<Verdict> {
    let Classifier::Command(cmd) = classifier else { return None };

    let mut parts = cmd.split_whitespace();
    let program = parts.next()?;
    let args: Vec<&str> = parts.collect();

    let mut child = crate::base::quiet(program)
        .args(&args)
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // The dossier goes on stdin, never in the argument list: it contains the user's
    // message, and an argument list is a quoting surface. Same reason the desk does it.
    child.stdin.take()?.write_all(dossier.as_bytes()).ok()?;

    let out = child.wait_with_output().ok()?;
    let reply = String::from_utf8_lossy(&out.stdout);
    parse(&reply, roster)
}

/// The line a surface shows when the fleet has no owner for a subject.
///
/// **This is the sentence Richard asked for**, and the reason it lives here rather than in
/// a caller is that every surface must say the same thing: the desk, the hook, and the
/// reading room are three places one wrong answer could be worded three ways.
pub fn coverage_note(v: &Verdict) -> Option<String> {
    match v.coverage {
        Coverage::Covered => None,
        Coverage::Adjacent => Some(format!(
            "VESTA: no agent owns {}. {} is the nearest, because {} Answering from there is \
             a stretch, and the honest options are to give that agent the knowledge or to \
             create an agent for this.",
            if v.subject.is_empty() { "this subject".into() } else { v.subject.clone() },
            v.owner.clone().unwrap_or_else(|| "no one".into()),
            v.reason
        )),
        Coverage::Uncovered => Some(format!(
            "VESTA: nothing in this fleet covers {}, and no agent is near enough to name. \
             {} This is a gap in the fleet rather than a gap in the question: it is worth \
             deciding whether an agent should exist for it.",
            if v.subject.is_empty() { "this subject".into() } else { v.subject.clone() },
            v.reason
        )),
    }
}

/// The deterministic choice, kept as the fallback and as the thing the classifier is
/// measured against.
pub fn fallback(choice: Option<AgentChoice>) -> Option<Verdict> {
    choice.map(|c| Verdict {
        owner: Some(c.agent),
        coverage: Coverage::Covered,
        subject: String::new(),
        reason: "chosen by keyword score, with no classifier configured".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster() -> Vec<String> {
        vec!["aldo".into(), "steve".into(), "yaron".into(), "zed".into()]
    }

    #[test]
    fn a_clean_verdict_parses() {
        let v = parse(
            "OWNER: zed\nCOVERAGE: covered\nSUBJECT: routing architecture\nREASON: it is about the router.",
            &roster(),
        )
        .expect("parses");
        assert_eq!(v.owner.as_deref(), Some("zed"));
        assert_eq!(v.coverage, Coverage::Covered);
        assert_eq!(v.subject, "routing architecture");
    }

    /// The case the whole module exists for: a subject nobody owns, with the nearest
    /// agent named and the gap reported instead of hidden behind a confident owner.
    #[test]
    fn an_uncovered_subject_names_the_nearest_agent_and_says_it_is_a_gap() {
        let v = parse(
            "OWNER: zed\nCOVERAGE: adjacent\nSUBJECT: devops and infrastructure\n\
             REASON: zed owns building software but not running it.",
            &roster(),
        )
        .expect("parses");
        assert_eq!(v.coverage, Coverage::Adjacent);
        let note = coverage_note(&v).expect("a note");
        assert!(note.contains("no agent owns devops and infrastructure"));
        assert!(note.contains("create an agent"), "the person is offered the real option");
    }

    /// A model naming an agent that does not exist must not produce a route: every
    /// surface downstream treats the owner as real.
    #[test]
    fn an_invented_agent_is_not_an_owner() {
        let v = parse("OWNER: devops\nCOVERAGE: covered\nSUBJECT: x\nREASON: y", &roster())
            .expect("parses");
        assert_eq!(v.owner, None, "a name off the roster is no owner at all");
    }

    #[test]
    fn a_model_that_answered_with_prose_is_not_a_verdict() {
        assert!(parse("I think Zed should handle this one.", &roster()).is_none());
    }

    /// Models decorate. The parser takes the field however it is dressed.
    #[test]
    fn markdown_and_case_do_not_break_the_parse() {
        let v = parse(
            "**OWNER:** Zed\n**COVERAGE:** Covered\nSUBJECT: the router\nREASON: because.",
            &roster(),
        )
        .expect("parses");
        assert_eq!(v.owner.as_deref(), Some("zed"));
        assert_eq!(v.coverage, Coverage::Covered);
    }

    fn choice(name: &str, totals: &[(&str, f64)]) -> AgentChoice {
        let score = totals.iter().find(|(n, _)| *n == name).map(|(_, w)| *w).unwrap_or(0.0);
        AgentChoice {
            agent: name.into(),
            score,
            files: 1,
            margin: 2.0,
            contenders: totals.len(),
            totals: totals.iter().map(|(n, w)| (n.to_string(), *w)).collect(),
        }
    }

    /// One agent holding the field needs no model. Two agents sharing it do, and so
    /// does a field with nothing in it, which is the case the whole module exists for.
    #[test]
    fn the_cascade_escalates_exactly_when_the_arithmetic_is_not_alone() {
        use crate::memory::Verdict as V;
        assert!(
            is_uncontested(Some(&choice("zed", &[("zed", 90.0), ("steve", 5.0)])), V::Hit),
            "one agent holding the field stands alone"
        );
        assert!(
            !is_uncontested(Some(&choice("zed", &[("zed", 55.0), ("steve", 45.0)])), V::Hit),
            "a contested field is worth a model"
        );
        assert!(
            !is_uncontested(None, V::Hit),
            "nothing scoring is not clarity, it is the DevOps case"
        );
        assert!(
            !is_uncontested(Some(&choice("zed", &[("zed", 90.0)])), V::Guess),
            "below the floor the arithmetic never stands alone"
        );
    }

    #[test]
    fn no_classifier_configured_means_no_verdict_and_no_error() {
        assert!(run(&Classifier::None, Path::new("."), "x", &roster()).is_none());
    }

    #[test]
    fn a_command_that_does_not_exist_falls_back_rather_than_failing() {
        let c = Classifier::Command("this-binary-does-not-exist-4114".into());
        assert!(run(&c, Path::new("."), "x", &roster()).is_none());
    }
}
