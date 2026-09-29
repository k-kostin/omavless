//! One bounded JSON document per exchange; transport framing/deadlines are future work.
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::fmt;

pub const MAX_FRAME_BYTES: usize = 8192;
pub const VERSION: u32 = 1;
pub const POLICY_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status {},
    Arm { generation: u64, mode: Mode },
    Disarm { generation: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Verified,
    ManualRecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    Unauthorized,
    GenerationConflict,
    Unavailable,
    ManualRecoveryRequired,
}

/// Emergency protection has no trustworthy generation. It must never be
/// presented as an ordinary verified armed connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Protection {
    Disarmed {},
    Armed { generation: u64 },
    Emergency {},
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Status {
        policy_version: u32,
        protection: Protection,
        health: Health,
    },
    Error {
        code: ErrorCode,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    version: u32,
    payload: T,
}

/// Deliberately carries no parser text or input fragments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireError {
    TooLarge,
    Invalid,
    UnsupportedVersion,
}

// A strict first pass is essential: internally tagged Serde enums may accept
// ignored fields in empty variants. Never deserialize via an ordinary Value,
// which silently collapses duplicate keys before the typed parser sees them.
struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded JSON")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Strict, A::Error> {
                let mut result = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if result.contains_key(&key) {
                        return Err(de::Error::custom("duplicate"));
                    }
                    result.insert(key, map.next_value::<Strict>()?.0);
                }
                Ok(Strict(Value::Object(result)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<Strict, A::Error> {
                // No v1 message contains arrays, at any depth.
                Err(de::Error::custom("array"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(value.to_owned())))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(value.into())))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(value.into())))
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(value)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            // Floats are not part of the fixed protocol.
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

fn decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, WireError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(WireError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| WireError::Invalid)?;
    // Serde structs can also deserialize positional arrays; the wire cannot.
    if !text.trim_start().starts_with('{') {
        return Err(WireError::Invalid);
    }
    let strict: Strict = serde_json::from_str(text).map_err(|_| WireError::Invalid)?;
    let envelope: Envelope<T> =
        serde_json::from_value(strict.0.clone()).map_err(|_| WireError::Invalid)?;
    if serde_json::to_value(&envelope).map_err(|_| WireError::Invalid)? != strict.0 {
        return Err(WireError::Invalid);
    }
    if envelope.version != VERSION {
        return Err(WireError::UnsupportedVersion);
    }
    Ok(envelope.payload)
}

fn encode<T: Serialize>(payload: T) -> Result<Vec<u8>, WireError> {
    let bytes = serde_json::to_vec(&Envelope {
        version: VERSION,
        payload,
    })
    .map_err(|_| WireError::Invalid)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(WireError::TooLarge);
    }
    Ok(bytes)
}

pub fn decode_request(bytes: &[u8]) -> Result<Request, WireError> {
    decode(bytes)
}

pub fn encode_request(request: Request) -> Result<Vec<u8>, WireError> {
    encode(request)
}

pub fn decode_response(bytes: &[u8]) -> Result<Response, WireError> {
    let response = decode(bytes)?;
    validate_response(response)?;
    Ok(response)
}

pub fn encode_response(response: Response) -> Result<Vec<u8>, WireError> {
    validate_response(response)?;
    encode(response)
}

fn validate_response(response: Response) -> Result<(), WireError> {
    if let Response::Status {
        policy_version,
        protection,
        health,
    } = response
    {
        if policy_version != POLICY_VERSION {
            return Err(WireError::UnsupportedVersion);
        }
        if protection == (Protection::Emergency {}) && health != Health::ManualRecoveryRequired {
            return Err(WireError::Invalid);
        }
    }
    Ok(())
}
