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
///
/// **A bibliography entry is exempt, and the exemption is the rule read correctly.** House
/// style governs what we write; a book's real title is what somebody else wrote, and
/// `kb write` refuses a note outright on a dash, so without this a source titled with an em
/// dash could be recorded and never cited. Rewriting the title to satisfy our style would
/// corrupt the one string the whole citation model exists to keep single.
pub fn dashes(text: &str) -> Vec<(usize, char)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if crate::sources::bibliography_line(line).is_some() {
            continue;
        }
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
///
/// **Public because `write.rs` reads them from here rather than keeping its own copy.**
/// It kept its own copy until 2026-08-20, and the two drifted the moment `captured` was
/// added: the linter accepted the word, the writer rejected it, and `kb promote` could not
/// write a single note. Nobody saw it, because every proposal until then had been refused
/// before reaching the write. One list, so a new rung is added once or not at all.
pub const PROVENANCE: &[&str] = &["human", "agent", "external"];
/// `captured` is the rung a promoted note lands on.
///
/// Kept out of `distilled` deliberately. A note that arrived through `kb promote` was read
/// by a model and reviewed by a model and by no person, and ADR-0007's rule is that an
/// agent claim is never quietly promoted. The word says who has and has not looked at it.
pub const STAGE: &[&str] = &["raw", "captured", "distilled", "derived"];

// ---------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------

pub fn run(base: &Base) -> Vec<Finding> {
    let mut findings = Vec::new();

    // The base's source records, read off disk here rather than carried on `Base`, the same
    // way `check_ignores` reads the `.gitignore`. They are `.txt`, so the markdown walk never
    // saw them, and giving `Base` a field for them would put them in front of every consumer
    // of a base including the ones that only want notes.
    //
    // **The private declaration is honoured here or E06 becomes a lie.** On a base that
    // declares itself private as a whole, discovery without `--all` leaves `base.files`
    // empty. Loading the records anyway would find every one of them cited by nobody and
    // report the base as broken, when the truth is that the notes citing them were filtered
    // out one function earlier. Reading a second thing off the same disk means making the
    // same decision the walk made.
    let layer = crate::base::private_layer(&base.root);
    let hidden = !base.all && layer.covers(crate::sources::DIR);
    let (sources, bad_records) = if hidden {
        (Vec::new(), Vec::new())
    } else {
        crate::sources::load(&base.root)
    };
    for bad in &bad_records {
        findings.push(Finding {
            level: Level::Error,
            code: "E09",
            file: bad.path.clone(),
            line: 0,
            message: format!(
                "not a source record: {}. Records are written by `kb source add`, so a \
                 broken one means the file was edited by hand",
                bad.problem
            ),
        });
    }
    let by_key = crate::sources::by_key(&sources);

    // **A private note still counts as a citation, and counting it leaks nothing.**
    //
    // Without this, `kb check` on an agent base reports every source cited only from
    // `records/` or `projects/` as cited by nobody, because the walk dropped those files
    // one function earlier. The finding would be an error, and it would be false.
    //
    // Privacy here is about what gets reported and served, not about whether a fact is
    // counted. E06 names the source record, which is public, and never the file that cites
    // it, so the only thing crossing the line is the number of files a key appeared in, and
    // that number never reaches an output. E05, E08 and W09 do name the file, so they are
    // reported only over `base.files`, which is the filtered set.
    //
    // The scan is over the declared private folders alone, not over the base again: the
    // public files are already in memory.
    let mut cited: std::collections::BTreeSet<String> = if base.all || sources.is_empty() {
        std::collections::BTreeSet::new()
    } else {
        cited_in_private_layer(&base.root, &layer)
    };

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

        // **Reachability is asked of every file, because `index::build` walks every file.**
        //
        // This used to sit inside `if base.is_note(file)`, so it ran only over the knowledge
        // folder. `index::build` stopped iterating map entries on 2026-08-20 and started
        // walking `base.files`; the check did not move with it. Measured the same week: 33
        // real knowledge files outside `knowledge/` carried no `Search for:` line, were
        // absent from the index and could not be returned by any question, while
        // `kb check fleet/zed` printed "31 tracked files, clean" over the top of them. A
        // linter that certifies an unreachable file is worse than no linter, because it is
        // the reason nobody looked.
        check_reachable(base, file, &mut findings);
        check_citations(file, &sources, &by_key, &mut cited, &mut findings);

        if base.is_note(file) {
            check_note(base, file, &mut findings);
        }
    }

    check_uncited_sources(&sources, &cited, &mut findings);

    // **W02 is retired, and it goes with the line it graded.** It warned that a map entry
    // carried no `Search for:` line, "so grep cannot route to it", which stopped being true
    // with ADR-0028: the router reads the note's own header and has not read a map entry
    // since. What was left was a check on a second copy of the keys, and the copy has no
    // reader. E02, above, asks the same question of the file that decides it, so a note the
    // map does not describe is still reported and a note the map describes badly is not
    // reported twice. Keeping W02 would have meant every entry `kb write` now produces
    // warning about itself, which is a tool whose linter complains about its own output.
    //
    // Measured before removing it: 0 findings across the fleet's fifteen bases on
    // 2026-09-07, because every existing entry carries the line. It graded 391 lines and
    // reported nothing, which is what a check on a redundant copy looks like.

    check_ignores(base, &mut findings);

    findings.sort_by(|a, b| {
        (a.file.clone(), a.line, a.code).cmp(&(b.file.clone(), b.line, b.code))
    });
    findings
}

/// W08: a `.gitignore` is here and does not cover a folder the base declares private.
///
/// `kb` publishes nothing and asks git nothing (ADR-0034), so on its own this cannot
/// leak a file. What it can do is let two declarations drift: the manifest says a
/// folder is private, the ignore file a person may later push with says nothing about
/// it, and the day they run `git add -A` the folder is in the history. A warning and
/// never a refusal, because git is optional here and a base with no ignore file at all
/// is a base that has made no promise to git.
///
/// A base that is private as a whole is skipped: there is no folder list to compare,
/// and whether such a base is versioned at all is the person's call.
fn check_ignores(base: &Base, findings: &mut Vec<Finding>) {
    let Ok(ignores) = std::fs::read_to_string(base.root.join(".gitignore")) else { return };
    let folders = match crate::base::private_layer(&base.root) {
        crate::base::PrivateLayer::Whole => return,
        crate::base::PrivateLayer::Folders(f) => f,
    };
    let covered: Vec<String> = ignores
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .map(|l| l.trim_start_matches('/').trim_end_matches('/').to_string())
        .filter(|l| !l.is_empty())
        .collect();
    for folder in folders {
        if !covered.iter().any(|c| c == &folder) {
            findings.push(Finding {
                level: Level::Warning,
                code: "W08",
                file: ".gitignore".into(),
                line: 0,
                message: format!(
                    "`{folder}/` is declared private and this .gitignore does not cover it. \
                     kb serves nothing from it without --all and publishes nothing ever, \
                     so this is a warning; a repository pushed from here would publish it"
                ),
            });
        }
    }
}

/// Citation keys used by markdown inside the base's declared private folders.
///
/// Only the keys come back. Nothing about which file, how many, or what it said, so this
/// cannot put a private path into a finding even by accident.
fn cited_in_private_layer(
    root: &std::path::Path,
    layer: &crate::base::PrivateLayer,
) -> std::collections::BTreeSet<String> {
    let folders: Vec<String> = match layer {
        crate::base::PrivateLayer::Whole => vec![String::new()],
        crate::base::PrivateLayer::Folders(f) => f.clone(),
    };
    let mut keys = std::collections::BTreeSet::new();
    for folder in folders {
        walk_for_citations(&root.join(folder), &mut keys);
    }
    keys
}

fn walk_for_citations(dir: &std::path::Path, keys: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        // `.git` and `.kb` hold no notes and are the two directories large enough to make
        // this walk visible in the time `kb check` takes.
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk_for_citations(&path, keys);
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                for (_, token) in crate::sources::citations(&text) {
                    keys.insert(token);
                }
            }
        }
    }
}

/// E05, E08 and W09: the three things a `[src:KEY]` pointer can be wrong about.
///
/// **A citation is a pointer, and these are the ways a pointer stops pointing.** The
/// bibliography at the foot of a note is a rendering of the records it cites, so it is a
/// cache with an equality check rather than a second copy a person maintains. That is the
/// whole difference between this and the prose paragraphs it replaces: a restatement can
/// drift and nothing notices, a projection cannot drift without failing E08.
///
/// - **E05**, the key does not resolve. A malformed token or no record with that key.
/// - **E08**, the note cites a key and carries no bibliography line for it, or carries one
///   that no longer matches the record. Without this, the embedded copy is exactly the
///   restatement the pointer was introduced to remove, and item three of the design would
///   have reintroduced the bug item one exists to kill.
/// - **W09**, the note cites a source another record supersedes. A warning and not an error
///   on purpose: citing what was actually read on the day is legitimate, and an error would
///   force somebody to rewrite the history of their own reading. The correction is surfaced
///   and the choice is left where it belongs.
///
/// A key that has been superseded still resolves, which is `dc:replaces` doing its one job.
/// The old record is never deleted.
fn check_citations(
    file: &MdFile,
    sources: &[crate::sources::Source],
    by_key: &std::collections::BTreeMap<&str, &crate::sources::Source>,
    cited: &mut std::collections::BTreeSet<String>,
    findings: &mut Vec<Finding>,
) {
    let pointers = crate::sources::citations(&file.text);
    if pointers.is_empty() {
        return;
    }

    // Every bibliography entry in this file, by the key it opens with.
    let entries: HashMap<String, (usize, String)> = file
        .text
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            crate::sources::bibliography_line(l).map(|(k, rest)| (k, (i + 1, rest)))
        })
        .collect();

    let mut reported: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (line, token) in pointers {
        let Some(source) = by_key.get(token.as_str()) else {
            if reported.insert(token.clone()) {
                let why = if crate::sources::is_key(&token) {
                    format!("no {}/{token}.txt in this base", crate::sources::DIR)
                } else {
                    format!(
                        "`{token}` is not a key: {} characters from {}",
                        crate::sources::KEY_LEN,
                        String::from_utf8_lossy(crate::sources::ALPHABET)
                    )
                };
                findings.push(Finding {
                    level: Level::Error,
                    code: "E05",
                    file: file.rel.clone(),
                    line,
                    message: format!(
                        "citation [src:{token}] does not resolve: {why}. A citation is a \
                         pointer, so a key with no record behind it is a claim with no source"
                    ),
                });
            }
            continue;
        };
        cited.insert(token.clone());

        // Once per key per file. A note citing a superseded source names it twice by
        // construction, in the prose and in its own bibliography line, and reporting the
        // same correction twice teaches the reader that the second half of the output is
        // padding.
        if reported.insert(format!("old:{token}")) {
            if let Some(newer) = crate::sources::replacement_of(sources, &token) {
                findings.push(Finding {
                    level: Level::Warning,
                    code: "W09",
                    file: file.rel.clone(),
                    line,
                    message: format!(
                        "[src:{token}] is superseded by [src:{}], \"{}\". The old key still \
                         resolves, so this is a correction to consider and not a broken link",
                        newer.key, newer.title
                    ),
                });
            }
        }

        if !reported.insert(format!("bib:{token}")) {
            continue;
        }
        let expected = crate::sources::render_line(source);
        match entries.get(&token) {
            None => findings.push(Finding {
                level: Level::Error,
                code: "E08",
                file: file.rel.clone(),
                line,
                message: format!(
                    "cited [src:{token}] with no bibliography line for it. Add:\n    - \
                     [src:{token}] {expected}"
                ),
            }),
            Some((at, actual)) if actual != &expected => findings.push(Finding {
                level: Level::Error,
                code: "E08",
                file: file.rel.clone(),
                line: *at,
                message: format!(
                    "the bibliography line for [src:{token}] has drifted from the record. \
                     The record says:\n    - [src:{token}] {expected}"
                ),
            }),
            Some(_) => {}
        }
    }
}

/// E06: a source record no note in this base cites.
///
/// **An error rather than a warning, because an uncited source is a claim about work that
/// was never used.** A record is minted by reading something; if nothing cites it, either a
/// note is missing its pointer or the record should not exist, and both are the writer's to
/// resolve. Left as a warning it would accumulate exactly the way the `SOURCES.md` ledgers
/// did, into a list nobody reconciles.
///
/// A superseded record is exempt, and that exemption is what makes `dc:replaces` work. The
/// old record stays on disk so its key keeps resolving for every note that already cites it,
/// and notes will stop citing it over time. Without the exemption, correcting a source would
/// fail the check the moment the last note moved to the new key, which is the opposite of
/// what a correction should cost.
fn check_uncited_sources(
    sources: &[crate::sources::Source],
    cited: &std::collections::BTreeSet<String>,
    findings: &mut Vec<Finding>,
) {
    for source in sources {
        if cited.contains(&source.key) {
            continue;
        }
        if crate::sources::replacement_of(sources, &source.key).is_some() {
            continue;
        }
        findings.push(Finding {
            level: Level::Error,
            code: "E06",
            file: format!("{}/{}.txt", crate::sources::DIR, source.key),
            line: 0,
            message: format!(
                "no note cites [src:{}], \"{}\". The bibliography is a projection of the \
                 pointers, so a record nothing points at is in no bibliography and answers \
                 no question. Cite it or delete it",
                source.key, source.title
            ),
        });
    }
}

/// Checks that only apply to a distilled note inside the knowledge folder.
/// Can a question reach this file at all.
///
/// Split out of `check_note` so it runs over every file the index walks rather than only over
/// the knowledge folder. The two halves answer different questions: this one asks whether the
/// router can see the file, and `check_note` asks whether a note it CAN see carries the
/// provenance the evidence rules require.
fn check_reachable(base: &Base, file: &MdFile, findings: &mut Vec<Finding>) {
    // The base's map is passed rather than assumed, so this reports exactly the population
    // `index::build` leaves in `Built::unreachable`. They were already required to agree;
    // once the catalogue became an arm of the exemption, agreeing meant asking with the
    // same argument.
    if crate::index::is_exempt(base.map.as_deref(), &file.rel) {
        return;
    }

    {
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
                message: "not indexed: no `Search for:` line in this file, so the router \
                          builds no entry for it and it scores zero on every question. A \
                          file nobody can find does not exist"
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
                    "thin keyword line: {} terms. A question uses words the writer did not think of, so a short list is a file only reachable by luck. Aim for thirty, in both languages, including the words somebody types from inside the problem",
                    keywords.len()
                ),
            });
        }

            // **Asked of every keyed file, not only the thin ones.** This sat inside the
        // `keywords.len() < 12` branch for one build and reported zero findings across
        // the whole fleet, which read exactly like a corpus with no dead keys in it.
        //
        // A key in neither bag is a term the author believed they had written and that
        // no question can ever use. That is the silence E02 exists to break, one level
        // down: E02 catches a file nothing can reach, this catches a term nothing can
        // reach inside a file that is otherwise fine.
        let dead = crate::index::unreachable_keys(&keywords);
        if !dead.is_empty() {
            findings.push(Finding {
                level: Level::Warning,
                code: "W07",
                file: file.rel.clone(),
                line: 0,
                message: format!(
                    "unsearchable key(s): {}. Several written words that reduce to one \
                     after stopwords, so the term reaches neither the keyword index nor \
                     the phrase index and no question can find it. Rewrite so the word \
                     that carries the meaning is the one that survives: `nao contestar` \
                     indexed as `contestar`, its own opposite, until it became \
                     `proibido contestar`",
                    dead.join(", ")
                ),
            });
        }
    }

}

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
    // The tier and the version only make sense for a note distilled from a
    // source. A house document that happens to live in the knowledge folder,
    // like the evidence ruler itself, is not evidence about anything, so the
    // requirement keys off the file declaring a source rather than off where it
    // sits.
    if let Some(pairs) = front_matter(&file.text) {
        let has = |k: &str| pairs.iter().any(|(key, _)| key == k);
        let value = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());

        // **W04 used to be opt in, and nothing opted in.**
        //
        // The condition was `has("source") || has("type")`: two front matter keys a writer
        // adds or does not, so the evidence ruler graded only the notes whose author had
        // already decided to be graded. Measured on 2026-09-08: 204 knowledge notes across
        // the fleet, 17 carrying a front matter `source`. Cosimo's base was 73 notes with
        // zero of either, so the 63 sources read into it in one week were never graded at
        // all, and `kb check` printed nothing about any of them. A rule nothing triggers is
        // not a rule.
        //
        // **The new condition is a fact about the body, not about optional front matter: the
        // note cites something.** A `[src:KEY]` pointer, or the hand written `Sources, ...:`
        // paragraph that predates pointers. A note that cites a source is making a sourced
        // claim by definition, and that is exactly the population the evidence ruler was
        // written for.
        //
        // The alternative considered and refused was **every note in the knowledge folder**.
        // It is the strongest rule and it fires on 188 of 204 notes on the day it lands,
        // which turns `kb check` into a wall the reader learns to scroll past. This
        // repository already recorded that failure once, in the note above about why E03
        // was removed. The condition here fires on the 29 that cite in prose today, which is
        // the population the ruler was already supposed to have caught.
        let cites = !crate::sources::citations(&file.text).is_empty()
            || crate::sources::prose_paragraph(&file.text).is_some();
        if has("source") || has("type") || cites {
            for required in ["evidence_tier", "valid_for"] {
                if !has(required) {
                    findings.push(Finding {
                        level: Level::Warning,
                        code: "W04",
                        file: file.rel.clone(),
                        line: 1,
                        message: format!(
                            "this note cites a source and has no {required}. A sourced claim \
                             that grades itself nowhere is a claim nobody can weigh"
                        ),
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

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("kb-checks-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// The one place the declaration and git can still disagree, since git stopped
    /// being asked: an ignore file somebody wrote by hand that forgets a private
    /// folder. A warning, because git is optional and nothing here publishes.
    #[test]
    fn a_gitignore_that_misses_a_declared_private_folder_is_warned_about() {
        let dir = scratch("w08");
        std::fs::write(dir.join("agent.txt"), "name = Probe\nprivate = drafts/, profile\n")
            .expect("manifest");
        std::fs::write(dir.join(".gitignore"), "# mine\n.kb/\n/profile/\n").expect("ignores");
        std::fs::create_dir_all(dir.join("knowledge")).expect("mkdir");

        let base = Base::discover(&dir, true).expect("discover");
        let mut findings = Vec::new();
        check_ignores(&base, &mut findings);

        assert_eq!(findings.len(), 1, "{:?}", findings.iter().map(|f| &f.message).collect::<Vec<_>>());
        assert_eq!(findings[0].code, "W08");
        assert!(findings[0].message.contains("`drafts/`"), "{}", findings[0].message);
        assert!(matches!(findings[0].level, Level::Warning), "git is optional, so never an error");
    }

    #[test]
    fn no_gitignore_is_no_promise_and_no_warning() {
        let dir = scratch("w08-none");
        std::fs::write(dir.join("agent.txt"), "name = Probe\n").expect("manifest");
        std::fs::create_dir_all(dir.join("knowledge")).expect("mkdir");

        let base = Base::discover(&dir, true).expect("discover");
        let mut findings = Vec::new();
        check_ignores(&base, &mut findings);
        assert!(findings.is_empty(), "a base that never mentioned git owes git nothing");
    }

    // -----------------------------------------------------------------------
    // Citations
    // -----------------------------------------------------------------------

    /// A base on disk with one source record and one note, so the citation checks run
    /// against the same two files a person would have.
    fn base_with_a_source(name: &str, note_body: &str) -> (std::path::PathBuf, String) {
        let dir = scratch(name);
        std::fs::write(dir.join("agent.txt"), "name = Probe\n").expect("manifest");
        std::fs::create_dir_all(dir.join("knowledge")).expect("mkdir");
        std::fs::create_dir_all(dir.join(crate::sources::DIR)).expect("mkdir");
        std::fs::write(
            dir.join(crate::sources::DIR).join("K7M2QX4BTF.txt"),
            "type = article\ntitle = A Theory of Human Motivation\nauthor = Maslow, A. H.\n\
             year = 1943\nretrieved_on = 2026-09-06\nretrieval_status = full\n",
        )
        .expect("record");
        std::fs::write(
            dir.join("knowledge").join("note.md"),
            format!("**Search for:** `a`\n\n{note_body}\n"),
        )
        .expect("note");
        let expected = "Maslow, A. H. \"A Theory of Human Motivation\". 1943. \
                        read in full, 2026-09-06."
            .to_string();
        (dir, expected)
    }

    fn codes(dir: &std::path::Path) -> Vec<String> {
        let base = Base::discover(dir, false).expect("discover");
        run(&base).into_iter().map(|f| f.code.to_string()).collect()
    }

    /// The shape that is supposed to be clean: a pointer in the prose and one bibliography
    /// line rendering the record it points at.
    #[test]
    fn a_pointer_with_a_matching_bibliography_line_is_clean() {
        let (dir, expected) = base_with_a_source(
            "cite-ok",
            "Maslow is [src:K7M2QX4BTF].\n\n## Sources\n\n- [src:K7M2QX4BTF] PLACEHOLDER",
        );
        let note = dir.join("knowledge").join("note.md");
        let text = std::fs::read_to_string(&note).expect("read").replace("PLACEHOLDER", &expected);
        std::fs::write(&note, text).expect("write");

        let found = codes(&dir);
        assert!(
            !found.iter().any(|c| c.starts_with('E')),
            "expected no errors, got {found:?}"
        );
    }

    #[test]
    fn a_citation_whose_key_has_no_record_is_an_error() {
        let (dir, _) = base_with_a_source("cite-dead", "See [src:AAAAAAAAAA].");
        let base = Base::discover(&dir, false).expect("discover");
        let dead: Vec<_> = run(&base).into_iter().filter(|f| f.code == "E05").collect();
        assert_eq!(dead.len(), 1, "one unresolved key");
        assert!(dead[0].message.contains("sources/AAAAAAAAAA.txt"), "{}", dead[0].message);
        assert!(matches!(dead[0].level, Level::Error));
    }

    /// A key that is not a key at all takes the same code and a different sentence, because
    /// the fix is different: one is a missing file, the other is a typo.
    #[test]
    fn a_malformed_key_is_the_same_error_with_a_different_reason() {
        let (dir, _) = base_with_a_source("cite-malformed", "See [src:maslow1943].");
        let base = Base::discover(&dir, false).expect("discover");
        let dead: Vec<_> = run(&base).into_iter().filter(|f| f.code == "E05").collect();
        assert_eq!(dead.len(), 1);
        assert!(dead[0].message.contains("is not a key"), "{}", dead[0].message);
    }

    /// **The check that keeps the embedded copy from being a fourth restatement.** Without
    /// it, a note could carry a bibliography line saying anything at all, which is the state
    /// the prose paragraphs were already in.
    #[test]
    fn a_bibliography_line_that_drifted_from_the_record_is_an_error() {
        let (dir, _) = base_with_a_source(
            "cite-drift",
            "See [src:K7M2QX4BTF].\n\n- [src:K7M2QX4BTF] Maslow, A. H. \"A Theory\". 1965.",
        );
        let base = Base::discover(&dir, false).expect("discover");
        let drift: Vec<_> = run(&base).into_iter().filter(|f| f.code == "E08").collect();
        assert_eq!(drift.len(), 1);
        assert!(drift[0].message.contains("has drifted"), "{}", drift[0].message);
        assert!(drift[0].message.contains("1943"), "the message carries the fix");
    }

    #[test]
    fn citing_a_source_with_no_bibliography_line_at_all_is_an_error() {
        let (dir, expected) = base_with_a_source("cite-nobib", "See [src:K7M2QX4BTF].");
        let base = Base::discover(&dir, false).expect("discover");
        let missing: Vec<_> = run(&base).into_iter().filter(|f| f.code == "E08").collect();
        assert_eq!(missing.len(), 1);
        assert!(missing[0].message.contains(&expected), "{}", missing[0].message);
    }

    #[test]
    fn a_source_no_note_cites_is_an_error_against_the_record() {
        let (dir, _) = base_with_a_source("cite-orphan", "A note that cites nothing.");
        let base = Base::discover(&dir, false).expect("discover");
        let orphan: Vec<_> = run(&base).into_iter().filter(|f| f.code == "E06").collect();
        assert_eq!(orphan.len(), 1);
        assert_eq!(orphan[0].file, "sources/K7M2QX4BTF.txt");
        assert!(matches!(orphan[0].level, Level::Error));
    }

    /// `dc:replaces` in one test: the superseded record keeps resolving, it stops being
    /// reported as uncited, and the note that still points at it is told what replaced it.
    #[test]
    fn a_superseded_source_still_resolves_and_stops_being_an_orphan() {
        let (dir, _) = base_with_a_source("cite-replaces", "Old reading: [src:K7M2QX4BTF].");
        std::fs::write(
            dir.join(crate::sources::DIR).join("BBBBBBBBBB.txt"),
            "type = article\ntitle = Who Built Maslow's Pyramid\nauthor = Bridgman, T.\n\
             year = 2019\nretrieved_on = 2026-09-06\nretrieval_status = full\n\
             replaces = K7M2QX4BTF\n",
        )
        .expect("record");
        std::fs::write(
            dir.join("knowledge").join("other.md"),
            "**Search for:** `b`\n\nThe correction is [src:BBBBBBBBBB].\n",
        )
        .expect("note");

        let base = Base::discover(&dir, false).expect("discover");
        let findings = run(&base);

        assert!(
            !findings.iter().any(|f| f.code == "E05"),
            "the old key must keep resolving, which is the whole point of dc:replaces"
        );
        assert!(
            !findings.iter().any(|f| f.code == "E06"),
            "a superseded record is exempt from the orphan rule, or a correction costs a \
             check failure the day the last note moves off it"
        );
        let superseded: Vec<_> = findings.iter().filter(|f| f.code == "W09").collect();
        assert_eq!(superseded.len(), 1);
        assert!(superseded[0].message.contains("BBBBBBBBBB"), "{}", superseded[0].message);
        assert!(matches!(superseded[0].level, Level::Warning), "a correction is not a break");
    }

    #[test]
    fn a_file_in_sources_that_is_not_a_record_is_reported_rather_than_ignored() {
        let (dir, _) = base_with_a_source("cite-badrecord", "See [src:K7M2QX4BTF].");
        std::fs::write(dir.join(crate::sources::DIR).join("CCCCCCCCCC.txt"), "type = tweet\n")
            .expect("write");
        let base = Base::discover(&dir, false).expect("discover");
        let bad: Vec<_> = run(&base).into_iter().filter(|f| f.code == "E09").collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].file, "sources/CCCCCCCCCC.txt");
    }

    /// W04's condition is a fact about the body now. The note here declares neither `source`
    /// nor `type` in front matter, which is exactly the shape that went ungraded 73 times.
    #[test]
    fn a_note_that_cites_a_source_is_graded_even_with_no_source_line_in_front_matter() {
        let file = md(
            "knowledge/n.md",
            "---\nprovenance: agent\nstage: derived\n---\n\n**Search for:** `a`\n\n\
             See [src:K7M2QX4BTF].\n",
        );
        let mut findings = Vec::new();
        check_note(&four_files(), &file, &mut findings);
        let w04: Vec<_> = findings.iter().filter(|f| f.code == "W04").collect();
        assert_eq!(w04.len(), 2, "evidence_tier and valid_for");
    }

    /// The 29 notes that cite in prose today are the population this was written to reach,
    /// so the old shape has to trigger it too or the change grades nothing until a retrofit.
    #[test]
    fn a_prose_sources_paragraph_triggers_the_evidence_rules_as_well() {
        let file = md(
            "knowledge/n.md",
            "---\nprovenance: agent\nstage: derived\n---\n\n**Search for:** `a`\n\n\
             Sources, read 2026-09-06: Maslow, A. H., Psychological Review 50.\n",
        );
        let mut findings = Vec::new();
        check_note(&four_files(), &file, &mut findings);
        assert_eq!(findings.iter().filter(|f| f.code == "W04").count(), 2);
    }

    #[test]
    fn a_note_that_cites_nothing_is_not_asked_for_a_tier() {
        let file = md(
            "knowledge/n.md",
            "---\nprovenance: agent\nstage: derived\n---\n\n**Search for:** `a`\n\nPlain prose.\n",
        );
        let mut findings = Vec::new();
        check_note(&four_files(), &file, &mut findings);
        assert!(findings.iter().all(|f| f.code != "W04"), "{:?}", findings.len());
    }

    /// House style is about our prose. A book's real title belongs to somebody else, and
    /// `write::note` refuses a whole note on a dash, so without the exemption a source
    /// titled with one could be recorded and never cited.
    #[test]
    fn a_dash_inside_a_bibliography_line_is_not_a_house_style_finding() {
        let text = "our prose \u{2014} with a dash\n- [src:K7M2QX4BTF] Someone. \
                    \"Title \u{2014} Subtitle\". 2019. read in full, 2026-09-06.";
        let found = dashes(text);
        assert_eq!(found.len(), 1, "only our own line is reported");
        assert_eq!(found[0].0, 1);
    }

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

    fn md(rel: &str, text: &str) -> MdFile {
        MdFile {
            rel: rel.into(),
            stem: rel.rsplit('/').next().unwrap().trim_end_matches(".md").into(),
            text: text.into(),
            private: false,
        }
    }

    fn four_files() -> Base {
        Base {
            root: std::path::PathBuf::from("probe"),
            map: None,
            knowledge_dir: Some("knowledge".into()),
            files: vec![
                md("knowledge/keyed.md", "# Keyed\n\n**Search for:** `alpha`, `beta`\n"),
                md("knowledge/keyless.md", "# Keyless\n\njust prose.\n"),
                md("README.md", "# Readme\n\nthe front door.\n"),
                md("inbox/2026-09-01-session-x.md", "# Deposit\n\nwhat a session left.\n"),
            ],
            unreadable: Vec::new(),
            aliases: Vec::new(),
            all: false,
        }
    }

    /// **One predicate, read from one place, or the two outputs answer the same question
    /// with two numbers.**
    ///
    /// E02 and the unreachable count `kb index` prints ask whether the router can build an
    /// entry for a file, and the exemption is most of that answer: measured on this fleet,
    /// all 63 keyless files are exempt by name and E02 reports zero. A count that tested
    /// only for an empty keyword list would have shipped printing 63 against the linter's
    /// 0, and a reader would have had no way to know which of the two was lying.
    #[test]
    fn the_index_and_the_linter_agree_about_which_files_are_unreachable() {
        let base = four_files();

        let linter: std::collections::BTreeSet<String> = run(&base)
            .into_iter()
            .filter(|f| f.code == "E02")
            .map(|f| f.file)
            .collect();
        let index: std::collections::BTreeSet<String> =
            crate::index::build(&base).unreachable.into_iter().collect();

        assert_eq!(linter, index, "the index and the linter disagree");
        assert_eq!(
            linter,
            std::collections::BTreeSet::from(["knowledge/keyless.md".to_string()]),
            "the keyless note is the only defect: README and inbox/ are exempt"
        );
    }
}
