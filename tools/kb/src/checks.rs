//! Parsing and the checks themselves.
//!
//! Every check answers a question the convention already claims to guarantee.
//! If a rule is written in `index.md` and nothing here enforces it, it is a wish.

use crate::base::{Base, MdFile};
use std::collections::HashMap;

#[derive(PartialEq, Clone, Copy)]
pub enum Level {
    Error,
    Warning,
}

pub struct Finding {
    pub level: Level,
    pub code: &'static str,
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Directories whose `[[links]]` are placeholders rather than real references.
const LINK_EXEMPT: &[&str] = &["templates/"];

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Every `[[wikilink]]` in the text, as (line number, target).
///
/// Fenced code blocks and inline code spans are skipped, because a base that
/// documents its own link convention writes `[[file-name]]` in backticks and
/// those are examples, not references.
pub fn wikilinks(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_fence = false;

    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for segment in outside_inline_code(raw) {
            for target in links_in(&segment) {
                out.push((i + 1, target));
            }
        }
    }
    out
}

/// The parts of a line that are not inside single backticks.
///
/// Backticks split the line into alternating outside and inside runs, so the
/// even indexed pieces are the ones outside code.
fn outside_inline_code(line: &str) -> Vec<String> {
    line.split('`').step_by(2).map(|s| s.to_string()).collect()
}

/// Wikilink targets in a single fragment, with `|alias` and `#anchor` stripped.
pub fn links_in(fragment: &str) -> Vec<String> {
    let chars: Vec<char> = fragment.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i + 1 < chars.len() {
        if chars[i] == '[' && chars[i + 1] == '[' {
            if let Some(end) = find_close(&chars, i + 2) {
                let raw: String = chars[i + 2..end].iter().collect();
                let target = raw
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !target.is_empty() {
                    out.push(target);
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn find_close(chars: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < chars.len() {
        if chars[i] == ']' && chars[i + 1] == ']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Lines carrying an em dash or an en dash, at most one report per line.
pub fn dashes(text: &str) -> Vec<(usize, char)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if let Some(c) = line.chars().find(|c| *c == '\u{2014}' || *c == '\u{2013}') {
            out.push((i + 1, c));
        }
    }
    out
}

/// Front matter as key and value pairs, if the file opens with a `---` fenced block.
pub fn front_matter(text: &str) -> Option<Vec<(String, String)>> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut pairs = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            return Some(pairs);
        }
        // An indented line belongs to the key above it. The test has to run on the
        // raw line, because trimming first makes the leading whitespace vanish and
        // the check always pass, which is how the first version of this shipped.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            if !key.is_empty() && !key.starts_with('#') {
                pairs.push((key.to_string(), value.trim().to_string()));
            }
        }
    }
    None // opened and never closed, so not front matter
}

/// Legal values for the two axes in ADR-0007. They are orthogonal on purpose:
/// provenance says who made the claim, stage says where it is in its life, and
/// collapsing them is how a base loses track of what was verified.
const PROVENANCE: &[&str] = &["human", "agent", "external"];
const STAGE: &[&str] = &["raw", "distilled", "derived"];

pub struct MapEntry {
    pub line: usize,
    pub name: String,
    pub has_search_line: bool,
    /// Every line the entry owns, which is what the index reads for keywords.
    pub body: String,
}

/// Entries in a map file: top level list items opening with a bold wikilink.
///
/// An entry owns every line until the next entry or the next heading, which is
/// where its `Search for:` line has to be.
///
/// The shape is exact on purpose, `- **[[name]]**` with no indentation, because
/// all three maps use the other shapes for something else: an indented
/// `- [[name]]` is a sub item inside an entry, and a `- [[name]] beats [[other]]`
/// line in a connections section is a cross reference. Counting those as entries
/// produced 20 false warnings on the first real run, all of them demanding a
/// keyword line for something that is not a file's entry at all.
pub fn map_entries(text: &str) -> Vec<MapEntry> {
    let lines: Vec<&str> = text.lines().collect();

    let mut starts: Vec<(usize, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("- **[[") {
            if let Some(name) = links_in(line).into_iter().next() {
                starts.push((i, name));
            }
        }
    }

    let mut out = Vec::new();
    for (index, (start, name)) in starts.iter().enumerate() {
        let mut end = starts
            .get(index + 1)
            .map(|(next, _)| *next)
            .unwrap_or(lines.len());

        for j in (start + 1)..end {
            if lines[j].starts_with('#') {
                end = j;
                break;
            }
        }

        let has_search_line = lines[*start..end]
            .iter()
            .any(|l| l.contains("Search for:") || l.contains("Buscar por:"));

        out.push(MapEntry {
            line: start + 1,
            name: name.clone(),
            has_search_line,
            body: lines[*start..end].join("\n"),
        });
    }
    out
}

// ---------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------

pub fn run(base: &Base) -> Vec<Finding> {
    let mut findings = Vec::new();

    // stem -> every file that answers to it
    let mut by_stem: HashMap<&str, Vec<&str>> = HashMap::new();
    for file in &base.files {
        by_stem
            .entry(file.stem.as_str())
            .or_default()
            .push(file.rel.as_str());
    }

    // **E03 is gone: a map is no longer required.**
    //
    // It errored when a base had no `MAP.md`, which was right while `index::build` iterated
    // map entries and a base without one contributed nothing. ADR-0028 moved the index onto
    // a walk over files, so a base with no map indexes perfectly well, and ADR-0029 made a
    // directory in the fleet root a base whether or not it has one. Keeping the rule would
    // have printed one error per base forever, which is how people learn to ignore
    // `kb check` output and stop reading the fourteen rules that are still true.

    for file in &base.files {
        let exempt = LINK_EXEMPT.iter().any(|dir| file.rel.starts_with(dir));

        if !exempt {
            for (line, target) in wikilinks(&file.text) {
                match by_stem.get(target.as_str()) {
                    None => findings.push(Finding {
                        level: Level::Error,
                        code: "E01",
                        file: file.rel.clone(),
                        line,
                        message: format!("broken link [[{target}]], no {target}.md in the base"),
                    }),
                    Some(matches) if matches.len() > 1 => findings.push(Finding {
                        level: Level::Warning,
                        code: "W01",
                        file: file.rel.clone(),
                        line,
                        message: format!(
                            "ambiguous link [[{target}]], matches {}",
                            matches.join(", ")
                        ),
                    }),
                    Some(_) => {}
                }
            }
        }

        for (line, dash) in dashes(&file.text) {
            let name = if dash == '\u{2014}' { "em dash" } else { "en dash" };
            findings.push(Finding {
                level: Level::Warning,
                code: "W03",
                file: file.rel.clone(),
                line,
                message: format!("{name}, house style forbids it"),
            });
        }

        if base.is_note(file) {
            check_note(base, file, &mut findings);
        }
    }

    if let Some(map) = base.map_file() {
        for entry in map_entries(&map.text) {
            if !entry.has_search_line {
                findings.push(Finding {
                    level: Level::Warning,
                    code: "W02",
                    file: map.rel.clone(),
                    line: entry.line,
                    message: format!(
                        "entry [[{}]] has no Search for line, so grep cannot route to it",
                        entry.name
                    ),
                });
            }
        }
    }

    findings.sort_by(|a, b| {
        (a.file.clone(), a.line, a.code).cmp(&(b.file.clone(), b.line, b.code))
    });
    findings
}

/// Checks that only apply to a distilled note inside the knowledge folder.
fn check_note(_base: &Base, file: &MdFile, findings: &mut Vec<Finding>) {
    // **Files that orient a person rather than answer a question**, exempt from the
    // reachability rules because nobody searches for them: they are found by standing in
    // the directory they describe.
    //
    // Three names, because one name was doing three jobs across thirteen files and a reader
    // could not tell which they had opened. README.md is a base's front door.
    // `what-goes-here.md` is a folder legend, read by whoever is about to drop a file in.
    // `MOVED.md` is a signpost where content used to be, and it shouts because it has to
    // catch somebody who arrived expecting the content.
    let name = file.rel.to_lowercase();
    let is_readme = name.ends_with("readme.md")
        || name.ends_with("what-goes-here.md")
        || name.ends_with("moved.md");

    if !is_readme {
        // **E02 asks the same question it always asked, of the file instead of the map.**
        //
        // It used to read the base's `MAP.md`: a note the map did not list was unreachable,
        // and that was true while `index::build` iterated map entries. ADR-0028 moved the
        // index onto a walk over files, so a note is reachable when it declares its own
        // keys and a map has no say in it.
        //
        // E03 and E07 are gone with the reason they existed. E03 demanded a map file, which
        // is now optional. E07 caught a map entry written in a shape the router's parser
        // could not read, which cannot matter when the router does not read the map.
        //
        // The sentence is kept word for word, because it was the true part all along.
        let (keywords, _) = crate::index::header_of(&file.text);
        if keywords.is_empty() {
            findings.push(Finding {
                level: Level::Error,
                code: "E02",
                file: file.rel.clone(),
                line: 0,
                message: "not indexed: no `Search for:` line in this file, so the router                           builds no entry for it and it scores zero on every question. A                           file nobody can find does not exist"
                    .into(),
            });
        } else if keywords.len() < 12 {
            // **Reachable is not the same as findable**, and the corpus measured the gap.
            // The median was six terms before 2026-08-20, and a real question missed a
            // file that exists to answer it: `eating out, restaurant, poke, salmon` carried
            // nothing a person would type, and "hoje vou sair com meus amigos, to com azia,
            // o que vou comer" reached it at zero. Widened to thirty terms it answers at
            // 130.21.
            //
            // A warning rather than an error, because a thin line is a note that works
            // badly and a missing one is a note that does not work.
            findings.push(Finding {
                level: Level::Warning,
                code: "W06",
                file: file.rel.clone(),
                line: 0,
                message: format!(
                    "thin keyword line: {} terms. A question uses words the writer did not                      think of, so a short list is a file only reachable by luck. Aim for                      thirty, in both languages, including the words somebody types from                      inside the problem",
                    keywords.len()
                ),
            });
        }
    }

    // The tier and the version only make sense for a note distilled from a
    // source. A house document that happens to live in the knowledge folder,
    // like the evidence ruler itself, is not evidence about anything, so the
    // requirement keys off the file declaring a source rather than off where it
    // sits.
    if let Some(pairs) = front_matter(&file.text) {
        let has = |k: &str| pairs.iter().any(|(key, _)| key == k);
        let value = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());

        if has("source") || has("type") {
            for required in ["evidence_tier", "valid_for"] {
                if !has(required) {
                    findings.push(Finding {
                        level: Level::Warning,
                        code: "W04",
                        file: file.rel.clone(),
                        line: 1,
                        message: format!("front matter declares a source but has no {required}"),
                    });
                }
            }
        }

        // ADR-0007. Without these two, the rule that an agent claim is never
        // promoted to human or external cannot be applied by anyone reading it.
        for (field, legal) in [("provenance", PROVENANCE), ("stage", STAGE)] {
            match value(field) {
                None => findings.push(Finding {
                    level: Level::Warning,
                    code: "W05",
                    file: file.rel.clone(),
                    line: 1,
                    message: format!("front matter has no {field}, so who wrote this is unknown"),
                }),
                Some(v) if !legal.contains(&v.as_str()) => findings.push(Finding {
                    level: Level::Error,
                    code: "E04",
                    file: file.rel.clone(),
                    line: 1,
                    message: format!("{field} is '{v}', which is not one of {}", legal.join(", ")),
                }),
                Some(_) => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_plain_link() {
        let found = wikilinks("see [[the-note]] for more");
        assert_eq!(found, vec![(1, "the-note".to_string())]);
    }

    #[test]
    fn strips_alias_and_anchor() {
        assert_eq!(links_in("[[note|call it this]]"), vec!["note".to_string()]);
        assert_eq!(links_in("[[note#section]]"), vec!["note".to_string()]);
    }

    #[test]
    fn ignores_links_inside_inline_code() {
        // This is the case that matters: a base documenting its own convention.
        let text = "Link convention: `[[file-name]]`, Obsidian style.";
        assert!(wikilinks(text).is_empty());
    }

    #[test]
    fn ignores_links_inside_a_fenced_block() {
        let text = "before\n```\n[[not-a-link]]\n```\nafter [[real]]";
        assert_eq!(wikilinks(text), vec![(5, "real".to_string())]);
    }

    #[test]
    fn reports_the_right_line_number() {
        let text = "one\ntwo\nthree [[here]]";
        assert_eq!(wikilinks(text)[0].0, 3);
    }

    #[test]
    fn finds_two_links_on_one_line() {
        let found = links_in("[[a]] and [[b]]");
        assert_eq!(found, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn unclosed_link_is_not_a_link() {
        assert!(links_in("[[open forever").is_empty());
    }

    #[test]
    fn dashes_are_reported_once_per_line() {
        let text = "a \u{2014} b \u{2014} c\nclean line\nd \u{2013} e";
        let found = dashes(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, 1);
        assert_eq!(found[1].0, 3);
    }

    #[test]
    fn reads_front_matter_keys_and_values() {
        let text = "---\ntitle: A note\nevidence_tier: B\n---\n\n# A note";
        let pairs = front_matter(text).expect("front matter");
        assert_eq!(pairs[0], ("title".to_string(), "A note".to_string()));
        assert_eq!(pairs[1], ("evidence_tier".to_string(), "B".to_string()));
    }

    #[test]
    fn nested_front_matter_lines_are_not_keys() {
        // An indented line belongs to the key above it, not to the document.
        let text = "---\nmetadata:\n  type: project\n---\n";
        let pairs = front_matter(text).expect("front matter");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "metadata");
    }

    #[test]
    fn no_front_matter_when_the_file_does_not_open_with_it() {
        assert!(front_matter("# Just a heading").is_none());
    }

    #[test]
    fn unterminated_front_matter_does_not_count() {
        assert!(front_matter("---\ntitle: x\n\n# body").is_none());
    }

    #[test]
    fn map_entry_sees_its_search_line() {
        let text = "### knowledge/\n\n- **[[note-one]]** does a thing.\n  Search for: `word`.\n\n- **[[note-two]]** does another.\n";
        let entries = map_entries(text);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].has_search_line);
        assert!(!entries[1].has_search_line);
    }

    #[test]
    fn an_entry_does_not_borrow_the_search_line_of_the_next_section() {
        let text = "- **[[lonely]]** no keywords here.\n\n## Another section\n\nSearch for: `stolen`.\n";
        let entries = map_entries(text);
        assert!(!entries[0].has_search_line);
    }

    #[test]
    fn a_cross_reference_in_prose_is_not_an_entry() {
        // From Yaron's "Conexões principais" section.
        let text = "- [[seguranca-e-limites]] vence [[formulas]] em qualquer conflito.\n";
        assert!(map_entries(text).is_empty());
    }

    #[test]
    fn an_indented_sub_item_belongs_to_its_parent_entry() {
        // From Steve's carousel series, where one entry lists its studies.
        let text = "- **[[series]]** the parent.\n  - [[study-one]]: a slide by slide read.\n  Search for: `series`.\n";
        let entries = map_entries(text);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].has_search_line);
    }

    #[test]
    fn portuguese_search_line_counts() {
        let text = "- **[[nota]]** faz uma coisa.\n  Buscar por: `palavra`.\n";
        assert!(map_entries(text)[0].has_search_line);
    }
}
