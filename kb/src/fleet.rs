//! What the fleet is, as facts rather than as an answer.
//!
//! An earlier version of this module classified questions and replied to them. It
//! matched "quem é você?" against a table of forms and returned a string we had
//! written. That was the wrong shape, and Richard said so: a question a model can
//! obviously answer should be answered by the model.
//!
//! The part that was right is smaller and stays. **The facts have to come from
//! somewhere that cannot be ranked wrong.** Retrieval got that question wrong for a
//! reason worth keeping written down: `index::normalise` drops stopwords, its list
//! holds `voce`, `e`, `que` and `qual`, so the question survived as the single term
//! `quem`, which is frequent in notes about audience research. Searching a knowledge
//! base for the system's own name will keep going wrong, because the name is not in
//! the knowledge base. It is in `fleet.txt`.
//!
//! So this module reads, and does not decide. It hands the orchestrator the roster the
//! same way a directory hands out a phone number: no judgement, no ranking, no prose
//! anyone has to trust. What the orchestrator does with it is the orchestrator's job.

use std::path::{Path, PathBuf};

/// A name and a role, as written in `fleet.txt` or `agent.txt`.
pub struct Card {
    pub name: String,
    /// Absent means the file has no `role =` line. Absent, not invented: a role we
    /// made up would read exactly like one the owner chose.
    pub role: Option<String>,
}

/// One agent, as the fleet sees it from outside.
pub struct Member {
    pub card: Card,
    pub root: PathBuf,
}

/// The whole fleet, described from its own files.
pub struct Description {
    pub fleet: Card,
    pub members: Vec<Member>,
}

/// Reads `name` and `role` from a `key = value` file, falling back to `fallback` for
/// the name.
///
/// The fallback is load bearing, not a nicety. Zed, Steve and Yaron were built by hand
/// and only `kb init` writes an `agent.txt`, so a roster that said "unknown" for the
/// three agents that actually exist would be a roster nobody trusts.
pub fn card(dir: &Path, file: &str, fallback: &str) -> Card {
    let text = std::fs::read_to_string(dir.join(file)).unwrap_or_default();
    let mut name = None;
    let mut role = None;

    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "name" => name = Some(value.to_string()),
            "role" => role = Some(value.to_string()),
            _ => {}
        }
    }

    Card { name: name.unwrap_or_else(|| title_case(fallback)), role }
}

fn title_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

impl Description {
    /// The roster as text, for a model to read.
    ///
    /// Written in English because the repository is, and because this is **input to a
    /// model, not output to a person**. The model answers in whatever language it was
    /// asked in; that is a thing models are good at and a thing a table of canned
    /// strings is bad at.
    ///
    /// Every line says where it came from. A model that cannot tell a looked up fact
    /// from a retrieved passage will eventually cite one as the other.
    pub fn to_text(&self) -> String {
        let mut out = format!("FLEET: {}\n", self.fleet.name);
        match &self.fleet.role {
            Some(role) => out.push_str(&format!("ROLE: {role}\n")),
            None => out.push_str("ROLE: not set. Add a `role = ` line to fleet.txt.\n"),
        }
        out.push_str(&format!("AGENTS: {}\n", self.members.len()));

        for m in &self.members {
            out.push_str(&format!(
                "\n- {}\n  root: {}\n  role: {}\n",
                m.card.name,
                m.root.display(),
                m.card.role.as_deref().unwrap_or("not set in agent.txt")
            ));
        }

        if self.members.is_empty() {
            out.push_str("\nThe fleet has no agents yet.\n");
        }

        out.push_str(
            "\nSOURCE: fleet.txt, each agent's agent.txt, and the directory names under \
             agents/. No index was queried and no ranking took place, so none of the above \
             is a retrieved passage and none of it can be attributed to a knowledge file.\n",
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_is_read_when_it_is_there() {
        let dir = std::env::temp_dir().join(format!("kb-fleet-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("agent.txt"),
            "# comment\nname = Yaron\nrole = nutrição e treino\n",
        )
        .unwrap();

        let c = card(&dir, "agent.txt", "ignored");
        assert_eq!(c.name, "Yaron");
        assert_eq!(c.role.as_deref(), Some("nutrição e treino"));
    }

    /// The three agents that exist have no `agent.txt`, so the fallback is the path
    /// most real lookups take today.
    #[test]
    fn an_agent_without_a_card_still_gets_a_name_and_no_invented_role() {
        let dir = std::env::temp_dir().join("kb-fleet-tests-missing");
        let _ = std::fs::create_dir_all(&dir);
        let c = card(&dir, "agent.txt", "yaron");
        assert_eq!(c.name, "Yaron");
        assert!(c.role.is_none(), "an absent role is absent, not invented");
    }

    /// The description is a model's input, so what it must never do is read like a
    /// passage. Every rendering carries where it came from.
    #[test]
    fn the_description_names_every_agent_and_says_where_it_came_from() {
        let d = Description {
            fleet: Card { name: "Fleet".into(), role: None },
            members: vec![
                Member {
                    card: Card { name: "Steve".into(), role: Some("marketing".into()) },
                    root: PathBuf::from("/f/agents/steve"),
                },
                Member {
                    card: Card { name: "Zed".into(), role: None },
                    root: PathBuf::from("/f/agents/zed"),
                },
            ],
        };

        let text = d.to_text();
        assert!(text.contains("AGENTS: 2"));
        assert!(text.contains("Steve"));
        assert!(text.contains("role: marketing"));
        assert!(text.contains("role: not set in agent.txt"), "a missing role is stated");
        assert!(text.contains("ROLE: not set"), "so is a missing fleet role");
        assert!(text.contains("SOURCE:"), "provenance is not optional");
        assert!(
            text.contains("no ranking took place"),
            "a model has to be able to tell this from a retrieved passage"
        );
    }

    #[test]
    fn an_empty_fleet_says_so_rather_than_rendering_nothing() {
        let d = Description {
            fleet: Card { name: "Fleet".into(), role: Some("orchestrator".into()) },
            members: vec![],
        };
        let text = d.to_text();
        assert!(text.contains("AGENTS: 0"));
        assert!(text.contains("no agents yet"));
    }
}
