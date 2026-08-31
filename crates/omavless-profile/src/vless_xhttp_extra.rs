// SPDX-License-Identifier: MIT

//! Bounded, credential-safe decoding of VLESS XHTTP `extra` JSON.
//!
//! This R2h1 module owns only the JSON decoder and shape contract. It retains
//! the private parsed object for later normalized XHTTP slices, while public
//! facts and `Debug` output expose only bounded structural information.

use std::collections::BTreeSet;
use std::fmt;

pub const MAX_XHTTP_EXTRA_BYTES: usize = 12 * 1024;
pub const MAX_XHTTP_EXTRA_ITEMS: usize = 160;
pub const MAX_XHTTP_EXTRA_DEPTH: usize = 8;
pub const MAX_XHTTP_EXTRA_KEY_BYTES: usize = 128;
pub const MAX_XHTTP_EXTRA_STRING_BYTES: usize = 2048;

const MAX_PARSER_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XhttpExtraFacts {
    pub source_empty: bool,
    pub root_field_count: usize,
    pub value_count: usize,
    pub object_count: usize,
    pub array_count: usize,
    pub string_count: usize,
    pub integer_count: usize,
    pub float_count: usize,
    pub boolean_count: usize,
    pub true_count: usize,
    pub false_count: usize,
    pub null_count: usize,
    pub maximum_depth: usize,
    pub non_ascii_present: bool,
}

pub struct XhttpExtraDocument {
    root: Vec<(String, JsonValue)>,
    facts: XhttpExtraFacts,
}

impl XhttpExtraDocument {
    #[must_use]
    pub const fn facts(&self) -> XhttpExtraFacts {
        self.facts
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }
}

impl fmt::Debug for XhttpExtraDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XhttpExtraDocument")
            .field("facts", &self.facts)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpExtraError {
    InvalidUtf8,
    TooLarge,
    InvalidJson,
    DuplicateFields,
    TooDeep,
    TooManyValues,
    OversizedFieldName,
    OversizedString,
    NonObjectRoot,
}

impl XhttpExtraError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "invalid_utf8",
            Self::TooLarge => "too_large",
            Self::InvalidJson => "invalid_json",
            Self::DuplicateFields => "duplicate_fields",
            Self::TooDeep => "too_deep",
            Self::TooManyValues => "too_many_values",
            Self::OversizedFieldName => "oversized_field_name",
            Self::OversizedString => "oversized_string",
            Self::NonObjectRoot => "non_object_root",
        }
    }
}

impl fmt::Display for XhttpExtraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUtf8 => "VLESS XHTTP extra is not valid UTF-8",
            Self::TooLarge => "VLESS XHTTP extra is too large",
            Self::InvalidJson => "VLESS XHTTP extra is not valid JSON",
            Self::DuplicateFields => "VLESS XHTTP extra contains duplicate fields",
            Self::TooDeep => "VLESS XHTTP extra is nested too deeply",
            Self::TooManyValues => "VLESS XHTTP extra contains too many values",
            Self::OversizedFieldName => "VLESS XHTTP extra contains an oversized field name",
            Self::OversizedString => "VLESS XHTTP extra contains an oversized string",
            Self::NonObjectRoot => "VLESS XHTTP extra must be a JSON object",
        })
    }
}

impl std::error::Error for XhttpExtraError {}

enum JsonNumber {
    Integer(String),
    Float(String),
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

enum JsonValue {
    Null,
    Boolean(bool),
    Number(JsonNumber),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}

pub fn decode_xhttp_extra_bytes(input: &[u8]) -> Result<XhttpExtraDocument, XhttpExtraError> {
    if input.len() > MAX_XHTTP_EXTRA_BYTES {
        return Err(XhttpExtraError::TooLarge);
    }
    let text = std::str::from_utf8(input).map_err(|_| XhttpExtraError::InvalidUtf8)?;
    decode_xhttp_extra(text)
}

pub fn decode_xhttp_extra(input: &str) -> Result<XhttpExtraDocument, XhttpExtraError> {
    if input.len() > MAX_XHTTP_EXTRA_BYTES {
        return Err(XhttpExtraError::TooLarge);
    }

    let source_empty = input.is_empty();
    let value = if source_empty {
        JsonValue::Object(Vec::new())
    } else {
        Parser::new(input).parse()?
    };

    let mut facts = XhttpExtraFacts {
        source_empty,
        ..XhttpExtraFacts::default()
    };
    validate_shape(&value, 0, &mut facts)?;
    let JsonValue::Object(root) = value else {
        return Err(XhttpExtraError::NonObjectRoot);
    };
    facts.root_field_count = root.len();
    Ok(XhttpExtraDocument { root, facts })
}

fn validate_shape(
    value: &JsonValue,
    depth: usize,
    facts: &mut XhttpExtraFacts,
) -> Result<(), XhttpExtraError> {
    if depth > MAX_XHTTP_EXTRA_DEPTH {
        return Err(XhttpExtraError::TooDeep);
    }
    facts.value_count += 1;
    if facts.value_count > MAX_XHTTP_EXTRA_ITEMS {
        return Err(XhttpExtraError::TooManyValues);
    }
    facts.maximum_depth = facts.maximum_depth.max(depth);

    match value {
        JsonValue::Null => facts.null_count += 1,
        JsonValue::Boolean(value) => {
            facts.boolean_count += 1;
            if *value {
                facts.true_count += 1;
            } else {
                facts.false_count += 1;
            }
        }
        JsonValue::Number(JsonNumber::Integer(raw)) => {
            debug_assert!(!raw.is_empty());
            facts.integer_count += 1;
        }
        JsonValue::Number(JsonNumber::Float(raw)) => {
            debug_assert!(!raw.is_empty());
            facts.float_count += 1;
        }
        JsonValue::Number(
            JsonNumber::NaN | JsonNumber::PositiveInfinity | JsonNumber::NegativeInfinity,
        ) => facts.float_count += 1,
        JsonValue::String(value) => {
            if value.len() > MAX_XHTTP_EXTRA_STRING_BYTES {
                return Err(XhttpExtraError::OversizedString);
            }
            facts.string_count += 1;
            facts.non_ascii_present |= !value.is_ascii();
        }
        JsonValue::Array(values) => {
            facts.array_count += 1;
            for child in values {
                validate_shape(child, depth + 1, facts)?;
            }
        }
        JsonValue::Object(values) => {
            facts.object_count += 1;
            for (key, child) in values {
                if key.len() > MAX_XHTTP_EXTRA_KEY_BYTES {
                    return Err(XhttpExtraError::OversizedFieldName);
                }
                facts.non_ascii_present |= !key.is_ascii();
                validate_shape(child, depth + 1, facts)?;
            }
        }
    }
    Ok(())
}

struct Parser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            offset: 0,
        }
    }

    fn parse(mut self) -> Result<JsonValue, XhttpExtraError> {
        self.skip_whitespace();
        let value = self.parse_value(0)?;
        self.skip_whitespace();
        if self.offset != self.bytes.len() {
            return Err(XhttpExtraError::InvalidJson);
        }
        Ok(value)
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonValue, XhttpExtraError> {
        if depth > MAX_PARSER_DEPTH {
            return Err(XhttpExtraError::TooDeep);
        }
        match self.peek() {
            Some(b'n') => {
                self.consume_literal(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b't') => {
                self.consume_literal(b"true")?;
                Ok(JsonValue::Boolean(true))
            }
            Some(b'f') => {
                self.consume_literal(b"false")?;
                Ok(JsonValue::Boolean(false))
            }
            Some(b'N') => {
                self.consume_literal(b"NaN")?;
                Ok(JsonValue::Number(JsonNumber::NaN))
            }
            Some(b'I') => {
                self.consume_literal(b"Infinity")?;
                Ok(JsonValue::Number(JsonNumber::PositiveInfinity))
            }
            Some(b'-') if self.remaining().starts_with("-Infinity") => {
                self.consume_literal(b"-Infinity")?;
                Ok(JsonValue::Number(JsonNumber::NegativeInfinity))
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            _ => Err(XhttpExtraError::InvalidJson),
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, XhttpExtraError> {
        let start = self.offset;
        if self.peek() == Some(b'-') {
            self.offset += 1;
        }

        match self.peek() {
            Some(b'0') => self.offset += 1,
            Some(b'1'..=b'9') => {
                self.offset += 1;
                while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.offset += 1;
                }
            }
            _ => return Err(XhttpExtraError::InvalidJson),
        }

        let mut integer = true;
        if self.peek() == Some(b'.') {
            integer = false;
            self.offset += 1;
            let digits = self.offset;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset == digits {
                return Err(XhttpExtraError::InvalidJson);
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            integer = false;
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            let digits = self.offset;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset == digits {
                return Err(XhttpExtraError::InvalidJson);
            }
        }

        let raw = self.input[start..self.offset].to_owned();
        let number = if integer {
            JsonNumber::Integer(raw)
        } else {
            JsonNumber::Float(raw)
        };
        Ok(JsonValue::Number(number))
    }

    fn parse_string(&mut self) -> Result<String, XhttpExtraError> {
        self.expect(b'"')?;
        let mut result = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(XhttpExtraError::InvalidJson);
            };
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(result);
                }
                b'\\' => {
                    self.offset += 1;
                    self.parse_escape(&mut result)?;
                }
                0x00..=0x1f => return Err(XhttpExtraError::InvalidJson),
                _ => {
                    let character = self
                        .remaining()
                        .chars()
                        .next()
                        .ok_or(XhttpExtraError::InvalidJson)?;
                    result.push(character);
                    self.offset += character.len_utf8();
                }
            }
        }
    }

    fn parse_escape(&mut self, result: &mut String) -> Result<(), XhttpExtraError> {
        let Some(escape) = self.peek() else {
            return Err(XhttpExtraError::InvalidJson);
        };
        self.offset += 1;
        match escape {
            b'"' => result.push('"'),
            b'\\' => result.push('\\'),
            b'/' => result.push('/'),
            b'b' => result.push('\u{0008}'),
            b'f' => result.push('\u{000c}'),
            b'n' => result.push('\n'),
            b'r' => result.push('\r'),
            b't' => result.push('\t'),
            b'u' => {
                let first = self.parse_hex_quad()?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    if !self.remaining().starts_with("\\u") {
                        return Err(XhttpExtraError::InvalidJson);
                    }
                    self.offset += 2;
                    let second = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(XhttpExtraError::InvalidJson);
                    }
                    0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(XhttpExtraError::InvalidJson);
                } else {
                    u32::from(first)
                };
                result.push(char::from_u32(scalar).ok_or(XhttpExtraError::InvalidJson)?);
            }
            _ => return Err(XhttpExtraError::InvalidJson),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, XhttpExtraError> {
        if self.bytes.len().saturating_sub(self.offset) < 4 {
            return Err(XhttpExtraError::InvalidJson);
        }
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = hex_value(self.bytes[self.offset]).ok_or(XhttpExtraError::InvalidJson)?;
            value = (value << 4) | u16::from(digit);
            self.offset += 1;
        }
        Ok(value)
    }

    fn parse_array(&mut self, depth: usize) -> Result<JsonValue, XhttpExtraError> {
        self.expect(b'[')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.peek() == Some(b']') {
            self.offset += 1;
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.offset += 1;
                    self.skip_whitespace();
                }
                Some(b']') => {
                    self.offset += 1;
                    return Ok(JsonValue::Array(values));
                }
                _ => return Err(XhttpExtraError::InvalidJson),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<JsonValue, XhttpExtraError> {
        self.expect(b'{')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.peek() == Some(b'}') {
            self.offset += 1;
            return Ok(JsonValue::Object(values));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(XhttpExtraError::InvalidJson);
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            values.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.offset += 1;
                    self.skip_whitespace();
                }
                Some(b'}') => {
                    self.offset += 1;
                    reject_duplicate_keys(&values)?;
                    return Ok(JsonValue::Object(values));
                }
                _ => return Err(XhttpExtraError::InvalidJson),
            }
        }
    }

    fn consume_literal(&mut self, literal: &[u8]) -> Result<(), XhttpExtraError> {
        if self.bytes[self.offset..].starts_with(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            Err(XhttpExtraError::InvalidJson)
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), XhttpExtraError> {
        if self.peek() == Some(expected) {
            self.offset += 1;
            Ok(())
        } else {
            Err(XhttpExtraError::InvalidJson)
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.offset += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.offset..]
    }
}

fn reject_duplicate_keys(values: &[(String, JsonValue)]) -> Result<(), XhttpExtraError> {
    let mut seen = BTreeSet::new();
    for (key, _) in values {
        if !seen.insert(key.as_str()) {
            return Err(XhttpExtraError::DuplicateFields);
        }
    }
    Ok(())
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_python_numeric_extensions_are_bounded() {
        let empty = decode_xhttp_extra("").expect("empty object");
        assert!(empty.is_empty());
        assert_eq!(
            empty.facts(),
            XhttpExtraFacts {
                source_empty: true,
                value_count: 1,
                object_count: 1,
                ..XhttpExtraFacts::default()
            }
        );

        let extensions = decode_xhttp_extra(
            r#"{"integer":123456789012345678901234567890,"nan":NaN,"positive":Infinity,"negative":-Infinity,"overflow":1e400}"#,
        )
        .expect("Python-compatible numeric values");
        assert_eq!(extensions.facts().integer_count, 1);
        assert_eq!(extensions.facts().float_count, 4);
    }

    #[test]
    fn duplicate_keys_are_detected_after_decoding() {
        assert!(matches!(
            decode_xhttp_extra(r#"{"a":1,"\u0061":2}"#),
            Err(XhttpExtraError::DuplicateFields)
        ));
        assert!(matches!(
            decode_xhttp_extra(r#"{"outer":{"a":1,"a":2}}"#),
            Err(XhttpExtraError::DuplicateFields)
        ));
    }

    #[test]
    fn debug_and_errors_do_not_reveal_private_values() {
        let document =
            decode_xhttp_extra(r#"{"password":"private-secret","uri":"vless://private.invalid"}"#)
                .expect("private object");
        let debug = format!("{document:?}");
        for marker in ["private-secret", "vless://", "password", "uri"] {
            assert!(!debug.contains(marker));
        }

        for error in [
            XhttpExtraError::InvalidUtf8,
            XhttpExtraError::TooLarge,
            XhttpExtraError::InvalidJson,
            XhttpExtraError::DuplicateFields,
            XhttpExtraError::TooDeep,
            XhttpExtraError::TooManyValues,
            XhttpExtraError::OversizedFieldName,
            XhttpExtraError::OversizedString,
            XhttpExtraError::NonObjectRoot,
        ] {
            assert!(error.to_string().len() <= 80);
            assert!(!error.to_string().contains("private"));
        }
    }
}
