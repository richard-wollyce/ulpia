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
use std::path::{Path, PathBuf};

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
///
/// **Split into a prefix that never varies and a tail that always does**, and the split is
/// worth real time rather than being tidiness. A resident llama.cpp server keeps the KV
/// cache of the longest prefix it has already computed, so everything ahead of the first
/// difference is free after the first message. Measured on this machine with a 2B: the
/// server processed 755 tokens per message before the split and about 495 after it,
/// 7.9 seconds of prefill down to 4.0.
///
/// The two halves are separate functions so the boundary is a thing the compiler knows
/// about. Written as one function with a comment in the middle, the boundary survives
/// exactly until someone adds a line in the wrong place.
pub fn dossier(
    memory: &Memory,
    question: &str,
    found: &[Retrieved],
    confidence: crate::memory::Confidence,
) -> String {
    let mut out = stable_prefix(memory);
    out.push_str(&variable_tail(question, found, confidence, &species_table(memory, question)));
    out
}

/// Everything identical for every message: who is being asked, the roster, and the rules
/// for judging. Changes only when the fleet does.
fn stable_prefix(memory: &Memory) -> String {
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

    // The judging rules sit here, ahead of the evidence, rather than after the message
    // where they used to be. They are the same for every message, so they belong on this
    // side of the boundary.
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
         worth creating.\n",
    );

    // **A second worked example, for `uncovered`, was written here and measured out.**
    // The reasoning was clean: the rules demonstrate `adjacent` and never `uncovered`,
    // so the answer that matters most has no example. It cost a question. Haiku scored
    // 13 of 14 on the coverage set without it and 12 with it, and the 2B was unmoved at
    // 6 either way. What the same session's measurements did buy was an edit to one
    // agent's `ends` line, worth two questions.
    //
    // The general shape, since it is now the third time: **more instruction is not more
    // accuracy, and the roster is where the leverage is.** A rule added to the prompt
    // competes with every other rule for the model's attention; a fact added to an
    // agent's card is the only description of that agent there is.

    out
}

/// Reduces a catalogue blurb to the one line the classifier can use.
///
/// Strips the list marker, the wikilink that repeats the filename, and any leading
/// decoration, then cuts at a sentence end near the cap. The cap exists because the
/// evidence block is the variable half of the dossier and every character in it is
/// recomputed on every message.
fn one_line(summary: &str) -> String {
    const CAP: usize = 160;

    let mut t = summary.trim();
    t = t.trim_start_matches(['-', '*', ' ']);
    // `**[[name]]**` repeats the path printed on the line above it.
    if let Some(rest) = t.strip_prefix("[[") {
        if let Some((_, after)) = rest.split_once("]]") {
            t = after;
        }
    }
    let t: String = t
        .trim_start_matches(['*', ' '])
        .chars()
        .filter(|c| c.is_ascii() || c.is_alphabetic())
        .collect();
    let t = t.replace('*', "").replace('`', "");
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");

    if t.chars().count() <= CAP {
        return t;
    }
    // Prefer a sentence boundary inside the budget over a hard cut mid-word.
    let head: String = t.chars().take(CAP).collect();
    match head.rfind(". ") {
        Some(i) if i > CAP / 3 => head[..=i].trim().to_string(),
        _ => match head.rfind(' ') {
            Some(i) => format!("{}...", head[..i].trim_end_matches(',')),
            None => head,
        },
    }
}

/// Everything that changes with the message: what retrieval found, the message itself,
/// and the four lines to answer in.
/// The best keyword score per species per agent, over the fold's own window.
///
/// **Computed here and not from the fused top 5, which was the first version's defect.**
/// Measured on the ads-library probe: the fold counted Steve's tools declaration at keyword
/// rank 8 (memory 115.9 plus tools 37.8), while the fused top 5 was five memory files, so
/// the classifier was told one species contributed when the router had acted on two. Same
/// query and the same oversample as `Memory::ask`, so the table and the fold cannot see
/// different worlds. Lives beside `dossier` rather than inside `variable_tail` so the tail
/// stays testable without a fleet on disk.
fn species_table(memory: &Memory, question: &str) -> Vec<(String, [f32; 3])> {
    let hits = memory.route(question, 5 * crate::retrieve::KEYWORD_OVERSAMPLE);
    let mut per: Vec<(String, [f32; 3])> = Vec::new();
    for h in &hits {
        if h.score <= 0.0 {
            continue;
        }
        let k = crate::index::kind_of(&h.entry.rel) as usize;
        match per.iter_mut().find(|(b, _)| *b == h.entry.base) {
            Some((_, slots)) => {
                if h.score > slots[k] {
                    slots[k] = h.score;
                }
            }
            None => {
                let mut slots = [0.0f32; 3];
                slots[k] = h.score;
                per.push((h.entry.base.clone(), slots));
            }
        }
    }
    per.sort_by(|a, b| {
        let sa: f32 = a.1.iter().sum();
        let sb: f32 = b.1.iter().sum();
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    per
}

fn variable_tail(
    question: &str,
    found: &[Retrieved],
    confidence: crate::memory::Confidence,
    species: &[(String, [f32; 3])],
) -> String {
    let mut out = String::new();

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
                "  [{}] {}/{}  score {:.1}  matched: {}\n",
                crate::index::kind_of(&f.path).label(),
                f.base,
                f.path,
                f.keyword_score,
                if f.matched.is_empty() { "text only".into() } else { f.matched.join(", ") }
            ));
            // **A path is not evidence about a subject.** Without this line the
            // classifier is shown a filename and a score and asked whether the fleet
            // covers a domain, so it has to infer what the file is from how it is named.
            //
            // The text comes from the map entry, which was written to be read in a
            // catalogue rather than to answer this question, so it arrives carrying its
            // own list marker, its wikilink and sometimes an emoji. Those are stripped
            // and it is cut to one line: five untrimmed entries added about four hundred
            // tokens to a three hundred token dossier.
            if !f.purpose.is_empty() {
                out.push_str(&format!("      about: {}\n", one_line(&f.purpose)));
            }
        }
    }

    // **What retrieval thinks of its own answer, which the classifier was never told.**
    //
    // The evidence list is scores and paths, and a score alone cannot be read. The keyword
    // lines were widened from a median of six terms to about seventy, and the hit and miss
    // ranges now overlap completely: 20.02 to 187.39 against 21.27 to 132.66. **No threshold
    // in code separates them.** That is the second time this has been measured here, the
    // first being the rejected cascade, and it is why this is a sentence for a reader rather
    // than a gate in the router.
    //
    // So the numbers are handed over with their meaning attached and the judgement stays
    // where ADR-0013 put it. Agreement between the two independent scorers is the strongest
    // signal available without a model, and it is exactly what a bare score hides.

    // **The three questions, separated, per agent.** ADR-0031: "knows about it" is memory,
    // "knows how" is skills, "has the means" is tools, and a list of five files cannot
    // carry that distinction on its own. The table arrives computed from the fold's own
    // window (see `species_table` for the measured defect that rule closes), so what the
    // classifier reads is the evidence the router acted on.
    if !species.is_empty() {
        out.push_str(
            "\nBEST OF EACH SPECIES, per agent: knows about it (memory), knows how \
             (skills), has the means (tools). A dash is no evidence of that species at \
             all, which is itself information:\n",
        );
        for (base, slots) in species.iter().take(4) {
            let cell = |v: f32| {
                if v > 0.0 { format!("{v:.1}") } else { "-".into() }
            };
            out.push_str(&format!(
                "  {base}: memory {}, skills {}, tools {}\n",
                cell(slots[0]),
                cell(slots[1]),
                cell(slots[2])
            ));
        }
    }

    out.push_str(&format!(
        "\nWHAT RETRIEVAL THINKS OF THAT. Top keyword score {:.1}, against a floor of {:.1} \
         below which nothing is worth answering from. {} of the two independent scorers \
         ranked that file, and it leads the runner-up by {:.2}x. Retrieval's own verdict: \
         {}.\n\n\
         Weigh that before deciding coverage. A high score on a base whose domain does not \
         fit is a coincidence of vocabulary, and one scorer alone is the case this system \
         reports as a guess rather than an answer.\n",
        confidence.keyword_score,
        crate::memory::SCORE_FLOOR,
        match confidence.agreement {
            2 => "Both",
            1 => "Only one",
            _ => "Neither",
        },
        confidence.margin,
        match confidence.verdict {
            crate::memory::Verdict::Hit => "something here matches",
            crate::memory::Verdict::Guess =>
                "this is a guess, too weak or too close to the runner-up to tell from a \
                 coincidence of vocabulary",
            crate::memory::Verdict::Nothing => "nothing matched at all",
        }
    ));

    out.push_str(&format!("\nTHE MESSAGE:\n  {}\n", question.replace('\n', " ")));

    // The four fields are answered in the order they are derived in, not the order they
    // are read in. A 0.8B asked for the owner first wrote `OWNER: steve` above a REASON
    // that described a different agent entirely: it committed to a name and then
    // narrated around it. Naming the subject first costs about twenty tokens of output
    // and gives the choice something to follow from.
    out.push_str(
        "\nAnswer in exactly these four lines, in this order:\n\
         SUBJECT: <two to five words naming the domain this message belongs to>\n\
         REASON: <one sentence>\n\
         COVERAGE: <covered|adjacent|uncovered>\n\
         OWNER: <name from the list, or none>\n",
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

/// An empty directory, outside the fleet, for the classifier to run in.
///
/// **This is worth 33 seconds a message and it is the difference between the classifier
/// fitting inside the hook's budget and not.** Measured on 2026-08-20 with the same
/// dossier, the same model and the same flags, varying only the working directory:
///
/// | working directory | wall | of which API |
/// |---|---|---|
/// | the fleet root | 47.4s | 12.8s |
/// | an empty directory | 11.5s | 7.1s |
///
/// A CLI runtime inspects the directory it starts in: its instruction files, its settings,
/// its git repository, its tree. This fleet root holds 11,510 files and 2.5 GB of Rust
/// build output, and the classifier was paying to have all of it looked at, on every
/// message, to answer a question whose entire input arrives on stdin.
///
/// **It is also the isolation the design already claimed.** `classify-claude.cmd` spends
/// two flags stopping the classifier from reading the base, on the grounds that a
/// classifier that can search stops being a judge and becomes a second agent. Starting it
/// inside the base contradicted that, and the cost was the tell.
///
/// Outside the fleet rather than under `.kb/`, because a runtime that walks up from its
/// working directory looking for instruction files would find the fleet's own from any
/// directory inside it, and the saving would quietly disappear.
pub(crate) fn scratch_cwd(root: &Path) -> PathBuf {
    let dir = std::env::temp_dir().join("kb-classifier-cwd");
    // Best effort on purpose. If the directory cannot be made, running in the root is
    // slow and correct, and slow and correct beats refusing to route.
    if std::fs::create_dir_all(&dir).is_ok() {
        dir
    } else {
        root.to_path_buf()
    }
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

    // The command is named relative to the fleet root in `fleet.txt`, so it has to be
    // resolved against the root before the working directory stops being the root.
    // An absolute path or a bare name on PATH passes through untouched.
    let resolved = {
        let candidate = root.join(program);
        if candidate.is_file() { candidate } else { PathBuf::from(program) }
    };

    let mut child = crate::base::quiet(&resolved.to_string_lossy())
        .args(&args)
        .current_dir(scratch_cwd(root))
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
pub fn fallback(choice: Option<AgentChoice>, why: FellBack) -> Option<Verdict> {
    choice.map(|c| Verdict {
        owner: Some(c.agent),
        coverage: Coverage::Covered,
        subject: String::new(),
        reason: why.reason().into(),
    })
}

/// Why the deterministic choice is answering instead of a model.
///
/// **The two cases must not share a sentence.** The previous version said "no classifier
/// configured" for both, so a fleet whose classifier was configured and dead reported
/// itself as a fleet that had never asked for one. That is the same failure as running a
/// stale binary and reporting the feature it did not contain: a message asserting
/// something it never checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FellBack {
    /// `fleet.txt` names no classifier. The deterministic sum is the whole router, which
    /// is a supported configuration and not a fault.
    NotConfigured,
    /// A classifier is configured and did not answer: not installed, not running, timed
    /// out, or it named an agent off the roster. Routing continues, worse, and silently
    /// unless somebody is told.
    DidNotAnswer,
}

impl FellBack {
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotConfigured => "chosen by keyword score, with no classifier configured",
            Self::DidNotAnswer => {
                "chosen by keyword score, because the configured classifier did not answer"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster() -> Vec<String> {
        vec!["aldo".into(), "steve".into(), "yaron".into(), "zed".into()]
    }

    fn evidence(n: usize) -> Vec<Retrieved> {
        (0..n)
            .map(|i| Retrieved {
                base: "zed".into(),
                path: format!("knowledge/systems/a-file-with-a-realistic-name-{i}.md"),
                layer: crate::retrieve::Layer::Long,
                title: String::new(),
                purpose: String::new(),
                score: 0.0,
                keyword_score: 15.0,
                why: vec![],
                matched: vec!["deploy".into(), "monitoring".into()],
                passages: vec![],
            })
            .collect()
    }

    /// **That the prefix cannot contain the message is the compiler's job, not this
    /// test's**: `stable_prefix` is not handed the question, so no edit can put one there
    /// without changing a signature. What no signature can check is how much is left on
    /// the variable side, and that is what decides whether caching the prefix pays.
    ///
    /// The measured prefix on this fleet is about 550 tokens. A tail that grew past it
    /// would mean most of each message is recomputed anyway and the split stopped earning
    /// its complexity. The number is a tripwire rather than a target.
    ///
    /// **It fired once, on 2026-08-20, and the limit moved rather than the code.** The tail
    /// gained two things that turn a list of paths into evidence a reader can weigh: an
    /// `about:` line per file saying what it is for, and a paragraph carrying what retrieval
    /// thinks of its own answer. 1400 characters became 1489. Both earn their room, and 1800
    /// was still roughly 450 tokens against a 550 token prefix.
    ///
    /// Raised again to 2200 on 2026-08-21, when ADR-0031 added the species table, and this
    /// time the halves are roughly equal rather than prefix-heavy. Taken with eyes open:
    /// the table IS the coverage judgement stated as data (knows about it, knows how, has
    /// the means, per agent), which is the one judgement the classifier exists to make, so
    /// it outranks everything else in the tail for its cost. Caching the prefix still pays;
    /// what stops paying at this size is adding anything more, and the next addition should
    /// evict something instead of raising this number a third time.
    #[test]
    fn the_variable_half_stays_small_enough_for_caching_the_other_half_to_pay() {
        let tail = variable_tail(
            "como faco deploy com zero downtime e monitoramento de infra",
            &evidence(5),
            crate::memory::Confidence {
                verdict: crate::memory::Verdict::Hit,
                agreement: 2,
                keyword_score: 40.0,
                margin: 2.0,
            },
            // A realistic species table, so the budget below is measured against the tail
            // as it actually ships and not against a version with the block missing.
            &[
                ("zed".into(), [40.0, 12.5, 8.0]),
                ("steve".into(), [21.0, 0.0, 5.5]),
                ("yaron".into(), [11.0, 0.0, 0.0]),
                ("aldus".into(), [7.5, 3.0, 0.0]),
            ],
        );
        assert!(
            tail.len() < 2200,
            "the variable tail grew to {} chars; caching the prefix stops paying",
            tail.len()
        );
        assert!(tail.contains("zero downtime"), "the message belongs on the variable side");
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
