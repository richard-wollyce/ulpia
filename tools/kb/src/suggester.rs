//! What the base offers back when it has already refused, and the one place a second
//! way of measuring "looks like what was asked" is allowed to land.
//!
//! **This module adds no model, no dependency and no behaviour.** It is a seam with
//! exactly one implementation, [`Trigram`], whose body is the call to `index::suggest`
//! that `Memory::suggest` used to make directly. The reason to give it a name anyway is
//! that "a second scorer lands here" was a fact about the call graph that no signature
//! stated and no test protected, so the first person to want one would have had to
//! rediscover where it is safe to put it.
//!
//! **Where it is safe is decided by ordering, not by this type.** A suggester runs
//! strictly after the gate, at every surface: `Memory::recall_loss` returns `None` for
//! anything but `Verdict::Nothing` before it asks for words, and `confidence_of` reads
//! only `index::Hit::score` and `store::Hit`, neither of which a suggester can reach.
//! So the worst a wrong suggestion can do is cost a reader one wasted retry after being
//! told the base does not cover the question. That is the whole argument for allowing a
//! model here and nowhere else, and it is an argument about the call graph: `suggest` is
//! `pub`, so a future caller could feed these words back into a query before the gate
//! runs, and nothing in the compiler stops it. Do not read this trait as a guarantee it
//! does not make.
//!
//! ## The bar a second implementation clears, before it is written
//!
//! [[0018-no-model-in-the-retrieval-path]] measured six model configurations against the
//! keyword scorer and kept none: both rerankers degraded the ranking they were handed,
//! and **no model produced a usable abstention signal**, which is the one property this
//! system sells. Its revisit trigger names what would change that, and it is quoted here
//! rather than summarised because a summary is how a bar gets lowered:
//!
//! > An embedding or reranking model appears that is Apache/MIT licensed, under 1 GB,
//! > and demonstrates hit/miss separation on someone else's benchmark, which is the one
//! > property no candidate had today.
//!
//! Someone else's benchmark, because ours was written by the person who tuned the keys
//! against it and its bias is stated in its own header. Choosing that model is a
//! measurement nobody has made, so this module ships the seam and stops there.

/// One way of answering "what does this base know that looks like what was asked".
///
/// **`Send` is load bearing and `cargo test` here would not tell you.** The tray holds
/// `Mutex<Option<Memory>>` inside the state it hands to Tauri's `manage`, which is bounded
/// `Send + Sync + 'static`, and `Mutex<T>: Sync` needs `T: Send`. A boxed trait object is
/// `Send` only if its trait says so, and `fleet-tray` is a separate crate depending on
/// `kb` by path, so no green run here builds it. `Sync` is not required by that bound and
/// is kept anyway: `Memory` is not `Sync` today, because `Store` holds a `rusqlite`
/// connection with a `RefCell` in it, and a suggester should not be the second reason.
/// Both are pinned by a test in this module rather than left to the tray to discover.
pub trait Suggester: Send + Sync {
    /// Terms from the base's own vocabulary, best first, at most `limit` of them.
    /// Empty is a normal answer and means the base has nothing that looks like this.
    fn words(&self, question: &str, entries: &[crate::index::Entry], limit: usize) -> Vec<String>;
}

/// Trigram overlap over the keyword lines. The only implementation, and the one that has
/// been running all along.
///
/// A zero sized type, so the boxed field on `Memory` allocates nothing measurable and the
/// default costs a base nothing to open.
///
/// It measures *spelling*, which is half of [[0006-language-architecture]]'s step 2 and
/// says so: a typo and a cognate are orthographic distance and land here, a translation
/// is semantic distance and never will. The reasoning, the floor and the seven cases that
/// pin it stay with [`crate::index::suggest`], which is unchanged and still `pub`.
pub struct Trigram;

impl Suggester for Trigram {
    fn words(&self, question: &str, entries: &[crate::index::Entry], limit: usize) -> Vec<String> {
        crate::index::suggest(question, entries, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{self, Entry};

    fn entry(stem: &str, keywords: &[&str]) -> Entry {
        Entry {
            base: "zed".into(),
            rel: format!("knowledge/{stem}.md"),
            stem: stem.into(),
            title: stem.into(),
            keywords: keywords.iter().map(|k| k.to_string()).collect(),
            summary: String::new(),
            body: String::new(),
        }
    }

    /// **The supertrait, checked here because the crate that needs it is not built here.**
    ///
    /// The tray holds `Mutex<Option<Memory>>` inside the state it hands to Tauri's
    /// `manage`, which is bounded `Send + Sync + 'static`, and `Mutex<T>: Sync` needs
    /// `T: Send`. So `Memory` must be `Send`, and a boxed trait object is `Send` only if
    /// the trait says so. `fleet-tray` is a separate crate depending on `kb` by path, so
    /// a green `cargo test` here builds none of it: dropping `Send` from the trait would
    /// fail somebody's tray build a week later and nothing in this suite would notice.
    ///
    /// **`Memory` is deliberately not asserted `Sync`, because it is not.** Written that
    /// way first and the compiler refused it: `Store` holds a `rusqlite::Connection`,
    /// which is a `RefCell` inside, so `Memory` crosses to a thread and is not shared
    /// between two. That is what the tray's mutex is for. `Sync` stays on the trait
    /// anyway, so a suggester is never the reason that changes.
    #[test]
    fn the_contract_still_crosses_a_thread_because_the_tray_needs_it_to() {
        fn crosses<T: Send>() {}
        fn shared<T: Send + Sync>() {}
        crosses::<crate::memory::Memory>();
        shared::<Box<dyn Suggester>>();
    }

    /// **The extraction changed nothing, asserted against the function it extracted
    /// from rather than against remembered outputs.**
    ///
    /// Three cases, and the third is the one that matters. `nunca` and `never` mean the
    /// same thing and look nothing alike, so trigrams cannot reach it, and that boundary
    /// is quoted as a promise on four surfaces: `print_suggestions` prints it, the MCP
    /// miss reply says it in its own words, and README.md and the built site pages
    /// reproduce the terminal run verbatim. While the only implementation is spelling,
    /// the promise is true. It is the first thing a meaning tier would falsify, and it
    /// would have to arrive with a diff to that text.
    #[test]
    fn the_seam_ships_the_scorer_it_replaced_and_nothing_else() {
        let cognate = vec![entry("ingestion", &["ingest a source", "distill"])];
        let typo = vec![entry("layout", &["repository", "tenancy"])];
        let translation = vec![entry("limits", &["never", "stop and ask"])];

        for (question, entries, want) in [
            ("o que e um protocolo de ingestao", &cognate, vec!["ingest a source"]),
            ("onde fica o repositry", &typo, vec!["repository"]),
            ("o que voce nunca faz", &translation, vec![]),
        ] {
            let through_the_seam = Trigram.words(question, entries, 5);
            assert_eq!(
                through_the_seam,
                index::suggest(question, entries, 5),
                "the seam is the free function and nothing more: {question}"
            );
            assert_eq!(through_the_seam, want, "{question}");
        }
    }
}
