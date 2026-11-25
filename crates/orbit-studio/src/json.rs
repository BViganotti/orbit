//! Strict, bounded JSON parser and writer.
//!
//! Limits are part of the API contract: 2 MiB input, 64 levels of nesting,
//! 100,000 values and 1 MiB strings. Malformed input returns an error; it never
//! panics. Numbers must be finite.

use std::fmt;

/// Maximum accepted JSON document size.
pub const MAX_INPUT: usize = 2 * 1024 * 1024;
/// Maximum nesting depth.
pub const MAX_DEPTH: usize = 64;
/// Maximum total values in one document.
pub const MAX_VALUES: usize = 100_000;
/// Maximum string length in bytes after unescaping.
pub const MAX_STRING: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(v) if v.is_finite() => Some(*v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(v) => Some(v),
            _ => None,
        }
    }

    pub fn number(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    pub fn array(&self, key: &str) -> Option<&[Value]> {
        self.get(key).and_then(Value::as_array)
    }

    pub fn obj(&self, key: &str) -> Option<&[(String, Value)]> {
        self.get(key).and_then(Value::as_object)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub offset: usize,
}

impl JsonError {
    fn new(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for JsonError {}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
    values: usize,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            depth: 0,
            values: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn count_value(&mut self) -> Result<(), JsonError> {
        self.values += 1;
        if self.values > MAX_VALUES {
            return Err(JsonError::new("too many values", self.pos));
        }
        Ok(())
    }

    fn parse_value(&mut self) -> Result<Value, JsonError> {
        self.skip_ws();
        self.count_value()?;
        match self.peek() {
            Some(b'n') => {
                self.keyword(b"null")?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.keyword(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.keyword(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(JsonError::new("unexpected character", self.pos)),
            None => Err(JsonError::new("unexpected end of input", self.pos)),
        }
    }

    fn keyword(&mut self, word: &[u8]) -> Result<(), JsonError> {
        if self.bytes.len() < self.pos + word.len() {
            return Err(JsonError::new("truncated literal", self.pos));
        }
        if &self.bytes[self.pos..self.pos + word.len()] != word {
            return Err(JsonError::new("invalid literal", self.pos));
        }
        self.pos += word.len();
        Ok(())
    }

    fn parse_array(&mut self) -> Result<Value, JsonError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(JsonError::new("nesting too deep", self.pos));
        }
        self.bump();
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                Some(_) => return Err(JsonError::new("expected ',' or ']'", self.pos)),
                None => return Err(JsonError::new("unterminated array", self.pos)),
            }
        }
        self.depth -= 1;
        Ok(Value::Array(items))
    }

    fn parse_object(&mut self) -> Result<Value, JsonError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(JsonError::new("nesting too deep", self.pos));
        }
        self.bump();
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            self.depth -= 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(JsonError::new("object key must be a string", self.pos));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.bump() != Some(b':') {
                return Err(JsonError::new("expected ':'", self.pos));
            }
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                Some(_) => return Err(JsonError::new("expected ',' or '}'", self.pos)),
                None => return Err(JsonError::new("unterminated object", self.pos)),
            }
        }
        self.depth -= 1;
        Ok(Value::Object(entries))
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        if self.bump() != Some(b'"') {
            return Err(JsonError::new("expected string", self.pos));
        }
        let mut out = String::new();
        loop {
            let b = self
                .bump()
                .ok_or_else(|| JsonError::new("unterminated string", self.pos))?;
            match b {
                b'"' => break,
                b'\\' => {
                    let esc = self
                        .bump()
                        .ok_or_else(|| JsonError::new("unterminated escape", self.pos))?;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let cp = self.parse_hex4()?;
                            if (0xD800..=0xDBFF).contains(&cp) {
                                if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                                    return Err(JsonError::new(
                                        "high surrogate without low surrogate",
                                        self.pos,
                                    ));
                                }
                                let low = self.parse_hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&low) {
                                    return Err(JsonError::new("invalid low surrogate", self.pos));
                                }
                                let c = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                let ch = char::from_u32(c).ok_or_else(|| {
                                    JsonError::new("invalid code point", self.pos)
                                })?;
                                out.push(ch);
                            } else if (0xDC00..=0xDFFF).contains(&cp) {
                                return Err(JsonError::new("unexpected low surrogate", self.pos));
                            } else {
                                let ch = char::from_u32(cp).ok_or_else(|| {
                                    JsonError::new("invalid code point", self.pos)
                                })?;
                                out.push(ch);
                            }
                        }
                        _ => return Err(JsonError::new("invalid escape", self.pos)),
                    }
                }
                0x00..=0x1f => return Err(JsonError::new("control character in string", self.pos)),
                _ => {
                    // Re-decode UTF-8 from the current position.
                    let start = self.pos - 1;
                    let width = utf8_width(b)
                        .ok_or_else(|| JsonError::new("invalid UTF-8 lead byte", start))?;
                    if start + width > self.bytes.len() {
                        return Err(JsonError::new("truncated UTF-8 sequence", start));
                    }
                    let s = std::str::from_utf8(&self.bytes[start..start + width])
                        .map_err(|_| JsonError::new("invalid UTF-8", start))?;
                    out.push_str(s);
                    self.pos = start + width;
                }
            }
            if out.len() > MAX_STRING {
                return Err(JsonError::new("string too long", self.pos));
            }
        }
        Ok(out)
    }

    fn parse_hex4(&mut self) -> Result<u32, JsonError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let b = self
                .bump()
                .ok_or_else(|| JsonError::new("truncated unicode escape", self.pos))?;
            let digit = match b {
                b'0'..=b'9' => u32::from(b - b'0'),
                b'a'..=b'f' => u32::from(b - b'a') + 10,
                b'A'..=b'F' => u32::from(b - b'A') + 10,
                _ => return Err(JsonError::new("invalid hex digit", self.pos)),
            };
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek() {
            Some(b'0') => {
                self.bump();
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(JsonError::new("leading zero in number", self.pos));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump();
                }
            }
            _ => return Err(JsonError::new("invalid number", self.pos)),
        }
        if self.peek() == Some(b'.') {
            self.bump();
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(JsonError::new("missing fraction digits", self.pos));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(JsonError::new("missing exponent digits", self.pos));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| JsonError::new("invalid number bytes", start))?;
        let value: f64 = text
            .parse()
            .map_err(|_| JsonError::new("invalid number", start))?;
        if !value.is_finite() {
            return Err(JsonError::new("number is not finite", start));
        }
        Ok(Value::Number(value))
    }
}

fn utf8_width(lead: u8) -> Option<usize> {
    match lead {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

/// Parse a JSON document.
pub fn parse(input: &str) -> Result<Value, JsonError> {
    if input.len() > MAX_INPUT {
        return Err(JsonError::new("document too large", 0));
    }
    let mut parser = Parser::new(input.as_bytes());
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos != input.len() {
        return Err(JsonError::new("trailing data", parser.pos));
    }
    Ok(value)
}

/// Serialize a value.
pub fn stringify(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.is_finite() {
                if *n == 0.0 {
                    out.push('0');
                } else {
                    out.push_str(&format!("{n}"));
                }
            } else {
                out.push_str("null");
            }
        }
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (i, (key, item)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Convenience constructors.
pub fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

pub fn number(v: f64) -> Value {
    Value::Number(v)
}

pub fn string(v: impl Into<String>) -> Value {
    Value::String(v.into())
}

pub fn array(v: Vec<Value>) -> Value {
    Value::Array(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_document() {
        let text = r#"{"a":1,"b":[true,false,null,"x\n\u00e9"],"c":{"d":-2.5e3}}"#;
        let value = parse(text).unwrap();
        assert_eq!(value.number("a"), Some(1.0));
        assert_eq!(value.array("b").unwrap().len(), 4);
        assert_eq!(value.get("c").unwrap().number("d"), Some(-2500.0));
        let encoded = stringify(&value);
        let again = parse(&encoded).unwrap();
        assert_eq!(again, value);
    }

    #[test]
    fn rejects_malformed_documents() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\":}",
            "{\"a\" 1}",
            "tru",
            "01",
            "1.",
            "1e",
            "1e9999",
            "\"\\uD800\"",
            "\"\\uDC00\"",
            "\"unterminated",
            "{} trailing",
            "[1 2]",
            "{\"a\":1,}",
        ] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn enforces_depth_and_size() {
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(&deep).is_err());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok).is_ok());
    }

    #[test]
    fn handles_unicode_surrogates() {
        let v = parse(r#""\uD83D\uDE80""#).unwrap();
        assert_eq!(v.as_str(), Some("🚀"));
    }

    #[test]
    fn numbers_are_finite_and_bounded() {
        assert!(parse("1e308").is_ok());
        assert!(parse("1e309").is_err());
        assert!(parse("-1e309").is_err());
    }

    #[test]
    fn writer_escapes_control_characters() {
        let v = Value::String("a\nb\t\u{0001}".into());
        let s = stringify(&v);
        assert!(s.contains("\\n"));
        assert!(s.contains("\\u0001"));
        assert_eq!(parse(&s).unwrap(), v);
    }

    #[test]
    fn duplicate_keys_use_last_value() {
        let v = parse(r#"{"a":1,"a":2}"#).unwrap();
        assert_eq!(v.number("a"), Some(2.0));
    }
}
