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
///
/// That reasoning holds where the fleet is on a disk somebody owns, and only there.
/// See [`MISSES_PATH_ENV`] for the machine where it does not.
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

/// One line of the log, as the writer wrote it and as the reader reads it back.
///
/// `Debug` and nothing else. The struct is the log's schema: a derive is cheap, but a
/// change to these fields changes what [`render`] writes and what every log already on
/// disk can be read back as.
#[derive(Debug)]
pub struct Miss {
    pub count: u32,
    pub first: String,
    pub last: String,
    pub question: String,
    pub looked_like: Vec<String>,
}

/// The variable that moves the log somewhere writable.
///
/// **Named because the default cannot work everywhere.** The log lives beside the
/// fleet on purpose, and that is right on a machine somebody owns. A hosted consumer
/// has a read only filesystem and one writable directory that is not beside anything,
/// so without this the recall loss log on the surface with the most real questions in
/// it is empty. F-03 in `reports/2026-08-29-first-integration.md`.
///
/// It names a file and not a directory, so nothing is guessed. Two fleets pointed at
/// one path share one log, which is the caller's decision to make and is why the path
/// is theirs to write rather than ours to derive.
pub const MISSES_PATH_ENV: &str = "KB_MISSES_PATH";

/// Where the log goes, given the override or the absence of one.
///
/// Pure, so the branch can be tested without a process wide variable racing every
/// other test in the binary. [`path_in`] is the one line that reads the environment.
pub fn path_for(root: &Path, override_path: Option<&str>) -> PathBuf {
    match override_path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => PathBuf::from(p),
        // An empty variable is a platform that exported the name and chose no value,
        // which is not the same as choosing a path and is not worth honouring.
        None => root.join(MISSES_TXT),
    }
}

pub fn path_in(root: &Path) -> PathBuf {
    path_for(root, std::env::var(MISSES_PATH_ENV).ok().as_deref())
}

/// The marker beside the log while one writer has it. Follows the log, so an override
/// moves both.
fn lock_path(log: &Path) -> PathBuf {
    let mut p = log.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

/// How long to wait for another writer before giving up, and how old a marker has to
/// be before it is debris. A record is one read and one write of a small file, so a
/// writer that holds the marker for longer than a few seconds is a writer that died.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(500);
const LOCK_STALE: std::time::Duration = std::time::Duration::from_secs(30);

/// Held while the log is read, merged and rewritten. Dropping it releases it.
///
/// **Why a lock, and why now.** `record` is read, merge, write, and it was fine while
/// the writers were a person at a terminal and one serve process. ADR-0035 puts it on
/// the boot hook, which runs on every message of every session, and two sessions that
/// end a message in the same instant both read the file, both merge their line, and the
/// one that writes second erases the first. The mechanism is `create_new`, `O_EXCL` on
/// Unix and `CREATE_NEW` on Windows: the file system decides the race, so two processes
/// cannot both believe they made the marker. Same shape as `promote::Lock`, smaller,
/// because a record is milliseconds and a promotion is minutes.
///
/// **Shared with [`crate::abstain`] rather than copied.** That log is written from the same
/// hook, on the same message, and loses rows to the same race. The mechanism is "hold a
/// marker beside this file while merging" and is not specific to which file, so a second
/// copy of it would be a second place for the stale-marker rule to drift.
pub(crate) struct Guard(PathBuf);

impl Guard {
    pub(crate) fn take(log: &Path) -> Result<Guard, String> {
        let path = lock_path(log);
        let started = std::time::Instant::now();
        loop {
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Guard(path)),
                Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                    return Err(format!("could not take {}: {e}", path.display()));
                }
                Err(_) => {}
            }
            // Somebody died holding it: a marker older than any live record could be.
            let stale = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > LOCK_STALE);
            if stale {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            if started.elapsed() > LOCK_WAIT {
                return Err(format!(
                    "another writer held {} for longer than {}ms",
                    path.display(),
                    LOCK_WAIT.as_millis()
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Adds one miss, or bumps the one already there, and rewrites the file sorted.
///
/// Read, merge, write on every miss rather than append. It costs a whole file
/// rewrite, which at the size this reaches is microseconds, and it buys a file that
/// is always sorted, always deduplicated and always readable by a human with `cat`.
///
/// **The file stays the interface for what was lost.** `kb misses` reads it back and
/// does not replace it: everything the log holds, a person can already see with `cat`,
/// and the reader exists for the half the file cannot hold. Which file nearly caught
/// the question, and with which keys, is a fact about the base **as it stands now**,
/// so it changes every time a note is written and cannot be recorded beside a question
/// that was asked last month.
pub fn record(
    root: &Path,
    question: &str,
    looked_like: &[String],
    today: &str,
) -> Result<(), String> {
    let question = question.trim();
    if question.is_empty() {
        return Ok(());
    }

    let path = path_in(root);
    // Taken before the read, because a merge over a stale read is exactly the lost
    // update the lock exists to prevent. Released when this function returns.
    let _held = Guard::take(&path)?;
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
    //
    // **The reason is returned as well as printed, and that is F-03.** stderr from a
    // child process inside a serverless function is where information goes to die: the
    // deployment that found this had a `Permission denied` on every query and a caller
    // that could not see it, because `route` exits 0 and the caller reads stdout. A
    // caller handed the reason can put the loss somewhere else, or at least say out
    // loud that it is not being kept.
    match std::fs::write(&path, render(&seen)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let reason = format!("could not write {}: {e}", path.display());
            eprintln!("kb: {reason}");
            Err(reason)
        }
    }
}

/// Most asked first, then alphabetical so a file with no new misses in it produces no
/// diff.
///
/// **One comparator, because the writer and the reader are two copies of one order.**
/// The expression lived inline in [`render`] and nothing read the file back, so there
/// was nothing to drift from. `load` has to reproduce the file's own order exactly:
/// the count is the worklist, so the order is the payload, and a second copy of this
/// expression is how two orders appear.
fn by_priority(a: &Miss, b: &Miss) -> std::cmp::Ordering {
    b.count.cmp(&a.count).then(a.question.cmp(&b.question))
}

/// The log's text, back as the misses that made it, in the file's own priority order.
///
/// Pure, so the parse can be tested without touching the filesystem or
/// `KB_MISSES_PATH`: see the note on `an_override_moves_the_log_and_an_empty_one_does_not`
/// for why a process wide variable inside a parallel test suite is a race rather than a
/// setup step.
///
/// **The sort is not tidying.** [`parse`] returns a `BTreeMap` keyed on the lowercased
/// question, which is the right key for merging on write and the wrong order on read:
/// `into_values()` alone hands back an alphabetical list and silently destroys the one
/// property the file exists to carry, which is that the most asked question comes first.
pub fn ranked(text: &str) -> Vec<Miss> {
    let mut all: Vec<Miss> = parse(text).into_values().collect();
    all.sort_by(by_priority);
    all
}

/// The log on disk, or an empty list when nobody has missed anything yet.
///
/// **An absent file is `Ok(Vec::new())` and not an error.** A fleet nothing has missed
/// against is not a failure, and `kb misses` runs from a terminal beside a hook that may
/// never have fired. The caller distinguishes the two by asking whether the path exists,
/// which is why it is handed the path rather than a root. Every other io failure comes
/// back as a `String`, the same shape [`record`] returns.
///
/// **Print the path you read.** [`MISSES_PATH_ENV`] can point this at a file beside no
/// fleet at all, and two fleets pointed at one path share one log, so a reader that
/// reports questions without naming the file they came from is reporting something
/// nobody can check.
///
/// One lossy edge, stated because it is deliberate and not fixed here: [`parse`] keys on
/// `question.to_lowercase()` and inserts, so a hand edited log carrying two casings of
/// one question keeps only the last of them. That is correct merging for `record`, whose
/// job is to count a question once however it was typed, and it is a read that loses a
/// line. Fixing it would change what `record` merges, which changes the number both of
/// ADR-0006's and ADR-0013's revisit triggers are measured against.
pub fn load(log: &Path) -> Result<Vec<Miss>, String> {
    match std::fs::read_to_string(log) {
        Ok(text) => Ok(ranked(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("could not read {}: {e}", log.display())),
    }
}

fn render(seen: &BTreeMap<String, Miss>) -> String {
    let mut all: Vec<&Miss> = seen.values().collect();
    all.sort_by(|a, b| by_priority(a, b));

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

    /// The override, tested as a pure function over the value rather than over the
    /// environment. Setting a process wide variable inside a test that runs in
    /// parallel with every other test is a race by construction: the surrounding
    /// tests write miss logs of their own, and one of them reading a variable set for
    /// a different test would send its log somewhere else and fail intermittently.
    ///
    /// So the branch is here, where it is pure, and the one line that reads the
    /// environment is verified by running the binary with the variable set. That run
    /// is in `reports/2026-08-29-first-integration.md`; it is not in this file, and
    /// saying so is the point.
    #[test]
    fn an_override_moves_the_log_and_an_empty_one_does_not() {
        let base = Path::new("/fleet");

        assert_eq!(path_for(base, None), base.join(MISSES_TXT), "beside the base by default");
        assert_eq!(
            path_for(base, Some("/tmp/kb-misses.txt")),
            PathBuf::from("/tmp/kb-misses.txt"),
            "an ephemeral machine points this at the one writable directory it has"
        );
        assert_eq!(
            path_for(base, Some("   ")),
            base.join(MISSES_TXT),
            "a platform that sets the variable to nothing has not chosen a path"
        );
        assert_eq!(path_for(base, Some("")), base.join(MISSES_TXT), "nor has an empty one");
    }

    /// A write that cannot happen is reported rather than swallowed, and the caller
    /// gets the reason. On a read only deployment this is the difference between a
    /// caller that knows the evidence was lost and one that finds out months later
    /// that its recall loss log has two lines in it.
    #[test]
    fn a_write_that_fails_comes_back_with_its_reason() {
        let dir = scratch("unwritable");
        // A directory where the file has to go. `fs::write` fails on Windows and on
        // Linux alike, which a permission bit does not: chmod is a no-op for an
        // administrator on one of them.
        std::fs::create_dir_all(path_in(&dir)).expect("mkdir");

        let outcome = record(&dir, "uma pergunta perdida", &[], "2026-08-30");
        let reason = outcome.expect_err("writing into a directory cannot succeed");
        assert!(!reason.is_empty(), "the caller is handed something it can print");
    }

    /// Two writers, one file, no lost update. Without the lock this loses lines:
    /// read, merge, write, and the second writer's read predates the first writer's
    /// write. The counts have to come out exact, which is the property the boot hook
    /// needs before it is allowed to record on every message.
    #[test]
    fn two_writers_recording_at_once_lose_nothing() {
        let dir = scratch("concurrent");
        let rounds = 20;
        // **Counts what succeeded, not what was attempted, and the difference is the
        // whole guarantee.** The first version of this test asserted a count of exactly
        // `rounds` per question, which made it a test of the timeout as well as of the
        // lock: `LOCK_WAIT` is 500ms, and under the full suite running in parallel two
        // threads contending twenty times each can exceed it, so the test failed in the
        // suite and passed alone. That is the worst kind of test, because it teaches
        // people to re-run rather than to read.
        //
        // A writer that gives up is not a lost update. It returns `Err`, the caller is
        // told, and the evidence is not silently gone: that is the lock behaving. What
        // must never happen is a write that returned `Ok` and is not in the file. So the
        // count asserted is the number of successes, which makes this a test of the
        // merge and of nothing else.
        let run = |question: &'static str| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                (0..rounds).filter(|_| record(&dir, question, &[], "2026-09-01").is_ok()).count()
            })
        };
        let a = run("pergunta de a");
        let b = run("pergunta de b");
        let (ok_a, ok_b) = (a.join().expect("thread a"), b.join().expect("thread b"));

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert!(ok_a > 0 && ok_b > 0, "both writers got in at least once: {ok_a}, {ok_b}");
        assert!(text.contains(&format!("{ok_a:<4} 2026-09-01 2026-09-01 pergunta de a")), "{text}");
        assert!(text.contains(&format!("{ok_b:<4} 2026-09-01 2026-09-01 pergunta de b")), "{text}");
        assert!(!lock_path(&path_in(&dir)).exists(), "the marker is gone when nobody holds it");
    }

    /// A marker somebody died holding is debris, and debris does not switch the log off
    /// forever. A fresh marker is respected and the caller is told, with the reason.
    #[test]
    fn a_stale_marker_is_stepped_over_and_a_live_one_is_respected() {
        let dir = scratch("markers");
        let log = path_in(&dir);
        let marker = lock_path(&log);

        std::fs::write(&marker, "pid 4242\n").expect("plant");
        let reason = record(&dir, "bloqueada", &[], "2026-09-01").expect_err("a live marker wins");
        assert!(reason.contains("another writer held"), "{reason}");

        // Now make it old enough to be debris.
        let old = std::time::SystemTime::now() - LOCK_STALE - std::time::Duration::from_secs(5);
        let f = std::fs::OpenOptions::new().write(true).open(&marker).expect("open");
        f.set_modified(old).expect("backdate");
        drop(f);
        record(&dir, "liberada", &[], "2026-09-01").expect("a stale marker is stepped over");
        assert!(std::fs::read_to_string(&log).expect("read").contains("liberada"));
    }

    #[test]
    fn a_write_that_succeeds_says_so() {
        let dir = scratch("writable");
        assert!(record(&dir, "uma pergunta perdida", &[], "2026-08-30").is_ok());
    }

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
        record(&dir, "quanto de disco livre", &[], "2026-08-17").expect("write");
        record(&dir, "quanto de disco livre", &[], "2026-08-18").expect("write");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert_eq!(text.matches("quanto de disco livre").count(), 1, "{text}");
        assert!(text.contains("2    2026-08-17 2026-08-18"), "{text}");
    }

    /// The count is the worklist, so the order has to be the priority.
    #[test]
    fn the_most_asked_question_is_first() {
        let dir = scratch("order");
        record(&dir, "asked once", &[], "2026-08-17").expect("write");
        record(&dir, "asked twice", &[], "2026-08-17").expect("write");
        record(&dir, "asked twice", &[], "2026-08-17").expect("write");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        let twice = text.find("asked twice").expect("present");
        let once = text.find("asked once").expect("present");
        assert!(twice < once, "most asked first:\n{text}");
    }

    #[test]
    fn what_the_base_offered_back_survives_a_round_trip() {
        let dir = scratch("suggestions");
        record(&dir, "protocolo de ingestao", &["ingest a source".into()], "2026-08-17").expect("write");
        record(&dir, "protocolo de ingestao", &["ingest a source".into()], "2026-08-18").expect("write");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert_eq!(text.matches("ingest a source").count(), 1, "{text}");
        assert!(text.contains("2    "), "the count survived the reparse:\n{text}");
    }

    /// A question containing whatever a person types must come back unchanged, and
    /// the count field must not be confused by spacing inside it.
    // -----------------------------------------------------------------------
    // Reading the log back
    //
    // The writer had tests and the reader had none, because there was no reader.
    // These pin the four properties `kb misses` depends on: the round trip, the
    // order, the absent file, and the header.
    // -----------------------------------------------------------------------

    /// [`load`] is the inverse of [`record`], on all five fields.
    ///
    /// The two that break quietly are the indented `looked like:` line, which a
    /// reader that only splits on whitespace drops on the floor, and the question's
    /// own internal spacing, which a reader that normalises silently turns into a
    /// different question from the one a person typed. Both are the class of bug
    /// `three_fields_then_rest` was written for, from the other direction.
    #[test]
    fn a_log_the_writer_produced_reads_back_as_the_misses_that_made_it() {
        let dir = scratch("readback");
        record(&dir, "protocolo  de  ingestao", &["ingest a source".into()], "2026-08-17")
            .expect("write");
        record(&dir, "protocolo  de  ingestao", &["ingest a source".into()], "2026-08-18")
            .expect("write");
        record(&dir, "quanto de disco livre", &[], "2026-08-18").expect("write");

        let all = load(&path_in(&dir)).expect("the log reads back");
        assert_eq!(all.len(), 2, "two distinct questions: {all:?}");

        let top = &all[0];
        assert_eq!(top.count, 2, "{top:?}");
        assert_eq!(top.first, "2026-08-17", "{top:?}");
        assert_eq!(top.last, "2026-08-18", "{top:?}");
        assert_eq!(top.question, "protocolo  de  ingestao", "the spacing a person typed: {top:?}");
        assert_eq!(top.looked_like, vec!["ingest a source".to_string()], "{top:?}");
    }

    /// Most asked first survives the read.
    ///
    /// This is the test the obvious implementation fails. [`parse`] returns a
    /// `BTreeMap` keyed on the lowercased question, so `parse(text).into_values()`
    /// hands back **alphabetical** order and puts "asked once" in front of a question
    /// that missed twice. The count is the worklist, so the order is the payload.
    #[test]
    fn the_reader_keeps_the_files_priority_order_and_not_the_maps() {
        let dir = scratch("read-order");
        record(&dir, "asked once", &[], "2026-08-17").expect("write");
        record(&dir, "asked twice", &[], "2026-08-17").expect("write");
        record(&dir, "asked twice", &[], "2026-08-17").expect("write");

        let all = load(&path_in(&dir)).expect("read");
        assert_eq!(all[0].question, "asked twice", "most asked first, not alphabetical: {all:?}");
        assert_eq!(all[1].question, "asked once", "{all:?}");
    }

    /// A fleet nobody has missed against is not a failure.
    ///
    /// `kb misses` is run from a terminal beside a hook that may never have fired, so
    /// an absent log has to exit 0 and say nothing was lost. An `Err` here would make
    /// the healthy case look like a broken one.
    #[test]
    fn a_log_that_was_never_written_reads_as_no_misses_rather_than_an_error() {
        let dir = scratch("never-written");
        let all = load(&path_in(&dir)).expect("no log is not an error");
        assert!(all.is_empty(), "{all:?}");
    }

    /// The header is not data, and it looks enough like data to be tried.
    ///
    /// `# One line per distinct question: count, first seen, last seen, then the
    /// question.` is three whitespace separated fields followed by text, which is
    /// exactly the shape [`three_fields_then_rest`] accepts. Only the comment skip
    /// keeps it out, so the skip is pinned here rather than trusted.
    #[test]
    fn the_shipped_header_is_not_read_as_a_miss() {
        assert!(ranked(HEADER).is_empty(), "the header is comments and blank lines");

        let with_one = format!("{HEADER}
1    2026-08-17 2026-08-17 uma pergunta perdida
");
        let all = ranked(&with_one);
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!(all[0].question, "uma pergunta perdida", "{all:?}");
    }

    #[test]
    fn a_question_with_odd_spacing_round_trips() {
        let dir = scratch("odd");
        record(&dir, "por que  o  cargo test falhou?", &[], "2026-08-17").expect("write");
        record(&dir, "por que  o  cargo test falhou?", &[], "2026-08-17").expect("write");

        let text = std::fs::read_to_string(path_in(&dir)).expect("read");
        assert!(text.contains("por que  o  cargo test falhou?"), "{text}");
        assert!(text.contains("2    "), "counted rather than duplicated:\n{text}");
    }
}
