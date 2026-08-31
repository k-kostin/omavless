// SPDX-License-Identifier: MIT

//! Bounded client-side VLESS Encryption grammar accepted by current Mihomo.
//!
//! Key material is validated in memory but is never retained in the public
//! metadata model or included in errors/debug output.

use std::fmt;

use crate::base64url::decoded_len_if_canonical;

pub const MAX_VLESS_ENCRYPTION_BYTES: usize = 12 * 1024;
const MAX_PARTS: usize = 32;
const MAX_PADDING: u64 = 65_553;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessEncryptionMode {
    Native,
    XorPub,
    Random,
}

impl VlessEncryptionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::XorPub => "xorpub",
            Self::Random => "random",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessEncryptionRtt {
    ZeroRtt,
    OneRtt,
}

impl VlessEncryptionRtt {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZeroRtt => "0rtt",
            Self::OneRtt => "1rtt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessEncryption {
    pub mode: VlessEncryptionMode,
    pub rtt: VlessEncryptionRtt,
    pub key_count: usize,
    pub large_key_present: bool,
    pub padding_count: usize,
    pub total_padding: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessEncryptionError {
    TooLarge,
    InvalidFormat,
    Unsupported,
    InvalidPadding,
    PaddingRange,
    FirstPaddingTooSmall,
    InvalidKey,
    KeyRequired,
    TotalPaddingTooLarge,
}

impl VlessEncryptionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "encryption_too_large",
            Self::InvalidFormat => "invalid_encryption_format",
            Self::Unsupported => "unsupported_encryption",
            Self::InvalidPadding => "invalid_encryption_padding",
            Self::PaddingRange => "encryption_padding_range",
            Self::FirstPaddingTooSmall => "encryption_first_padding_too_small",
            Self::InvalidKey => "invalid_encryption_key",
            Self::KeyRequired => "encryption_key_required",
            Self::TotalPaddingTooLarge => "encryption_total_padding_too_large",
        }
    }
}

impl fmt::Display for VlessEncryptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "VLESS Encryption value is too large",
            Self::InvalidFormat => "VLESS Encryption value has an invalid format",
            Self::Unsupported => "VLESS Encryption value is unsupported",
            Self::InvalidPadding => "VLESS Encryption padding has an invalid format",
            Self::PaddingRange => "VLESS Encryption padding is outside the supported range",
            Self::FirstPaddingTooSmall => "VLESS Encryption first padding range is too small",
            Self::InvalidKey => "VLESS Encryption key has an invalid format",
            Self::KeyRequired => "VLESS Encryption requires at least one client key",
            Self::TotalPaddingTooLarge => "VLESS Encryption total padding is too large",
        })
    }
}

impl std::error::Error for VlessEncryptionError {}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn padding(token: &str) -> Result<(u64, u64, u64), VlessEncryptionError> {
    let mut values = token.split('-');
    let (Some(probability), Some(minimum), Some(maximum), None) =
        (values.next(), values.next(), values.next(), values.next())
    else {
        return Err(VlessEncryptionError::InvalidPadding);
    };
    let (Some(probability), Some(minimum), Some(maximum)) =
        (decimal(probability), decimal(minimum), decimal(maximum))
    else {
        return Err(VlessEncryptionError::InvalidPadding);
    };
    Ok((probability, minimum, maximum))
}

pub fn parse_vless_encryption(
    value: &str,
) -> Result<Option<VlessEncryption>, VlessEncryptionError> {
    if matches!(value, "" | "none") {
        return Ok(None);
    }
    if value.len() > MAX_VLESS_ENCRYPTION_BYTES {
        return Err(VlessEncryptionError::TooLarge);
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(VlessEncryptionError::InvalidFormat);
    }
    let parts = value.split('.').collect::<Vec<_>>();
    if !(4..=MAX_PARTS).contains(&parts.len()) || parts[0] != "mlkem768x25519plus" {
        return Err(VlessEncryptionError::Unsupported);
    }
    let mode = match parts[1] {
        "native" => VlessEncryptionMode::Native,
        "xorpub" => VlessEncryptionMode::XorPub,
        "random" => VlessEncryptionMode::Random,
        _ => return Err(VlessEncryptionError::Unsupported),
    };
    let rtt = match parts[2] {
        "0rtt" => VlessEncryptionRtt::ZeroRtt,
        "1rtt" => VlessEncryptionRtt::OneRtt,
        _ => return Err(VlessEncryptionError::Unsupported),
    };

    let mut key_count = 0;
    let mut large_key_present = false;
    let mut padding_count = 0;
    let mut total_padding = 0_u64;
    for token in &parts[3..] {
        if token.len() < 20 {
            let (probability, minimum, maximum) = padding(token)?;
            if probability > 100 || minimum > maximum || maximum > MAX_PADDING {
                return Err(VlessEncryptionError::PaddingRange);
            }
            if padding_count == 0 && (probability != 100 || minimum < 35) {
                return Err(VlessEncryptionError::FirstPaddingTooSmall);
            }
            if padding_count % 2 == 0 {
                total_padding += maximum;
            }
            padding_count += 1;
            continue;
        }
        match decoded_len_if_canonical(token) {
            Some(32) => {}
            Some(1184) => large_key_present = true,
            _ => return Err(VlessEncryptionError::InvalidKey),
        }
        key_count += 1;
    }
    if key_count == 0 {
        return Err(VlessEncryptionError::KeyRequired);
    }
    if total_padding > MAX_PADDING {
        return Err(VlessEncryptionError::TotalPaddingTooLarge);
    }
    Ok(Some(VlessEncryption {
        mode,
        rtt,
        key_count,
        large_key_present,
        padding_count,
        total_padding: total_padding as usize,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";

    #[test]
    fn accepts_disabled_modes_keys_and_padding() {
        assert_eq!(parse_vless_encryption("none"), Ok(None));
        let parsed = parse_vless_encryption(&format!(
            "mlkem768x25519plus.random.1rtt.100-35-100.75-0-50.50-0-200.{KEY}"
        ))
        .expect("bounded encryption")
        .expect("enabled encryption");
        assert_eq!(parsed.mode, VlessEncryptionMode::Random);
        assert_eq!(parsed.rtt, VlessEncryptionRtt::OneRtt);
        assert_eq!(parsed.key_count, 1);
        assert_eq!(parsed.padding_count, 3);
        assert_eq!(parsed.total_padding, 300);
    }

    #[test]
    fn accepts_32_and_1184_byte_canonical_keys() {
        let parsed = parse_vless_encryption(&format!(
            "mlkem768x25519plus.native.0rtt.{}.{}",
            "A".repeat(43),
            "A".repeat(1579),
        ))
        .expect("canonical key sizes")
        .expect("enabled encryption");
        assert_eq!(parsed.key_count, 2);
        assert!(parsed.large_key_present);
    }

    #[test]
    fn rejects_private_values_with_fixed_errors() {
        let private = "private-secret";
        let cases = [
            (private.to_owned(), VlessEncryptionError::Unsupported),
            (
                format!("mlkem768x25519plus.native.1rtt.{private}"),
                VlessEncryptionError::InvalidPadding,
            ),
            (
                format!("mlkem768x25519plus.native.1rtt.{}B", "A".repeat(42)),
                VlessEncryptionError::InvalidKey,
            ),
        ];
        for (value, expected) in cases {
            let error = parse_vless_encryption(&value).expect_err("invalid encryption");
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(private));
            assert!(error.to_string().len() <= 80);
        }
    }
}
