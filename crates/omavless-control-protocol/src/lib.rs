// SPDX-License-Identifier: MIT

//! Strict, bounded mechanics for OmaVLESS control protocol v1.
//!
//! This crate owns only wire parsing, validation, encoding, and unary stream
//! helpers. It does not open sockets, dispatch methods, read private state, or
//! manage the VPN runtime.

use serde::Deserializer;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fmt;
use std::io::{Read, Write};

pub const API_NAME: &str = "omavless.control";
pub const API_VERSION: u64 = 1;
pub const MAX_REQUEST_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_RESPONSE_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_NESTING_DEPTH: usize = 16;
pub const MAX_STRING_BYTES: usize = 32 * 1024;
pub const MAX_ID_LENGTH: usize = 64;
pub const MAX_METHOD_LENGTH: usize = 128;
pub const MAX_REVISION: u64 = i64::MAX as u64;

const REQUEST_FIELDS: &[&str] = &["api", "version", "id", "method", "params"];
const SUCCESS_RESPONSE_FIELDS: &[&str] = &["api", "version", "id", "ok", "revision", "result"];
const ERROR_RESPONSE_FIELDS: &[&str] = &["api", "version", "id", "ok", "revision", "error"];
const ERROR_FIELDS: &[&str] = &["code", "message", "retryable", "details"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableErrorCode {
    InvalidRequest,
    UnsupportedVersion,
    UnknownMethod,
    InvalidArgument,
    NotFound,
    Conflict,
    Busy,
    CapabilityUnavailable,
    PermissionDenied,
    CoreRejected,
    TransitionFailedRestored,
    ManualRecoveryRequired,
    DaemonRestarting,
    InternalError,
}

impl StableErrorCode {
    pub const ALL: [Self; 14] = [
        Self::InvalidRequest,
        Self::UnsupportedVersion,
        Self::UnknownMethod,
        Self::InvalidArgument,
        Self::NotFound,
        Self::Conflict,
        Self::Busy,
        Self::CapabilityUnavailable,
        Self::PermissionDenied,
        Self::CoreRejected,
        Self::TransitionFailedRestored,
        Self::ManualRecoveryRequired,
        Self::DaemonRestarting,
        Self::InternalError,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnknownMethod => "unknown_method",
            Self::InvalidArgument => "invalid_argument",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Busy => "busy",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::PermissionDenied => "permission_denied",
            Self::CoreRejected => "core_rejected",
            Self::TransitionFailedRestored => "transition_failed_restored",
            Self::ManualRecoveryRequired => "manual_recovery_required",
            Self::DaemonRestarting => "daemon_restarting",
            Self::InternalError => "internal_error",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => "The control request is invalid",
            Self::UnsupportedVersion => "The control protocol version is unsupported",
            Self::UnknownMethod => "The control method is not supported",
            Self::InvalidArgument => "A control request argument is invalid",
            Self::NotFound => "The requested item was not found",
            Self::Conflict => "The request conflicts with current state",
            Self::Busy => "Another operation is in progress",
            Self::CapabilityUnavailable => "The requested capability is unavailable",
            Self::PermissionDenied => "The request is not permitted",
            Self::CoreRejected => "The proxy core rejected the operation",
            Self::TransitionFailedRestored => "The transition failed and prior state was restored",
            Self::ManualRecoveryRequired => "Manual recovery is required",
            Self::DaemonRestarting => "The OmaVLESS runtime is restarting",
            Self::InternalError => "The OmaVLESS runtime encountered an internal error",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolError {
    code: StableErrorCode,
}

impl ProtocolError {
    #[must_use]
    pub const fn new(code: StableErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> StableErrorCode {
        self.code
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.message())
    }
}

impl std::error::Error for ProtocolError {}

type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Request,
    Response,
}

impl FrameKind {
    const fn limit(self) -> usize {
        match self {
            Self::Request => MAX_REQUEST_FRAME_BYTES,
            Self::Response => MAX_RESPONSE_FRAME_BYTES,
        }
    }
}

struct BoundedValueSeed {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for BoundedValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(BoundedValueVisitor { depth: self.depth })
    }
}

struct BoundedValueVisitor {
    depth: usize,
}

impl BoundedValueVisitor {
    fn checked_string<E>(value: &str) -> std::result::Result<(), E>
    where
        E: de::Error,
    {
        if value.len() > MAX_STRING_BYTES {
            return Err(E::custom("string exceeds protocol bound"));
        }
        Ok(())
    }

    fn child_depth<E>(self) -> std::result::Result<usize, E>
    where
        E: de::Error,
    {
        let depth = self.depth + 1;
        if depth > MAX_NESTING_DEPTH {
            return Err(E::custom("nesting exceeds protocol bound"));
        }
        Ok(depth)
    }
}

impl<'de> Visitor<'de> for BoundedValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Value, E>
    where
        E: de::Error,
    {
        Self::checked_string(value)?;
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Value, E>
    where
        E: de::Error,
    {
        Self::checked_string(&value)?;
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.child_depth()?;
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(BoundedValueSeed { depth })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> std::result::Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.child_depth()?;
        let mut keys = BTreeSet::new();
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            Self::checked_string(&key)?;
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom("duplicate object key"));
            }
            let value = object.next_value_seed(BoundedValueSeed { depth })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn parse_json(payload: &[u8]) -> Result<Value> {
    std::str::from_utf8(payload)
        .map_err(|_| ProtocolError::new(StableErrorCode::InvalidRequest))?;
    validate_integer_tokens(payload)?;
    let mut deserializer = serde_json::Deserializer::from_slice(payload);
    let value = BoundedValueSeed { depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(|_| ProtocolError::new(StableErrorCode::InvalidRequest))?;
    deserializer
        .end()
        .map_err(|_| ProtocolError::new(StableErrorCode::InvalidRequest))?;
    Ok(value)
}

fn validate_integer_tokens(payload: &[u8]) -> Result<()> {
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < payload.len() {
        let byte = payload[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'-' || byte.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < payload.len()
                && (payload[index].is_ascii_digit()
                    || matches!(payload[index], b'+' | b'-' | b'.' | b'e' | b'E'))
            {
                index += 1;
            }
            let token = &payload[start..index];
            if !token.iter().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
                let valid = if token.starts_with(b"-") {
                    std::str::from_utf8(token)
                        .ok()
                        .and_then(|value| value.parse::<i64>().ok())
                        .is_some()
                } else {
                    std::str::from_utf8(token)
                        .ok()
                        .and_then(|value| value.parse::<u64>().ok())
                        .is_some()
                };
                if !valid {
                    return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
                }
            }
            continue;
        }
        index += 1;
    }
    Ok(())
}

fn decode_frame(frame: &[u8], kind: FrameKind) -> Result<Value> {
    if frame.is_empty()
        || frame.len() > kind.limit()
        || !frame.ends_with(b"\n")
        || frame.iter().filter(|byte| **byte == b'\n').count() != 1
    {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    let payload = &frame[..frame.len() - 1];
    if payload.is_empty() {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    parse_json(payload)
}

fn validate_structure(value: &Value, depth: usize) -> Result<()> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(text) => {
            if text.len() > MAX_STRING_BYTES {
                return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
            }
            Ok(())
        }
        Value::Array(values) => {
            let next = depth + 1;
            if next > MAX_NESTING_DEPTH {
                return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
            }
            for value in values {
                validate_structure(value, next)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            let next = depth + 1;
            if next > MAX_NESTING_DEPTH {
                return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
            }
            for (key, value) in values {
                if key.len() > MAX_STRING_BYTES {
                    return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
                }
                validate_structure(value, next)?;
            }
            Ok(())
        }
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| ProtocolError::new(StableErrorCode::InvalidRequest))
}

fn has_exact_fields(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|field| object.contains_key(*field))
}

fn visible_ascii(value: &Value, maximum: usize) -> bool {
    value.as_str().is_some_and(|text| {
        !text.is_empty()
            && text.len() <= maximum
            && text.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    })
}

fn valid_revision(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|revision| revision <= MAX_REVISION)
}

fn valid_version(value: &Value) -> bool {
    value.as_u64() == Some(API_VERSION)
}

fn valid_public_message(value: &Value) -> bool {
    value.as_str().is_some_and(|message| {
        !message.is_empty()
            && message.len() <= 256
            && message
                .bytes()
                .all(|byte| matches!(byte, 9 | 32) || (33..=126).contains(&byte))
    })
}

pub fn validate_request(value: &Value) -> Result<()> {
    validate_structure(value, 0)?;
    let request = object(value)?;
    if !has_exact_fields(request, REQUEST_FIELDS)
        || request.get("api").and_then(Value::as_str) != Some(API_NAME)
    {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    if !valid_version(&request["version"]) {
        return Err(ProtocolError::new(StableErrorCode::UnsupportedVersion));
    }
    if !visible_ascii(&request["id"], MAX_ID_LENGTH)
        || !visible_ascii(&request["method"], MAX_METHOD_LENGTH)
    {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    let params = request["params"]
        .as_object()
        .ok_or_else(|| ProtocolError::new(StableErrorCode::InvalidArgument))?;
    if params
        .get("operationId")
        .is_some_and(|value| !visible_ascii(value, MAX_ID_LENGTH))
        || params
            .get("expectedRevision")
            .is_some_and(|value| !valid_revision(value))
    {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    Ok(())
}

pub fn decode_request(frame: &[u8]) -> Result<Value> {
    let value = decode_frame(frame, FrameKind::Request)?;
    validate_request(&value)?;
    Ok(value)
}

fn encode_frame(value: &Value, kind: FrameKind) -> Result<Vec<u8>> {
    validate_structure(value, 0)?;
    let mut frame = serde_json::to_vec(value)
        .map_err(|_| ProtocolError::new(StableErrorCode::InvalidRequest))?;
    frame.push(b'\n');
    if frame.len() > kind.limit() {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    Ok(frame)
}

pub fn encode_request(value: &Value) -> Result<Vec<u8>> {
    validate_request(value)?;
    encode_frame(value, FrameKind::Request)
}

pub fn make_request(request_id: &str, method: &str, params: Value) -> Result<Value> {
    let request = json!({
        "api": API_NAME,
        "version": API_VERSION,
        "id": request_id,
        "method": method,
        "params": params,
    });
    validate_request(&request)?;
    Ok(request)
}

pub fn negotiate_version(versions: &Value) -> Result<u64> {
    let versions = versions
        .as_array()
        .filter(|versions| !versions.is_empty())
        .ok_or_else(|| ProtocolError::new(StableErrorCode::InvalidArgument))?;
    if versions.iter().any(|version| version.as_u64().is_none()) {
        return Err(ProtocolError::new(StableErrorCode::InvalidArgument));
    }
    if versions
        .iter()
        .any(|version| version.as_u64() == Some(API_VERSION))
    {
        Ok(API_VERSION)
    } else {
        Err(ProtocolError::new(StableErrorCode::UnsupportedVersion))
    }
}

pub fn validate_response(value: &Value) -> Result<()> {
    validate_structure(value, 0)?;
    let response = object(value)?;
    if response.get("api").and_then(Value::as_str) != Some(API_NAME) {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    if response
        .get("version")
        .is_none_or(|version| !valid_version(version))
    {
        return Err(ProtocolError::new(StableErrorCode::UnsupportedVersion));
    }
    if response
        .get("id")
        .is_none_or(|id| !visible_ascii(id, MAX_ID_LENGTH))
        || response
            .get("revision")
            .is_none_or(|revision| !valid_revision(revision))
    {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    let ok = response
        .get("ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| ProtocolError::new(StableErrorCode::InvalidRequest))?;
    let expected = if ok {
        SUCCESS_RESPONSE_FIELDS
    } else {
        ERROR_RESPONSE_FIELDS
    };
    if !has_exact_fields(response, expected) {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    if ok {
        return Ok(());
    }

    let error = response["error"]
        .as_object()
        .filter(|error| {
            error.contains_key("code")
                && error.contains_key("message")
                && error.contains_key("retryable")
                && error
                    .keys()
                    .all(|field| ERROR_FIELDS.contains(&field.as_str()))
        })
        .ok_or_else(|| ProtocolError::new(StableErrorCode::InvalidRequest))?;
    if error["code"]
        .as_str()
        .and_then(StableErrorCode::parse)
        .is_none()
        || !valid_public_message(&error["message"])
        || !error["retryable"].is_boolean()
        || error
            .get("details")
            .is_some_and(|details| !details.is_object())
    {
        return Err(ProtocolError::new(StableErrorCode::InvalidRequest));
    }
    Ok(())
}

pub fn decode_response(frame: &[u8]) -> Result<Value> {
    let value = decode_frame(frame, FrameKind::Response)?;
    validate_response(&value)?;
    Ok(value)
}

pub fn encode_response(value: &Value) -> Result<Vec<u8>> {
    validate_response(value)?;
    encode_frame(value, FrameKind::Response)
}

pub fn success_response(request_id: &str, revision: u64, result: Value) -> Result<Value> {
    let response = json!({
        "api": API_NAME,
        "version": API_VERSION,
        "id": request_id,
        "ok": true,
        "revision": revision,
        "result": result,
    });
    validate_response(&response)?;
    Ok(response)
}

pub fn error_response(
    request_id: &str,
    revision: u64,
    code: StableErrorCode,
    retryable: bool,
    details: Option<Value>,
) -> Result<Value> {
    if details.as_ref().is_some_and(|details| {
        code != StableErrorCode::UnsupportedVersion || details != &json!({"supported": [1]})
    }) {
        return Err(ProtocolError::new(StableErrorCode::InvalidArgument));
    }
    let mut error = Map::new();
    error.insert("code".to_owned(), Value::String(code.as_str().to_owned()));
    error.insert(
        "message".to_owned(),
        Value::String(code.message().to_owned()),
    );
    error.insert("retryable".to_owned(), Value::Bool(retryable));
    if let Some(details) = details {
        error.insert("details".to_owned(), details);
    }
    let response = json!({
        "api": API_NAME,
        "version": API_VERSION,
        "id": request_id,
        "ok": false,
        "revision": revision,
        "error": error,
    });
    validate_response(&response)?;
    Ok(response)
}

pub fn read_unary_frame<R: Read>(reader: &mut R, kind: FrameKind) -> Result<Vec<u8>> {
    let mut frame = Vec::new();
    reader
        .take((kind.limit() + 1) as u64)
        .read_to_end(&mut frame)
        .map_err(|_| ProtocolError::new(StableErrorCode::InternalError))?;
    match kind {
        FrameKind::Request => {
            decode_request(&frame)?;
        }
        FrameKind::Response => {
            decode_response(&frame)?;
        }
    }
    Ok(frame)
}

pub fn write_unary_frame<W: Write>(writer: &mut W, frame: &[u8], kind: FrameKind) -> Result<()> {
    match kind {
        FrameKind::Request => {
            decode_request(frame)?;
        }
        FrameKind::Response => {
            decode_response(frame)?;
        }
    }
    writer
        .write_all(frame)
        .map_err(|_| ProtocolError::new(StableErrorCode::InternalError))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn request() -> Value {
        make_request("request-1", "status.get", json!({})).expect("valid request")
    }

    #[test]
    fn request_round_trip_and_version_negotiation() {
        let request = make_request(
            "hello-1",
            "system.hello",
            json!({"versions": [1], "label": "Проверка"}),
        )
        .expect("valid request");
        let frame = encode_request(&request).expect("encodable request");
        assert_eq!(decode_request(&frame).expect("decodable request"), request);
        assert_eq!(negotiate_version(&json!([3, 1])).expect("v1"), 1);
    }

    #[test]
    fn response_helpers_round_trip_every_stable_error() {
        let success = success_response("status-1", 7, json!({"actual": "disconnected"}))
            .expect("success response");
        assert_eq!(
            decode_response(&encode_response(&success).expect("success frame"))
                .expect("success decode"),
            success
        );
        for code in StableErrorCode::ALL {
            let response =
                error_response("error-1", 7, code, false, None).expect("stable error response");
            let decoded = decode_response(&encode_response(&response).expect("error frame"))
                .expect("error decode");
            assert_eq!(decoded["error"]["message"], code.message());
        }
    }

    #[test]
    fn duplicate_keys_invalid_utf8_and_trailing_data_fail_closed() {
        let cases: &[&[u8]] = &[
            b"{\"api\":\"omavless.control\",\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{}}\n",
            b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{\"x\":1,\"x\":2}}\n",
            b"{\"api\":\"\xff\"}\n",
            b"{}{}\n",
            b"{}\n{}\n",
        ];
        for frame in cases {
            let error = decode_request(frame).expect_err("invalid frame");
            assert_eq!(error.code(), StableErrorCode::InvalidRequest);
            assert!(!error.to_string().contains("api"));
        }
    }

    #[test]
    fn unary_helpers_accept_one_frame_and_reject_two() {
        let frame = encode_request(&request()).expect("request frame");
        let mut input = Cursor::new(frame.clone());
        assert_eq!(
            read_unary_frame(&mut input, FrameKind::Request).expect("one frame"),
            frame
        );
        let mut output = Vec::new();
        write_unary_frame(&mut output, &frame, FrameKind::Request).expect("write frame");
        assert_eq!(output, frame);

        let mut two = Cursor::new([frame.as_slice(), frame.as_slice()].concat());
        assert!(read_unary_frame(&mut two, FrameKind::Request).is_err());
    }

    #[test]
    fn helper_rejects_arbitrary_error_details() {
        let error = error_response(
            "error-1",
            0,
            StableErrorCode::InvalidArgument,
            false,
            Some(json!({"raw": "vless://private.invalid"})),
        )
        .expect_err("unsafe details");
        assert_eq!(error.code(), StableErrorCode::InvalidArgument);
        assert!(!error.to_string().contains("private"));
    }
}
