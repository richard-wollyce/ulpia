//! One model call to any of four providers: a prompt in, the answer out.
//!
//! Same contract as every command in `fleet.txt` and as ADR-0027. The binary wraps this in
//! stdin and stdout; the desktop app is expected to link it and call [`call`] directly, the
//! way ADR-0009 has the tray link `kb` rather than shell out and parse.
//!
//! ## What the four disagree about, which is the whole content of this file
//!
//! Verified against each vendor's current reference on 2026-09-13. Every one of these is a
//! place where treating the four as one shape breaks quietly rather than loudly:
//!
//! 1. **The model id is in the URL for Gemini and in the body for the other three.**
//! 2. **`max_tokens` is required by Anthropic alone**, and omitting it is a 400. OpenAI
//!    renamed its own to `max_completion_tokens`, whose budget also covers hidden reasoning
//!    tokens, so a small value there returns empty content with `finish_reason: length`.
//! 3. **Only Anthropic has a required version header**, `anthropic-version`. Nothing sends
//!    it for you and the call fails without it.
//! 4. **A 200 is not success on OpenRouter.** A failure mid generation returns HTTP 200
//!    with an `error` object in the body, because the headers were committed before the
//!    model produced a token. No HTTP client raises on that, so every response is checked
//!    for an error key and not only the non-2xx ones.
//! 5. **The text leaf is not a string everywhere.** Anthropic returns a list of blocks whose
//!    first can be a thinking block with no `text` field at all; OpenRouter's `content` is a
//!    string, a list of parts, or null. Reaching for `[0].text` finds nothing on one and a
//!    serialised array on the other.
//! 6. **Reasoning is on by default and billed as output** on the current OpenAI and
//!    Anthropic flagships. For a one shot judgement that is money spent on tokens nobody
//!    reads, so both are turned off explicitly.
//!
//! ## The system prompt, and why there is none
//!
//! The four put it in four places: a `system` role inside `messages` for OpenAI and
//! OpenRouter, a top level `system` string for Anthropic, which rejects a system role in
//! messages, and a separate field again for Gemini. None is used. `kb` assembles the whole
//! instruction and it arrives as one user turn, so the rules live in the caller and this is
//! a pen.

pub mod keys;

use serde_json::{json, Value};

const TIMEOUT_SECONDS: u64 = 120;
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Anthropic requires a cap and has no unlimited. Large enough for any answer these call
/// sites ask for, small enough that a runaway costs cents.
const ANTHROPIC_MAX_TOKENS: u32 = 4096;

pub fn endpoint(provider: &str, model: &str) -> String {
    match provider {
        "gemini" => format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
        ),
        "openai" => "https://api.openai.com/v1/chat/completions".into(),
        "anthropic" => "https://api.anthropic.com/v1/messages".into(),
        _ => "https://openrouter.ai/api/v1/chat/completions".into(),
    }
}

/// The auth header and anything else the API rejects the call without.
pub fn headers(provider: &str, key: &str) -> Vec<(&'static str, String)> {
    match provider {
        "gemini" => vec![("x-goog-api-key", key.to_string())],
        "anthropic" => vec![
            ("x-api-key", key.to_string()),
            ("anthropic-version", ANTHROPIC_VERSION.to_string()),
        ],
        _ => vec![("Authorization", format!("Bearer {key}"))],
    }
}

/// The request body. `temperature` zero everywhere it is accepted: these call sites are
/// judgements that must not move between two identical runs, or `kb eval` compares noise.
pub fn body(provider: &str, model: &str, prompt: &str) -> Value {
    match provider {
        "gemini" => json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": { "temperature": 0 }
        }),
        "anthropic" => json!({
            "model": model,
            "max_tokens": ANTHROPIC_MAX_TOKENS,
            "temperature": 0,
            "messages": [{ "role": "user", "content": prompt }],
            "thinking": { "type": "disabled" }
        }),
        "openai" => json!({
            "model": model,
            "temperature": 0,
            "messages": [{ "role": "user", "content": prompt }],
            "reasoning_effort": "none"
        }),
        _ => json!({
            "model": model,
            "temperature": 0,
            "messages": [{ "role": "user", "content": prompt }]
        }),
    }
}

/// The assistant's text, out of four differently shaped responses.
pub fn extract(provider: &str, parsed: &Value) -> Result<String, String> {
    match provider {
        "gemini" => {
            let candidates = parsed["candidates"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            let Some(first) = candidates.first() else {
                let blocked = parsed["promptFeedback"]["blockReason"].as_str().unwrap_or("");
                return Err(if blocked.is_empty() {
                    "no candidate returned".into()
                } else {
                    format!("no candidate returned, blocked: {blocked}")
                });
            };
            Ok(first["content"]["parts"]
                .as_array()
                .map(|parts| {
                    parts.iter().filter_map(|p| p["text"].as_str()).collect::<String>()
                })
                .unwrap_or_default())
        }
        "anthropic" => Ok(parsed["content"]
            .as_array()
            .map(|blocks| {
                // A list of blocks, and the first can be a thinking block with no text.
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()),
        _ => {
            let Some(message) = parsed["choices"].as_array().and_then(|c| c.first()).map(|c| &c["message"])
            else {
                return Err("the response carried no choices".into());
            };
            match &message["content"] {
                // OpenAI puts a refusal here instead of content.
                Value::Null => Ok(message["refusal"].as_str().unwrap_or("").to_string()),
                // OpenRouter's content is a three branch union; an array is parts.
                Value::Array(parts) => {
                    Ok(parts.iter().filter_map(|p| p["text"].as_str()).collect::<String>())
                }
                Value::String(text) => Ok(text.clone()),
                other => Err(format!("unexpected content shape: {other}")),
            }
        }
    }
}

/// Make the call. Every failure is a message a person can act on, never a panic.
pub fn call(provider: &str, model: &str, key: &str, prompt: &str) -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(TIMEOUT_SECONDS)))
        .build()
        .new_agent();

    let mut request = agent.post(&endpoint(provider, model));
    for (name, value) in headers(provider, key) {
        request = request.header(name, &value);
    }

    let parsed: Value = match request.send_json(body(provider, model, prompt)) {
        Ok(mut response) => response
            .body_mut()
            .read_json()
            .map_err(|e| format!("{provider} sent something that is not JSON: {e}"))?,
        Err(ureq::Error::StatusCode(code)) => {
            return Err(format!("HTTP {code} from {provider}"));
        }
        Err(e) => return Err(format!("could not reach {provider}: {e}")),
    };

    // Checked on EVERY response, not only the non-2xx ones. OpenRouter commits headers
    // before the model produces a token, so a mid generation failure arrives as a 200 with
    // an error body and no client raises anything at all.
    if !parsed["error"].is_null() {
        let message = parsed["error"]["message"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| parsed["error"].to_string());
        return Err(format!("{provider} returned an error: {}", truncate(&message, 300)));
    }

    extract(provider, &parsed)
}

fn truncate(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((cut, _)) => format!("{}...", &text[..cut]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four request shapes, in the four places they differ. This is the test that would
    /// have caught the version header and the required cap before a 400 did.
    #[test]
    fn each_provider_gets_the_request_its_api_actually_requires() {
        let has = |provider: &str, name: &str| {
            headers(provider, "K").iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
        };
        assert!(has("anthropic", "anthropic-version"), "anthropic rejects a call without it");
        assert!(!has("openai", "anthropic-version"), "nobody else has a version header");
        assert!(has("gemini", "x-goog-api-key") && !has("gemini", "Authorization"));
        assert!(has("openrouter", "Authorization"));

        assert!(endpoint("gemini", "m").contains("/models/m:generateContent"), "id in the url");
        assert!(body("gemini", "m", "hi")["model"].is_null(), "and therefore not in the body");
        assert_eq!(body("anthropic", "m", "hi")["model"], "m", "id in the body for the rest");

        assert_eq!(body("anthropic", "m", "hi")["max_tokens"], ANTHROPIC_MAX_TOKENS);
        assert!(body("openai", "m", "hi")["max_tokens"].is_null(), "required by anthropic alone");

        assert_eq!(body("anthropic", "m", "hi")["thinking"]["type"], "disabled");
        assert_eq!(body("openai", "m", "hi")["reasoning_effort"], "none");
    }

    /// Five response shapes, four of which are not "the string at choices[0]".
    #[test]
    fn the_text_is_found_in_all_five_shapes_the_vendors_return() {
        let gemini = json!({"candidates":[{"content":{"parts":[{"text":"A"},{"text":"B"}]}}]});
        assert_eq!(extract("gemini", &gemini).unwrap(), "AB");

        // The first block carries no text field at all, which is a KeyError in a naive reader.
        let anthropic = json!({"content":[{"type":"thinking"},{"type":"text","text":"hello"}]});
        assert_eq!(extract("anthropic", &anthropic).unwrap(), "hello");

        let plain = json!({"choices":[{"message":{"content":"plain"}}]});
        assert_eq!(extract("openai", &plain).unwrap(), "plain");

        let refusal = json!({"choices":[{"message":{"content":null,"refusal":"no"}}]});
        assert_eq!(extract("openai", &refusal).unwrap(), "no");

        let parts = json!({"choices":[{"message":{"content":[{"text":"a"},{"text":"b"}]}}]});
        assert_eq!(extract("openrouter", &parts).unwrap(), "ab");
    }

    #[test]
    fn a_blocked_gemini_prompt_says_why_rather_than_returning_nothing() {
        let blocked = json!({"candidates":[],"promptFeedback":{"blockReason":"SAFETY"}});
        let error = extract("gemini", &blocked).unwrap_err();
        assert!(error.contains("SAFETY"), "{error}");
    }

    #[test]
    fn truncate_never_splits_a_character() {
        let accented = "á".repeat(400);
        let cut = truncate(&accented, 300);
        assert!(cut.ends_with("..."));
        assert_eq!(cut.chars().count(), 303);
    }
}
