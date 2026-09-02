//! Listing what the library holds, by facet, with nothing ranked.
//!
//! **The four search tools all score a question against a floor**, which is the right
//! shape for "what answers this" and the wrong shape for "what is in here". A filter
//! question carries no ranking problem: "which tools notes are still raw" has one exact
//! answer, and asking it of a scorer produces a top five with a verdict attached,
//! because the machinery behind `Memory::route` cannot express "all of them, in no
//! particular order". A caller then reads a `Guess` and cannot tell a base that holds
//! three such files from a base that holds none.
//!
//! So a [`Listed`] deliberately carries **no score, no gate, no verdict and no
//! `matched` list**. Every one of those is evidence about a ranking, and nothing here
//! was ranked. There is no number for a caller to argue with because there was no
//! question to compute one against.
//!
//! **The walk is over `Base::files`, not over `index::build`'s entries**, and that is
//! the single decision that makes the `stage` and `provenance` facets mean anything.
//! `index::build` builds no entry for a file with no `Search for:` line, which is the
//! entire deposit plus every README, so a listing over entries would answer
//! `--stage raw` with zero rows on a base full of raw captures. The two populations
//! differ on purpose: a filter is a question about what the library holds, not about
//! what a question can reach. The cost is real and worth stating, because nothing warns
//! about it: `MAP.md`, `README.md` and `CLAUDE.md` are rows here, and an operator
//! narrows them away with `--folder knowledge`.
//!
//! **The facets are read off disk on every call, never out of the store.** The `files`
//! table carries the same `provenance` and `stage` columns, written at `kb index` time,
//! and the two disagree whenever the index is stale: a note whose front matter changed
//! an hour ago is `distilled` here and `raw` to `kb_retrieve`. Disk cannot be stale, and
//! the store holds only what the last sync happened to see, with the private layer
//! deleted outright if that run omitted `--all`. Reading disk is the right call and the
//! divergence is still real.

use crate::base::Base;
use crate::checks::{PROVENANCE, STAGE};
use crate::index::{self, Kind};
use crate::json::Value;
use crate::retrieve::{self, Layer};

/// The three species, so the legal set for `--kind` is the enum and not a copy of it.
/// `write.rs` reads `PROVENANCE` and `STAGE` out of `checks.rs` for the same reason, and
/// it does so because its own copy drifted the day `captured` was added, leaving the
/// linter accepting a word the writer refused and `kb promote` unable to write anything.
const KINDS: &[Kind] = &[Kind::Memory, Kind::Skills, Kind::Tools];

/// How many rows the MCP surface returns before it starts counting instead of listing.
///
/// The reasoning is `Memory::SUGGEST_LIMIT`'s, verbatim: a shortlist is what a caller can
/// act on, a dump is something it has to route through a second time. An unbounded
/// listing over a real fleet is several hundred rows landing in a model's context. The
/// count of what was cut travels with the cut, so a caller can tell a narrow filter from
/// a truncated one, which is the distinction a bare truncation destroys. The terminal is
/// not capped: a person has a scrollback and a pipe, and a model has neither.
pub const MCP_LIMIT: usize = 50;

/// One file on the shelf, with its facets and deliberately nothing about ranking.
///
/// Shaped like [`retrieve::Retrieved`] so a reader recognises it, minus `score`,
/// `keyword_score`, `why`, `matched` and `passages`. That subtraction is the type doing
/// the arguing: there is no field to put a number in, so no surface downstream can start
/// reporting one it computed itself.
pub struct Listed {
    pub base: String,
    /// Relative to the base root, forward slashes, exactly as `Base::files` carries it.
    pub path: String,
    /// The file's first heading, empty when it has none.
    pub title: String,
    pub kind: Kind,
    /// Short or long memory. **Included because ADR-0034 says a deposit file is served
    /// with its label on at every surface**, and a listing that shows `inbox/` unlabelled
    /// is that rule broken on a fifth one.
    pub layer: Layer,
    /// The directory the file sits in, empty at the base root. See [`folder_of`].
    pub folder: String,
    /// From the file's own front matter, not from the store's `files` table. `None` when
    /// the file declares none, which is a different thing from declaring a wrong one and
    /// is what `kb check` reports as W05.
    pub provenance: Option<String>,
    pub stage: Option<String>,
    /// Whether this file is in the base's private layer, by `base::private_layer`.
    ///
    /// **Without `--all` this is always false**, because `Base::discover` dropped those
    /// files before the listing ever saw them. A caller must not read "no row is marked
    /// private" as "this base has no private layer": it is the same shape as the bug
    /// `MdFile::private` was added to fix, where the filter lived only on the walk and
    /// nothing downstream could recognise a private file as private.
    pub private: bool,
}

/// The facets to narrow by. A `None` means "do not filter on this", never "any value".
#[derive(Default)]
pub struct Filter {
    pub base: Option<String>,
    pub folder: Option<String>,
    pub kind: Option<Kind>,
    pub stage: Option<String>,
    pub provenance: Option<String>,
}

/// A facet value outside its legal set.
///
/// **It is an error and not an empty result**, which is the whole reason this type
/// exists. `--kind memories` matching nothing would print zero rows, and zero rows reads
/// as "the base holds none of these": the wrong lesson, learned confidently, about a
/// typo. `checks.rs` refuses a bad provenance on write for exactly the same reason.
#[derive(Debug)]
pub enum BadFacet {
    Kind(String),
    Provenance(String),
    Stage(String),
}

impl std::fmt::Display for BadFacet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (flag, given, legal) = match self {
            BadFacet::Kind(v) => {
                ("--kind", v, KINDS.iter().map(|k| k.label()).collect::<Vec<_>>().join(", "))
            }
            BadFacet::Provenance(v) => ("--provenance", v, PROVENANCE.join(", ")),
            BadFacet::Stage(v) => ("--stage", v, STAGE.join(", ")),
        };
        write!(f, "{flag} {given} is not a value this facet has. Legal: {legal}")
    }
}

impl Filter {
    /// Builds a filter from five optional strings, refusing any value outside its set.
    ///
    /// `base` and `folder` are not validated against anything, and that is deliberate
    /// rather than an omission: their legal sets are whatever happens to be on disk, so
    /// refusing an unknown one would mean walking the fleet before an argument could be
    /// parsed, and a base that is genuinely absent is a listing an operator should read
    /// as empty. `kind`, `stage` and `provenance` have closed sets written down in this
    /// crate, and a closed set is exactly what can be checked before any work happens.
    pub fn parse(
        base: Option<&str>,
        folder: Option<&str>,
        kind: Option<&str>,
        stage: Option<&str>,
        provenance: Option<&str>,
    ) -> Result<Filter, BadFacet> {
        let kind = match kind {
            Some(k) => Some(
                KINDS
                    .iter()
                    .copied()
                    .find(|c| c.label().eq_ignore_ascii_case(k.trim()))
                    .ok_or_else(|| BadFacet::Kind(k.to_string()))?,
            ),
            None => None,
        };
        let stage = match stage {
            Some(s) => Some(one_of(s, STAGE).ok_or_else(|| BadFacet::Stage(s.to_string()))?),
            None => None,
        };
        let provenance = match provenance {
            Some(p) => {
                Some(one_of(p, PROVENANCE).ok_or_else(|| BadFacet::Provenance(p.to_string()))?)
            }
            None => None,
        };

        Ok(Filter {
            base: base.map(|b| b.trim().to_string()),
            // The trailing slash is optional here because it is optional in `agent.txt`'s
            // `private =` line and in `kb write --folder`. One spelling rule for a folder
            // across the crate, or an operator has to learn three.
            folder: folder.map(|f| f.trim().trim_end_matches('/').to_string()),
            kind,
            stage,
            provenance,
        })
    }

    /// AND across facets: every named one has to match.
    ///
    /// **OR is the failure this shape is chosen against**, and it is invisible until two
    /// facets are combined, because with one facet the union and the intersection are the
    /// same set. A caller narrowing twice and getting more rows back has no reason to
    /// suspect the filter rather than its own question.
    pub fn matches(&self, l: &Listed) -> bool {
        if let Some(b) = &self.base {
            if &l.base != b {
                return false;
            }
        }
        if let Some(dir) = &self.folder {
            if !(&l.folder == dir || l.folder.starts_with(&format!("{dir}/"))) {
                return false;
            }
        }
        if let Some(k) = self.kind {
            if l.kind != k {
                return false;
            }
        }
        if let Some(s) = &self.stage {
            if l.stage.as_deref() != Some(s.as_str()) {
                return false;
            }
        }
        if let Some(p) = &self.provenance {
            if l.provenance.as_deref() != Some(p.as_str()) {
                return false;
            }
        }
        true
    }
}

/// A value from a closed set, matched case insensitively and returned in the set's own
/// spelling, so what the filter compares against a file is always the canonical word.
fn one_of(given: &str, legal: &[&str]) -> Option<String> {
    legal.iter().find(|l| l.eq_ignore_ascii_case(given.trim())).map(|l| l.to_string())
}

/// The directory a base relative path sits in, slash separated, empty at the base root.
///
/// `--folder F` then matches when this equals `F` or starts with `F/`, which is the same
/// segment-wise prefix rule as `PrivateLayer::covers`. Segment-wise and not substring on
/// purpose: `--folder know` reaching `knowledge/` would make the facet answer a question
/// nobody asked, and `kb write --folder knowledge/systems` has to name the same set here
/// that it names there, or two commands disagree about one string.
pub fn folder_of(rel: &str) -> String {
    match rel.replace('\\', "/").rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// Every file in one discovered base, with its facets, in the order the walk found them.
///
/// Which files those are is entirely `Base::discover`'s answer, and that is where the
/// private layer was already applied. Nothing here re-derives it: `base::private_layer`
/// is the single declaration, ADR-0034, and a copy of `profile/ projects/ records/` in
/// this file would be a fifth surface owning the rule.
pub fn build(base: &Base) -> Vec<Listed> {
    let name = index::base_name(&base.root);
    base.files
        .iter()
        .map(|f| {
            let (provenance, stage) = declared(&f.text);
            Listed {
                base: name.clone(),
                path: f.rel.clone(),
                title: index::first_heading(&f.text),
                kind: index::kind_of(&f.rel),
                layer: retrieve::layer_of(&f.rel),
                folder: folder_of(&f.rel),
                provenance,
                stage,
                private: f.private,
            }
        })
        .collect()
}

/// The two ADR-0007 axes a file declares about itself, out of its front matter.
///
/// An empty value is `None` rather than `Some("")`: `provenance:` with nothing after it
/// declares nothing, and a filter that matched it would be answering about punctuation.
fn declared(text: &str) -> (Option<String>, Option<String>) {
    let Some(pairs) = crate::checks::front_matter(text) else {
        return (None, None);
    };
    let field = |want: &str| {
        pairs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(want))
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    (field("provenance"), field("stage"))
}

/// The listing as JSON, one object per file.
///
/// A `json::Value` rather than the hand rolled string `index::to_json` emits, so
/// `kb list --json` is one line on stdout like `kb route --json` and `kb remember
/// --json`, and so the shape can be asserted key by key in a test instead of matched as
/// text. **No key here is a score, a gate or a verdict**; see the module header.
pub fn to_json(rows: &[Listed]) -> Value {
    Value::Arr(
        rows.iter()
            .map(|l| {
                let mut o = Value::obj();
                o.set("base", l.base.clone().into());
                o.set("path", l.path.clone().into());
                o.set("title", l.title.clone().into());
                o.set("kind", l.kind.label().into());
                o.set("layer", l.layer.label().into());
                o.set("folder", l.folder.clone().into());
                // Null rather than an absent key. A missing key and a key holding null
                // read the same way in most languages, which is why `route_payload` pins
                // its own field list by name: a caller has to be able to tell "declares
                // no stage" from "this build stopped emitting stage".
                o.set("provenance", l.provenance.clone().map(Value::Str).unwrap_or(Value::Null));
                o.set("stage", l.stage.clone().map(Value::Str).unwrap_or(Value::Null));
                o.set("private", l.private.into());
                o
            })
            .collect(),
    )
}

/// The listing as text, one line per file, for the terminal and for the MCP reply.
///
/// One line rather than two, because the point of a filter is that somebody scans the
/// result: fifty files at two lines each is a page nobody reads to the end. The facets
/// ride in brackets in a fixed order, so they line up by eye even though the paths do
/// not, and the deposit label rides with them rather than being appended as prose.
pub fn to_text(rows: &[Listed]) -> String {
    let mut out = String::new();
    for l in rows {
        out.push_str(&format!("{}/{}", l.base, l.path));
        let mut facets = vec![l.kind.label().to_string(), l.layer.label().to_string()];
        match (&l.provenance, &l.stage) {
            (Some(p), Some(s)) => facets.push(format!("{p}/{s}")),
            (Some(p), None) => facets.push(p.clone()),
            (None, Some(s)) => facets.push(s.clone()),
            (None, None) => {}
        }
        if l.private {
            facets.push("private".into());
        }
        out.push_str(&format!("  [{}]", facets.join(", ")));
        if !l.title.is_empty() {
            out.push_str(&format!("  {}", l.title));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(rel: &str, stage: Option<&str>) -> Listed {
        Listed {
            base: "probe".into(),
            path: rel.into(),
            title: "T".into(),
            kind: crate::index::kind_of(rel),
            layer: crate::retrieve::layer_of(rel),
            folder: folder_of(rel),
            provenance: Some("human".into()),
            stage: stage.map(|s| s.to_string()),
            private: false,
        }
    }

    /// Four files across two folders, two species and two stages.
    ///
    /// **AND across facets is the one semantic a filter API gets wrong invisibly.** OR
    /// looks correct on every single-facet call, because with one facet the two are the
    /// same set, and only shows itself when a caller combines two and gets more rows
    /// back than it asked for rather than fewer.
    #[test]
    fn facets_narrow_together_rather_than_widening() {
        let rows = vec![
            row("tools/mcp.md", Some("raw")),
            row("tools/cli.md", Some("distilled")),
            row("knowledge/a.md", Some("raw")),
            row("knowledge/b.md", Some("distilled")),
        ];

        let kind_only =
            Filter { kind: Some(crate::index::Kind::Tools), ..Filter::default() };
        let kept: Vec<&str> =
            rows.iter().filter(|r| kind_only.matches(r)).map(|r| r.path.as_str()).collect();
        assert_eq!(kept, vec!["tools/mcp.md", "tools/cli.md"], "one facet, one species");

        let stage_only = Filter { stage: Some("raw".into()), ..Filter::default() };
        let kept: Vec<&str> =
            rows.iter().filter(|r| stage_only.matches(r)).map(|r| r.path.as_str()).collect();
        assert_eq!(kept, vec!["tools/mcp.md", "knowledge/a.md"]);

        let both = Filter {
            kind: Some(crate::index::Kind::Tools),
            stage: Some("raw".into()),
            ..Filter::default()
        };
        let kept: Vec<&str> =
            rows.iter().filter(|r| both.matches(r)).map(|r| r.path.as_str()).collect();
        assert_eq!(kept, vec!["tools/mcp.md"], "the intersection, never the union");
    }

    /// A typo must not come back as an empty result set.
    ///
    /// Zero rows reads as "the base holds none of these", which is the wrong lesson and
    /// the expensive one: it is the same failure `nothing_to_search` and `Store.rebuilt`
    /// already exist to prevent on the other surfaces. The legal values are read from
    /// `checks::PROVENANCE` and `checks::STAGE` rather than copied, because those two
    /// lists drifted from a copy in `write.rs` once already and `kb promote` could not
    /// write a single note for a day.
    #[test]
    fn an_unknown_facet_value_is_refused_and_names_the_legal_set() {
        let e = Filter::parse(None, None, Some("memories"), None, None)
            .err()
            .expect("a species that does not exist is refused, not answered with zero rows");
        let said = e.to_string();
        for legal in ["memory", "skills", "tools"] {
            assert!(said.contains(legal), "the legal set is named: {said}");
        }

        let e = Filter::parse(None, None, None, None, Some("authored"))
            .err()
            .expect("provenance is refused too");
        let said = e.to_string();
        for legal in crate::checks::PROVENANCE {
            assert!(said.contains(legal), "read from checks.rs, not copied: {said}");
        }

        let e = Filter::parse(None, None, None, Some("cooked"), None)
            .err()
            .expect("stage is refused too");
        let said = e.to_string();
        for legal in crate::checks::STAGE {
            assert!(said.contains(legal), "read from checks.rs, not copied: {said}");
        }

        assert!(
            Filter::parse(Some("steve"), Some("knowledge"), Some("tools"), Some("raw"), Some("human"))
                .is_ok(),
            "every legal value together is still legal"
        );
    }

    /// The prefix rule is segment-wise and not substring, the same shape as
    /// `PrivateLayer::covers`. Anything looser and `kb write --folder knowledge/systems`
    /// and `kb list --folder knowledge/systems` name two different sets, which is the
    /// drift that makes a filter untrustworthy rather than merely wrong.
    #[test]
    fn a_folder_facet_matches_a_directory_and_everything_under_it() {
        assert_eq!(folder_of("knowledge/systems/x.md"), "knowledge/systems");
        assert_eq!(folder_of("knowledge/x.md"), "knowledge");
        assert_eq!(folder_of("a.md"), "", "the base root is the empty folder");

        let deep = row("knowledge/systems/x.md", None);
        let shallow = row("knowledge/x.md", None);

        let f = Filter { folder: Some("knowledge".into()), ..Filter::default() };
        assert!(f.matches(&deep), "a parent folder reaches everything under it");
        assert!(f.matches(&shallow));

        let exact = Filter { folder: Some("knowledge/systems".into()), ..Filter::default() };
        assert!(exact.matches(&deep));
        assert!(!exact.matches(&shallow), "and not back up out of itself");

        let prefix = Filter { folder: Some("know".into()), ..Filter::default() };
        assert!(!prefix.matches(&deep), "a substring is not a folder");
        assert!(!prefix.matches(&shallow));
    }
}
