// SPDX-License-Identifier: MIT

//! One future-facing import dispatcher across URI profiles and native
//! WireGuard-family documents. It is intentionally pure and is not wired into
//! the installed Python runtime yet.

use std::fmt;

use crate::canonical::{CanonicalError, CanonicalProfile, parse_canonical};
use crate::wireguard::{
    WireGuardError, WireGuardFlavor, WireGuardProfile, parse_amnezia_vpn_link,
    parse_wireguard_config,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFamily {
    Vless,
    Trojan,
    Hysteria2,
    Tuic,
    WireGuard,
    AmneziaWg,
}

impl ImportFamily {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vless => "vless",
            Self::Trojan => "trojan",
            Self::Hysteria2 => "hysteria2",
            Self::Tuic => "tuic",
            Self::WireGuard => "wireguard",
            Self::AmneziaWg => "amneziawg",
        }
    }
}

pub enum ImportedProfile {
    Uri(CanonicalProfile),
    WireGuard(WireGuardProfile),
}

impl fmt::Debug for ImportedProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportedProfile")
            .field("family", &self.family())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    Uri(CanonicalError),
    WireGuard(WireGuardError),
}

impl ImportError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Uri(error) => error.code(),
            Self::WireGuard(error) => error.code(),
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uri(error) => error.fmt(formatter),
            Self::WireGuard(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ImportError {}

fn first_nonempty_line(input: &str) -> &str {
    for (index, raw_line) in input.lines().enumerate() {
        let mut line = raw_line.trim();
        if index == 0 {
            line = line.strip_prefix('\u{feff}').unwrap_or(line).trim_start();
        }
        if !line.is_empty() && !line.starts_with('#') && !line.starts_with(';') {
            return line;
        }
    }
    ""
}

pub fn parse_import(input: &str) -> Result<ImportedProfile, ImportError> {
    let trimmed = input.trim_start();
    if trimmed
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("vpn://"))
    {
        return parse_amnezia_vpn_link(input)
            .map(ImportedProfile::WireGuard)
            .map_err(ImportError::WireGuard);
    }
    if matches!(
        first_nonempty_line(input).to_ascii_lowercase().as_str(),
        "[interface]" | "[peer]"
    ) {
        return parse_wireguard_config(input)
            .map(ImportedProfile::WireGuard)
            .map_err(ImportError::WireGuard);
    }
    parse_canonical(input)
        .map(ImportedProfile::Uri)
        .map_err(ImportError::Uri)
}

impl ImportedProfile {
    #[must_use]
    pub fn family(&self) -> ImportFamily {
        match self {
            Self::Uri(profile) => match profile.protocol() {
                crate::Protocol::Vless => ImportFamily::Vless,
                crate::Protocol::Trojan => ImportFamily::Trojan,
                crate::Protocol::Hysteria2 => ImportFamily::Hysteria2,
                crate::Protocol::Tuic => ImportFamily::Tuic,
            },
            Self::WireGuard(profile) => match profile.facts().flavor {
                WireGuardFlavor::Standard => ImportFamily::WireGuard,
                WireGuardFlavor::Amnezia(_) => ImportFamily::AmneziaWg,
            },
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        match self {
            Self::Uri(profile) => profile.subscription_identity(),
            Self::WireGuard(profile) => profile.subscription_identity(),
        }
    }

    #[must_use]
    pub fn render_mihomo_proxy(&self, name: &str, server_override: Option<&str>) -> String {
        match self {
            Self::Uri(profile) => profile.render_mihomo_proxy(name, server_override),
            Self::WireGuard(profile) => profile.render_mihomo_proxy(name, server_override),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE_KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    const PUBLIC_KEY: &str = "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=";

    #[test]
    fn one_dispatcher_accepts_existing_uris_and_native_wireguard() {
        let uri = parse_import(
            "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp",
        )
        .unwrap();
        assert_eq!(uri.family(), ImportFamily::Vless);

        let config = format!(
            "[Interface]\nPrivateKey={PRIVATE_KEY}\nAddress=10.0.0.2/32\n[Peer]\nPublicKey={PUBLIC_KEY}\nAllowedIPs=0.0.0.0/0\nEndpoint=203.0.113.2:51820\n"
        );
        let wireguard = parse_import(&config).unwrap();
        assert_eq!(wireguard.family(), ImportFamily::WireGuard);
        assert_eq!(wireguard.subscription_identity().len(), 64);
        assert!(
            wireguard
                .render_mihomo_proxy("WG", None)
                .contains("type: wireguard")
        );
    }

    #[test]
    fn dispatcher_debug_and_errors_remain_private() {
        let input = format!(
            "[Interface]\nPrivateKey={PRIVATE_KEY}\nAddress=10.0.0.2/32\n[Peer]\nPublicKey={PUBLIC_KEY}\nAllowedIPs=0.0.0.0/0\nEndpoint=private.example.invalid:51820\n"
        );
        let debug = format!("{:?}", parse_import(&input).unwrap());
        assert!(!debug.contains(PRIVATE_KEY));
        assert!(!debug.contains(PUBLIC_KEY));
        assert!(!debug.contains("private.example.invalid"));
    }
}
