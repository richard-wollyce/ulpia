//! The spawnable half: prompt on stdin, answer on stdout, nothing else.
//!
//! Every failure exits non-zero with a reason on stderr and **nothing on stdout**. That
//! silence is the contract, not tidiness: `kb` parses stdout for a verdict, and a partial
//! answer followed by an error is worse than no answer because the parser may accept it.
//! With stdout empty and the exit code set, routing falls back to the deterministic choice,
//! which is what ADR-0027 promises.

use std::io::Read;
use std::io::Write;

use model_call::keys;

fn fail(message: &str) -> ! {
    eprintln!("model-call: {message}");
    std::process::exit(1)
}

const USAGE: &str = "\
usage:
  model-call <provider> <model>          prompt on stdin, answer on stdout
  model-call --probe <provider> <model>...
  model-call keys list
  model-call keys set <provider>         secret on stdin, never an argument
  model-call keys rm <provider>

providers: gemini, openai, anthropic, openrouter";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        [] | ["-h"] | ["--help"] => {
            println!("{USAGE}");
        }
        ["keys", "list"] => {
            for (provider, variable) in keys::PROVIDERS {
                println!("{provider:<12} {:<13} {variable}", keys::source(provider));
            }
        }
        ["keys", "set", provider] => set_key(provider),
        ["keys", "rm", provider] => {
            require_provider(provider);
            match keys::remove(provider) {
                Ok(()) => println!("{provider}: removed from the os store"),
                Err(e) => fail(&format!("could not remove {provider}: {e}")),
            }
        }
        ["--probe", provider, models @ ..] if !models.is_empty() => {
            require_provider(provider);
            let key = key_or_fail(provider);
            for model in models {
                probe(provider, model, &key);
            }
        }
        [provider, model] => {
            require_provider(provider);
            let key = key_or_fail(provider);
            let mut prompt = String::new();
            if std::io::stdin().read_to_string(&mut prompt).is_err() {
                fail("could not read the prompt from stdin");
            }
            if prompt.trim().is_empty() {
                fail("nothing arrived on stdin");
            }
            match model_call::call(provider, model, &key, &prompt) {
                Ok(text) if text.trim().is_empty() => fail("the model returned no text"),
                Ok(text) => {
                    print!("{text}");
                    let _ = std::io::stdout().flush();
                }
                Err(e) => fail(&e),
            }
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn require_provider(provider: &str) {
    if !keys::is_provider(provider) {
        let names: Vec<&str> = keys::PROVIDERS.iter().map(|(p, _)| *p).collect();
        fail(&format!("provider must be one of: {}", names.join(", ")));
    }
}

fn key_or_fail(provider: &str) -> String {
    keys::get(provider).unwrap_or_else(|| {
        let variable = keys::PROVIDERS.iter().find(|(p, _)| *p == provider).map(|(_, v)| *v).unwrap_or("");
        fail(&format!(
            "no key for {provider}. Store one with:  model-call keys set {provider}  \
             (or set {variable} in the environment)"
        ))
    })
}

fn set_key(provider: &str) {
    require_provider(provider);
    // Read from stdin, never from an argument: an argument is visible in the process list
    // to every other process on the machine while it runs.
    let mut secret = String::new();
    if std::io::stdin().read_to_string(&mut secret).is_err() {
        fail("could not read the secret from stdin");
    }
    let secret = secret.trim();
    if secret.is_empty() {
        fail("nothing arrived on stdin. Pipe the key in rather than typing it as an argument");
    }
    match keys::set(provider, secret) {
        // Length only. A confirmation that echoes the key is a confirmation that leaks it.
        Ok(()) => println!("{provider}: stored in the os store, {} characters", secret.chars().count()),
        Err(e) => fail(&format!("could not store the key: {e}")),
    }
}

/// The smallest real call, to answer the one question a catalogue cannot.
///
/// A model can be listed and still refuse: `gemini-2.5-flash` appears in Gemini's own
/// ListModels for this fleet's key and answers "no longer available to new users" when
/// called. Listing says whether it exists; only calling says whether this key may use it.
/// Costs one request against that model's daily quota, so probe what you mean to use.
fn probe(provider: &str, model: &str, key: &str) {
    match model_call::call(provider, model, key, "ok") {
        Ok(text) => println!("{model:<34} CALLABLE ({} chars back)", text.chars().count()),
        Err(e) => println!("{model:<34} {e}"),
    }
}
