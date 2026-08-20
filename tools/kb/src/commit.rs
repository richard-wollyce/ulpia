//! Committing from one session while other sessions are writing the same tree.
//!
//! **The failure this exists to prevent has already happened here.** Commit `cdc0e52`
//! in the private repository carries one session's new agent and another session's
//! unrelated backlog edit, because the first session staged with `git add -A` while the
//! second was mid write. Nothing was lost, and that is the trap: the damage is not lost
//! work, it is a commit message that describes half of what the commit contains. Six
//! months later the audit trail is a liar and nobody can tell which half.
//!
//! ## The primitive, verified rather than remembered
//!
//! `git commit -- <paths>` builds the commit from **only** those paths and ignores the
//! rest of the index entirely. Tested on 2026-08-18: with another session's file staged
//! in the same repository, a pathspec commit took only its own file and left the other
//! staged and uncommitted.
//!
//! That is the whole concurrency story, and it means **no lock is needed for the case
//! that actually bites.** The dangerous window in `git add` then `git commit` is that a
//! plain `git commit` takes the entire index, including whatever another session staged
//! in between. A pathspec on the commit closes the window instead of guarding it.
//!
//! Two more behaviours were measured because designing against a remembered API is how
//! this goes wrong:
//!
//! - An **untracked** path fails a pathspec commit with `pathspec did not match any
//!   file(s) known to git`, so `git add -- <paths>` has to run first. That add is
//!   path scoped too, so it is safe for the same reason.
//! - A **deleted** path commits fine through a pathspec, no special case needed.
//!
//! ## What is left, and it is honest about it
//!
//! Git's own `index.lock` serialises the two invocations, and loses with exit 128 and
//! `Unable to create ... index.lock` when contended. That is a retry, not a design
//! problem, and it is bounded here rather than retried forever.
//!
//! **This does not stop two sessions editing the same file.** No git technique does:
//! that race is at the filesystem, before git sees anything. What it does is guarantee
//! that whatever you commit is what you named, and that nothing else moved because of
//! you. See ADR-0021 for why a claim or lease layer was not built for that.

use std::path::{Path, PathBuf};

use crate::base::quiet;

/// How many times to wait out another git process holding the index.
///
/// Bounded, because an unbounded retry against a lock that is never released is a hang
/// with no error message, which is worse than the error. Six attempts with the backoff
/// below waits about 3.1 seconds, which covers a concurrent commit and gives up well
/// before a human wonders whether the tool is broken.
const LOCK_ATTEMPTS: u32 = 6;

/// Doubling from 100ms: 100, 200, 400, 800, 1600.
const LOCK_BACKOFF_MS: u64 = 100;

#[derive(Debug)]
pub struct Committed {
    pub sha: String,
    /// What actually landed, read back off the commit rather than assumed.
    pub files: Vec<String>,
    /// Paths that were dirty before and are still dirty after: other sessions' work,
    /// demonstrably untouched. Reported because "I did not take your files" is a claim
    /// that should come with its evidence.
    pub left_alone: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    NoPaths,
    NoMessage,
    NotARepository(PathBuf),
    /// Paths resolving into different repositories. Real here: `fleet/` is a separate
    /// repository nested inside the public one, so a careless path list spans both and
    /// half of it silently does nothing.
    SpansRepositories(PathBuf, PathBuf),
    NothingToCommit,
    Locked,
    Git(String),
    /// The commit landed and contains something that was not asked for. Reported and
    /// never silently repaired: undoing it means rewriting history, which is a
    /// stop-and-ask, and the person has to know either way.
    Absorbed(Vec<String>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoPaths => write!(
                f,
                "commit needs the paths it should commit. There is deliberately no flag \
                 that means everything: another session's work is usually in this tree"
            ),
            Error::NoMessage => write!(f, "commit needs a message, with -m"),
            Error::NotARepository(p) => write!(f, "{} is not inside a git repository", p.display()),
            Error::SpansRepositories(a, b) => write!(
                f,
                "those paths are in two different repositories, {} and {}. Commit them \
                 separately, so each message describes one repository's change",
                a.display(),
                b.display()
            ),
            Error::NothingToCommit => {
                write!(f, "none of those paths has changes. Nothing was committed")
            }
            Error::Locked => write!(
                f,
                "another git process held the index for longer than the retry window. \
                 It is probably another session committing right now: try again"
            ),
            Error::Git(e) => write!(f, "git refused: {e}"),
            Error::Absorbed(files) => write!(
                f,
                "the commit landed but it contains {} path(s) that were not asked for: \
                 {}. Another session's work was swept in. This is not repaired \
                 automatically because undoing it rewrites history",
                files.len(),
                files.join(", ")
            ),
        }
    }
}

/// The repository a path belongs to, which is not assumed from the working directory.
pub fn repo_root(path: &Path) -> Option<PathBuf> {
    // A path that does not exist yet still has an ancestor that does, and `git -C` needs a
    // directory that is there. Deleted files land here too, which is why this walks up.
    //
    // **It used to walk up exactly one level, and one level is not enough for the case that
    // matters.** Removing a whole generated directory deletes the file's parent along with
    // the file, so `git -C <parent>` failed and `kb commit` refused the deletion with "not
    // inside a git repository". The tool could stage a removal and then not commit it, which
    // made it useless for exactly the work that is mostly removals: cleaning up.
    let mut dir = if path.is_dir() { path.to_path_buf() } else { path.parent()?.to_path_buf() };
    while !dir.as_os_str().is_empty() && !dir.is_dir() {
        dir = dir.parent()?.to_path_buf();
    }
    let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };

    let out = quiet("git").arg("-C").arg(&dir).args(["rev-parse", "--show-toplevel"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() { None } else { Some(PathBuf::from(root)) }
}

/// Every path git considers dirty, as `status -> path`. Untracked included, because an
/// untracked file another session is about to add is exactly the kind of thing a sweep
/// takes.
pub fn dirty(repo: &Path) -> Vec<String> {
    let Ok(out) =
        quiet("git").arg("-C").arg(repo).args(["status", "--porcelain", "-z"]).output()
    else {
        return Vec::new();
    };
    // NUL separated so a path with a space or a quote cannot be misparsed. The porcelain
    // format's quoting rules were a real source of wrong answers before -z.
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|e| e.len() > 3)
        .map(|e| e[3..].to_string())
        .collect()
}

fn run(repo: &Path, args: &[&str], paths: &[String]) -> Result<String, Error> {
    let mut attempt = 0;
    loop {
        let mut cmd = quiet("git");
        // Tells the pre-commit guard this commit came through here and therefore has
        // an explicit pathspec behind it. Set on every git call rather than only the
        // commit, so a hook on another verb sees it too.
        cmd.env("KB_COMMIT", "1");
        cmd.arg("-C").arg(repo).args(args);
        if !paths.is_empty() {
            cmd.arg("--");
            for p in paths {
                cmd.arg(p);
            }
        }
        let out = cmd.output().map_err(|e| Error::Git(e.to_string()))?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }

        let err = String::from_utf8_lossy(&out.stderr).to_string();
        // The exact string git emits under contention, captured by holding the lock and
        // running a commit, not guessed.
        if err.contains("index.lock") && attempt < LOCK_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(
                LOCK_BACKOFF_MS << attempt.min(4),
            ));
            attempt += 1;
            continue;
        }
        if err.contains("index.lock") {
            return Err(Error::Locked);
        }
        return Err(Error::Git(err.trim().to_string()));
    }
}

/// Commits exactly the named paths and proves it afterwards.
///
/// The verification is the point. Any of this could be done by hand, and the step a
/// person skips by hand is reading back what actually landed.
pub fn commit(paths: &[String], message: &str) -> Result<Committed, Error> {
    if paths.is_empty() {
        return Err(Error::NoPaths);
    }
    if message.trim().is_empty() {
        return Err(Error::NoMessage);
    }

    let first = Path::new(&paths[0]);
    let repo = repo_root(first).ok_or_else(|| Error::NotARepository(first.to_path_buf()))?;
    for p in &paths[1..] {
        let other = repo_root(Path::new(p)).ok_or_else(|| Error::NotARepository(p.into()))?;
        if other != repo {
            return Err(Error::SpansRepositories(repo, other));
        }
    }

    // Relative to the repository, so the pathspec means the same thing regardless of
    // where the caller was standing. Absolute paths in a pathspec work, but they also
    // make the failure message unreadable and hide which repository is being addressed.
    let rel: Vec<String> = paths.iter().map(|p| relative_to(&repo, Path::new(p))).collect();

    let before = dirty(&repo);
    if before.is_empty() {
        return Err(Error::NothingToCommit);
    }

    run(&repo, &["add"], &rel)?;

    // Whether any of the named paths actually differs from HEAD. Without this, git
    // reports "nothing to commit" as a failure and the caller cannot tell that from a
    // real error.
    let staged = run(&repo, &["diff", "--cached", "--name-only"], &rel)?;
    if staged.trim().is_empty() {
        return Err(Error::NothingToCommit);
    }

    let mut args = vec!["commit", "-q", "-m", message];
    // Pathspec on the commit, which is the mechanism this whole module rests on.
    args.push("--");
    let mut cmd = quiet("git");
    cmd.env("KB_COMMIT", "1");
    cmd.arg("-C").arg(&repo).args(&args[..args.len() - 1]).arg("--");
    for p in &rel {
        cmd.arg(p);
    }
    let out = cmd.output().map_err(|e| Error::Git(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(if err.contains("index.lock") {
            Error::Locked
        } else {
            Error::Git(err.trim().to_string())
        });
    }

    let sha = run(&repo, &["rev-parse", "--short", "HEAD"], &[])?.trim().to_string();
    let landed: Vec<String> = run(&repo, &["show", "--pretty=", "--name-only", "HEAD"], &[])?
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    // Read back rather than assumed: did anything land that was not asked for?
    let asked: Vec<String> = rel.iter().map(|p| p.trim_end_matches('/').to_string()).collect();
    let absorbed: Vec<String> = landed
        .iter()
        .filter(|f| !asked.iter().any(|a| *f == a || f.starts_with(&format!("{a}/"))))
        .cloned()
        .collect();
    if !absorbed.is_empty() {
        return Err(Error::Absorbed(absorbed));
    }

    let after = dirty(&repo);
    let left_alone: Vec<String> =
        before.iter().filter(|p| after.contains(p)).cloned().collect();

    Ok(Committed { sha, files: landed, left_alone })
}

/// Repository relative, slash separated. Falls back to the path as given when it is
/// already relative and cannot be canonicalised, which happens for a path that was
/// just deleted.
fn relative_to(repo: &Path, path: &Path) -> String {
    let repo_abs = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());

    // **Both sides have to be normalised the same way or neither is.** A path that was
    // just deleted cannot be canonicalised, and on Windows `canonicalize` returns an
    // extended length `\\?\` prefix, so comparing a canonical repository against a
    // merely absolute path fails to strip and the pathspec silently comes out absolute.
    // Canonicalising the parent and rejoining the file name works for a deleted file,
    // because the directory it was in still exists.
    let abs = path.canonicalize().unwrap_or_else(|_| {
        match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => parent
                .canonicalize()
                .map(|p| p.join(name))
                .unwrap_or_else(|_| path.to_path_buf()),
            _ => path.to_path_buf(),
        }
    });

    let rel = abs.strip_prefix(&repo_abs).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {

    /// **Deleting a whole directory deletes the file's parent with it**, and `repo_root`
    /// walked up exactly one level, so `kb commit` refused every such removal with "not
    /// inside a git repository". The tool could stage the deletion and then not commit it.
    #[test]
    fn a_path_whose_whole_parent_directory_is_gone_still_finds_its_repository() {
        let repo = scratch("deep-delete");
        crate::base::quiet("git").arg("-C").arg(&repo).arg("init").arg("-q").output().ok();

        let gone = repo.join("build").join("generated").join("schemas").join("thing.json");
        assert!(!gone.exists(), "the point is that nothing on this path exists");

        let found = repo_root(&gone).expect("walks up to the repository");
        assert_eq!(
            found.canonicalize().ok(),
            repo.canonicalize().ok(),
            "found {found:?} instead of {repo:?}"
        );
    }
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-commit-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn repo(name: &str) -> PathBuf {
        let dir = scratch(name);
        let git = |args: &[&str]| {
            quiet("git").arg("-C").arg(&dir).args(args).output().expect("git");
        };
        git(&["init", "-q", "."]);
        git(&["config", "user.email", "t@example.invalid"]);
        git(&["config", "user.name", "test"]);
        std::fs::write(dir.join("mine.md"), "base\n").expect("write");
        std::fs::write(dir.join("theirs.md"), "base\n").expect("write");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "init"]);
        dir
    }

    fn p(dir: &Path, name: &str) -> String {
        dir.join(name).to_string_lossy().to_string()
    }

    /// The regression this module exists for, as the shape it had in `cdc0e52`: one
    /// session commits while another session's work sits staged in the same index.
    #[test]
    fn another_sessions_staged_work_is_not_swept_into_the_commit() {
        let dir = repo("sweep");
        std::fs::write(dir.join("mine.md"), "my work\n").expect("write");
        std::fs::write(dir.join("theirs.md"), "their work\n").expect("write");
        quiet("git").arg("-C").arg(&dir).args(["add", "theirs.md"]).output().expect("git");

        let out = commit(&[p(&dir, "mine.md")], "only mine").expect("commits");

        assert_eq!(out.files, vec!["mine.md"], "exactly what was named, read back off the commit");
        assert!(
            out.left_alone.iter().any(|f| f == "theirs.md"),
            "and the other session's work is still sitting there dirty"
        );
    }

    /// An untracked file fails a bare pathspec commit, which is why `add` runs first.
    /// Measured against real git, so this test is the guard on that ordering.
    #[test]
    fn a_brand_new_file_commits_even_though_a_pathspec_alone_cannot_see_it() {
        let dir = repo("untracked");
        std::fs::write(dir.join("new.md"), "new\n").expect("write");
        let out = commit(&[p(&dir, "new.md")], "add one").expect("commits");
        assert_eq!(out.files, vec!["new.md"]);
    }

    #[test]
    fn a_deletion_is_a_change_like_any_other() {
        let dir = repo("delete");
        std::fs::remove_file(dir.join("mine.md")).expect("rm");
        let out = commit(&[p(&dir, "mine.md")], "remove one").expect("commits");
        assert_eq!(out.files, vec!["mine.md"]);
    }

    /// There is no flag meaning everything, and an empty list is the one call that
    /// would silently reintroduce the bug.
    #[test]
    fn committing_nothing_in_particular_is_refused() {
        assert!(matches!(commit(&[], "msg"), Err(Error::NoPaths)));
    }

    #[test]
    fn a_message_is_not_optional() {
        let dir = repo("nomsg");
        std::fs::write(dir.join("mine.md"), "x\n").expect("write");
        assert!(matches!(commit(&[p(&dir, "mine.md")], "   "), Err(Error::NoMessage)));
    }

    /// `fleet/` is a separate repository nested inside the public one, so this is not a
    /// hypothetical: a path list spanning both would commit half and silently drop the
    /// rest into the other repository's working tree.
    #[test]
    fn paths_in_two_repositories_are_refused_rather_than_half_committed() {
        let outer = repo("outer");
        let inner = outer.join("nested");
        std::fs::create_dir_all(&inner).expect("mkdir");
        for args in [
            vec!["init", "-q", "."],
            vec!["config", "user.email", "t@example.invalid"],
            vec!["config", "user.name", "test"],
        ] {
            quiet("git").arg("-C").arg(&inner).args(&args).output().expect("git");
        }
        std::fs::write(inner.join("a.md"), "a\n").expect("write");
        std::fs::write(outer.join("mine.md"), "changed\n").expect("write");

        let err = commit(&[p(&outer, "mine.md"), p(&inner, "a.md")], "both").unwrap_err();
        assert!(matches!(err, Error::SpansRepositories(_, _)));
    }

    #[test]
    fn a_clean_path_reports_nothing_to_commit_rather_than_an_empty_commit() {
        let dir = repo("clean");
        std::fs::write(dir.join("theirs.md"), "dirty\n").expect("write");
        assert!(matches!(commit(&[p(&dir, "mine.md")], "msg"), Err(Error::NothingToCommit)));
    }

    /// A directory as a pathspec commits everything under it, and the verification has
    /// to accept those as asked for rather than calling them absorbed.
    #[test]
    fn a_directory_pathspec_commits_what_is_under_it() {
        let dir = repo("dirspec");
        std::fs::create_dir_all(dir.join("notes")).expect("mkdir");
        std::fs::write(dir.join("notes/a.md"), "a\n").expect("write");
        std::fs::write(dir.join("notes/b.md"), "b\n").expect("write");
        std::fs::write(dir.join("theirs.md"), "theirs\n").expect("write");

        let out = commit(&[p(&dir, "notes")], "the notes").expect("commits");
        assert_eq!(out.files.len(), 2, "both files under the directory");
        assert!(out.left_alone.iter().any(|f| f == "theirs.md"));
    }
}
