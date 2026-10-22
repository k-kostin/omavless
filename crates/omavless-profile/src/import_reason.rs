// SPDX-License-Identifier: MIT
//! Closed, input-free import diagnostics. Never retain a field name or value.

use crate::canonical::CanonicalError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportReason {
    UnsupportedField,
    UnsupportedTransport,
    UnsupportedOption,
    InvalidProfile,
    UnsupportedProtocol,
    Duplicate,
}

impl ImportReason {
    pub const ALL: [Self; 6] = [
        Self::UnsupportedField,
        Self::UnsupportedTransport,
        Self::UnsupportedOption,
        Self::InvalidProfile,
        Self::UnsupportedProtocol,
        Self::Duplicate,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedField => "unsupported_field",
            Self::UnsupportedTransport => "unsupported_transport",
            Self::UnsupportedOption => "unsupported_option",
            Self::InvalidProfile => "invalid_profile",
            Self::UnsupportedProtocol => "unsupported_protocol",
            Self::Duplicate => "duplicate",
        }
    }

    #[must_use]
    pub fn from_canonical(error: CanonicalError) -> Self {
        match error.code() {
            "unsupported_fields" | "unsupported_field" => Self::UnsupportedField,
            "unsupported_transport" => Self::UnsupportedTransport,
            "unsupported_scheme" | "unsupported_protocol" => Self::UnsupportedProtocol,
            code if code.starts_with("unsupported_") || code == "local_bandwidth" => {
                Self::UnsupportedOption
            }
            _ => Self::InvalidProfile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::parse_canonical;

    #[test]
    fn rejected_profiles_have_closed_reasons_without_private_data() {
        let base = "vless://11111111-1111-4111-8111-111111111111@fixture.invalid:443?security=none&type=tcp";
        for (input, expected) in [
            (
                format!("{base}&fm=private-secret"),
                ImportReason::UnsupportedField,
            ),
            (
                base.replace("type=tcp", "type=kcp"),
                ImportReason::UnsupportedTransport,
            ),
            (
                base.replace("security=none", "security=unknown"),
                ImportReason::UnsupportedOption,
            ),
            (
                "vless://private-secret@invalid".into(),
                ImportReason::InvalidProfile,
            ),
            (
                "hy2://private-secret@fixture.invalid:443?fm=private-value".into(),
                ImportReason::UnsupportedField,
            ),
        ] {
            let error = parse_canonical(&input).unwrap_err();
            let reason = ImportReason::from_canonical(error);
            assert_eq!(reason, expected);
            let mut reasons = ImportReasons::default();
            reasons.record(reason);
            let output = format!("{reason:?} {}", reasons.value());
            for private in [
                "private-secret",
                "private-value",
                "fixture.invalid",
                "11111111",
                "fm",
            ] {
                assert!(!output.contains(private));
            }
        }
    }
}

/// Fixed-size counters: memory/output do not grow with rejected private input.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportReasons([usize; 6]);

impl ImportReasons {
    #[must_use]
    pub fn from_value(value: &serde_json::Value, expected: usize) -> Option<Self> {
        if expected > 65536 {
            return None;
        }
        let object = value.as_object()?;
        let mut result = Self::default();
        let mut total = 0;
        for (key, count) in object {
            let index = ImportReason::ALL
                .iter()
                .position(|reason| reason.code() == key)?;
            let count = usize::try_from(count.as_u64()?).ok()?;
            if count == 0 || count > expected {
                return None;
            }
            total += count;
            result.0[index] = count;
        }
        (total == expected).then_some(result)
    }
    #[must_use]
    pub fn entries(self) -> impl Iterator<Item = (ImportReason, usize)> {
        ImportReason::ALL
            .into_iter()
            .zip(self.0)
            .filter(|(_, count)| *count > 0)
    }
    pub fn record(&mut self, reason: ImportReason) {
        self.0[reason as usize] += 1;
    }
    pub fn merge(&mut self, other: Self) {
        for (count, next) in self.0.iter_mut().zip(other.0) {
            *count += next;
        }
    }
    #[must_use]
    pub fn value(self) -> serde_json::Value {
        serde_json::Value::Object(
            ImportReason::ALL
                .into_iter()
                .enumerate()
                .filter(|(i, _)| self.0[*i] > 0)
                .map(|(i, reason)| (reason.code().into(), serde_json::json!(self.0[i])))
                .collect(),
        )
    }
}
