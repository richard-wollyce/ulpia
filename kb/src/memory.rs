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

pub struct Memory {
    entries: Vec<Entry>,
    aliases: Vec<(String, String)>,
    store: Store,
    scope: Scope,
    /// The bases actually opened, after any fleet root was expanded. Reported so a
    /// caller can say what it is serving rather than what it was asked for.
    pub bases: Vec<PathBuf>,
    /// True when the index had to be discarded on open. The caller has to surface
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
    pub fn open(paths: &[&Path], private: bool, db: &Path) -> Result<Memory, OpenError> {
        let mut entries = Vec::new();
        let mut aliases = Vec::new();
        let mut bases = Vec::new();

        for path in expand_roots(paths) {
            let base = Base::discover(&path, private)
                .map_err(|e| OpenError::Unreadable(path.clone(), e))?;

            if !private && !base.tracked_only {
                return Err(OpenError::PrivacyUnknowable(path));
            }

            entries.extend(index::build(&base));
            aliases.extend(base.aliases.clone());
            bases.push(path);
        }

        let store = Store::open(db).map_err(|e| OpenError::Store(e.to_string()))?;
        let index_was_rebuilt = store.rebuilt;

        Ok(Memory {
            entries,
            aliases,
            store,
            scope: if private { Scope::All } else { Scope::Public },
            bases,
            index_was_rebuilt,
        })
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn alias_count(&self) -> usize {
        self.aliases.len()
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
        let text = self
            .store
            .search(&terms, top * retrieve::TEXT_OVERSAMPLE, self.scope)
            .unwrap_or_default();

        retrieve::fuse(&keyword, &text, top)
    }

    /// Measures a claim against what the base already says and proposes ADD, UPDATE
    /// or NOOP with the evidence. **Writes nothing and decides nothing.**
    pub fn remember(&self, claim: &str) -> Assessment {
        let terms = index::expand_query(claim, &self.aliases);
        let hits = self
            .store
            .search(&terms, remember::EVIDENCE_WIDTH, self.scope)
            .unwrap_or_default();
        remember::assess(claim, &terms, &hits)
    }

    /// True when the full text index has no chunks for anything the keywords ranked,
    /// which almost always means the index is stale rather than the base being thin.
    /// Worth saying out loud: the alternative is a caller concluding the base is empty.
    pub fn looks_stale(&self, found: &[Retrieved]) -> bool {
        !found.is_empty() && found.iter().all(|f| f.passages.is_empty())
    }
}

/// Expands any fleet root into the bases under it, leaving real bases alone.
///
/// A directory is a fleet root when it holds no map file of its own but has immediate
/// children that do. Anything else is passed through untouched, including a path that
/// is neither, so the error comes from `Base::discover` where it can say why.
fn expand_roots(paths: &[&Path]) -> Vec<PathBuf> {
    let mut out = Vec::new();

    for path in paths {
        if crate::base::has_map(path) {
            out.push(path.to_path_buf());
            continue;
        }

        let mut children: Vec<PathBuf> = match std::fs::read_dir(path) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir() && crate::base::has_map(p))
                .collect(),
            Err(_) => Vec::new(),
        };

        if children.is_empty() {
            // Not a base and not a root. Pass it through so discover reports it.
            out.push(path.to_path_buf());
        } else {
            // Sorted, so the order a fleet is opened in does not depend on the order
            // the filesystem happens to hand back.
            children.sort();
            out.append(&mut children);
        }
    }

    out
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
        let db = root.join("i.db");

        match Memory::open(&[&base], false, &db) {
            Err(OpenError::PrivacyUnknowable(p)) => assert_eq!(p, base),
            Err(e) => panic!("expected a privacy refusal, got {e}"),
            Ok(_) => panic!("a base outside git must not open read only: privacy is unknowable"),
        }

        // Asking for it explicitly is allowed: that is the deliberate act.
        let m = Memory::open(&[&base], true, &db).expect("private open");
        assert_eq!(m.scope(), Scope::All);
    }
}
