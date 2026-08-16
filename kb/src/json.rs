//! A JSON parser and writer, hand written, for the MCP server.
//!
//! Written by hand rather than taken as a dependency, which is Richard's call
//! against the standard the `Cargo.toml` comment sets. The cost is this file and its
//! tests; the benefit is that `kb` still builds from one dependency.
//!
//! What actually makes a JSON parser hard is escapes: `\"`, `\\`, `\uXXXX` and the
//! surrogate pairs that carry anything above U+FFFF. Accented characters are not
//! hard and never were: Rust strings are UTF-8 and `í` costs the same as `i`. That
//! matters because the tempting shortcut, folding accents before parsing, would
//! destroy the framing (a quote inside a string arrives as `\"`) and would corrupt
//! any claim on its way to disk. Diacritic folding belongs where it already is, in
//! the FTS5 tokenizer, applied to search terms after parsing and never to text being
//! written.
//!
//! Two hard rules come from the transport, not from JSON:
//!
//! 1. Output is a single line. The MCP stdio framing is newline delimited, and a
//!    message containing a raw newline is a framing bug, not a formatting choice.
//! 2. Depth is bounded. The input arrives from another process, and an unbounded
//!    recursive descent parser turns `[[[[[...` into a stack overflow, which is a
//!    crash rather than an error.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Deep enough for any real MCP message, shallow enough that the recursion cannot
/// exhaust the stack. A message that needs more nesting than this is not a message.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    /// Ordered so output is deterministic, which is what makes the tests readable
    /// and a diff between two runs meaningful.
    Obj(BTreeMap<String, Value>),
}

impl Value {
    pub fn obj() -> Value {
        Value::Obj(BTreeMap::new())
    }

    pub fn set(&mut self, key: &str, value: Value) -> &mut Self {
        if let Value::Obj(map) = self {
            map.insert(key.to_string(), value);
        }
        self
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_usize(&self) -> Option<usize> {
        let n = self.as_f64()?;
        if n.is_finite() && n >= 0.0 { Some(n as usize) } else { None }
    }

    /// Serialises to one line. Never emits a raw newline, in any string, at any depth.
    pub fn to_string(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Num(n) => {
                if n.is_finite() {
                    // An id that arrived as 3 has to go back as 3, not 3.0: a client
                    // matching responses by identity would not recognise it.
                    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                        let _ = write!(out, "{}", *n as i64);
                    } else {
                        let _ = write!(out, "{n}");
                    }
                } else {
                    // NaN and infinity are not JSON. Null is the only honest encoding.
                    out.push_str("null");
                }
            }
            Value::Str(s) => write_string(s, out),
            Value::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Value::Obj(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}
impl From<usize> for Value {
    fn from(n: usize) -> Value {
        Value::Num(n as f64)
    }
}
impl From<f64> for Value {
    fn from(n: f64) -> Value {
        Value::Num(n)
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(items: Vec<T>) -> Value {
        Value::Arr(items.into_iter().map(Into::into).collect())
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            // Everything below 0x20 has to be escaped or the JSON is invalid, and
            // some of it would break the line framing outright.
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            // Everything else, accents and emoji included, is written as itself.
            // UTF-8 output is valid JSON and escaping it would only make it longer.
            c => out.push(c),
        }
    }
    out.push('"');
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn parse(input: &str) -> Result<Value, ParseError> {
    let chars: Vec<char> = input.chars().collect();
    let mut p = Parser { c: &chars, i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.c.len() {
        return Err(ParseError(format!("trailing input at char {}", p.i)));
    }
    Ok(v)
}

struct Parser<'a> {
    c: &'a [char],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n') | Some('\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, expected: char) -> Result<(), ParseError> {
        if self.peek() == Some(expected) {
            self.i += 1;
            Ok(())
        } else {
            Err(ParseError(format!(
                "expected '{expected}' at char {}, found {:?}",
                self.i,
                self.peek()
            )))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), ParseError> {
        for expected in word.chars() {
            self.eat(expected)?;
        }
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError(format!("nesting deeper than {MAX_DEPTH}")));
        }
        match self.peek() {
            None => Err(ParseError("unexpected end of input".into())),
            Some('n') => {
                self.literal("null")?;
                Ok(Value::Null)
            }
            Some('t') => {
                self.literal("true")?;
                Ok(Value::Bool(true))
            }
            Some('f') => {
                self.literal("false")?;
                Ok(Value::Bool(false))
            }
            Some('"') => Ok(Value::Str(self.string()?)),
            Some('[') => self.array(depth),
            Some('{') => self.object(depth),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(ParseError(format!("unexpected {c:?} at char {}", self.i))),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.eat('[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(']') {
            self.i += 1;
            return Ok(Value::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    return Ok(Value::Arr(items));
                }
                other => {
                    return Err(ParseError(format!(
                        "expected ',' or ']' at char {}, found {other:?}",
                        self.i
                    )));
                }
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.eat('{')?;
        let mut map = BTreeMap::new();
        self.ws();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(Value::Obj(map));
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.ws();
            self.eat(':')?;
            self.ws();
            let value = self.value(depth + 1)?;
            map.insert(key, value);
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some('}') => {
                    self.i += 1;
                    return Ok(Value::Obj(map));
                }
                other => {
                    return Err(ParseError(format!(
                        "expected ',' or '}}' at char {}, found {other:?}",
                        self.i
                    )));
                }
            }
        }
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            let c = self
                .peek()
                .ok_or_else(|| ParseError("unterminated string".into()))?;
            self.i += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let esc = self
                        .peek()
                        .ok_or_else(|| ParseError("unterminated escape".into()))?;
                    self.i += 1;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{08}'),
                        'f' => out.push('\u{0c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => out.push(self.unicode_escape()?),
                        other => {
                            return Err(ParseError(format!("unknown escape \\{other}")));
                        }
                    }
                }
                // A raw control character inside a string is invalid JSON. Accepting
                // it would also let a newline through, which breaks the framing.
                c if (c as u32) < 0x20 => {
                    return Err(ParseError(format!(
                        "raw control character U+{:04X} in string",
                        c as u32
                    )));
                }
                c => out.push(c),
            }
        }
    }

    /// Reads `\uXXXX`, joining a surrogate pair when it finds one.
    ///
    /// JSON has no way to write a character above U+FFFF directly, so an emoji
    /// arrives as two escapes that mean nothing apart. Handling only the first is the
    /// classic bug: it turns a valid emoji into a replacement character, silently.
    fn unicode_escape(&mut self) -> Result<char, ParseError> {
        let first = self.hex4()?;

        // A high surrogate is the first half of a pair and is meaningless alone.
        if (0xD800..0xDC00).contains(&first) {
            if self.peek() != Some('\\') {
                return Err(ParseError("high surrogate with no pair".into()));
            }
            self.i += 1;
            self.eat('u')?;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err(ParseError("high surrogate followed by a non surrogate".into()));
            }
            let combined = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
            return char::from_u32(combined)
                .ok_or_else(|| ParseError(format!("surrogate pair is not a character: {combined:#x}")));
        }

        if (0xDC00..0xE000).contains(&first) {
            return Err(ParseError("low surrogate with no high surrogate before it".into()));
        }

        char::from_u32(first).ok_or_else(|| ParseError(format!("not a character: {first:#x}")))
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let mut n = 0u32;
        for _ in 0..4 {
            let c = self
                .peek()
                .ok_or_else(|| ParseError("truncated \\u escape".into()))?;
            let d = c
                .to_digit(16)
                .ok_or_else(|| ParseError(format!("{c:?} is not a hex digit")))?;
            n = n * 16 + d;
            self.i += 1;
        }
        Ok(n)
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.i;
        if self.peek() == Some('-') {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.peek() == Some('.') {
            self.i += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.i += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.i += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text: String = self.c[start..self.i].iter().collect();
        text.parse::<f64>()
            .map(Value::Num)
            .map_err(|_| ParseError(format!("not a number: {text}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(s: &str) -> String {
        parse(s).expect("parse").to_string()
    }

    // -- the cases Richard was worried about, which the parser has to handle rather
    // -- than have stripped from it beforehand -----------------------------------

    /// Accents are not a special case and must never be folded. `kb_remember` turns a
    /// claim into a file, so "proteína" arriving as "proteina" would corrupt the base
    /// permanently and without a sound.
    #[test]
    fn accents_survive_a_round_trip_unchanged() {
        let v = parse(r#"{"q":"por que o poke é caro em proteína?"}"#).expect("parse");
        assert_eq!(v.get("q").unwrap().as_str(), Some("por que o poke é caro em proteína?"));
        assert!(v.to_string().contains("proteína"), "written back as itself, not escaped");
    }

    /// A quote inside a string arrives as `\"`. Stripping quotes before parsing would
    /// remove the framing, not protect it.
    #[test]
    fn an_escaped_quote_is_content_and_not_framing() {
        let v = parse(r#"{"claim":"he said \"no\" twice"}"#).expect("parse");
        assert_eq!(v.get("claim").unwrap().as_str(), Some(r#"he said "no" twice"#));
        assert_eq!(round_trip(r#"{"claim":"he said \"no\" twice"}"#), r#"{"claim":"he said \"no\" twice"}"#);
    }

    #[test]
    fn a_backslash_before_a_quote_does_not_end_the_string() {
        let v = parse(r#"{"path":"C:\\Users\\richa\\Desktop"}"#).expect("parse");
        assert_eq!(v.get("path").unwrap().as_str(), Some(r"C:\Users\user\Desktop"));
    }

    #[test]
    fn a_unicode_escape_becomes_its_character() {
        let v = parse(r#""caf\u00e9""#).expect("parse");
        assert_eq!(v.as_str(), Some("café"));
    }

    /// The classic \u bug: an emoji is two escapes that mean nothing apart, and
    /// handling only the first turns it into a replacement character in silence.
    #[test]
    fn a_surrogate_pair_becomes_one_emoji() {
        let v = parse(r#""\ud83d\udd34 red""#).expect("parse");
        assert_eq!(v.as_str(), Some("🔴 red"));
    }

    #[test]
    fn a_lone_high_surrogate_is_an_error_not_a_replacement_character() {
        assert!(parse(r#""\ud83d""#).is_err());
        assert!(parse(r#""\ud83dx""#).is_err());
        assert!(parse(r#""\udd34""#).is_err(), "a low surrogate alone is equally broken");
    }

    /// Yaron's notes contain emoji in headings, so a passage carrying one has to come
    /// back out of the server intact.
    #[test]
    fn emoji_written_directly_survive_a_round_trip() {
        let out = Value::Str("🔴 Poke is expensive".into()).to_string();
        assert_eq!(out, "\"🔴 Poke is expensive\"");
        assert_eq!(parse(&out).unwrap().as_str(), Some("🔴 Poke is expensive"));
    }

    // -- framing -----------------------------------------------------------------

    /// The stdio transport is newline delimited, so a newline inside a passage has to
    /// leave as `\n`. Emitting it raw would split one message into two and desync the
    /// stream for good.
    #[test]
    fn a_newline_in_a_passage_never_reaches_the_output_raw() {
        let out = Value::Str("first line\nsecond line".into()).to_string();
        assert!(!out.contains('\n'), "a raw newline would break the framing");
        assert_eq!(out, r#""first line\nsecond line""#);
        assert_eq!(parse(&out).unwrap().as_str(), Some("first line\nsecond line"));
    }

    #[test]
    fn a_raw_control_character_in_the_input_is_rejected() {
        assert!(parse("\"a\nb\"").is_err());
        assert!(parse("\"a\tb\"").is_err());
    }

    #[test]
    fn every_control_character_is_escaped_on_the_way_out() {
        let out = Value::Str("a\u{01}b".into()).to_string();
        assert_eq!(out, r#""a\u0001b""#);
    }

    // -- numbers, which matter because a JSON-RPC id has to come back identical ----

    #[test]
    fn an_integer_id_goes_back_as_an_integer() {
        assert_eq!(round_trip(r#"{"id":3}"#), r#"{"id":3}"#, "3.0 would not match the client's id");
        assert_eq!(round_trip(r#"{"id":-17}"#), r#"{"id":-17}"#);
    }

    #[test]
    fn a_string_id_stays_a_string() {
        assert_eq!(round_trip(r#"{"id":"req-1"}"#), r#"{"id":"req-1"}"#);
    }

    #[test]
    fn floats_and_exponents_parse() {
        assert_eq!(parse("1.5").unwrap().as_f64(), Some(1.5));
        assert_eq!(parse("-2.5e3").unwrap().as_f64(), Some(-2500.0));
    }

    // -- structure ---------------------------------------------------------------

    #[test]
    fn nested_structures_round_trip() {
        let s = r#"{"a":[1,{"b":[true,false,null]}],"c":{}}"#;
        assert_eq!(round_trip(s), s);
    }

    #[test]
    fn empty_containers_parse() {
        assert_eq!(round_trip("[]"), "[]");
        assert_eq!(round_trip("{}"), "{}");
    }

    #[test]
    fn whitespace_between_tokens_is_ignored() {
        assert_eq!(round_trip("  { \"a\" : [ 1 , 2 ] }  "), r#"{"a":[1,2]}"#);
    }

    /// The input comes from another process. Unbounded recursion turns a hostile or
    /// merely broken message into a stack overflow, which is a crash rather than an
    /// error, and a crashed server loses every in-flight request.
    #[test]
    fn nesting_past_the_limit_is_an_error_and_not_a_crash() {
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(parse(&deep).is_err());
    }

    #[test]
    fn malformed_input_is_rejected_rather_than_guessed_at() {
        for bad in [r#"{"a":}"#, r#"{"a" 1}"#, "[1,2", r#""unterminated"#, "{,}", "tru", ""] {
            assert!(parse(bad).is_err(), "{bad:?} must not parse");
        }
    }

    #[test]
    fn trailing_input_is_rejected() {
        assert!(parse("{} {}").is_err(), "two messages on one line is a framing error");
    }

    #[test]
    fn object_keys_are_written_in_a_stable_order() {
        assert_eq!(round_trip(r#"{"z":1,"a":2}"#), r#"{"a":2,"z":1}"#);
    }
}
