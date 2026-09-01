// SPDX-License-Identifier: MIT

//! Bounded, credential-safe decoding of VLESS XHTTP `extra` JSON.
//!
//! This R2h1 module owns only the JSON decoder and shape contract. It retains
//! the private parsed object for later normalized XHTTP slices, while public
//! facts and `Debug` output expose only bounded structural information.

use std::collections::BTreeSet;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::vless::HostKind;
use crate::vless_query::{valid_reality_public_key, valid_reality_short_id};

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

#[derive(Clone)]
enum JsonNumber {
    Integer(String),
    Float(String),
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

#[derive(Clone)]
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

/// Maximum number of XHTTP headers accepted by the production Python oracle.
pub const MAX_XHTTP_HEADER_COUNT: usize = 32;
/// Maximum byte length of one XHTTP header value.
pub const MAX_XHTTP_HEADER_VALUE_BYTES: usize = 1024;
const MAX_XHTTP_HEADER_NAME_BYTES: usize = 64;
const MAX_XHTTP_ASCII_BYTES: usize = 128;
const MAX_XHTTP_TOKEN_BYTES: usize = 64;
const MAX_XHTTP_RANGE_VALUE: u32 = 2_147_483_647;

/// A canonical inclusive non-negative XHTTP integer range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XhttpRange {
    pub start: u32,
    pub end: u32,
}

impl XhttpRange {
    #[must_use]
    pub const fn is_single(self) -> bool {
        self.start == self.end
    }
}

/// Public XHTTP placement vocabulary shared by session and sequence fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpPlacement {
    Path,
    Query,
    Cookie,
    Header,
}

impl XhttpPlacement {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Cookie => "cookie",
            Self::Header => "header",
        }
    }
}

/// Public XHTTP padding-placement vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpPaddingPlacement {
    QueryInHeader,
    Cookie,
    Header,
    Query,
}

impl XhttpPaddingPlacement {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::QueryInHeader => "queryInHeader",
            Self::Cookie => "cookie",
            Self::Header => "header",
            Self::Query => "query",
        }
    }
}

/// Public XHTTP padding-method vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpPaddingMethod {
    RepeatX,
    Tokenish,
}

impl XhttpPaddingMethod {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RepeatX => "repeat-x",
            Self::Tokenish => "tokenish",
        }
    }
}

/// Public XHTTP upload-method vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpHttpMethod {
    Post,
    Put,
    Patch,
    Delete,
}

impl XhttpHttpMethod {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Public XHTTP uplink-data placement vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpDataPlacement {
    Auto,
    Body,
    Cookie,
    Header,
}

impl XhttpDataPlacement {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Body => "body",
            Self::Cookie => "cookie",
            Self::Header => "header",
        }
    }
}

/// Credential-safe normalized facts for the top-level XHTTP option subset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XhttpOptionsFacts {
    pub normalized_field_count: usize,
    pub header_count: usize,
    pub x_padding_bytes: Option<XhttpRange>,
    pub uplink_chunk_size: Option<XhttpRange>,
    pub sc_max_each_post_bytes: Option<XhttpRange>,
    pub sc_min_posts_interval_ms: Option<XhttpRange>,
    pub x_padding_obfs_mode: bool,
    pub no_grpc_header: bool,
    pub x_padding_placement: Option<XhttpPaddingPlacement>,
    pub x_padding_method: Option<XhttpPaddingMethod>,
    pub uplink_http_method: Option<XhttpHttpMethod>,
    pub seq_placement: Option<XhttpPlacement>,
    pub uplink_data_placement: Option<XhttpDataPlacement>,
    pub x_padding_key_present: bool,
    pub x_padding_header_present: bool,
    pub seq_key_present: bool,
    pub uplink_data_key_present: bool,
    pub session_placement: Option<XhttpPlacement>,
    pub session_key_present: bool,
    pub session_table_present: bool,
    pub session_length: Option<XhttpRange>,
    pub reuse_field_count: usize,
    pub max_concurrency: Option<XhttpRange>,
    pub max_connections: Option<XhttpRange>,
    pub c_max_reuse_times: Option<XhttpRange>,
    pub h_max_request_times: Option<XhttpRange>,
    pub h_max_reusable_secs: Option<XhttpRange>,
    pub h_keep_alive_period: Option<i32>,
}

/// Private normalized XHTTP options. Sensitive values remain available to later
/// rendering slices but never appear in `Debug` output or public parity facts.
pub struct XhttpOptions {
    headers: Vec<(String, String)>,
    x_padding_key: Option<String>,
    x_padding_header: Option<String>,
    seq_key: Option<String>,
    uplink_data_key: Option<String>,
    session_key: Option<String>,
    session_table: Option<String>,
    facts: XhttpOptionsFacts,
}

impl XhttpOptions {
    #[must_use]
    pub const fn facts(&self) -> XhttpOptionsFacts {
        self.facts
    }

    fn private_value_count(&self) -> usize {
        self.headers.len()
            + usize::from(self.x_padding_key.is_some())
            + usize::from(self.x_padding_header.is_some())
            + usize::from(self.seq_key.is_some())
            + usize::from(self.uplink_data_key.is_some())
            + usize::from(self.session_key.is_some())
            + usize::from(self.session_table.is_some())
    }

    pub(crate) fn normalized_entries(&self) -> Vec<(String, XhttpValue)> {
        let mut entries = Vec::new();
        if !self.headers.is_empty() {
            entries.push(("headers".to_owned(), headers_value(&self.headers)));
        }
        for (name, value) in [
            ("x-padding-bytes", self.facts.x_padding_bytes),
            ("uplink-chunk-size", self.facts.uplink_chunk_size),
            ("sc-max-each-post-bytes", self.facts.sc_max_each_post_bytes),
            (
                "sc-min-posts-interval-ms",
                self.facts.sc_min_posts_interval_ms,
            ),
        ] {
            if let Some(value) = value {
                entries.push((name.to_owned(), XhttpValue::String(range_text(value))));
            }
        }
        if self.facts.x_padding_obfs_mode {
            entries.push(("x-padding-obfs-mode".to_owned(), XhttpValue::Boolean(true)));
        }
        if self.facts.no_grpc_header {
            entries.push(("no-grpc-header".to_owned(), XhttpValue::Boolean(true)));
        }
        for (name, value) in [
            (
                "x-padding-placement",
                self.facts
                    .x_padding_placement
                    .map(XhttpPaddingPlacement::code),
            ),
            (
                "x-padding-method",
                self.facts.x_padding_method.map(XhttpPaddingMethod::code),
            ),
            (
                "uplink-http-method",
                self.facts.uplink_http_method.map(XhttpHttpMethod::code),
            ),
            (
                "seq-placement",
                self.facts.seq_placement.map(XhttpPlacement::code),
            ),
            (
                "uplink-data-placement",
                self.facts
                    .uplink_data_placement
                    .map(XhttpDataPlacement::code),
            ),
        ] {
            if let Some(value) = value {
                entries.push((name.to_owned(), XhttpValue::String(value.to_owned())));
            }
        }
        for (name, value) in [
            ("x-padding-key", self.x_padding_key.as_deref()),
            ("x-padding-header", self.x_padding_header.as_deref()),
            ("seq-key", self.seq_key.as_deref()),
            ("uplink-data-key", self.uplink_data_key.as_deref()),
        ] {
            if let Some(value) = value {
                entries.push((name.to_owned(), XhttpValue::String(value.to_owned())));
            }
        }
        if let Some(value) = self.facts.session_placement {
            entries.push((
                "session-placement".to_owned(),
                XhttpValue::String(value.code().to_owned()),
            ));
        }
        if let Some(value) = &self.session_key {
            entries.push(("session-key".to_owned(), XhttpValue::String(value.clone())));
        }
        if let Some(value) = &self.session_table {
            entries.push((
                "session-table".to_owned(),
                XhttpValue::String(value.clone()),
            ));
        }
        if let Some(value) = self.facts.session_length {
            entries.push((
                "session-length".to_owned(),
                XhttpValue::String(range_text(value)),
            ));
        }
        let reuse = reuse_value_from_options(self.facts);
        if let Some(reuse) = reuse {
            entries.push(("reuse-settings".to_owned(), reuse));
        }
        debug_assert_eq!(entries.len(), self.facts.normalized_field_count);
        entries
    }
}

impl fmt::Debug for XhttpOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XhttpOptions")
            .field("facts", &self.facts)
            .field("private_value_count", &self.private_value_count())
            .finish_non_exhaustive()
    }
}

/// Fixed, credential-safe top-level XHTTP option errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpOptionsError {
    Decode(XhttpExtraError),
    UnsupportedFields,
    CompatibilityFieldFormat,
    CompatibilityMode,
    RecursiveExtra,
    ServerOnlyField,
    DownloadSettingsOutsideSlice,
    HeadersFormat,
    HeaderName,
    HeaderValue,
    RangeType,
    RangeBounds,
    BooleanType,
    TokenFormat,
    EnumValue,
    AliasConflict,
    SessionPlacement,
    SessionSequenceConflict,
    XmuxObject,
    XmuxFields,
    XmuxExclusive,
    XmuxKeepAlive,
}

impl XhttpOptionsError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Decode(error) => error.code(),
            Self::UnsupportedFields => "unsupported_fields",
            Self::CompatibilityFieldFormat => "compatibility_field_format",
            Self::CompatibilityMode => "compatibility_mode",
            Self::RecursiveExtra => "recursive_extra",
            Self::ServerOnlyField => "server_only_field",
            Self::DownloadSettingsOutsideSlice => "download_settings_outside_slice",
            Self::HeadersFormat => "headers_format",
            Self::HeaderName => "header_name",
            Self::HeaderValue => "header_value",
            Self::RangeType => "range_type",
            Self::RangeBounds => "range_bounds",
            Self::BooleanType => "boolean_type",
            Self::TokenFormat => "token_format",
            Self::EnumValue => "enum_value",
            Self::AliasConflict => "alias_conflict",
            Self::SessionPlacement => "session_placement",
            Self::SessionSequenceConflict => "session_sequence_conflict",
            Self::XmuxObject => "xmux_object",
            Self::XmuxFields => "xmux_fields",
            Self::XmuxExclusive => "xmux_exclusive",
            Self::XmuxKeepAlive => "xmux_keep_alive",
        }
    }
}

impl fmt::Display for XhttpOptionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Decode(error) = self {
            return error.fmt(formatter);
        }
        formatter.write_str(match self {
            Self::Decode(_) => unreachable!(),
            Self::UnsupportedFields => "VLESS XHTTP extra contains unsupported fields",
            Self::CompatibilityFieldFormat => {
                "VLESS XHTTP compatibility field has an invalid format"
            }
            Self::CompatibilityMode => "VLESS XHTTP compatibility mode is invalid",
            Self::RecursiveExtra => "VLESS XHTTP recursive extra objects are not supported",
            Self::ServerOnlyField => "VLESS XHTTP server-only field cannot be imported",
            Self::DownloadSettingsOutsideSlice => {
                "VLESS XHTTP downloadSettings is outside this compatibility slice"
            }
            Self::HeadersFormat => "VLESS XHTTP headers have an invalid format",
            Self::HeaderName => "VLESS XHTTP contains an unsupported header name",
            Self::HeaderValue => "VLESS XHTTP contains an invalid header value",
            Self::RangeType => "VLESS XHTTP range field has an invalid type",
            Self::RangeBounds => "VLESS XHTTP range field is outside the supported range",
            Self::BooleanType => "VLESS XHTTP boolean field has an invalid type",
            Self::TokenFormat => "VLESS XHTTP token field has an invalid format",
            Self::EnumValue => "VLESS XHTTP enum field has an unsupported value",
            Self::AliasConflict => "VLESS XHTTP extra contains conflicting field aliases",
            Self::SessionPlacement => "VLESS XHTTP session placement is unsupported",
            Self::SessionSequenceConflict => "VLESS XHTTP session and sequence placements conflict",
            Self::XmuxObject => "VLESS XHTTP xmux must be an object",
            Self::XmuxFields => "VLESS XHTTP xmux contains unsupported fields",
            Self::XmuxExclusive => "VLESS XHTTP xmux concurrency fields conflict",
            Self::XmuxKeepAlive => "VLESS XHTTP xmux keep-alive period is invalid",
        })
    }
}

impl std::error::Error for XhttpOptionsError {}

impl From<XhttpExtraError> for XhttpOptionsError {
    fn from(error: XhttpExtraError) -> Self {
        Self::Decode(error)
    }
}

/// Decode once through R2h1 and normalize the supported top-level XHTTP option
/// subset. `downloadSettings` is deliberately classified for the next slice.
pub fn parse_xhttp_options(input: &str) -> Result<XhttpOptions, XhttpOptionsError> {
    let document = decode_xhttp_extra(input)?;
    normalize_xhttp_options(document)
}

/// Byte-oriented entry point preserving the R2h1 UTF-8 and size classes.
pub fn parse_xhttp_options_bytes(input: &[u8]) -> Result<XhttpOptions, XhttpOptionsError> {
    let document = decode_xhttp_extra_bytes(input)?;
    normalize_xhttp_options(document)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum XhttpDownloadPolicy {
    Reject,
    Allow,
}

fn normalize_xhttp_options(
    document: XhttpExtraDocument,
) -> Result<XhttpOptions, XhttpOptionsError> {
    let data = &document.root;
    validate_xhttp_options_envelope(data, XhttpDownloadPolicy::Reject)?;
    normalize_xhttp_option_fields(data)
}

fn validate_xhttp_options_envelope(
    data: &[(String, JsonValue)],
    policy: XhttpDownloadPolicy,
) -> Result<(), XhttpOptionsError> {
    const ALLOWED: &[&str] = &[
        "headers",
        "xPaddingBytes",
        "xPaddingObfsMode",
        "xPaddingKey",
        "xPaddingHeader",
        "xPaddingPlacement",
        "xPaddingMethod",
        "uplinkHTTPMethod",
        "seqPlacement",
        "seqKey",
        "uplinkDataPlacement",
        "uplinkDataKey",
        "uplinkChunkSize",
        "noGRPCHeader",
        "scMaxEachPostBytes",
        "scMinPostsIntervalMs",
        "xmux",
        "downloadSettings",
        "sessionIDPlacement",
        "sessionPlacement",
        "sessionIDKey",
        "sessionKey",
        "sessionIDTable",
        "sessionTable",
        "sessionIDLength",
        "sessionLength",
        "host",
        "path",
        "mode",
        "extra",
        "noSSEHeader",
        "scMaxBufferedPosts",
        "scStreamUpServerSecs",
        "serverMaxHeaderBytes",
    ];
    if data
        .iter()
        .any(|(name, _)| !ALLOWED.contains(&name.as_str()))
    {
        return Err(XhttpOptionsError::UnsupportedFields);
    }

    for name in ["host", "path"] {
        if let Some(value) = field(data, name)
            && !matches!(value, JsonValue::Null | JsonValue::String(_))
        {
            return Err(XhttpOptionsError::CompatibilityFieldFormat);
        }
    }
    if let Some(value) = field(data, "mode")
        && !is_null_or_one_of_strings(value, &["", "auto", "stream-one", "stream-up", "packet-up"])
    {
        return Err(XhttpOptionsError::CompatibilityMode);
    }
    if field(data, "extra").is_some_and(|value| !matches!(value, JsonValue::Null)) {
        return Err(XhttpOptionsError::RecursiveExtra);
    }

    for name in [
        "noSSEHeader",
        "scMaxBufferedPosts",
        "scStreamUpServerSecs",
        "serverMaxHeaderBytes",
    ] {
        if let Some(value) = field(data, name) {
            let accepted = if name == "noSSEHeader" {
                matches!(value, JsonValue::Null) || python_equal(value, &JsonValue::Boolean(false))
            } else {
                matches!(value, JsonValue::Null)
                    || python_zero(value)
                    || matches!(value, JsonValue::String(text) if text.is_empty() || text == "0")
            };
            if !accepted {
                return Err(XhttpOptionsError::ServerOnlyField);
            }
        }
    }
    if policy == XhttpDownloadPolicy::Reject
        && field(data, "downloadSettings").is_some_and(|value| !matches!(value, JsonValue::Null))
    {
        return Err(XhttpOptionsError::DownloadSettingsOutsideSlice);
    }
    Ok(())
}

fn normalize_xhttp_option_fields(
    data: &[(String, JsonValue)],
) -> Result<XhttpOptions, XhttpOptionsError> {
    if data.is_empty() {
        return Ok(empty_xhttp_options());
    }
    let headers = parse_headers(field(data, "headers"))?;
    let x_padding_bytes = parse_range(field(data, "xPaddingBytes"))?;
    let uplink_chunk_size = parse_range(field(data, "uplinkChunkSize"))?;
    let sc_max_each_post_bytes = parse_range(field(data, "scMaxEachPostBytes"))?;
    let sc_min_posts_interval_ms = parse_range(field(data, "scMinPostsIntervalMs"))?;

    let x_padding_obfs_mode = parse_boolean(field(data, "xPaddingObfsMode"))?;
    let no_grpc_header = parse_boolean(field(data, "noGRPCHeader"))?;

    let x_padding_placement = parse_padding_placement(field(data, "xPaddingPlacement"))?;
    let x_padding_method = parse_padding_method(field(data, "xPaddingMethod"))?;
    let uplink_http_method = parse_http_method(field(data, "uplinkHTTPMethod"))?;
    let seq_placement = parse_placement(field(data, "seqPlacement"), false)?;
    let uplink_data_placement = parse_data_placement(field(data, "uplinkDataPlacement"))?;

    let x_padding_key = parse_token(field(data, "xPaddingKey"))?;
    let x_padding_header = parse_token(field(data, "xPaddingHeader"))?;
    let seq_key = parse_token(field(data, "seqKey"))?;
    let uplink_data_key = parse_token(field(data, "uplinkDataKey"))?;

    let session_placement_value = alias(data, &["sessionIDPlacement", "sessionPlacement"])?;
    let session_placement = parse_placement(session_placement_value, true)?;
    let session_key = parse_token(alias(data, &["sessionIDKey", "sessionKey"])?)?;
    let session_table = parse_ascii(
        alias(data, &["sessionIDTable", "sessionTable"])?,
        MAX_XHTTP_ASCII_BYTES,
    )?;
    let session_length_value = alias(data, &["sessionIDLength", "sessionLength"])?;
    let session_length = if session_length_value.is_some_and(is_python_empty_range_value) {
        None
    } else {
        parse_range(session_length_value)?
    };

    if session_placement.unwrap_or(XhttpPlacement::Path) == XhttpPlacement::Path
        && seq_placement.unwrap_or(XhttpPlacement::Path) != XhttpPlacement::Path
    {
        return Err(XhttpOptionsError::SessionSequenceConflict);
    }

    let reuse = parse_reuse(field(data, "xmux"))?;

    let mut normalized_field_count = usize::from(!headers.is_empty())
        + usize::from(x_padding_bytes.is_some())
        + usize::from(uplink_chunk_size.is_some())
        + usize::from(sc_max_each_post_bytes.is_some())
        + usize::from(sc_min_posts_interval_ms.is_some())
        + usize::from(x_padding_obfs_mode)
        + usize::from(no_grpc_header)
        + usize::from(x_padding_placement.is_some())
        + usize::from(x_padding_method.is_some())
        + usize::from(uplink_http_method.is_some())
        + usize::from(seq_placement.is_some())
        + usize::from(uplink_data_placement.is_some())
        + usize::from(x_padding_key.is_some())
        + usize::from(x_padding_header.is_some())
        + usize::from(seq_key.is_some())
        + usize::from(uplink_data_key.is_some())
        + usize::from(session_placement.is_some())
        + usize::from(session_key.is_some())
        + usize::from(session_table.is_some())
        + usize::from(session_length.is_some());
    if reuse.field_count > 0 {
        normalized_field_count += 1;
    }

    let facts = XhttpOptionsFacts {
        normalized_field_count,
        header_count: headers.len(),
        x_padding_bytes,
        uplink_chunk_size,
        sc_max_each_post_bytes,
        sc_min_posts_interval_ms,
        x_padding_obfs_mode,
        no_grpc_header,
        x_padding_placement,
        x_padding_method,
        uplink_http_method,
        seq_placement,
        uplink_data_placement,
        x_padding_key_present: x_padding_key.is_some(),
        x_padding_header_present: x_padding_header.is_some(),
        seq_key_present: seq_key.is_some(),
        uplink_data_key_present: uplink_data_key.is_some(),
        session_placement,
        session_key_present: session_key.is_some(),
        session_table_present: session_table.is_some(),
        session_length,
        reuse_field_count: reuse.field_count,
        max_concurrency: reuse.max_concurrency,
        max_connections: reuse.max_connections,
        c_max_reuse_times: reuse.c_max_reuse_times,
        h_max_request_times: reuse.h_max_request_times,
        h_max_reusable_secs: reuse.h_max_reusable_secs,
        h_keep_alive_period: reuse.h_keep_alive_period,
    };
    Ok(XhttpOptions {
        headers,
        x_padding_key,
        x_padding_header,
        seq_key,
        uplink_data_key,
        session_key,
        session_table,
        facts,
    })
}

fn empty_xhttp_options() -> XhttpOptions {
    XhttpOptions {
        headers: Vec::new(),
        x_padding_key: None,
        x_padding_header: None,
        seq_key: None,
        uplink_data_key: None,
        session_key: None,
        session_table: None,
        facts: XhttpOptionsFacts::default(),
    }
}

fn field<'a>(data: &'a [(String, JsonValue)], name: &str) -> Option<&'a JsonValue> {
    data.iter()
        .find_map(|(key, value)| (key == name).then_some(value))
}

fn is_null_or_one_of_strings(value: &JsonValue, choices: &[&str]) -> bool {
    match value {
        JsonValue::Null => true,
        JsonValue::String(text) => choices.contains(&text.as_str()),
        _ => false,
    }
}

fn python_equal(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Boolean(left), JsonValue::Boolean(right)) => left == right,
        (JsonValue::Boolean(value), JsonValue::Number(number))
        | (JsonValue::Number(number), JsonValue::Boolean(value)) => {
            if *value {
                number_is_one(number)
            } else {
                number_is_zero(number)
            }
        }
        (JsonValue::Number(left), JsonValue::Number(right)) => numbers_equal(left, right),
        (JsonValue::String(left), JsonValue::String(right)) => left == right,
        (JsonValue::Array(left), JsonValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| python_equal(left, right))
        }
        (JsonValue::Object(left), JsonValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    field(right, key).is_some_and(|other| python_equal(value, other))
                })
        }
        _ => false,
    }
}

fn numbers_equal(left: &JsonNumber, right: &JsonNumber) -> bool {
    match (left, right) {
        (JsonNumber::NaN, _) | (_, JsonNumber::NaN) => false,
        (JsonNumber::PositiveInfinity, JsonNumber::PositiveInfinity)
        | (JsonNumber::NegativeInfinity, JsonNumber::NegativeInfinity) => true,
        (JsonNumber::PositiveInfinity | JsonNumber::NegativeInfinity, _)
        | (_, JsonNumber::PositiveInfinity | JsonNumber::NegativeInfinity) => false,
        (JsonNumber::Integer(left), JsonNumber::Integer(right)) => {
            normalized_integer(left) == normalized_integer(right)
        }
        (JsonNumber::Float(left), JsonNumber::Float(right)) => parse_finite_float(left)
            .zip(parse_finite_float(right))
            .is_some_and(|(left, right)| left == right),
        (JsonNumber::Integer(integer), JsonNumber::Float(float))
        | (JsonNumber::Float(float), JsonNumber::Integer(integer)) => {
            let Some(float) = parse_finite_float(float) else {
                return false;
            };
            let Ok(integer) = integer.parse::<f64>() else {
                return false;
            };
            integer.is_finite() && integer == float
        }
    }
}

fn normalized_integer(raw: &str) -> (bool, &str) {
    let negative = raw.starts_with('-');
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        (false, "0")
    } else {
        (negative, digits)
    }
}

fn parse_finite_float(raw: &str) -> Option<f64> {
    raw.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn number_is_zero(number: &JsonNumber) -> bool {
    match number {
        JsonNumber::Integer(raw) => normalized_integer(raw) == (false, "0"),
        JsonNumber::Float(raw) => raw.parse::<f64>().is_ok_and(|value| value == 0.0),
        JsonNumber::NaN | JsonNumber::PositiveInfinity | JsonNumber::NegativeInfinity => false,
    }
}

fn number_is_one(number: &JsonNumber) -> bool {
    match number {
        JsonNumber::Integer(raw) => normalized_integer(raw) == (false, "1"),
        JsonNumber::Float(raw) => raw.parse::<f64>().is_ok_and(|value| value == 1.0),
        JsonNumber::NaN | JsonNumber::PositiveInfinity | JsonNumber::NegativeInfinity => false,
    }
}

fn python_zero(value: &JsonValue) -> bool {
    match value {
        JsonValue::Boolean(value) => !value,
        JsonValue::Number(number) => number_is_zero(number),
        _ => false,
    }
}

fn is_python_empty_range_value(value: &JsonValue) -> bool {
    matches!(value, JsonValue::Null)
        || matches!(value, JsonValue::String(text) if text.is_empty() || text == "0")
        || python_zero(value)
}

fn parse_headers(value: Option<&JsonValue>) -> Result<Vec<(String, String)>, XhttpOptionsError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if matches!(value, JsonValue::Null) {
        return Ok(Vec::new());
    }
    let JsonValue::Object(values) = value else {
        return Err(XhttpOptionsError::HeadersFormat);
    };
    if values.len() > MAX_XHTTP_HEADER_COUNT {
        return Err(XhttpOptionsError::HeadersFormat);
    }
    let mut headers = Vec::with_capacity(values.len());
    for (name, value) in values {
        if name.len() > MAX_XHTTP_HEADER_NAME_BYTES
            || name.eq_ignore_ascii_case("host")
            || !name.bytes().all(is_http_token_byte)
        {
            return Err(XhttpOptionsError::HeaderName);
        }
        let JsonValue::String(value) = value else {
            return Err(XhttpOptionsError::HeaderValue);
        };
        if value.len() > MAX_XHTTP_HEADER_VALUE_BYTES
            || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        {
            return Err(XhttpOptionsError::HeaderValue);
        }
        headers.push((name.clone(), value.clone()));
    }
    Ok(headers)
}

const fn is_http_token_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.'
            | b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z'
            | b'a'..=b'z'
    )
}

fn parse_range(value: Option<&JsonValue>) -> Result<Option<XhttpRange>, XhttpOptionsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(value, JsonValue::Null)
        || matches!(value, JsonValue::String(text) if text.is_empty() || text == "0")
        || python_zero(value)
    {
        return Ok(None);
    }
    if matches!(value, JsonValue::Boolean(_)) {
        return Err(XhttpOptionsError::RangeType);
    }
    let (start, end) = match value {
        JsonValue::Number(JsonNumber::Integer(raw)) => {
            if raw.starts_with('-') {
                return Err(XhttpOptionsError::RangeBounds);
            }
            let value = raw
                .parse::<u64>()
                .map_err(|_| XhttpOptionsError::RangeBounds)?;
            (value, value)
        }
        JsonValue::String(raw) => parse_range_string(raw)?,
        _ => return Err(XhttpOptionsError::RangeType),
    };
    if end < start || end > u64::from(MAX_XHTTP_RANGE_VALUE) {
        return Err(XhttpOptionsError::RangeBounds);
    }
    Ok(Some(XhttpRange {
        start: u32::try_from(start).map_err(|_| XhttpOptionsError::RangeBounds)?,
        end: u32::try_from(end).map_err(|_| XhttpOptionsError::RangeBounds)?,
    }))
}

fn parse_range_string(raw: &str) -> Result<(u64, u64), XhttpOptionsError> {
    let mut parts = raw.split('-');
    let Some(start) = parts.next() else {
        return Err(XhttpOptionsError::RangeType);
    };
    let end = parts.next();
    if start.is_empty()
        || !start.bytes().all(|byte| byte.is_ascii_digit())
        || end.is_some_and(|value| {
            value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())
        })
        || parts.next().is_some()
    {
        return Err(XhttpOptionsError::RangeType);
    }
    let start = start
        .parse::<u64>()
        .map_err(|_| XhttpOptionsError::RangeBounds)?;
    let end = end
        .unwrap_or(raw)
        .parse::<u64>()
        .map_err(|_| XhttpOptionsError::RangeBounds)?;
    Ok((start, end))
}

fn parse_boolean(value: Option<&JsonValue>) -> Result<bool, XhttpOptionsError> {
    match value {
        None | Some(JsonValue::Null) => Ok(false),
        Some(JsonValue::Boolean(value)) => Ok(*value),
        Some(_) => Err(XhttpOptionsError::BooleanType),
    }
}

fn parse_ascii(
    value: Option<&JsonValue>,
    maximum: usize,
) -> Result<Option<String>, XhttpOptionsError> {
    match value {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(text)) if text.is_empty() => Ok(None),
        Some(JsonValue::String(text))
            if text.len() <= maximum && text.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) =>
        {
            Ok(Some(text.clone()))
        }
        Some(_) => Err(XhttpOptionsError::TokenFormat),
    }
}

fn parse_token(value: Option<&JsonValue>) -> Result<Option<String>, XhttpOptionsError> {
    let value = parse_ascii(value, MAX_XHTTP_TOKEN_BYTES)?;
    if value
        .as_ref()
        .is_some_and(|value| !value.bytes().all(is_http_token_byte))
    {
        return Err(XhttpOptionsError::TokenFormat);
    }
    Ok(value)
}

fn parse_padding_placement(
    value: Option<&JsonValue>,
) -> Result<Option<XhttpPaddingPlacement>, XhttpOptionsError> {
    match optional_string(value)? {
        None => Ok(None),
        Some("queryInHeader") => Ok(Some(XhttpPaddingPlacement::QueryInHeader)),
        Some("cookie") => Ok(Some(XhttpPaddingPlacement::Cookie)),
        Some("header") => Ok(Some(XhttpPaddingPlacement::Header)),
        Some("query") => Ok(Some(XhttpPaddingPlacement::Query)),
        Some(_) => Err(XhttpOptionsError::EnumValue),
    }
}

fn parse_padding_method(
    value: Option<&JsonValue>,
) -> Result<Option<XhttpPaddingMethod>, XhttpOptionsError> {
    match optional_string(value)? {
        None => Ok(None),
        Some("repeat-x") => Ok(Some(XhttpPaddingMethod::RepeatX)),
        Some("tokenish") => Ok(Some(XhttpPaddingMethod::Tokenish)),
        Some(_) => Err(XhttpOptionsError::EnumValue),
    }
}

fn parse_http_method(
    value: Option<&JsonValue>,
) -> Result<Option<XhttpHttpMethod>, XhttpOptionsError> {
    let Some(value) = optional_string(value)? else {
        return Ok(None);
    };
    if !value.is_ascii() {
        return Err(XhttpOptionsError::EnumValue);
    }
    match value.to_ascii_uppercase().as_str() {
        "POST" => Ok(Some(XhttpHttpMethod::Post)),
        "PUT" => Ok(Some(XhttpHttpMethod::Put)),
        "PATCH" => Ok(Some(XhttpHttpMethod::Patch)),
        "DELETE" => Ok(Some(XhttpHttpMethod::Delete)),
        _ => Err(XhttpOptionsError::EnumValue),
    }
}

fn parse_placement(
    value: Option<&JsonValue>,
    session: bool,
) -> Result<Option<XhttpPlacement>, XhttpOptionsError> {
    let value = match optional_string(value) {
        Ok(value) => value,
        Err(_) if session => return Err(XhttpOptionsError::SessionPlacement),
        Err(error) => return Err(error),
    };
    let result = match value {
        None => return Ok(None),
        Some("path") => XhttpPlacement::Path,
        Some("query") => XhttpPlacement::Query,
        Some("cookie") => XhttpPlacement::Cookie,
        Some("header") => XhttpPlacement::Header,
        Some(_) if session => return Err(XhttpOptionsError::SessionPlacement),
        Some(_) => return Err(XhttpOptionsError::EnumValue),
    };
    Ok(Some(result))
}

fn parse_data_placement(
    value: Option<&JsonValue>,
) -> Result<Option<XhttpDataPlacement>, XhttpOptionsError> {
    match optional_string(value)? {
        None => Ok(None),
        Some("auto") => Ok(Some(XhttpDataPlacement::Auto)),
        Some("body") => Ok(Some(XhttpDataPlacement::Body)),
        Some("cookie") => Ok(Some(XhttpDataPlacement::Cookie)),
        Some("header") => Ok(Some(XhttpDataPlacement::Header)),
        Some(_) => Err(XhttpOptionsError::EnumValue),
    }
}

fn optional_string(value: Option<&JsonValue>) -> Result<Option<&str>, XhttpOptionsError> {
    match value {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(text)) if text.is_empty() => Ok(None),
        Some(JsonValue::String(text)) => Ok(Some(text)),
        Some(_) => Err(XhttpOptionsError::EnumValue),
    }
}

fn alias<'a>(
    data: &'a [(String, JsonValue)],
    names: &[&str],
) -> Result<Option<&'a JsonValue>, XhttpOptionsError> {
    let mut present = names.iter().filter_map(|name| field(data, name));
    let first = present.next();
    if let Some(first) = first
        && present.any(|value| !python_equal(first, value))
    {
        return Err(XhttpOptionsError::AliasConflict);
    }
    Ok(first)
}

#[derive(Debug, Clone, Copy, Default)]
struct ReuseFacts {
    field_count: usize,
    max_concurrency: Option<XhttpRange>,
    max_connections: Option<XhttpRange>,
    c_max_reuse_times: Option<XhttpRange>,
    h_max_request_times: Option<XhttpRange>,
    h_max_reusable_secs: Option<XhttpRange>,
    h_keep_alive_period: Option<i32>,
}

fn parse_reuse(value: Option<&JsonValue>) -> Result<ReuseFacts, XhttpOptionsError> {
    let Some(value) = value else {
        return Ok(ReuseFacts::default());
    };
    if matches!(value, JsonValue::Null) {
        return Ok(ReuseFacts::default());
    }
    let JsonValue::Object(values) = value else {
        return Err(XhttpOptionsError::XmuxObject);
    };
    const ALLOWED: &[&str] = &[
        "maxConcurrency",
        "maxConnections",
        "cMaxReuseTimes",
        "hMaxRequestTimes",
        "hMaxReusableSecs",
        "hKeepAlivePeriod",
    ];
    if values
        .iter()
        .any(|(name, _)| !ALLOWED.contains(&name.as_str()))
    {
        return Err(XhttpOptionsError::XmuxFields);
    }

    let max_concurrency = parse_range(field(values, "maxConcurrency"))?;
    let max_connections = parse_range(field(values, "maxConnections"))?;
    let c_max_reuse_times = parse_range(field(values, "cMaxReuseTimes"))?;
    let h_max_request_times = parse_range(field(values, "hMaxRequestTimes"))?;
    let h_max_reusable_secs = parse_range(field(values, "hMaxReusableSecs"))?;
    if max_concurrency.is_some() && max_connections.is_some() {
        return Err(XhttpOptionsError::XmuxExclusive);
    }

    let mut h_keep_alive_period = None;
    if let Some(value) = field(values, "hKeepAlivePeriod") {
        let JsonValue::Number(JsonNumber::Integer(raw)) = value else {
            return Err(XhttpOptionsError::XmuxKeepAlive);
        };
        let period = raw
            .parse::<i64>()
            .map_err(|_| XhttpOptionsError::XmuxKeepAlive)?;
        if !(-1..=86_400).contains(&period) {
            return Err(XhttpOptionsError::XmuxKeepAlive);
        }
        if period != 0
            || max_concurrency.is_some()
            || max_connections.is_some()
            || c_max_reuse_times.is_some()
            || h_max_request_times.is_some()
            || h_max_reusable_secs.is_some()
        {
            h_keep_alive_period =
                Some(i32::try_from(period).map_err(|_| XhttpOptionsError::XmuxKeepAlive)?);
        }
    }

    let field_count = usize::from(max_concurrency.is_some())
        + usize::from(max_connections.is_some())
        + usize::from(c_max_reuse_times.is_some())
        + usize::from(h_max_request_times.is_some())
        + usize::from(h_max_reusable_secs.is_some())
        + usize::from(h_keep_alive_period.is_some());
    Ok(ReuseFacts {
        field_count,
        max_concurrency,
        max_connections,
        c_max_reuse_times,
        h_max_request_times,
        h_max_reusable_secs,
        h_keep_alive_period,
    })
}

/// Normalized upload/download XHTTP mode vocabulary used by the bounded
/// `downloadSettings` adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpDownloadMode {
    Auto,
    StreamUp,
    PacketUp,
    StreamOne,
}

impl XhttpDownloadMode {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::StreamUp => "stream-up",
            Self::PacketUp => "packet-up",
            Self::StreamOne => "stream-one",
        }
    }
}

/// Normalized upload security vocabulary needed to validate inherited Reality
/// download settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpDownloadSecurity {
    None,
    Tls,
    Reality,
}

impl XhttpDownloadSecurity {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Tls => "tls",
            Self::Reality => "reality",
        }
    }
}

/// Credential-safe projection of normalized XHTTP `downloadSettings`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XhttpDownloadFacts {
    pub normalized_field_count: usize,
    pub server_kind: Option<HostKind>,
    pub port: Option<u16>,
    pub tls: Option<bool>,
    pub servername_kind: Option<HostKind>,
    pub alpn_count: usize,
    pub alpn_h2: bool,
    pub alpn_h3: bool,
    pub alpn_http_1_1: bool,
    pub fingerprint_present: bool,
    pub skip_cert_verify: Option<bool>,
    pub reality_present: bool,
    pub reality_short_id_present: bool,
    pub reality_pq_enabled: bool,
    pub reality_spider_compatibility: bool,
    pub reality_pq_compatibility: bool,
    pub path_present: bool,
    pub host_kind: Option<HostKind>,
    pub header_count: usize,
    pub reuse_field_count: usize,
    pub max_concurrency: Option<XhttpRange>,
    pub max_connections: Option<XhttpRange>,
    pub c_max_reuse_times: Option<XhttpRange>,
    pub h_max_request_times: Option<XhttpRange>,
    pub h_max_reusable_secs: Option<XhttpRange>,
    pub h_keep_alive_period: Option<i32>,
}

struct XhttpDownloadReality {
    public_key: String,
    short_id: Option<String>,
    pq_enabled: bool,
}

/// Private normalized XHTTP download-side configuration. Reusable endpoint,
/// header, fingerprint and Reality credential material remains in memory for a
/// later rendering slice and never appears in `Debug` or parity facts.
pub struct XhttpDownloadSettings {
    server: Option<String>,
    port: Option<u16>,
    tls: Option<bool>,
    servername: Option<String>,
    alpn: Vec<String>,
    fingerprint: Option<String>,
    skip_cert_verify: Option<bool>,
    reality: Option<XhttpDownloadReality>,
    path: Option<String>,
    host: Option<String>,
    headers: Vec<(String, String)>,
    reuse: ReuseFacts,
    facts: XhttpDownloadFacts,
}

impl XhttpDownloadSettings {
    #[must_use]
    pub const fn facts(&self) -> XhttpDownloadFacts {
        self.facts
    }

    fn private_value_count(&self) -> usize {
        usize::from(self.server.is_some())
            + usize::from(self.servername.is_some())
            + self.alpn.len()
            + usize::from(self.fingerprint.is_some())
            + self.reality.as_ref().map_or(0, |reality| {
                usize::from(!reality.public_key.is_empty())
                    + usize::from(reality.short_id.is_some())
                    + usize::from(reality.pq_enabled)
            })
            + usize::from(self.path.is_some())
            + usize::from(self.host.is_some())
            + self.headers.len()
            + usize::from(self.port.is_some())
            + usize::from(self.tls.is_some())
            + usize::from(self.skip_cert_verify.is_some())
            + self.reuse.field_count
    }

    fn normalized_entries(&self) -> Vec<(String, XhttpValue)> {
        let mut entries = Vec::new();
        if let Some(value) = &self.server {
            entries.push(("server".to_owned(), XhttpValue::String(value.clone())));
        }
        if let Some(value) = self.port {
            entries.push(("port".to_owned(), XhttpValue::Integer(i64::from(value))));
        }
        if let Some(value) = self.tls {
            entries.push(("tls".to_owned(), XhttpValue::Boolean(value)));
        }
        if let Some(value) = &self.servername {
            entries.push(("servername".to_owned(), XhttpValue::String(value.clone())));
        }
        if !self.alpn.is_empty() {
            entries.push((
                "alpn".to_owned(),
                XhttpValue::Array(self.alpn.iter().cloned().map(XhttpValue::String).collect()),
            ));
        }
        if let Some(value) = &self.fingerprint {
            entries.push((
                "client-fingerprint".to_owned(),
                XhttpValue::String(value.clone()),
            ));
        }
        if let Some(value) = self.skip_cert_verify {
            entries.push(("skip-cert-verify".to_owned(), XhttpValue::Boolean(value)));
        }
        if let Some(reality) = &self.reality {
            let mut values = vec![(
                "public-key".to_owned(),
                XhttpValue::String(reality.public_key.clone()),
            )];
            if let Some(value) = &reality.short_id {
                values.push(("short-id".to_owned(), XhttpValue::String(value.clone())));
            }
            if reality.pq_enabled {
                values.push((
                    "support-x25519mlkem768".to_owned(),
                    XhttpValue::Boolean(true),
                ));
            }
            entries.push(("reality-opts".to_owned(), XhttpValue::Object(values)));
        }
        if let Some(value) = &self.path {
            entries.push(("path".to_owned(), XhttpValue::String(value.clone())));
        }
        if let Some(value) = &self.host {
            entries.push(("host".to_owned(), XhttpValue::String(value.clone())));
        }
        if !self.headers.is_empty() {
            entries.push(("headers".to_owned(), headers_value(&self.headers)));
        }
        if let Some(value) = reuse_value_from_reuse(self.reuse) {
            entries.push(("reuse-settings".to_owned(), value));
        }
        debug_assert_eq!(entries.len(), self.facts.normalized_field_count);
        entries
    }
}

impl fmt::Debug for XhttpDownloadSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XhttpDownloadSettings")
            .field("facts", &self.facts)
            .field("private_value_count", &self.private_value_count())
            .finish_non_exhaustive()
    }
}

pub(crate) enum XhttpValue {
    Boolean(bool),
    Integer(i64),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}

impl fmt::Debug for XhttpValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(_) => formatter.write_str("XhttpValue::Boolean(..)"),
            Self::Integer(_) => formatter.write_str("XhttpValue::Integer(..)"),
            Self::String(_) => formatter.write_str("XhttpValue::String(..)"),
            Self::Array(values) => formatter
                .debug_struct("XhttpValue::Array")
                .field("len", &values.len())
                .finish(),
            Self::Object(values) => formatter
                .debug_struct("XhttpValue::Object")
                .field("len", &values.len())
                .finish(),
        }
    }
}

pub(crate) struct XhttpConfiguration {
    options: XhttpOptions,
    download: Option<XhttpDownloadSettings>,
}

impl XhttpConfiguration {
    #[must_use]
    pub(crate) fn normalized_entries(&self) -> Vec<(String, XhttpValue)> {
        let mut entries = Vec::new();
        if let Some(download) = &self.download {
            entries.push((
                "download-settings".to_owned(),
                XhttpValue::Object(download.normalized_entries()),
            ));
        }
        entries.extend(self.options.normalized_entries());
        entries
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.download.is_none() && self.options.facts.normalized_field_count == 0
    }

    #[must_use]
    pub(crate) fn download_reality_pq(&self) -> bool {
        self.download
            .as_ref()
            .is_some_and(|download| download.facts.reality_pq_enabled)
    }

    #[must_use]
    pub(crate) fn download_reality_spider_compatibility(&self) -> bool {
        self.download
            .as_ref()
            .is_some_and(|download| download.facts.reality_spider_compatibility)
    }

    #[must_use]
    pub(crate) fn download_reality_pq_compatibility(&self) -> bool {
        self.download
            .as_ref()
            .is_some_and(|download| download.facts.reality_pq_compatibility)
    }
}

impl fmt::Debug for XhttpConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XhttpConfiguration")
            .field("normalized_field_count", &self.normalized_entries().len())
            .field("download_present", &self.download.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpConfigurationError {
    Options(XhttpOptionsError),
    Download(XhttpDownloadError),
}

impl XhttpConfigurationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Options(error) => error.code(),
            Self::Download(error) => error.code(),
        }
    }
}

impl fmt::Display for XhttpConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Options(error) => error.fmt(formatter),
            Self::Download(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for XhttpConfigurationError {}

fn range_text(value: XhttpRange) -> String {
    if value.is_single() {
        value.start.to_string()
    } else {
        format!("{}-{}", value.start, value.end)
    }
}

fn headers_value(headers: &[(String, String)]) -> XhttpValue {
    XhttpValue::Object(
        headers
            .iter()
            .map(|(name, value)| (name.clone(), XhttpValue::String(value.clone())))
            .collect(),
    )
}

fn reuse_value(
    max_concurrency: Option<XhttpRange>,
    max_connections: Option<XhttpRange>,
    c_max_reuse_times: Option<XhttpRange>,
    h_max_request_times: Option<XhttpRange>,
    h_max_reusable_secs: Option<XhttpRange>,
    h_keep_alive_period: Option<i32>,
) -> Option<XhttpValue> {
    let mut values = Vec::new();
    for (name, value) in [
        ("max-concurrency", max_concurrency),
        ("max-connections", max_connections),
        ("c-max-reuse-times", c_max_reuse_times),
        ("h-max-request-times", h_max_request_times),
        ("h-max-reusable-secs", h_max_reusable_secs),
    ] {
        if let Some(value) = value {
            values.push((name.to_owned(), XhttpValue::String(range_text(value))));
        }
    }
    if let Some(value) = h_keep_alive_period {
        values.push((
            "h-keep-alive-period".to_owned(),
            XhttpValue::Integer(i64::from(value)),
        ));
    }
    (!values.is_empty()).then_some(XhttpValue::Object(values))
}

fn reuse_value_from_options(facts: XhttpOptionsFacts) -> Option<XhttpValue> {
    reuse_value(
        facts.max_concurrency,
        facts.max_connections,
        facts.c_max_reuse_times,
        facts.h_max_request_times,
        facts.h_max_reusable_secs,
        facts.h_keep_alive_period,
    )
}

fn reuse_value_from_reuse(facts: ReuseFacts) -> Option<XhttpValue> {
    reuse_value(
        facts.max_concurrency,
        facts.max_connections,
        facts.c_max_reuse_times,
        facts.h_max_request_times,
        facts.h_max_reusable_secs,
        facts.h_keep_alive_period,
    )
}

/// Fixed, credential-safe XHTTP `downloadSettings` errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpDownloadError {
    Decode(XhttpExtraError),
    Shared(XhttpOptionsError),
    StreamOne,
    UnsupportedFields,
    Sockopt,
    EndpointFormat,
    Port,
    Network,
    Security,
    TlsObject,
    TlsFields,
    TlsSecurityConflict,
    TlsShow,
    AlpnFormat,
    AlpnValue,
    RealitySecurityConflict,
    RealityObject,
    RealityFields,
    RealityShow,
    RealityMldsa,
    RealityPublicKeyRequired,
    RealityPublicKey,
    RealityShortId,
    RealitySettingsRequired,
    TransportObject,
    TransportFields,
    PathFormat,
    Mode,
    ModeMismatch,
    TransportExtraObject,
    TransportExtraFields,
    RecursiveDownload,
    TransportCompatibilityFormat,
    TransportMode,
    IndependentOverride,
    HeadersConflict,
}

impl XhttpDownloadError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Decode(error) => error.code(),
            Self::Shared(error) => error.code(),
            Self::StreamOne => "stream_one",
            Self::UnsupportedFields => "unsupported_fields",
            Self::Sockopt => "sockopt",
            Self::EndpointFormat => "endpoint_format",
            Self::Port => "port",
            Self::Network => "network",
            Self::Security => "security",
            Self::TlsObject => "tls_object",
            Self::TlsFields => "tls_fields",
            Self::TlsSecurityConflict => "tls_security_conflict",
            Self::TlsShow => "tls_show",
            Self::AlpnFormat => "alpn_format",
            Self::AlpnValue => "alpn_value",
            Self::RealitySecurityConflict => "reality_security_conflict",
            Self::RealityObject => "reality_object",
            Self::RealityFields => "reality_fields",
            Self::RealityShow => "reality_show",
            Self::RealityMldsa => "reality_mldsa",
            Self::RealityPublicKeyRequired => "reality_public_key_required",
            Self::RealityPublicKey => "reality_public_key",
            Self::RealityShortId => "reality_short_id",
            Self::RealitySettingsRequired => "reality_settings_required",
            Self::TransportObject => "transport_object",
            Self::TransportFields => "transport_fields",
            Self::PathFormat => "path_format",
            Self::Mode => "mode",
            Self::ModeMismatch => "mode_mismatch",
            Self::TransportExtraObject => "transport_extra_object",
            Self::TransportExtraFields => "transport_extra_fields",
            Self::RecursiveDownload => "recursive_download",
            Self::TransportCompatibilityFormat => "transport_compatibility_format",
            Self::TransportMode => "transport_mode",
            Self::IndependentOverride => "independent_override",
            Self::HeadersConflict => "headers_conflict",
        }
    }
}

impl fmt::Display for XhttpDownloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => return error.fmt(formatter),
            Self::Shared(error) => return error.fmt(formatter),
            _ => {}
        }
        formatter.write_str(match self {
            Self::Decode(_) | Self::Shared(_) => unreachable!(),
            Self::StreamOne => "VLESS XHTTP stream-one cannot use downloadSettings",
            Self::UnsupportedFields => "VLESS XHTTP downloadSettings contains unsupported fields",
            Self::Sockopt => "VLESS XHTTP download sockopt is not imported",
            Self::EndpointFormat => "VLESS XHTTP download endpoint has an invalid format",
            Self::Port => "VLESS XHTTP download port is invalid",
            Self::Network => "VLESS XHTTP download network must be xhttp",
            Self::Security => "VLESS XHTTP download security is unsupported",
            Self::TlsObject => "VLESS XHTTP download tlsSettings must be an object",
            Self::TlsFields => "VLESS XHTTP download tlsSettings contains unsupported fields",
            Self::TlsSecurityConflict => {
                "VLESS XHTTP download TLS settings conflict with security none"
            }
            Self::TlsShow => "VLESS XHTTP download tlsSettings show is unsupported",
            Self::AlpnFormat => "VLESS XHTTP download ALPN has an invalid format",
            Self::AlpnValue => "VLESS XHTTP download ALPN has an unsupported value",
            Self::RealitySecurityConflict => {
                "VLESS XHTTP download Reality conflicts with its security"
            }
            Self::RealityObject => "VLESS XHTTP download realitySettings must be an object",
            Self::RealityFields => {
                "VLESS XHTTP download realitySettings contains unsupported fields"
            }
            Self::RealityShow => "VLESS XHTTP download Reality show is unsupported",
            Self::RealityMldsa => "VLESS XHTTP download Reality ML-DSA verification is unsupported",
            Self::RealityPublicKeyRequired => "VLESS XHTTP download Reality requires a public key",
            Self::RealityPublicKey => {
                "VLESS XHTTP download Reality public key has an invalid format"
            }
            Self::RealityShortId => "VLESS XHTTP download Reality short ID has an invalid format",
            Self::RealitySettingsRequired => {
                "VLESS XHTTP download Reality requires realitySettings"
            }
            Self::TransportObject => "VLESS XHTTP download xhttpSettings must be an object",
            Self::TransportFields => {
                "VLESS XHTTP download xhttpSettings contains unsupported fields"
            }
            Self::PathFormat => "VLESS XHTTP download path has an invalid format",
            Self::Mode => "VLESS XHTTP download mode is unsupported",
            Self::ModeMismatch => "VLESS XHTTP upload and download modes must match",
            Self::TransportExtraObject => "VLESS XHTTP download transport extra must be an object",
            Self::TransportExtraFields => {
                "VLESS XHTTP download transport extra contains unsupported fields"
            }
            Self::RecursiveDownload => "VLESS XHTTP recursive download settings are not supported",
            Self::TransportCompatibilityFormat => {
                "VLESS XHTTP download transport compatibility field is invalid"
            }
            Self::TransportMode => "VLESS XHTTP download transport mode is invalid",
            Self::IndependentOverride => {
                "VLESS XHTTP download field cannot be overridden independently"
            }
            Self::HeadersConflict => "VLESS XHTTP download headers conflict",
        })
    }
}

impl std::error::Error for XhttpDownloadError {}

impl From<XhttpExtraError> for XhttpDownloadError {
    fn from(error: XhttpExtraError) -> Self {
        Self::Decode(error)
    }
}

impl From<XhttpOptionsError> for XhttpDownloadError {
    fn from(error: XhttpOptionsError) -> Self {
        Self::Shared(error)
    }
}

/// Decode and normalize one bounded XHTTP `downloadSettings` object while the
/// current Python backend remains the production owner and oracle.
pub fn parse_xhttp_download_settings(
    input: &str,
    main_mode: XhttpDownloadMode,
    main_security: XhttpDownloadSecurity,
) -> Result<XhttpDownloadSettings, XhttpDownloadError> {
    let document = decode_xhttp_extra(input)?;
    normalize_xhttp_download(document, main_mode, main_security)
}

/// Byte-oriented entry point preserving the R2h1 UTF-8 and size classes.
pub fn parse_xhttp_download_settings_bytes(
    input: &[u8],
    main_mode: XhttpDownloadMode,
    main_security: XhttpDownloadSecurity,
) -> Result<XhttpDownloadSettings, XhttpDownloadError> {
    let document = decode_xhttp_extra_bytes(input)?;
    normalize_xhttp_download(document, main_mode, main_security)
}

/// Compose the accepted top-level and download-side XHTTP semantic slices from
/// one bounded JSON decode. This remains crate-private until the canonical
/// VLESS adapter owns the complete private model and rendering boundary.
pub(crate) fn parse_xhttp_configuration(
    input: &str,
    main_mode: XhttpDownloadMode,
    main_security: XhttpDownloadSecurity,
) -> Result<XhttpConfiguration, XhttpConfigurationError> {
    let document = decode_xhttp_extra(input)
        .map_err(|error| XhttpConfigurationError::Options(XhttpOptionsError::Decode(error)))?;
    let data = &document.root;
    validate_xhttp_options_envelope(data, XhttpDownloadPolicy::Allow)
        .map_err(XhttpConfigurationError::Options)?;

    // Python validates downloadSettings before the remaining top-level option
    // fields. Preserve that ordering for inputs containing multiple faults.
    let download = match field(data, "downloadSettings") {
        None | Some(JsonValue::Null) => None,
        Some(value) => Some(
            normalize_xhttp_download_value(value, main_mode, main_security)
                .map_err(XhttpConfigurationError::Download)?,
        ),
    };
    let options = normalize_xhttp_option_fields(data).map_err(XhttpConfigurationError::Options)?;
    Ok(XhttpConfiguration { options, download })
}

fn normalize_xhttp_download(
    document: XhttpExtraDocument,
    main_mode: XhttpDownloadMode,
    main_security: XhttpDownloadSecurity,
) -> Result<XhttpDownloadSettings, XhttpDownloadError> {
    normalize_xhttp_download_data(&document.root, main_mode, main_security)
}

fn normalize_xhttp_download_value(
    value: &JsonValue,
    main_mode: XhttpDownloadMode,
    main_security: XhttpDownloadSecurity,
) -> Result<XhttpDownloadSettings, XhttpDownloadError> {
    let JsonValue::Object(data) = value else {
        return Err(XhttpDownloadError::Decode(XhttpExtraError::NonObjectRoot));
    };
    normalize_xhttp_download_data(data, main_mode, main_security)
}

fn normalize_xhttp_download_data(
    data: &[(String, JsonValue)],
    main_mode: XhttpDownloadMode,
    main_security: XhttpDownloadSecurity,
) -> Result<XhttpDownloadSettings, XhttpDownloadError> {
    if main_mode == XhttpDownloadMode::StreamOne {
        return Err(XhttpDownloadError::StreamOne);
    }
    const ALLOWED: &[&str] = &[
        "address",
        "port",
        "network",
        "security",
        "tlsSettings",
        "realitySettings",
        "xhttpSettings",
        "sockopt",
    ];
    if data
        .iter()
        .any(|(name, _)| !ALLOWED.contains(&name.as_str()))
    {
        return Err(XhttpDownloadError::UnsupportedFields);
    }
    if field(data, "sockopt").is_some_and(|value| {
        !matches!(value, JsonValue::Null)
            && !matches!(value, JsonValue::String(text) if text.is_empty())
            && !matches!(value, JsonValue::Object(values) if values.is_empty())
    }) {
        return Err(XhttpDownloadError::Sockopt);
    }

    let (server, server_kind) = parse_download_endpoint(field(data, "address"))?;
    let port = parse_download_port(field(data, "port"))?;
    if let Some(value) = field(data, "network")
        && !is_null_or_one_of_strings(value, &["", "xhttp"])
    {
        return Err(XhttpDownloadError::Network);
    }
    let security = parse_download_security(field(data, "security"))?;
    let mut tls = security.map(|value| value != XhttpDownloadSecurity::None);
    let mut servername = None;
    let mut servername_kind = None;
    let mut alpn = Vec::new();
    let mut fingerprint = None;
    let mut skip_cert_verify = None;

    if let Some(value) = field(data, "tlsSettings")
        && !matches!(value, JsonValue::Null)
    {
        let JsonValue::Object(values) = value else {
            return Err(XhttpDownloadError::TlsObject);
        };
        const ALLOWED_TLS: &[&str] =
            &["serverName", "alpn", "fingerprint", "allowInsecure", "show"];
        if values
            .iter()
            .any(|(name, _)| !ALLOWED_TLS.contains(&name.as_str()))
        {
            return Err(XhttpDownloadError::TlsFields);
        }
        if security == Some(XhttpDownloadSecurity::None) && !values.is_empty() {
            return Err(XhttpDownloadError::TlsSecurityConflict);
        }
        if field(values, "show").is_some_and(|value| !python_none_or_false(value)) {
            return Err(XhttpDownloadError::TlsShow);
        }
        (servername, servername_kind) = parse_download_endpoint(field(values, "serverName"))?;
        alpn = parse_download_alpn(field(values, "alpn"))?;
        fingerprint = parse_token(field(values, "fingerprint"))?;
        if let Some(value) = field(values, "allowInsecure") {
            skip_cert_verify = Some(parse_boolean(Some(value))?);
        }
    }

    let mut reality = None;
    let mut reality_spider_compatibility = false;
    let mut reality_pq_compatibility = false;
    if let Some(value) = field(data, "realitySettings")
        && !matches!(value, JsonValue::Null)
    {
        if security.is_some_and(|value| value != XhttpDownloadSecurity::Reality) {
            return Err(XhttpDownloadError::RealitySecurityConflict);
        }
        let JsonValue::Object(values) = value else {
            return Err(XhttpDownloadError::RealityObject);
        };
        const ALLOWED_REALITY: &[&str] = &[
            "publicKey",
            "password",
            "shortId",
            "spiderX",
            "fingerprint",
            "serverName",
            "mldsa65Verify",
            "show",
            "supportX25519MLKEM768",
            "support-x25519mlkem768",
        ];
        if values
            .iter()
            .any(|(name, _)| !ALLOWED_REALITY.contains(&name.as_str()))
        {
            return Err(XhttpDownloadError::RealityFields);
        }
        if field(values, "show").is_some_and(|value| !python_none_or_false(value)) {
            return Err(XhttpDownloadError::RealityShow);
        }
        if field(values, "mldsa65Verify").is_some_and(|value| !is_null_or_empty_string(value)) {
            return Err(XhttpDownloadError::RealityMldsa);
        }
        let public_key_value = alias(values, &["publicKey", "password"])?;
        let Some(JsonValue::String(public_key)) = public_key_value else {
            return Err(XhttpDownloadError::RealityPublicKeyRequired);
        };
        if public_key.is_empty() {
            return Err(XhttpDownloadError::RealityPublicKeyRequired);
        }
        if !valid_reality_public_key(public_key) {
            return Err(XhttpDownloadError::RealityPublicKey);
        }
        let short_id = match field(values, "shortId") {
            None => None,
            Some(JsonValue::String(value)) if value.is_empty() => None,
            Some(JsonValue::String(value)) if valid_reality_short_id(value) => Some(value.clone()),
            Some(_) => return Err(XhttpDownloadError::RealityShortId),
        };
        let pq_value = alias(values, &["supportX25519MLKEM768", "support-x25519mlkem768"])?;
        let pq_enabled = match pq_value {
            None => false,
            Some(value) => parse_boolean(Some(value))?,
        };
        reality_pq_compatibility = pq_enabled;
        reality = Some(XhttpDownloadReality {
            public_key: public_key.clone(),
            short_id,
            pq_enabled,
        });
        tls = Some(true);
        let (reality_servername, reality_servername_kind) =
            parse_download_endpoint(field(values, "serverName"))?;
        if reality_servername.is_some() {
            servername = reality_servername;
            servername_kind = reality_servername_kind;
        }
        if let Some(value) = parse_token(field(values, "fingerprint"))? {
            fingerprint = Some(value);
        }
        reality_spider_compatibility =
            field(values, "spiderX").is_some_and(|value| !is_null_or_empty_string(value));
    } else if security == Some(XhttpDownloadSecurity::Reality)
        && main_security != XhttpDownloadSecurity::Reality
    {
        return Err(XhttpDownloadError::RealitySettingsRequired);
    }

    let mut path = None;
    let mut host = None;
    let mut host_kind = None;
    let mut headers = Vec::new();
    let mut reuse = ReuseFacts::default();
    if let Some(value) = field(data, "xhttpSettings")
        && !matches!(value, JsonValue::Null)
    {
        let JsonValue::Object(values) = value else {
            return Err(XhttpDownloadError::TransportObject);
        };
        const ALLOWED_TRANSPORT: &[&str] = &["path", "host", "mode", "headers", "extra"];
        if values
            .iter()
            .any(|(name, _)| !ALLOWED_TRANSPORT.contains(&name.as_str()))
        {
            return Err(XhttpDownloadError::TransportFields);
        }
        path = parse_download_path(field(values, "path"))?;
        (host, host_kind) = parse_download_endpoint(field(values, "host"))?;
        if let Some(value) = field(values, "mode")
            && !is_null_or_empty_string(value)
        {
            let JsonValue::String(value) = value else {
                return Err(XhttpDownloadError::Mode);
            };
            let mode = parse_download_mode(value).ok_or(XhttpDownloadError::Mode)?;
            if mode != main_mode {
                return Err(XhttpDownloadError::ModeMismatch);
            }
        }
        let direct_headers = parse_headers(field(values, "headers"))?;
        let nested = parse_download_transport_extra(field(values, "extra"))?;
        if !direct_headers.is_empty()
            && !nested.headers.is_empty()
            && !headers_equal(&direct_headers, &nested.headers)
        {
            return Err(XhttpDownloadError::HeadersConflict);
        }
        headers = if nested.headers.is_empty() {
            direct_headers
        } else {
            nested.headers
        };
        reuse = nested.reuse;
    }

    let normalized_field_count = usize::from(server.is_some())
        + usize::from(port.is_some())
        + usize::from(tls.is_some())
        + usize::from(servername.is_some())
        + usize::from(!alpn.is_empty())
        + usize::from(fingerprint.is_some())
        + usize::from(skip_cert_verify.is_some())
        + usize::from(reality.is_some())
        + usize::from(path.is_some())
        + usize::from(host.is_some())
        + usize::from(!headers.is_empty())
        + usize::from(reuse.field_count > 0);
    let facts = XhttpDownloadFacts {
        normalized_field_count,
        server_kind,
        port,
        tls,
        servername_kind,
        alpn_count: alpn.len(),
        alpn_h2: alpn.iter().any(|value| value == "h2"),
        alpn_h3: alpn.iter().any(|value| value == "h3"),
        alpn_http_1_1: alpn.iter().any(|value| value == "http/1.1"),
        fingerprint_present: fingerprint.is_some(),
        skip_cert_verify,
        reality_present: reality.is_some(),
        reality_short_id_present: reality
            .as_ref()
            .is_some_and(|value| value.short_id.is_some()),
        reality_pq_enabled: reality.as_ref().is_some_and(|value| value.pq_enabled),
        reality_spider_compatibility,
        reality_pq_compatibility,
        path_present: path.is_some(),
        host_kind,
        header_count: headers.len(),
        reuse_field_count: reuse.field_count,
        max_concurrency: reuse.max_concurrency,
        max_connections: reuse.max_connections,
        c_max_reuse_times: reuse.c_max_reuse_times,
        h_max_request_times: reuse.h_max_request_times,
        h_max_reusable_secs: reuse.h_max_reusable_secs,
        h_keep_alive_period: reuse.h_keep_alive_period,
    };
    Ok(XhttpDownloadSettings {
        server,
        port,
        tls,
        servername,
        alpn,
        fingerprint,
        skip_cert_verify,
        reality,
        path,
        host,
        headers,
        reuse,
        facts,
    })
}

fn parse_download_port(value: Option<&JsonValue>) -> Result<Option<u16>, XhttpDownloadError> {
    match value {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Number(JsonNumber::Integer(raw))) => {
            let value = raw.parse::<u32>().map_err(|_| XhttpDownloadError::Port)?;
            if value == 0 || value > u32::from(u16::MAX) {
                return Err(XhttpDownloadError::Port);
            }
            Ok(Some(value as u16))
        }
        Some(_) => Err(XhttpDownloadError::Port),
    }
}

fn parse_download_security(
    value: Option<&JsonValue>,
) -> Result<Option<XhttpDownloadSecurity>, XhttpDownloadError> {
    match value {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(value)) if value.is_empty() => Ok(None),
        Some(JsonValue::String(value)) => match value.as_str() {
            "none" => Ok(Some(XhttpDownloadSecurity::None)),
            "tls" => Ok(Some(XhttpDownloadSecurity::Tls)),
            "reality" => Ok(Some(XhttpDownloadSecurity::Reality)),
            _ => Err(XhttpDownloadError::Security),
        },
        Some(_) => Err(XhttpDownloadError::Security),
    }
}

fn parse_download_mode(value: &str) -> Option<XhttpDownloadMode> {
    match value {
        "auto" => Some(XhttpDownloadMode::Auto),
        "stream-up" => Some(XhttpDownloadMode::StreamUp),
        "packet-up" => Some(XhttpDownloadMode::PacketUp),
        "stream-one" => Some(XhttpDownloadMode::StreamOne),
        _ => None,
    }
}

fn parse_download_alpn(value: Option<&JsonValue>) -> Result<Vec<String>, XhttpDownloadError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if matches!(value, JsonValue::Null) {
        return Ok(Vec::new());
    }
    let JsonValue::Array(values) = value else {
        return Err(XhttpDownloadError::AlpnFormat);
    };
    if values.len() > 8 {
        return Err(XhttpDownloadError::AlpnFormat);
    }
    let mut result = Vec::new();
    for value in values {
        let JsonValue::String(value) = value else {
            return Err(XhttpDownloadError::AlpnValue);
        };
        if !matches!(value.as_str(), "h2" | "h3" | "http/1.1") {
            return Err(XhttpDownloadError::AlpnValue);
        }
        if !result.contains(value) {
            result.push(value.clone());
        }
    }
    Ok(result)
}

fn parse_download_path(value: Option<&JsonValue>) -> Result<Option<String>, XhttpDownloadError> {
    match value {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(value)) if value.is_empty() => Ok(None),
        Some(JsonValue::String(value))
            if value.len() <= MAX_XHTTP_EXTRA_STRING_BYTES
                && !value
                    .chars()
                    .any(|character| character <= '\u{1f}' || character == '\u{7f}') =>
        {
            if value.starts_with('/') {
                Ok(Some(value.clone()))
            } else {
                Ok(Some(format!("/{value}")))
            }
        }
        Some(_) => Err(XhttpDownloadError::PathFormat),
    }
}

fn parse_download_endpoint(
    value: Option<&JsonValue>,
) -> Result<(Option<String>, Option<HostKind>), XhttpDownloadError> {
    let value = match value {
        None | Some(JsonValue::Null) => return Ok((None, None)),
        Some(JsonValue::String(value)) if value.is_empty() => return Ok((None, None)),
        Some(JsonValue::String(value)) => value,
        Some(_) => return Err(XhttpDownloadError::EndpointFormat),
    };
    if value.len() > 253
        || value
            .chars()
            .any(|character| character.is_whitespace() || character <= '\u{1f}')
    {
        return Err(XhttpDownloadError::EndpointFormat);
    }
    if Ipv4Addr::from_str(value).is_ok() {
        return Ok((Some(value.clone()), Some(HostKind::Ipv4)));
    }
    if Ipv6Addr::from_str(value).is_ok() || scoped_ipv6(value) {
        return Ok((Some(value.clone()), Some(HostKind::Ipv6)));
    }
    if value.contains(['/', '@', '[', ']', ':', '#']) || !valid_download_dns_name(value) {
        return Err(XhttpDownloadError::EndpointFormat);
    }
    Ok((Some(value.clone()), Some(HostKind::Dns)))
}

fn scoped_ipv6(value: &str) -> bool {
    value.split_once('%').is_some_and(|(address, scope)| {
        !scope.is_empty()
            && !scope
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
            && Ipv6Addr::from_str(address).is_ok()
    })
}

fn valid_download_dns_name(value: &str) -> bool {
    let value = value.trim_end_matches('.');
    if value.is_empty() {
        return false;
    }
    let labels = value
        .split(['.', '\u{3002}', '\u{ff0e}', '\u{ff61}'])
        .collect::<Vec<_>>();
    if labels.iter().any(|label| label.is_empty()) {
        return false;
    }
    let mut ascii_length = labels.len().saturating_sub(1);
    for label in labels {
        let Some(length) = idna_label_length(label) else {
            return false;
        };
        if length == 0 || length > 63 {
            return false;
        }
        ascii_length = match ascii_length.checked_add(length) {
            Some(length) => length,
            None => return false,
        };
    }
    ascii_length <= 253
}

fn idna_label_length(label: &str) -> Option<usize> {
    if label.is_ascii() {
        let bytes = label.as_bytes();
        if bytes.is_empty()
            || !bytes[0].is_ascii_alphanumeric()
            || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
            || bytes
                .iter()
                .any(|byte| !byte.is_ascii_alphanumeric() && *byte != b'-')
        {
            return None;
        }
        return Some(bytes.len());
    }
    if label.starts_with('-')
        || label.ends_with('-')
        || label.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || (character.is_ascii() && !character.is_ascii_alphanumeric() && character != '-')
        })
    {
        return None;
    }
    let normalized = label
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<Vec<_>>();
    punycode_length(&normalized).and_then(|length| length.checked_add(4))
}

fn punycode_length(input: &[char]) -> Option<usize> {
    const BASE: u64 = 36;
    const TMIN: u64 = 1;
    const TMAX: u64 = 26;
    const INITIAL_BIAS: u64 = 72;
    const INITIAL_N: u64 = 128;

    let mut output = input
        .iter()
        .filter(|character| character.is_ascii())
        .count();
    let basic = output;
    if basic > 0 && basic < input.len() {
        output = output.checked_add(1)?;
    }
    let mut handled = basic;
    let mut n = INITIAL_N;
    let mut delta = 0_u64;
    let mut bias = INITIAL_BIAS;
    while handled < input.len() {
        let m = input
            .iter()
            .map(|character| u64::from(u32::from(*character)))
            .filter(|value| *value >= n)
            .min()?;
        delta = delta.checked_add((m - n).checked_mul(u64::try_from(handled + 1).ok()?)?)?;
        n = m;
        for character in input {
            let value = u64::from(u32::from(*character));
            if value < n {
                delta = delta.checked_add(1)?;
            } else if value == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let threshold = if k <= bias {
                        TMIN
                    } else if k >= bias + TMAX {
                        TMAX
                    } else {
                        k - bias
                    };
                    if q < threshold {
                        break;
                    }
                    output = output.checked_add(1)?;
                    q = (q - threshold) / (BASE - threshold);
                    k = k.checked_add(BASE)?;
                }
                output = output.checked_add(1)?;
                bias = adapt_punycode_bias(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n = n.checked_add(1)?;
    }
    Some(output)
}

fn adapt_punycode_bias(delta: u64, points: usize, first: bool) -> u64 {
    const BASE: u64 = 36;
    const TMIN: u64 = 1;
    const TMAX: u64 = 26;
    const SKEW: u64 = 38;
    const DAMP: u64 = 700;
    let mut delta = if first { delta / DAMP } else { delta / 2 };
    delta += delta / u64::try_from(points).unwrap_or(1);
    let mut k = 0;
    while delta > ((BASE - TMIN) * TMAX) / 2 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + (((BASE - TMIN + 1) * delta) / (delta + SKEW))
}

#[derive(Default)]
struct DownloadTransportExtra {
    headers: Vec<(String, String)>,
    reuse: ReuseFacts,
}

fn parse_download_transport_extra(
    value: Option<&JsonValue>,
) -> Result<DownloadTransportExtra, XhttpDownloadError> {
    let Some(value) = value else {
        return Ok(DownloadTransportExtra::default());
    };
    if matches!(value, JsonValue::Null) {
        return Ok(DownloadTransportExtra::default());
    }
    let JsonValue::Object(values) = value else {
        return Err(XhttpDownloadError::TransportExtraObject);
    };
    const ALLOWED: &[&str] = &[
        "headers",
        "xmux",
        "host",
        "path",
        "mode",
        "extra",
        "xPaddingBytes",
        "xPaddingObfsMode",
        "xPaddingKey",
        "xPaddingHeader",
        "xPaddingPlacement",
        "xPaddingMethod",
        "uplinkHTTPMethod",
        "sessionIDPlacement",
        "sessionPlacement",
        "sessionIDKey",
        "sessionKey",
        "sessionIDTable",
        "sessionTable",
        "sessionIDLength",
        "sessionLength",
        "seqPlacement",
        "seqKey",
        "uplinkDataPlacement",
        "uplinkDataKey",
        "uplinkChunkSize",
        "noGRPCHeader",
        "noSSEHeader",
        "scMaxEachPostBytes",
        "scMinPostsIntervalMs",
        "scMaxBufferedPosts",
        "scStreamUpServerSecs",
        "serverMaxHeaderBytes",
        "downloadSettings",
    ];
    if values
        .iter()
        .any(|(name, _)| !ALLOWED.contains(&name.as_str()))
    {
        return Err(XhttpDownloadError::TransportExtraFields);
    }
    if ["downloadSettings", "extra"]
        .iter()
        .any(|name| field(values, name).is_some_and(|value| !matches!(value, JsonValue::Null)))
    {
        return Err(XhttpDownloadError::RecursiveDownload);
    }
    for name in ["host", "path"] {
        if let Some(value) = field(values, name)
            && !matches!(value, JsonValue::Null | JsonValue::String(_))
        {
            return Err(XhttpDownloadError::TransportCompatibilityFormat);
        }
    }
    if let Some(value) = field(values, "mode")
        && !is_null_or_one_of_strings(value, &["", "auto", "stream-one", "stream-up", "packet-up"])
    {
        return Err(XhttpDownloadError::TransportMode);
    }
    let headers = parse_headers(field(values, "headers"))?;
    let reuse = parse_reuse(field(values, "xmux"))?;
    const IGNORED: &[&str] = &[
        "xPaddingBytes",
        "xPaddingObfsMode",
        "xPaddingKey",
        "xPaddingHeader",
        "xPaddingPlacement",
        "xPaddingMethod",
        "uplinkHTTPMethod",
        "sessionIDPlacement",
        "sessionPlacement",
        "sessionIDKey",
        "sessionKey",
        "sessionIDTable",
        "sessionTable",
        "sessionIDLength",
        "sessionLength",
        "seqPlacement",
        "seqKey",
        "uplinkDataPlacement",
        "uplinkDataKey",
        "uplinkChunkSize",
        "noGRPCHeader",
        "noSSEHeader",
        "scMaxEachPostBytes",
        "scMinPostsIntervalMs",
        "scMaxBufferedPosts",
        "scStreamUpServerSecs",
        "serverMaxHeaderBytes",
    ];
    if IGNORED
        .iter()
        .any(|name| field(values, name).is_some_and(|value| !python_download_default(value)))
    {
        return Err(XhttpDownloadError::IndependentOverride);
    }
    Ok(DownloadTransportExtra { headers, reuse })
}

fn python_download_default(value: &JsonValue) -> bool {
    matches!(value, JsonValue::Null)
        || matches!(value, JsonValue::String(text) if text.is_empty() || text == "0")
        || matches!(value, JsonValue::Boolean(false))
        || matches!(value, JsonValue::Number(number) if number_is_zero(number))
}

fn python_none_or_false(value: &JsonValue) -> bool {
    matches!(value, JsonValue::Null | JsonValue::Boolean(false))
        || matches!(value, JsonValue::Number(number) if number_is_zero(number))
}

fn is_null_or_empty_string(value: &JsonValue) -> bool {
    matches!(value, JsonValue::Null) || matches!(value, JsonValue::String(text) if text.is_empty())
}

fn headers_equal(left: &[(String, String)], right: &[(String, String)]) -> bool {
    left.len() == right.len()
        && left.iter().all(|(name, value)| {
            right
                .iter()
                .any(|(other_name, other_value)| name == other_name && value == other_value)
        })
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
