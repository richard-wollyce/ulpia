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
    pub bytes: usize,
    pub missing: Vec<String>,
}

impl Block {
    pub fn tokens(&self) -> usize {
        tokens(self.bytes)
    }
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
                missing: Vec::new(),
            });
            continue;
        }

        if let Some(block) = blocks.last_mut() {
            match fs::read_to_string(root.join(line)) {
                Ok(content) => {
                    block.bytes += content.len();
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
pub fn assemble(root: &Path, blocks: &[Block]) -> String {
    let mut out = String::new();

    for block in blocks.iter().filter(|b| b.mode == Mode::Resident) {
        out.push_str(&format!("\n<!-- block: {} -->\n\n", block.name));
        for file in &block.files {
            if let Ok(content) = fs::read_to_string(root.join(file)) {
                out.push_str(&content);
                if !content.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
        }
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
}
