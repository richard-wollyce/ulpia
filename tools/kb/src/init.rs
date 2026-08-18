//! Creating an agent, in the shape ADR-0011 defines.
//!
//! This exists because the orchestrator will create agents, and an orchestrator that
//! creates an agent has to know what an agent is. Zed, Steve and Yaron were built by
//! hand, which is fine for three and impossible for three hundred.
//!
//! **A fresh agent passes `kb check` with no findings.** That is the acceptance test
//! and it is not decorative: a generator whose output fails the project's own linter
//! teaches everyone to ignore the linter.
//!
//! The templates are deliberately thin. A generated agent should read like a
//! skeleton waiting to be filled, not like a finished thing with the wrong contents:
//! prose nobody wrote is prose nobody owns, and it survives for years because
//! deleting someone else's words feels presumptuous.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Directories every agent has. Empty ones are kept with a `.gitkeep`, because a
/// shape that only appears once it is used is a shape nobody discovers.
const DIRS: &[&str] = &[
    "knowledge",
    "inbox",
    "decisions",
    "protocols",
    "templates",
];

pub struct Created {
    pub path: PathBuf,
    pub files: usize,
    /// False when git was already there or could not be run. Reported rather than
    /// assumed: the privacy gate needs git per base, so an agent without it is an
    /// agent whose private layer cannot be told apart from its public one.
    pub git: bool,
}

#[derive(Debug)]
pub enum InitError {
    Exists(PathBuf),
    BadName(String),
    Io(PathBuf, io::Error),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::Exists(p) => write!(
                f,
                "{} already exists. Refusing to write into it: an agent half \
                 overwritten is worse than one not created.",
                p.display()
            ),
            InitError::BadName(n) => write!(
                f,
                "'{n}' is not a usable agent name. Use lowercase letters, digits and \
                 hyphens: the name becomes a directory, and a directory name that \
                 needs quoting is one every later command has to quote."
            ),
            InitError::Io(p, e) => write!(f, "cannot write {}: {e}", p.display()),
        }
    }
}

/// Creates an agent under `<root>/fleet/<name>`, or directly at `at` when given.
///
/// The fleet root is where ADR-0011 says agents live. Passing an explicit path is
/// still allowed, because the library accepts any path even though the product has
/// a home.
pub fn agent(fleet: &Path, name: &str, at: Option<&Path>) -> Result<Created, InitError> {
    if !valid_name(name) {
        return Err(InitError::BadName(name.to_string()));
    }

    let root = match at {
        Some(p) => p.to_path_buf(),
        None => fleet.join("fleet").join(name),
    };

    if root.exists() {
        return Err(InitError::Exists(root));
    }

    let title = title_case(name);
    let mut files = 0usize;

    for dir in DIRS {
        let path = root.join(dir);
        fs::create_dir_all(&path).map_err(|e| InitError::Io(path.clone(), e))?;
        write(&path.join(".gitkeep"), "")?;
        files += 1;
    }

    write(&root.join("CLAUDE.md"), &claude_md(&title))?;
    write(&root.join("index.md"), &index_md(&title))?;
    write(&root.join("MAP.md"), &map_md(&title))?;
    write(&root.join("agent.txt"), &agent_txt(&title))?;
    write(&root.join("blocks.txt"), BLOCKS_TXT)?;
    write(&root.join("kb-aliases.txt"), ALIASES_TXT)?;
    write(&root.join(".gitignore"), GITIGNORE)?;
    files += 7;

    // Not optional, and **not finished at `git init`**. The privacy gate reads
    // `git ls-files`, which is empty in a repository with no commit, and an empty
    // listing is indistinguishable from "git could not be asked". A freshly created
    // agent would therefore be refused by the very system that created it, which is
    // the kind of gap only found by running the thing end to end.
    let git = run_git(&root, &["init", "--quiet"])
        && run_git(&root, &["add", "-A"])
        && run_git(
            &root,
            &[
                "-c",
                "user.name=kb",
                "-c",
                "user.email=kb@localhost",
                "commit",
                "--quiet",
                "-m",
                "Create the agent, in the shape ADR-0011 defines",
            ],
        );

    Ok(Created { path: root, files, git })
}

/// The identity is set per command rather than written into the repository config,
/// so the first real commit uses whoever the machine says the author is. A generated
/// identity that outlives generation is an author nobody chose.
fn run_git(root: &Path, args: &[&str]) -> bool {
    crate::base::quiet("git")
        .args(args)
        .current_dir(root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Lowercase, digits and hyphens. The name becomes a directory and appears in every
/// path afterwards, so anything needing a quote is rejected at the only moment it is
/// cheap to reject.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

fn title_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn write(path: &Path, contents: &str) -> Result<(), InitError> {
    fs::write(path, contents).map_err(|e| InitError::Io(path.to_path_buf(), e))
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

fn claude_md(title: &str) -> String {
    format!(
        "# {title}\n\n\
         You are **{title}**.\n\n\
         **Read [`index.md`](index.md) before answering anything.** It is the master file: \n\
         identity, method, the bar, autonomy, folder map, protocols. It is the first read, every\n\
         time.\n\n\
         Rules that always apply, even in a one line answer:\n\n\
         - **Name the mechanism.** A recommendation without the reason it works does not ship.\n\
         - **Two options and their consequences**, or it is a preference, not a decision.\n\
         - **Mark what is unverified.** Ran it, read the source, read the docs, or guessing. Say which.\n\
         - **Never claim something works without running it.** If it was not run, say it was not run.\n\
         - **No em dashes.** Not in chat, not in files, not in code comments, not in commit messages.\n"
    )
}

fn index_md(title: &str) -> String {
    format!(
        "# {title}, operating instructions\n\n\
         > Generated by `kb init`. **Everything below is a placeholder waiting to be replaced.**\n\
         > A generated file left unedited is a file nobody owns.\n\n\
         ## Who this agent is\n\n\
         One paragraph. What it is for, and what it is deliberately not for. The second half\n\
         matters more: an agent with no stated limit gets asked for everything and is bad at\n\
         most of it.\n\n\
         ## Reading order\n\n\
         1. `index.md`, these instructions\n\
         2. `agent.txt`, the name and role\n\
         3. `MAP.md`, what exists in the base\n\n\
         ## What this agent never does\n\n\
         The list that outranks everything else. Write it before the capability list, because\n\
         a limit added after the fact is a limit that loses every argument with a deadline.\n"
    )
}

fn map_md(title: &str) -> String {
    format!(
        "# MAP: the knowledge base map\n\n\
         > The first file to read on any query, after `index.md`. It says what exists, where it\n\
         > lives, and what connects to what.\n\
         >\n\
         > Link convention: `[[file-name]]`, no extension. One subject per file, descriptive\n\
         > kebab-case names. **Every new file gets an entry here, in the same move that creates\n\
         > it. A file nobody can find does not exist.**\n\n\
         ---\n\n\
         ## Folder structure\n\n\
         | Folder | What goes here |\n\
         |---|---|\n\
         | `knowledge/` | Distillations by domain. **The brain** |\n\
         | `inbox/` | Raw material awaiting distillation |\n\
         | `decisions/` | Decisions that outlive a conversation |\n\
         | `protocols/` | This agent's own procedures |\n\
         | `templates/` | Document skeletons |\n\n\
         ---\n\n\
         ## Current contents\n\n\
         Nothing yet. {title} was created by `kb init` and has not been fed.\n\n\
         Each entry below gets a `Search for:` line carrying the words a real question would\n\
         use, because that line is what the router matches against. An entry without one is an\n\
         entry grep cannot reach.\n"
    )
}

fn agent_txt(title: &str) -> String {
    format!(
        "# Read by the orchestrator to name and route.\n\
         #\n\
         # Separate from index.md on purpose: blocks.txt orders the constitution by stability\n\
         # and index.md sits in the most stable block, so a field drawn in a menu does not\n\
         # belong inside it.\n\n\
         name = {title}\n\
         role = \n"
    )
}

const BLOCKS_TXT: &str = "\
# The constitution, as blocks. See zed/decisions/0007-memory-architecture.md.
#
# Order is by how often a block changes, most stable first, and it is not cosmetic.
# Prefix caching reuses the KV state of a prompt only up to the first token that
# differs, so a change invalidates its own block and everything after it. Put a
# frequently changing block early and every switch pays to recompute the stable ones
# behind it, for nothing.
#
# Run `kb blocks .` to see what each one costs and what changing it costs.

# Who the agent is and how it works. Changes rarely.
[identity]
CLAUDE.md
index.md

# What exists in the base. Changes whenever a note is added, so it goes last among
# the resident blocks, and it leaves the resident set once routing happens outside
# the model.
[map]
MAP.md

# What is open right now. Fetched only when the question needs it.
[session] on-demand
";

const ALIASES_TXT: &str = "\
# alias = canonical, one per line, # starts a comment.
#
# Expansion is additive: the original words always survive, so a wrong line can add
# noise and can never remove signal.
#
# **Only add a line after a real question missed.** This is a record of misses, not a
# dictionary, and a dictionary is what makes it unmaintainable.
";

const GITIGNORE: &str = "\
# The derived index. Disposable by ADR-0003: deleting it costs a rebuild.
.kb/

# The private layer. Gitignored by design, because it is nobody's to publish.
profile/
records/
projects/
";

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-init-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// The acceptance test for the whole module: a generated agent has to satisfy
    /// the project's own linter. A generator whose output fails the linter teaches
    /// everyone to ignore the linter.
    #[test]
    fn a_generated_agent_passes_check_with_no_findings() {
        let fleet = scratch("clean");
        let made = agent(&fleet, "newton", None).expect("init");

        let base = crate::base::Base::discover(&made.path, true).expect("discover");
        let findings = crate::checks::run(&base);
        assert!(
            findings.is_empty(),
            "a fresh agent must be clean, got: {:?}",
            findings.iter().map(|f| &f.code).collect::<Vec<_>>()
        );
    }

    /// The gap that `git init` alone left. `git ls-files` is empty in a repository
    /// with no commit, and `tracked_files` cannot tell an empty listing from git
    /// being unavailable, so `Memory::open` refused the agent it had just created.
    #[test]
    fn a_generated_agent_can_be_opened_without_asking_for_the_private_layer() {
        let fleet = scratch("openable");
        let made = agent(&fleet, "newton", None).expect("init");
        assert!(made.git, "git has to be usable, or the privacy gate cannot run");

        let base = crate::base::Base::discover(&made.path, false).expect("discover");
        assert!(
            base.tracked_only,
            "a fresh agent must have tracked files, or Memory::open refuses it"
        );
        assert!(!base.files.is_empty(), "and the tracked set must not be empty");

        crate::memory::Memory::open(&[&made.path], false)
            .expect("the system must be able to serve what it just created");
    }

    #[test]
    fn a_generated_agent_is_a_base_the_fleet_finds() {
        let fleet = scratch("found");
        agent(&fleet, "newton", None).expect("init");
        agent(&fleet, "curie", None).expect("init");

        let names: Vec<String> = crate::memory::expand_roots(&[&fleet])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["curie", "newton"]);
    }

    #[test]
    fn the_blocks_manifest_is_readable_by_the_blocks_command() {
        let fleet = scratch("blocks");
        let made = agent(&fleet, "newton", None).expect("init");
        let report = crate::blocks::read(&made.path).expect("blocks.txt must parse");
        assert!(
            report.iter().any(|b| b.name == "identity"),
            "the identity block is what an agent wakes with"
        );
    }

    /// The published skeleton has to be exactly what this code writes.
    ///
    /// `agent-skeleton/` exists so somebody browsing the repository can see the shape
    /// of an agent without installing a toolchain and running anything. That is worth
    /// having, and it introduces the failure this project keeps meeting: two places
    /// describing the same thing, drifting apart, with nothing to notice.
    ///
    /// So the skeleton is **generated by this function and checked against it**. Change
    /// a template and this test fails and tells you to regenerate. There is no version
    /// where the repository shows a shape the tool no longer produces.
    ///
    /// Line endings are normalised before comparing, because git converts them on
    /// checkout when `core.autocrlf` is on, and a test that fails on a Windows clone
    /// but not a Linux one is a test people learn to ignore.
    #[test]
    fn the_published_skeleton_is_what_this_code_writes() {
        let published = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../agent-skeleton");
        assert!(
            published.is_dir(),
            "expected the published skeleton at {}. Regenerate it with \
             `kb init skeleton <tmp>`, delete its .git, and move it to the repository root.",
            published.display()
        );

        let fleet = scratch("skeleton-drift");
        let made = agent(&fleet, "skeleton", None).expect("init");

        let mut theirs = listing(&published);
        let mut ours = listing(&made.path);
        theirs.sort();
        ours.sort();
        assert_eq!(
            ours.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            theirs.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            "the published skeleton has a different set of files from what kb init writes"
        );

        for ((path, ours), (_, theirs)) in ours.iter().zip(&theirs) {
            assert_eq!(
                ours, theirs,
                "{path} differs between kb init and the published skeleton"
            );
        }
    }

    /// Every file under `root`, as (relative path, contents with `\r\n` folded to `\n`).
    /// `.git` is skipped: `kb init` makes a repository and the published copy has none.
    fn listing(root: &Path) -> Vec<(String, String)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name == ".git" {
                    continue;
                }
                if path.is_dir() {
                    walk(root, &path, out);
                    continue;
                }
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
                out.push((rel, text));
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out
    }

    /// Refusing beats merging. An agent half written over is harder to diagnose than
    /// one that was never created.
    #[test]
    fn creating_over_an_existing_agent_is_refused() {
        let fleet = scratch("exists");
        agent(&fleet, "newton", None).expect("first");
        assert!(matches!(
            agent(&fleet, "newton", None),
            Err(InitError::Exists(_))
        ));
    }

    #[test]
    fn a_name_that_would_need_quoting_is_rejected_at_the_cheap_moment() {
        let fleet = scratch("names");
        for bad in ["", "My Agent", "agent/", "-lead", "trail-", "Ágata"] {
            assert!(
                matches!(agent(&fleet, bad, None), Err(InitError::BadName(_))),
                "{bad:?} must be rejected"
            );
        }
        assert!(agent(&fleet, "agent-2", None).is_ok());
    }
}
