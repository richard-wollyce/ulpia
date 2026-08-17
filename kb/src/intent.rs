//! The questions that must never reach a model.
//!
//! "quem é você?" returned marketing psychology from Steve's base. The mechanism is
//! not a ranking subtlety: `index::normalise` drops stopwords, and its list holds
//! `voce`, `e`, `que` and `qual`, so the question reduces to the single term `quem`,
//! which is frequent in notes about audience research. The router did exactly what it
//! was asked. It was asked the wrong thing.
//!
//! Retrieval is the wrong tool for that question in the first place. The fleet's own
//! name and roster are **facts about the running system**, not passages in a base, and
//! a system that has to search for its own name will eventually search wrong.
//!
//! So this tier answers them by lookup. No model, no index, no ranking. The
//! consequences are worth stating because each one is a property retrieval cannot have:
//!
//! - **It cannot drift.** The same question gives the same answer next month.
//! - **It cannot leak.** Nothing is read except `fleet.txt`, `agent.txt` and the names
//!   of the directories under `agents/`.
//! - **It cannot answer in the wrong language.** The form that matched carries the
//!   language, so a Portuguese question selects Portuguese text that we wrote. Language
//!   drift is a generation failure, and there is no generation here.
//!
//! ## Why the matching is strict rather than clever
//!
//! The fallthrough is a working retrieval system, so a false positive here is strictly
//! worse than a miss: missing costs a search, firing wrongly replaces a real answer
//! with a canned one. That asymmetry sets the rule:
//!
//! **A form has to account for the whole question, not appear inside it.**
//!
//! `quem e voce` matches. `quem e o publico alvo do instagram` does not, though it
//! starts with the same three words. Only greetings and politeness are stripped before
//! the comparison, because those can wrap any question without changing it.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    /// Who is answering.
    Identity,
    /// What agents the fleet holds.
    Agents,
    /// What this system does at all.
    Capabilities,
}

/// The language the question was asked in, carried by the form that matched.
///
/// This is the whole answer to "if I ask in Portuguese, will it reply in English?".
/// At this tier it cannot: the reply is a string chosen by the match, not text produced
/// by a model that saw English passages and drifted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Pt,
    En,
}

#[derive(Clone, Debug)]
pub struct Answer {
    pub text: String,
    pub intent: Intent,
    pub lang: Lang,
}

/// A name and a role, as read from `fleet.txt` or `agent.txt`.
pub struct Card {
    pub name: String,
    pub role: Option<String>,
}

/// Greetings and politeness, dropped before matching.
///
/// Kept deliberately short. Every word here is a word that can no longer distinguish
/// one question from another, so a long list is how a strict matcher quietly becomes a
/// loose one.
const FILLER: &[&str] = &[
    "oi", "ola", "ei", "hey", "hi", "hello", "afinal", "entao", "ai", "please", "pf",
    "por", "favor", "me", "diga", "fala", "fale", "tell", "hein", "ne",
];

/// Every question this tier claims, as its normalised shape.
///
/// A table rather than rules on purpose. Rules over questions this short generalise in
/// the wrong direction: a rule loose enough to cover the ways people ask their own
/// system its name is also loose enough to swallow real questions.
const FORMS: &[(&str, Intent, Lang)] = &[
    // Identity, Portuguese
    ("quem e voce", Intent::Identity, Lang::Pt),
    ("quem e vc", Intent::Identity, Lang::Pt),
    ("quem es tu", Intent::Identity, Lang::Pt),
    ("voce quem e", Intent::Identity, Lang::Pt),
    ("quem esta falando", Intent::Identity, Lang::Pt),
    ("qual e o seu nome", Intent::Identity, Lang::Pt),
    ("qual o seu nome", Intent::Identity, Lang::Pt),
    ("qual e seu nome", Intent::Identity, Lang::Pt),
    ("qual seu nome", Intent::Identity, Lang::Pt),
    ("qual e o teu nome", Intent::Identity, Lang::Pt),
    ("qual teu nome", Intent::Identity, Lang::Pt),
    ("seu nome", Intent::Identity, Lang::Pt),
    ("como voce se chama", Intent::Identity, Lang::Pt),
    ("como se chama", Intent::Identity, Lang::Pt),
    // Identity, English
    ("who are you", Intent::Identity, Lang::En),
    ("what are you", Intent::Identity, Lang::En),
    ("who is this", Intent::Identity, Lang::En),
    ("who am i talking to", Intent::Identity, Lang::En),
    ("what is your name", Intent::Identity, Lang::En),
    ("whats your name", Intent::Identity, Lang::En),
    ("what s your name", Intent::Identity, Lang::En),
    ("your name", Intent::Identity, Lang::En),
    // Roster, Portuguese
    ("quais agentes existem", Intent::Agents, Lang::Pt),
    ("quais agentes existem na frota", Intent::Agents, Lang::Pt),
    ("quais sao os agentes", Intent::Agents, Lang::Pt),
    ("quais os agentes", Intent::Agents, Lang::Pt),
    ("quais agentes", Intent::Agents, Lang::Pt),
    ("que agentes existem", Intent::Agents, Lang::Pt),
    ("quantos agentes existem", Intent::Agents, Lang::Pt),
    ("quantos agentes", Intent::Agents, Lang::Pt),
    ("liste os agentes", Intent::Agents, Lang::Pt),
    ("lista os agentes", Intent::Agents, Lang::Pt),
    ("lista de agentes", Intent::Agents, Lang::Pt),
    ("agentes", Intent::Agents, Lang::Pt),
    ("quem esta na frota", Intent::Agents, Lang::Pt),
    ("quem faz parte da frota", Intent::Agents, Lang::Pt),
    // Roster, English
    ("list agents", Intent::Agents, Lang::En),
    ("list the agents", Intent::Agents, Lang::En),
    ("which agents", Intent::Agents, Lang::En),
    ("which agents exist", Intent::Agents, Lang::En),
    ("what agents exist", Intent::Agents, Lang::En),
    ("what agents are there", Intent::Agents, Lang::En),
    ("who is in the fleet", Intent::Agents, Lang::En),
    ("agents", Intent::Agents, Lang::En),
    // Capabilities, Portuguese
    ("o que voce faz", Intent::Capabilities, Lang::Pt),
    ("o que voce pode fazer", Intent::Capabilities, Lang::Pt),
    ("o que voce sabe fazer", Intent::Capabilities, Lang::Pt),
    ("para que voce serve", Intent::Capabilities, Lang::Pt),
    ("pra que voce serve", Intent::Capabilities, Lang::Pt),
    ("como funciona", Intent::Capabilities, Lang::Pt),
    ("ajuda", Intent::Capabilities, Lang::Pt),
    // Capabilities, English
    ("what can you do", Intent::Capabilities, Lang::En),
    ("what do you do", Intent::Capabilities, Lang::En),
    ("what are you for", Intent::Capabilities, Lang::En),
    ("how does this work", Intent::Capabilities, Lang::En),
    ("help", Intent::Capabilities, Lang::En),
];

/// Recognises a question, or declines.
///
/// `None` is the normal outcome and is not a failure: it means the question is about
/// the base, which is what retrieval is for.
pub fn classify(question: &str) -> Option<(Intent, Lang)> {
    let shape = shape(question);
    FORMS
        .iter()
        .find(|(form, _, _)| *form == shape)
        .map(|(_, intent, lang)| (*intent, *lang))
}

/// Folds accents, lowercases, splits on anything that is not alphanumeric, and drops
/// filler. Deliberately **not** `index::normalise`, which also drops stopwords: the
/// stopword list holds `voce`, `qual` and `que`, so running it here would erase exactly
/// the words that carry the intent. That erasure is the bug this module answers.
fn shape(question: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();

    for c in question.chars() {
        let folded = fold(c);
        if folded.is_alphanumeric() {
            current.push(folded.to_ascii_lowercase());
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    words.retain(|w| !FILLER.contains(&w.as_str()));
    words.join(" ")
}

/// The same folding `index.rs` does, repeated rather than shared because that one is
/// private to a module whose public entry point is the wrong function to call here.
fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'ã' | 'â' | 'ä' | 'Á' | 'À' | 'Ã' | 'Â' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'õ' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Õ' | 'Ô' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Reading the fleet's own description
// ---------------------------------------------------------------------------

/// Reads `name` and `role` from a `key = value` file, falling back to `fallback` for
/// the name.
///
/// The fallback is not a nicety. Zed, Steve and Yaron were built by hand and have no
/// `agent.txt`, because only `kb init` writes one. An identity tier that answered
/// "unknown" for the three agents that actually exist would be a tier nobody trusts.
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

// ---------------------------------------------------------------------------
// Writing the answer
// ---------------------------------------------------------------------------

/// Builds the reply. Every string here is ours, in the language the form carried.
pub fn compose(intent: Intent, lang: Lang, fleet: &Card, agents: &[Card]) -> Answer {
    let text = match (intent, lang) {
        (Intent::Identity, Lang::Pt) => format!(
            "Sou {}, {}.\n\n{}\n\n{}",
            fleet.name,
            fleet.role.as_deref().unwrap_or("orquestrador desta frota"),
            roster_line(agents, lang),
            PROVENANCE_PT
        ),
        (Intent::Identity, Lang::En) => format!(
            "I am {}, {}.\n\n{}\n\n{}",
            fleet.name,
            fleet.role.as_deref().unwrap_or("the orchestrator of this fleet"),
            roster_line(agents, lang),
            PROVENANCE_EN
        ),
        (Intent::Agents, _) => format!("{}\n\n{}", roster(agents, lang), provenance(lang)),
        (Intent::Capabilities, Lang::Pt) => format!(
            "Sou {}, {}.\n\n{}\n\n{}",
            fleet.name,
            fleet.role.as_deref().unwrap_or("orquestrador desta frota"),
            CAPABILITIES_PT,
            PROVENANCE_PT
        ),
        (Intent::Capabilities, Lang::En) => format!(
            "I am {}, {}.\n\n{}\n\n{}",
            fleet.name,
            fleet.role.as_deref().unwrap_or("the orchestrator of this fleet"),
            CAPABILITIES_EN,
            PROVENANCE_EN
        ),
    };

    Answer { text, intent, lang }
}

fn provenance(lang: Lang) -> &'static str {
    match lang {
        Lang::Pt => PROVENANCE_PT,
        Lang::En => PROVENANCE_EN,
    }
}

/// One line, for when the roster is context rather than the question.
fn roster_line(agents: &[Card], lang: Lang) -> String {
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    match (lang, names.len()) {
        (Lang::Pt, 0) => "A frota ainda não tem nenhum agente.".to_string(),
        (Lang::Pt, 1) => format!("A frota tem 1 agente: {}.", names[0]),
        (Lang::Pt, n) => format!("A frota tem {n} agentes: {}.", names.join(", ")),
        (Lang::En, 0) => "The fleet has no agents yet.".to_string(),
        (Lang::En, 1) => format!("The fleet has 1 agent: {}.", names[0]),
        (Lang::En, n) => format!("The fleet has {n} agents: {}.", names.join(", ")),
    }
}

/// The full roster, with each agent's role, for when that is the question.
fn roster(agents: &[Card], lang: Lang) -> String {
    if agents.is_empty() {
        return roster_line(agents, lang);
    }

    let missing = match lang {
        Lang::Pt => "sem role definido em agent.txt",
        Lang::En => "no role set in agent.txt",
    };
    let lines: Vec<String> = agents
        .iter()
        .map(|a| format!("- {}: {}", a.name, a.role.as_deref().unwrap_or(missing)))
        .collect();

    format!("{}\n\n{}", roster_line(agents, lang), lines.join("\n"))
}

const CAPABILITIES_PT: &str = "\
O que faço sem chamar modelo nenhum:

- Digo quem sou e quais agentes existem na frota.
- Aponto quais arquivos uma pergunta deve abrir, por roteamento.
- Trago as passagens em si, de todos os agentes ao mesmo tempo.
- Meço uma afirmação contra o que a base já diz e proponho ADD, UPDATE ou NOOP, \
sem escrever nada.

Qualquer outra pergunta vira busca na base.";

const CAPABILITIES_EN: &str = "\
What I do without calling a model at all:

- Say who I am and which agents the fleet holds.
- Point at the files a question should open, by routing.
- Return the passages themselves, across every agent at once.
- Measure a claim against what the base already says and propose ADD, UPDATE or NOOP, \
without writing anything.

Any other question becomes a search over the base.";

const PROVENANCE_PT: &str =
    "Lido direto da estrutura da frota, sem modelo e sem busca.";
const PROVENANCE_EN: &str =
    "Read straight from the fleet structure, with no model and no search.";

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact question that returned marketing psychology, in the forms a person
    /// actually types it.
    #[test]
    fn the_question_that_broke_is_recognised_however_it_is_typed() {
        for q in [
            "quem é você?",
            "Quem e voce",
            "oi, quem é você?",
            "quem é você afinal",
            "me diga quem é você",
            "QUEM É VOCÊ",
            "qual é o seu nome?",
            "who are you?",
        ] {
            assert_eq!(
                classify(q).map(|(i, _)| i),
                Some(Intent::Identity),
                "{q:?} has to be answered by lookup, never by search"
            );
        }
    }

    /// The property that makes the tier safe to put in front of retrieval. A form has
    /// to be the whole question; sharing a prefix with one is not enough.
    #[test]
    fn a_real_question_that_starts_the_same_way_is_left_to_retrieval() {
        for q in [
            "quem é o público alvo do instagram",
            "quem é o cliente ideal do yaron",
            "qual o seu nome favorito para uma marca",
            "what agents exist in the reinforcement learning literature",
            "como funciona o algoritmo do youtube",
            "o que você faz quando o cliente some por duas semanas",
            "quantos agentes de vendas uma empresa precisa",
        ] {
            assert_eq!(
                classify(q),
                None,
                "{q:?} is a question about the base and must reach the index"
            );
        }
    }

    /// Richard's stated fear, tested at the only place this tier could fail it.
    /// A Portuguese question cannot come back in English, because the reply is not
    /// generated: it is selected by the form that matched.
    #[test]
    fn the_language_of_the_answer_is_the_language_of_the_question() {
        let fleet = Card { name: "Fleet".into(), role: None };
        let agents = vec![Card { name: "Zed".into(), role: Some("arquiteto".into()) }];

        for (q, lang) in [
            ("quem é você", Lang::Pt),
            ("who are you", Lang::En),
            ("quais agentes existem", Lang::Pt),
            ("list agents", Lang::En),
            ("o que você faz", Lang::Pt),
            ("what can you do", Lang::En),
        ] {
            let (intent, matched) = classify(q).expect(q);
            assert_eq!(matched, lang, "{q:?} carries its own language");

            let answer = compose(intent, matched, &fleet, &agents);
            let looks_pt = answer.text.contains("frota") || answer.text.contains("Sou ");
            let looks_en = answer.text.contains("fleet") || answer.text.contains("I am ");
            match lang {
                Lang::Pt => assert!(looks_pt && !looks_en, "{q:?} answered in English: {}", answer.text),
                Lang::En => assert!(looks_en && !looks_pt, "{q:?} answered in Portuguese: {}", answer.text),
            }
        }
    }

    /// The three agents that exist have no `agent.txt`, so the fallback is the path
    /// most real questions take today.
    #[test]
    fn an_agent_without_a_card_still_gets_a_name() {
        let dir = std::env::temp_dir().join("kb-intent-tests-missing");
        let _ = std::fs::create_dir_all(&dir);
        let c = card(&dir, "agent.txt", "yaron");
        assert_eq!(c.name, "Yaron");
        assert!(c.role.is_none(), "an absent role is absent, not invented");
    }

    #[test]
    fn a_card_is_read_when_it_is_there() {
        let dir = std::env::temp_dir().join(format!("kb-intent-{}", std::process::id()));
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

    /// The roster is the answer, so the count and the roles both have to be in it.
    #[test]
    fn the_roster_names_every_agent_and_says_when_a_role_is_missing() {
        let fleet = Card { name: "Fleet".into(), role: None };
        let agents = vec![
            Card { name: "Steve".into(), role: Some("marketing".into()) },
            Card { name: "Zed".into(), role: None },
        ];

        let (intent, lang) = classify("quais agentes existem").unwrap();
        let text = compose(intent, lang, &fleet, &agents).text;

        assert!(text.contains("2 agentes"));
        assert!(text.contains("Steve: marketing"));
        assert!(text.contains("Zed: sem role definido em agent.txt"));
    }
}
