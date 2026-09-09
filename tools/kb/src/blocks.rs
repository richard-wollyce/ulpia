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

/// Characters per token for ordinary prose, measured 2026-09-08 with the tokenizer the
/// fleet actually runs, `qwen3.5-0.8b-q4_0` through `llama-tokenize`.
///
/// **The corpus is the thing being priced and not a sample of something else:** every
/// resident file of all thirteen bases, after [`strip_keyword_lines`], with fenced blocks
/// taken out. 340,553 bytes, 85,282 tokens, 3.993 characters per token.
///
/// This replaces a comment that claimed 4.06 for prose and 2.7 for code. The 4.06 was
/// Zed's constitution alone in August and the fleet has since grown to thirteen bases;
/// the 2.7 came from one HTML file, and it does not generalise. See
/// [`CODE_CHARS_PER_TOKEN`].
const PROSE_CHARS_PER_TOKEN: f64 = 3.99;

/// Characters per token inside a fenced code block, same tokenizer and same day.
///
/// **Measured, and the number that was there before was not.** Every fenced block in
/// every resident file of the fleet, 772 bytes, 261 tokens, 2.958 characters per token.
/// The corpus is small because the constitutions carry almost no code, so this rate is
/// carried by a second measurement over whole files of each kind on this machine:
/// HTML 2.62 to 2.99, Rust 3.84 to 3.92, `Cargo.toml` 3.45, English markdown 3.95 to
/// 4.20.
///
/// **"Code" is therefore not one rate, and the old comment's 2.7 was markup, not code.**
/// 2.96 is the markup rate, and it is applied to a fence by [`tokens_of`] and to a whole
/// markup file by [`tokens_of_artifact`]. A source file that is not markup keeps the prose
/// rate, because that is what it measures.
const CODE_CHARS_PER_TOKEN: f64 = 2.96;

/// Bytes to tokens for text known to be prose, in the one place that does the arithmetic.
///
/// A second caller wanted this the moment a panel had to price the artifact its reviewers
/// read as well as the constitutions they boot with. Two estimates of the same thing drift
/// by a factor nobody notices until a report and a budget disagree, so the constants stay
/// private and this is how they are reached.
///
/// **This entry point exists for callers holding a byte count and no text.** Anything that
/// still has the text calls [`tokens_of`], which is the same arithmetic with the fences
/// priced at their own rate.
pub fn tokens(bytes: usize) -> usize {
    (bytes as f64 / PROSE_CHARS_PER_TOKEN).round() as usize
}

/// Bytes to tokens for markdown, with fenced code priced at its own rate.
///
/// **The mechanism, and why it is worth a scanner.** A tokenizer merges frequent
/// character runs, so English words come out near one token each and punctuation-dense
/// text does not. Prose and code therefore convert at different rates, and a single
/// constant is only ever right for the mix it was measured on. Splitting the file at its
/// fences prices each part at the rate measured for that part.
///
/// **What this buys on today's fleet is small, and that is a finding rather than a
/// disappointment.** Fenced code is 772 of 351,972 resident bytes, 0.22%, so the whole
/// correction is 68 tokens on a fleet of 88,829. Measured against the real tokenizer the
/// old single-constant estimate was 0.94% low overall; it is the per-file case that was
/// wrong, not the fleet total, and the file that goes wrong is an artifact of markup
/// handed to `kb panel`.
///
/// **What is deliberately not segmented, with the number, so the choice is auditable.**
/// *Indented code blocks*: zero bytes across every resident file in the fleet, and the
/// four-space rule cannot be told from a wrapped list item without a full block parser,
/// so recognising them would invent code where the maps have bullets. *Inline spans*:
/// 10,647 bytes, 3.0% of the resident set, measured at 3.10 characters per token back to
/// back against 3.99 for the prose around them. Pricing them separately moves the fleet
/// estimate by about 0.9% and needs a second scanner with its own escaping rules, which
/// is complexity bought before the second use case.
///
/// **What actually drives the spread is language, and this function does not touch it.**
/// Per base the real rate runs from 3.667 (`cosimo`) to 4.167 (`pegolotti`), and the
/// bases at the dense end are the ones carrying Portuguese. `person/body.md`, prose with
/// no code in it at all, measures 2.852 characters per token, denser than any fence in
/// the fleet. A language dimension would cut the error several times more than this does
/// and it is a separate decision, recorded here rather than smuggled in.
pub fn tokens_of(text: &str) -> usize {
    let code = code_bytes(text);
    let prose = text.len() - code;
    (prose as f64 / PROSE_CHARS_PER_TOKEN + code as f64 / CODE_CHARS_PER_TOKEN).round() as usize
}

/// Bytes to tokens for a whole file, priced by what the file is.
///
/// **Markup is the one extension family worth a rule, and the measurement says which.**
/// Tokenized on this machine on 2026-09-08, whole files come out at: HTML 2.62 to 2.99
/// characters per token (`src/ui.html` 2.98, `site` drafts 2.94 and 2.99), Rust 3.84 to
/// 3.92 (`src/memory.rs`, 147,437 bytes, 37,661 tokens), `Cargo.toml` 3.45, English
/// markdown 3.95 to 4.20. So "code" splits in two and only one half is dense: markup
/// carries attribute punctuation and quoting on every line, while a commented source file
/// is mostly English words and lands within four percent of prose.
///
/// The rule follows the numbers rather than the word "code". A markup file is priced whole
/// at [`CODE_CHARS_PER_TOKEN`]; everything else goes through [`tokens_of`], which still
/// finds a fence inside a markdown file. Pricing `memory.rs` as markup would overstate it
/// by a third, which is the same error the old single constant made, pointing the other
/// way.
///
/// **This exists because `kb panel` prices a real file.** A round on `src/ui.html` read by
/// four reviewers used to be costed at 12,036 tokens against a measured 16,092, and every
/// reviewer paid that error again.
pub fn tokens_of_artifact(path: &Path, text: &str) -> usize {
    const MARKUP: [&str; 5] = ["html", "htm", "xml", "svg", "xhtml"];

    let markup = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MARKUP.iter().any(|m| e.eq_ignore_ascii_case(m)));

    if markup {
        return (text.len() as f64 / CODE_CHARS_PER_TOKEN).round() as usize;
    }
    tokens_of(text)
}

/// The bytes of `text` that sit inside a fenced code block, markers included.
///
/// CommonMark's fence rules, and only those: an opening fence is three or more backticks
/// or tildes at up to three spaces of indent, and it closes on a fence of the **same
/// character** that is at least as long. Both halves matter, and getting either wrong is
/// silent rather than loud: a tilde fence closed by a backtick line, or a four-backtick
/// fence closed by a three-backtick line, swallows the rest of the file into the code
/// rate and prices a constitution a third too high with nothing to see.
///
/// An unclosed fence runs to the end of the text, which is CommonMark's rule and the safe
/// direction here: the alternative is deciding a fence was a typo and pricing a code
/// listing as prose.
pub fn code_bytes(text: &str) -> usize {
    let mut code = 0usize;
    let mut open: Option<(u8, usize)> = None;

    for line in text.split_inclusive('\n') {
        match (fence_of(line), open) {
            // Inside a block: the line is code either way, and a matching fence ends it.
            (Some((c, n)), Some((oc, on))) => {
                code += line.len();
                if c == oc && n >= on {
                    open = None;
                }
            }
            (Some(fence), None) => {
                open = Some(fence);
                code += line.len();
            }
            (None, Some(_)) => code += line.len(),
            (None, None) => {}
        }
    }
    code
}

/// The fence character and its run length, for a line that is one.
///
/// Up to three leading spaces, then three or more backticks or tildes. A backtick fence
/// may not carry a backtick in its info string, because that is how CommonMark keeps an
/// inline span from opening a block; a tilde fence has no such rule.
fn fence_of(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let marker = trimmed.as_bytes().first().copied()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let run = trimmed.bytes().take_while(|b| *b == marker).count();
    if run < 3 {
        return None;
    }
    if marker == b'`' && trimmed[run..].contains('`') {
        return None;
    }
    Some((marker, run))
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
    /// **The part of [`Block::bytes`] that sits inside a fenced code block.** Kept as a
    /// field rather than recomputed because the text is read once, in [`read`], and a
    /// second scan would have to re-read every file off disk to answer the same question.
    pub code_bytes: usize,
    pub missing: Vec<String>,
}

impl Block {
    /// What this block costs in the prompt, with its fences priced at the code rate.
    pub fn tokens(&self) -> usize {
        let prose = self.bytes - self.code_bytes;
        (prose as f64 / PROSE_CHARS_PER_TOKEN + self.code_bytes as f64 / CODE_CHARS_PER_TOKEN)
            .round() as usize
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
                code_bytes: 0,
                missing: Vec::new(),
            });
            continue;
        }

        if let Some(block) = blocks.last_mut() {
            match fs::read_to_string(root.join(line)) {
                Ok(content) => {
                    let sent = strip_keyword_lines(&content);
                    block.file_bytes += content.len();
                    block.bytes += sent.len();
                    block.code_bytes += code_bytes(&sent);
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
/// **What it does not reach, corrected 2026-09-07.** This used to say a retrieved passage
/// still carries its keyword line and that resident and retrieved text were therefore filtered
/// differently. That was wrong, and it was written without looking: `store::chunk` drops those
/// lines before a chunk is ever stored, for a different reason and with the same effect, so
/// that the keyword scorer and the text scorer do not read the same words and call their
/// agreement evidence. Counted over the fleet's fifteen indexes, **0 of 4,357 stored chunks
/// carry one**, so every surface built from passages, `kb answer`, `kb serve`'s `kb_retrieve`,
/// `kb route --json`, `promote::evidence_for` and the reading room, was already clean.
///
/// The one path that genuinely was not is `kb answer --complete`, which reads whole files off
/// disk and never touches the chunker. It is filtered in [`crate::answer::map_prompt`], by
/// this function, at the point the file becomes a prompt.
///
/// **The two stated exceptions, where a keyword line is the subject and not overhead.**
/// `gate::propose` shows a model the keys each near-miss file declares, because its job is to
/// propose an alias from a query term to a term a file already carries and it cannot do that
/// blind. `promote::review_prompt` shows the reviewer the proposed note's own keys, because
/// judging the keys is half of what the reviewer is for. Neither reads a file's prose, so
/// neither goes through here, and both would be broken by a filter that did.
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

    // -----------------------------------------------------------------------
    // Fenced code is priced at its own rate
    // -----------------------------------------------------------------------

    #[test]
    fn a_fence_is_priced_denser_than_the_prose_around_it() {
        // Prose and a fence in one file, each long enough that the two rates are
        // separable by arithmetic instead of by eyeballing a total.
        let prose = "p".repeat(200);
        let fence = format!("```rust\n{}\n```\n", "c".repeat(93));
        let text = format!("{prose}\n{fence}");

        let code = code_bytes(&text);
        assert_eq!(code, fence.len(), "the fence, its markers included, is the code span");

        let prose_part = text.len() - code;
        let expected = (prose_part as f64 / PROSE_CHARS_PER_TOKEN
            + code as f64 / CODE_CHARS_PER_TOKEN)
            .round() as usize;
        assert_eq!(tokens_of(&text), expected);
        assert!(
            tokens_of(&text) > tokens(text.len()),
            "counting the fence at the code rate has to cost more than pricing it as prose"
        );
    }

    #[test]
    fn a_bare_markup_file_is_priced_as_markup_and_not_as_prose() {
        // The case a fence-only rule misses, and it is the one `kb panel` actually meets:
        // an artifact handed to a review round is a whole file, and half of what this
        // repository publishes is markup with no fence anywhere in it. Measured on this
        // machine, `src/ui.html` is 2.98 characters per token against 3.99 for prose.
        let html = "<div class=\"card\"><p>hello</p></div>\n".repeat(40);
        assert_eq!(code_bytes(&html), 0, "there is no fence in a bare .html file");

        let priced = tokens_of_artifact(Path::new("site/index.html"), &html);
        assert_eq!(
            priced,
            (html.len() as f64 / CODE_CHARS_PER_TOKEN).round() as usize,
            "a markup file is markup all the way down"
        );
        assert!(priced > tokens(html.len()), "and it costs more than the same bytes of prose");
    }

    #[test]
    fn a_markdown_artifact_still_goes_through_the_fence_rule() {
        let md = format!("prose {}\n```\n{}\n```\n", "p".repeat(300), "c".repeat(300));
        assert_eq!(tokens_of_artifact(Path::new("notes/x.md"), &md), tokens_of(&md));
    }

    #[test]
    fn a_source_file_that_is_not_markup_keeps_the_prose_rate() {
        // Not every file called code is dense. Measured whole-file on this machine, Rust
        // runs 3.84 to 3.92 characters per token, within 4% of prose, because a
        // well-commented source file is mostly English. Pricing it at the markup rate
        // would overstate it by a third, which is the same error pointing the other way.
        let rs = "pub fn thing(argument: usize) -> usize { argument + 1 }\n".repeat(30);
        assert_eq!(tokens_of_artifact(Path::new("src/lib.rs"), &rs), tokens_of(&rs));
    }

    #[test]
    fn a_file_with_no_fence_prices_exactly_as_prose() {
        let text = "# Title\n\nA paragraph with an `inline span` in it.\n";
        assert_eq!(code_bytes(text), 0, "an inline span is not a fenced block");
        assert_eq!(tokens_of(text), tokens(text.len()));
    }

    #[test]
    fn a_fence_closes_only_on_its_own_marker() {
        // A tilde fence is not closed by a backtick line, and a longer opening fence is
        // not closed by a shorter one. Getting either wrong swallows the rest of the file
        // into the code rate, which is a third too much and it is silent.
        let text = "a\n~~~\n```\nstill code\n~~~\nb\n";
        assert_eq!(code_bytes(text), "~~~\n```\nstill code\n~~~\n".len());

        let longer = "````\n```\nx\n````\n";
        assert_eq!(code_bytes(longer), longer.len(), "a shorter fence closes nothing");
    }

    #[test]
    fn an_unclosed_fence_runs_to_the_end_of_the_file() {
        // CommonMark's rule, and the safe direction: the alternative is deciding a fence
        // was a typo and pricing a code listing as prose.
        let text = "intro\n```\ncode\nmore code\n";
        assert_eq!(code_bytes(text), "```\ncode\nmore code\n".len());
    }

    #[test]
    fn the_block_prices_the_fences_its_files_carry() {
        let dir = scratch("fenced-block");
        fs::write(dir.join("blocks.txt"), "[identity]\ni.md\n").unwrap();
        let file = format!("prose {}\n```\n{}\n```\n", "p".repeat(400), "c".repeat(400));
        fs::write(dir.join("i.md"), &file).unwrap();

        let blocks = read(&dir).expect("manifest");
        assert_eq!(blocks[0].code_bytes, "```\n".len() * 2 + 401);
        assert!(
            blocks[0].tokens() > tokens(blocks[0].bytes),
            "a block holding a fence costs more than the same bytes of prose"
        );
    }

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
        // 400 + 800 + 1200 characters of prose, all resident. The literals used to be
        // 600, 500 and 300, which was 4.0 characters per token; they are written through
        // `tokens` now because the rate is a measurement and measurements move.
        let dir = base("cost", "[identity]\na.md\n\n[user]\nb.md\n\n[map]\nc.md\n");
        let blocks = read(&dir).expect("manifest");
        let cost = invalidation_cost(&blocks);

        assert_eq!(cost[0].1, tokens(400) + tokens(800) + tokens(1200), "identity pays for all");
        assert_eq!(cost[1].1, tokens(800) + tokens(1200));
        assert_eq!(cost[2].1, tokens(1200), "the last block invalidates only itself");
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
