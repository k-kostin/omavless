// SPDX-License-Identifier: MIT

//! One private canonical profile boundary for the future Rust runtime.

use crate::hysteria2::{Hysteria2Error, Hysteria2Profile, parse_hysteria2};
use crate::trojan::{TrojanError, TrojanProfile, parse_trojan};
use crate::tuic::{TuicError, TuicProfile, parse_tuic};
use crate::vless_canonical::{VlessCanonicalError, VlessCanonicalProfile, parse_vless_canonical};
use crate::{ClassificationError, Protocol, classify_protocol};
use std::fmt;

pub enum CanonicalProfile {
    Vless(VlessCanonicalProfile),
    Trojan(TrojanProfile),
    Hysteria2(Hysteria2Profile),
    Tuic(TuicProfile),
}

impl fmt::Debug for CanonicalProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalProfile")
            .field("protocol", &self.protocol())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalError {
    Classification(ClassificationError),
    Vless(VlessCanonicalError),
    Trojan(TrojanError),
    Hysteria2(Hysteria2Error),
    Tuic(TuicError),
}

impl CanonicalError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Classification(error) => error.code(),
            Self::Vless(error) => error.code(),
            Self::Trojan(error) => error.code(),
            Self::Hysteria2(error) => error.code(),
            Self::Tuic(error) => error.code(),
        }
    }
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Classification(error) => error.fmt(formatter),
            Self::Vless(error) => error.fmt(formatter),
            Self::Trojan(error) => error.fmt(formatter),
            Self::Hysteria2(error) => error.fmt(formatter),
            Self::Tuic(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CanonicalError {}

pub fn parse_canonical(input: &str) -> Result<CanonicalProfile, CanonicalError> {
    match classify_protocol(input).map_err(CanonicalError::Classification)? {
        Protocol::Vless => parse_vless_canonical(input)
            .map(CanonicalProfile::Vless)
            .map_err(CanonicalError::Vless),
        Protocol::Trojan => parse_trojan(input)
            .map(CanonicalProfile::Trojan)
            .map_err(CanonicalError::Trojan),
        Protocol::Hysteria2 => parse_hysteria2(input)
            .map(CanonicalProfile::Hysteria2)
            .map_err(CanonicalError::Hysteria2),
        Protocol::Tuic => parse_tuic(input)
            .map(CanonicalProfile::Tuic)
            .map_err(CanonicalError::Tuic),
    }
}

impl CanonicalProfile {
    #[must_use]
    pub const fn protocol(&self) -> Protocol {
        match self {
            Self::Vless(_) => Protocol::Vless,
            Self::Trojan(_) => Protocol::Trojan,
            Self::Hysteria2(_) => Protocol::Hysteria2,
            Self::Tuic(_) => Protocol::Tuic,
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        match self {
            Self::Vless(profile) => profile.subscription_identity(),
            Self::Trojan(profile) => profile.subscription_identity(),
            Self::Hysteria2(profile) => profile.subscription_identity(),
            Self::Tuic(profile) => profile.subscription_identity(),
        }
    }

    #[must_use]
    pub fn render_mihomo_proxy(&self, name: &str, server_override: Option<&str>) -> String {
        match self {
            Self::Vless(profile) => profile.render_mihomo_proxy(name, server_override),
            Self::Trojan(profile) => profile.render_mihomo_proxy(name, server_override),
            Self::Hysteria2(profile) => profile.render_mihomo_proxy(name, server_override),
            Self::Tuic(profile) => profile.render_mihomo_proxy(name, server_override),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: [(&str, Protocol); 4] = [
        (
            "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#PrivateVless",
            Protocol::Vless,
        ),
        (
            "trojan://private-password@203.0.113.2:443?security=tls&sni=cdn.example.invalid#PrivateTrojan",
            Protocol::Trojan,
        ),
        (
            "hy2://private-auth@203.0.113.3:443?sni=cdn.example.invalid#PrivateHy2",
            Protocol::Hysteria2,
        ),
        (
            "tuic://22222222-2222-4222-8222-222222222222:private-password@203.0.113.4:443?sni=cdn.example.invalid#PrivateTuic",
            Protocol::Tuic,
        ),
    ];

    #[test]
    fn one_dispatcher_parses_and_renders_every_existing_family() {
        for (index, (input, protocol)) in CASES.into_iter().enumerate() {
            let profile = parse_canonical(input).unwrap();
            assert_eq!(profile.protocol(), protocol);
            assert_eq!(profile.subscription_identity().len(), 64);
            let rendered = profile.render_mihomo_proxy(&format!("Node {index}"), None);
            assert!(rendered.starts_with("- "));
            assert!(rendered.contains(&format!("Node {index}")));
        }
    }

    #[test]
    fn dispatcher_debug_and_errors_never_echo_private_input() {
        for (input, _) in CASES {
            let debug = format!("{:?}", parse_canonical(input).unwrap());
            for marker in [
                "private",
                "203.0.113",
                "example.invalid",
                "11111111",
                "22222222",
            ] {
                assert!(!debug.to_lowercase().contains(marker));
            }
        }
        let error = parse_canonical("trojan://private-password@bad").unwrap_err();
        assert!(!error.to_string().contains("private-password"));
        assert!(!format!("{error:?}").contains("private-password"));
    }
}
