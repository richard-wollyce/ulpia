//! Discovering a knowledge base on disk.
//!
//! A base is a directory holding markdown notes plus one map file at its root.
//! The three agents use different names for the same things, so the shapes are
//! detected rather than configured:
//!
//! | Agent | Map file    | Knowledge folder |
//! |-------|-------------|------------------|
//! | Zed   | `MAP.md`    | `knowledge/`     |
//! | Steve | `INDEX.md`  | `knowledge/`     |
//! | Yaron | `MAPA.md`   | `conhecimento/`  |

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories never worth walking into. Dot directories are skipped separately.
const SKIP_DIRS: &[&str] = &["target", "node_modules", "dist", "build"];

/// Root map file names, in the order they are looked for.
const MAP_NAMES: &[&str] = &["MAP.md", "INDEX.md", "MAPA.md"];

/// Folder holding the distilled notes, in the order it is looked for.
const KNOWLEDGE_DIRS: &[&str] = &["knowledge", "conhecimento"];

pub struct MdFile {
    /// Path relative to the base root, always with forward slashes.
    pub rel: String,
    /// File name without the `.md`, which is what a `[[wikilink]]` points at.
    pub stem: String,
    pub text: String,
    /// Whether this file is in the base's private layer, by the declaration in
    /// [`private_layer`]. Two states, because there is no longer a way for privacy to
    /// be unknown: it used to be asked of git, which could be absent, and absent was a
    /// third state that refused whole bases. ADR-0034.
    ///
    /// It travels with the file into the index on purpose: the filter used to live
    /// only on the file walk, so `kb index --all` wrote private files into the index
    /// and every later query returned them whether or not `--all` was passed. The
    /// index has to be able to answer who is allowed to see a row.
    pub private: bool,
}

/// What a base declares private, read from its manifest.
#[derive(Debug, Clone, PartialEq)]
pub enum PrivateLayer {
    /// Every file. The person's base, ADR-0025, and any base saying `private = .`.
    Whole,
    /// These folders, relative to the base root, with the folder map as the default.
    Folders(Vec<String>),
}

/// The folder map's private half, which is the declaration a base has when it makes
/// none. `profile/`, `projects/` and `records/` describe a real person and real work.
/// `inbox/` is deliberately not here: it is the short memory, served and labelled as
/// such rather than hidden, because a fact the base has not judged yet is still a fact
/// the owner's agents should be able to reach. ADR-0034.
pub const PRIVATE_DEFAULT: &[&str] = &["profile", "projects", "records"];

/// The manifest key. One line, folders separated by commas, a trailing slash optional,
/// and `.` for the whole base.
const PRIVATE_KEY: &str = "private";

/// The base directory that is private as a whole by name, because that is the name
/// `kb init --person` writes and the person's base carries no manifest of its own.
const PERSON_DIR: &str = "person";

/// Reads the private layer off `agent.txt`, or answers with the default.
///
/// **This replaces `git ls-files`, and the difference is who is asked.** Git answered
/// "is this file tracked", which is a question about publication, and the answer was
/// read as "may this be served", which is a different question with the same shape.
/// The two agreed only while a repository existed, was committed, and was shipped with
/// the base. A deployment bundle, a fresh folder, and a note written a minute ago all
/// broke the agreement, each in the direction of serving nothing. A declaration read
/// off disk cannot be absent, cannot be stale, and costs a file read instead of a
/// subprocess per base per question.
pub fn private_layer(root: &Path) -> PrivateLayer {
    let manifest = fs::read_to_string(root.join("agent.txt")).unwrap_or_default();
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() != PRIVATE_KEY {
            continue;
        }
        let value = value.trim();
        if value == "." {
            return PrivateLayer::Whole;
        }
        let folders: Vec<String> = value
            .split(',')
            .map(|f| f.trim().trim_end_matches('/').to_string())
            .filter(|f| !f.is_empty())
            .collect();
        return PrivateLayer::Folders(folders);
    }

    let is_person = root
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case(PERSON_DIR))
        .unwrap_or(false);
    if is_person {
        return PrivateLayer::Whole;
    }
    PrivateLayer::Folders(PRIVATE_DEFAULT.iter().map(|s| s.to_string()).collect())
}

impl PrivateLayer {
    /// Whether a base relative path, slash separated, is inside the private layer.
    pub fn covers(&self, rel: &str) -> bool {
        match self {
            PrivateLayer::Whole => true,
            PrivateLayer::Folders(folders) => {
                folders.iter().any(|f| rel == f || rel.starts_with(&format!("{f}/")))
            }
        }
    }
}

pub struct Base {
    pub root: PathBuf,
    /// Relative path of the map file, if one was found.
    pub map: Option<String>,
    /// Name of the knowledge folder, if one was found.
    pub knowledge_dir: Option<String>,
    pub files: Vec<MdFile>,
    /// Files that could not be read, with the reason. Reported rather than
    /// swallowed: a file skipped in silence is a check that silently did not run.
    pub unreadable: Vec<(String, String)>,
    /// `alias -> canonical` pairs read from `kb-aliases.txt` at the root.
    pub aliases: Vec<(String, String)>,
}

/// Reads the alias table, if the base has one.
///
/// Format is one `alias = canonical` per line, `#` starts a comment. Deliberately
/// not a config format: the file is edited by whoever just watched a real question
/// miss, and a parser they have to look up is a parser they will not use.
fn load_aliases(root: &Path) -> Vec<(String, String)> {
    let text = match fs::read_to_string(root.join("kb-aliases.txt")) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_once('='))
        .map(|(alias, canonical)| (alias.trim().to_string(), canonical.trim().to_string()))
        .filter(|(a, c)| !a.is_empty() && !c.is_empty())
        .collect()
}

/// True when the directory holds a root map file, which is what makes it a base.
///
/// Matched case sensitively against the names actually on disk, for the same reason
/// `discover` does: Windows answers yes to `INDEX.md` when the file is really
/// `index.md`, which once made Yaron's operating instructions look like its map and
/// silently skipped every map check.
pub fn has_map(dir: &Path) -> bool {
    let names: Vec<String> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => return false,
    };
    MAP_NAMES.iter().any(|m| names.iter().any(|n| n == m))
}

impl Base {
    /// Discovers a base. Unless `all` is set, the private layer is left out: it
    /// describes a real person and real work, it is nobody's to serve to a stranger,
    /// and linting it drowns the findings that matter in noise from files we would
    /// never edit anyway. Which files that is comes from [`private_layer`], read off
    /// the base itself. Nothing outside the directory is consulted.
    pub fn discover(root: &Path, all: bool) -> io::Result<Base> {
        let mut base = Base {
            root: root.to_path_buf(),
            map: None,
            knowledge_dir: None,
            files: Vec::new(),
            unreadable: Vec::new(),
            aliases: load_aliases(root),
        };

        collect(root, root, &mut base)?;
        base.files.sort_by(|a, b| a.rel.cmp(&b.rel));

        // Mark every file, whether or not the list is about to be narrowed. Marking
        // only when filtering would leave `--all` runs with no record of which files
        // were private, which is exactly how the index came to hold private chunks
        // that nothing downstream could recognise as private.
        let layer = private_layer(root);
        for f in &mut base.files {
            f.private = layer.covers(&f.rel);
        }
        if !all {
            base.files.retain(|f| !f.private);
        }

        // Match against the names actually on disk, case sensitively, rather than
        // asking the filesystem whether a path exists. Windows answers yes to
        // `INDEX.md` when the file is really `index.md`, which made Yaron's
        // operating instructions look like its map, and then the map lookup
        // failed and every map check was skipped without a word.
        base.map = MAP_NAMES.iter().find_map(|name| {
            base.files
                .iter()
                .find(|f| f.rel == **name)
                .map(|f| f.rel.clone())
        });

        base.knowledge_dir = KNOWLEDGE_DIRS
            .iter()
            .find(|dir| root.join(dir).is_dir())
            .map(|dir| dir.to_string());

        Ok(base)
    }

    pub fn map_file(&self) -> Option<&MdFile> {
        let name = self.map.as_ref()?;
        self.files.iter().find(|f| &f.rel == name)
    }

    /// True when the file sits inside the knowledge folder.
    pub fn is_note(&self, file: &MdFile) -> bool {
        match &self.knowledge_dir {
            Some(dir) => file.rel.starts_with(&format!("{dir}/")),
            None => false,
        }
    }
}


/// Spawns a child process without letting Windows open a console window for it.
///
/// A GUI process has no console, so Windows creates one for any console child it
/// spawns. That is why the tray flashed a terminal on every fleet open, three times,
/// once per `git ls-files` back when discovery asked git, and why one of them surfaced
/// as ERROR_BROKEN_PIPE (0x800700e8) when the window it created went away underneath
/// the pipe. Discovery no longer spawns anything (ADR-0034); `kb commit` still does.
///
/// `CREATE_NO_WINDOW` is 0x08000000. Named rather than imported so this file keeps
/// its zero dependency property, and behind cfg so nothing changes elsewhere.
pub fn quiet(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

fn collect(root: &Path, dir: &Path, base: &mut Base) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            collect(root, &path, base)?;
            continue;
        }

        if !name.to_lowercase().ends_with(".md") {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        match fs::read_to_string(&path) {
            Ok(text) => {
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                base.files.push(MdFile { rel, stem, text, private: false });
            }
            Err(e) => base.unreadable.push((rel, e.to_string())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// The bug this guards: on a case insensitive filesystem, asking whether
    /// `INDEX.md` exists returns true for a file called `index.md`, so Yaron's
    /// operating instructions were picked as its map and MAPA.md was ignored.
    #[test]
    fn lowercase_index_is_not_the_map() {
        let dir = scratch("yaron-shape");
        fs::write(dir.join("index.md"), "# operating instructions").unwrap();
        fs::write(dir.join("MAPA.md"), "# map").unwrap();
        fs::create_dir_all(dir.join("conhecimento")).unwrap();

        let base = Base::discover(&dir, true).expect("discover");

        assert_eq!(base.map.as_deref(), Some("MAPA.md"));
        assert!(base.map_file().is_some(), "the map file must be resolvable");
        assert_eq!(base.knowledge_dir.as_deref(), Some("conhecimento"));
    }

    #[test]
    fn a_declared_map_always_resolves_to_a_collected_file() {
        let dir = scratch("zed-shape");
        fs::write(dir.join("MAP.md"), "# map").unwrap();
        fs::write(dir.join("index.md"), "# instructions").unwrap();
        fs::create_dir_all(dir.join("knowledge")).unwrap();

        let base = Base::discover(&dir, true).expect("discover");

        assert_eq!(base.map.as_deref(), Some("MAP.md"));
        assert!(base.map_file().is_some());
    }

    // -----------------------------------------------------------------------
    // ADR-0034: the private layer is declared, not asked of git
    // -----------------------------------------------------------------------

    fn note(dir: &Path, rel: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "# note

**Search for:** `x`
").unwrap();
    }

    /// The first interaction with a memory layer: a folder, a note, a question. It
    /// used to be refused until somebody ran `git init`, which for a database is the
    /// equivalent of refusing to open until the user has set up a backup.
    #[test]
    fn a_folder_with_a_note_is_a_base_and_needs_no_repository() {
        let dir = scratch("no-repo");
        note(&dir, "knowledge/a.md");
        assert!(!dir.join(".git").exists(), "the fixture has no repository on purpose");

        let public = Base::discover(&dir, false).expect("discover");
        assert_eq!(public.files.len(), 1, "the note is served without asking anybody");
        assert!(!public.files[0].private);
    }

    /// The folder map is the default. A base that declares nothing behaves exactly as
    /// every base already does, so no existing fleet changes behaviour on upgrade.
    #[test]
    fn the_folder_map_is_the_private_layer_when_nothing_is_declared() {
        let dir = scratch("default-private");
        for rel in [
            "knowledge/public.md",
            "inbox/fresh.md",
            "profile/me.md",
            "projects/client.md",
            "records/sessions/today.md",
        ] {
            note(&dir, rel);
        }

        let public = Base::discover(&dir, false).expect("discover");
        let served: Vec<&str> = public.files.iter().map(|f| f.rel.as_str()).collect();
        assert!(served.contains(&"knowledge/public.md"), "{served:?}");
        assert!(
            served.contains(&"inbox/fresh.md"),
            "the short memory is served, labelled, not hidden: {served:?}"
        );
        for hidden in ["profile/me.md", "projects/client.md", "records/sessions/today.md"] {
            assert!(!served.contains(&hidden), "{hidden} is the private layer: {served:?}");
        }

        let all = Base::discover(&dir, true).expect("discover");
        assert_eq!(all.files.len(), 5, "--all is the deliberate act that includes it");
        assert!(all.files.iter().any(|f| f.rel == "profile/me.md" && f.private));
        assert!(all.files.iter().any(|f| f.rel == "knowledge/public.md" && !f.private));
    }

    /// One line in the manifest replaces the default, and it is a replacement rather
    /// than an addition: a base that wants `records/` served says so by leaving it out.
    #[test]
    fn a_declared_private_layer_replaces_the_default() {
        let dir = scratch("declared-private");
        fs::write(dir.join("agent.txt"), "name = Probe
private = drafts/, records
").unwrap();
        for rel in ["knowledge/a.md", "drafts/b.md", "records/c.md", "profile/d.md"] {
            note(&dir, rel);
        }

        let public = Base::discover(&dir, false).expect("discover");
        let served: Vec<&str> = public.files.iter().map(|f| f.rel.as_str()).collect();
        assert!(served.contains(&"knowledge/a.md"));
        assert!(served.contains(&"profile/d.md"), "not declared, so served: {served:?}");
        assert!(!served.contains(&"drafts/b.md"), "{served:?}");
        assert!(!served.contains(&"records/c.md"), "with or without the slash: {served:?}");
    }

    /// The person is one base and every word of it is private, ADR-0025. The base
    /// `kb init --person` writes is called `person`, and that name is the declaration.
    #[test]
    fn the_person_base_is_private_as_a_whole() {
        let root = scratch("person-private");
        let dir = root.join("person");
        note(&dir, "core.md");
        note(&dir, "work.md");

        let public = Base::discover(&dir, false).expect("discover");
        assert!(public.files.is_empty(), "nothing of the person is served without --all");

        let all = Base::discover(&dir, true).expect("discover");
        assert_eq!(all.files.len(), 2);
        assert!(all.files.iter().all(|f| f.private));
    }

    /// `private = .` is the explicit spelling of the same thing, for a base with a
    /// name of its own.
    #[test]
    fn a_dot_declares_the_whole_base_private() {
        let dir = scratch("dot-private");
        fs::write(dir.join("agent.txt"), "name = Vault
private = .
").unwrap();
        note(&dir, "knowledge/a.md");

        assert!(Base::discover(&dir, false).expect("discover").files.is_empty());
        assert_eq!(Base::discover(&dir, true).expect("discover").files.len(), 1);
    }

    #[test]
    fn skips_build_and_dot_directories() {
        let dir = scratch("skips");
        fs::create_dir_all(dir.join("target/debug")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join("target/debug/note.md"), "x").unwrap();
        fs::write(dir.join(".git/note.md"), "x").unwrap();
        fs::write(dir.join("real.md"), "x").unwrap();

        let base = Base::discover(&dir, true).expect("discover");

        assert_eq!(base.files.len(), 1);
        assert_eq!(base.files[0].rel, "real.md");
    }
}
