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
}

// **Nothing here touches git, and it used to, at length.** `kb init` ran `git init`,
// staged the files and committed, because the privacy gate read `git ls-files` and an
// agent with no repository was an agent the system it came from refused to serve. That
// grew two more branches, one for an enclosing repository and one for an enclosing
// repository that ignores the path, each found by a real failure. ADR-0034 removed the
// question all three answered. What survives is the `.gitignore` written below, which
// mirrors the private layer so that anybody who later chooses to version a fleet gets
// the right ignores for free. A courtesy, not a dependency.

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

/// Creates the person's base at `<root>/fleet/person`, or directly at `at`.
///
/// **A fleet has agents and it has exactly one person, and the person is not an agent.**
/// So this writes a base with a map, a gitignore and three empty files, and deliberately
/// **no `agent.txt`**: without one the router reads the base and can never elect it as
/// the one who answers, which is the rule ADR-0024 rests on and the code already enforces.
///
/// It also writes no constitution, because a person does not boot. Agents reach the core
/// file through a `[user]` block pointing at `../person/core.md`, and `kb init` writes
/// that block into every agent it creates.
///
/// **The files come out empty on purpose.** This is the shape, published so anyone can
/// build the same structure; the content is one specific human and belongs to whoever
/// runs the fleet. That split is the same one the agent skeleton already makes, and it
/// is the whole answer to how this can be public while its user's file is not.
pub fn person(fleet: &Path, at: Option<&Path>) -> Result<Created, InitError> {
    let root = match at {
        Some(p) => p.to_path_buf(),
        None => fleet.join("fleet").join(PERSON_DIR),
    };

    if root.exists() {
        return Err(InitError::Exists(root));
    }
    fs::create_dir_all(&root).map_err(|e| InitError::Io(root.clone(), e))?;

    write(&root.join("MAP.md"), PERSON_MAP)?;
    write(&root.join(".gitignore"), PERSON_GITIGNORE)?;
    write(&root.join("core.md"), PERSON_CORE)?;
    write(&root.join("work.md"), PERSON_WORK)?;
    write(&root.join("presence.md"), PERSON_PRESENCE)?;

    Ok(Created { path: root, files: 5 })
}

/// The directory the person's base lives in, beside the agents.
pub const PERSON_DIR: &str = "person";

const PERSON_GITIGNORE: &str = "\
# The index is derived from the markdown and rebuilt by `kb index`, so it is never
# committed: the files are the source of truth and the index is a projection.
.kb/
";

const PERSON_MAP: &str = "\
# MAP: who the person is

> **A base, not an agent.** It has no `agent.txt`, so the router reads it and can never
> choose it as the one who answers: a person is not an agent.
>
> Every agent carries `core.md` resident, through a `[user]` block pointing at
> `../person/core.md`. The rest is retrieved when a question calls for it.
>
> **The shape is public and the content is not.** This file and the empty files beside it
> describe how a fleet records the human it works for. What gets written into them is one
> specific person, and belongs to whoever runs the fleet.

---

- **[[core]]** Resident in every constitution, so keep it short: who they are, the
  language they write in, the machine, and how they want to be worked with. Everything
  here is paid for by every question every agent answers.
  Search for: `who am i`, `user`, `person`, `profile`, `how they work`.

- **[[work]]** Retrieved. Employment, stack, level, projects. What an agent touching code
  needs and nobody else does on every question.
  Search for: `work`, `job`, `stack`, `projects`, `level`.

- **[[presence]]** Retrieved. Goals, public surface, how work should find them. What an
  agent writing something publishable needs.
  Search for: `goal`, `presence`, `public`, `positioning`.
";

const PERSON_CORE: &str = "\
# The person, core

**Search for:** `who am i`, `quem sou eu`, `quem e o usuario`, `sobre mim`, `user`, \
`usuario`, `humano`, `pessoa`, `dono`, `owner`, `perfil`, `profile`, `identidade`, \
`identity`, `nome`, `name`, `onde moro`, `cidade`, `fuso`, `timezone`, `idioma`, \
`language`, `portugues`, `ingles`, `como trabalho`, `how i work`, `como me tratar`, \
`preferencias`, `preferences`, `maquina`, `machine`, `notebook`, `hardware`, `setup`

> **Resident in every agent's constitution**, so keep it short: it is paid for by every
> question every agent answers. Detail belongs in the retrieved files beside this one.
>
> Replace everything below. An empty profile is a fleet that does not know who it works
> for, and an agent that does not know who it works for gives generic answers confidently.

| | |
|---|---|
| Name | |
| Role here | |
| Languages | The ones they write in, and the one the repositories use |
| Machine | OS and shell, because half of what an agent suggests depends on it |

## How to work with them

- Who decides, and how.
- What they consider good work, and what they consider bad work, in their own words.
- Whether an agent should push back, and how hard.
- House style: anything that would make an answer read wrong to them.

## Where the rest is

| File | What it holds |
|---|---|
| `work.md` | Employment, stack, level, projects |
| `presence.md` | Goals, public surface |

Anything an agent needs on every question belongs here. Anything one agent needs
sometimes belongs beside this file, and the router will find it.
";

const PERSON_WORK: &str = "\
# The person at work

**Search for:** `work`, `trabalho`, `emprego`, `job`, `carreira`, `career`, `cargo`, \
`role`, `empresa`, `company`, `time`, `team`, `stack`, `tecnologias`, `linguagens`, \
`languages`, `ferramentas`, `tools`, `nivel`, `level`, `senioridade`, `experiencia`, \
`experience`, `projetos`, `projects`, `o que eu faco`, `what i do`, `cliente`, `clients`, \
`freelance`, `renda`, `income`

> Retrieved, not resident. Replace everything below.

## Employment

## Stack

## Level, honestly assessed

An agent that overestimates the person explains too little; one that underestimates them
wastes their time. Write what is true, including what they are weak at, in their words
where possible.

## Projects
";

const PERSON_PRESENCE: &str = "\
# The person in public

**Search for:** `presence`, `presenca`, `publico`, `public`, `posicionamento`, \
`positioning`, `marca pessoal`, `personal brand`, `portfolio`, `site`, `website`, `blog`, \
`redes`, `social`, `linkedin`, `github`, `twitter`, `instagram`, `objetivo`, `goal`, \
`meta`, `onde quero chegar`, `como me acham`, `reputacao`, `reputation`, `audiencia`, \
`audience`, `visibilidade`, `oportunidades`, `opportunities`

> Retrieved, not resident. Replace everything below.

## The goal

What they are trying to become, on what horizon. This is a constraint on what gets built,
not decoration: work that can be shown serves a public goal and private work does not.

## The public surface

Addresses, handles, and which is which.

## What may never be published

The standing rule about their own material. Whatever is written here binds every agent.
";

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

    Ok(Created { path: root, files })
}

/// Whether `dir` is already inside somebody's work tree.
///
/// Whether the enclosing repository ignores this path entirely.
///

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
         - **No em dashes.** Not in chat, not in files, not in code comments, not in commit messages.\n\
         - **Commit with `kb commit <path>... -m \"message\"`, naming every path.** More\n\
           than one session may be writing this tree, and `git add -A` puts another\n\
           session's work under your message. A raw `git commit` is refused by a hook.\n"
    )
}

/// **The generated index declares keys, because a generated agent used to be born invisible.**
///
/// `index::build` skips any file whose header carries no `Search for:` line, so every agent
/// created by this command had operating instructions no question could reach. Measured
/// 2026-08-20: `steve/index.md` and `yaron/index.md` were both absent from the index for
/// that reason, and both were generated here. The seed list is deliberately thin and says
/// so, because the terms that matter are the ones only the author of the agent knows.
///
/// **The obvious phrasings are deliberately absent.** `quem e {name}`, `who is {name}`,
/// `o que {name} faz`, `what does {name} do`, `o que nunca faz` and `never does` were all
/// here and all six were dead on arrival: every other word in them is a stopword, so each
/// reduced to a single word and reached neither the keyword index nor the phrase index.
/// `kb check` says W07 about exactly this, and it said it about this template first.
fn index_md(title: &str) -> String {
    let lower = title.to_lowercase();
    format!(
        "# {title}, operating instructions\n\n\
         > Generated by `kb init`. **Everything below is a placeholder waiting to be replaced.**\n\
         > A generated file left unedited is a file nobody owns.\n\n\
         **Search for:** `{lower}`, `agente {lower}`, `funcao do {lower}`, \
         `role of {lower}`, `operating instructions`, `instrucoes de operacao`, \
         `constituicao`, `constitution`, `ordem de leitura`, `reading order`, \
         `limites`, `limits`, `limite declarado`, `declared limit`, `escopo`, `scope`, \
         `papel`, `role`, `competencia`, `capability`\n\n\
         > **Replace that line.** Half of those terms are this agent's own name and the\n\
         > rest are true of every agent in the fleet, so together they distinguish nothing.\n\
         > Thirty to seventy terms naming what THIS agent is actually asked about, in both\n\
         > languages, is what makes it reachable by a real question.\n\n\
         **Exists to:** Say what {title} is for, what it is asked, and where it stops\n\n\
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
         role = \n\n\
         # The private layer: folders served only with --all. This is the default, so the\n\
         # line is here to be edited, not to be needed. `.` means the whole base. ADR-0034.\n\
         # private = profile/, projects/, records/\n\n\
         # Where this agent stops. Read by the classifier: a roster of roles alone\n\
         # tells it what each agent does and never what none of them does, so an agent\n\
         # generated without this line is one the classifier cannot bound.\n\
         ends = \n"
    )
}

const BLOCKS_TXT: &str = "\
# The constitution, as blocks. See decisions/0007-memory-architecture.md in the
# Ulpia repository.
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

# Who the human is. One file, shared by the whole fleet: a person is not an agent,
# so it lives in a base of its own that the router can read and never elect.
#
# The block is written here and not left to be added later. Until 2026-08-20 this
# template carried the comment and not the block, so every generated agent booted
# without knowing who it works for, which is the exact failure ADR-0024 was written
# about. A comment describing a block that is not there is worse than neither.
[user]
../person/core.md

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

# The inbox, denied by default. The root of it is ours, the subfolders are
# theirs: an agent keeps its own notes as loose files here, and every typed
# subfolder receives somebody else's material. That material is derived by
# ADR-0003, because the source it came from is the truth and the file is a
# projection of it, and it is usually under somebody else's copyright.
#
# **Denied by default because the enumerated version provably failed.** The
# first shape of this rule listed each payload folder by hand. In yaron's base
# that list covered four of the six folders his own `what-goes-here.md`
# documents, missing `articles/` and `posts/`, and the parallel list further
# down the same file missed exactly the same two. A rule you have to remember
# to extend is a rule that is one forgotten line away from committing a file
# nobody meant to publish, and git history is expensive to retract.
#
# The failure modes are not symmetrical, which is the whole argument: forgetting
# to ignore puts somebody else's material in the history permanently, while
# forgetting to un-ignore only means a note of ours is not backed up, and that
# is noticed and fixed in a minute. Prefer the failure you can see.
#
# Line by line, because three of these four are load bearing and none is obvious:
#   inbox/**             everything under the inbox, at any depth.
#   !inbox/*.md          our own notes, at the root only. A `*` never crosses a
#                        `/`, so this cannot reach into a subfolder.
#   !inbox/**/           re-includes the DIRECTORIES. Without this line git never
#                        descends into an excluded directory and every negation
#                        below it dies in silence, which is the single mistake
#                        this pattern is usually written with.
#   !inbox/**/.gitkeep   git versions files and not folders, so an empty folder
#                        does not survive a clone. This file is what makes the
#                        structure exist.
#   !inbox/**/SOURCES.md the ledger. Ignoring a derived file is only honest when
#                        what it came from is written down, or a fresh clone can
#                        neither regenerate the material nor say what was here.
inbox/**
!inbox/*.md
!inbox/**/
!inbox/**/.gitkeep
!inbox/**/SOURCES.md
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

    /// The system must be able to serve what it just created, and it must do so
    /// without having created a repository to make that possible. Three tests stood
    /// here before ADR-0034, each pinning a branch of `git init` behaviour found by a
    /// real failure; they went with the branches, and this is the property they were
    /// all in service of.
    #[test]
    fn a_generated_agent_is_served_at_once_and_no_repository_was_created_for_it() {
        let fleet = scratch("openable");
        let made = agent(&fleet, "newton", None).expect("init");
        assert!(!made.path.join(".git").exists(), "no repository, by design");

        let base = crate::base::Base::discover(&made.path, false).expect("discover");
        assert!(!base.files.is_empty(), "the public files are there without asking anybody");

        crate::memory::Memory::open(&[&made.path], false)
            .expect("the system must be able to serve what it just created");
    }

    /// The courtesy that survived: anybody who later versions the fleet gets ignores
    /// that match the declared private layer, so the two cannot start out disagreeing.
    #[test]
    fn the_gitignore_written_mirrors_the_default_private_layer() {
        let fleet = scratch("ignores");
        let made = agent(&fleet, "newton", None).expect("init");
        let ignores = fs::read_to_string(made.path.join(".gitignore")).expect("written");
        for folder in crate::base::PRIVATE_DEFAULT {
            assert!(
                ignores.lines().any(|l| l.trim() == format!("{folder}/")),
                "{folder}/ is in the private layer and must be ignored: {ignores}"
            );
        }
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

    /// The same drift guard as the agent skeleton, for the same reason, with more at
    /// stake. `person-skeleton/` is the public answer to a question the product has to
    /// answer out loud: **how does a fleet record the human it works for, and how does
    /// that stay publishable when the human's own file never can be.** A published shape
    /// that no longer matches what the tool writes is documentation lying about its own
    /// product, and it lies most convincingly about the part nobody can check by reading
    /// the private repository.
    #[test]
    fn the_published_person_skeleton_is_what_this_code_writes() {
        let published = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../person-skeleton");
        assert!(
            published.is_dir(),
            "expected the published person skeleton at {}. Regenerate it with \
             `kb init --person <tmp>` and move fleet/person to the repository root.",
            published.display()
        );

        let scratch = scratch("person-skeleton-drift");
        let made = person(&scratch, None).expect("writes");

        let mut checked = 0;
        for entry in fs::read_dir(&made.path).expect("read generated") {
            let entry = entry.expect("entry");
            let name = entry.file_name();
            let theirs = published.join(&name);
            assert!(theirs.is_file(), "{:?} is generated but not published", name);
            let a = fs::read_to_string(entry.path()).expect("generated").replace("\r\n", "\n");
            let b = fs::read_to_string(&theirs).expect("published").replace("\r\n", "\n");
            assert_eq!(a, b, "{:?} drifted from what kb init --person writes", name);
            checked += 1;
        }
        assert_eq!(checked, 5, "every generated file is compared, none skipped");
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
