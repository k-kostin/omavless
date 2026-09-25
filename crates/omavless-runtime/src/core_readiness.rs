// SPDX-License-Identifier: MIT

//! Private, read-only admission for the generated native selector topology.
//! This is not a connectivity test or proof that remote rule providers fetched
//! successfully. Never format the expected profile name or controller payload.

use crate::desired::RoutingMode;
use omavless_mihomo::ReadOnlyEndpoint;
use serde_json::Value;
use std::path::Path;
use std::time::Instant;

pub(crate) struct ConfigReadiness {
    pub(crate) mode: RoutingMode,
    profile_name: String,
    managed_dns: bool,
}

impl ConfigReadiness {
    pub(crate) fn restore_selection(&self, socket: &Path, pid: u32, deadline: Instant) -> bool {
        self.mode == RoutingMode::Global
            && crate::core_selector::restore_global(socket, pid, &self.profile_name, deadline)
    }

    pub(crate) fn new(mode: RoutingMode, profile_name: String) -> Self {
        Self {
            mode,
            profile_name,
            managed_dns: false,
        }
    }

    /// The opt-in flags request broker use; they do not grant authority. Only
    /// administrator enrollment and the root broker can do that. Restrict this
    /// experimental spelling to the canonical block form, not a second YAML
    /// parser. The reviewed core separately validates the actual DNS/TUN policy.
    pub(crate) fn from_generated_config(
        mode: RoutingMode,
        profile_name: String,
        config: &str,
    ) -> Option<Self> {
        let mut expected = Self::new(mode, profile_name);
        if config.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
            return None;
        }
        let flags = ["omavless-dns-broker", "disable-system-dns"];
        let mut tun_count = 0;
        let mut in_tun = false;
        let mut found = [false; 2];
        let mut requested = false;
        for line in config.lines() {
            if !line.starts_with([' ', '\t']) && !line.trim().is_empty() && !line.starts_with('#') {
                in_tun = line == "tun:";
                if in_tun {
                    tun_count += 1;
                }
            }
            let Some(position) = dns_flag_key(line) else {
                continue;
            };
            requested = true;
            if position >= flags.len()
                || found[position]
                || !in_tun
                || line != format!("  {}: true", flags[position])
            {
                return None;
            }
            found[position] = true;
        }
        if !requested {
            return Some(expected);
        }
        if tun_count != 1 || !found.into_iter().all(|value| value) {
            return None;
        }
        expected.managed_dns = true;
        Some(expected)
    }

    pub(crate) fn startup_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(if self.managed_dns { 45 } else { 10 })
    }

    pub(crate) fn stop_timeout(&self) -> std::time::Duration {
        // OwnedCore allows 80% grace before SIGKILL: 55s gives 44s, beyond the
        // core's bounded 40s broker release (broker idle5s + release30s).
        std::time::Duration::from_secs(if self.managed_dns { 55 } else { 5 })
    }

    fn matches(&self, endpoint: ReadOnlyEndpoint, payload: &Value) -> bool {
        match endpoint {
            ReadOnlyEndpoint::Configs => {
                let tun = &payload["tun"];
                let dns_ready = if self.managed_dns {
                    tun["enable"].as_bool() == Some(true)
                        && tun["omavless-dns-broker"].as_bool() == Some(true)
                        && tun["disable-system-dns"].as_bool() == Some(true)
                        && tun["omavless-dns-ready"].as_bool() == Some(true)
                } else {
                    // Unexpected ownership changes must not silently be adopted,
                    // even if YAML spelling evaded the conservative flag detector.
                    [
                        "omavless-dns-broker",
                        "disable-system-dns",
                        "omavless-dns-ready",
                    ]
                    .iter()
                    .all(|key| tun[*key].is_null() || tun[*key].as_bool() == Some(false))
                };
                payload["mode"].as_str() == Some(self.mode.as_str()) && dns_ready
            }
            ReadOnlyEndpoint::Proxies => {
                let Some(proxies) = payload["proxies"].as_object() else {
                    return false;
                };
                let selector = |name: &str, target: &str| {
                    proxies.get(name).is_some_and(|group| {
                        group["type"] == "Selector"
                            && group["now"].as_str() == Some(target)
                            && group["all"].as_array().is_some_and(|members| {
                                members.iter().any(|member| member.as_str() == Some(target))
                            })
                    })
                };
                proxies
                    .get(&self.profile_name)
                    .is_some_and(Value::is_object)
                    && ((!proxies.contains_key("PROXY") && self.mode != RoutingMode::Global)
                        || selector("PROXY", &self.profile_name))
                    && (self.mode != RoutingMode::Global || selector("GLOBAL", "PROXY"))
            }
            // Empty rules/providers are legal for custom templates. Null is
            // not a loaded collection. Never hard-code fixture row counts.
            // Provider *availability* remains a separate diagnostic boundary.
            ReadOnlyEndpoint::Rules => payload["rules"].is_array(),
            ReadOnlyEndpoint::RuleProviders => payload["providers"].is_object(),
            _ => false,
        }
    }

    pub(crate) fn ready_for_pid(&self, socket: &Path, pid: u32, deadline: Instant) -> bool {
        self.ready_with(deadline, |endpoint| {
            crate::core_selector::read_configuration(socket, pid, endpoint, deadline)
        })
    }

    fn ready_with(
        &self,
        deadline: Instant,
        mut read: impl FnMut(ReadOnlyEndpoint) -> Option<Value>,
    ) -> bool {
        for endpoint in [
            ReadOnlyEndpoint::Configs,
            ReadOnlyEndpoint::Rules,
            ReadOnlyEndpoint::RuleProviders,
            ReadOnlyEndpoint::Proxies,
        ] {
            if Instant::now() >= deadline {
                return false;
            }
            let Some(payload) = read(endpoint) else {
                return false;
            };
            if !self.matches(endpoint, &payload) {
                return false;
            }
        }
        Instant::now() < deadline
    }
}

/// Only key positions are opt-in requests. Provider/profile scalar text and
/// comments are not policy. This recognizes JSON/single-quoted spellings so
/// they can be refused, not accepted as an alternate managed syntax. Exotic
/// YAML evasions cannot authorize anything; actual controller flags must still
/// match the expected legacy/managed policy through the authenticated transport.
fn dns_flag_key(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let (key, _) = trimmed
        .strip_prefix("- ")
        .unwrap_or(trimmed)
        .split_once(':')?;
    let key = key.trim_end();
    let decoded;
    let key = if key.starts_with('"') {
        decoded = serde_json::from_str::<String>(key).ok()?;
        decoded.as_str()
    } else if key.starts_with('\'') && key.ends_with('\'') && key.len() >= 2 {
        &key[1..key.len() - 1]
    } else {
        key
    };
    [
        "omavless-dns-broker",
        "disable-system-dns",
        "omavless-dns-ready",
    ]
    .iter()
    .position(|expected| *expected == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MANAGED: &str = "tun:\n  enable: true\n  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\ndns:\n  enable: true\n";

    #[test]
    fn managed_request_requires_both_canonical_flags_in_one_tun_section() {
        let expected =
            ConfigReadiness::from_generated_config(RoutingMode::Rule, "Synthetic".into(), MANAGED)
                .unwrap();
        assert!(expected.managed_dns);
        assert_eq!(expected.startup_timeout().as_secs(), 45);
        assert_eq!(expected.stop_timeout().as_secs(), 55);
        assert!(expected.stop_timeout().mul_f32(0.8) > std::time::Duration::from_secs(40));
        for invalid in [
            MANAGED.replace("  omavless-dns-broker: true\n", ""),
            MANAGED.replace("  disable-system-dns: true\n", ""),
            MANAGED.replace("disable-system-dns: true", "disable-system-dns: false"),
            MANAGED.replace("omavless-dns-broker: true", "omavless-dns-broker: yes"),
            MANAGED.replace("omavless-dns-broker: true", "omavless-dns-broker: \"true\""),
            MANAGED.replace(
                "omavless-dns-broker: true",
                "omavless-dns-broker: true # comment",
            ),
            format!("{MANAGED}tun:\n  enable: true\n"),
            format!("{MANAGED}  omavless-dns-broker: true\n"),
            MANAGED.replace("tun:", "other:"),
            format!("{MANAGED}omavless-dns-ready: true\n"),
            MANAGED.replace("omavless-dns-broker:", "\"omavless-dns-broker\":"),
            MANAGED.replace("omavless-dns-broker:", "'omavless-dns-broker':"),
            MANAGED.replace("omavless-dns-broker:", "\"\\u006fmavless-dns-broker\":"),
        ] {
            assert!(
                ConfigReadiness::from_generated_config(
                    RoutingMode::Rule,
                    "Synthetic".into(),
                    &invalid
                )
                .is_none()
            );
        }
    }

    #[test]
    fn reserved_words_in_profile_values_and_comments_do_not_enable_dns_policy() {
        let text = "# omavless-dns-broker: true\ntun:\n  enable: true\nproxies:\n  - name: omavless-dns\n    server: disable-system-dns.example.invalid\n  - name: \"omavless-dns-broker: true\"\n  - name: omavless-dns-ready\n";
        let legacy =
            ConfigReadiness::from_generated_config(RoutingMode::Rule, "Synthetic".into(), text)
                .unwrap();
        assert!(!legacy.managed_dns);
        assert_eq!(legacy.stop_timeout().as_secs(), 5);
        let managed = ConfigReadiness::from_generated_config(
            RoutingMode::Rule,
            "Synthetic".into(),
            &format!(
                "{MANAGED}# disable-system-dns: false\nproxies:\n  - name: omavless-dns-broker\n"
            ),
        )
        .unwrap();
        assert!(managed.managed_dns);
        for key in [
            "omavless-dns-broker",
            "disable-system-dns",
            "omavless-dns-ready",
        ] {
            assert!(
                ConfigReadiness::from_generated_config(
                    RoutingMode::Rule,
                    "Synthetic".into(),
                    &format!("proxies:\n  {key}: true\n")
                )
                .is_none()
            );
        }
    }

    #[test]
    fn managed_readiness_requires_strict_actual_ready_not_only_tun_or_version() {
        let expected = ConfigReadiness::from_generated_config(
            RoutingMode::Global,
            "Synthetic".into(),
            MANAGED,
        )
        .unwrap();
        let ready = json!({"mode":"global","tun":{"enable":true,"disable-system-dns":true,
            "omavless-dns-broker":true,"omavless-dns-ready":true}});
        assert!(expected.matches(ReadOnlyEndpoint::Configs, &ready));
        for key in [
            "enable",
            "disable-system-dns",
            "omavless-dns-broker",
            "omavless-dns-ready",
        ] {
            for value in [Value::Null, json!(false), json!(1), json!("true")] {
                let mut bad = ready.clone();
                bad["tun"][key] = value;
                assert!(!expected.matches(ReadOnlyEndpoint::Configs, &bad));
            }
        }
        assert!(!expected.matches(
            ReadOnlyEndpoint::Configs,
            &json!({"mode":"global","tun":{"enable":true}})
        ));
    }

    #[test]
    fn legacy_never_adopts_unexpected_managed_or_dns_disabled_core() {
        let expected = ConfigReadiness::from_generated_config(
            RoutingMode::Rule,
            "Synthetic".into(),
            "tun:\n  enable: true\n",
        )
        .unwrap();
        assert!(!expected.managed_dns);
        assert_eq!(expected.startup_timeout().as_secs(), 10);
        assert_eq!(expected.stop_timeout().as_secs(), 5);
        assert!(expected.matches(ReadOnlyEndpoint::Configs, &json!({"mode":"rule"})));
        for key in [
            "disable-system-dns",
            "omavless-dns-broker",
            "omavless-dns-ready",
        ] {
            for value in [json!(true), json!(1), json!("false")] {
                let mut bad = json!({"mode":"rule","tun":{}});
                bad["tun"][key] = value;
                assert!(!expected.matches(ReadOnlyEndpoint::Configs, &bad));
            }
        }
    }

    fn proxies() -> Value {
        json!({"proxies": {
            "Synthetic": {"type": "Vless"},
            "PROXY": {"type": "Selector", "now": "Synthetic", "all": ["Synthetic"]},
            "GLOBAL": {"type": "Selector", "now": "PROXY", "all": ["PROXY"]}
        }})
    }

    #[test]
    fn requires_actual_mode_and_selected_member_not_just_version() {
        for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
            let expected = ConfigReadiness::new(mode, "Synthetic".into());
            assert!(!expected.matches(ReadOnlyEndpoint::Version, &json!({"version":"ready"})));
            assert!(expected.matches(ReadOnlyEndpoint::Configs, &json!({"mode":mode.as_str()})));
            assert!(!expected.matches(ReadOnlyEndpoint::Configs, &json!({"mode":"unknown"})));
            assert!(expected.matches(ReadOnlyEndpoint::Proxies, &proxies()));
            for field in ["now", "all", "type"] {
                let mut wrong = proxies();
                wrong["proxies"]["PROXY"][field] = Value::Null;
                assert!(!expected.matches(ReadOnlyEndpoint::Proxies, &wrong));
            }
            let mut missing = proxies();
            missing["proxies"]
                .as_object_mut()
                .unwrap()
                .remove("Synthetic");
            assert!(!expected.matches(ReadOnlyEndpoint::Proxies, &missing));
        }
    }

    #[test]
    fn custom_rule_or_direct_template_need_not_have_a_proxy_selector() {
        let payload = json!({"proxies":{"Synthetic":{"type":"Vless"}}});
        for mode in [RoutingMode::Rule, RoutingMode::Direct] {
            assert!(
                ConfigReadiness::new(mode, "Synthetic".into())
                    .matches(ReadOnlyEndpoint::Proxies, &payload)
            );
        }
        assert!(
            !ConfigReadiness::new(RoutingMode::Global, "Synthetic".into())
                .matches(ReadOnlyEndpoint::Proxies, &payload)
        );
    }

    #[test]
    fn global_requires_nested_selector_and_rejects_cached_direct_selection() {
        let expected = ConfigReadiness::new(RoutingMode::Global, "Synthetic".into());
        let mut wrong = proxies();
        wrong["proxies"]["GLOBAL"]["now"] = json!("DIRECT");
        assert!(!expected.matches(ReadOnlyEndpoint::Proxies, &wrong));
        wrong["proxies"]["GLOBAL"]["now"] = json!("PROXY");
        wrong["proxies"]["GLOBAL"]["all"] = json!(["DIRECT"]);
        assert!(!expected.matches(ReadOnlyEndpoint::Proxies, &wrong));
    }

    #[test]
    fn null_collections_fail_but_empty_custom_rules_and_providers_are_legal() {
        let expected = ConfigReadiness::new(RoutingMode::Rule, "Synthetic".into());
        for payload in [json!({}), json!({"rules":null})] {
            assert!(!expected.matches(ReadOnlyEndpoint::Rules, &payload));
        }
        assert!(expected.matches(
            ReadOnlyEndpoint::Rules,
            &json!({"rules":[{"type":"Match"}]})
        ));
        assert!(expected.matches(ReadOnlyEndpoint::Rules, &json!({"rules":[]})));
        assert!(!expected.matches(ReadOnlyEndpoint::RuleProviders, &json!({"providers":null})));
        assert!(expected.matches(ReadOnlyEndpoint::RuleProviders, &json!({"providers":{}})));
    }
}
