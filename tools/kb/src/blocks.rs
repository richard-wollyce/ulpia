//! The constitution, as labelled blocks in stability order.
//!
//! ADR-0007, taken from Letta's core memory. Today the constitution is one blob that
//! gets read whole on every question. As blocks it becomes several, each labelled by
//! purpose and each independently swappable.
//!
//! **The order is the mechanism.** Prefix caching reuses the KV state of a prompt
//! only up to the first token that differs, so everything after a changed block has
//! to be recomputed whatever else it says. Ordering the blocks most stable first
//! means a project switch invalidates the project block onward and leaves the
//! identity alone. Ordering them any other way silently throws away the 12x measured
//! in [[local-inference-latitude-3420]].

use std::fs;
use std::path::Path;

/// Prose in this base measures 4.06 characters per token with the real tokenizer, and
/// code 2.7. Four is close enough for a budget and honest about being an estimate.
const CHARS_PER_TOKEN: f64 = 4.0;

/// Bytes to tokens, in the one place that does the arithmetic.
///
/// A second caller wanted this the moment a panel had to price the artifact its reviewers
/// read as well as the constitutions they boot with. Two estimates of the same thing drift
/// by a factor nobody notices until a report and a budget disagree, so the constant stays
/// private and this is how it is reached.
pub fn tokens(bytes: usize) -> usize {
    (bytes as f64 / CHARS_PER_TOKEN).round() as usize
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Mode {
    /// Loaded into the prompt at wake, and paid for by every question afterwards.
    Resident,
    /// Fetched only when the question needs it.
    OnDemand,
}

pub struct Block {
    pub name: String,
    pub mode: Mode,
    /// Relative paths, in order.
    pub files: Vec<String>,
    /// **What this block puts in the prompt**, after [`strip_keyword_lines`], and therefore
    /// what it costs. Every consumer that turns a block into a number reads this one, so
    /// `kb blocks`, the panel's cost, the reading room and [`invalidation_cost`] all price
    /// what is sent without any of them knowing a filter exists.
    pub bytes: usize,
    /// **What the same files measure on disk**, always at least [`Block::bytes`]. Kept
    /// separately because a single number that quietly changed meaning is the drift the
    /// reports read this output to catch: a size that dropped by a third with no column
    /// saying why is indistinguishable from files somebody deleted.
    pub file_bytes: usize,
    pub missing: Vec<String>,
}

impl Block {
    pub fn tokens(&self) -> usize {
        tokens(self.bytes)
    }

    /// Bytes on disk that never reach the prompt. Zero for a block with no keyword lines.
    pub fn trimmed(&self) -> usize {
        self.file_bytes.saturating_sub(self.bytes)
    }
}

/// The note stapled to a block header whose files lost keyword lines.
///
/// **Cheap honesty, and the reason it earns its twenty tokens is written in the maps
/// themselves.** Seven of the fifteen `MAP.md` files tell their reader, in prose that
/// survives the filter, that "that line is what the router matches against". A reader who
/// follows that sentence looking for the line and finds nothing cannot tell an entry that
/// was trimmed from an entry that is broken. One clause per trimmed block buys that
/// difference, and it is conditional so a block that lost nothing never claims it did.
const TRIMMED_NOTE: &str = " (`Search for:` lines removed: the router matches those, not you)";

/// Removes the keyword lines a file carries for the router, and the wrapped lines that
/// continue them.
///
/// **The mechanism, and it is the whole justification.** A `Search for:` line exists so
/// that `index::header_of` can build a SQLite entry for the file. Retrieval then happens
/// in `retrieve`, against that index, before the model is handed anything. The model
/// never scores a candidate, so a keyword line in its prompt is a copy of data that was
/// already consumed somewhere it could not see. On this fleet that copy is 133,105 bytes
/// of the 269,899 in the fifteen maps, 49.3%, measured 2026-09-07.
///
/// **What counts as the line is `index::labelled`'s decision and not a second one.** The
/// label has to be the entire head before the first colon, so `Search for:` and the bolded
/// `**Search for:**` both go and a sentence that merely mentions the label in backticks
/// stays. That distinction is not theoretical: every map preamble contains such a sentence.
///
/// **A wrapped line is taken only when two independent signs agree**, that the line before
/// it ended on a comma and that it opens on a backtick. Either alone would do on today's
/// files, where the two agree on all 391 keyword lines in the fleet, and requiring both
/// means the filter under-removes rather than over-removes when somebody writes a keyword
/// line this file has not seen. Leaving a stray line of keywords in a prompt is a wasted
/// line; taking a line of prose out of a constitution is a lost instruction.
///
/// Byte exact when nothing matches: the split keeps each line's own newline, so a file
/// with no keyword line comes back identical rather than newline-normalised.
pub fn strip_keyword_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut lines = text.split_inclusive('\n').peekable();
    let open = |t: &str| t.trim_end().ends_with(',');

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if crate::index::labelled(trimmed, &["search for", "buscar por"]).is_none() {
            out.push_str(line);
            continue;
        }

        let mut wrapping = open(trimmed);
        while wrapping {
            match lines.peek() {
                Some(next) if next.trim().starts_with('`') => {
                    wrapping = open(next.trim());
                    lines.next();
                }
                _ => break,
            }
        }
    }
    out
}

/// Reads `blocks.txt` from the base root.
///
/// Same shape as the alias table and for the same reason: this file is edited by a
/// person deciding what the agent wakes up knowing, and a format they have to look up
/// is a format they will not touch.
pub fn read(root: &Path) -> Option<Vec<Block>> {
    let text = fs::read_to_string(root.join("blocks.txt")).ok()?;
    let mut blocks: Vec<Block> = Vec::new();

    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix('[') {
            let (name, tail) = match rest.split_once(']') {
                Some(pair) => pair,
                None => continue,
            };
            let mode = if tail.trim().eq_ignore_ascii_case("on-demand") {
                Mode::OnDemand
            } else {
                Mode::Resident
            };
            blocks.push(Block {
                name: name.trim().to_string(),
                mode,
                files: Vec::new(),
                bytes: 0,
                file_bytes: 0,
                missing: Vec::new(),
            });
            continue;
        }

        if let Some(block) = blocks.last_mut() {
            match fs::read_to_string(root.join(line)) {
                Ok(content) => {
                    block.file_bytes += content.len();
                    block.bytes += strip_keyword_lines(&content).len();
                    block.files.push(line.to_string());
                }
                // A manifest that points at a file nobody moved yet is a real finding,
                // not a reason to fail: the report shows it and the sizes stay honest.
                Err(_) => block.missing.push(line.to_string()),
            }
        }
    }

    Some(blocks)
}

/// What changing each resident block costs, in tokens that must be prefilled again.
///
/// A change invalidates its own block and everything after it, so the first block is
/// the most expensive to touch. That asymmetry is the reason the order exists, and
/// printing it is what stops the order from becoming decoration.
pub fn invalidation_cost(blocks: &[Block]) -> Vec<(String, usize)> {
    let resident: Vec<&Block> = blocks.iter().filter(|b| b.mode == Mode::Resident).collect();
    let mut out = Vec::new();

    for (i, block) in resident.iter().enumerate() {
        let cost: usize = resident[i..].iter().map(|b| b.tokens()).sum();
        out.push((block.name.clone(), cost));
    }
    out
}

/// The resident blocks concatenated in order, with a marker before each one.
///
/// The markers are for the human reading the assembled prompt. They cost a handful of
/// tokens and they are what makes a 12,000 token wall of text reviewable.
///
/// **This is the one place a file becomes prompt, which is why the keyword filter lives
/// here and not on disk.** The lines stay in the files, so `kb check` goes on validating
/// them and an agent goes on editing them by hand; what changes is only what the model is
/// handed. The alternative, deleting them from disk, is a migration across fifteen bases
/// plus a linter change, and it would take the router's index down with it.
///
/// **It applies to every resident file and not to the map alone.** The map is the only
/// block carrying these lines today at 133,105 bytes; the other resident files carry
/// 28,428 more, all of them the file-top form that `index::header_of` actually reads, so
/// the general rule saves a further fifth and the narrow one leaves the same waste free to
/// come back the moment somebody adds a keyed file to the identity block. The cost of the
/// general rule is that it reaches further than the block that motivated it, which is what
/// [`TRIMMED_NOTE`] is for.
///
/// **What it does not reach**, and this is a real seam and not an oversight: a passage the
/// router retrieves arrives through `retrieve`, not through here, so a note reached by a
/// question still carries its keyword line. Resident and retrieved text are therefore
/// filtered differently, and closing that is a separate change against a different caller.
pub fn assemble(root: &Path, blocks: &[Block]) -> String {
    let mut out = String::new();

    for block in blocks.iter().filter(|b| b.mode == Mode::Resident) {
        let mut body = String::new();
        let mut trimmed = false;

        for file in &block.files {
            if let Ok(content) = fs::read_to_string(root.join(file)) {
                let kept = strip_keyword_lines(&content);
                trimmed |= kept.len() != content.len();
                body.push_str(&kept);
                if !kept.ends_with('\n') {
                    body.push('\n');
                }
                body.push('\n');
            }
        }

        let note = if trimmed { TRIMMED_NOTE } else { "" };
        out.push_str(&format!("\n<!-- block: {}{note} -->\n\n", block.name));
        out.push_str(&body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-block-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn base(name: &str, manifest: &str) -> std::path::PathBuf {
        let dir = scratch(name);
        fs::write(dir.join("blocks.txt"), manifest).unwrap();
        fs::write(dir.join("a.md"), "a".repeat(400)).unwrap();
        fs::write(dir.join("b.md"), "b".repeat(800)).unwrap();
        fs::write(dir.join("c.md"), "c".repeat(1200)).unwrap();
        dir
    }

    #[test]
    fn reads_blocks_and_modes_in_order() {
        let dir = base(
            "modes",
            "[identity]\na.md\n\n[user]\nb.md\n\n[project] on-demand\nc.md\n",
        );
        let blocks = read(&dir).expect("manifest");

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].name, "identity");
        assert_eq!(blocks[0].mode, Mode::Resident);
        assert_eq!(blocks[2].mode, Mode::OnDemand);
        assert_eq!(blocks[0].bytes, 400);
    }

    #[test]
    fn a_missing_file_is_reported_and_not_counted() {
        let dir = base("missing", "[identity]\na.md\ngone.md\n");
        let blocks = read(&dir).expect("manifest");
        assert_eq!(blocks[0].bytes, 400);
        assert_eq!(blocks[0].missing, vec!["gone.md".to_string()]);
    }

    #[test]
    fn changing_the_first_block_costs_the_most() {
        // 400 + 800 + 1200 characters, all resident, at 4 characters per token.
        let dir = base("cost", "[identity]\na.md\n\n[user]\nb.md\n\n[map]\nc.md\n");
        let blocks = read(&dir).expect("manifest");
        let cost = invalidation_cost(&blocks);

        assert_eq!(cost[0].1, 600, "identity invalidates everything after it too");
        assert_eq!(cost[1].1, 500);
        assert_eq!(cost[2].1, 300, "the last block invalidates only itself");
        assert!(cost[0].1 > cost[1].1 && cost[1].1 > cost[2].1);
    }

    #[test]
    fn on_demand_blocks_are_not_in_the_resident_cost() {
        let dir = base("ondemand", "[identity]\na.md\n\n[project] on-demand\nc.md\n");
        let blocks = read(&dir).expect("manifest");
        let cost = invalidation_cost(&blocks);
        assert_eq!(cost.len(), 1);
        assert_eq!(cost[0].1, 100);
    }

    #[test]
    fn assembly_contains_only_resident_blocks_in_order() {
        let dir = base("assemble", "[identity]\na.md\n\n[project] on-demand\nc.md\n");
        let blocks = read(&dir).expect("manifest");
        let text = assemble(&dir, &blocks);

        assert!(text.contains("block: identity"));
        assert!(!text.contains("block: project"), "on-demand must stay out");
        assert!(text.contains(&"a".repeat(400)));
        assert!(!text.contains(&"c".repeat(1200)));
    }

    #[test]
    fn no_manifest_is_not_an_error() {
        let dir = scratch("none");
        assert!(read(&dir).is_none());
    }

    // -----------------------------------------------------------------------
    // The keyword filter
    // -----------------------------------------------------------------------

    #[test]
    fn a_file_with_no_keyword_line_comes_back_byte_identical() {
        let text = "# Title\n\nA paragraph, ending on a comma,\nand a second line.\n";
        assert_eq!(strip_keyword_lines(text), text);
        // No trailing newline either: the filter must not normalise one in.
        assert_eq!(strip_keyword_lines("one\ntwo"), "one\ntwo");
    }

    #[test]
    fn both_spellings_of_the_label_go() {
        let text = "# T\n\n**Search for:** `a`, `b`\n\nbody\n\n- **[[x]]** an entry.\n  \
                    Search for: `c`, `d`.\n";
        let out = strip_keyword_lines(text);
        assert!(!out.contains("Search for:"), "neither spelling survives: {out}");
        assert!(out.contains("body") && out.contains("[[x]]"), "the entry itself stays: {out}");
    }

    #[test]
    fn a_sentence_that_mentions_the_label_is_prose_and_stays() {
        // Every map preamble in the fleet contains a line of this shape. Removing it
        // would take a paragraph of instructions out of the constitution.
        let text = "Each entry gets a `Search for:` line carrying the words a real\n\
                    question would use, because that line is what the router matches.\n";
        assert_eq!(strip_keyword_lines(text), text);
    }

    #[test]
    fn a_wrapped_keyword_line_goes_with_its_continuations() {
        let text = "- **[[n]]** a note.\n  Search for: `one`, `two`,\n  `three`, `four`.\n\
                    \n- **[[m]]** the next note.\n";
        let out = strip_keyword_lines(text);
        assert!(!out.contains("three"), "the wrapped half goes too: {out}");
        assert!(out.contains("[[m]]"), "the next entry survives: {out}");
    }

    #[test]
    fn a_line_after_a_comma_that_is_not_keywords_survives() {
        // Both signs are required, so a prose line that happens to follow a keyword
        // line ending on a comma is kept. Under-removing is the safe direction.
        let text = "  Search for: `one`, `two`,\n  and then a sentence nobody meant to lose.\n";
        let out = strip_keyword_lines(text);
        assert_eq!(out, "  and then a sentence nobody meant to lose.\n");
    }

    #[test]
    fn the_block_sizes_separate_what_is_sent_from_what_is_on_disk() {
        let dir = scratch("trim");
        fs::write(dir.join("blocks.txt"), "[map]\nm.md\n").unwrap();
        let file = "# M\n\n- **[[n]]** a note.\n  Search for: `one`, `two`.\n";
        fs::write(dir.join("m.md"), file).unwrap();

        let blocks = read(&dir).expect("manifest");
        assert_eq!(blocks[0].file_bytes, file.len());
        assert!(blocks[0].bytes < blocks[0].file_bytes, "the sent size is the smaller one");
        assert_eq!(blocks[0].trimmed(), blocks[0].file_bytes - blocks[0].bytes);
        assert_eq!(blocks[0].tokens(), tokens(blocks[0].bytes), "cost prices what is sent");
    }

    #[test]
    fn only_a_block_that_lost_something_says_so() {
        let dir = scratch("note");
        fs::write(dir.join("blocks.txt"), "[identity]\ni.md\n\n[map]\nm.md\n").unwrap();
        fs::write(dir.join("i.md"), "# I\n\nplain prose, no keys.\n").unwrap();
        fs::write(dir.join("m.md"), "# M\n\n- **[[n]]** x.\n  Search for: `one`.\n").unwrap();

        let text = assemble(&dir, &read(&dir).expect("manifest"));
        assert!(text.contains("<!-- block: identity -->"), "untouched block is unannotated");
        assert!(
            text.contains(&format!("<!-- block: map{TRIMMED_NOTE} -->")),
            "the trimmed block says so: {text}"
        );
    }
}
