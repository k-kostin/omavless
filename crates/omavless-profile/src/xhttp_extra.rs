// SPDX-License-Identifier: MIT

//! Strict, bounded parsing for the private VLESS XHTTP `extra` JSON object.
//!
//! The parsed tree remains private to this module. Public callers receive only
//! credential-safe shape facts and fixed error classifications. Python remains
//! the production owner and migration oracle for this R2 slice.

use std::collections::BTreeSet;
use std::fmt;

pub const MAX_XHTTP_EXTRA_BYTES: usize = 12 * 1024;
pub const MAX_XHTTP_EXTRA_ITEMS: usize = 160;
pub const MAX_XHTTP_EXTRA_DEPTH: usize = 8;
pub const MAX_XHTTP_EXTRA_STRING_BYTES: usize = 2048;
pub const MAX_XHTTP_EXTRA_KEY_BYTES: usize = 128;

// CPython 3.11+ defaults to this decimal-integer conversion limit. Keeping the
// same explicit bound makes the candidate deterministic instead of depending
// on process-global interpreter configuration. It is not a public value and is
// intentionally separate from the later XHTTP semantic/range validation.
const MAX_DECIMAL_INTEGER_DIGITS: usize = 4300;
const MAX_PARSER_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpExtraError {
    TooLarge,
    InvalidUtf8,
    InvalidJson,
    InvalidUnicode,
    DuplicateFields,
    NestedTooDeeply,
    TooManyValues,
    FieldNameTooLong,
    StringTooLong,
    RootNotObject,
}

impl XhttpExtraError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::InvalidJson => "invalid_json",
            Self::InvalidUnicode => "invalid_unicode",
            Self::DuplicateFields => "duplicate_fields",
            Self::NestedTooDeeply => "nested_too_deeply",
            Self::TooManyValues => "too_many_values",
            Self::FieldNameTooLong => "field_name_too_long",
            Self::StringTooLong => "string_too_long",
            Self::RootNotObject => "root_not_object",
        }
    }
}

impl fmt::Display for XhttpExtraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "VLESS XHTTP extra is too large",
            Self::InvalidUtf8 => "VLESS XHTTP extra is not valid UTF-8",
            Self::InvalidJson => "VLESS XHTTP extra is invalid JSON",
            Self::InvalidUnicode => "VLESS XHTTP extra contains invalid Unicode",
            Self::DuplicateFields => "VLESS XHTTP extra contains duplicate fields",
            Self::NestedTooDeeply => "VLESS XHTTP extra is nested too deeply",
            Self::TooManyValues => "VLESS XHTTP extra contains too many values",
            Self::FieldNameTooLong => "VLESS XHTTP extra field name is too long",
            Self::StringTooLong => "VLESS XHTTP extra string is too long",
            Self::RootNotObject => "VLESS XHTTP extra must be a JSON object",
        })
    }
}

impl std::error::Error for XhttpExtraError {}

/// Credential-safe summary of the accepted private JSON tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XhttpExtraShapeFacts {
    pub raw_empty: bool,
    pub root_field_count: usize,
    pub total_field_count: usize,
    pub value_count: usize,
    pub object_count: usize,
    pub array_count: usize,
    pub string_count: usize,
    pub integer_count: usize,
    pub float_count: usize,
    pub non_finite_float_count: usize,
    pub boolean_count: usize,
    pub null_count: usize,
    pub max_depth: usize,
    pub non_ascii_present: bool,
}

/// Parsed private XHTTP `extra` document.
///
/// `Debug` deliberately exposes only the safe shape summary. The raw object,
/// keys, strings and number tokens are never rendered.
#[derive(Clone, PartialEq)]
pub struct XhttpExtraDocument {
    root: Vec<(String, XhttpJsonValue)>,
    facts: XhttpExtraShapeFacts,
}

impl XhttpExtraDocument {
    #[must_use]
    pub const fn facts(&self) -> XhttpExtraShapeFacts {
        self.facts
    }
}

impl fmt::Debug for XhttpExtraDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XhttpExtraDocument")
            .field("facts", &self.facts)
            .field("private_root_field_count", &self.root.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq)]
enum XhttpJsonValue {
    Object(Vec<(String, Self)>),
    Array(Vec<Self>),
    String(String),
    Number(XhttpJsonNumber),
    Boolean(bool),
    Null,
}

impl fmt::Debug for XhttpJsonValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Object(fields) => formatter
                .debug_struct("PrivateObject")
                .field("field_count", &fields.len())
                .finish(),
            Self::Array(values) => formatter
                .debug_struct("PrivateArray")
                .field("value_count", &values.len())
                .finish(),
            Self::String(value) => formatter
                .debug_struct("PrivateString")
                .field("byte_length", &value.len())
                .finish(),
            Self::Number(number) => number.fmt(formatter),
            Self::Boolean(value) => {
                let _private_value = *value;
                formatter.write_str("PrivateBoolean")
            }
            Self::Null => formatter.write_str("Null"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum XhttpJsonNumberKind {
    Integer,
    Float,
}

#[derive(Clone, PartialEq, Eq)]
struct XhttpJsonNumber {
    kind: XhttpJsonNumberKind,
    token: String,
    non_finite: bool,
}

impl fmt::Debug for XhttpJsonNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateNumber")
            .field(
                "kind",
                &match self.kind {
                    XhttpJsonNumberKind::Integer => "integer",
                    XhttpJsonNumberKind::Float => "float",
                },
            )
            .field("token_byte_length", &self.token.len())
            .field("non_finite", &self.non_finite)
            .finish()
    }
}

/// Parse one bounded UTF-8 JSON object and return only a private document plus
/// credential-safe shape facts.
///
/// Exact empty input preserves the established Python behavior and means an
/// empty object. Whitespace-only input remains invalid JSON.
pub fn parse_xhttp_extra_bytes(input: &[u8]) -> Result<XhttpExtraDocument, XhttpExtraError> {
    if input.len() > MAX_XHTTP_EXTRA_BYTES {
        return Err(XhttpExtraError::TooLarge);
    }

    let raw = std::str::from_utf8(input).map_err(|_| XhttpExtraError::InvalidUtf8)?;
    let raw_empty = raw.is_empty();
    let value = if raw_empty {
        XhttpJsonValue::Object(Vec::new())
    } else {
        Parser::new(raw).parse()?
    };

    let mut facts = ShapeBuilder::new(raw_empty);
    validate_shape(&value, 0, &mut facts)?;

    let XhttpJsonValue::Object(root) = value else {
        return Err(XhttpExtraError::RootNotObject);
    };
    facts.root_field_count = root.len();

    Ok(XhttpExtraDocument {
        root,
        facts: facts.finish(),
    })
}

struct Parser<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn parse(mut self) -> Result<XhttpJsonValue, XhttpExtraError> {
        self.skip_whitespace();
        if self.is_end() {
            return Err(XhttpExtraError::InvalidJson);
        }
        let value = self.parse_value(0)?;
        self.skip_whitespace();
        if !self.is_end() {
            return Err(XhttpExtraError::InvalidJson);
        }
        Ok(value)
    }

    fn parse_value(&mut self, depth: usize) -> Result<XhttpJsonValue, XhttpExtraError> {
        if depth > MAX_PARSER_DEPTH {
            return Err(XhttpExtraError::NestedTooDeeply);
        }
        match self.peek_byte() {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(XhttpJsonValue::String),
            Some(b't') => {
                self.consume_literal("true")?;
                Ok(XhttpJsonValue::Boolean(true))
            }
            Some(b'f') => {
                self.consume_literal("false")?;
                Ok(XhttpJsonValue::Boolean(false))
            }
            Some(b'n') if self.remaining().starts_with("null") => {
                self.consume_literal("null")?;
                Ok(XhttpJsonValue::Null)
            }
            Some(b'N') if self.remaining().starts_with("NaN") => {
                self.consume_literal("NaN")?;
                Ok(XhttpJsonValue::Number(XhttpJsonNumber {
                    kind: XhttpJsonNumberKind::Float,
                    token: "NaN".to_owned(),
                    non_finite: true,
                }))
            }
            Some(b'I') if self.remaining().starts_with("Infinity") => {
                self.consume_literal("Infinity")?;
                Ok(XhttpJsonValue::Number(XhttpJsonNumber {
                    kind: XhttpJsonNumberKind::Float,
                    token: "Infinity".to_owned(),
                    non_finite: true,
                }))
            }
            Some(b'-') if self.remaining().starts_with("-Infinity") => {
                self.consume_literal("-Infinity")?;
                Ok(XhttpJsonValue::Number(XhttpJsonNumber {
                    kind: XhttpJsonNumberKind::Float,
                    token: "-Infinity".to_owned(),
                    non_finite: true,
                }))
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(XhttpJsonValue::Number),
            _ => Err(XhttpExtraError::InvalidJson),
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<XhttpJsonValue, XhttpExtraError> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        if self.consume_if(b'}') {
            return Ok(XhttpJsonValue::Object(Vec::new()));
        }

        let mut fields = Vec::new();
        let mut names = BTreeSet::new();
        loop {
            if self.peek_byte() != Some(b'"') {
                return Err(XhttpExtraError::InvalidJson);
            }
            let name = self.parse_string()?;
            if !names.insert(name.clone()) {
                return Err(XhttpExtraError::DuplicateFields);
            }
            self.skip_whitespace();
            self.expect_byte(b':')?;
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            fields.push((name, value));
            self.skip_whitespace();
            if self.consume_if(b'}') {
                break;
            }
            self.expect_byte(b',')?;
            self.skip_whitespace();
        }
        Ok(XhttpJsonValue::Object(fields))
    }

    fn parse_array(&mut self, depth: usize) -> Result<XhttpJsonValue, XhttpExtraError> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        if self.consume_if(b']') {
            return Ok(XhttpJsonValue::Array(Vec::new()));
        }

        let mut values = Vec::new();
        loop {
            values.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            if self.consume_if(b']') {
                break;
            }
            self.expect_byte(b',')?;
            self.skip_whitespace();
        }
        Ok(XhttpJsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, XhttpExtraError> {
        self.expect_byte(b'"')?;
        let mut output = String::new();
        loop {
            let Some(character) = self.remaining().chars().next() else {
                return Err(XhttpExtraError::InvalidJson);
            };
            self.index += character.len_utf8();
            match character {
                '"' => return Ok(output),
                '\\' => self.parse_escape(&mut output)?,
                character if character <= '\u{001f}' => {
                    return Err(XhttpExtraError::InvalidJson);
                }
                character => output.push(character),
            }
        }
    }

    fn parse_escape(&mut self, output: &mut String) -> Result<(), XhttpExtraError> {
        let Some(escaped) = self.peek_byte() else {
            return Err(XhttpExtraError::InvalidJson);
        };
        self.index += 1;
        match escaped {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => {
                let first = self.parse_hex_quad()?;
                if (0xd800..=0xdbff).contains(&first) {
                    if !self.remaining().starts_with("\\u") {
                        return Err(XhttpExtraError::InvalidUnicode);
                    }
                    self.index += 2;
                    let second = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(XhttpExtraError::InvalidUnicode);
                    }
                    let scalar = 0x1_0000
                        + ((u32::from(first) - 0xd800) << 10)
                        + (u32::from(second) - 0xdc00);
                    let character =
                        char::from_u32(scalar).ok_or(XhttpExtraError::InvalidUnicode)?;
                    output.push(character);
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(XhttpExtraError::InvalidUnicode);
                } else {
                    let character = char::from_u32(u32::from(first))
                        .ok_or(XhttpExtraError::InvalidUnicode)?;
                    output.push(character);
                }
            }
            _ => return Err(XhttpExtraError::InvalidJson),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, XhttpExtraError> {
        if self.remaining().len() < 4 {
            return Err(XhttpExtraError::InvalidJson);
        }
        let mut value = 0_u16;
        for _ in 0..4 {
            let byte = self.peek_byte().ok_or(XhttpExtraError::InvalidJson)?;
            self.index += 1;
            value = value
                .checked_mul(16)
                .and_then(|current| current.checked_add(u16::from(hex_value(byte)?)))
                .ok_or(XhttpExtraError::InvalidJson)?;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<XhttpJsonNumber, XhttpExtraError> {
        let start = self.index;
        self.consume_if(b'-');

        let integer_start = self.index;
        match self.peek_byte() {
            Some(b'0') => {
                self.index += 1;
                if matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                    return Err(XhttpExtraError::InvalidJson);
                }
            }
            Some(b'1'..=b'9') => {
                self.index += 1;
                while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                    self.index += 1;
                }
            }
            _ => return Err(XhttpExtraError::InvalidJson),
        }
        let integer_digits = self.index - integer_start;

        let mut kind = XhttpJsonNumberKind::Integer;
        if self.consume_if(b'.') {
            kind = XhttpJsonNumberKind::Float;
            let fraction_start = self.index;
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
            if self.index == fraction_start {
                return Err(XhttpExtraError::InvalidJson);
            }
        }

        if matches!(self.peek_byte(), Some(b'e' | b'E')) {
            kind = XhttpJsonNumberKind::Float;
            self.index += 1;
            if matches!(self.peek_byte(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            let exponent_start = self.index;
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
            if self.index == exponent_start {
                return Err(XhttpExtraError::InvalidJson);
            }
        }

        if !self.is_value_delimiter() {
            return Err(XhttpExtraError::InvalidJson);
        }
        if kind == XhttpJsonNumberKind::Integer
            && integer_digits > MAX_DECIMAL_INTEGER_DIGITS
        {
            return Err(XhttpExtraError::InvalidJson);
        }

        let token = self.input[start..self.index].to_owned();
        let non_finite = kind == XhttpJsonNumberKind::Float
            && token.parse::<f64>().is_ok_and(|value| !value.is_finite());
        Ok(XhttpJsonNumber {
            kind,
            token,
            non_finite,
        })
    }

    fn consume_literal(&mut self, literal: &str) -> Result<(), XhttpExtraError> {
        if !self.remaining().starts_with(literal) {
            return Err(XhttpExtraError::InvalidJson);
        }
        self.index += literal.len();
        if !self.is_value_delimiter() {
            return Err(XhttpExtraError::InvalidJson);
        }
        Ok(())
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), XhttpExtraError> {
        if self.consume_if(expected) {
            Ok(())
        } else {
            Err(XhttpExtraError::InvalidJson)
        }
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        if self.peek_byte() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn is_value_delimiter(&self) -> bool {
        self.peek_byte().is_none_or(|byte| {
            matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
        })
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.index += 1;
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.index).copied()
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.index..]
    }

    fn is_end(&self) -> bool {
        self.index == self.input.len()
    }
}

const fn hex_value(byte: u8) -> Result<u8, XhttpExtraError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(XhttpExtraError::InvalidJson),
    }
}

struct ShapeBuilder {
    facts: XhttpExtraShapeFacts,
}

impl ShapeBuilder {
    const fn new(raw_empty: bool) -> Self {
        Self {
            facts: XhttpExtraShapeFacts {
                raw_empty,
                root_field_count: 0,
                total_field_count: 0,
                value_count: 0,
                object_count: 0,
                array_count: 0,
                string_count: 0,
                integer_count: 0,
                float_count: 0,
                non_finite_float_count: 0,
                boolean_count: 0,
                null_count: 0,
                max_depth: 0,
                non_ascii_present: false,
            },
        }
    }

    const fn finish(self) -> XhttpExtraShapeFacts {
        self.facts
    }
}

fn validate_shape(
    value: &XhttpJsonValue,
    depth: usize,
    builder: &mut ShapeBuilder,
) -> Result<(), XhttpExtraError> {
    if depth > MAX_XHTTP_EXTRA_DEPTH {
        return Err(XhttpExtraError::NestedTooDeeply);
    }
    builder.facts.value_count += 1;
    if builder.facts.value_count > MAX_XHTTP_EXTRA_ITEMS {
        return Err(XhttpExtraError::TooManyValues);
    }
    builder.facts.max_depth = builder.facts.max_depth.max(depth);

    match value {
        XhttpJsonValue::Object(fields) => {
            builder.facts.object_count += 1;
            for (name, child) in fields {
                if name.len() > MAX_XHTTP_EXTRA_KEY_BYTES {
                    return Err(XhttpExtraError::FieldNameTooLong);
                }
                builder.facts.total_field_count += 1;
                builder.facts.non_ascii_present |= !name.is_ascii();
                validate_shape(child, depth + 1, builder)?;
            }
        }
        XhttpJsonValue::Array(values) => {
            builder.facts.array_count += 1;
            for child in values {
                validate_shape(child, depth + 1, builder)?;
            }
        }
        XhttpJsonValue::String(text) => {
            if text.len() > MAX_XHTTP_EXTRA_STRING_BYTES {
                return Err(XhttpExtraError::StringTooLong);
            }
            builder.facts.string_count += 1;
            builder.facts.non_ascii_present |= !text.is_ascii();
        }
        XhttpJsonValue::Number(number) => match number.kind {
            XhttpJsonNumberKind::Integer => builder.facts.integer_count += 1,
            XhttpJsonNumberKind::Float => {
                builder.facts.float_count += 1;
                builder.facts.non_finite_float_count += usize::from(number.non_finite);
            }
        },
        XhttpJsonValue::Boolean(value) => {
            let _private_value = *value;
            builder.facts.boolean_count += 1;
        }
        XhttpJsonValue::Null => builder.facts.null_count += 1,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_an_empty_object() {
        let document = parse_xhttp_extra_bytes(b"").expect("empty input must be accepted");
        assert_eq!(document.facts().value_count, 1);
        assert_eq!(document.facts().object_count, 1);
        assert!(document.facts().raw_empty);
    }

    #[test]
    fn private_values_are_redacted_from_debug() {
        let document = parse_xhttp_extra_bytes(
            br#"{"private-key":"private-value","number":123456789}"#,
        )
        .expect("private fixture must parse");
        let rendered = format!("{document:?}");
        assert!(!rendered.contains("private-key"));
        assert!(!rendered.contains("private-value"));
        assert!(!rendered.contains("123456789"));
    }

    #[test]
    fn duplicate_names_are_rejected_after_unescaping() {
        assert_eq!(
            parse_xhttp_extra_bytes(br#"{"a":1,"\u0061":2}"#),
            Err(XhttpExtraError::DuplicateFields)
        );
    }

    #[test]
    fn python_constants_and_overflowing_float_are_counted_safely() {
        let facts = parse_xhttp_extra_bytes(
            br#"{"nan":NaN,"positive":Infinity,"negative":-Infinity,"overflow":1e400}"#,
        )
        .expect("Python-compatible constants must parse")
        .facts();
        assert_eq!(facts.float_count, 4);
        assert_eq!(facts.non_finite_float_count, 4);
    }

    #[test]
    fn lone_surrogates_fail_closed() {
        assert_eq!(
            parse_xhttp_extra_bytes(br#"{"value":"\ud800"}"#),
            Err(XhttpExtraError::InvalidUnicode)
        );
    }
}
