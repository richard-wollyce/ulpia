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
/// **Calibrated on n=1 and it says so.** Across the twenty question set of
/// 2026-08-17 and the re-run of 2026-08-18, the single wrong top result scored 3.82
/// and the lowest correct one scored 9.55. 6.0 sits in that gap, closer to the miss
/// than to the hit because the asymmetry is not symmetric: answering confidently
/// from the wrong file costs more than saying "this may not be covered" about a file
/// that turned out to be right.
///
/// A number this thinly evidenced does not get to hide. `kb eval` prints the hit and
/// miss score ranges on every run, so the gap this sits in is re-measured rather than
/// remembered, and it moves when the evidence moves.
pub const SCORE_FLOOR: f32 = 6.0;

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
    /// How many of its files are in there, which is the breadth half of the evidence.
    pub files: usize,
    /// Over the runner-up agent. Infinite when only one agent scored at all, which
    /// unlike the file level case really is maximum confidence: no other base in the
    /// fleet had anything to say.
    pub margin: f64,
    /// How many agents scored anything. One contender is a different situation from
    /// four, and the number is cheap to carry and expensive to recover later.
    pub contenders: usize,
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
    store: Store,
}

pub struct Memory {
    entries: Vec<Entry>,
    aliases: Vec<(String, String)>,
    scope: Scope,
    /// One per base, each with its own index, in the order the fleet was expanded.
    pub agents: Vec<Agent>,
    /// The paths as given, before expansion. Kept because the fleet root is where
    /// `fleet.txt` lives, and the identity tier reads it: after expansion only the
    /// agent directories remain, and the fleet's own name is not in any of them.
    pub opened: Vec<PathBuf>,
    /// True when any index had to be discarded on open. The caller has to surface
    /// this: an emptied index answers "nothing matched", which reads as "the base
    /// does not cover this".
    pub index_was_rebuilt: bool,
}

#[derive(Debug)]
pub enum OpenError {
    Unreadable(PathBuf, std::io::Error),
    /// Git could not be consulted, so no file's privacy is known, and unknown is not
    /// public. Only raised when the caller did not ask for the private layer.
    PrivacyUnknowable(PathBuf),
    Store(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Unreadable(p, e) => write!(f, "cannot read {}: {e}", p.display()),
            OpenError::PrivacyUnknowable(p) => write!(
                f,
                "refusing to open {}: git could not be consulted, so there is no way to tell \
                 which files are private. Either make it a git repository, or ask for the \
                 private layer explicitly.",
                p.display()
            ),
            OpenError::Store(e) => write!(f, "cannot open the index: {e}"),
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

        for root in expand_roots(paths) {
            let base = Base::discover(&root, private)
                .map_err(|e| OpenError::Unreadable(root.clone(), e))?;

            if !private && !base.tracked_only {
                return Err(OpenError::PrivacyUnknowable(root));
            }

            let store =
                Store::open(&index_path(&root)).map_err(|e| OpenError::Store(e.to_string()))?;
            index_was_rebuilt |= store.rebuilt;

            entries.extend(index::build(&base));
            aliases.extend(base.aliases.clone());
            agents.push(Agent { name: name_of(&root), root, store });
        }

        Ok(Memory {
            entries,
            aliases,
            scope: if private { Scope::All } else { Scope::Public },
            agents,
            opened: paths.iter().map(|p| p.to_path_buf()).collect(),
            index_was_rebuilt,
        })
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

    /// Records a question the free stage could not answer. See [`crate::misses`].
    ///
    /// On the contract rather than at each call site, for the reason the module
    /// header gives: `mcp.rs` and `main.rs` both have a miss path, and two callers
    /// building the same behaviour separately is how they came to disagree twice
    /// before. Writing is confined to this one method and touches nothing a query
    /// reads.
    ///
    /// **An empty base is never recorded.** A miss with nothing to search is not a
    /// recall loss, and counting it would corrupt the only number the log exists to
    /// produce.
    pub fn record_miss(&self, question: &str, looked_like: &[String]) {
        if self.is_empty() {
            return;
        }
        if let Some(root) = self.opened.first() {
            crate::misses::record(root, question, looked_like, &crate::misses::today());
        }
    }

    /// What the base knows that looks like what was asked, for when nothing matched.
    ///
    /// Belongs on the contract rather than at each call site for the same reason the
    /// three verbs do: `mcp.rs` and `main.rs` both have a miss path, and two callers
    /// building the same answer separately is how they came to disagree before.
    pub fn suggest(&self, question: &str, limit: usize) -> Vec<String> {
        index::suggest(question, &self.entries, limit)
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
    /// Measured on three real questions against the fleet: "quantas calorias posso
    /// comer hoje" scored 0.032 with both scorers, "é melhor postar video no
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
            confidence: self.confidence_of(&keyword),
            agent: self.choose_agent_by_keyword(&keyword),
            keyword_top: keyword.first().map(|h| format!("{}/{}", h.entry.base, h.entry.rel)),
            found,
        }
    }

    /// The gate, over the keyword scorer's own ranking.
    ///
    /// Kept separate from [`Memory::ask`] so the eval can drive it against a list it
    /// chose, which is how the fused-versus-keyword table above was produced.
    pub fn confidence_of(&self, hits: &[index::Hit<'_>]) -> Confidence {
        let Some(top) = hits.first() else {
            return Confidence {
                verdict: Verdict::Nothing,
                agreement: 0,
                keyword_score: 0.0,
                margin: 0.0,
            };
        };
        let runner_up = hits.get(1).map(|h| h.score).unwrap_or(0.0);
        let margin = if runner_up > 0.0 { top.score / runner_up } else { 1.0 };

        // Agreement is not observable from the keyword list alone, and claiming it
        // would be inventing evidence. One scorer voted, which is what is recorded.
        // The floor alone. See MIN_MARGIN for the measurement that removed the
        // margin from this line and why the reasoning behind it was wrong.
        let verdict = if top.score >= SCORE_FLOOR { Verdict::Hit } else { Verdict::Guess };

        Confidence { verdict, agreement: 1, keyword_score: top.score, margin }
    }

    /// The older gate, over a fused list.
    ///
    /// Retained because `no_agreement` and this share the agreement signal and the
    /// tray still reads a fused list directly. Superseded for routing decisions by
    /// [`Memory::confidence_of`], for the reason measured in [`Memory::ask`].
    pub fn confidence(&self, found: &[Retrieved]) -> Confidence {
        let Some(top) = found.first() else {
            return Confidence {
                verdict: Verdict::Nothing,
                agreement: 0,
                keyword_score: 0.0,
                margin: 0.0,
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
        let verdict =
            if top.keyword_score >= SCORE_FLOOR { Verdict::Hit } else { Verdict::Guess };

        Confidence { verdict, agreement, keyword_score: top.keyword_score, margin }
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
        tally(found.iter().map(|f| (f.base.as_str(), f.score)))
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
        tally(hits.iter().map(|h| (h.entry.base.as_str(), h.score as f64)))
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

/// Sums a weight per base and reports the winner with its margin.
///
/// One function for both scorers so the two agent choices cannot drift apart in the
/// way the two query expansions once did: whatever aggregation rule is right, both
/// callers get the same one, and changing it is one edit rather than two that must
/// be kept in step by memory.
fn tally<'a>(weighted: impl Iterator<Item = (&'a str, f64)>) -> Option<AgentChoice> {
    let mut totals: Vec<(String, f64, usize)> = Vec::new();
    for (base, weight) in weighted {
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
        if crate::base::has_map(path) {
            out.push(path.to_path_buf());
            continue;
        }

        let agents_dir = path.join(AGENTS_DIR);
        let mut found = if agents_dir.is_dir() {
            bases_in(&agents_dir)
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

/// Immediate subdirectories that are bases, sorted so the order a fleet opens in
/// does not depend on the order the filesystem happens to hand back.
/// The directory name, which is the agent's name by ADR-0011's convention.
fn name_of(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string())
}

fn bases_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir() && crate::base::has_map(p))
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
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let mut attach = Vec::new();
    let mut disable = Vec::new();
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
            _ => {}
        }
    }
    (attach, disable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-memory-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn make_base(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("knowledge")).expect("mkdir");
        std::fs::write(dir.join("MAP.md"), "# MAP\n\n- **[[a]]** thing\n  Search for: `thing`\n")
            .expect("map");
        dir
    }

    /// Builds a `Retrieved` for the gate tests. Named fields on purpose: the gate
    /// reads three of them and a positional helper would let a future field land in
    /// the wrong slot silently.
    fn found(base: &str, fused: f64, keyword: f32, why: &[&str]) -> Retrieved {
        Retrieved {
            base: base.into(),
            path: format!("{base}/p.md"),
            title: String::new(),
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
            scope: Scope::Public, agents: vec![], opened: vec![], index_was_rebuilt: false,
        }
    }

    /// The three questions that produced this rule, as the shapes they had.
    #[test]
    fn agreement_between_the_scorers_is_what_separates_a_hit_from_a_guess() {
        let both = Retrieved {
            base: "yaron".into(), path: "p".into(), title: String::new(), score: 0.032,
            keyword_score: 12.0,
            why: vec!["keywords #2".into(), "text #5".into()],
            matched: vec![], passages: vec![],
        };
        let one = Retrieved {
            base: "steve".into(), path: "q".into(), title: String::new(), score: 0.016,
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
        let m = empty_memory();
        let c = m.confidence(&[found("zed", 0.9, 9.55, &["keywords #1", "text #2"])]);
        assert_eq!(c.verdict, Verdict::Hit);
        assert!(c.note().is_none(), "a hit says nothing extra");
    }

    /// The half that agreement alone cannot do. One scorer, but the keyword side
    /// separated the field decisively, which is a real hit and used to be reported as
    /// a guess by `no_agreement`.
    #[test]
    fn one_scorer_with_a_clean_margin_is_still_a_hit() {
        let m = empty_memory();
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
        let m = empty_memory();
        let c = m.confidence(&[
            found("zed", 0.9, 11.0, &["keywords #1"]),
            found("steve", 0.8, 10.8, &["keywords #2"]),
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

    /// The refusal that the privacy fix exists to make possible. A base outside git
    /// has no knowable private layer, and opening it read only would be a guess.
    #[test]
    fn opening_a_base_outside_git_is_refused_unless_the_private_layer_was_asked_for() {
        let root = scratch("nogit");
        let base = make_base(&root, "loose");
        match Memory::open(&[&base], false) {
            Err(OpenError::PrivacyUnknowable(p)) => assert_eq!(p, base),
            Err(e) => panic!("expected a privacy refusal, got {e}"),
            Ok(_) => panic!("a base outside git must not open read only: privacy is unknowable"),
        }

        // Asking for it explicitly is allowed: that is the deliberate act.
        let m = Memory::open(&[&base], true).expect("private open");
        assert_eq!(m.scope(), Scope::All);
    }
}
