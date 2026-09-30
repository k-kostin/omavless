// SPDX-License-Identifier: MIT

//! Pure parser for the provider-defined `Subscription-Userinfo` field.
//!
//! This is not an HTTP standard or a source of VPN health. The caller must pass
//! only the final HTTP response; redirects and failed responses cannot supply
//! usage. Ordinary refresh discards these values; the explicit private usage
//! read may present them without persistence or scheduling.

pub const MAX_SUBSCRIPTION_USERINFO_BYTES: usize = 256;
/// Last representable second of 9999-12-31 UTC. Later dates are not safely
/// displayable in ordinary calendar implementations.
pub const MAX_EXPIRY_UNIX_SECONDS: u64 = 253_402_300_799;

/// Provider-reported values, not locally measured bytes, entitlement or health.
/// Intentionally not `Debug`, `Display` or serializable: usage may identify an
/// account and must not enter ordinary diagnostics or public error output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionUsage {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    /// Raw Unix seconds. Zero, if supplied, is not interpreted as "unlimited".
    pub expiry_unix_seconds: Option<u64>,
}

/// Fixed classifications only; never retain or format a provider header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionMetadataError {
    DuplicateHeader,
    TooLong,
    Malformed,
    UnknownField,
    DuplicateField,
    MissingField,
    InvalidNumber,
    ExpiryOutOfRange,
}

impl SubscriptionMetadataError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateHeader => "subscription_metadata_duplicate_header",
            Self::TooLong => "subscription_metadata_too_long",
            Self::Malformed => "subscription_metadata_malformed",
            Self::UnknownField => "subscription_metadata_unknown_field",
            Self::DuplicateField => "subscription_metadata_duplicate_field",
            Self::MissingField => "subscription_metadata_missing_field",
            Self::InvalidNumber => "subscription_metadata_invalid_number",
            Self::ExpiryOutOfRange => "subscription_metadata_expiry_out_of_range",
        }
    }
}

/// Parse only the final successful response's single usage header. Missing
/// metadata and redirect/error responses yield `None`; present but ambiguous
/// metadata is rejected, never partially interpreted. Header names are ASCII
/// case-insensitive as HTTP requires, and fields are exact lower-case tokens.
pub fn parse_final_subscription_userinfo<'a>(
    status: u16,
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Option<SubscriptionUsage>, SubscriptionMetadataError> {
    if !(200..300).contains(&status) {
        return Ok(None);
    }

    let mut value = None;
    for (name, candidate) in headers {
        if name.eq_ignore_ascii_case("subscription-userinfo") && value.replace(candidate).is_some()
        {
            return Err(SubscriptionMetadataError::DuplicateHeader);
        }
    }
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.len() > MAX_SUBSCRIPTION_USERINFO_BYTES {
        return Err(SubscriptionMetadataError::TooLong);
    }
    if !raw.is_ascii()
        || raw
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
    {
        return Err(SubscriptionMetadataError::Malformed);
    }

    let mut upload = None;
    let mut download = None;
    let mut total = None;
    let mut expiry = None;
    for item in raw.split(';') {
        let Some((key, number)) = item.trim_matches([' ', '\t']).split_once('=') else {
            return Err(SubscriptionMetadataError::Malformed);
        };
        let value = parse_number(number.trim_matches([' ', '\t']))?;
        let slot = match key.trim_matches([' ', '\t']) {
            "upload" => &mut upload,
            "download" => &mut download,
            "total" => &mut total,
            "expire" => {
                if value > MAX_EXPIRY_UNIX_SECONDS {
                    return Err(SubscriptionMetadataError::ExpiryOutOfRange);
                }
                &mut expiry
            }
            _ => return Err(SubscriptionMetadataError::UnknownField),
        };
        if slot.replace(value).is_some() {
            return Err(SubscriptionMetadataError::DuplicateField);
        }
    }
    Ok(Some(SubscriptionUsage {
        upload_bytes: upload.ok_or(SubscriptionMetadataError::MissingField)?,
        download_bytes: download.ok_or(SubscriptionMetadataError::MissingField)?,
        total_bytes: total.ok_or(SubscriptionMetadataError::MissingField)?,
        expiry_unix_seconds: expiry,
    }))
}

fn parse_number(raw: &str) -> Result<u64, SubscriptionMetadataError> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SubscriptionMetadataError::InvalidNumber);
    }
    raw.parse()
        .map_err(|_| SubscriptionMetadataError::InvalidNumber)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPLETE: &str = "upload=12;download=34; total=100; expire=1893456000";

    fn parse(value: &str) -> Result<Option<SubscriptionUsage>, SubscriptionMetadataError> {
        parse_final_subscription_userinfo(200, [("Subscription-Userinfo", value)])
    }

    #[test]
    fn complete_header_and_optional_expiry() {
        let value = parse(COMPLETE).unwrap().unwrap();
        assert_eq!(
            (value.upload_bytes, value.download_bytes, value.total_bytes),
            (12, 34, 100)
        );
        assert_eq!(value.expiry_unix_seconds, Some(1_893_456_000));
        assert_eq!(
            parse("upload=0;download=0;total=0")
                .unwrap()
                .unwrap()
                .expiry_unix_seconds,
            None
        );
    }

    #[test]
    fn absent_failed_and_redirect_headers_never_supply_usage() {
        assert!(
            parse_final_subscription_userinfo(200, [("content-type", "text/plain")])
                .unwrap()
                .is_none()
        );
        for status in [301, 302, 303, 307, 308, 401, 500] {
            assert!(
                parse_final_subscription_userinfo(status, [("subscription-userinfo", COMPLETE)])
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            parse_final_subscription_userinfo(200, [("SuBsCrIpTiOn-UsErInFo", COMPLETE)])
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn duplicate_header_and_fields_are_rejected() {
        assert!(matches!(
            parse_final_subscription_userinfo(
                200,
                [
                    ("subscription-userinfo", COMPLETE),
                    ("SUBSCRIPTION-USERINFO", COMPLETE)
                ]
            ),
            Err(SubscriptionMetadataError::DuplicateHeader)
        ));
        assert!(matches!(
            parse("upload=1;upload=2;download=1;total=2"),
            Err(SubscriptionMetadataError::DuplicateField)
        ));
    }

    #[test]
    fn incomplete_and_unknown_fields_are_not_partially_accepted() {
        assert!(matches!(
            parse("upload=1;total=3"),
            Err(SubscriptionMetadataError::MissingField)
        ));
        assert!(matches!(
            parse("upload=1;download=2;total=3;token=4"),
            Err(SubscriptionMetadataError::UnknownField)
        ));
        assert!(matches!(
            parse("upload=1;download=2;total=3;"),
            Err(SubscriptionMetadataError::Malformed)
        ));
    }

    #[test]
    fn malformed_numbers_and_overflow_are_rejected() {
        for value in ["-1", "+1", "1.5", "1e3", "", "18446744073709551616"] {
            let header = format!("upload={value};download=2;total=3");
            assert!(matches!(
                parse(&header),
                Err(SubscriptionMetadataError::InvalidNumber)
            ));
        }
        assert!(matches!(
            parse("upload=1;download=2;total=3;expire=253402300800"),
            Err(SubscriptionMetadataError::ExpiryOutOfRange)
        ));
        assert_eq!(
            parse("upload=1;download=2;total=3;expire=253402300799")
                .unwrap()
                .unwrap()
                .expiry_unix_seconds,
            Some(MAX_EXPIRY_UNIX_SECONDS)
        );
    }

    #[test]
    fn controls_unicode_and_size_are_rejected_without_echo() {
        for header in [
            "upload=1\r\ndownload=2;total=3",
            "upload=1;download=2;total=3;é=4",
        ] {
            assert!(matches!(
                parse(header),
                Err(SubscriptionMetadataError::Malformed)
            ));
        }
        let long = format!(
            "upload=1;download=2;total=3;{}",
            "x".repeat(MAX_SUBSCRIPTION_USERINFO_BYTES)
        );
        assert!(matches!(
            parse(&long),
            Err(SubscriptionMetadataError::TooLong)
        ));
        assert_eq!(
            SubscriptionMetadataError::TooLong.code(),
            "subscription_metadata_too_long"
        );
    }
}
