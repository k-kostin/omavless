// SPDX-License-Identifier: MIT
//! Selected-host DNS policy for rendering, never a saved-template mutation.
//!
//! Flags are DATA. The caller must independently verify the selected managed
//! package and normal enrollment/owner gates; these bytes grant no authority.
use std::borrow::Cow;

const BUNDLES: [&str; 3] = [
    include_str!("../../../templates/default.yaml"),
    include_str!("../../../templates/china.yaml"),
    include_str!("../../../templates/iran.yaml"),
];
const ANCHOR: &str = "  device: Meta\n";
const MANAGED: &str = "  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\n";

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
    let known = BUNDLES.iter().any(|bundle| {
        ["rule", "global", "direct"].iter().any(|mode| {
            omavless_domain::routing::template_with_mode(bundle, mode)
                .is_ok_and(|candidate| candidate == template)
        })
    });
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
    fn every_plain_preset_and_mode_gets_local_flags_without_changing_source() {
        for bundle in BUNDLES {
            for mode in ["rule", "global", "direct"] {
                let original = omavless_domain::routing::template_with_mode(bundle, mode).unwrap();
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
