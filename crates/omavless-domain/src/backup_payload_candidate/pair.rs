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
        "roscomvpn-default" => Some(include_str!("../../../../templates/default.yaml")),
        "china-cn-direct" => Some(include_str!("../../../../templates/china.yaml")),
        "iran-ir-direct" => Some(include_str!("../../../../templates/iran.yaml")),
        _ => None,
    }
}

// Only the normal managed-selection producer's exact default transformation.
// The argument is a trusted bundled mode candidate, never incoming YAML.
fn managed_default(candidate: &str) -> Option<String> {
    const ANCHOR: &str = "  device: Meta\n";
    if candidate.matches(ANCHOR).count() != 1 {
        return None;
    }
    Some(candidate.replacen(
        ANCHOR,
        "  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\n",
        1,
    ))
}

impl<'a> FramedPayload<'a> {
    pub(crate) fn validate_bundled_pair(self) -> Result<ValidatedPair<'a>, InvalidPayload> {
        let store = self.validate_store()?;
        let expected = bundled(&store.routing_preset).ok_or(InvalidPayload)?;
        // Recognize trusted source bytes only. This is not a YAML security
        // parser or permission to carry arbitrary paths, providers or scripts.
        let matches = ["rule", "global", "direct"].iter().any(|mode| {
            crate::routing::template_with_mode(expected, mode).is_ok_and(|candidate| {
                candidate.as_bytes() == self.template
                    || (store.routing_preset == "roscomvpn-default"
                        && managed_default(&candidate)
                            .is_some_and(|managed| managed.as_bytes() == self.template))
            })
        });
        if !matches {
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

    fn managed_template(mode: &str) -> String {
        // Independent exact spelling of the normal default producer.
        template(PRESETS[0], mode).replace(
            "  device: Meta\n",
            "  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\n",
        )
    }

    #[test]
    fn twelve_trusted_templates_preserve_original_borrowed_pair() {
        let mut accepted = 0;
        for preset in PRESETS {
            for mode in MODES {
                let mut variants = vec![template(preset, mode)];
                if preset == PRESETS[0] {
                    variants.push(managed_template(mode));
                }
                for original_template in variants {
                    let original_store = format!("\n  {}\n", store(preset));
                    let wire =
                        encode(original_store.as_bytes(), original_template.as_bytes()).unwrap();
                    let framed = decode(&wire).unwrap();
                    let pointers = (framed.store.as_ptr(), framed.template.as_ptr());
                    let pair = framed.validate_bundled_pair().unwrap();
                    assert_eq!(pair.store.bytes, original_store.as_bytes());
                    assert_eq!(pair.template, original_template.as_bytes());
                    assert_eq!(pair.store.bytes.as_ptr(), pointers.0);
                    assert_eq!(pair.template.as_ptr(), pointers.1);
                    accepted += 1;
                }
            }
        }
        assert_eq!(accepted, 12);
    }

    #[test]
    fn managed_default_is_not_cross_preset_or_unknown_custom_authority() {
        for mode in MODES {
            let managed = managed_template(mode);
            for preset in [PRESETS[1], PRESETS[2], "custom", "unknown"] {
                let wire = encode(store(preset).as_bytes(), managed.as_bytes()).unwrap();
                assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
            }
            for preset in [PRESETS[1], PRESETS[2]] {
                let altered = template(preset, mode).replace(
                    "  device: Meta\n",
                    "  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\n",
                );
                let wire = encode(store(preset).as_bytes(), altered.as_bytes()).unwrap();
                assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
            }
        }
    }

    #[test]
    fn managed_template_requires_exact_original_producer_bytes() {
        let expected = managed_template("rule");
        for altered in [
            expected.replace("  disable-system-dns: true\n", ""),
            expected.replace("  omavless-dns-broker: true\n", ""),
            expected.replace("disable-system-dns: true", "disable-system-dns: false"),
            expected.replace("omavless-dns-broker: true", "omavless-dns-broker: false"),
            expected.replace(
                "  disable-system-dns: true\n",
                "  disable-system-dns: true\n  disable-system-dns: true\n",
            ),
            expected.replace(
                "  disable-system-dns: true\n  omavless-dns-broker: true\n",
                "  omavless-dns-broker: true\n  disable-system-dns: true\n",
            ),
            expected.replace("  device: Meta\n", "  device: omavless0\n"),
            expected.replace("  device: Meta\n", "  device: Meta \n"),
            expected.replace(
                "  disable-system-dns: true\n",
                "  disable-system-dns: true # comment\n",
            ),
            expected.replace('\n', "\r\n"),
            format!("{expected}\nexternal-controller: /synthetic/private.sock\n"),
            format!("{expected}\nproxy-providers: {{}}\n"),
            format!("{expected}\nscript: synthetic-private\n"),
        ] {
            assert_ne!(altered, expected);
            let wire = encode(store(PRESETS[0]).as_bytes(), altered.as_bytes()).unwrap();
            assert!(decode(&wire).unwrap().validate_bundled_pair().is_err());
        }
    }

    #[test]
    fn managed_pair_preserves_custom_rules_and_startup_as_data() {
        let mut original: serde_json::Value = serde_json::from_str(&store(PRESETS[0])).unwrap();
        let profile = "10000000-0000-4000-8000-000000000001";
        original["profiles"] = json!([{"id":profile,"name":"Synthetic",
            "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic",
            "protocol":"vless","favorite":false}]);
        original["lastId"] = json!(profile);
        original["customRules"] = json!([{"id":"30000000-0000-4000-8000-000000000001",
            "kind":"domain","value":"example.invalid","action":"direct"}]);
        original["startup"] =
            json!({"enabled":true,"target":"last","profileId":"","mode":"global"});
        let raw = original.to_string();
        let managed = managed_template("direct");
        let wire = encode(raw.as_bytes(), managed.as_bytes()).unwrap();
        let pair = decode(&wire).unwrap().validate_bundled_pair().unwrap();
        assert_eq!(pair.store.bytes, raw.as_bytes());
        assert_eq!(pair.template, managed.as_bytes());
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
