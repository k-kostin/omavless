// SPDX-License-Identifier: MIT
//! Selected-host DNS policy for rendering, never a saved-template mutation.
//!
//! Flags are DATA. The caller must independently verify the selected managed
//! package and normal enrollment/owner gates; these bytes grant no authority.
use omavless_domain::private_store::DnsPreset;
use std::borrow::Cow;

const BUNDLES: [&str; 3] = [
    include_str!("../../../templates/default.yaml"),
    include_str!("../../../templates/china.yaml"),
    include_str!("../../../templates/iran.yaml"),
];
const ANCHOR: &str = "  device: Meta\n";
const MANAGED: &str = "  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\n";

fn is_bundled_mode_variant(bundle: &str, template: &str) -> bool {
    ["rule", "global", "direct"].iter().any(|mode| {
        omavless_domain::routing::template_with_mode(bundle, mode)
            .is_ok_and(|candidate| candidate == template)
    })
}

/// Only byte-exact default variants qualify for persistent setup preparation.
/// Rendering also accepts the other presets, but that is a separate boundary.
pub(crate) fn is_bundled_default_mode_variant(template: &str) -> bool {
    is_bundled_mode_variant(BUNDLES[0], template)
}

pub(crate) fn dns_editable(template: &str) -> bool {
    bundled_dns_source(template).is_some()
}

/// Also recognize setup's exact persisted managed DEFAULT counterpart. This
/// fixed identity is already portable/managed-selection policy, never a general
/// admission of custom templates which merely contain the managed flags.
pub(crate) fn bundled_dns_source(template: &str) -> Option<&'static str> {
    if template.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
        return None;
    }
    BUNDLES
        .into_iter()
        .find(|bundle| is_bundled_mode_variant(bundle, template))
        .or_else(|| {
            let managed_default = BUNDLES[0].replacen(ANCHOR, MANAGED, 1);
            is_bundled_mode_variant(&managed_default, template).then_some(BUNDLES[0])
        })
}

/// Pure transient rendering. Eligibility is checked against ORIGINAL bytes,
/// before managed flags or any other config assembly. Custom DNS is untouched.
pub(crate) fn with_dns(template: &str, preset: DnsPreset) -> Option<Cow<'_, str>> {
    if template.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
        return None;
    }
    if preset == DnsPreset::PresetDefault || !dns_editable(template) {
        return Some(Cow::Borrowed(template));
    }
    overlay_dns(template, preset).map(Cow::Owned)
}

pub(crate) fn for_host_with_dns(
    template: &str,
    selected: bool,
    preset: DnsPreset,
) -> Option<Cow<'_, str>> {
    let eligible = dns_editable(template);
    let managed = for_host(template, selected)?;
    if eligible && preset != DnsPreset::PresetDefault {
        overlay_dns(&managed, preset).map(Cow::Owned)
    } else {
        Some(managed)
    }
}

// Called only after exact bundled identity admission. A deliberately small
// structural transform replaces four list values, preserving all other bytes.
fn overlay_dns(template: &str, preset: DnsPreset) -> Option<String> {
    let ip = match preset {
        DnsPreset::Google => "8.8.8.8",
        DnsPreset::Cloudflare => "1.1.1.1",
        DnsPreset::PresetDefault => return Some(template.to_owned()),
    };
    let dns = template.find("\ndns:\n")? + 1;
    let end = template[dns + 5..].find("\n\n")? + dns + 5;
    let mut section = template[dns..end].to_owned();
    for (key, value) in [
        ("default-nameserver", ip.to_owned()),
        ("proxy-server-nameserver", ip.to_owned()),
        ("direct-nameserver", format!("https://{ip}/dns-query")),
        ("nameserver", format!("https://{ip}/dns-query#PROXY")),
    ] {
        let anchor = format!("  {key}:\n");
        if section.matches(&anchor).count() != 1 {
            return None;
        }
        let start = section.find(&anchor)? + anchor.len();
        let length: usize = section[start..]
            .split_inclusive('\n')
            .take_while(|line| line.starts_with("    - "))
            .map(str::len)
            .sum();
        if length == 0 {
            return None;
        }
        // The last list item can lack a newline at the section boundary.
        let newline = section[start..start + length].ends_with('\n');
        section.replace_range(
            start..start + length,
            &format!("    - {value}{}", if newline { "\n" } else { "" }),
        );
    }
    let mut rendered = String::with_capacity(template.len() + section.len());
    rendered.push_str(&template[..dns]);
    rendered.push_str(&section);
    rendered.push_str(&template[end..]);
    Some(rendered)
}

pub(crate) fn for_host(template: &str, selected: bool) -> Option<Cow<'_, str>> {
    if template.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
        return None;
    }
    if !selected {
        return Some(Cow::Borrowed(template));
    }
    let policy = crate::core_readiness::ConfigReadiness::from_generated_config(
        crate::desired::RoutingMode::Rule,
        String::new(),
        template,
    )?;
    // Preserve every already accepted explicit managed policy, without repair.
    if policy.managed_dns() {
        return Some(Cow::Borrowed(template));
    }
    // No YAML rewriting of custom/partial policies. Only exact trusted bundles
    // and their existing canonical mode variants earn this local rendering.
    let known = BUNDLES
        .iter()
        .any(|bundle| is_bundled_mode_variant(bundle, template));
    if !known || template.matches(ANCHOR).count() != 1 {
        return None;
    }
    let rendered = template.replacen(ANCHOR, MANAGED, 1);
    crate::core_readiness::ConfigReadiness::from_generated_config(
        crate::desired::RoutingMode::Rule,
        String::new(),
        &rendered,
    )
    .filter(|policy| policy.managed_dns())?;
    Some(Cow::Owned(rendered))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_choices_only_change_four_values_in_exact_original_bundles() {
        for bundle in BUNDLES {
            for mode in ["rule", "global", "direct"] {
                let original = omavless_domain::routing::template_with_mode(bundle, mode).unwrap();
                assert!(dns_editable(&original));
                assert!(matches!(
                    with_dns(&original, DnsPreset::PresetDefault),
                    Some(Cow::Borrowed(_))
                ));
                for preset in [DnsPreset::Google, DnsPreset::Cloudflare] {
                    let rendered = with_dns(&original, preset).unwrap();
                    let selected = for_host_with_dns(&original, true, preset).unwrap();
                    assert_eq!(selected, rendered.replacen(ANCHOR, MANAGED, 1));
                    let ip = if preset == DnsPreset::Google {
                        "8.8.8.8"
                    } else {
                        "1.1.1.1"
                    };
                    assert!(rendered.contains(&format!(
                        "  nameserver:\n    - https://{ip}/dns-query#PROXY"
                    )));
                    assert_eq!(
                        original.split("\ndns:\n").next(),
                        rendered.split("\ndns:\n").next()
                    );
                    assert_eq!(
                        original.split("\nsniffer:\n").nth(1),
                        rendered.split("\nsniffer:\n").nth(1)
                    );
                    for protected in [
                        "  ipv6: false\n",
                        "  enhanced-mode: fake-ip\n",
                        "  fake-ip-range: 198.18.0.1/16\n",
                    ] {
                        assert!(rendered.contains(protected));
                    }
                }
            }
        }
    }

    #[test]
    fn custom_dns_is_not_overridden_even_if_already_managed() {
        let custom = format!("# custom\n{}", BUNDLES[0].replacen(ANCHOR, MANAGED, 1));
        assert!(!dns_editable(&custom));
        assert_eq!(
            for_host_with_dns(&custom, true, DnsPreset::Google).unwrap(),
            custom
        );
        assert_eq!(with_dns(&custom, DnsPreset::Cloudflare).unwrap(), custom);
    }

    #[test]
    fn setup_managed_default_keeps_flags_and_accepts_dns_in_all_modes() {
        let managed = BUNDLES[0].replacen(ANCHOR, MANAGED, 1);
        for mode in ["rule", "global", "direct"] {
            let original = omavless_domain::routing::template_with_mode(&managed, mode).unwrap();
            assert!(dns_editable(&original));
            let rendered = for_host_with_dns(&original, true, DnsPreset::Google).unwrap();
            assert_eq!(rendered, with_dns(&original, DnsPreset::Google).unwrap());
            assert_eq!(rendered.matches("  omavless-dns-broker: true\n").count(), 1);
            assert!(rendered.contains("  nameserver:\n    - https://8.8.8.8/dns-query#PROXY"));
            let edited = original.replace(
                "  fake-ip-range: 198.18.0.1/16",
                "  fake-ip-range: 198.19.0.1/16",
            );
            assert!(!dns_editable(&edited));
            assert_eq!(with_dns(&edited, DnsPreset::Google).unwrap(), edited);
        }
    }

    #[test]
    fn user_choice_does_not_replace_wireguard_outbound_dns_or_relax_refusal() {
        use omavless_domain::private_store::{apply_profile_import, parse_private_store};
        let id = "40000000-0000-4000-8000-000000000001";
        for dns in [
            None,
            Some("9.9.9.9, 1.1.1.1"),
            Some("2001:4860:4860::8888"),
            Some("9.9.9.9, 2001:4860:4860::8888"),
        ] {
            let input = format!(
                "[Interface]\nPrivateKey = BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\nAddress = 10.8.0.2/32\n{}[Peer]\nPublicKey = CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\n",
                dns.map(|dns| format!("DNS = {dns}\n")).unwrap_or_default()
            );
            let imported = apply_profile_import(
                r#"{"version":3,"profiles":[],"subscriptions":[]}"#,
                id,
                "Synthetic",
                &input,
            )
            .unwrap();
            let store =
                parse_private_store(std::str::from_utf8(imported.payload()).unwrap()).unwrap();
            let mut original = None;
            for preset in [
                DnsPreset::PresetDefault,
                DnsPreset::Google,
                DnsPreset::Cloudflare,
            ] {
                let template = for_host_with_dns(BUNDLES[0], true, preset).unwrap();
                let config = store.prepare_config_mode(
                    id,
                    &template,
                    "/synthetic/controller.sock",
                    "global",
                );
                if dns.is_some_and(|dns| dns.contains(':')) {
                    assert!(config.is_err());
                    continue;
                }
                let config = config.unwrap();
                let outbound = config.split("\nproxies:\n").nth(1).unwrap();
                if let Some(original) = original.as_deref() {
                    assert!(outbound == original);
                } else {
                    original = Some(outbound.to_owned());
                }
                if dns.is_none() {
                    assert!(!config.contains("  remote-dns-resolve: true"));
                } else {
                    assert!(config.contains("  remote-dns-resolve: true"));
                    assert!(outbound.find("9.9.9.9").unwrap() < outbound.find("1.1.1.1").unwrap());
                }
            }
        }
    }

    #[test]
    fn every_plain_preset_and_mode_gets_local_flags_without_changing_source() {
        for (index, bundle) in BUNDLES.into_iter().enumerate() {
            for mode in ["rule", "global", "direct"] {
                let original = omavless_domain::routing::template_with_mode(bundle, mode).unwrap();
                assert_eq!(is_bundled_default_mode_variant(&original), index == 0);
                let rendered = for_host(&original, true).unwrap();
                assert!(matches!(rendered, Cow::Owned(_)));
                assert_eq!(rendered, original.replacen(ANCHOR, MANAGED, 1));
                assert!(!original.contains("omavless-dns-broker"));
                assert_eq!(for_host(&rendered, true).unwrap(), rendered);
            }
        }
    }

    #[test]
    fn no_selected_pair_preserves_the_original_legacy_policy() {
        for text in [BUNDLES[0], "custom policy\n", "tun:\n  device: other\n"] {
            assert!(matches!(for_host(text, false), Some(Cow::Borrowed(value)) if value == text));
        }
    }

    #[test]
    fn custom_and_partial_managed_templates_are_not_silently_repaired() {
        for text in [
            format!("# custom\n{}", BUNDLES[0]),
            BUNDLES[0].replace('\n', "\r\n"),
            BUNDLES[0].replacen(ANCHOR, "  device: another\n", 1),
            BUNDLES[0].replacen(ANCHOR, "  device: Meta\n  disable-system-dns: true\n", 1),
            BUNDLES[0].replacen(ANCHOR, "  device: Meta\n  omavless-dns-broker: false\n", 1),
            BUNDLES[0].replacen(
                ANCHOR,
                "  device: Meta\n  omavless-dns-broker: true\n  disable-system-dns: false\n",
                1,
            ),
        ] {
            assert!(for_host(&text, true).is_none());
        }
    }

    #[test]
    fn already_managed_custom_policy_keeps_its_previous_accepted_bytes() {
        let managed = format!(
            "# existing custom policy\n{}",
            BUNDLES[0].replacen(ANCHOR, MANAGED, 1)
        );
        assert!(matches!(for_host(&managed, true), Some(Cow::Borrowed(value)) if value == managed));
    }

    #[test]
    fn over_limit_input_refuses_before_any_copy() {
        let oversized = "x".repeat(omavless_domain::config::MAX_TEMPLATE_BYTES + 1);
        assert!(for_host(&oversized, true).is_none());
        assert!(for_host(&oversized, false).is_none());
    }
}
