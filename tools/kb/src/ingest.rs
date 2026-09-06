//! Taking a document off somebody's disk and leaving knowledge behind instead.
//!
//! ## What this exists to fix
//!
//! The fleet had an ingestion procedure and no ingestion code. `source-ingestion.md` is
//! seven steps, an evidence ladder, a discard rule and an expiry rule, and it lived in two
//! of thirteen agents' private bases. Measured on 2026-09-05: eleven of thirteen agents had
//! an empty `protocols/` directory, including the one that owned the document being filed.
//! So the agent asked to ingest something had, in most cases, no procedure at all, and the
//! two that did held it as prose.
//!
//! That is the failure ADR-0022 named when it moved identity out of prose: an instruction
//! that has to be followed a thousand times gets followed nine hundred. The em dash rule
//! moved into `kb check`. The commit rule moved into `kb commit` after prose failed to
//! prevent `cdc0e52`. This is the ingestion rule making the same move.
//!
//! ## What is here and what is deliberately not
//!
//! **The filing is here. The judgement is not.** Distillation stays in
//! [`crate::promote`], entered through the same [`crate::promote::run`] the nightly sweep
//! calls, with the same promoter prompt and the same three lenses. A CLI verb is an entry
//! point, not an architectural seam: ADR-0030's independence is about which data reaches
//! which model call, and those boundaries are function signatures. Two implementations of
//! one rule drift, and this repository has twice paid for that.
//!
//! ## The order, which is the safety property
//!
//! 1. **Move the original into the deposit.** First act, before extraction, before any
//!    model call. After it, exactly one copy exists and it sits at a path this code built.
//! 2. Extract, promote, decide.
//! 3. **Delete, only if four conditions hold.**
//!
//! The move is the reversible half and the delete is the irreversible one, and the whole
//! promotion runs between them. If anything fails anywhere in that gap the document is
//! intact, it has only moved, and the message says where it is. **No code path here passes
//! a caller-supplied path to a delete**, which is a stronger property than any precondition
//! could be, because it holds even when the precondition is wrong.

use std::path::{Path, PathBuf};

/// Where a moved original waits while its extraction is promoted.
///
/// It is a subfolder of the deposit and [`crate::promote::deposit_files`] skips it, which
/// is the one exemption in that walk and needs its reason stated. A PDF is not a deposit:
/// nothing can read it, and offering it to a promoter spends a model call to be told so.
/// The extracted text sitting beside it in `inbox/` is the deposit. This is not the
/// `processed/` skip that the 2026-09-05 ruling abolished, which protected material after
/// it had been absorbed; this holds material that has not been read yet.
pub const RAW: &str = "raw";

/// A document that has been moved into a base and turned into something readable.
#[derive(Debug)]
pub struct Staged {
    /// The original, now inside `inbox/raw/`. Never the path the caller typed.
    pub original: PathBuf,
    /// The extracted text, in `inbox/`, which is what promotion reads.
    pub deposit: PathBuf,
    pub agent: String,
}

#[derive(Debug)]
pub enum IngestError {
    NoSuchFile(PathBuf),
    NoAgent(String),
    Empty(PathBuf),
    Io(PathBuf, std::io::Error),
    /// The move left the original where it was. Named separately because the fix is
    /// different: everything else means try again, this means the document is untouched.
    CannotStage(PathBuf, String),
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::NoSuchFile(p) => write!(f, "there is no file at {}", p.display()),
            IngestError::NoAgent(a) => write!(
                f,
                "no agent called '{a}'. `kb fleet` lists the agents that exist."
            ),
            IngestError::Empty(p) => write!(
                f,
                "{} produced no text, so there is nothing to distil and the original is \
                 still the only copy of whatever it holds. It has not been moved.",
                p.display()
            ),
            IngestError::Io(p, e) => write!(f, "cannot read or write {}: {e}", p.display()),
            IngestError::CannotStage(p, why) => write!(
                f,
                "could not move {} into the deposit: {why}. Nothing was changed and the \
                 document is where it was.",
                p.display()
            ),
        }
    }
}

/// Moves the document into the agent's deposit and writes its text beside it.
///
/// The move happens first and the extraction second, so a failure to extract leaves a
/// document safely inside the base rather than half gone from outside it.
pub fn stage(
    agent_root: &Path,
    agent: &str,
    source: &Path,
    text: &str,
    today: &str,
) -> Result<Staged, IngestError> {
    if !source.is_file() {
        return Err(IngestError::NoSuchFile(source.to_path_buf()));
    }
    if text.trim().is_empty() {
        return Err(IngestError::Empty(source.to_path_buf()));
    }

    let stem = slug_of(source);
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "bin".into());

    let inbox = agent_root.join(crate::promote::DEPOSIT);
    let raw_dir = inbox.join(RAW);
    std::fs::create_dir_all(&raw_dir).map_err(|e| IngestError::Io(raw_dir.clone(), e))?;

    let original = unique(&raw_dir, &format!("{today}-{stem}"), &ext);
    move_file(source, &original)
        .map_err(|e| IngestError::CannotStage(source.to_path_buf(), e))?;

    // `staged_from` names the path the caller handed over, which is the only durable way
    // to say which document this was. The deposit path is an implementation detail that
    // is about to be deleted; `C:\...\teorico.pdf` is what a person recognises.
    //
    // It is also the field the delete keys off. A deposit written by `kb capture` carries
    // no `staged_from`, so a session's conversational residue is **structurally** ineligible
    // for deletion rather than protected by a folder rule somebody could get wrong.
    let deposit = unique(&inbox, &format!("{today}-{stem}"), "txt");
    let body = format!(
        "---\nprovenance: external\nstage: raw\nstaged_from: {}\nstaged_raw: {}\n---\n\n{}\n",
        source.display(),
        original.display(),
        text.trim()
    );
    if let Err(e) = std::fs::write(&deposit, body) {
        // The extraction could not be written, so put the document back where it came
        // from rather than leaving it filed somewhere the caller never chose.
        let _ = move_file(&original, source);
        return Err(IngestError::Io(deposit, e));
    }

    Ok(Staged { original, deposit, agent: agent.to_string() })
}

/// Why a staged source was not deleted, in the caller's words rather than a code.
#[derive(Debug)]
pub enum Kept {
    /// Promotion did not finish over this file: a model was unreachable, or `--max` bit.
    Incomplete(String),
    /// Nothing on disk names this deposit, so nothing proves it was absorbed.
    NotAbsorbed { proposals: usize, refused: usize },
    /// The caller asked for the source to stay.
    Asked,
}

impl std::fmt::Display for Kept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kept::Incomplete(why) => write!(
                f,
                "promotion did not run to completion over this document ({why}), so what \
                 it holds has not all been offered yet"
            ),
            Kept::NotAbsorbed { proposals, refused } => write!(
                f,
                "this run wrote no note naming this document, or no note on disk names it. \
                 {proposals} proposal(s) were made and {refused} refused; the reasons are \
                 in kb-rejections.txt. A source that produced no note has not been \
                 absorbed, and destroying the only copy of material the base decided it \
                 had nothing to say about is the worst trade available"
            ),
            Kept::Asked => write!(f, "--keep was given"),
        }
    }
}

/// Whether the staged source may be destroyed, and why not when it may not.
///
/// **Gated on absorption, not on quality.** A note with a broken wikilink is still
/// absorbed knowledge, and letting a stray defect preserve a 5 MB PDF forever would make
/// the gate meaningless inside a week. Quality is enforced upstream, at the write, where
/// [`crate::write::note`] refuses a dead key or an em dash before anything lands.
///
/// ## Two conditions, and the second one was added after a review found the hole
///
/// The proof used to be the disk read alone, on the argument that a run which believes it
/// wrote a note and did not is exactly the case where the difference matters. That
/// argument is still right and it was not enough, because **the deposit name is not an
/// identity.** It is `<agent>/inbox/<today>-<slug>.txt`, [`unique`] picks it on
/// `!exists()`, and [`destroy`] unlinks it, so the name is free again the moment the first
/// document is destroyed. Two documents whose stems slug the same, ingested into the same
/// base on the same day, therefore get the same string, and the second one inherits the
/// first one's note as its proof.
///
/// The realistic path to that, named in ADR-0001's own revisit trigger: the duplication
/// lens refuses every proposal about material the base already absorbed, so the second
/// document is exactly the one that writes nothing. It would have been deleted on a proof
/// belonging to another document, having been unanimously rejected, and the caller would
/// have seen exit 0.
///
/// So a deletion now needs both: **this run wrote a note**, which is what ties the proof to
/// this document, and **a note on disk names the deposit**, which is what catches a run
/// that only believed it wrote one. Neither alone is sufficient and the two fail in
/// opposite directions, which is why both are here rather than one replacing the other.
///
/// The name collision itself is not fixed here. It remains an audit ambiguity: two
/// documents can leave notes carrying one `captured_from` string, and afterwards nothing
/// says which note came from which. That wants the deposit name to carry something
/// derived from the content, and it is a change to every deposit filename rather than to
/// one condition.
pub fn may_delete(
    fleet_root: &Path,
    deposit_name: &str,
    outcome: &crate::promote::Outcome,
) -> Result<(), Kept> {
    if let Some(cap) = outcome.stopped_at {
        return Err(Kept::Incomplete(format!("it stopped at the --max cap of {cap}")));
    }
    if let Some(first) = outcome.unreachable.first() {
        return Err(Kept::Incomplete(first.clone()));
    }

    let this_run = outcome.written();
    let on_disk = notes_captured_from(fleet_root, deposit_name);
    if this_run == 0 || on_disk == 0 {
        let refused = outcome.decided.iter().filter(|d| !d.accepted()).count();
        return Err(Kept::NotAbsorbed { proposals: outcome.decided.len(), refused });
    }
    Ok(())
}

/// How many notes on disk declare this deposit as what they were captured from.
///
/// **This is the proof the whole deletion stands on**, and it is a fact read back off
/// disk rather than a count the run kept in memory. A run that believes it wrote a note
/// and did not is exactly the case where the difference matters.
pub fn notes_captured_from(fleet_root: &Path, deposit_name: &str) -> usize {
    let mut found = 0;
    let agents = fleet_root.join("fleet");
    let roots = if agents.is_dir() { agents } else { fleet_root.to_path_buf() };
    let Ok(entries) = std::fs::read_dir(&roots) else { return 0 };
    for base in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        count_in(&base, deposit_name, &mut found);
    }
    found
}

fn count_in(dir: &Path, deposit_name: &str, found: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // The deposit itself is skipped: the staged file names itself in its own
            // `staged_from` line, and counting it would let a document prove its own
            // absorption without a single note having been written.
            if path.file_name().is_some_and(|n| n == crate::promote::DEPOSIT) {
                continue;
            }
            count_in(&path, deposit_name, found);
        } else if path.extension().is_some_and(|x| x == "md") {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Some(pairs) = crate::checks::front_matter(&text) else { continue };
            if pairs
                .iter()
                .any(|(k, v)| k == "captured_from" && v.trim() == deposit_name)
            {
                *found += 1;
            }
        }
    }
}

/// Destroys the staged document and its extraction. Only ever called on paths this
/// module built, and only after [`may_delete`] said yes.
pub fn destroy(staged: &Staged) -> Vec<PathBuf> {
    let mut gone = Vec::new();
    for path in [&staged.deposit, &staged.original] {
        if std::fs::remove_file(path).is_ok() {
            gone.push(path.clone());
        }
    }
    gone
}

/// A file name safe to build a path from, out of whatever the document was called.
fn slug_of(source: &Path) -> String {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "document".into());
    let mut out = String::new();
    let mut last_dash = true;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "document".into() } else { trimmed.chars().take(60).collect() }
}

/// A path in `dir` that nothing occupies, by counting up rather than overwriting.
fn unique(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = dir.join(format!("{stem}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-{}.{ext}", std::process::id()))
}

/// Rename where possible, copy and unlink where not.
///
/// `fs::rename` fails across volumes, and a document in `Downloads` on one drive being
/// filed into a repository on another is the ordinary case rather than the exotic one.
/// **The unlink runs only after the copy is verified by size**, so an interrupted copy
/// leaves the original where it was rather than leaving nothing anywhere.
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    let bytes = std::fs::copy(from, to).map_err(|e| e.to_string())?;
    let expected = std::fs::metadata(from).map_err(|e| e.to_string())?.len();
    if bytes != expected {
        let _ = std::fs::remove_file(to);
        return Err(format!("copied {bytes} bytes of {expected}"));
    }
    std::fs::remove_file(from).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kb-ingest-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    /// The move is first, so a document is inside the base before anything can fail.
    #[test]
    fn staging_moves_the_original_and_leaves_the_caller_s_path_empty() {
        let dir = scratch("stage");
        let agent = dir.join("fleet/gti");
        std::fs::create_dir_all(&agent).expect("agent");
        let source = dir.join("teorico.pdf");
        std::fs::write(&source, "not really a pdf").expect("source");

        let staged = stage(&agent, "gti", &source, "the extracted text", "2026-09-05")
            .expect("stages");

        assert!(!source.exists(), "the caller's copy is gone, because it moved");
        assert!(staged.original.exists(), "and it is in the deposit's raw folder");
        assert!(
            staged.original.to_string_lossy().contains("raw"),
            "raw originals are not deposits: {:?}",
            staged.original
        );

        let text = std::fs::read_to_string(&staged.deposit).expect("deposit");
        assert!(text.contains("staged_from:"), "the document it came from is named: {text}");
        assert!(text.contains("the extracted text"));
        assert!(text.starts_with("---\nprovenance: external\nstage: raw\n"), "{text}");
    }

    /// An extraction that produced nothing must not move anything.
    #[test]
    fn an_empty_extraction_leaves_the_document_untouched() {
        let dir = scratch("empty");
        let agent = dir.join("fleet/gti");
        std::fs::create_dir_all(&agent).expect("agent");
        let source = dir.join("scan.pdf");
        std::fs::write(&source, "an image with no text layer").expect("source");

        let err = stage(&agent, "gti", &source, "   \n  ", "2026-09-05")
            .expect_err("nothing to distil");
        assert!(matches!(err, IngestError::Empty(_)), "{err}");
        assert!(source.exists(), "the only copy of the content stays where the caller put it");
    }

    /// The proof a deletion stands on is read off disk, and the staged file cannot
    /// vouch for itself.
    #[test]
    fn only_a_written_note_counts_as_proof_that_a_document_was_absorbed() {
        let dir = scratch("proof");
        let knowledge = dir.join("fleet/gti/knowledge");
        let inbox = dir.join("fleet/gti/inbox");
        std::fs::create_dir_all(&knowledge).expect("knowledge");
        std::fs::create_dir_all(&inbox).expect("inbox");

        // The staged deposit names itself, and must not be counted.
        std::fs::write(
            inbox.join("2026-09-05-teorico.txt"),
            "---\nprovenance: external\nstage: raw\ncaptured_from: gti/inbox/2026-09-05-teorico.txt\n---\n\ntext\n",
        )
        .expect("deposit");
        assert_eq!(
            notes_captured_from(&dir, "gti/inbox/2026-09-05-teorico.txt"),
            0,
            "a document cannot prove its own absorption"
        );

        std::fs::write(
            knowledge.join("itil.md"),
            "---\nprovenance: agent\nstage: captured\ncaptured_from: gti/inbox/2026-09-05-teorico.txt\n---\n\n# ITIL\n",
        )
        .expect("note");
        assert_eq!(notes_captured_from(&dir, "gti/inbox/2026-09-05-teorico.txt"), 1);
        assert_eq!(notes_captured_from(&dir, "gti/inbox/something-else.txt"), 0);
    }

    /// A second document must not inherit the first one's proof of absorption.
    ///
    /// The deposit name is `<agent>/inbox/<today>-<slug>.txt` and nothing in it identifies
    /// the document, so a second file whose stem slugs the same, ingested into the same
    /// base on the same day after the first was destroyed, gets the identical string back
    /// from `unique`. Before the run condition was added, `may_delete` said yes on the
    /// strength of a note written about a different document, and the second document was
    /// destroyed having produced nothing.
    #[test]
    fn a_second_document_cannot_inherit_the_first_ones_proof() {
        let dir = scratch("inherit");
        let knowledge = dir.join("fleet/gti/knowledge");
        let inbox = dir.join("fleet/gti/inbox");
        std::fs::create_dir_all(&knowledge).expect("knowledge");
        std::fs::create_dir_all(&inbox).expect("inbox");

        let name = "gti/inbox/2026-09-05-teorico.txt";
        std::fs::write(
            knowledge.join("itil.md"),
            format!("---\nprovenance: agent\nstage: captured\ncaptured_from: {name}\n---\n\n# ITIL\n"),
        )
        .expect("note from the first document");

        // The name really is free again, which is the half that makes the collision reachable.
        assert_eq!(
            unique(&inbox, "2026-09-05-teorico", "txt"),
            inbox.join("2026-09-05-teorico.txt"),
            "a destroyed deposit frees its own name"
        );

        // A run that wrote nothing: every proposal refused, or none made at all.
        let barren = crate::promote::Outcome::default();
        assert_eq!(barren.written(), 0);
        assert_eq!(
            notes_captured_from(&dir, name),
            1,
            "and a note on disk does name that string, from the other document"
        );

        let verdict = may_delete(&dir, name, &barren);
        assert!(
            matches!(verdict, Err(Kept::NotAbsorbed { .. })),
            "a run that wrote nothing must never delete, whatever the disk says: {verdict:?}"
        );
    }

    #[test]
    fn a_name_becomes_a_path_safe_slug() {
        assert_eq!(slug_of(Path::new("/tmp/Gestão da T.I. (2012).pdf")), "gest-o-da-t-i-2012");
        assert_eq!(slug_of(Path::new("/tmp/teorico.pdf")), "teorico");
        assert_eq!(slug_of(Path::new("/tmp/___.pdf")), "document");
    }
}
