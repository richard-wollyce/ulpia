//! Writing a note, and the map entry that makes it reachable, as one act.
//!
//! [ADR-0007](../../../decisions/0007-memory-architecture.md) makes every write a
//! proposal a human approves, and `remember.rs` is the proposing half: it measures a
//! claim against the base and returns ADD, UPDATE or NOOP without touching disk.
//! **Nothing turned an approved proposal into a file**, so the base could only grow
//! by hand, and in practice it did not grow.
//!
//! # Why a note and its map entry are one operation
//!
//! A note with no map entry is unreachable. That is not a lint opinion, it is how
//! routing works: the keyword scorer ranks map entries, so a file nobody listed is a
//! file no question can reach, and the full text scorer alone reduces to the single
//! scorer case this system already calls a guess rather than an answer.
//!
//! It is measured, twice, on 2026-08-17. A routing audit of twenty real questions
//! found the largest single cause of failure was vocabulary the map did not carry.
//! A separate audit found 25 alias lines pointing at canonical terms **no map
//! mentions anywhere**, which is the same defect seen from the other end.
//!
//! So `keys` is required and there is no flag to skip it. A tool that can create an
//! unreachable note has handed you a way to grow a base while making it worse, and
//! leaving that to the linter means finding it later instead of preventing it now.
//!
//! # Why this is a command and not an MCP tool
//!
//! [ADR-0010](../../../decisions/0010-memory-as-mcp-server.md) deferred a model
//! reachable write deliberately: a write tool reachable by a model is a different
//! security surface, and it gets built deliberately rather than as an afterthought
//! while the retrieval side is still warm. That still holds. The mechanism belongs
//! here either way, and putting it on MCP is a separate decision that needs its own
//! ADR rather than arriving as a side effect of this one.

use std::path::{Path, PathBuf};

// The linter's lists, not a second pair. Keeping a copy here is what let `captured` be legal
// to `kb check` and illegal to `kb write` at the same time, which made `kb promote` unable to
// write anything it admitted. See the note on those constants.
use crate::checks::{PROVENANCE, STAGE};

/// What a note needs before it is allowed to exist.
pub struct Note {
    /// One line for the map, saying what the file is **about**. Not an inventory of
    /// what it mentions: that distinction cost three answers on 2026-08-17, when a
    /// language decision given the keys `private layer` and `tray language` started
    /// winning questions that belonged to other files.
    pub summary: String,
    /// The words a real question would use. Required. See the module header.
    pub keys: Vec<String>,
    /// Where under the agent it lands, and which map section it joins.
    pub folder: String,
    pub provenance: String,
    pub stage: String,
    pub body: String,
}

#[derive(Debug)]
pub struct Written {
    pub note: PathBuf,
    pub map: PathBuf,
    pub section: String,
    pub section_created: bool,
    pub staged: Staged,
}

/// What happened when the new files were offered to git.
///
/// **Staging is part of writing here, and that is not tidiness.** `kb` reads
/// `git ls-files` to know what is public, so an untracked note is a note the router
/// will not serve. Without this step the loop does not close: a model writes a
/// memory, the write reports success, and the next question cannot find it. Measured
/// exactly that way on 2026-08-17 against a fresh `kb init`.
///
/// Staged and not committed, deliberately. `git ls-files` reads the index, so
/// staging is enough to make the note findable, while what the history says stays a
/// human decision. It is also visible in `git status` and undone with one command.
#[derive(Debug, PartialEq)]
pub enum Staged {
    /// Tracked now, so the router can serve it.
    Yes,
    /// A `.gitignore` rule covers it. **Not an error.** The note landed in a private
    /// folder and a private note is one the public scope is supposed to skip.
    Ignored,
    /// No git here, so nothing knows what is public. `Memory::open` refuses such a
    /// base outright unless the private layer was asked for.
    NoGit,
    Failed(String),
}

#[derive(Debug)]
pub enum WriteError {
    NoAgent(PathBuf),
    NoMap(PathBuf),
    BadSlug(String),
    Exists(PathBuf),
    NoKeys,
    EmptyBody,
    BadField(&'static str, String, String),
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::NoAgent(p) => write!(
                f,
                "no agent at {}. `kb fleet` lists the agents that exist, and `kb init \
                 <name>` creates one.",
                p.display()
            ),
            WriteError::NoMap(p) => write!(
                f,
                "{} has no MAP.md, so there is nowhere to list the note, and a base \
                 that cannot list cannot route.",
                p.display()
            ),
            WriteError::BadSlug(s) => write!(
                f,
                "'{s}' is not a usable name. Lower case letters, digits and hyphens, \
                 no leading or trailing hyphen, 40 characters at most. It becomes both \
                 the filename and the [[link]] every other note uses to point here."
            ),
            WriteError::Exists(p) => write!(
                f,
                "{} already exists. Refusing to overwrite, because a write over a note \
                 is a silent deletion of whatever it said before. Edit it, or choose \
                 another name.",
                p.display()
            ),
            WriteError::NoKeys => write!(
                f,
                "a note needs keys, and there is no flag to skip it. The keyword scorer \
                 ranks map entries, so a note with no Search for line is a note no \
                 question can reach. Give the words a real question would use, not a \
                 description of the file."
            ),
            WriteError::EmptyBody => write!(
                f,
                "the note body was empty. It is read from stdin: pipe the markdown in, \
                 or redirect a file into it."
            ),
            WriteError::BadField(field, got, legal) => {
                write!(f, "{field} was '{got}', which is not one of: {legal}")
            }
            WriteError::Io(p, e) => write!(f, "cannot write {}: {e}", p.display()),
        }
    }
}

/// Writes the note and its map entry, or neither.
///
/// The note lands first and the map second, and **a failed map write removes the
/// note again**. Leaving the note behind would produce exactly the unreachable file
/// this module refuses to create on purpose, except discovered weeks later by
/// somebody who did not run the command.
pub fn note(fleet: &Path, agent: &str, slug: &str, spec: &Note) -> Result<Written, WriteError> {
    if !valid_slug(slug) {
        return Err(WriteError::BadSlug(slug.to_string()));
    }
    if spec.keys.is_empty() {
        return Err(WriteError::NoKeys);
    }
    if spec.body.trim().is_empty() {
        return Err(WriteError::EmptyBody);
    }
    check_field("provenance", &spec.provenance, PROVENANCE)?;
    check_field("stage", &spec.stage, STAGE)?;

    let root = agent_root(fleet, agent);
    if !root.is_dir() {
        return Err(WriteError::NoAgent(root));
    }
    let map_path = root.join("MAP.md");
    if !map_path.is_file() {
        return Err(WriteError::NoMap(root));
    }

    let folder = spec.folder.trim_matches('/').to_string();
    let note_path = root.join(&folder).join(format!("{slug}.md"));
    if note_path.exists() {
        return Err(WriteError::Exists(note_path));
    }

    let map_before =
        std::fs::read_to_string(&map_path).map_err(|e| WriteError::Io(map_path.clone(), e))?;
    let (map_after, section, section_created) = place_entry(&map_before, &folder, slug, spec);

    if let Some(parent) = note_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| WriteError::Io(parent.to_path_buf(), e))?;
    }
    std::fs::write(&note_path, render_note(spec))
        .map_err(|e| WriteError::Io(note_path.clone(), e))?;

    if let Err(e) = std::fs::write(&map_path, map_after) {
        let _ = std::fs::remove_file(&note_path);
        return Err(WriteError::Io(map_path, e));
    }

    let staged = stage(&root, &note_path, &map_path);
    Ok(Written { note: note_path, map: map_path, section, section_created, staged })
}

fn stage(root: &Path, note: &Path, map: &Path) -> Staged {
    // **A pathspec is resolved relative to `-C`, not to the shell's directory**, so the
    // paths this function is handed have to lose the root prefix before git sees them.
    // Without this, `kb write <agent> <slug> <fleet-root>` built `fleet/apelles/knowledge/
    // masters.md`, handed it to `git -C fleet/apelles add`, and git looked for
    // `fleet/apelles/fleet/apelles/knowledge/masters.md` and failed the whole call. The
    // command still reported the note as written, so the note existed, was untracked, and
    // therefore invisible to the router: exactly the unreachable file this module exists
    // to prevent, produced by the step that exists to prevent it. Found on 2026-08-20 by
    // creating an agent from the fleet root, which is the only way anybody creates one.
    let note = note.strip_prefix(root).unwrap_or(note);
    let map = map.strip_prefix(root).unwrap_or(map);
    let out = crate::base::quiet("git")
        .arg("-C")
        .arg(root)
        .arg("add")
        .arg(note)
        .arg(map)
        .output();

    let out = match out {
        Ok(o) => o,
        Err(_) => return Staged::NoGit,
    };
    if out.status.success() {
        return Staged::Yes;
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("ignored by one of your .gitignore") {
        return Staged::Ignored;
    }
    if err.contains("not a git repository") {
        return Staged::NoGit;
    }
    Staged::Failed(err.trim().to_string())
}

/// The agent's directory, accepting both shapes ADR-0011 and ADR-0008 leave open:
/// a fleet with agents under it, or a base addressed directly.
fn agent_root(fleet: &Path, agent: &str) -> PathBuf {
    let under_agents = fleet.join("fleet").join(agent);
    if under_agents.is_dir() {
        return under_agents;
    }
    if fleet.join("MAP.md").is_file() {
        return fleet.to_path_buf();
    }
    under_agents
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 40
        && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

fn check_field(
    name: &'static str,
    got: &str,
    legal: &'static [&'static str],
) -> Result<(), WriteError> {
    if legal.contains(&got) {
        return Ok(());
    }
    // Built from the list rather than typed beside it. The hand written version said
    // "raw, distilled, derived" for months after a fourth rung existed, so the message
    // that was supposed to tell you the legal values was itself the stale copy.
    let joined: String = legal.join(", ");
    Err(WriteError::BadField(name, got.to_string(), joined))
}

fn render_note(spec: &Note) -> String {
    // **The keys go in the note, per ADR-0028.** They used to go only into the map entry,
    // which was right while `index::build` iterated map entries and is now the one place the
    // router does not look. A note written by this command yesterday would be invisible to
    // retrieval today, and clean to the linter.
    //
    // ADR-0016's principle is untouched and is why this is here rather than optional: a note
    // and the keys that make it reachable arrive together. Only the destination moved.
    let keys = spec
        .keys
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "---\nprovenance: {}\nstage: {}\n---\n\n**Search for:** {keys}\n\n**Exists to:** \
         {}\n\n{}\n",
        spec.provenance,
        spec.stage,
        spec.summary.trim().trim_end_matches('.'),
        spec.body.trim()
    )
}

/// The map entry, in the shape `checks::map_entries` reads back.
fn render_entry(slug: &str, spec: &Note) -> String {
    let keys = spec
        .keys
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("- **[[{slug}]]** {}\n  Search for: {keys}.\n", spec.summary.trim())
}

/// Inserts the entry at the end of its folder's section, creating the section when
/// the map has none.
///
/// End of section rather than sorted, because a map's order carries meaning nothing
/// here can read: sections group by folder and entries are often ordered by how they
/// build on each other. **Appending is the only placement that cannot be wrong about
/// an ordering it does not understand.**
fn place_entry(map: &str, folder: &str, slug: &str, spec: &Note) -> (String, String, bool) {
    let heading = format!("### {folder}/");
    let entry = render_entry(slug, spec);
    let lines: Vec<&str> = map.lines().collect();

    let start = match lines.iter().position(|l| l.trim() == heading) {
        Some(i) => i,
        None => {
            let mut out = map.trim_end().to_string();
            out.push_str(&format!("\n\n{heading}\n\n{entry}"));
            return (out, heading, true);
        }
    };

    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("### "))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());

    // Back up over the blank lines that separate one section from the next, so the
    // entry joins the list rather than landing in the gap after it.
    let mut at = end;
    while at > start + 1 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }

    let mut out: Vec<String> = lines[..at].iter().map(|s| s.to_string()).collect();
    out.push(entry.trim_end().to_string());
    out.extend(lines[at..].iter().map(|s| s.to_string()));

    let mut text = out.join("\n");
    if map.ends_with('\n') {
        text.push('\n');
    }
    (text, heading, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Note {
        Note {
            summary: "What a thing does and why it works.".into(),
            keys: vec!["prefill".into(), "kv cache".into()],
            folder: "knowledge".into(),
            provenance: "agent".into(),
            stage: "derived".into(),
            body: "# A thing\n\nThe body.".into(),
        }
    }

    fn base(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kb-write-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fleet/zed/knowledge")).expect("mkdir");
        std::fs::write(
            dir.join("fleet/zed/MAP.md"),
            "# Map\n\n### knowledge/\n\n- **[[old]]** Already here.\n  Search for: `old`.\n\n### templates/\n\n- **[[t]]** A template.\n  Search for: `template`.\n",
        )
        .expect("write map");
        dir
    }

    #[test]
    fn the_stage_a_promotion_writes_is_a_stage_this_can_write() {
        // The bug this pins: `checks.rs` gained `captured` for ADR-0030 and this file kept
        // its own copy of the list, so the linter accepted the word and the writer refused
        // it. `kb promote` could admit a proposal unanimously and then fail at the write,
        // and nobody saw it, because until the trigger existed every proposal had been
        // refused before it got that far. Asserting the constant rather than the string
        // means the next rung is added in one place or the test says so.
        assert!(
            STAGE.contains(&crate::promote::CAPTURED),
            "promote::CAPTURED is '{}', which kb write will not accept, so promotion \
             cannot write the notes it admits",
            crate::promote::CAPTURED
        );

        let dir = base("captured-stage");
        let mut spec = spec();
        spec.stage = crate::promote::CAPTURED.into();
        note(&dir, "zed", "promoted-note", &spec).expect("a promoted note is writable");
    }

    #[test]
    fn a_note_and_its_map_entry_are_written_together() {
        let dir = base("together");
        let out = note(&dir, "zed", "new-thing", &spec()).expect("written");

        let text = std::fs::read_to_string(&out.note).expect("note");
        assert!(text.starts_with("---\nprovenance: agent\nstage: derived\n---"), "{text}");

        let map = std::fs::read_to_string(&out.map).expect("map");
        assert!(map.contains("- **[[new-thing]]**"), "{map}");
        assert!(map.contains("Search for: `prefill`, `kv cache`."), "{map}");
    }

    /// The entry has to join its own section, not whichever one happens to be last.
    #[test]
    fn the_entry_lands_in_the_section_for_its_folder() {
        let dir = base("section");
        note(&dir, "zed", "new-thing", &spec()).expect("written");

        let map = std::fs::read_to_string(dir.join("fleet/zed/MAP.md")).expect("map");
        let at_new = map.find("[[new-thing]]").expect("present");
        let at_templates = map.find("### templates/").expect("present");
        assert!(at_new < at_templates, "it belongs under knowledge/:\n{map}");
    }

    #[test]
    fn a_folder_with_no_section_gets_one() {
        let dir = base("newsection");
        let mut s = spec();
        s.folder = "knowledge/systems".into();
        let out = note(&dir, "zed", "new-thing", &s).expect("written");

        assert!(out.section_created, "the section did not exist before");
        let map = std::fs::read_to_string(&out.map).expect("map");
        assert!(map.contains("### knowledge/systems/"), "{map}");
        assert!(out.note.ends_with("knowledge/systems/new-thing.md"), "{:?}", out.note);
    }

    /// The rule this module exists for.
    #[test]
    fn a_note_without_keys_is_refused() {
        let dir = base("nokeys");
        let mut s = spec();
        s.keys.clear();
        assert!(matches!(note(&dir, "zed", "x", &s), Err(WriteError::NoKeys)));
        assert!(!dir.join("fleet/zed/knowledge/x.md").exists(), "nothing left behind");
    }

    #[test]
    fn an_existing_note_is_never_overwritten() {
        let dir = base("exists");
        note(&dir, "zed", "twice", &spec()).expect("first");
        let again = note(&dir, "zed", "twice", &spec());
        assert!(matches!(again, Err(WriteError::Exists(_))), "{again:?}");
    }

    #[test]
    fn a_bad_slug_is_refused_before_anything_is_touched() {
        let dir = base("slug");
        for bad in ["", "New Thing", "-lead", "trail-", "under_score"] {
            assert!(
                matches!(note(&dir, "zed", bad, &spec()), Err(WriteError::BadSlug(_))),
                "{bad}"
            );
        }
        let map = std::fs::read_to_string(dir.join("fleet/zed/MAP.md")).expect("map");
        assert!(!map.contains("New Thing"), "the map was not touched");
    }

    #[test]
    fn an_illegal_provenance_is_refused() {
        let dir = base("prov");
        let mut s = spec();
        s.provenance = "robot".into();
        assert!(matches!(
            note(&dir, "zed", "x", &s),
            Err(WriteError::BadField("provenance", _, _))
        ));
    }

    #[test]
    fn a_missing_agent_says_so_rather_than_creating_one() {
        let dir = base("noagent");
        let out = note(&dir, "ghost", "x", &spec());
        assert!(matches!(out, Err(WriteError::NoAgent(_))), "{out:?}");
    }
}
