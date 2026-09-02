//! Promotion: how something said in a conversation becomes something the base knows.
//!
//! ## The problem this exists to not have
//!
//! Every memory product on the market extracts facts from conversation and writes them
//! straight into the durable store. A production audit filed against mem0 as issue #4573
//! counted 10,134 entries accumulated over 32 days, of which **97.8 percent were judged
//! junk**. That number is not a capability gap and a better model does not fix it: it is
//! what happens when the thing that writes is also the only thing that decides.
//!
//! So this base has always been written deliberately, and `ADR-0016` made a note and the
//! keys that reach it arrive together. That rule holds at low volume for one person. It
//! inverts the moment somebody uses this through an API and never opens an editor, which
//! is the boundary condition `ai-memory-layers-market` wrote down before we crossed it.
//!
//! ## The shape
//!
//! Two promoters, and the second is not a second opinion on the first.
//!
//! **Promoter one proposes.** It reads the deposit, which is `inbox/`, and returns notes it
//! thinks belong in the base. It never sees the base. Its only job is to find what is worth
//! keeping in a pile of unreviewed material.
//!
//! **Promoter two decides**, three times, through three lenses. It never sees promoter one's
//! reasoning, and that is enforced here rather than asked for in a prompt: [`Proposal`] has
//! no field in which reasoning could travel, so there is no version of this code where the
//! reviewer is shown the argument it is supposed to be independent of. Two models reading
//! the same material reach the same mistake with twice the confidence, which is the failure
//! `letta-architecture` records as "a second opinion from the same species of error". What
//! makes the second reader independent is a **different input**, not a bigger model.
//!
//! The different input is the base itself, and specifically what our own router says about
//! the proposal. The keys the proposal declares are run through [`crate::memory::Memory::ask`],
//! which is deterministic and has no model in it, and the reviewer is handed the result: the
//! files that already answer to those words, their purpose lines, and the confidence
//! evidence. Its question is not "did promoter one do a good job". Its question is "is this
//! new, and does it agree with what is already written". Different question, so a different
//! answer is possible.
//!
//! ## Three lenses rather than one stronger reader
//!
//! Latency is not the constraint here and quality is, so the reviewer runs three times with
//! three questions instead of once with one. Redundancy catches what one reader misses only
//! when the readers differ; three identical calls to a better model mostly agree with
//! themselves. See [`Lens`].
//!
//! ## A rejection is evidence, not garbage
//!
//! Rejections are appended to `kb-rejections.txt`, counted, with the reason and the dates.
//! A proposal refused three times for the same reason is not noise: it is the base being
//! asked for something it does not hold. That is the same argument `misses.rs` makes about
//! questions nobody could answer, applied to the writing side instead of the reading side.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::classify::Classifier;
use crate::memory::Memory;

/// Where unreviewed material waits. The short memory.
///
/// **This used to say "not indexed, and that is the feature", and it was never true.**
/// Nothing excluded the deposit from the text index; it carried no `Search for:` line,
/// so the router never named it, while the text scorer surfaced it whenever its words
/// matched. ADR-0034 made the real behaviour the intended one: the deposit is served,
/// and every surface labels a passage from it as short memory, so a model deciding to
/// lean on it does so knowing it is recent and unjudged. `retrieve::layer_of` is the one
/// place that rule lives, and `checks::is_exempt` keeps E02 quiet about the missing
/// keyword line, because a deposit file is not supposed to have one.
pub const DEPOSIT: &str = "inbox";

/// The stage a promoted note carries until a person or a later pass distils it.
///
/// Kept distinct from `distilled` so the provenance ladder does not quietly gain a rung:
/// a note that arrived by promotion has been reviewed by a model and by nobody else, and
/// `ADR-0007`'s rule that an agent claim is never promoted to human or external still holds.
pub const CAPTURED: &str = "captured";

/// What promoter one returns, and deliberately all it can return.
///
/// **There is no `reasoning` field and that absence is the mechanism.** The reviewer is
/// built from this struct, so promoter one's argument cannot reach it even by accident.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub agent: String,
    pub slug: String,
    pub folder: String,
    pub summary: String,
    pub keys: Vec<String>,
    pub body: String,
    /// The deposit file this came out of, so a written note can be traced back and the
    /// deposit can be cleared only when everything in it was decided.
    pub source: String,
}

/// The three questions asked of every proposal, each in its own call.
///
/// They are different questions rather than three phrasings of one, which is the only
/// reason running three is worth more than running one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lens {
    /// Does this disagree with something the base already states.
    Contradiction,
    /// Is this already held, in this or another file.
    Duplication,
    /// Does this belong to this agent at all, or to another one, or to nobody.
    Scope,
}

impl Lens {
    pub const ALL: [Lens; 3] = [Lens::Contradiction, Lens::Duplication, Lens::Scope];

    pub fn name(self) -> &'static str {
        match self {
            Lens::Contradiction => "contradiction",
            Lens::Duplication => "duplication",
            Lens::Scope => "scope",
        }
    }

    /// The question, written so an answer of "reject" is as easy to give as "accept".
    /// A lens that only knows how to approve is a lens that approves everything.
    fn question(self) -> &'static str {
        match self {
            Lens::Contradiction => {
                "Does this proposed note assert something that CANNOT BOTH BE TRUE with what \
                 the base already states, or with its own source? Two claims conflict when \
                 believing one means disbelieving the other: a different number for the same \
                 quantity, a different fact about the same event, an instruction that reverses \
                 an existing one.\n\n\
                 This is NOT the question of whether the base already says this. Something \
                 already covered is not a contradiction, it is a duplicate, and a different \
                 reader is being asked that separately. If your only objection is that an \
                 existing file already holds this, ACCEPT and let that reader do its job.\n\n\
                 Reject only when the note and the base cannot both stand."
            }
            Lens::Duplication => {
                "Is this proposed note ALREADY HELD by one of the existing files below? \
                 Answering the same question with the same content is duplication even when \
                 the wording differs, and restating an existing line in a second place is \
                 exactly this and not a contradiction. Two files answering one question is \
                 how a base stops having an answer.\n\n\
                 Reject if an existing file already holds this, and name it. Accept if the \
                 note genuinely adds something no listed file holds, even when it sits close \
                 to one of them."
            }
            Lens::Scope => {
                "Does this belong to THIS agent? An agent has a subject and an edge. Material \
                 that is true but belongs to another agent, or to no agent in the fleet, must \
                 not be filed here just because it arrived here.\n\n\
                 This is NOT the question of whether the note is true, new, or well written. \
                 Another reader is asking each of those. Judge only where it belongs.\n\n\
                 Reject if it is out of scope, and name the agent it belongs to if one does."
            }
        }
    }
}

/// One lens's answer.
#[derive(Debug, Clone)]
pub struct Review {
    pub lens: Lens,
    pub accept: bool,
    pub reason: String,
}

/// What happened to one proposal.
#[derive(Debug, Clone)]
pub struct Decided {
    pub proposal: Proposal,
    pub reviews: Vec<Review>,
    pub written: Option<PathBuf>,
}

impl Decided {
    /// **Unanimity, not majority.** Two of three would let one lens be overruled by two
    /// that were not asked its question, which defeats the point of asking three different
    /// questions rather than one question three times.
    pub fn accepted(&self) -> bool {
        self.reviews.len() == Lens::ALL.len() && self.reviews.iter().all(|r| r.accept)
    }

    pub fn refusal(&self) -> Option<&Review> {
        self.reviews.iter().find(|r| !r.accept)
    }

    /// **Every lens that refused, not only the first.**
    ///
    /// The first run reported one refusal per proposal and the reason always named the
    /// contradiction lens, which made it look like one reader was doing all the work. It
    /// was not: the reasons it gave were duplication's, because the prompts overlapped.
    /// Showing all of them is what made that visible, so it is what gets shown.
    pub fn refusals(&self) -> impl Iterator<Item = &Review> {
        self.reviews.iter().filter(|r| !r.accept)
    }
}

/// Everything one run decided.
#[derive(Debug, Default)]
pub struct Outcome {
    pub decided: Vec<Decided>,
    /// Deposit files that produced no proposal at all.
    pub barren: Vec<String>,
    /// Promoters that were configured and did not answer. Degraded and silent is the
    /// combination this repository keeps paying for, so it is carried rather than logged.
    pub unreachable: Vec<String>,
    /// The cap, if a cap ended the run before the deposit was exhausted.
    ///
    /// Carried rather than inferred, because a run that stopped at its cap and a run that
    /// finished the deposit print the same counts, and only one of them means "the rest of
    /// the deposit is still there". A truncated run that looks complete is how an unattended
    /// command quietly stops doing half its job.
    pub stopped_at: Option<usize>,
}

impl Outcome {
    pub fn written(&self) -> usize {
        self.decided.iter().filter(|d| d.written.is_some()).count()
    }

    pub fn refused(&self) -> usize {
        self.decided.iter().filter(|d| !d.accepted()).count()
    }

    /// Proposals all three lenses admitted, whether or not a file was written for them.
    ///
    /// This and not [`Outcome::written`] is what the cap counts, so `--dry-run --max 3`
    /// stops where the real run would stop. A cap that only bites when something is
    /// written cannot be rehearsed, and a cap nobody can rehearse is a cap nobody trusts.
    pub fn admitted(&self) -> usize {
        self.decided.iter().filter(|d| d.accepted()).count()
    }
}

// -- one run at a time -------------------------------------------------------

/// Where the running-now marker lives, beside `fleet.txt` and `kb-rejections.txt`.
pub const LOCK: &str = ".kb-promote.lock";

/// How long a marker is believed before it is treated as debris, in seconds.
///
/// A promotion run is minutes, not hours, because it is bounded by model calls that each
/// have their own timeout. An hour is long enough that no live run is ever shot in the
/// back, and short enough that a machine which lost power at the wrong moment promotes
/// again the same day instead of never again.
pub const LOCK_STALE_SECS: u64 = 3600;

/// Held for the duration of a run, released by dropping it.
///
/// **Why this exists at all:** the trigger is a session-end hook, and sessions end
/// together. Two runs over the same deposit both call promoter one on the same file, and
/// the duplication lens cannot save them, because it is shown what the base holds and
/// neither run has written anything yet. Both proposals look new, both are admitted, and
/// the base gains the same note twice. Serialising the runs is the only place that is
/// fixable: by the time the reviewer sees it, the information needed is in another process.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
    /// Set when this lock was taken over from a marker old enough to be debris. The
    /// caller prints it. A run that quietly stepped over another run's marker is a run
    /// that will do it again next time and never say so.
    pub took_over: Option<String>,
}

impl Lock {
    /// Takes the lock, or explains who has it.
    ///
    /// The mechanism is `create_new`, which is `O_EXCL` on Unix and `CREATE_NEW` on
    /// Windows: the file system decides the race, so two processes cannot both believe
    /// they created it. A check-then-create would leave exactly the window this exists
    /// to close.
    pub fn take(fleet_root: &Path) -> Result<Lock, String> {
        let path = fleet_root.join(LOCK);
        match Self::create(&path) {
            Ok(()) => return Ok(Lock { path, took_over: None }),
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                return Err(format!("could not write {}: {e}", path.display()));
            }
            Err(_) => {}
        }

        // Somebody holds it, or somebody died holding it. A lock that is never broken
        // turns one crash into a feature that is off forever and says nothing.
        let age = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs());

        match age {
            Some(secs) if secs > LOCK_STALE_SECS => {
                let held = std::fs::read_to_string(&path).unwrap_or_default().trim().to_string();
                let _ = std::fs::remove_file(&path);
                match Self::create(&path) {
                    Ok(()) => Ok(Lock {
                        path,
                        took_over: Some(format!(
                            "took over a lock last touched {} minutes ago, left by {}. \
                             A previous run died without releasing it.",
                            secs / 60,
                            if held.is_empty() { "an unnamed process" } else { &held }
                        )),
                    }),
                    Err(e) => Err(format!("stale lock, and re-taking it failed: {e}")),
                }
            }
            Some(secs) => Err(format!(
                "another promotion run holds {} (started {}s ago).",
                path.display(),
                secs
            )),
            None => Err(format!("another promotion run holds {}.", path.display())),
        }
    }

    fn create(path: &Path) -> std::io::Result<()> {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        writeln!(f, "pid {}", std::process::id())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Best effort on purpose. A lock file left behind is recovered by the staleness
        // rule above; a panic inside Drop while unwinding would abort the process.
        let _ = std::fs::remove_file(&self.path);
    }
}

// -- talking to a promoter ---------------------------------------------------

/// One model call, same contract as the classifier: text on stdin, text on stdout.
///
/// Reusing the shape rather than a provider SDK is `ADR-0027`'s decision and it holds here
/// for the same reason: any model behind any runtime satisfies a process contract, and a
/// promoter that cannot be reached must degrade rather than fail.
pub fn ask_model(who: &Classifier, root: &Path, prompt: &str) -> Option<String> {
    let Classifier::Command(cmd) = who else { return None };

    let mut parts = cmd.split_whitespace();
    let program = parts.next()?;
    let args: Vec<&str> = parts.collect();

    let resolved = {
        let candidate = root.join(program);
        if candidate.is_file() { candidate } else { PathBuf::from(program) }
    };

    let mut child = crate::base::quiet(&resolved.to_string_lossy())
        .args(&args)
        .current_dir(crate::classify::scratch_cwd(root))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // On stdin, never in the argument list. The deposit contains whatever a person said,
    // and an argument list is a quoting surface.
    child.stdin.take()?.write_all(prompt.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

// -- promoter one ------------------------------------------------------------

/// The prompt that reads a pile and proposes notes.
///
/// It is told what the agent is and where it stops, and it is NOT told what the base
/// already contains. Proposing and checking are different jobs and this half only proposes;
/// giving it the base here would make the reviewer's independent input its input too.
pub fn proposal_prompt(agent: &str, ends: Option<&str>, source: &str, deposit: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "You are reading unreviewed material that landed in an agent's deposit. Propose the \
         notes, if any, that deserve to become part of that agent's durable knowledge.\n\n",
    );
    out.push_str(&format!("THE AGENT: {agent}\n"));
    if let Some(e) = ends {
        out.push_str(&format!("WHERE IT STOPS: {e}\n"));
    }
    out.push_str(&format!("\nTHE DEPOSIT FILE: {source}\n---\n{deposit}\n---\n\n"));
    out.push_str(
        "RULES.\n\
         - Propose nothing rather than something. Most material in a deposit is conversation, \
           not knowledge. A note is worth writing when somebody would later ask a question it \
           answers.\n\
         - One note answers one question. If the material holds three unrelated things, \
           propose three notes.\n\
         - Do not propose a note that only records that a conversation happened.\n\
         - `keys` are the words a real question would use, thirty to seventy of them, in both \
           Portuguese and English, because there is no stemmer and no translation on the file \
           side. Avoid multi word keys whose component words are generic: the index splits \
           them and a bare common word steals unrelated questions.\n\
         - `summary` says what the note is ABOUT in one line, not an inventory of what it \
           mentions.\n\n\
         FORMAT. Zero or more blocks, exactly like this, nothing else in your output:\n\n\
         PROPOSAL\n\
         slug: short-kebab-case-name\n\
         folder: knowledge/<subfolder>\n\
         summary: one line, no trailing period\n\
         keys: term, term, term\n\
         body:\n\
         the markdown body of the note\n\
         END\n\n\
         If nothing here belongs in the base, output the single word NOTHING.\n",
    );
    out
}

/// Parses promoter one's reply. Anything malformed is dropped rather than guessed at:
/// a proposal we had to repair is a proposal nobody wrote.
pub fn parse_proposals(reply: &str, agent: &str, source: &str) -> Vec<Proposal> {
    let mut out = Vec::new();
    let mut lines = reply.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim() != "PROPOSAL" {
            continue;
        }
        let (mut slug, mut folder, mut summary) = (String::new(), String::new(), String::new());
        let mut keys: Vec<String> = Vec::new();
        let mut body = String::new();
        let mut in_body = false;

        for l in lines.by_ref() {
            let t = l.trim();
            if t == "END" {
                break;
            }
            if in_body {
                body.push_str(l);
                body.push('\n');
                continue;
            }
            if let Some(v) = t.strip_prefix("slug:") {
                slug = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("folder:") {
                folder = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("summary:") {
                summary = v.trim().trim_end_matches('.').to_string();
            } else if let Some(v) = t.strip_prefix("keys:") {
                keys = v
                    .split(',')
                    .map(|k| k.trim().trim_matches('`').to_string())
                    .filter(|k| !k.is_empty())
                    .collect();
            } else if t == "body:" {
                in_body = true;
            }
        }

        // A proposal with no keys cannot be found once written, which `index::build` makes
        // literal: it would be skipped and the note would exist and be unreachable.
        if slug.is_empty() || summary.is_empty() || keys.is_empty() || body.trim().is_empty() {
            continue;
        }
        out.push(Proposal {
            agent: agent.to_string(),
            slug,
            folder: if folder.is_empty() { "knowledge".into() } else { folder },
            summary,
            keys,
            body: body.trim().to_string(),
            source: source.to_string(),
        });
    }
    out
}

// -- the independent input ---------------------------------------------------

/// What the base already says about the words this proposal claims.
///
/// **Deterministic and model free.** This is the reviewer's independent input, so it must
/// not come from a model or the independence is theatre. It is our own router, asked with
/// the proposal's own keys, and it returns the files that already answer to those words
/// together with the confidence evidence `Memory::ask` carries.
pub fn evidence_for(memory: &Memory, proposal: &Proposal, top: usize) -> String {
    let question = proposal.keys.join(" ");
    let answer = memory.ask(&question, top);

    let mut out = String::new();
    out.push_str(
        "WHAT THE BASE ALREADY HOLDS FOR THESE WORDS. This is the deterministic router, not \
         a model, asked with the proposal's own keys.\n\n",
    );
    out.push_str(&format!(
        "  top keyword score {:.1} against a floor of {:.1}; {} of the two independent \
         scorers ranked the top file; it leads the runner-up by {:.2}x\n\n",
        answer.confidence.keyword_score,
        answer.confidence.floor,
        match answer.confidence.agreement {
            2 => "both",
            1 => "only one",
            _ => "neither",
        },
        answer.confidence.margin
    ));

    if answer.found.is_empty() {
        out.push_str("  Nothing in the base ranks for these words at all.\n");
        return out;
    }

    for (i, f) in answer.found.iter().enumerate() {
        out.push_str(&format!(
            "{}. {}/{}  ({})\n   about: {}\n",
            i + 1,
            f.base,
            f.path,
            f.title,
            if f.purpose.is_empty() { "no purpose line" } else { &f.purpose }
        ));
        if let Some(p) = f.passages.first() {
            let text = p.text.replace('\n', " ");
            let cut: String = text.chars().take(400).collect();
            out.push_str(&format!("   holds: {cut}\n"));
        }
    }
    out
}

/// The reviewer's prompt.
///
/// Built from [`Proposal`] and [`evidence_for`] and nothing else, which is what keeps
/// promoter one's reasoning out of it.
pub fn review_prompt(proposal: &Proposal, evidence: &str, lens: Lens) -> String {
    format!(
        "You are reviewing ONE proposed note against a knowledge base that already exists. \
         You did not write it and you are not being shown who did or why. Judge the note and \
         the base, nothing else.\n\n\
         YOUR ONE QUESTION: {question}\n\n\
         THE PROPOSED NOTE\n\
         agent: {agent}\n\
         file: {folder}/{slug}.md\n\
         about: {summary}\n\
         keys: {keys}\n\
         ---\n{body}\n---\n\n\
         {evidence}\n\
         Answer in exactly two lines and nothing else:\n\
         VERDICT: accept\n\
         REASON: one sentence\n\n\
         or\n\n\
         VERDICT: reject\n\
         REASON: one sentence naming the specific file or line that decided it\n\n\
         Rejecting is a normal outcome and costs nothing. Writing a note the base did not \
         need costs every question that note will later win.\n",
        question = lens.question(),
        agent = proposal.agent,
        folder = proposal.folder,
        slug = proposal.slug,
        summary = proposal.summary,
        keys = proposal.keys.join(", "),
        body = proposal.body,
        evidence = evidence,
    )
}

/// Strips the decoration a model puts around a line it was asked to emit bare.
///
/// **Measured, not anticipated.** The first real run refused a proposal with "the answer
/// could not be read", because the format was matched with `strip_prefix("VERDICT:")` and
/// anything wrapping it, a bold marker, a bullet, a code fence, a space before the colon,
/// missed. A gate that refuses on formatting is a gate whose record fills with parse
/// failures instead of judgements, and those are the same word to whoever reads it later.
fn undecorate(line: &str) -> String {
    line.trim()
        .trim_start_matches(&['-', '*', '>', '#', '`'][..])
        .trim()
        .replace("**", "")
        .replace("__", "")
        .trim()
        .to_string()
}

/// Reads `LABEL:` off a line however the model dressed it, tolerating a space before the
/// colon and any case.
fn labelled(line: &str, label: &str) -> Option<String> {
    let clean = undecorate(line);
    let (head, rest) = clean.split_once(':')?;
    if head.trim().eq_ignore_ascii_case(label) {
        Some(rest.trim().to_string())
    } else {
        None
    }
}

/// Parses one lens's answer.
///
/// **An unreadable reply is a rejection, never an approval.** The failure mode of a step
/// that mutates the base must be to write nothing, so every ambiguity here resolves toward
/// refusing. Where that fires, the reply itself is carried into the reason: a record saying
/// only "could not be read" is a record nobody can act on, and this parser was already
/// wrong once.
pub fn parse_review(reply: &str, lens: Lens) -> Review {
    let mut accept = None;
    let mut reason = String::new();

    for line in reply.lines() {
        if let Some(v) = labelled(line, "verdict") {
            let v = v.to_lowercase();
            // Checked in this order because a reviewer explaining why it did not reject
            // writes both words, and the safe reading of an ambiguous answer is the one
            // that writes nothing.
            if v.contains("reject") {
                accept = Some(false);
            } else if v.contains("accept") {
                accept = Some(true);
            }
        } else if let Some(r) = labelled(line, "reason") {
            if reason.is_empty() {
                reason = r;
            }
        }
    }

    // A bare verdict on its own line, which is what a model does when it decides the label
    // was scaffolding. Only exact, so a sentence mentioning the word cannot decide anything.
    if accept.is_none() {
        for line in reply.lines() {
            match undecorate(line).to_lowercase().trim_end_matches('.') {
                "reject" | "rejected" => {
                    accept = Some(false);
                    break;
                }
                "accept" | "accepted" => {
                    accept = Some(true);
                    break;
                }
                _ => {}
            }
        }
    }

    let Some(accept) = accept else {
        let seen: String = reply.split_whitespace().collect::<Vec<_>>().join(" ");
        let seen: String = seen.chars().take(200).collect();
        return Review {
            lens,
            accept: false,
            reason: if seen.is_empty() {
                "the reviewer returned nothing, which is refused rather than guessed at".into()
            } else {
                format!(
                    "no verdict could be read, which is refused rather than guessed at. It said: {seen}"
                )
            },
        };
    };

    if reason.is_empty() {
        reason = "no reason given".into();
    }
    Review { lens, accept, reason }
}

// -- the rejection record ----------------------------------------------------

/// Refused proposals, counted, beside `kb-misses.txt` and for the same reason.
///
/// **A rejection is evidence.** The same note refused three times by the same lens is not
/// noise to be swept up: it is the base being asked for something it does not hold, or an
/// agent being handed material that belongs to another one. `misses.rs` makes this argument
/// about questions nobody could answer; this is the writing side of it.
///
/// Lives beside `fleet.txt` rather than in `.kb/`, because `.kb/` is derived and disposable
/// and this is neither.
pub const REJECTIONS_TXT: &str = "kb-rejections.txt";

const REJECT_HEADER: &str = "\
# Proposals the reviewer refused, most refused first.
#
# One line per distinct proposal: count, first seen, last seen, agent, slug, then the lens
# that refused it and why. A proposal refused repeatedly for the same reason is a gap in the
# base rather than a bad proposal, which is the whole point of writing them down.
#
";

/// Appends one refusal, folding a repeat into a count rather than a second line.
pub fn record_rejection(fleet_root: &Path, decided: &Decided, today: &str) {
    let Some(refusal) = decided.refusal() else { return };
    let path = fleet_root.join(REJECTIONS_TXT);
    let key = format!("{}\t{}\t{}", decided.proposal.agent, decided.proposal.slug, refusal.lens.name());

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut folded = false;

    for line in existing.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        // count, first, last, agent, slug, lens, reason
        if parts.len() >= 7 && format!("{}\t{}\t{}", parts[3], parts[4], parts[5]) == key {
            let count: usize = parts[0].trim().parse().unwrap_or(1);
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                count + 1,
                parts[1],
                today,
                key,
                refusal.reason.replace('\t', " ")
            ));
            folded = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !folded {
        lines.push(format!("1\t{today}\t{today}\t{key}\t{}", refusal.reason.replace('\t', " ")));
    }

    // Most refused first, so the gap that keeps being asked for is the first thing read.
    lines.sort_by_key(|l| std::cmp::Reverse(l.split('\t').next().and_then(|c| c.parse::<usize>().ok()).unwrap_or(0)));

    let mut out = String::from(REJECT_HEADER);
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    let _ = std::fs::write(&path, out);
}

// -- the run -----------------------------------------------------------------

/// `ends = ` from an agent's card, read here rather than threaded through `Memory` because
/// it is the only thing promoter one needs from it.
fn ends_of(agent_root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(agent_root.join("agent.txt")).ok()?;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == "ends" && !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// Every markdown file waiting in one agent's deposit.
pub fn deposit_files(agent_root: &Path) -> Vec<PathBuf> {
    let dir = agent_root.join(DEPOSIT);
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    // Orientation files are not deposits. `what-goes-here.md` is the legend telling a
    // person what to drop in this folder, and feeding it to a promoter spends a model call
    // to be told, correctly, that a folder legend is not knowledge. Measured on the first
    // run: two of the five deposits examined were these.
    let boilerplate = |p: &PathBuf| {
        p.file_name()
            .map(|n| {
                let n = n.to_string_lossy().to_lowercase();
                n == "what-goes-here.md" || n == "readme.md" || n == "moved.md"
            })
            .unwrap_or(false)
    };

    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .filter(|p| !boilerplate(p))
        .collect();
    out.sort();
    out
}

/// One pass over the fleet's deposits.
///
/// **The order is the design.** Promoter one never sees the base, the router runs with no
/// model in it, and promoter two sees the router's answer and the proposal but never
/// promoter one. Each step is separated by a function boundary rather than by discipline,
/// so the independence survives whoever edits the prompts next.
/// `max` caps how many proposals one run may admit before it stops and leaves the rest of
/// the deposit where it is. It exists because this command now runs with nobody watching:
/// a bounded blast radius is the difference between a bad afternoon and a base to restore.
/// It is **not** the self-limiting rule ADR-0030 leaves unbuilt, which is a junk-rate
/// threshold measured from what was written, and which cannot be chosen from zero runs.
pub fn run(
    memory: &Memory,
    fleet_root: &Path,
    promoter: &Classifier,
    reviewer: &Classifier,
    top: usize,
    dry_run: bool,
    today: &str,
    max: Option<usize>,
) -> Outcome {
    let mut outcome = Outcome::default();

    'agents: for agent in memory.agents.iter().filter(|a| a.routable) {
        for file in deposit_files(&agent.root) {
            // Checked before the deposit file is read rather than after it is decided, so
            // the cap costs nothing once it has bitten. A run that keeps calling models
            // after it stopped writing is paying for output it already refused to use.
            if let Some(cap) = max {
                if outcome.admitted() >= cap {
                    outcome.stopped_at = Some(cap);
                    break 'agents;
                }
            }
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let source = format!("{}/{}/{}", agent.name, DEPOSIT, name);
            let Ok(text) = std::fs::read_to_string(&file) else { continue };

            let prompt = proposal_prompt(&agent.name, ends_of(&agent.root).as_deref(), &source, &text);
            let Some(reply) = ask_model(promoter, fleet_root, &prompt) else {
                outcome.unreachable.push(format!("promoter, on {source}"));
                continue;
            };

            let proposals = parse_proposals(&reply, &agent.name, &source);
            if proposals.is_empty() {
                outcome.barren.push(source);
                continue;
            }

            for proposal in proposals {
                // And again here, because one deposit file can produce several proposals
                // and the cap is a cap on notes, not on files.
                if let Some(cap) = max {
                    if outcome.admitted() >= cap {
                        outcome.stopped_at = Some(cap);
                        break 'agents;
                    }
                }

                // **Ingestion writes memory, and this is a rule rather than a lens.**
                // A conversation can teach the fleet a fact; it cannot grant a competence
                // or install a ferramenta, because those are written and configured
                // deliberately (ADR-0031). A promoter proposing into skills or tools is
                // refused before a single review call is spent, and the refusal is
                // recorded like any other, because a promoter that keeps trying is a
                // signal about the promoter.
                let kind = crate::index::kind_of(&proposal.folder);
                if kind != crate::index::Kind::Memory {
                    let decided = Decided {
                        proposal,
                        reviews: vec![Review {
                            lens: Lens::Scope,
                            accept: false,
                            reason: format!(
                                "hard rule, not a judgement: ingestion writes memory only, \
                                 and the target folder is {}-kind (ADR-0031)",
                                kind.label()
                            ),
                        }],
                        written: None,
                    };
                    if !dry_run {
                        record_rejection(fleet_root, &decided, today);
                    }
                    outcome.decided.push(decided);
                    continue;
                }

                // The independent input. Deterministic, no model, our own router.
                let evidence = evidence_for(memory, &proposal, top);

                let mut reviews = Vec::new();
                let mut reachable = true;
                for lens in Lens::ALL {
                    let p = review_prompt(&proposal, &evidence, lens);
                    match ask_model(reviewer, fleet_root, &p) {
                        Some(r) => reviews.push(parse_review(&r, lens)),
                        None => {
                            outcome.unreachable.push(format!("reviewer {}, on {}", lens.name(), proposal.slug));
                            reachable = false;
                            break;
                        }
                    }
                }
                if !reachable {
                    // A reviewer that could not be reached writes nothing. Degrading toward
                    // silence is the only safe direction for something that mutates the base.
                    continue;
                }

                let mut decided = Decided { proposal, reviews, written: None };

                if decided.accepted() && !dry_run {
                    let spec = crate::write::Note {
                        summary: decided.proposal.summary.clone(),
                        keys: decided.proposal.keys.clone(),
                        folder: decided.proposal.folder.clone(),
                        provenance: "agent".into(),
                        stage: CAPTURED.into(),
                        body: decided.proposal.body.clone(),
                    };
                    match crate::write::note(fleet_root, &decided.proposal.agent, &decided.proposal.slug, &spec) {
                        Ok(w) => decided.written = Some(w.note),
                        Err(e) => outcome.unreachable.push(format!("write {}: {e}", decided.proposal.slug)),
                    }
                } else if !decided.accepted() && !dry_run {
                    record_rejection(fleet_root, &decided, today);
                }

                outcome.decided.push(decided);
            }
        }
    }

    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proposal_carries_no_place_to_put_reasoning() {
        // The independence of the reviewer is a property of this type, so it is pinned by a
        // test rather than left to whoever edits the prompt next. If a `reasoning` or
        // `rationale` field is ever added, the reviewer can be shown promoter one's argument
        // and the second reader stops being a second reader.
        let rendered = format!("{:?}", Proposal {
            agent: "zed".into(),
            slug: "s".into(),
            folder: "knowledge".into(),
            summary: "x".into(),
            keys: vec!["a".into()],
            body: "b".into(),
            source: "inbox/x.md".into(),
        });
        for forbidden in ["reasoning", "rationale", "argument", "because", "confidence"] {
            assert!(
                !rendered.contains(forbidden),
                "Proposal gained a `{forbidden}` field, which is a channel for promoter one's \
                 argument to reach the reviewer. See the module header."
            );
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kb-promote-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn a_second_run_is_turned_away_while_the_first_holds_the_lock() {
        // The trigger is a session-end hook and sessions end together. If both runs get
        // in, both call promoter one on the same deposit file, both proposals look new to
        // the duplication lens because neither has written yet, and the base gains the
        // note twice. This is the only place that race is visible.
        let dir = scratch("two-runs");
        let first = Lock::take(&dir).expect("the first run takes it");
        assert!(first.took_over.is_none(), "nothing was there to take over");
        assert!(dir.join(LOCK).exists(), "the marker is a real file or it guards nothing");

        let second = Lock::take(&dir);
        assert!(second.is_err(), "a second run must not start while the first is in flight");

        drop(first);
        assert!(!dir.join(LOCK).exists(), "releasing the lock removes the marker");
        Lock::take(&dir).expect("once released, the next run may start");
    }

    #[test]
    fn a_marker_left_by_a_dead_run_does_not_switch_promotion_off_forever() {
        // The opposite failure to the one above, and the more expensive one: a machine
        // that lost power mid-run leaves a marker, every later run declines, and the
        // feature is off with nobody told. Backdating the file is how the staleness rule
        // gets tested without waiting an hour.
        let dir = scratch("stale");
        std::fs::write(dir.join(LOCK), "pid 4242\n").expect("plant a marker");
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(LOCK_STALE_SECS + 60);
        filetime_set(&dir.join(LOCK), old);

        let took = Lock::take(&dir).expect("a stale marker is debris, not a holder");
        let note = took.took_over.clone().expect("taking one over is never silent");
        assert!(note.contains("4242"), "the note names who left it: {note}");
    }

    /// Backdates a file's modified time. Written by hand because the crate has one
    /// dependency and adding a second to age a file in one test is not a trade.
    fn filetime_set(path: &Path, when: std::time::SystemTime) {
        let f = std::fs::OpenOptions::new().write(true).open(path).expect("open");
        f.set_modified(when).expect("set mtime");
    }

    #[test]
    fn the_cap_counts_what_was_admitted_so_a_dry_run_stops_where_a_real_run_would() {
        // `--max` has to bite identically with and without `--dry-run`, or the rehearsal
        // is not a rehearsal. `written()` is zero in a dry run, so counting writes would
        // make the cap silently infinite there.
        let admit = |accept: bool| Decided {
            proposal: Proposal {
                agent: "zed".into(),
                slug: "s".into(),
                folder: "knowledge".into(),
                summary: "x".into(),
                keys: vec!["a".into()],
                body: "b".into(),
                source: "inbox/x.md".into(),
            },
            reviews: Lens::ALL
                .iter()
                .map(|&lens| Review { lens, accept, reason: String::new() })
                .collect(),
            written: None,
        };

        let outcome = Outcome {
            decided: vec![admit(true), admit(false), admit(true)],
            ..Outcome::default()
        };
        assert_eq!(outcome.written(), 0, "a dry run writes nothing");
        assert_eq!(outcome.admitted(), 2, "and still admitted two, which is what the cap counts");
    }

    #[test]
    fn an_unreadable_review_refuses_and_says_what_came_back() {
        let r = parse_review("I think it is probably fine?", Lens::Scope);
        assert!(!r.accept, "an answer with no verdict must not write anything");
        assert!(
            r.reason.contains("probably fine"),
            "a refusal on a parse failure has to carry the reply, or the record cannot be \
             acted on: {}",
            r.reason
        );
    }

    #[test]
    fn a_verdict_is_read_through_whatever_the_model_wrapped_it_in() {
        // Every one of these is a shape a model actually produces when asked for two bare
        // lines. The first run of `kb promote` lost a decision to exactly this.
        for reply in [
            "**VERDICT:** accept\n**REASON:** it is new",
            "- VERDICT: accept\n- REASON: it is new",
            "VERDICT : accept\nREASON : it is new",
            "```\nVERDICT: accept\nREASON: it is new\n```",
            "verdict: ACCEPT\nreason: it is new",
        ] {
            let r = parse_review(reply, Lens::Duplication);
            assert!(r.accept, "did not read an accept out of: {reply:?}");
            assert_eq!(r.reason, "it is new", "lost the reason in: {reply:?}");
        }
    }

    #[test]
    fn an_ambiguous_verdict_refuses() {
        // A reviewer explaining why it did NOT reject writes both words. The safe reading
        // of that is the one that writes nothing.
        let r = parse_review("VERDICT: accept, I did not reject it\nREASON: fine", Lens::Scope);
        assert!(!r.accept, "a verdict naming both outcomes must not write anything");
    }

    #[test]
    fn a_bare_verdict_word_on_its_own_line_is_read() {
        assert!(!parse_review("reject\n", Lens::Scope).accept);
        assert!(parse_review("Accept.\n", Lens::Scope).accept);
        // But a sentence merely containing the word decides nothing.
        let r = parse_review("I would not reject this outright, it seems ok", Lens::Scope);
        assert!(!r.accept);
        assert!(r.reason.contains("It said:"), "prose is a parse failure, not a verdict");
    }

    #[test]
    fn a_reject_is_parsed_with_its_reason() {
        let r = parse_review(
            "VERDICT: reject\nREASON: yaron/calculations/formulas.md already states this",
            Lens::Duplication,
        );
        assert!(!r.accept);
        assert!(r.reason.contains("formulas.md"));
    }

    #[test]
    fn the_three_lenses_ask_three_different_questions() {
        // **The design property, pinned by a test because it was violated once.** The
        // contradiction prompt originally said that restating a fact differently was a
        // contradiction, which is duplication's question, and the first real run duly
        // produced three contradiction refusals whose reasons were all "the base already
        // holds this". Three lenses answering one question is one lens run three times.
        let c = Lens::Contradiction.question();
        let d = Lens::Duplication.question();
        let s = Lens::Scope.question();

        assert!(
            c.contains("NOT the question of whether the base already says this"),
            "the contradiction lens must hand restatement to duplication"
        );
        assert!(
            d.contains("not a contradiction"),
            "the duplication lens must claim restatement for itself"
        );
        assert!(
            s.contains("NOT the question of whether the note is true"),
            "the scope lens must not re-judge truth or novelty"
        );
        for (name, q) in [("contradiction", c), ("duplication", d), ("scope", s)] {
            assert!(
                q.contains("Reject") || q.contains("reject"),
                "{name} must make refusing as available as accepting"
            );
        }
    }

    #[test]
    fn unanimity_is_required_and_two_of_three_is_not_enough() {
        let p = Proposal {
            agent: "zed".into(),
            slug: "s".into(),
            folder: "knowledge".into(),
            summary: "x".into(),
            keys: vec!["a".into()],
            body: "b".into(),
            source: "inbox/x.md".into(),
        };
        let ok = |l| Review { lens: l, accept: true, reason: String::new() };
        let no = |l| Review { lens: l, accept: false, reason: "no".into() };

        let all_yes = Decided {
            proposal: p.clone(),
            reviews: vec![ok(Lens::Contradiction), ok(Lens::Duplication), ok(Lens::Scope)],
            written: None,
        };
        assert!(all_yes.accepted());

        let two_yes = Decided {
            proposal: p.clone(),
            reviews: vec![ok(Lens::Contradiction), ok(Lens::Duplication), no(Lens::Scope)],
            written: None,
        };
        assert!(!two_yes.accepted(), "two of three must not carry a proposal");
        assert_eq!(two_yes.refusal().map(|r| r.lens), Some(Lens::Scope));

        let short = Decided {
            proposal: p,
            reviews: vec![ok(Lens::Contradiction), ok(Lens::Duplication)],
            written: None,
        };
        assert!(!short.accepted(), "a lens that did not answer is not a lens that agreed");
    }

    #[test]
    fn ingestion_refuses_to_write_anything_but_memory() {
        // The hard rule from ADR-0031, checked at the cheapest point: kind_of on the
        // proposal's folder, before any review call is spent.
        use crate::index::{kind_of, Kind};
        assert_eq!(kind_of("knowledge/reference"), Kind::Memory);
        assert_eq!(kind_of("memory"), Kind::Memory);
        assert_ne!(kind_of("skills/novas"), Kind::Memory, "a conversation grants no skill");
        assert_ne!(kind_of("tools"), Kind::Memory, "a conversation installs no tool");
    }

    #[test]
    fn a_proposal_with_no_keys_is_dropped() {
        let reply = "PROPOSAL\nslug: a\nfolder: knowledge\nsummary: x\nbody:\ntext\nEND\n";
        assert!(
            parse_proposals(reply, "zed", "inbox/x.md").is_empty(),
            "a note with no keys is a note no question can reach, so it is not written"
        );
    }

    #[test]
    fn a_well_formed_proposal_parses() {
        let reply = "noise before\nPROPOSAL\nslug: sleep-target\nfolder: knowledge/reference\n\
                     summary: A sleep target\nkeys: sono, sleep, dormir\nbody:\n\
                     Eight hours.\nEND\ntrailing noise\n";
        let got = parse_proposals(reply, "yaron", "inbox/2026-08-20.md");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].slug, "sleep-target");
        assert_eq!(got[0].keys, vec!["sono", "sleep", "dormir"]);
        assert_eq!(got[0].body, "Eight hours.");
        assert_eq!(got[0].agent, "yaron");
    }
}
