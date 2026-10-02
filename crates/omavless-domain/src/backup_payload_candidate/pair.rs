// SPDX-License-Identifier: MIT

//! Restricted whole-pair gate. Unknown/custom templates are refused, never
//! rewritten, approximated or silently omitted. This is not authentication.

use super::{FramedPayload, InvalidPayload};
use crate::private_store::backup_candidate::ValidatedBackupStore;

// No formatting/serialization/clone: the exact plaintext remains private.
pub(crate) struct ValidatedPair<'a> {
    pub(crate) store: ValidatedBackupStore<'a>,
    pub(crate) template: &'a [u8],
}

fn bundled(preset: &str) -> Option<&'static str> {
    match preset {
        "roscomvpn-default" => Some(include_str!("catalog/v1/default.yaml")),
        "china-cn-direct" => Some(include_str!("catalog/v1/china.yaml")),
        "iran-ir-direct" => Some(include_str!("catalog/v1/iran.yaml")),
        _ => None,
    }
}

// V1 catalog recognition, not a renderer or YAML parser. Only this exact byte
// slot in immutable source snapshots varies; ordinary template/helper edits
// cannot redefine the accepted archive bytes. Never normalize the input.
fn matches_v1(expected: &str, input: &[u8]) -> bool {
    let Some((prefix, suffix)) = expected.split_once("\nmode: rule\n") else {
        return false;
    };
    if suffix.contains("\nmode: rule\n") {
        return false;
    }
    ["rule", "global", "direct"].iter().any(|mode| {
        prefix
            .bytes()
            .chain(b"\nmode: ".iter().copied())
            .chain(mode.bytes())
            .chain(*b"\n")
            .chain(suffix.bytes())
            .eq(input.iter().copied())
    })
}

impl<'a> FramedPayload<'a> {
    pub(crate) fn validate_bundled_pair(self) -> Result<ValidatedPair<'a>, InvalidPayload> {
        let store = self.validate_store()?;
        let expected = bundled(&store.routing_preset).ok_or(InvalidPayload)?;
        // Recognize trusted source bytes only. This is not a YAML security
        // parser or permission to carry arbitrary paths, providers or scripts.
        if !matches_v1(expected, self.template) {
            return Err(InvalidPayload);
        }
        Ok(ValidatedPair {
            store,
            template: self.template,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup_payload_candidate::{decode, encode};
    use serde_json::json;

    const PRESETS: [&str; 3] = ["roscomvpn-default", "china-cn-direct", "iran-ir-direct"];
    const MODES: [&str; 3] = ["rule", "global", "direct"];

    #[test]
    fn v1_snapshots_have_fixed_provenance_not_mutable_runtime_templates() {
        use sha2::{Digest, Sha256};
        for (preset, digest) in PRESETS.into_iter().zip([
            "1e5ad1aeb3149c73b1ae5dfffe21c9043b56662d220ae84c9af047a36c2c2f5c",
            "b2f027d9f46fa7557b0343e33e65490321560bdb4179ba92caf1d90f2c522e9e",
            "2cbe97008efd5859bfb20055472a15650e85375a270dcc8a57c31e3537fde4e8",
        ]) {
            assert_eq!(
                format!("{:x}", Sha256::digest(bundled(preset).unwrap())),
                digest
            );
        }
    }

    #[test]
    fn simulated_runtime_template_and_renderer_changes_do_not_redefine_v1() {
        for preset in PRESETS {
            let frozen = bundled(preset).unwrap();
            let future_template = format!("# future runtime template\n{frozen}");
            // Deliberately different hypothetical renderer. Neither it nor
            // the mutable source is consulted by archive recognition.
            let future_renderer = |text: &str, mode: &str| {
                text.replace("mode: rule\n", &format!("mode: {mode} # future\n"))
            };
            for mode in MODES {
                let legacy = template(preset, mode);
                for rejected in [
                    future_renderer(frozen, mode),
                    future_renderer(&future_template, mode),
                ] {
                    let wire = encode(store(preset).as_bytes(), rejected.as_bytes()).unwrap();
                    assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
                }
                let wire = encode(store(preset).as_bytes(), legacy.as_bytes()).unwrap();
                let pair = decode(&wire).unwrap().validate_bundled_pair().unwrap();
                assert_eq!(pair.template, legacy.as_bytes());
            }
            // Pre-fix historical proxy selection is not implicitly trusted
            // merely because those bytes once appeared in the repository.
            let obsolete = frozen.replace(
                "    include-all-proxies: true\n    exclude-type: Direct|Reject|Pass|Compatible\n  - name: GLOBAL\n    type: select\n    proxies:\n      - PROXY\n    default-selected: PROXY\n",
                "    include-all: true\n",
            );
            assert_ne!(obsolete, frozen);
            let wire = encode(store(preset).as_bytes(), obsolete.as_bytes()).unwrap();
            assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
        }
    }

    fn store(preset: &str) -> String {
        json!({"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"",
            "routingPreset":preset,"customRules":[],"rulesUpdatedAt":0,
            "startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},
            "startupConfigured":true,"onboardingComplete":false})
        .to_string()
    }

    fn template(preset: &str, mode: &str) -> String {
        // Independent byte replacement, not the implementation's mode helper.
        bundled(preset)
            .unwrap()
            .replace("\nmode: rule\n", &format!("\nmode: {mode}\n"))
    }

    #[test]
    fn whole_pair_accepts_three_presets_in_all_three_modes_without_rewriting() {
        for preset in PRESETS {
            for mode in MODES {
                let original_store = format!("\n  {}\n", store(preset));
                let original_template = template(preset, mode);
                let wire = encode(original_store.as_bytes(), original_template.as_bytes()).unwrap();
                let framed = decode(&wire).unwrap();
                let store_ptr = framed.store.as_ptr();
                let template_ptr = framed.template.as_ptr();
                let pair = framed.validate_bundled_pair().unwrap();
                assert!(pair.store.bytes == original_store.as_bytes());
                assert!(pair.template == original_template.as_bytes());
                assert!(std::ptr::eq(pair.store.bytes.as_ptr(), store_ptr));
                assert!(std::ptr::eq(pair.template.as_ptr(), template_ptr));
                assert_eq!(pair.store.profiles, 0);
                assert_eq!(pair.store.subscriptions, 0);
            }
        }
    }

    #[test]
    fn whole_pair_refuses_every_cross_preset_combination_in_all_modes() {
        for stored_preset in PRESETS {
            for template_preset in PRESETS {
                if stored_preset == template_preset {
                    continue;
                }
                for mode in MODES {
                    let wire = encode(
                        store(stored_preset).as_bytes(),
                        template(template_preset, mode).as_bytes(),
                    )
                    .unwrap();
                    assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
                }
            }
        }
    }

    #[test]
    fn whole_pair_refuses_custom_unconfigured_and_invalid_store_even_with_trusted_template() {
        let template = bundled(PRESETS[0]).unwrap().as_bytes();
        for input in [
            store("custom"),
            store(""),
            store("unknown"),
            "{}".into(),
            store(PRESETS[0]).replace("\"version\":3", "\"version\":4"),
        ] {
            let wire = encode(input.as_bytes(), template).unwrap();
            assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
        }
    }

    #[test]
    fn whole_pair_refuses_changed_templates_without_yaml_normalization_or_private_echo() {
        let expected = bundled(PRESETS[0]).unwrap();
        for altered in [
            format!("{expected}\n# synthetic-private-marker\n"),
            format!("{expected}\nexternal-controller: /synthetic/private.sock\n"),
            expected.replace("\nmode: rule\n", "\nmode: rule\nmode: global\n"),
            expected.replace("\nmode: rule\n", "\nmode: unknown\n"),
            expected.replace('\n', "\r\n"),
            "synthetic-private-marker".into(),
        ] {
            assert_ne!(altered, expected);
            let wire = encode(store(PRESETS[0]).as_bytes(), altered.as_bytes()).unwrap();
            let error = decode(&wire)
                .unwrap()
                .validate_bundled_pair()
                .err()
                .unwrap();
            assert_eq!(error.to_string(), "backup_unreadable");
            assert_eq!(format!("{error:?}"), "InvalidPayload");
        }
        let wire = encode(store(PRESETS[0]).as_bytes(), &[0xff]).unwrap();
        assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
    }

    #[test]
    fn whole_pair_preserves_store_custom_rules_and_startup_preferences_as_data_only() {
        let mut input: serde_json::Value = serde_json::from_str(&store(PRESETS[0])).unwrap();
        input["customRules"] = json!([{"id":"30000000-0000-4000-8000-000000000001",
            "kind":"domain","value":"example.invalid","action":"direct"}]);
        let original = input.to_string();
        let wire = encode(original.as_bytes(), bundled(PRESETS[0]).unwrap().as_bytes()).unwrap();
        let pair = decode(&wire).unwrap().validate_bundled_pair().unwrap();
        assert!(pair.store.bytes == original.as_bytes());
        // No custom rule is merged into template bytes during admission.
        assert!(pair.template == bundled(PRESETS[0]).unwrap().as_bytes());
    }
}
