//! The memory, as one object with three verbs.
//!
//! This is the contract. Everything that reaches the base goes through here: the
//! `serve` subcommand wraps it in MCP, the GUI will link it and call it directly,
//! and a hosted service later would wrap it in HTTP. Three surfaces, one pipeline,
//! and no way for them to answer differently, because there is only one place the
//! answer is computed.
//!
//! That property is the whole reason this type exists. `mcp.rs` was rebuilding the
//! pipeline itself, and a second caller doing the same would have been a second
//! chance to expand the aliases on one scorer and not the other, or to oversample by
//! a different factor. Both of those have already happened once in this codebase.
//!
//! Nothing here decides anything. `remember` measures and proposes; writing is a
//! separate, deliberate act, per ADR-0007.

use std::path::{Path, PathBuf};

use crate::base::Base;
use crate::index::{self, Entry};
use crate::remember::{self, Assessment};
use crate::retrieve::{self, Retrieved};
use crate::store::{Scope, Store};

/// The keyword score below which a top result is not worth answering from.
///
/// **Re-derived 2026-08-20, and the honest headline is that this constant does much
/// less than it used to.** It was 6.0, calibrated when the single wrong top result
/// scored 3.82 and the lowest correct one scored 9.55. Then the keyword lines widened
/// from a median of six terms to about seventy, every score moved up an order of
/// magnitude, and 6.0 stopped touching anything: `kb eval` reported it flagging 0 of 11
/// misses and demoting 0 of 9 hits. A gate that never fires is not conservative, it is
/// decoration.
///
/// The measurement it now sits on, from the same run:
///
/// - correct answers scored **19.92 to 188.56**
/// - wrong ones scored **21.39 to 133.43**, so **no number separates a hit from a miss**
/// - questions the set says to decline scored 0.00, 15.1, 23.8, 23.8, 39.6, 39.6, 57.3
///   and 71.6
///
/// 17.5 is the midpoint between the one declinable question that sits below every
/// correct answer (15.1) and the lowest correct answer (19.92). It buys exactly one
/// question and costs none, which is a small gain honestly derived rather than a large
/// one invented.
///
/// **What it cannot do is the part worth writing down.** Six of the eight declinable
/// questions score above every correct answer in the set, so no floor reaches them; they
/// are real matches on files that are not the answer. Separating those is the
/// classifier's job and the reason ADR-0027 exists. This constant's remaining job is the
/// one it still does perfectly: telling nothing from something, which is why "ok
/// obrigado" scores 0.00 and abstains.
///
/// A number this thinly evidenced does not get to hide. `kb eval` prints the hit and
/// miss score ranges on every run, so the gap this sits in is re-measured rather than
/// remembered, and it moves when the evidence moves. It has now moved twice.
pub const SCORE_FLOOR: f32 = 17.5;

/// **The corpus size `SCORE_FLOOR` was measured at.** The two constants are one fact,
/// "17.5 was the right floor for a fleet of 226 entries", and they are re-derived
/// together or not at all: change the floor without this and every other corpus size
/// silently inherits a calibration that was never made for it.
///
/// Why the floor has to travel with the size at all, ADR-0036. A matched key is worth
/// `W_KEYWORD × idf`, and idf grows with the corpus, because rarity needs a corpus to be
/// rare in. So a fixed 17.5 meant "two unique keys" on a base of ten entries, "one" on
/// the base it was measured on, and on a thousand entries a word that appears in fifty of
/// them clears it alone. The floor got harder as the base shrank and easier as it grew,
/// in exactly the two directions that hurt. Measured on the demo corpus before the change:
/// with fewer than five entries not one of the gold questions reached `hit`.
pub const FLOOR_CALIBRATED_AT: usize = 226;

/// Below this many entries the verdict is never `hit`, whatever the score.
///
/// The scaled floor corrects the ruler; this says when there is no ruler yet. With one
/// entry every word it carries has `df = 1`, so every word weighs the same and idf can
/// tell nothing apart: any shared word clears a floor built from it, not because the
/// evidence is good but because there is nothing to compare rarity against. Two entries
/// is the structural minimum, the first size at which a word in both weighs less than a
/// word in one. The corpus size sweep in ADR-0036 shows every gold question hitting the
/// right file from two entries up with no wrong file and every refusal holding, so a
/// larger number here would refuse bases that are measured to route correctly. Revisit
/// on the first fleet that misroutes at two to four entries.
pub const MIN_ENTRIES_TO_ROUTE: usize = 2;

/// The floor, in units of what one unique key scores in the corpus it was measured on.
///
/// `SCORE_FLOOR / (W_KEYWORD × idf_unique(FLOOR_CALIBRATED_AT))`: 17.5 over 6 × 4.74,
/// about 0.62. Read it as "a result has to score at least 62% of what a single word
/// found in exactly one note would score here". Derived rather than typed, so the two
/// constants above cannot drift from it and the floor on the calibration fleet is 17.5
/// to the last decimal, by construction.
pub fn floor_in_unique_keys() -> f32 {
    SCORE_FLOOR / (index::W_KEYWORD * index::idf_unique(FLOOR_CALIBRATED_AT))
}

/// The floor for a corpus of this many entries: the same fraction of a unique key,
/// re-expressed in this corpus's idf. Equal to `SCORE_FLOOR` at `FLOOR_CALIBRATED_AT`.
pub fn floor_for(entries: usize) -> f32 {
    floor_in_unique_keys() * index::W_KEYWORD * index::idf_unique(entries)
}

/// **The incumbent margin was built, measured and removed on 2026-08-19.** The constant
/// is gone; this note is what it left behind.
///
/// It was meant for follow-ups with no domain content, after "yes, run it and measure"
/// routed to the nutrition agent at 28.44 because a recipes file contains the word "out".
/// The diagnosis was wrong: the cause was an incomplete stopword list, and completing the
/// closed classes dropped every such message to a score of **zero**, where the confidence
/// floor already handles them.
///
/// What the margin actually did was freeze the first answer. A session that started on the
/// wrong agent stayed there, because a challenger had to double the incumbent to take the
/// conversation, and the hook kept reporting "still steve" through four messages about
/// routing and interface design. **A rule that requires the first answer to be right is
/// not a correction mechanism, it is a commitment mechanism.**
///
/// Third time this shape has appeared: reasoned into existence, kept until `kb eval` and a
/// replay disagreed, then removed. See `MIN_MARGIN` and Z33.

/// The margin over the runner-up is **measured, reported, and deliberately not part
/// of the verdict.** This constant is what it would have been.
///
/// The prediction, registered before the measurement: an IDF weighted sum scales with
/// the query, so a fixed floor means different things to different questions, and the
/// scale free margin should be the primary gate. `kb eval` refuted it on the first
/// run, 2026-08-18, 19 questions:
///
/// - Correct top results had margins of **1.00, 1.12, 1.16, 1.18, 1.19, 1.21, 1.30,
///   1.30, 1.39, 1.44, 1.86, 2.20, 2.61, 2.71, 3.34, 4.00 and 7.00**. There is no cut
///   anywhere in that range that does not throw away correct answers, and 1.5 threw
///   away twelve of eighteen.
/// - The only keyword miss in the set scored **0.00**, so the margin had nothing to
///   discriminate that the floor did not already catch.
///
/// **Why the reasoning failed, because the reasoning is the reusable part.** The floor
/// is not being asked to rank hits against each other, where query scale would matter.
/// It is being asked whether *any* meaningful term matched at all, and the keyword
/// scorer assigns a file no score whatsoever unless one did. So the real distribution
/// is not continuous, it is a gap between zero and the first real match, and query
/// scale moves both sides of that gap together.
///
/// Kept as a named constant rather than deleted because the number is evidence: the
/// next person to propose margin gating should meet it before spending the work.
pub const MIN_MARGIN: f32 = 1.5;

/// A question the base could not answer, handed back to whoever asked.
///
/// **It travels rather than only landing in a file, and that is the change F-03
/// asked for.** The log lives beside the fleet, which works on a machine somebody
/// owns and cannot work on a hosted one: the filesystem is read only, the instance is
/// gone a second later, and the failure to write reached the caller as one line on the
/// stderr of a child process. The measured result was a recall loss log holding two
/// lines from a laptop while six real questions went unrecorded in production.
///
/// So the loss is returned whether or not it could be stored, with `error` saying why
/// when it could not. A caller with nowhere to write can persist this where its own
/// stack already writes, and one that can write gets the same object plus a file.
#[derive(Debug, Clone)]
pub struct RecallLoss {
    pub question: String,
    /// The vocabulary the base does know that looks like what was asked. Empty is an
    /// honest answer: trigram overlap measures spelling, so a base with no
    /// orthographic neighbour of any question word has nothing to offer.
    pub looked_like: Vec<String>,
    /// The day it was seen, in the log's own format, so a caller storing this
    /// elsewhere keeps the field the log would have kept.
    pub date: String,
    /// Where the log was written, or where the attempt was made.
    pub log: std::path::PathBuf,
    /// Why the write failed, when it did.
    pub error: Option<String>,
}

impl RecallLoss {
    /// Whether the log holds this too, or whether the caller is the only copy.
    pub fn recorded(&self) -> bool {
        self.error.is_none()
    }
}

/// One candidate the gate refused, with the keys the file actually carries.
///
/// **The half a recall loss log cannot hold.** `kb-misses.txt` records the question and
/// what the base offered back on the day it was asked. It cannot record which file
/// nearly caught it, because that depends on the base as it stands now and changes every
/// time a note is written. So this is computed against today's index and handed to the
/// reader beside the logged question.
///
/// Owned rather than borrowing an `index::Hit`, for two reasons that point the same way:
/// the caller holds one list per logged question while it prints, and `keys` has to be
/// copied out of the entries anyway.
///
/// `keys` empty is a finding rather than a blank. It means the index holds no entry for
/// this file at all, so the text scorer reached a file the keyword scorer can never see,
/// and the work is a `Search for:` line rather than an alias.
#[derive(Debug, Clone)]
pub struct NearMiss {
    pub base: String,
    pub rel: String,
    /// From the map entry. Empty when only the text scorer found the file.
    pub title: String,
    /// Zero when only the text scorer found it, which is the case worth reading: the
    /// question's words are in the body and not on the `Search for:` line.
    pub keyword_score: f32,
    /// Which scorer ranked it and at what position, in `retrieve::fuse`'s own wording.
    pub why: Vec<String>,
    /// The words the file declares it can be found by. This is the line the reader edits.
    pub keys: Vec<String>,
}

/// What the router is willing to claim about its own top result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Above the noise floor and separated from the runner-up. Answer from it.
    Hit,
    /// Something ranked, but nothing that distinguishes it from a coincidence.
    /// The passages are still returned: this is a warning, never a filter, for the
    /// same reason `no_agreement` is. Dropping weak results loses real answers, and
    /// saying "this is a guess" loses nothing.
    Guess,
    /// Nothing ranked at all, in either scorer.
    Nothing,
}

impl Verdict {
    /// The wire name, for surfaces a program reads rather than a person.
    ///
    /// Separate from the sentences the terminal prints on purpose. Those are worded
    /// for whoever is looking at them (`kb answer` says "guess, read the sources
    /// yourself", `kb eval` says "none") and are free to change with the surface. This
    /// one is a contract: a caller branching on it breaks the day it is reworded, so
    /// it lives on the type and has exactly one home.
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Hit => "hit",
            Verdict::Guess => "guess",
            Verdict::Nothing => "nothing",
        }
    }
}

/// The evidence behind a verdict, so a caller can disagree with it.
///
/// All three numbers travel with the verdict deliberately. A gate that reports only
/// its conclusion cannot be argued with, cannot be recalibrated from logs, and turns
/// every future question about it into a re-measurement from scratch.
#[derive(Debug, Clone, Copy)]
pub struct Confidence {
    pub verdict: Verdict,
    /// How many of the two scorers ranked the top file.
    pub agreement: usize,
    /// The top file's raw keyword score. Zero when only the text scorer found it.
    pub keyword_score: f32,
    /// Top keyword score divided by the best runner-up's. 1.0 when nothing else
    /// scored, because standing alone is not the same as winning.
    pub margin: f32,
    /// The floor this verdict was measured against, for this corpus. Travels with the
    /// verdict because it is no longer one number: every surface that prints "against a
    /// floor of" reads it from here, and a caller disagreeing with the gate needs the
    /// threshold that actually applied and not the one from the calibration fleet.
    pub floor: f32,
}

impl Confidence {
    /// The one line a surface prints when it has to say what it is handing over.
    /// Here rather than at each call site because `main.rs`, `mcp.rs` and the tray
    /// all need it, and three of them wording it separately is how they came to
    /// disagree about the miss message before.
    pub fn note(&self) -> Option<&'static str> {
        match self.verdict {
            Verdict::Hit => None,
            Verdict::Guess => Some(
                "this is a guess, not an answer: the top result is too weak or too \
                 close to the runner-up to distinguish from a coincidence",
            ),
            Verdict::Nothing => Some("nothing matched, in either scorer"),
        }
    }
}

/// The state a refusal is refusing from, so the refusal can name the next act.
///
/// **A refusal that stops at "nothing matched" is unintelligible at the moment it is
/// read.** Every number that would explain it is already computed and already travels:
/// [`Confidence`] carries the floor it measured against, [`Memory`] knows its entry
/// count and which of its files no question can reach. What was missing was somewhere to
/// put them together, so each surface either said nothing or invented its own sentence.
///
/// Nothing here is recomputed. `floor` and `scored` are read off the `Confidence` the
/// gate actually used, never re-derived from [`Memory::floor`], because the two can
/// legitimately differ: the caller may be holding a verdict taken over a different corpus
/// than the one it is printing beside, and the number that refused the question is the
/// one the reader needs.
#[derive(Debug, Clone, Copy)]
pub struct Shortfall {
    /// Fleet wide, across every opened root, because [`Memory::entry_count`] is and the
    /// floor is derived from it. Never worded as "this base" anywhere it is printed.
    pub entries: usize,
    pub agents: usize,
    /// Off the `Confidence`, not off the memory. See the type comment.
    pub floor: f32,
    /// The top keyword score. Exactly zero on every `Verdict::Nothing`, which is what the
    /// branch in [`Shortfall::lines`] exists for.
    pub scored: f32,
    /// How many files across the open bases the router can build no entry for. Same
    /// population `kb check` reports as E02, because both read `index::is_exempt`.
    pub unreachable: usize,
}

/// One count and its noun, agreeing. Two irregular plurals is one too many to leave to
/// `format!`, and "1 entries" in a refusal is the kind of seam that makes a reader
/// distrust the numbers beside it.
fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Shortfall {
    /// The sentences a surface prints under its refusal, whole, most structural cause
    /// first, with no indentation and no trailing newline.
    ///
    /// **Sentences and not a paragraph, because the surfaces disagree about shape and
    /// must not disagree about content.** The terminal indents each by two spaces, the
    /// MCP reply joins them into one paragraph, and `kb boot` takes the first two and
    /// injects them into a model's context. Wording them at those three sites is how the
    /// miss message came to have three different meanings before, and none of the three
    /// had a test, because none of them is reachable from one: `main.rs` prints with
    /// `println!` and its tests assert on payloads and on files. Holding the words here
    /// leaves each surface choosing only which true sentences to show.
    ///
    /// **Every sentence is conditioned on a fact, and that is the whole safety rule.** A
    /// refusal that instructs can instruct wrongly, which is worse than one that says
    /// nothing: it sends somebody to repair a base that is not broken. So a large, fully
    /// keyed fleet that simply does not cover the question gets exactly one sentence, the
    /// true one about what happened, and no remedy at all.
    ///
    /// **Empty on an empty base, deliberately.** [`Memory::is_empty`] already has its own
    /// reply on the terminal and on the MCP surface, and that reply is right: the library
    /// has not been written yet, which is a fact about the base and not about the
    /// question. Two explanations of one fact is the drift this codebase keeps paying for.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.entries == 0 {
            return out;
        }

        // Structural first: below the minimum nothing can be a hit whatever it scores, so
        // every remedy that fits a large base is wasted work here.
        if self.entries < MIN_ENTRIES_TO_ROUTE {
            out.push(format!(
                "The library holds {} across {} in all, and under {} entries nothing can \
                 be a hit whatever it scores: with one entry every word has the same \
                 rarity, so there is nothing for the ranking to tell apart. A second note \
                 is what changes that, not a better question.",
                counted(self.entries, "entry", "entries"),
                counted(self.agents, "agent", "agents"),
                MIN_ENTRIES_TO_ROUTE
            ));
        }

        // Then the likeliest authoring cause, because a file with no keys is on disk,
        // readable by a person, and scores zero on every question ever asked. A count and
        // not a list: a list of eighty swamps the reply, and a count cannot name files, so
        // it hands over the verb that can.
        if self.unreachable > 0 {
            let (carry, object, subject, score) = match self.unreachable {
                1 => ("carries", "it", "it", "scores"),
                _ => ("carry", "them", "they", "score"),
            };
            out.push(format!(
                "{} across the open bases {carry} no `Search for:` line, so the index \
                 holds no entry for {object} and {subject} {score} zero on every \
                 question. `kb check` names {object}.",
                counted(self.unreachable, "markdown file", "markdown files")
            ));
        }

        // Then what this particular question did, which is one of two things and never
        // both. **The floor is named only where it actually refused something.** A
        // `Verdict::Nothing` reaches here with a score of exactly zero, because
        // `index::route` keeps a hit only under `score > 0.0` and `confidence_of` returns
        // `Nothing` only from its `hits.first()` early return: no term matched any key,
        // the floor was never reached, and printing "0.0 against a floor of 17.5" reads as
        // *nearly* and sends the reader to recalibrate a threshold that did nothing.
        if self.scored > 0.0 {
            if self.scored < self.floor {
                out.push(format!(
                    "The top result scored {:.1} against a floor of {:.1}, which is the \
                     floor for a corpus of this size and not a fixed number.",
                    self.scored, self.floor
                ));
            }
            // Above the floor and still refused is a third thing, and this type does not
            // know which: `kb boot` gets here when the classifier named nobody, and that
            // is not a scoring failure at all. Saying nothing is the honest answer.
        } else {
            out.push(
                "No term in the question matched any key here, so nothing was ranked at \
                 all: this is a vocabulary miss and not a near miss, and asking again with \
                 the words the notes declare is what changes it."
                    .to_string(),
            );
        }

        out
    }
}

/// Everything one question produces, computed in one pass.
///
/// Bundled rather than returned as a tuple because the three travel together to every
/// surface, and because a caller assembling them from separate calls is exactly how
/// the keyword scorer and the text scorer came to see different query terms once
/// before. One call, one expansion, no way to pair them wrong.
pub struct Answer {
    /// The passages, fused, because assembling a reading is what fusion is better at.
    pub found: Vec<Retrieved>,
    /// The verdict, from the keyword ranking, because judging is what it is better at.
    pub confidence: Confidence,
    /// The owner, from the same ranking and for the same reason.
    pub agent: Option<AgentChoice>,
    /// The keyword scorer's own first choice as `base/path`. Carried because it is
    /// frequently not the fused first choice, and a caller that wants the single most
    /// likely file rather than a reading list should be given the better one.
    pub keyword_top: Option<String>,
}

/// Which agent a question belongs to, with the evidence for it.
#[derive(Debug, Clone)]
pub struct AgentChoice {
    pub agent: String,
    /// Summed fused score of this agent's files in the result set.
    pub score: f64,
    /// How many contributions are in there. On the fused fold that is files; on the
    /// keyword fold it is species with evidence (at most three per agent, ADR-0031),
    /// which is the breadth half of the evidence stated more honestly: fifty nine
    /// matching files and one matching file are the same one species.
    pub files: usize,
    /// Over the runner-up agent. Infinite when only one agent scored at all, which
    /// unlike the file level case really is maximum confidence: no other base in the
    /// fleet had anything to say.
    pub margin: f64,
    /// How many agents scored anything. One contender is a different situation from
    /// four, and the number is cheap to carry and expensive to recover later.
    pub contenders: usize,
    /// Every agent that scored, best first. Carried so a caller holding an incumbent
    /// can ask how the incumbent did, which a single winner cannot answer.
    pub totals: Vec<(String, f64)>,
}

/// Where an agent's index lives: with the agent, never anywhere else.
///
/// The shared index it replaces defaulted to `.kb/index.db` **relative to the
/// working directory**, so which database you got depended on where you happened to
/// be standing. That cost three separate incidents in one week: a benchmark that
/// measured an index holding one base while reporting on three, an MCP server that
/// opened an empty index and answered "nothing matched", and a routing diagnosis
/// run against the wrong file and believed.
///
/// One index per agent also makes the privacy property structural rather than
/// enforced: **a missing predicate cannot leak a file that is not in the database
/// you opened.** That argument was accepted twice before it was applied here.
pub fn index_path(base_root: &Path) -> PathBuf {
    base_root.join(".kb").join("index.db")
}

/// One agent, with its own index.
pub struct Agent {
    pub name: String,
    pub root: PathBuf,
    /// Whether this base can be chosen as the one who **answers**, as opposed to one
    /// that gets read.
    ///
    /// **The two are not the same thing and conflating them misrouted a real question.**
    /// `decisions/` is attached to the fleet so every agent can read the records, and the
    /// router duly offered it as the agent who should reply. A library is not a librarian.
    ///
    /// The discriminator is `agent.txt`, which needs no new configuration because its own
    /// header already says what it is for: *read by the orchestrator to name and route*. A
    /// base that does not declare a name and a role has not claimed the job. That keeps
    /// ADR-0011's rule that the fleet is found by shape rather than declared in a list.
    pub routable: bool,
    /// Files in this base the router can build no entry for, qualified as `base/rel`.
    ///
    /// An authoring problem, and one nothing tells anybody about until it is carried here:
    /// the file is on disk, a person can open and read it, and it scores zero on every
    /// question. `kb check` reports the same set as E02, and nobody is required to run
    /// `kb check`.
    pub unreachable: Vec<String>,
    /// Files on disk this base's index holds no chunks for, qualified as `base/rel`.
    ///
    /// A different problem with a different fix: `kb index` has not run since these were
    /// written. **Which scope produced the number matters and is not obvious.** An index
    /// synced without `--all` and opened with `--all` correctly reports every private file
    /// as unindexed, and the reverse reports nothing at all, so two runs can disagree
    /// while both are telling the truth.
    pub unindexed: Vec<String>,
    store: Store,
}

pub struct Memory {
    entries: Vec<Entry>,
    aliases: Vec<(String, String)>,
    scope: Scope,
    /// How a miss is answered with the base's own vocabulary. Private, and swapped only
    /// by [`Memory::with_suggester`], for the reason this whole type exists: a surface
    /// holding its own suggester is a second definition of the same answer, and two of
    /// those have already come to disagree here. One place to land, one place to look.
    suggester: Box<dyn crate::suggester::Suggester>,
    /// One per base, each with its own index, in the order the fleet was expanded.
    pub agents: Vec<Agent>,
    /// The paths as given, before expansion. Kept because the fleet root is where
    /// `fleet.txt` lives, and the identity tier reads it: after expansion only the
    /// agent directories remain, and the fleet's own name is not in any of them.
    pub opened: Vec<PathBuf>,
    /// Bases that were left out, with the fleet still open. Empty since ADR-0034: the one
    /// reason a base used to be skipped, git not answering for its privacy, no longer
    /// exists. Kept on the contract because `kb route --json` carries it and a caller
    /// reads it; it is the field a future reason to leave a base out belongs in.
    pub skipped: Vec<PathBuf>,
    /// True when any index had to be discarded on open. The caller has to surface
    /// this: an emptied index answers "nothing matched", which reads as "the base
    /// does not cover this".
    pub index_was_rebuilt: bool,
}

#[derive(Debug)]
pub enum OpenError {
    Unreadable(PathBuf, std::io::Error),
    /// The index could not be opened. Carries whether one was there at all, because
    /// that decides what the reader should do next and the underlying error does not
    /// say: `Store::open` creates the index's parent before opening, so a base whose
    /// `.kb/` never shipped fails as a permission error on a read only filesystem, and
    /// permission is the proximate cause and the wrong thing to go and fix.
    Store { path: PathBuf, reason: String, existed: bool },
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Unreadable(p, e) => write!(f, "cannot read {}: {e}", p.display()),
            // **The cause first, then the fix, then the symptom.** This said only the
            // symptom, and a deployment that had left `.kb/` out of its bundle read
            // `cannot open the index: Permission denied (os error 13)` and went looking
            // at file permissions. The underlying error stays, in brackets, because it
            // is what distinguishes one cause from another when the guess is wrong.
            OpenError::Store { path, reason, existed: false } => write!(
                f,
                "cannot open the index at {}: nothing has been indexed here yet. \
                 Run `kb index` on the base, and make sure the .kb/ directory it \
                 writes reaches the machine that answers the question. ({reason})",
                path.display()
            ),
            OpenError::Store { path, reason, existed: true } => {
                write!(f, "cannot open the index at {}: {reason}", path.display())
            }
        }
    }
}

impl Memory {
    /// Opens one or more bases against one index.
    ///
    /// A path may be a base or a **fleet root**: a directory that is not itself a
    /// base but whose immediate children are. Accepting both is deliberate. Requiring
    /// a particular arrangement would be an assumption about the user's filesystem,
    /// and ADR-0008 says the base is addressed by path and never assumed. A tidy
    /// layout is then a convenience the user may adopt, not a shape we impose.
    pub fn open(paths: &[&Path], private: bool) -> Result<Memory, OpenError> {
        let mut entries = Vec::new();
        let mut aliases = Vec::new();
        let mut agents = Vec::new();
        let mut index_was_rebuilt = false;
        // Bases the open had to leave out. Carried on the Memory rather than logged, so a
        // surface can say which ones went missing instead of quietly answering from fewer.
        // Nothing fills it since ADR-0034; see the field's own comment.
        let skipped: Vec<PathBuf> = Vec::new();

        for root in expand_roots(paths) {
            let base = Base::discover(&root, private)
                .map_err(|e| OpenError::Unreadable(root.clone(), e))?;

            // **There is no longer a base that cannot be served.** This is where a base
            // git could not answer for was left out, with a notice, under the rule that
            // unknown is not public. ADR-0034 removed the question: the private layer is a
            // declaration read off the base itself, so it cannot be unknown, and a folder
            // with a note in it is served the moment it exists. `skipped` stays on the
            // contract, empty, because callers read it and a field that vanishes breaks
            // them; the day a base is left out for some other reason, it is the field to
            // name it in.

            // Asked before the open, because opening is what creates it.
            let index = index_path(&root);
            let existed = index.exists();
            let store = Store::open(&index).map_err(|e| OpenError::Store {
                path: index.clone(),
                reason: e.to_string(),
                existed,
            })?;
            index_was_rebuilt |= store.rebuilt;

            // **What the walk could not reach, carried rather than dropped.** `build`
            // classified every file it walked, so this costs nothing beyond the move.
            let name = index::base_name(&root);
            let built = index::build(&base);
            entries.extend(built.entries);
            let unreachable: Vec<String> =
                built.unreachable.iter().map(|rel| format!("{name}/{rel}")).collect();

            // **The derived lag, over two lists that already exist.** `unwrap_or_default`
            // on a store error is deliberate: a diagnostic number must never be the thing
            // that refuses to open a base.
            let indexed: std::collections::HashSet<String> =
                store.indexed_paths().unwrap_or_default().into_iter().collect();
            let unindexed: Vec<String> = base
                .files
                .iter()
                // `Store::sync` skips the map on purpose, because it is the keyword
                // scorer's corpus. Without the same filter here every base reports a
                // permanent lag of one, and a number that is never zero is a number people
                // learn to scroll past.
                .filter(|f| Some(&f.rel) != base.map.as_ref())
                .filter(|f| !indexed.contains(&f.rel))
                .map(|f| format!("{name}/{}", f.rel))
                .collect();

            aliases.extend(base.aliases.clone());
            let routable = root.join("agent.txt").is_file();
            agents.push(Agent {
                name: name_of(&root),
                root,
                routable,
                unreachable,
                unindexed,
                store,
            });
        }

        Ok(Memory {
            entries,
            aliases,
            scope: if private { Scope::All } else { Scope::Public },
            suggester: Box::new(crate::suggester::Trigram),
            agents,
            skipped,
            opened: paths.iter().map(|p| p.to_path_buf()).collect(),
            index_was_rebuilt,
        })
    }

    /// Swaps in another way of measuring what a question looks like. Nothing but the
    /// suggestion path changes: see [`crate::suggester`] for why that is the only place
    /// a second scorer is allowed to land, and for the bar
    /// [[0018-no-model-in-the-retrieval-path]] sets before one is written.
    ///
    /// **Consuming, rather than `&mut self`, and that is the point.** A surface holding
    /// a `&mut Memory` could otherwise swap the suggester between the gate and the miss
    /// reply, so one question would be judged by one thing and answered by another. Take
    /// the memory or leave it.
    ///
    /// Its only caller today is the test module, which installs a suggester that answers
    /// everything and one that panics on sight. That is deliberate: the property worth
    /// pinning is that neither can move a verdict, and neither belongs in production.
    pub fn with_suggester(mut self, suggester: Box<dyn crate::suggester::Suggester>) -> Memory {
        self.suggester = suggester;
        self
    }

    /// The fleet describing itself: its name, its role, and every agent with theirs.
    ///
    /// **A lookup, not a verb.** It answers nothing; it hands over the roster so
    /// whoever is orchestrating can answer. That split is deliberate and was arrived at
    /// the hard way: an earlier version classified questions here and replied with
    /// strings we had written, which is code doing a model's job badly.
    ///
    /// What stays is the half retrieval genuinely cannot do. The fleet's name lives in
    /// `fleet.txt`, not in any knowledge file, so ranking a base for it is searching
    /// the wrong corpus. That is why "quem é você?" came back with marketing
    /// psychology: `index::normalise` drops `voce`, `e` and `qual` as stopwords, the
    /// question survived as the single term `quem`, and `quem` is common in notes about
    /// audience research. No amount of tuning fixes a question aimed at the wrong file.
    pub fn describe(&self) -> crate::fleet::Description {
        let root = self.opened.first().cloned().unwrap_or_default();
        crate::fleet::Description {
            fleet: crate::fleet::card(&root, MANIFEST, &name_of(&root)),
            members: self
                .agents
                .iter()
                .map(|a| crate::fleet::Member {
                    card: crate::fleet::card(&a.root, "agent.txt", &a.name),
                    root: a.root.clone(),
                })
                .collect(),
        }
    }

    /// Every file on every open base that matches the facets, with nothing ranked.
    ///
    /// **A lookup, not a verb**, filed here beside [`Memory::describe`] for the same
    /// reason: the contract is three verbs over a set of bases, and this answers no
    /// question. It hands over what the library holds and lets the caller decide. Calling
    /// it a fourth verb would make three doc headers lie, in `lib.rs`, at the top of this
    /// file and on the front page of the README.
    ///
    /// **The privacy rule is one comparison and it is the only dangerous line here.**
    /// `self.scope == Scope::All` is the round trip of the `private` bool `Memory::open`
    /// handed `Base::discover`, so a listing sees exactly the files the memory was opened
    /// with. Invert it and every private folder in the fleet is listed by default, over
    /// MCP, to whatever model is reasoning. Nothing here re-derives which folders those
    /// are: `base::private_layer` stays the single declaration, ADR-0034.
    ///
    /// **It walks the disk again rather than caching a listing on `Memory::open`.** The
    /// alternative is a `Vec<Listed>` filled beside `entries.extend(...)`, and its cost
    /// lands in the wrong place: `kb boot` calls `Memory::open` on every single user
    /// message through the `UserPromptSubmit` hook, and would then pay one front matter
    /// parse and one heading scan per file per message to serve a command nobody ran.
    /// This way the second walk is paid by the caller that asked for it, and `Memory`
    /// gains no new state.
    pub fn list(
        &self,
        filter: &crate::list::Filter,
    ) -> Result<Vec<crate::list::Listed>, OpenError> {
        let mut out = Vec::new();
        for agent in &self.agents {
            let base = Base::discover(&agent.root, self.scope == Scope::All)
                .map_err(|e| OpenError::Unreadable(agent.root.clone(), e))?;
            out.extend(crate::list::build(&base).into_iter().filter(|l| filter.matches(l)));
        }
        Ok(out)
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// The roots actually opened, after any fleet root was expanded.
    pub fn roots(&self) -> Vec<&Path> {
        self.agents.iter().map(|a| a.root.as_path()).collect()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn alias_count(&self) -> usize {
        self.aliases.len()
    }

    /// Every file across the open bases that no question can reach, qualified as `base/rel`.
    ///
    /// **Separate from [`Memory::unindexed`] because they are two failures wanting opposite
    /// work.** This one is a file nobody wrote a `Search for:` line for, and the fix is
    /// somebody writing one. The other is a `kb index` that never ran, and the fix is
    /// running it. One combined number would say a base has a problem and refuse to say
    /// what to do about it.
    pub fn unreachable(&self) -> Vec<&str> {
        self.agents
            .iter()
            .flat_map(|a| a.unreachable.iter().map(|s| s.as_str()))
            .collect()
    }

    /// Every file on disk the indexes hold no chunks for, qualified as `base/rel`.
    pub fn unindexed(&self) -> Vec<&str> {
        self.agents
            .iter()
            .flat_map(|a| a.unindexed.iter().map(|s| s.as_str()))
            .collect()
    }

    /// How many paths a surface names when it reports what it cannot reach.
    ///
    /// One number in one place, for the reason [`Memory::SUGGEST_LIMIT`] carries below:
    /// two surfaces free to choose their own cap is one question answered two ways. **The
    /// count is always exact and only the list is capped**, because a short array beside a
    /// count taken from the same short array is a payload that lies about the size of the
    /// problem.
    pub const PATHS_SHOWN: usize = 8;

    /// True when there is nothing to search, as opposed to nothing found.
    ///
    /// **These are different answers and the difference is the whole first run.** A
    /// base with no map entries cannot match anything, so telling its owner that the
    /// keyword lines may not carry the words a real question uses blames them for a
    /// library they have not written yet. Measured on a fresh `kb init` on
    /// 2026-08-17: the header printed `0 entries` and the next line blamed the
    /// question.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many words the base offers back on a miss.
    ///
    /// Small on purpose. The whole keyword space is 849 terms and about 12 KB on this
    /// fleet, and handing all of it over on every miss would make the failure path the
    /// most expensive reply any surface produces. A shortlist is what a caller can act
    /// on; a dump is something it has to route through a second time.
    ///
    /// One number, here, because it lived in two places: `mcp.rs` had its own constant
    /// with this reasoning attached and `main.rs` had the literal 8 with none, and two
    /// surfaces free to choose meant one question could be answered two ways.
    pub const SUGGEST_LIMIT: usize = 8;

    /// **The recall loss path, whole, for every surface: decide, record, and answer
    /// with the vocabulary to offer.** Empty when this was not a loss.
    ///
    /// A refusal is the loss. A `guess` is not: it was served, with a warning, and a
    /// question that reached the caller is not a question the base failed to reach.
    /// That line is a decision and not an obvious truth, so it is pinned by a test:
    /// moving it changes what `kb-misses.txt` counts, and both of ADR-0006's and
    /// ADR-0013's revisit triggers are measured against that file.
    ///
    /// **Why the whole path and not just the writing.** The writing was already here,
    /// with a comment saying two callers building it separately is how they came to
    /// disagree twice. They disagreed a third time anyway, because only the writing
    /// moved and the *deciding* stayed at the call site: `route --json` and the MCP
    /// `retrieve` tool asked whether the fused list was empty, the terminal `route` and
    /// the MCP `route` tool asked the keyword list, and `kb answer` suggested without
    /// ever recording at all. Four definitions of one measurement, and the surface a
    /// deployment uses had the hole in it: a question the text scorer answered and the
    /// gate refused went unrecorded everywhere it mattered, which is exactly the loss
    /// worth having, because the base holds the answer and only its keys are wrong.
    /// F-02 in `reports/2026-08-29-first-integration.md`.
    ///
    /// A predicate would not have fixed it. Surfaces would have kept pairing it with
    /// their own `suggest` and their own write, which is three chances to differ. One
    /// call, one behaviour, nothing left at the call site to get wrong.
    pub fn recall_loss(&self, question: &str, confidence: &Confidence) -> Option<RecallLoss> {
        if self.is_empty() || confidence.verdict != Verdict::Nothing {
            return None;
        }
        let looked_like = self.suggest(question, Self::SUGGEST_LIMIT);
        let date = crate::misses::today();
        let root = self.opened.first().cloned().unwrap_or_default();
        let log = crate::misses::path_in(&root);
        let error = crate::misses::record(&root, question, &looked_like, &date).err();

        Some(RecallLoss {
            question: question.to_string(),
            looked_like,
            date,
            log,
            error,
        })
    }

    /// What the base knows that looks like what was asked, for when nothing matched.
    ///
    /// Belongs on the contract rather than at each call site for the same reason the
    /// three verbs do: `mcp.rs` and `main.rs` both have a miss path, and two callers
    /// building the same answer separately is how they came to disagree before.
    ///
    /// **Which comparison answers this is now a field and not a call.** The body used to
    /// name `index::suggest`, so "a second scorer lands here" was true of the call graph
    /// and stated nowhere. It is one line on [`crate::suggester::Suggester`] instead,
    /// with the default still being the trigram overlap that has always run.
    ///
    /// Reached from one place, [`Memory::recall_loss`], which has already returned `None`
    /// for every verdict but `Verdict::Nothing` before it gets here. That ordering, and
    /// not this signature, is what makes a suggester incapable of turning a refusal into
    /// a hit.
    pub fn suggest(&self, question: &str, limit: usize) -> Vec<String> {
        self.suggester.words(question, &self.entries, limit)
    }

    /// Which files a question should open. No text is read, so this is cheap and can
    /// be asked speculatively.
    pub fn route(&self, question: &str, top: usize) -> Vec<index::Hit<'_>> {
        index::route(question, &self.entries, &self.aliases, top)
            .into_iter()
            // A map entry naming a note with no file behind it has an empty path.
            // Offering it hands the caller something it cannot open.
            .filter(|h| !h.entry.rel.is_empty())
            .collect()
    }

    /// The passages themselves, fused from both scorers.
    ///
    /// The aliases are expanded exactly once and handed to both scorers. Expanding
    /// for one and not the other was a real bug: a Portuguese question routed
    /// correctly by keyword and matched zero chunks by text.
    pub fn retrieve(&self, question: &str, top: usize) -> Vec<Retrieved> {
        let terms = index::expand_query(question, &self.aliases);
        let keyword = index::route(
            question,
            &self.entries,
            &self.aliases,
            top * retrieve::KEYWORD_OVERSAMPLE,
        );
        let text = self.search_all(&terms, top * retrieve::TEXT_OVERSAMPLE);

        retrieve::fuse(&keyword, &text, top)
    }

    /// Searches every agent's index and merges the results **by rank**.
    ///
    /// BM25 scores are not comparable across indexes: the value depends on the term
    /// statistics of the corpus that produced it, so a 4.2 from Yaron's eighteen
    /// files means something different from a 4.2 from Steve's fifty-eight. Sorting
    /// the union by score would silently favour whichever corpus happens to make its
    /// numbers larger.
    ///
    /// Ranks need no conversion, which is the same reason RRF fuses the keyword and
    /// text lists by position. Round robin therefore treats each agent's best hit as
    /// comparable to every other agent's best hit, which is exactly the assumption
    /// already load bearing one level up.
    fn search_all(&self, terms: &[String], limit: usize) -> Vec<crate::store::Hit> {
        let per_agent: Vec<Vec<crate::store::Hit>> = self
            .agents
            .iter()
            .map(|a| a.store.search(terms, limit, self.scope).unwrap_or_default())
            .collect();

        let mut merged = Vec::new();
        let deepest = per_agent.iter().map(|l| l.len()).max().unwrap_or(0);
        'outer: for rank in 0..deepest {
            for list in &per_agent {
                if let Some(hit) = list.get(rank) {
                    merged.push(hit.clone());
                    if merged.len() == limit {
                        break 'outer;
                    }
                }
            }
        }
        merged
    }

    /// Measures a claim against what the base already says and proposes ADD, UPDATE
    /// or NOOP with the evidence. **Writes nothing and decides nothing.**
    pub fn remember(&self, claim: &str) -> Assessment {
        let terms = index::expand_query(claim, &self.aliases);
        let hits = self.search_all(&terms, remember::EVIDENCE_WIDTH);
        remember::assess(claim, &terms, &hits)
    }

    /// True when nothing was ranked by **both** scorers.
    ///
    /// Agreement between two independent scorers is the strongest signal available
    /// without a model, which is the whole argument for RRF and is written down in
    /// `retrieve.rs`. It was computed and never used as a gate, so a result nobody
    /// agreed on was presented exactly like one everybody agreed on.
    ///
    /// Measured on three real questions against the fleet: a personal nutrition
    /// question (redacted here; the score is what matters) scored 0.032 with both
    /// scorers, "é melhor postar video no
    /// instagram ou youtube" scored 0.033 with both, and "quem é você?" scored 0.016
    /// with the text scorer alone and returned marketing psychology. The number that
    /// separates the two right answers from the wrong one is not the score, it is
    /// how many scorers voted.
    ///
    /// This is a warning rather than a filter, deliberately. A file whose map entry
    /// does not happen to use the question's words but whose text does is a real hit,
    /// so dropping single scorer results would lose answers. Saying "nobody agreed"
    /// costs nothing and loses nothing.
    pub fn no_agreement(&self, found: &[Retrieved]) -> bool {
        !found.is_empty() && found.iter().all(|f| f.why.len() < 2)
    }

    /// How much the router trusts its own top result, as evidence rather than a
    /// verdict handed down without one.
    ///
    /// **Why this exists at all.** `route` always produces a rank 1. A question the
    /// base has never heard of produces a rank 1. Before this, the two were
    /// indistinguishable to every caller, so the honest sentence "I do not have that
    /// book" could only be said when *nothing at all* matched, which is the rarest
    /// case. That is the failure ADR-0013 left open when it made the router the first
    /// door: a door that always opens onto something is not a door.
    ///
    /// **Three free signals, and none of them is sufficient alone.**
    ///
    /// - *Agreement.* Measured as the strongest of the three: the two questions that
    ///   routed correctly on 2026-08-17 had both scorers voting, the one that returned
    ///   marketing psychology had one. It is also the narrowest, because a file whose
    ///   map entry misses the question's words but whose text carries them is a real
    ///   hit with one vote.
    /// - *The keyword floor.* The one wrong answer scored 3.82 and every right answer
    ///   scored 9.55 or higher. It catches what agreement misses: two scorers can
    ///   agree enthusiastically on the wrong file when the question shares a common
    ///   word with it.
    /// - *The margin over the runner-up.* Scale free, which the floor is not. Keyword
    ///   sums are IDF weighted totals, so a question made of rare terms scores every
    ///   file higher than a question made of common ones, and a fixed floor therefore
    ///   means different things to different questions.
    ///
    /// So the floor and the margin are checked together, and either agreement or a
    /// clean margin can carry a result over the line. Both constants are stated in
    /// the open and both are calibrated on a sample of one wrong answer, which is why
    /// `kb eval` ships in the same change: **a threshold with no instrument beside it
    /// is a number somebody made up.**
    /// One question, one expansion, and every answer about it computed together.
    ///
    /// **The split is the point, and it is measured rather than assumed** (19 question
    /// set, 2026-08-18, release binary):
    ///
    /// | | keyword alone | fused |
    /// |---|---|---|
    /// | right file first | 18/19 | 11/19 |
    /// | right agent | 16/19 | 13/19 |
    ///
    /// So the two scorers are not two attempts at one job. RRF rewards *agreement*,
    /// which is what you want when assembling passages a person will read, because a
    /// file both scorers noticed is worth putting in front of them. It is the wrong
    /// rule for picking a single winner: a file each scorer ranks fourth beats a file
    /// one scorer ranks first, and top-1 precision falls by half.
    ///
    /// Hence: **the owner and the verdict come from intent, the reading comes from
    /// agreement.** Each scorer does the job it was measured to be better at, in one
    /// call, so no caller can pair them differently. Before this, `confidence` read
    /// the keyword score of *fusion's* pick, which is neither number and was nobody's
    /// intention.
    pub fn ask(&self, question: &str, top: usize) -> Answer {
        let terms = index::expand_query(question, &self.aliases);
        let keyword = index::route(
            question,
            &self.entries,
            &self.aliases,
            top * retrieve::KEYWORD_OVERSAMPLE,
        );
        let keyword: Vec<index::Hit> =
            keyword.into_iter().filter(|h| !h.entry.rel.is_empty()).collect();

        let text = self.search_all(&terms, top * retrieve::TEXT_OVERSAMPLE);
        let found = retrieve::fuse(&keyword, &text, top);

        Answer {
            confidence: self.confidence_of(&keyword, &text),
            agent: self.choose_agent_by_keyword(&keyword),
            keyword_top: keyword.first().map(|h| format!("{}/{}", h.entry.base, h.entry.rel)),
            found,
        }
    }

    /// The gate, over the keyword scorer's own ranking.
    ///
    /// Kept separate from [`Memory::ask`] so the eval can drive it against a list it
    /// chose, which is how the fused-versus-keyword table above was produced.
    pub fn confidence_of(
        &self,
        hits: &[index::Hit<'_>],
        text: &[crate::store::Hit],
    ) -> Confidence {
        let floor = self.floor();
        let Some(top) = hits.first() else {
            return Confidence {
                verdict: Verdict::Nothing,
                agreement: 0,
                keyword_score: 0.0,
                margin: 0.0,
                floor,
            };
        };
        let runner_up = hits.get(1).map(|h| h.score).unwrap_or(0.0);
        let margin = if runner_up > 0.0 { top.score / runner_up } else { 1.0 };

        // **Agreement is observed here, and used to be the literal 1.**
        //
        // The old comment said agreement was not observable from the keyword list
        // alone, which was true of the argument and false of the caller. `ask` runs
        // both scorers before it gets here, so the evidence was one parameter away
        // the whole time. The constant was not a lie in this function; it became one
        // in `classify::dossier`, which renders 1 as "Only one of the two independent
        // scorers ranked that file" and follows it with "one scorer alone is the case
        // this system reports as a guess rather than an answer". That sentence went to
        // the classifier on every message, including perfect hits, and it argued
        // against exactly the coverage judgement ADR-0027 leans on. A field meaning
        // "not observed" was being read as "observed, and bad".
        //
        // What it means, stated narrowly so nobody reads more into it: the text
        // scorer also surfaced the file the keyword scorer put first. `search_all`
        // merges each agent's chunks round robin by rank, so a file that is its own
        // agent's best match is admitted without competing against other agents. That
        // makes a 2 cheaper than the phrase "both scorers agreed" suggests. It is
        // reported and it does not gate, for that reason and for the measured one
        // below.
        let agreement = match hits.first() {
            Some(top) => {
                let seen_by_text = text
                    .iter()
                    .any(|h| h.base == top.entry.base && h.path == top.entry.rel);
                if seen_by_text { 2 } else { 1 }
            }
            None => 0,
        };

        // The floor alone, scaled to this corpus. See MIN_MARGIN for the measurement
        // that removed the margin from this line and why the reasoning behind it was
        // wrong, and `floor_for` for why the floor is no longer one number.
        let verdict = if self.clears_floor(top.score) {
            Verdict::Hit
        } else {
            Verdict::Guess
        };

        Confidence { verdict, agreement, keyword_score: top.score, margin, floor }
    }

    /// The floor for this fleet's size. One place, so the gate and every surface that
    /// prints the gate's threshold read the same number.
    pub fn floor(&self) -> f32 {
        floor_for(self.entry_count())
    }

    /// The state behind a refusal, gathered once for whichever surface is about to print
    /// it. See [`Shortfall`] for why the floor is taken from the `Confidence` and not
    /// from [`Memory::floor`] here.
    pub fn shortfall(&self, c: &Confidence) -> Shortfall {
        Shortfall {
            entries: self.entry_count(),
            agents: self.agents.len(),
            floor: c.floor,
            scored: c.keyword_score,
            unreachable: self.unreachable().len(),
        }
    }

    /// Whether the fleet is large enough for a `hit` to mean anything. See
    /// `MIN_ENTRIES_TO_ROUTE`.
    pub fn enough_to_route(&self) -> bool {
        self.entry_count() >= MIN_ENTRIES_TO_ROUTE
    }

    /// The gate itself: whether a keyword score is one this fleet will answer from.
    ///
    /// **One predicate, because the expression was already written twice.**
    /// [`Memory::confidence_of`] gates the keyword ranking and [`Memory::confidence`]
    /// gates a fused list, and both spelled out `score >= floor && enough_to_route()`
    /// by hand with a comment between them saying they must not drift. A third caller
    /// now needs the same cut: [`Memory::near_misses`] splits a ranking into what the
    /// gate would serve and what it would refuse, and a surface that re-derived the
    /// gate would be a fourth opinion about the one decision this type exists to hold.
    ///
    /// **Both halves, and the second is the one an extraction drops.**
    /// `enough_to_route` is what stops a base of one entry calling any shared word a
    /// hit: with one entry every word has `df = 1`, so idf can tell nothing apart and
    /// the floor built from it is cleared by evidence that is not evidence. Keeping
    /// only the comparison would turn every tiny fleet's guess into an answer, on every
    /// surface at once.
    pub fn clears_floor(&self, keyword_score: f32) -> bool {
        keyword_score >= self.floor() && self.enough_to_route()
    }

    /// The words a file declares it can be found by, joined on the pair fusion keys on.
    ///
    /// `retrieve::fuse` accumulates keyword hits under `(entry.base, entry.rel)` and text
    /// hits under `(hit.base, hit.path)`, so both halves land in one namespace and this
    /// lookup can use it for either. That join is the thing that silently returns nothing
    /// if either side changes shape, which is why a test pins it rather than trusting it.
    ///
    /// An empty slice is an answer and not a failure: the index holds no entry for that
    /// file, which is exactly what a file with no `Search for:` line looks like from here.
    /// The caller prints it rather than skipping it.
    fn keys_of(&self, base: &str, rel: &str) -> &[String] {
        self.entries
            .iter()
            .find(|e| e.base == base && e.rel == rel)
            .map(|e| e.keywords.as_slice())
            .unwrap_or(&[])
    }

    /// The candidates this question reached and the gate refused, with their keys.
    ///
    /// **What "just under the floor" has to mean here, because the literal reading is
    /// empty by construction.** [`Memory::recall_loss`] records only on
    /// `Verdict::Nothing`, and [`Memory::confidence_of`] returns `Nothing` only when the
    /// keyword list is empty. A sub-floor *keyword* score is a `guess`, which is served,
    /// and a served question is deliberately not a recall loss. So every question in the
    /// log has a keyword ranking of nothing at all, and the candidates worth naming are
    /// the ones the **text** scorer reached whose keys missed. That is F-02 itself, the
    /// case the log was built for: the base holds the answer and only its keys are wrong.
    ///
    /// Built on [`Memory::retrieve`] rather than [`Memory::ask`] on purpose. `ask` also
    /// folds an agent choice and computes a verdict, neither of which this needs, and a
    /// reader must not re-run the gate over a question the log already says was refused.
    /// `retrieve` records nothing and applies no gate, so asking it costs the corpus once
    /// and decides nothing a second time.
    ///
    /// Cost, stated rather than claimed away: this runs the full retrieval once per
    /// question it is asked about, so a caller looping over a long log pays the corpus
    /// once per line. `top` bounds the candidates carried per question, not the number of
    /// questions.
    pub fn near_misses(&self, question: &str, top: usize) -> Vec<NearMiss> {
        self.retrieve(question, top)
            .into_iter()
            .filter(|f| !self.clears_floor(f.keyword_score))
            .map(|f| NearMiss {
                keys: self.keys_of(&f.base, &f.path).to_vec(),
                base: f.base,
                rel: f.path,
                title: f.title,
                keyword_score: f.keyword_score,
                why: f.why,
            })
            .collect()
    }

    /// The older gate, over a fused list.
    ///
    /// Retained because `no_agreement` and this share the agreement signal and the
    /// tray still reads a fused list directly. Superseded for routing decisions by
    /// [`Memory::confidence_of`], for the reason measured in [`Memory::ask`].
    pub fn confidence(&self, found: &[Retrieved]) -> Confidence {
        let floor = self.floor();
        let Some(top) = found.first() else {
            return Confidence {
                verdict: Verdict::Nothing,
                agreement: 0,
                keyword_score: 0.0,
                margin: 0.0,
                floor,
            };
        };

        // The runner-up on the keyword side, not on the fused ranking. The question
        // being asked is "did the keyword scorer actually distinguish anything", and
        // a runner-up the keyword scorer never saw answers a different question.
        let runner_up = found
            .iter()
            .skip(1)
            .map(|f| f.keyword_score)
            .fold(0.0f32, f32::max);

        // A top result standing alone is not thereby confident, so an absent runner-up
        // contributes no margin rather than infinite margin. Getting this backwards
        // would make the single worst case, one weak file matching one common word,
        // look like the strongest possible result.
        let margin = if runner_up > 0.0 { top.keyword_score / runner_up } else { 1.0 };
        let agreement = top.why.len();

        // The floor alone, the same rule as `confidence_of`, and the two must not
        // drift: a caller getting a different verdict depending on which list it
        // happened to hold is the bug this whole type exists to prevent.
        //
        // Agreement is reported and does not gate, which the measured case forces.
        // "quem e voce?" was ranked by BOTH scorers and was still wrong, at 3.82.
        // Agreement is real evidence that a file is on topic and no evidence at all
        // that the topic is one the base covers, so it cannot carry a result over the
        // line on its own. `no_agreement` still exists and still says its own thing.
        let verdict = if self.clears_floor(top.keyword_score) {
            Verdict::Hit
        } else {
            Verdict::Guess
        };

        Confidence { verdict, agreement, keyword_score: top.keyword_score, margin, floor }
    }

    /// Which agent a question belongs to, aggregated from the file level routing.
    ///
    /// **No new mechanism, which is the whole argument for it.** Routing already scores
    /// every file in every base and already knows which base each file came from, so
    /// the per agent total is a fold over a list we are already computing. Agent
    /// selection therefore inherits whatever the file level router is measured to be
    /// worth, costs one more pass over at most `top` elements, and adds nothing that
    /// can be wrong on its own.
    ///
    /// Summed rather than max, because an agent whose base answers a question from
    /// three angles is more likely to be the right agent than one with a single
    /// strong file. That is the opposite of the rule inside `fuse`, where a file
    /// contributes once however many chunks match, and the difference is deliberate:
    /// there the competing files are alternatives and only one gets read, here the
    /// competing agents are owners and breadth of coverage is the actual evidence of
    /// ownership.
    pub fn choose_agent(&self, found: &[Retrieved]) -> Option<AgentChoice> {
        // **Filters routable bases, so the comparison against the keyword fold is
        // like for like.** It did not, and the difference was being read as a verdict on
        // fusion when it was an artefact of eligibility: an attached base can be the answer
        // to a question and can never be the agent who answers, so a fold that names
        // `decisions` scores as a miss by definition. Measured on the gold set, that
        // happened on 14 of 29 questions.
        //
        // The two folds now differ in exactly one thing, which is the list they fold, and
        // that is the thing ADR-0018 was about.
        let routable: Vec<&str> = self
            .agents
            .iter()
            .filter(|a| a.routable)
            .map(|a| a.name.as_str())
            .collect();

        // An empty roster does not filter, which is the same convention `eval::Row`
        // already uses and is safe here for a reason rather than for convenience: a
        // `Memory` with no agents opened no bases, so `found` is empty too and the tally
        // returns `None` regardless. The branch is unreachable outside a unit test that
        // is exercising the tally arithmetic and nothing else.
        tally(found.iter().filter(|f| {
            routable.is_empty() || routable.iter().any(|r| r.eq_ignore_ascii_case(&f.base))
        }).map(|f| (f.base.as_str(), f.score)))
    }

    /// The same fold over the keyword scorer's own ranking, skipping fusion.
    ///
    /// **Measured 2026-08-18 and it is not a variant, it is the better one:** the
    /// keyword scorer picks the right file 18 times in 19 where the fused list picks
    /// it 11 times, so an agent choice folded from the fused list is aggregating a
    /// weaker signal for no reason. Fusion exists to assemble *passages a reader
    /// wants*, where the text scorer's recall is the point. Choosing an owner is a
    /// different question and the evidence says it should be asked of the scorer that
    /// carries intent, which is the hand written map.
    ///
    /// Keyword scores are comparable across agents because `index::route` builds one
    /// document frequency table over every entry in the fleet, so a rare word is rare
    /// fleet wide rather than per base. Summing them across bases would be meaningless
    /// otherwise, and that is the property that makes this fold legitimate rather than
    /// merely convenient.
    pub fn choose_agent_by_keyword(&self, hits: &[index::Hit<'_>]) -> Option<AgentChoice> {
        let routable: Vec<&str> = self
            .agents
            .iter()
            .filter(|a| a.routable)
            .map(|a| a.name.as_str())
            .collect();

        tally(species_bests(
            hits.iter()
                .filter(|h| routable.iter().any(|r| r.eq_ignore_ascii_case(&h.entry.base)))
                .map(|h| (h.entry.base.as_str(), h.entry.rel.as_str(), h.score as f64)),
        ))
    }

    /// **Corpus share normalisation was built, measured and removed on 2026-08-18.**
    ///
    /// The proposal: agent choice sums scores per base, so a base with twice the entries
    /// collects twice the votes for reasons unrelated to the question. Steve holds 59 of
    /// 139 tracked files and won a question about agent memory architecture on one file
    /// about connecting Claude to Meta Ads. Dividing each hit by the base's share of the
    /// map should turn the sum into a density.
    ///
    /// Two measurements killed it. `kb eval` scored **12/13 either way**, so it bought
    /// nothing on the gold set. And replaying the real conversation showed the mechanism
    /// is backwards for what it was aimed at: dividing by share means a **small** base is
    /// divided by a small number, so normalisation *boosts* small bases rather than
    /// levelling them. It swapped one volume bias for the mirror image.
    ///
    /// What actually fixed the Steve misroute was excluding non-agent bases, measured
    /// separately and kept. The volume concern is real and still open; the answer is not
    /// this. Left as a comment rather than dead code so the next person meets the
    /// measurement instead of rediscovering the idea.


    /// The classifier declared in the fleet manifest, if any.
    ///
    /// Read from the opened roots rather than stored at open time, because a fleet root
    /// is a path the caller gave and re-reading one small file costs nothing next to
    /// being wrong about which fleet is being served.
    /// Bases the open had to leave out, so a surface can name them.
    pub fn skipped_bases(&self) -> &[PathBuf] {
        &self.skipped
    }

    pub fn classifier(&self) -> crate::classify::Classifier {
        for root in &self.opened {
            if let (_, _, Some(cmd)) = manifest_full(&root.join(MANIFEST)) {
                return crate::classify::Classifier::Command(cmd);
            }
        }
        crate::classify::Classifier::None
    }

    /// The model that reads a deposit and proposes notes. See `promote.rs`.
    pub fn promoter(&self) -> crate::classify::Classifier {
        self.manifest_command("promoter")
    }

    /// The model that writes prose from what retrieval found, for `kb answer`.
    ///
    /// After the verdict, never inside retrieval: ADR-0018's line is untouched. Absent
    /// the key, `kb answer` degrades to the reading list `kb route` prints.
    pub fn answerer(&self) -> crate::classify::Classifier {
        self.manifest_command("answerer")
    }

    /// The model that decides, three times, on a proposal it did not write.
    ///
    /// Configured separately from the promoter on purpose: this one is meant to be the
    /// stronger reader, and a single key would make them the same model, which is the
    /// arrangement `promote.rs` exists to avoid.
    pub fn reviewer(&self) -> crate::classify::Classifier {
        self.manifest_command("reviewer")
    }

    fn manifest_command(&self, key: &str) -> crate::classify::Classifier {
        for root in &self.opened {
            if let Some(cmd) = manifest_key(&root.join(MANIFEST), key) {
                return crate::classify::Classifier::Command(cmd);
            }
        }
        crate::classify::Classifier::None
    }

    /// The names an owner may have: the bases that declared themselves agents.
    pub fn roster(&self) -> Vec<String> {
        self.agents
            .iter()
            .filter(|a| a.routable)
            .map(|a| a.name.to_lowercase())
            .collect()
    }

    /// True when the full text index has no chunks for anything the keywords ranked,
    /// which almost always means the index is stale rather than the base being thin.
    /// Worth saying out loud: the alternative is a caller concluding the base is empty.
    pub fn looks_stale(&self, found: &[Retrieved]) -> bool {
        !found.is_empty() && found.iter().all(|f| f.passages.is_empty())
    }
}

/// The directory a fleet keeps its agents in, per ADR-0011.
///
/// Everything inside it is an agent, which is why no configuration says so: a
/// convention cannot be wrong about itself, and a list can.
const AGENTS_DIR: &str = "fleet";

/// The manifest, for what convention cannot express.
const MANIFEST: &str = "fleet.txt";

/// The best file of each species per agent, which is what the fold counts. ADR-0031.
///
/// **This is the second answer to the volume problem the comment above records as open.**
/// Summing per base let an agent win by mass: Steve holds a dense memory and once took a
/// question about agent architecture on one file about connecting Claude to Meta Ads,
/// because fifty nine files each contribute and fifty nine small numbers beat one large
/// one. The first answer, corpus share normalisation, was measured dead on 2026-08-18.
///
/// This one changes what counts as evidence instead of rescaling it: within a species only
/// the best file speaks, so mass is worth exactly its best member, and across species the
/// bests sum, so an agent whose memory AND tools both score beats one that only remembers.
/// The two acceptance cases, from ADR-0031 and pinned by the tests below: dense memory
/// must not outrank a purpose-built agent whose skills carry the subject, and an agent
/// with memory plus tools on the subject must beat a deeper memory standing alone.
///
/// Rejected shapes, so they are met here rather than rediscovered: sum per species is the
/// defect itself one level down; max overall throws away the breadth that makes the ads
/// case right; corpus share is measured above.
fn species_bests<'a>(
    weighted: impl Iterator<Item = (&'a str, &'a str, f64)>,
) -> Vec<(String, f64)> {
    let mut bests: Vec<(String, crate::index::Kind, f64)> = Vec::new();
    for (base, rel, score) in weighted {
        let kind = crate::index::kind_of(rel);
        match bests.iter_mut().find(|(b, k, _)| b == base && *k == kind) {
            Some(slot) => {
                if score > slot.2 {
                    slot.2 = score;
                }
            }
            None => bests.push((base.to_string(), kind, score)),
        }
    }
    bests.into_iter().map(|(b, _, s)| (b, s)).collect()
}

/// Sums a weight per base and reports the winner with its margin.
///
/// One function for both scorers so the two agent choices cannot drift apart in the
/// way the two query expansions once did: whatever aggregation rule is right, both
/// callers get the same one, and changing it is one edit rather than two that must
/// be kept in step by memory.
fn tally<S: AsRef<str>>(
    weighted: impl IntoIterator<Item = (S, f64)>,
) -> Option<AgentChoice> {
    let mut totals: Vec<(String, f64, usize)> = Vec::new();
    for (base, weight) in weighted {
        let base = base.as_ref();
        match totals.iter_mut().find(|(name, _, _)| name == base) {
            Some(slot) => {
                slot.1 += weight;
                slot.2 += 1;
            }
            None => totals.push((base.to_string(), weight, 1)),
        }
    }
    if totals.is_empty() {
        return None;
    }
    totals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (agent, score, files) = totals[0].clone();
    let runner_up = totals.get(1).map(|t| t.1).unwrap_or(0.0);
    Some(AgentChoice {
        agent,
        score,
        files,
        margin: if runner_up > 0.0 { score / runner_up } else { f64::INFINITY },
        contenders: totals.len(),
        totals: totals.iter().map(|(n, w, _)| (n.clone(), *w)).collect(),
    })
}

/// Expands fleet roots into the bases under them, leaving real bases alone.
///
/// Three shapes are recognised, in this order:
///
/// 1. **A base.** It holds a map file. Returned untouched, even if it contains other
///    bases, because expanding it would silently drop the parent's own notes.
/// 2. **A fleet root**, ADR-0011's layout: it holds a `fleet/` directory. Every
///    directory in there is an agent, plus anything the manifest attaches, minus
///    anything it disables.
/// 3. **A loose directory of bases**, whose immediate children hold maps. Kept
///    because a directory of bases is a reasonable thing to point at and refusing it
///    would buy nothing.
///
/// Anything else passes through so `Base::discover` reports it, where the error can
/// name the path and say what was wrong with it.
///
/// **This is public because `check` and `index` need the same answer as `retrieve`.**
/// It was private, and the CLI consequently treated a whole fleet as a single base:
/// 18 errors and 276 warnings from three bases that are individually clean. Two
/// notions of what a path means is one more than a program can afford.
pub fn expand_roots(paths: &[&Path]) -> Vec<PathBuf> {
    let mut out = Vec::new();

    for path in paths {
        if looks_like_a_base(path) {
            out.push(path.to_path_buf());
            continue;
        }

        let agents_dir = path.join(AGENTS_DIR);
        let mut found = if agents_dir.is_dir() {
            agents_in(&agents_dir)
        } else {
            bases_in(path)
        };

        if found.is_empty() {
            out.push(path.to_path_buf());
            continue;
        }

        let (attach, disable) = manifest(&path.join(MANIFEST));
        found.retain(|p| {
            p.file_name()
                .map(|n| !disable.iter().any(|d| d == &n.to_string_lossy()))
                .unwrap_or(true)
        });
        for rel in attach {
            // Relative to the fleet root, because ADR-0011 forbids absolute paths
            // inside the fleet: that rule is what makes moving it a directory move.
            let joined = path.join(&rel);
            found.push(if joined.exists() { joined } else { PathBuf::from(rel) });
        }

        out.append(&mut found);
    }

    out
}

/// The directory name, which is the agent's name by ADR-0011's convention.
fn name_of(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string())
}

/// Evidence that a directory is a base rather than a folder inside one.
///
/// **Used only where evidence is actually needed**, which is when somebody points at a
/// directory and the program has to guess what they meant. Inside the fleet root no
/// evidence is required, because being there is the evidence: see [`agents_in`].
///
/// A map still counts, because [[0028-a-note-carries-its-own-keys]] demoted the map without
/// deleting it and every base has one today. `agent.txt` counts too, so a new agent written
/// by hand and never given a map is still recognised when pointed at directly.
fn looks_like_a_base(dir: &Path) -> bool {
    crate::base::has_map(dir) || dir.join("agent.txt").is_file()
}

/// Every immediate subdirectory of the fleet root, which is what `fleet/` means.
///
/// **No content test, and that is the change ADR-0028 needed.** This used to require a map
/// file, so `has_map` was what made a directory an agent, and removing maps would have
/// undiscovered the whole fleet. Worse, a marker file cannot replace it: checked against the
/// six live bases, `fleet/person` (then `fleet/profile`) has no `agent.txt`, no `knowledge/` directory and no
/// `attach` line, so every marker proposed for it was wrong. It is a base because it sits in
/// `fleet/`, and nothing else needs to be true.
///
/// Dotted directories are skipped because `fleet/` holds `.git` and `.githooks`, and a
/// directory that should not be a base for any other reason is named in the manifest's
/// `disable` list, which is an opt-out that already exists and is already read.
fn agents_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && !p
                        .file_name()
                        .map(|n| n.to_string_lossy().starts_with('.'))
                        .unwrap_or(true)
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    found.sort();
    found
}

/// Immediate subdirectories that are bases, sorted so the order a fleet opens in
/// does not depend on the order the filesystem happens to hand back.
///
/// This is the loose-directory case, where evidence is required: the children of an
/// arbitrary directory are not bases by virtue of being there, and without the test
/// `kb check` inside one agent would report its own `knowledge/` and `records/` folders as
/// separate bases.
fn bases_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir() && looks_like_a_base(p))
            .collect(),
        Err(_) => Vec::new(),
    };
    found.sort();
    found
}

/// Reads `attach` and `disable` from the manifest. Same shape as `kb-aliases.txt`,
/// and for the same reason: the person editing it just watched something go wrong,
/// and a format they have to look up is a format they will not use.
fn manifest(path: &Path) -> (Vec<String>, Vec<String>) {
    let (attach, disable, _) = manifest_full(path);
    (attach, disable)
}

/// The manifest, including the classifier line.
///
/// Separate from `manifest` because `expand_roots` runs before a `Memory` exists and
/// only needs the first two, while the classifier is read once the fleet root is known.
/// One `key = value` out of the manifest, for keys that carry a command.
///
/// Separate from `manifest_full` rather than a fourth and fifth element of its tuple: that
/// tuple already has three unnamed slots and a caller reading `(_, _, Some(cmd))` is one
/// reorder away from a silent bug.
fn manifest_key(path: &Path, want: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() == want && !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn manifest_full(path: &Path) -> (Vec<String>, Vec<String>, Option<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (Vec::new(), Vec::new(), None),
    };

    let mut attach = Vec::new();
    let mut disable = Vec::new();
    let mut classifier = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "attach" => attach.push(value),
            "disable" => disable.push(value),
            "classifier" => classifier = Some(value),
            _ => {}
        }
    }
    (attach, disable, classifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0031's first acceptance case: dense memory must not outvote purpose.
    ///
    /// "pesquisar noticias sobre tech": nobody remembers anything about it, Steve's files
    /// share incidental words, and the purpose-built agent's skills carry the subject.
    /// Under the old sum, steve wins 6+5+4+3 = 18 against 12; under best-per-species his
    /// four memory files are worth their best one.
    #[test]
    fn mass_of_memory_is_worth_its_best_file() {
        let hits = vec![
            ("steve", "knowledge/research/a.md", 6.0),
            ("steve", "knowledge/research/b.md", 5.0),
            ("steve", "knowledge/transcripts/c.md", 4.0),
            ("steve", "knowledge/transcripts/d.md", 3.0),
            ("techie", "skills/tech-news-research.md", 12.0),
        ];
        let choice = tally(species_bests(hits.into_iter())).expect("someone scored");
        assert_eq!(choice.agent, "techie", "purpose beats mass");
        assert_eq!(choice.score, 12.0);
        let steve = choice.totals.iter().find(|(n, _)| n == "steve").unwrap();
        assert_eq!(steve.1, 6.0, "many files are worth their best one");
    }

    /// The second acceptance case: breadth across species is real evidence.
    ///
    /// "buscar algo na biblioteca de anuncios": Steve's memory AND his tools declaration
    /// score, and both pointing the same way is what being prepared looks like. A deeper
    /// memory standing alone loses.
    #[test]
    fn memory_and_tools_together_beat_a_deeper_memory_alone() {
        let hits = vec![
            ("steve", "knowledge/research/meta-hardening.md", 8.0),
            ("steve", "tools/meta-ads-mcp.md", 5.0),
            ("hoarder", "knowledge/deep/ads-lore.md", 12.0),
        ];
        let choice = tally(species_bests(hits.into_iter())).expect("someone scored");
        assert_eq!(choice.agent, "steve", "two species agreeing beat one deeper");
        assert_eq!(choice.score, 13.0);
        assert_eq!(choice.files, 2, "files counts species with evidence now");
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-memory-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// **The rule this pins was published wrong once, so it is pinned by running it.**
    ///
    /// ADR-0028 first proposed that a directory is a base when it holds `agent.txt`, or is
    /// named in an `attach` line, or contains `knowledge/`. Checked against the six live
    /// bases afterwards, `fleet/person` (named `fleet/profile` at the time) has none of the
    /// three: it is not an agent, it holds
    /// four files at its root, and it is not attached. The published predicate would have
    /// dropped the user's own profile out of the fleet, silently, because `person/core.md`
    /// is resident in every agent and its disappearance reads as the person going quiet
    /// rather than as an error.
    ///
    /// So the rule is structural: inside the fleet root, being there is the whole evidence.
    #[test]
    fn a_directory_in_the_fleet_root_is_a_base_with_no_marker_of_any_kind() {
        let root = scratch("predicate");
        let fleet = root.join("fleet");

        // one with a map, the shape every base has today
        std::fs::create_dir_all(fleet.join("mapped")).expect("mkdir");
        std::fs::write(fleet.join("mapped").join("MAP.md"), "# MAP
").expect("map");

        // one with nothing at all: no map, no agent.txt, no knowledge/. This is the case
        // the change exists for and the one the first predicate got wrong.
        std::fs::create_dir_all(fleet.join("bare")).expect("mkdir");
        std::fs::write(fleet.join("bare").join("core.md"), "# core
").expect("note");

        // and one the filesystem contributes, which must not become a base
        std::fs::create_dir_all(fleet.join(".git")).expect("mkdir");

        let found = expand_roots(&[root.as_path()]);
        let names: Vec<String> = found.iter().map(|p| name_of(p)).collect();

        assert!(names.contains(&"mapped".to_string()), "a mapped base is still a base");
        assert!(
            names.contains(&"bare".to_string()),
            "a base with no marker of any kind must still be found: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.starts_with('.')),
            "a dotted directory is not a base: {names:?}"
        );
    }

    /// The other half: outside the fleet root, evidence is still required, or `kb check`
    /// run inside one agent reports that agent's own folders as separate bases.
    #[test]
    fn a_folder_inside_a_base_is_not_a_base() {
        let root = scratch("not-a-base");
        std::fs::create_dir_all(root.join("knowledge")).expect("mkdir");
        std::fs::create_dir_all(root.join("records")).expect("mkdir");
        std::fs::write(root.join("MAP.md"), "# MAP
").expect("map");

        let found = expand_roots(&[root.as_path()]);
        assert_eq!(found.len(), 1, "the base itself, not its folders: {found:?}");
        assert_eq!(found[0], root, "and it is returned untouched");
    }

    fn make_base(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("knowledge")).expect("mkdir");
        std::fs::write(dir.join("MAP.md"), "# MAP\n\n- **[[a]]** thing\n  Search for: `thing`\n")
            .expect("map");
        dir
    }

    /// A base holding one findable note, opened. Enough for the recall loss tests,
    /// which need `is_empty()` to be false and a root to write into. No index is
    /// synced because none of this reads chunks: the decision under test is made from
    /// the verdict, which is the entire point of it.
    fn one_note_base(name: &str) -> (PathBuf, Memory) {
        let root = scratch(name).join("probe");
        std::fs::create_dir_all(root.join("knowledge")).expect("mkdir");
        std::fs::write(root.join("MAP.md"), "# MAP\n").expect("map");
        std::fs::write(
            root.join("knowledge").join("zebra.md"),
            "# Zebra\n\n**Search for:** `zebra`, `quagga`\n\n**Exists to:** hold one animal\n",
        )
        .expect("note");
        let memory = Memory::open(&[root.as_path()], true).expect("opens");
        (root, memory)
    }

    fn saying(v: Verdict) -> Confidence {
        Confidence { verdict: v, agreement: 0, keyword_score: 0.0, margin: 0.0, floor: SCORE_FLOOR }
    }

    /// **The definition of a recall loss lives here and nowhere else.** F-02 in
    /// `reports/2026-08-29-first-integration.md`: the writing was already on the
    /// contract and the deciding was not, so six surfaces held four different opinions
    /// about which questions the log counts. Two tested list length on the keyword
    /// ranking, two on the fused one, `kb answer` suggested without ever recording, and
    /// the log that the two live revisit triggers are measured against counted a
    /// different population depending on which door the question came through.
    ///
    /// A refusal is the loss. A `guess` is not, today, and that is a decision rather
    /// than an oversight: it changes what the file counts, so it gets settled with a
    /// measurement and not in a commit message. This test is what makes changing it
    /// deliberate.
    #[test]
    fn only_a_refusal_is_a_recall_loss_and_the_decision_lives_on_the_contract() {
        let (root, memory) = one_note_base("recall-loss");
        let log = crate::misses::path_in(&root);

        assert!(memory.recall_loss("uma pergunta respondida", &saying(Verdict::Hit)).is_none());
        assert!(memory.recall_loss("uma pergunta chutada", &saying(Verdict::Guess)).is_none());
        assert!(!log.exists(), "an answer that was served is not a loss: {}", log.display());

        let loss = memory
            .recall_loss("uma pergunta sobre zebras", &saying(Verdict::Nothing))
            .expect("a refusal is a recall loss");
        assert_eq!(loss.looked_like, vec!["zebra".to_string()], "the vocabulary comes with it");
        assert_eq!(loss.question, "uma pergunta sobre zebras");
        assert_eq!(loss.log, log, "and it says where it went");
        assert!(loss.recorded(), "the base is writable here: {:?}", loss.error);

        let written = std::fs::read_to_string(&log).expect("the refusal was recorded");
        assert!(written.contains("uma pergunta sobre zebras"), "{written}");
        assert!(!written.contains("chutada"), "a guess was served, so it is not a loss: {written}");
    }

    /// **The property the seam exists for: a suggester cannot move a verdict.**
    ///
    /// The suggestion path runs strictly after the gate at every surface, and that
    /// ordering used to be a fact about the call graph that nothing stated. It is what
    /// makes a second implementation safe to add: whatever it returns is vocabulary
    /// offered to a reader who was already told the base does not cover the question,
    /// so a suggester that lies costs a wasted retry and can never turn a refusal into
    /// an answer the caller mistakes for a hit.
    ///
    /// The loud suggester answers every question with two words the trigram one would
    /// not have produced here, and the last assertion is what stops this test being
    /// vacuous: it proves the replacement actually ran, so the unchanged `Confidence`
    /// above is a measurement and not an artefact of nothing having happened.
    #[test]
    fn a_suggester_that_answers_everything_cannot_move_a_verdict() {
        struct AlwaysAnswers;
        impl crate::suggester::Suggester for AlwaysAnswers {
            fn words(&self, _question: &str, _entries: &[index::Entry], _limit: usize) -> Vec<String> {
                vec!["zebra".to_string(), "quagga".to_string()]
            }
        }

        let (_root, memory) = one_note_base("suggester-inert");
        let question = "uma pergunta sobre zebras";

        let before = memory.ask(question, 5).confidence;
        assert_eq!(before.verdict, Verdict::Nothing, "one entry never routes: the gate refuses");

        let memory = memory.with_suggester(Box::new(AlwaysAnswers));
        let after = memory.ask(question, 5).confidence;

        assert_eq!(after.verdict, before.verdict, "the verdict is the gate's, not the suggester's");
        assert_eq!(after.agreement, before.agreement, "agreement counts scorers, not suggestions");
        assert_eq!(after.keyword_score, before.keyword_score, "the keyword score is untouched");
        assert_eq!(after.margin, before.margin, "so is the runner-up margin");
        assert_eq!(after.floor, before.floor, "and the floor the verdict was measured against");

        let loss = memory.recall_loss(question, &after).expect("a refusal is a recall loss");
        assert_eq!(
            loss.looked_like,
            vec!["zebra".to_string(), "quagga".to_string()],
            "the installed suggester ran: trigrams answer this question with `zebra` alone"
        );
    }

    /// **The ordering, checked rather than read.**
    ///
    /// A suggester that panics the moment it is asked anything, installed on a memory
    /// big enough that the empty-base early return cannot be what saves it. If
    /// `recall_loss` ever asked for vocabulary before consulting the verdict, or asked
    /// on a verdict that was served, this is a panic instead of an assertion failure.
    ///
    /// `Verdict::Nothing` is deliberately not exercised here. `calibrated_memory` has
    /// no `opened` root, so `recall_loss` would resolve the log to a relative path and
    /// `misses::record` would write `kb-misses.txt` into the crate root. The refusal
    /// path is covered on `one_note_base` by the test above, which has somewhere to
    /// write.
    #[test]
    fn the_suggester_never_runs_on_a_verdict_that_was_served() {
        struct Explodes;
        impl crate::suggester::Suggester for Explodes {
            fn words(&self, _question: &str, _entries: &[index::Entry], _limit: usize) -> Vec<String> {
                panic!("the suggester ran on a verdict that was served")
            }
        }

        let memory = calibrated_memory().with_suggester(Box::new(Explodes));
        assert!(memory.recall_loss("uma pergunta respondida", &saying(Verdict::Hit)).is_none());
        assert!(memory.recall_loss("uma pergunta chutada", &saying(Verdict::Guess)).is_none());
    }

    /// **The loss comes back whether or not it could be stored, which is the whole
    /// point of F-03.** A hosted consumer has no writable path beside its base, so the
    /// only copy that will ever exist is the one it is handed. Reporting the failure
    /// on stderr and returning nothing is how a deployment ends up with a recall loss
    /// log holding two lines written months earlier on somebody's laptop.
    #[test]
    fn a_loss_that_could_not_be_written_is_still_returned_and_says_why() {
        let (root, memory) = one_note_base("recall-loss-readonly");
        // A directory where the log has to go, which no write can succeed against on
        // either platform. Simulating the read only filesystem itself is not portable.
        std::fs::create_dir_all(crate::misses::path_in(&root)).expect("mkdir");

        let loss = memory
            .recall_loss("uma pergunta sobre zebras", &saying(Verdict::Nothing))
            .expect("the loss happened even though the log did not");

        assert!(!loss.recorded(), "nothing could have been written");
        assert!(loss.error.is_some(), "and the caller is told why");
        assert_eq!(
            loss.looked_like,
            vec!["zebra".to_string()],
            "the vocabulary still reaches the caller: a failed write is not a failed query"
        );
    }

    /// **An error that names a symptom sends the reader to the wrong problem.** A
    /// deployment whose bundle left `.kb/` behind got
    /// `cannot open the index: Permission denied (os error 13)`, and permission is the
    /// true proximate cause and the misleading one: `Store::open` creates the index's
    /// parent directory before opening, so on a read only filesystem a missing index
    /// fails as a permission error. The integrator passes our text straight to the
    /// person on screen, which is what a good integrator does and what this should be
    /// written for. F-07 in `reports/2026-08-29-first-integration.md`.
    #[test]
    fn an_index_that_was_never_built_names_the_command_that_builds_it() {
        let base = scratch("index-missing").join("probe");
        std::fs::create_dir_all(&base).expect("mkdir");
        std::fs::write(base.join("agent.txt"), "name = Probe\n").expect("agent");
        // A file where `.kb/` has to go, so creating the index directory cannot
        // succeed. A read only mount produces the same failure and is not portable.
        std::fs::write(base.join(".kb"), "not a directory").expect("blocker");

        let said = match Memory::open(&[base.as_path()], true) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("the index cannot be opened"),
        };

        assert!(said.contains("kb index"), "it names the fix: {said}");
        assert!(
            said.to_lowercase().contains("not been built") || said.contains("nothing has been indexed"),
            "it names the cause before the symptom: {said}"
        );
        assert!(
            said.contains(&index_path(&base).display().to_string()),
            "and where it was looking: {said}"
        );
    }

    /// The other branch, which must not give that advice: an index that is there and
    /// will not open is not fixed by building one, and sending somebody to `kb index`
    /// for it is the same defect wearing the opposite sign.
    #[test]
    fn an_index_that_exists_and_will_not_open_reports_the_reason_and_no_wrong_advice() {
        let base = scratch("index-broken").join("probe");
        std::fs::create_dir_all(&base).expect("mkdir");
        std::fs::write(base.join("agent.txt"), "name = Probe\n").expect("agent");
        // A directory where the database file has to be: it exists, and no connection
        // can be opened to it.
        std::fs::create_dir_all(index_path(&base)).expect("blocker");

        let said = match Memory::open(&[base.as_path()], true) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("the index cannot be opened"),
        };

        assert!(!said.contains("kb index"), "building one is not the fix here: {said}");
        assert!(said.contains("cannot open the index"), "{said}");
    }

    /// A miss against a library nobody has filled in is a fact about the library. The
    /// log exists to say whether this design converges, and counting these would
    /// corrupt the one number it produces.
    #[test]
    fn an_empty_base_is_not_a_recall_loss_because_there_was_nothing_to_miss() {
        // No `Search for:` line anywhere, not even in a map: an entry is built from
        // that line and from nothing else since ADR-0028, so this base indexes to zero.
        let base = scratch("recall-loss-empty").join("hollow");
        std::fs::create_dir_all(base.join("knowledge")).expect("mkdir");
        std::fs::write(base.join("agent.txt"), "name = Hollow\n").expect("agent");
        std::fs::write(base.join("knowledge").join("draft.md"), "# Draft\n\nnothing yet.\n")
            .expect("note");

        let memory = Memory::open(&[base.as_path()], true).expect("opens");
        assert!(memory.is_empty(), "the fixture has no indexable note");

        assert!(memory.recall_loss("qualquer coisa", &saying(Verdict::Nothing)).is_none());
        assert!(!crate::misses::path_in(&base).exists(), "nothing to miss, nothing recorded");
    }

    /// Builds a `Retrieved` for the gate tests. Named fields on purpose: the gate
    /// reads three of them and a positional helper would let a future field land in
    /// the wrong slot silently.
    fn found(base: &str, fused: f64, keyword: f32, why: &[&str]) -> Retrieved {
        Retrieved {
            base: base.into(),
            path: format!("{base}/p.md"),
            layer: crate::retrieve::Layer::Long,
            title: String::new(),
            purpose: String::new(),
            score: fused,
            keyword_score: keyword,
            why: why.iter().map(|s| s.to_string()).collect(),
            matched: vec![],
            passages: vec![],
        }
    }

    fn empty_memory() -> Memory {
        Memory {
            entries: vec![], aliases: vec![],
            scope: Scope::Public, agents: vec![], opened: vec![], skipped: vec![],
            index_was_rebuilt: false,
            suggester: Box::new(crate::suggester::Trigram),
        }
    }

    /// A memory the size the floor was calibrated on, so `SCORE_FLOOR` means in these
    /// tests exactly what it meant when it was measured. The gate tests below used an
    /// empty memory, which was fine while the floor was one number; with the floor
    /// scaled to the corpus, an empty memory has a floor of zero and no ruler at all.
    fn calibrated_memory() -> Memory {
        let mut m = empty_memory();
        m.entries = (0..FLOOR_CALIBRATED_AT)
            .map(|i| index::Entry {
                base: "zed".into(),
                rel: format!("knowledge/{i}.md"),
                stem: i.to_string(),
                title: String::new(),
                keywords: vec![format!("k{i}")],
                summary: String::new(),
                body: String::new(),
            })
            .collect();
        m
    }

    /// Every entry carries one key the whole corpus carries, so a question made of it
    /// is a real match that no honest floor lets through: `idf` of a term in all 226
    /// entries is `ln(1 + 226/227)`, about 0.69, and 6 x 0.69 is 4.15 against a floor
    /// of 17.5. Each entry also carries a unique key, so the same fixture produces a
    /// result **above** the floor and the split can be tested in both directions.
    fn memory_with_one_key_everybody_shares() -> Memory {
        let mut m = empty_memory();
        m.entries = (0..FLOOR_CALIBRATED_AT)
            .map(|i| index::Entry {
                base: "zed".into(),
                rel: format!("knowledge/{i}.md"),
                stem: i.to_string(),
                title: String::new(),
                keywords: vec!["common".into(), format!("k{i}")],
                summary: String::new(),
                body: String::new(),
            })
            .collect();
        m
    }

    /// The gate is one predicate, and the size clause is half of it.
    ///
    /// Both existing gate sites wrote `score >= floor && enough_to_route()` by hand,
    /// with a comment between them saying they must not drift. `near_misses` needs the
    /// same cut to split a ranking, and a third copy is a third chance to disagree.
    ///
    /// The last assertion is the one a careless extraction drops. Without
    /// `enough_to_route` a fleet of one entry calls any shared word a hit, on every
    /// surface at once, because with one entry every word has `df = 1` and idf can tell
    /// nothing apart.
    #[test]
    fn one_floor_decides_the_gate_and_every_surface_that_splits_a_ranking() {
        let m = calibrated_memory();
        assert!(m.clears_floor(SCORE_FLOOR + 1.0), "above the floor on the fleet it was measured on");
        assert!(!m.clears_floor(SCORE_FLOOR - 1.0), "below it");

        // The same answer the gate itself gives, over the same numbers.
        for score in [SCORE_FLOOR - 1.0, SCORE_FLOOR + 1.0] {
            let verdict = m.confidence(&[found("zed", 0.9, score, &["keywords #1"])]).verdict;
            assert_eq!(
                m.clears_floor(score),
                verdict == Verdict::Hit,
                "the predicate and the gate must be one decision (at {score})"
            );
        }

        let mut tiny = empty_memory();
        tiny.entries = vec![index::Entry {
            base: "zed".into(),
            rel: "knowledge/only.md".into(),
            stem: "only".into(),
            title: String::new(),
            keywords: vec!["k".into()],
            summary: String::new(),
            body: String::new(),
        }];
        assert!(
            !tiny.clears_floor(1_000.0),
            "a base below MIN_ENTRIES_TO_ROUTE has no ruler, whatever the score"
        );
    }

    /// A near miss comes back with the keys the file carries, not just its path.
    ///
    /// The keys are the actionable half: they are what the reader compares against the
    /// question before writing an alias line or another `Search for:` term. Looking them
    /// up joins on the `(base, rel)` pair `retrieve::fuse` keys its map on, and that join
    /// returns nothing at all rather than failing loudly if either side changes shape,
    /// so it is pinned here.
    ///
    /// The second half is the split itself: an entry that clears the floor is served and
    /// is therefore not a near miss, whatever else it is.
    #[test]
    fn an_entry_that_ranked_below_the_floor_comes_back_with_the_keys_it_carries() {
        let m = memory_with_one_key_everybody_shares();

        let near = m.near_misses("common", 3);
        assert!(!near.is_empty(), "a shared key is a real match under the floor: {near:?}");
        let top = &near[0];
        assert_eq!(top.base, "zed", "{top:?}");
        assert!(top.rel.starts_with("knowledge/"), "{top:?}");
        assert!(
            top.keys.iter().any(|k| k == "common"),
            "the keys travelled with the path: {top:?}"
        );
        assert!(!m.clears_floor(top.keyword_score), "under the floor by construction: {top:?}");

        assert!(
            m.near_misses("k5", 3).is_empty(),
            "a unique key clears the floor, and what the gate serves is not a near miss"
        );
    }

    /// ADR-0036. The floor on the calibration fleet is the measured number, to the
    /// last decimal and by construction; smaller corpora get a lower one and larger a
    /// higher one, in the same fraction of one unique key.
    #[test]
    fn the_floor_is_the_measured_number_where_it_was_measured_and_scales_elsewhere() {
        assert!((floor_for(FLOOR_CALIBRATED_AT) - SCORE_FLOOR).abs() < 1e-4);
        assert!(floor_for(11) < SCORE_FLOOR, "a small base gets a lower floor");
        assert!(floor_for(1000) > SCORE_FLOOR, "a large base gets a higher one");

        // One unique key, at each size, against that size's floor: the fraction is the
        // invariant, so a single unique key clears the floor at every size or at none.
        for n in [4usize, 11, 226, 1000] {
            let one_key = index::W_KEYWORD * index::idf_unique(n);
            assert_eq!(
                one_key >= floor_for(n),
                index::W_KEYWORD * index::idf_unique(FLOOR_CALIBRATED_AT) >= SCORE_FLOOR,
                "the meaning of the floor in keys must not change with N (at N={n})"
            );
        }
    }

    /// A thousand entries, a word in fifty of them: under the fixed floor that scored
    /// 18.2 and was a hit on its own. It should not be, and now it is not.
    #[test]
    fn a_word_in_five_percent_of_a_big_corpus_is_not_a_hit_on_its_own() {
        let n = 1000usize;
        let df = 50usize;
        let score = index::W_KEYWORD * (1.0 + n as f32 / (1.0 + df as f32)).ln();
        assert!(score > SCORE_FLOOR, "the case that motivated this: {score} cleared 17.5");
        assert!(score < floor_for(n), "and does not clear the floor for its own corpus");
    }

    /// One entry is no ruler: every word in it has the same weight, so the best the
    /// gate can say is guess. Two entries and a unique key is a hit, at that size's
    /// floor.
    #[test]
    fn one_entry_never_routes_and_two_can() {
        let mut one = empty_memory();
        one.entries = vec![index::Entry {
            base: "zed".into(), rel: "knowledge/a.md".into(), stem: "a".into(),
            title: String::new(), keywords: vec!["deploy".into()], summary: String::new(),
            body: String::new(),
        }];
        let big = index::W_KEYWORD * index::idf_unique(1) * 4.0;
        let c = one.confidence(&[found("zed", 0.9, big, &["keywords #1"])]);
        assert_eq!(c.verdict, Verdict::Guess, "four keys on one note is still no ruler");

        let mut two = one;
        two.entries.push(index::Entry {
            base: "zed".into(), rel: "knowledge/b.md".into(), stem: "b".into(),
            title: String::new(), keywords: vec!["rollback".into()], summary: String::new(),
            body: String::new(),
        });
        let one_key = index::W_KEYWORD * index::idf_unique(2);
        let c = two.confidence(&[found("zed", 0.9, one_key, &["keywords #1"])]);
        assert_eq!(c.verdict, Verdict::Hit, "one unique key clears the floor at every size");
    }

    /// The verdict carries the floor it was measured against, so no surface prints the
    /// calibration constant as if it applied.
    #[test]
    fn the_verdict_carries_the_floor_that_applied() {
        let m = calibrated_memory();
        let c = m.confidence(&[found("zed", 0.9, SCORE_FLOOR + 1.0, &["keywords #1"])]);
        assert!((c.floor - SCORE_FLOOR).abs() < 1e-4);
        let e = empty_memory();
        assert_eq!(e.confidence(&[]).floor, 0.0, "no entries, no ruler, and it says so");
    }

    /// The three questions that produced this rule, as the shapes they had.
    #[test]
    fn agreement_between_the_scorers_is_what_separates_a_hit_from_a_guess() {
        let both = Retrieved {
            base: "yaron".into(), path: "p".into(), layer: crate::retrieve::Layer::Long, title: String::new(),
            purpose: String::new(), score: 0.032,
            keyword_score: 12.0,
            why: vec!["keywords #2".into(), "text #5".into()],
            matched: vec![], passages: vec![],
        };
        let one = Retrieved {
            base: "steve".into(), path: "q".into(), layer: crate::retrieve::Layer::Long, title: String::new(),
            purpose: String::new(), score: 0.016,
            keyword_score: 0.0,
            why: vec!["text #1".into()],
            matched: vec![], passages: vec![],
        };

        let m = empty_memory();

        assert!(m.no_agreement(&[one.clone()]), "one scorer alone is a guess");
        assert!(!m.no_agreement(&[both.clone()]), "two scorers agreeing is a hit");
        assert!(
            !m.no_agreement(&[one, both]),
            "one agreed result is enough; the warning is about nobody agreeing"
        );
        assert!(!m.no_agreement(&[]), "nothing found is a different message");
    }

    /// The measured case, in the shape it had: the wrong answer for "quem e voce?"
    /// scored 3.82 and the router presented it exactly like an answer.
    #[test]
    fn a_score_under_the_floor_is_a_guess_however_confident_the_ranking_looks() {
        let m = empty_memory();
        let c = m.confidence(&[found("steve", 0.9, 3.82, &["keywords #1", "text #1"])]);
        assert_eq!(c.verdict, Verdict::Guess, "both scorers agreed and it was still wrong");
        assert_eq!(c.agreement, 2, "agreement is reported, it is just not sufficient");
        assert!(c.note().is_some());
    }

    #[test]
    fn a_score_over_the_floor_with_agreement_is_a_hit() {
        let m = calibrated_memory();
        // 9.55 was the lowest correct answer when the floor was 6.0. Written against
        // the constant so it keeps meaning "over the floor" after the next re-derivation.
        let c = m.confidence(&[found("zed", 0.9, SCORE_FLOOR + 3.5, &["keywords #1", "text #2"])]);
        assert_eq!(c.verdict, Verdict::Hit);
        assert!(c.note().is_none(), "a hit says nothing extra");
    }

    /// The half that agreement alone cannot do. One scorer, but the keyword side
    /// separated the field decisively, which is a real hit and used to be reported as
    /// a guess by `no_agreement`.
    #[test]
    fn one_scorer_with_a_clean_margin_is_still_a_hit() {
        let m = calibrated_memory();
        let c = m.confidence(&[
            found("zed", 0.9, 30.0, &["keywords #1"]),
            found("zed", 0.4, 4.0, &["keywords #2"]),
        ]);
        assert_eq!(c.verdict, Verdict::Hit);
        assert!(c.margin >= MIN_MARGIN);
    }

    /// Two files the scorer could not choose between. This was asserted to be a
    /// guess before `kb eval` ran, on the reasoning recorded in `MIN_MARGIN`, and the
    /// measurement refuted it: correct answers routinely have margins at 1.0 to 1.2,
    /// so demoting this shape throws away twelve right answers in nineteen to catch
    /// nothing. The test is kept, inverted, so the refuted belief stays visible
    /// instead of being quietly deleted along with the code that held it.
    #[test]
    fn a_narrow_margin_no_longer_demotes_a_result_that_clears_the_floor() {
        let m = calibrated_memory();
        // Scores are written against SCORE_FLOOR rather than as literals. The floor was
        // re-derived on 2026-08-20 from 6.0 to 17.5 and these fixtures, at 11.0 and 10.8,
        // silently stopped testing what they were named for: they became two results under
        // the floor, and the test asserting a Hit failed for a reason nothing to do with
        // margins. A fixture pinned to a constant's old value is a fixture that expires.
        let c = m.confidence(&[
            found("zed", 0.9, SCORE_FLOOR + 5.0, &["keywords #1"]),
            found("steve", 0.8, SCORE_FLOOR + 4.8, &["keywords #2"]),
        ]);
        assert!(c.margin < MIN_MARGIN, "the margin is still measured and reported");
        assert_eq!(c.verdict, Verdict::Hit, "and it is no longer what decides");
    }

    /// Standing alone is not winning. A single weak file matching one common word is
    /// the worst case in the set, and treating an absent runner-up as infinite margin
    /// would rank it as the most confident result possible.
    #[test]
    fn a_lone_weak_result_does_not_get_infinite_margin() {
        let m = empty_memory();
        let c = m.confidence(&[found("steve", 0.9, 2.0, &["keywords #1"])]);
        assert_eq!(c.verdict, Verdict::Guess);
        assert_eq!(c.margin, 1.0, "no runner-up means no evidence of separation");
    }

    #[test]
    fn nothing_found_is_its_own_verdict_and_not_a_weak_hit() {
        let m = empty_memory();
        let c = m.confidence(&[]);
        assert_eq!(c.verdict, Verdict::Nothing);
        assert_eq!(c.keyword_score, 0.0);
    }

    /// Agent selection is a fold over the same list, so this is really a test that
    /// breadth counts: three mid files beat one strong file for *ownership*, which is
    /// the opposite of the rule inside `fuse` and deliberately so.
    #[test]
    fn the_agent_with_the_most_weight_across_its_files_wins() {
        let m = empty_memory();
        let c = m
            .choose_agent(&[
                found("zed", 0.016, 8.0, &["keywords #1"]),
                found("decisions", 0.015, 7.0, &["keywords #2"]),
                found("decisions", 0.014, 6.0, &["keywords #3"]),
                found("decisions", 0.013, 5.0, &["keywords #4"]),
            ])
            .expect("something ranked");
        assert_eq!(c.agent, "decisions");
        assert_eq!(c.files, 3);
        assert_eq!(c.contenders, 2);
        assert!(c.margin > 1.0);
    }

    /// One base answering and no other base having anything to say is the one case
    /// where an absent runner-up really is maximum confidence, unlike at file level.
    /// The incumbent rule is gone, and this is the test that replaced it: the words
    /// it was built for now score **nothing**, so the floor handles them and no
    /// second mechanism is needed. Guards the stopword list against a regression
    /// that would bring the whole argument back.
    #[test]
    fn a_message_with_no_content_scores_nothing_at_all() {
        let entries = vec![
            Entry {
                base: "yaron".into(), rel: "recipes/eating-out.md".into(),
                stem: "eating-out".into(), title: "Eating out".into(),
                keywords: vec!["comer fora".into(), "restaurante".into()],
                summary: String::new(), body: "comer fora restaurante".into(),
            },
        ];
        for empty in ["ok obrigado", "isso ai", "pode fazer", "continua", "yes lets do it"] {
            let hits = index::route(empty, &entries, &[], 5);
            assert!(
                hits.is_empty(),
                "{empty:?} carries no domain content and must not reach a file"
            );
        }
    }

    #[test]
    fn a_single_agent_scoring_alone_has_no_contender() {
        let m = empty_memory();
        let c = m.choose_agent(&[found("yaron", 0.03, 20.0, &["keywords #1"])]).expect("ranked");
        assert_eq!(c.agent, "yaron");
        assert_eq!(c.contenders, 1);
        assert!(c.margin.is_infinite());
    }

    #[test]
    fn no_results_means_no_agent_rather_than_a_default_one() {
        assert!(empty_memory().choose_agent(&[]).is_none());
    }

    #[test]
    fn a_base_path_is_passed_through_unchanged() {
        let root = scratch("plain");
        let base = make_base(&root, "zed");
        assert_eq!(expand_roots(&[&base]), vec![base]);
    }

    /// The layout Richard proposed: one parent holding the agents. Accepting it is
    /// what makes moving the folders optional rather than a migration.
    #[test]
    fn a_fleet_root_expands_into_the_bases_under_it() {
        let root = scratch("fleet");
        make_base(&root, "zed");
        make_base(&root, "steve");
        make_base(&root, "yaron");
        std::fs::create_dir_all(root.join("not-an-agent")).expect("mkdir");

        let found = expand_roots(&[&root]);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(names, vec!["steve", "yaron", "zed"], "sorted, not filesystem order");
        assert!(!names.contains(&"not-an-agent".to_string()), "a directory with no map is not a base");
    }

    /// ADR-0011's actual layout, which the first version of this function missed:
    /// it looked one level down and the ADR puts agents two levels down, under
    /// the agents directory. Written before the ADR existed and not revisited when the ADR
    /// landed, so the code and the decision disagreed until a real fleet was built.
    #[test]
    fn a_fleet_root_finds_the_agents_directory() {
        let root = scratch("agents-dir");
        let agents = root.join("fleet");
        std::fs::create_dir_all(&agents).expect("mkdir");
        make_base(&agents, "zed");
        make_base(&agents, "yaron");
        std::fs::create_dir_all(root.join("outbox")).expect("mkdir");

        let names: Vec<String> = expand_roots(&[&root])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["yaron", "zed"], "outbox is not an agent");
    }

    /// `disable` in the manifest, which is the only way to switch an agent off
    /// without deleting it.
    #[test]
    fn the_manifest_can_disable_an_agent() {
        let root = scratch("disable");
        let agents = root.join("fleet");
        std::fs::create_dir_all(&agents).expect("mkdir");
        make_base(&agents, "zed");
        make_base(&agents, "steve");
        std::fs::write(root.join("fleet.txt"), "# a comment\ndisable = steve\n").expect("write");

        let names: Vec<String> = expand_roots(&[&root])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["zed"]);
    }

    /// `attach` is how freedom survives structure: a base outside the fleet is not
    /// told to move, and the attachment is recorded in one file so something always
    /// knows where everything is.
    #[test]
    fn the_manifest_can_attach_a_base_from_outside() {
        let root = scratch("attach");
        let agents = root.join("fleet");
        std::fs::create_dir_all(&agents).expect("mkdir");
        make_base(&agents, "zed");
        make_base(&root, "outside");
        std::fs::write(root.join("fleet.txt"), "attach = outside\n").expect("write");

        let names: Vec<String> = expand_roots(&[&root])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["zed", "outside"]);
    }

    /// A base that happens to contain other bases is still a base. Expanding it would
    /// silently drop the parent's own notes.
    #[test]
    fn a_base_is_not_expanded_even_when_it_contains_bases() {
        let root = scratch("nested");
        let outer = make_base(&root, "outer");
        make_base(&outer, "inner");
        assert_eq!(expand_roots(&[&outer]), vec![outer]);
    }

    #[test]
    fn a_path_that_is_neither_is_passed_through_so_discover_can_explain() {
        let root = scratch("neither");
        let empty = root.join("empty");
        std::fs::create_dir_all(&empty).expect("mkdir");
        assert_eq!(expand_roots(&[&empty]), vec![empty]);
    }

    /// **The test this replaces asserted the opposite, and the opposite was the defect.**
    /// It pinned that a base outside git is skipped and nothing in it is served, under
    /// the rule that unknown is not public. ADR-0034: a memory layer that refuses a
    /// folder until somebody runs `git init` fails at the first interaction anybody has
    /// with it, and the rule protected nothing in daily use because every owner surface
    /// passes `--all`. A folder with a note in it is a base, and it serves the note.
    #[test]
    fn a_base_outside_git_is_served_because_privacy_is_declared_not_asked() {
        let root = scratch("nogit");
        let base = make_base(&root, "loose");
        assert!(!base.join(".git").exists(), "the fixture has no repository on purpose");

        let m = Memory::open(&[&base], false).expect("opens without asking anybody");
        assert_eq!(m.agents.len(), 1, "the base is served");
        assert!(m.entry_count() > 0, "and its entries reach the index");
        assert!(m.skipped_bases().is_empty(), "nothing was left out");
        assert_eq!(m.scope(), Scope::Public);
    }

    // -----------------------------------------------------------------------
    // kb list: a lookup by facet, with no ranking question in it
    // -----------------------------------------------------------------------

    /// A base with files placed by path, no index synced, because a listing walks the
    /// files and never reads a chunk. `MAP.md` is here only so `looks_like_a_base`
    /// recognises the directory when it is pointed at directly.
    fn listed_base(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = scratch(name).join("probe");
        std::fs::create_dir_all(&root).expect("mkdir");
        std::fs::write(root.join("MAP.md"), "# MAP
").expect("map");
        for (rel, text) in files {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
            std::fs::write(path, text).expect("file");
        }
        root
    }

    fn listed_paths(m: &Memory) -> Vec<String> {
        m.list(&crate::list::Filter::default())
            .expect("the listing walks the same bases the memory opened")
            .into_iter()
            .map(|l| l.path)
            .collect()
    }

    /// **THE privacy test for this surface, and it is aimed at one comparison.**
    ///
    /// `Memory::list` reaches the private layer through `Base::discover(root, all)`,
    /// the same call `Memory::open` makes, with `all` recovered from the scope the
    /// memory was opened with. Invert that comparison and every private folder in the
    /// fleet is listed by default, over MCP, to whatever model is reasoning. The
    /// declaration itself stays in `base::private_layer` and is not restated here,
    /// which is ADR-0034's single declaration; this pins that the fifth surface reads
    /// it rather than growing a fourth copy of `profile/ projects/ records/`.
    ///
    /// One surface up from `base::tests::the_folder_map_is_the_private_layer_when_nothing_is_declared`,
    /// which pins the same rule at the walk.
    #[test]
    fn a_private_folder_is_absent_from_a_listing_until_all_asks_for_it() {
        let root = listed_base(
            "list-private",
            &[("knowledge/public.md", "# Public
"), ("profile/me.md", "# Me
")],
        );
        assert!(!root.join("agent.txt").exists(), "nothing is declared, so the folder map applies");

        let public = Memory::open(&[root.as_path()], false).expect("opens");
        let served = listed_paths(&public);
        assert!(served.contains(&"knowledge/public.md".to_string()), "{served:?}");
        assert!(
            !served.contains(&"profile/me.md".to_string()),
            "the private layer is not listed without --all: {served:?}"
        );

        let all = Memory::open(&[root.as_path()], true).expect("opens");
        let rows = all.list(&crate::list::Filter::default()).expect("lists");
        let paths: Vec<&str> = rows.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"knowledge/public.md"), "{paths:?}");
        assert!(paths.contains(&"profile/me.md"), "--all is the deliberate act: {paths:?}");
        assert!(
            rows.iter().any(|r| r.path == "profile/me.md" && r.private),
            "and the row says which one it is"
        );
    }

    /// A listing walks `base.files`, not `index::build`'s entries.
    ///
    /// `index::build` classifies a file with no `Search for:` line as unreachable or
    /// exempt and builds no entry for it, which is the entire deposit plus every
    /// README. Listing the entries instead would answer `--stage raw` with zero rows on
    /// a base full of raw captures, and a raw capture is exactly what that facet is
    /// asked about. The two populations differ on purpose and this pins which one the
    /// listing serves.
    #[test]
    fn a_file_with_no_search_for_line_is_still_listed() {
        let root = listed_base(
            "list-keyless",
            &[
                ("knowledge/keyed.md", "# Keyed

**Search for:** `zebra`
"),
                ("inbox/2026-09-01-drop.md", "# Drop

something nobody has judged yet
"),
            ],
        );
        let m = Memory::open(&[root.as_path()], true).expect("opens");

        let served = listed_paths(&m);
        assert!(served.contains(&"knowledge/keyed.md".to_string()), "{served:?}");
        assert!(
            served.contains(&"inbox/2026-09-01-drop.md".to_string()),
            "a deposit file is on the shelf whether or not a question can reach it: {served:?}"
        );
        assert_eq!(m.entry_count(), 1, "while the index holds only the keyed one");
    }

    /// ADR-0034 on the fifth surface: unjudged material is served with its label on,
    /// never hidden and never bare. The same property
    /// `mcp::tests::a_passage_from_the_deposit_is_served_with_its_label_on` pins for
    /// `kb_retrieve`, so a caller reading a listing and a caller reading passages are
    /// told the same thing about the same file.
    #[test]
    fn a_deposit_file_is_listed_with_its_short_memory_label_on() {
        let root = listed_base(
            "list-layer",
            &[("knowledge/settled.md", "# Settled
"), ("inbox/fresh.md", "# Fresh
")],
        );
        let m = Memory::open(&[root.as_path()], true).expect("opens");
        let rows = m.list(&crate::list::Filter::default()).expect("lists");

        let fresh = rows.iter().find(|r| r.path == "inbox/fresh.md").expect("present");
        let settled = rows.iter().find(|r| r.path == "knowledge/settled.md").expect("present");
        assert_eq!(fresh.layer, crate::retrieve::Layer::Short);
        assert_eq!(settled.layer, crate::retrieve::Layer::Long);
    }

    /// A one agent base on disk with its index synced, the way `kb index` leaves it.
    ///
    /// The sync is not optional and it is the whole subject of the lag tests below.
    /// `Memory::open` reads an index and never builds one, so a fixture that skips the
    /// sync reports the entire base as unindexed, which is true and tests nothing.
    fn synced_base(name: &str, notes: &[(&str, &str)]) -> PathBuf {
        let root = scratch(name).join("probe");
        std::fs::create_dir_all(root.join("knowledge")).expect("mkdir");
        std::fs::write(root.join("agent.txt"), "name = Probe\nrole = testing\n").expect("agent");
        std::fs::write(root.join("MAP.md"), "# MAP\n").expect("map");
        for (rel, text) in notes {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
            std::fs::write(path, text).expect("note");
        }

        let base = Base::discover(&root, true).expect("discover");
        let mut db = Store::open(&index_path(&root)).expect("index");
        db.sync(&base, &index::base_name(&root)).expect("sync");
        root
    }

    fn zebra() -> (&'static str, &'static str) {
        (
            "knowledge/zebra.md",
            "# Zebra\n\n**Search for:** `zebra`, `quagga`\n\n**Exists to:** hold one animal\n",
        )
    }

    /// **The exact window the SessionEnd hook opens, and the reason the two numbers are
    /// two fields.**
    ///
    /// That hook runs `kb capture` and detaches `kb promote`, and it never runs
    /// `kb index`. What `capture::render` writes carries no `Search for:` line, so the
    /// deposit the system makes for itself lands on disk, out of the store, and exempt
    /// from the reachability rule because `inbox/` is a quarantine. Unindexed is a
    /// `kb index` that has not run; unreachable is a file nobody wrote keys for. Collapse
    /// them into one number and the answer to "what do I do about it" is gone.
    #[test]
    fn the_deposit_the_session_writes_for_itself_is_unindexed_and_not_unreachable() {
        let root = synced_base("session-end", &[zebra()]);

        std::fs::create_dir_all(root.join("inbox")).expect("mkdir");
        std::fs::write(
            root.join("inbox").join("2026-09-01-session-abc.md"),
            "# Session abc\n\nwhat the session left behind.\n",
        )
        .expect("deposit");

        let memory = Memory::open(&[root.as_path()], true).expect("opens");
        assert_eq!(memory.unindexed(), vec!["probe/inbox/2026-09-01-session-abc.md"]);
        assert!(
            memory.unreachable().is_empty(),
            "inbox is exempt by design: {:?}",
            memory.unreachable()
        );
    }

    /// **A `nothing` verdict never lost to the floor, and saying it did is the failure
    /// this whole change exists to avoid.**
    ///
    /// `index::route` keeps a hit only under `if score > 0.0`, and `confidence_of`
    /// returns `Verdict::Nothing` only from the `hits.first()` early return, so on every
    /// refusal path that reaches a terminal, an MCP reply or the boot briefing the
    /// keyword score is exactly zero: no term in the question matched any key at all and
    /// the floor was never reached, let alone failed. A sentence of the shape *you scored
    /// 0.0 against a floor of 4.1* reads as *nearly*, sends the reader off to recalibrate
    /// a threshold that did nothing, and is the "instructs wrongly" case that is worse
    /// than a bare refusal.
    #[test]
    fn a_refusal_that_scored_nothing_does_not_blame_the_floor() {
        let mut m = empty_memory();
        m.entries = (0..4)
            .map(|i| index::Entry {
                base: "zed".into(),
                rel: format!("knowledge/{i}.md"),
                stem: i.to_string(),
                title: String::new(),
                keywords: vec![format!("k{i}")],
                summary: String::new(),
                body: String::new(),
            })
            .collect();

        let said = m.shortfall(&saying(Verdict::Nothing)).lines().join(" ");
        assert!(!said.is_empty(), "a refusal that says nothing is the state being fixed");
        assert!(
            said.to_lowercase().contains("no term"),
            "it has to name what actually happened: {said}"
        );
        assert!(
            !said.contains("floor"),
            "nothing was measured against the floor, so the floor must not appear: {said}"
        );
    }

    /// The other half of the branch, and the one the boot briefing actually prints.
    ///
    /// A `Guess` carries a real score under a real floor, so here the floor sentence is
    /// true and both numbers belong in it. The floor printed is the one **on the
    /// `Confidence`**, never `Memory::floor()` recomputed at the print site: the fixture
    /// hands over a floor the memory would never derive, so a surface that recomputed it
    /// fails here rather than in production six months later.
    #[test]
    fn a_refusal_that_scored_and_lost_to_the_floor_names_the_floor_it_lost_to() {
        let m = calibrated_memory();
        let c = Confidence {
            verdict: Verdict::Guess,
            agreement: 1,
            keyword_score: 3.25,
            margin: 1.0,
            floor: 99.5,
        };
        assert!(m.floor() != 99.5, "the fixture floor has to be one the memory would not derive");

        let said = m.shortfall(&c).lines().join(" ");
        assert!(said.contains("3.2") || said.contains("3.3"), "the score it made: {said}");
        assert!(said.contains("99.5"), "the floor it lost to, off the Confidence: {said}");
        assert!(
            !said.contains(&format!("{:.1}", m.floor())),
            "not the floor recomputed from the memory: {said}"
        );
    }

    /// **A fleet under `MIN_ENTRIES_TO_ROUTE` refuses for a reason no score explains, and
    /// today it refuses in exactly the words a thousand entry fleet uses.**
    ///
    /// With one entry every word has `df = 1`, so idf can tell nothing apart and no
    /// evidence can be good evidence. The reader has to be told that, because every
    /// remedy that fits a large base (rewrite the question, fix the keys) is wasted work
    /// here and the only thing that helps is a second note. Written against the constant,
    /// so re-deriving it does not leave a literal 2 stranded in a sentence.
    #[test]
    fn a_refusal_on_a_fleet_too_small_to_route_says_so_and_says_what_to_do() {
        let mut m = empty_memory();
        m.entries = vec![index::Entry {
            base: "zed".into(),
            rel: "knowledge/only.md".into(),
            stem: "only".into(),
            title: String::new(),
            keywords: vec!["zebra".into()],
            summary: String::new(),
            body: String::new(),
        }];
        assert!(!m.enough_to_route(), "the fixture is the state under test");

        let said = m.shortfall(&saying(Verdict::Nothing)).lines().join(" ");
        assert!(said.contains('1'), "the count it actually has: {said}");
        assert!(
            said.contains(&MIN_ENTRIES_TO_ROUTE.to_string()),
            "and the threshold, read off the constant: {said}"
        );
        assert!(
            !said.contains("this base"),
            "the count is fleet wide across every opened root: {said}"
        );
    }

    /// **The guard on "a refusal that instructs can instruct wrongly".**
    ///
    /// A large, fully keyed fleet that simply does not cover the question has no cause to
    /// name, and a remedy printed here sends somebody to repair a base that is not broken.
    /// So the only sentence allowed is the true one about what happened. This fails the
    /// moment a remedy is added that is not conditioned on a fact.
    #[test]
    fn a_base_big_enough_and_fully_keyed_gets_no_invented_cause() {
        let m = calibrated_memory();
        assert!(m.unreachable().is_empty(), "the fixture is a healthy base");

        let said = m.shortfall(&saying(Verdict::Nothing)).lines();
        assert_eq!(said.len(), 1, "one true sentence and no invented cause: {said:?}");
        let one = &said[0];
        assert!(!one.contains("floor"), "nothing reached the floor: {one}");
        assert!(!one.contains("Search for"), "every file here has its keys: {one}");
        assert!(!one.to_lowercase().contains("too small"), "226 entries is not small: {one}");
    }

    /// **One fact, one message.** A base with no entries already has a reply of its own on
    /// the terminal and on the MCP surface, and it says the right thing: the library is
    /// empty, and here is what fills it. A second explanation printed beside it is the
    /// drift this codebase keeps paying for, so this returns nothing and leaves the case
    /// to the text that already owns it.
    #[test]
    fn an_empty_base_still_says_only_the_thing_it_already_says() {
        let m = empty_memory();
        assert!(m.is_empty(), "the fixture is the state under test");
        assert!(
            m.shortfall(&saying(Verdict::Nothing)).lines().is_empty(),
            "the empty base text is the sole answer for an empty base"
        );
    }

    /// **The likeliest cause of a miss on a base that does have files in it.**
    ///
    /// A note with no `Search for:` line is on disk, readable by a person, and scores zero
    /// on every question ever asked. Nothing tells anybody it is there unless they run
    /// `kb check`, and nobody is required to run `kb check`. The refusal is the one moment
    /// the reader is already looking, so it is where the number belongs. The count is
    /// [`Memory::unreachable`], which is `index::is_exempt`'s population and therefore the
    /// same set `kb check` reports as E02: a second definition here would accuse a base of
    /// a fault its own linter says it does not have.
    #[test]
    fn a_refusal_names_the_files_that_declare_no_keys_because_that_is_the_likeliest_cause() {
        let root = synced_base(
            "shortfall-keyless",
            &[zebra(), ("knowledge/keyless.md", "# Keyless\n\nno line saying what finds this.\n")],
        );
        let m = Memory::open(&[root.as_path()], true).expect("opens");
        assert_eq!(m.unreachable(), vec!["probe/knowledge/keyless.md"], "the fixture");

        let said = m.shortfall(&saying(Verdict::Nothing)).lines().join(" ");
        assert!(said.contains('1'), "the count: {said}");
        assert!(said.contains("Search for"), "and what is missing from it: {said}");
        assert!(said.contains("kb check"), "and the verb that names the files: {said}");
    }

    /// The false positive floor. A base whose index is current must report nothing
    /// lagging, which is what guards the map filter and the decision to key the query on
    /// nothing at all rather than on `Agent.name`, which is `"."` when a base is opened as
    /// `.` and would report the whole corpus as missing.
    #[test]
    fn a_base_that_was_indexed_a_moment_ago_is_not_lagging() {
        let root = synced_base("lag-floor", &[zebra()]);
        let memory = Memory::open(&[root.as_path()], true).expect("opens");
        assert!(memory.unindexed().is_empty(), "{:?}", memory.unindexed());
    }
}
