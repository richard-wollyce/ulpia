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

use std::collections::HashSet;
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
    /// Whether git tracks this file. `None` means git could not be asked, which is
    /// a third state and not a synonym for `false`.
    ///
    /// This exists because the tracked filter used to live only on the file walk,
    /// so `kb index --all` wrote private files into the index and every later query
    /// returned them whether or not `--all` was passed. The flag has to travel with
    /// the file into the index, or the index cannot answer who is allowed to see it.
    pub tracked: Option<bool>,
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
    /// True when the file list was narrowed to what git tracks.
    pub tracked_only: bool,
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
    /// Discovers a base. Unless `all` is set, only files tracked by git are
    /// considered: the private layer is gitignored by design, it is nobody's to
    /// publish, and linting it drowns the findings that matter in noise from
    /// files we would never edit anyway.
    pub fn discover(root: &Path, all: bool) -> io::Result<Base> {
        let mut base = Base {
            root: root.to_path_buf(),
            map: None,
            knowledge_dir: None,
            files: Vec::new(),
            unreadable: Vec::new(),
            tracked_only: false,
            aliases: load_aliases(root),
        };

        collect(root, root, &mut base)?;
        base.files.sort_by(|a, b| a.rel.cmp(&b.rel));

        // Ask git once, and mark every file, whether or not the list is about to be
        // narrowed. Marking only when filtering would leave `--all` runs with no
        // record of which files were private, which is exactly how the index came to
        // hold private chunks that nothing downstream could recognise as private.
        let tracked = tracked_files(root);
        if let Some(set) = &tracked {
            for f in &mut base.files {
                f.tracked = Some(set.contains(&f.rel));
            }
        }

        if !all {
            if tracked.is_some() {
                base.files.retain(|f| f.tracked == Some(true));
                base.tracked_only = true;
            }
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
/// once per `git ls-files`, and why one of them surfaced as ERROR_BROKEN_PIPE
/// (0x800700e8) when the window it created went away underneath the pipe.
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

/// The set of paths git tracks in this base, relative and slash separated.
///
/// Returns None when git is missing or the directory is not a repository, in
/// which case the caller keeps every file and says so in the report. A filter
/// that quietly does nothing is worse than no filter.
fn tracked_files(root: &Path) -> Option<HashSet<String>> {
    let output = quiet("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    let set: HashSet<String> = listing
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if set.is_empty() { None } else { Some(set) }
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
                base.files.push(MdFile { rel, stem, text, tracked: None });
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
