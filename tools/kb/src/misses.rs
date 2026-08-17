//! The log of questions the free stage could not answer.
//!
//! [ADR-0006](../../../decisions/0006-language-architecture.md) says the expansion log is the
//! worklist for improving step 1, and [ADR-0013](../../../decisions/0013-retrieval-precedes-classification.md)
//! sharpens what it is: a **recall loss log**. Every line is a question the base
//! failed to answer for free. Read as a keyword worklist it looks like maintenance.
//! Read as recall loss it is the only thing that says whether this design converges.
//!
//! That is the measurement both of today's revisit triggers are written against, and
//! it is why this exists at all: an ADR whose trigger nobody can check has the same
//! value as an ADR with no trigger.
//!
//! **Distinct questions, counted, not an append stream.** Growth is bounded by how
//! many different things get asked rather than by how often, and the count is the
//! priority: a question that missed five times earns an alias line before one that
//! missed once. An append log has neither property and has to be aggregated before
//! anyone can act on it.
//!
//! **Never written for a base with no entries.** A miss against an empty library is
//! not a recall loss, it is a base nobody has filled in yet, and counting it here
//! would corrupt the one number this file exists to produce.
//!
//! **Private by construction, and that is not a detail.** These are the user's real
//! questions, verbatim. A Yaron miss is a health question. The log is therefore
//! gitignored wherever it lands, and what gets committed is the alias line a miss
//! eventually earns, never the miss.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Lives beside `fleet.txt` rather than in `.kb/`, because `.kb/` is derived and
/// disposable per ADR-0003 and this is neither. Deleting the index should lose a
/// rebuild; deleting this loses evidence that cannot be recomputed.
pub const MISSES_TXT: &str = "kb-misses.txt";

const HEADER: &str = "\
# Questions the free stage could not answer, most asked first.
#
# One line per distinct question: count, first seen, last seen, then the question.
# An indented `looked like:` line records what the base offered back, so it is
# visible whether the free suggestion helped or whether a translation was needed.
#
# This is a recall loss log, not a list of chores. Every line is a question that
# reached nothing, and the count is which one to fix next: write the alias line or
# the Search for term that would have caught it, and delete the line.
#
# Not committed anywhere. These are real questions asked by a real person.
";

pub struct Miss {
    pub count: u32,
    pub first: String,
    pub last: String,
    pub question: String,
    pub looked_like: Vec<String>,
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(MISSES_TXT)
}

/// Adds one miss, or bumps the one already there, and rewrites the file sorted.
///
/// Read, merge, write on every miss rather than append. It costs a whole file
/// rewrite, which at the size this reaches is microseconds, and it buys a file that
/// is always sorted, always deduplicated and always readable by a human with `cat`.
/// **The file is the interface**, so no subcommand exists to render it.
pub fn record(root: &Path, question: &str, looked_like: &[String], today: &str) {
    let question = question.trim();
    if question.is_empty() {
        return;
    }

    let path = path_in(root);
    let mut seen = parse(&std::fs::read_to_string(&path).unwrap_or_default());

    let key = question.to_lowercase();
    match seen.get_mut(&key) {
        Some(existing) => {
            existing.count += 1;
            existing.last = today.to_string();
            // The suggestions can change as the base grows, and the newest ones are
            // the ones that describe the base as it is now.
            existing.looked_like = looked_like.to_vec();
        }
        None => {
            seen.insert(
                key,
                Miss {
                    count: 1,
                    first: today.to_string(),
                    last: today.to_string(),
                    question: question.to_string(),
                    looked_like: looked_like.to_vec(),
                },
            );
        }
    }

    // A miss log that cannot be written is not worth failing a query over. The
    // question still gets answered; only the evidence is lost, and saying so on
    // stderr is louder than a silent swallow without being fatal.
    if let Err(e) = std::fs::write(&path, render(&seen)) {
        eprintln!("kb: could not write {}: {e}", path.display());
    }
}

fn render(seen: &BTreeMap<String, Miss>) -> String {
    let mut all: Vec<&Miss> = seen.values().collect();
    all.sort_by(|a, b| b.count.cmp(&a.count).then(a.question.cmp(&b.question)));

    let mut out = String::from(HEADER);
    for m in all {
        out.push_str(&format!(
            "\n{:<4} {} {} {}\n",
            m.count, m.first, m.last, m.question
        ));
        if !m.looked_like.is_empty() {
            out.push_str(&format!("     looked like: {}\n", m.looked_like.join(", ")));
        }
    }
    out
}

/// Three whitespace separated fields, then the whole rest of the line untouched.
///
/// Written by hand because `splitn(4, ..)` cannot do it: the count is written
/// padded, so `splitn` spends its budget on the padding and the question ends up
/// in the third field. Filtering the empty pieces afterwards does not help, since
/// by then the split has already stopped. **The symptom was a log that counted
/// every question as new**, because nothing it wrote could be read back.
///
/// The question keeps its own internal spacing, so a question a person typed with
/// two spaces in it round trips as itself rather than as a normalised version.
fn three_fields_then_rest(line: &str) -> Option<(&str, &str, &str, &str)> {
    let mut rest = line;
    let mut fields = ["", "", ""];
    for field in fields.iter_mut() {
        rest = rest.trim_start();
        let end = rest.find(char::is_whitespace)?;
        *field = &rest[..end];
        rest = &rest[end..];
    }
    let question = rest.trim_start();
    if question.is_empty() {
        return None;
    }
    Some((fields[0], fields[1], fields[2], question))
}

fn parse(text: &str) -> BTreeMap<String, Miss> {
    let mut out: BTreeMap<String, Miss> = BTreeMap::new();
    let mut last_key: Option<String> = None;

    for line in text.lines() {
        if line.trim_start().starts_with("looked like:") {
            if let Some(key) = &last_key {
                if let Some(m) = out.get_mut(key) {
                    m.looked_like = line
                        .trim_start()
                        .trim_start_matches("looked like:")
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let (count, first, last, question) = match three_fields_then_rest(line) {
            Some(parts) => parts,
            None => continue,
        };
        let count: u32 = match count.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let key = question.to_lowercase();
        out.insert(
            key.clone(),
            Miss {
                count,
                first: first.to_string(),
                last: last.to_string(),
                question: question.to_string(),
                looked_like: Vec::new(),
            },
        );
        last_key = Some(key);
    }
    out
}

// ---------------------------------------------------------------------------
// The date, written here rather than taken as a dependency
// ---------------------------------------------------------------------------

/// Today as `YYYY-MM-DD`, from the system clock.
///
/// Hand written for the same reason `json.rs` is: one small algorithm is cheaper
/// than a dependency, and this one is a closed form that has been correct since
/// 1582. A log a human reads has to carry readable dates, and git cannot supply
/// them here because the file is never committed.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since the Unix epoch to a civil date, by Howard Hinnant's algorithm.
///
/// Shifts the year to start in March so that the leap day lands at the end of the
/// year and every month length becomes a linear function, which is what removes
/// the table and the branches.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kb-misses-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn the_epoch_and_a_leap_day_convert() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(59), (1970, 3, 1), "1970 is not a leap year");
        assert_eq!(civil_from_days(19_782), (2024, 2, 29), "a real leap day");
        assert_eq!(civil_from_days(20_678), (2026, 8, 13));
    }

    #[test]
    fn today_is_a_readable_date() {
        let t = today();
        assert_eq!(t.len(), 10, "{t}");
        assert_eq!(t.chars().filter(|c| *c == '-').count(), 2, "{t}");
    }

    #[test]
    fn the_same_question_is_counted_rather_than_repeated() {
        let dir = scratch("counted");
        record(&dir, "quanto de disco livre", &[], "2026-08-17");
        record(&dir, "quanto de disco livre", &[], "2026-08-18");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert_eq!(text.matches("quanto de disco livre").count(), 1, "{text}");
        assert!(text.contains("2    2026-08-17 2026-08-18"), "{text}");
    }

    /// The count is the worklist, so the order has to be the priority.
    #[test]
    fn the_most_asked_question_is_first() {
        let dir = scratch("order");
        record(&dir, "asked once", &[], "2026-08-17");
        record(&dir, "asked twice", &[], "2026-08-17");
        record(&dir, "asked twice", &[], "2026-08-17");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        let twice = text.find("asked twice").expect("present");
        let once = text.find("asked once").expect("present");
        assert!(twice < once, "most asked first:\n{text}");
    }

    #[test]
    fn what_the_base_offered_back_survives_a_round_trip() {
        let dir = scratch("suggestions");
        record(&dir, "protocolo de ingestao", &["ingest a source".into()], "2026-08-17");
        record(&dir, "protocolo de ingestao", &["ingest a source".into()], "2026-08-18");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert_eq!(text.matches("ingest a source").count(), 1, "{text}");
        assert!(text.contains("2    "), "the count survived the reparse:\n{text}");
    }

    /// A question containing whatever a person types must come back unchanged, and
    /// the count field must not be confused by spacing inside it.
    #[test]
    fn a_question_with_odd_spacing_round_trips() {
        let dir = scratch("odd");
        record(&dir, "por que  o  cargo test falhou?", &[], "2026-08-17");
        record(&dir, "por que  o  cargo test falhou?", &[], "2026-08-17");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert!(text.contains("por que  o  cargo test falhou?"), "{text}");
        assert!(text.contains("2    "), "counted rather than duplicated:\n{text}");
    }
}
