// SPDX-License-Identifier: MIT

//! Draft broker framing conformance, NOT the existing runtime control API.
//! No socket, dispatcher, enrollment, authentication or replay cache. Parsing a
//! lease reference grants no authority: the broker must resolve it against an
//! authenticated peer, fresh epoch and real held kernel lease before effects.

use serde::Deserialize;
use std::fmt;
use std::marker::PhantomData;

pub const API: &str = "omavless.dns";
pub const VERSION: u64 = 1;
pub const MAX_FRAME: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    InvalidFrame,
    InvalidRequest,
    UnsupportedVersion,
    UnknownMethod,
    InvalidParams,
}

impl WireError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidFrame => "invalid_frame",
            Self::InvalidRequest => "invalid_request",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnknownMethod => "unknown_method",
            Self::InvalidParams => "invalid_params",
        }
    }

    /// Parser failures never echo even an apparently valid ID from bad input.
    #[must_use]
    pub fn encode(self) -> Vec<u8> {
        format!(
            "{{\"api\":\"omavless.dns\",\"version\":1,\"id\":null,\"ok\":false,\"code\":\"{}\"}}\n",
            self.code()
        )
        .into_bytes()
    }
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFrame => "DNS frame is invalid",
            Self::InvalidRequest => "DNS request is invalid",
            Self::UnsupportedVersion => "DNS protocol version is unsupported",
            Self::UnknownMethod => "DNS method is unsupported",
            Self::InvalidParams => "DNS parameters are invalid",
        })
    }
}

impl std::error::Error for WireError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Hello,
    Status,
    Acquire,
    Apply,
    Verify,
    Release,
}

/// Debug is deliberately redacted; opaque tokens are not support-report fields.
pub struct Request {
    id: String,
    method: Method,
    epoch: Option<String>,
    operation: Option<String>,
    lease: Option<String>,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

impl Request {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn method(&self) -> Method {
        self.method
    }
    #[must_use]
    pub fn epoch(&self) -> Option<&str> {
        self.epoch.as_deref()
    }
    #[must_use]
    pub fn operation(&self) -> Option<&str> {
        self.operation.as_deref()
    }
    #[must_use]
    pub fn lease(&self) -> Option<&str> {
        self.lease.as_deref()
    }
}

// Deriving directly into closed structs rejects duplicate fields (including
// escape-equivalent names), unknown keys, wrong shapes and nested containers.
// Do not replace this with serde_json::Value, which accepts duplicate keys.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    api: String,
    version: u64,
    id: String,
    method: String,
    params: Object<Params>,
}

// Serde's derived structs also accept positional arrays. Restrict BOTH object
// levels explicitly; in particular [] must never be an alias for empty params.
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

fn present_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    #[serde(default, deserialize_with = "present_string")]
    epoch: Option<String>,
    #[serde(default, rename = "operationId", deserialize_with = "present_string")]
    operation: Option<String>,
    #[serde(default, rename = "leaseId", deserialize_with = "present_string")]
    lease: Option<String>,
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn reference(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Exactly one UTF-8 NDJSON frame. Size includes LF. CRLF is deliberately not
/// accepted in this draft internal protocol; JSON nesting is structurally fixed
/// to envelope + params objects with string/integer leaves, not recursive values.
pub fn decode(frame: &[u8]) -> Result<Request, WireError> {
    if frame.len() < 3 || frame.len() > MAX_FRAME || frame.last() != Some(&b'\n') {
        return Err(WireError::InvalidFrame);
    }
    let payload = &frame[..frame.len() - 1];
    if payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(WireError::InvalidFrame);
    }
    let text = std::str::from_utf8(payload).map_err(|_| WireError::InvalidFrame)?;
    let Object(envelope): Object<Envelope> =
        serde_json::from_str(text).map_err(|_| WireError::InvalidRequest)?;
    if envelope.api != API || !token(&envelope.id) {
        return Err(WireError::InvalidRequest);
    }
    if envelope.version != VERSION {
        return Err(WireError::UnsupportedVersion);
    }
    let method = match envelope.method.as_str() {
        "hello" => Method::Hello,
        "status" => Method::Status,
        "acquire" => Method::Acquire,
        "apply" => Method::Apply,
        "verify" => Method::Verify,
        "release" => Method::Release,
        _ => return Err(WireError::UnknownMethod),
    };
    let Params {
        epoch,
        operation,
        lease,
    } = envelope.params.0;
    let valid = match method {
        Method::Hello | Method::Status => epoch.is_none() && operation.is_none() && lease.is_none(),
        Method::Acquire => {
            epoch.as_deref().is_some_and(reference)
                && operation.as_deref().is_some_and(token)
                && lease.is_none()
        }
        Method::Apply | Method::Release => {
            epoch.as_deref().is_some_and(reference)
                && operation.as_deref().is_some_and(token)
                && lease.as_deref().is_some_and(reference)
        }
        Method::Verify => {
            epoch.as_deref().is_some_and(reference)
                && operation.is_none()
                && lease.as_deref().is_some_and(reference)
        }
    };
    if !valid {
        return Err(WireError::InvalidParams);
    }
    Ok(Request {
        id: envelope.id,
        method,
        epoch,
        operation,
        lease,
    })
}
