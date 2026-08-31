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

fn normalize_xhttp_options(
    document: XhttpExtraDocument,
) -> Result<XhttpOptions, XhttpOptionsError> {
    let data = &document.root;
    if data.is_empty() {
        return Ok(empty_xhttp_options());
    }

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
    if field(data, "downloadSettings").is_some_and(|value| !matches!(value, JsonValue::Null)) {
        return Err(XhttpOptionsError::DownloadSettingsOutsideSlice);
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
