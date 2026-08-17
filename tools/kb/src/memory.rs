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
const AGENTS_DIR: &str = "agents";

/// The manifest, for what convention cannot express.
const MANIFEST: &str = "fleet.txt";

/// Expands fleet roots into the bases under them, leaving real bases alone.
///
/// Three shapes are recognised, in this order:
///
/// 1. **A base.** It holds a map file. Returned untouched, even if it contains other
///    bases, because expanding it would silently drop the parent's own notes.
/// 2. **A fleet root**, ADR-0011's layout: it holds an `agents/` directory. Every
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

    /// The three questions that produced this rule, as the shapes they had.
    #[test]
    fn agreement_between_the_scorers_is_what_separates_a_hit_from_a_guess() {
        let both = Retrieved {
            base: "yaron".into(), path: "p".into(), title: String::new(), score: 0.032,
            why: vec!["keywords #2".into(), "text #5".into()],
            matched: vec![], passages: vec![],
        };
        let one = Retrieved {
            base: "steve".into(), path: "q".into(), title: String::new(), score: 0.016,
            why: vec!["text #1".into()],
            matched: vec![], passages: vec![],
        };

        let m = Memory {
            entries: vec![], aliases: vec![],
            scope: Scope::Public, agents: vec![], opened: vec![], index_was_rebuilt: false,
        };

        assert!(m.no_agreement(&[one.clone()]), "one scorer alone is a guess");
        assert!(!m.no_agreement(&[both.clone()]), "two scorers agreeing is a hit");
        assert!(
            !m.no_agreement(&[one, both]),
            "one agreed result is enough; the warning is about nobody agreeing"
        );
        assert!(!m.no_agreement(&[]), "nothing found is a different message");
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
    /// `agents/`. Written before the ADR existed and not revisited when the ADR
    /// landed, so the code and the decision disagreed until a real fleet was built.
    #[test]
    fn a_fleet_root_finds_the_agents_directory() {
        let root = scratch("agents-dir");
        let agents = root.join("agents");
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
        let agents = root.join("agents");
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
        let agents = root.join("agents");
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
