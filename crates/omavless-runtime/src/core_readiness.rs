// SPDX-License-Identifier: MIT

//! Private, read-only admission for the generated native selector topology.
//! This is not a connectivity test or proof that remote rule providers fetched
//! successfully. Never format the expected profile name or controller payload.

use crate::desired::RoutingMode;
use omavless_mihomo::{ReadOnlyEndpoint, controller_get};
use serde_json::Value;
use std::path::Path;
use std::time::Instant;

pub(crate) struct ConfigReadiness {
    pub(crate) mode: RoutingMode,
    profile_name: String,
}

impl ConfigReadiness {
    pub(crate) fn restore_selection(&self, socket: &Path, pid: u32, deadline: Instant) -> bool {
        self.mode == RoutingMode::Global
            && crate::core_selector::restore_global(socket, pid, &self.profile_name, deadline)
    }

    pub(crate) fn new(mode: RoutingMode, profile_name: String) -> Self {
        Self { mode, profile_name }
    }

    fn matches(&self, endpoint: ReadOnlyEndpoint, payload: &Value) -> bool {
        match endpoint {
            ReadOnlyEndpoint::Configs => payload["mode"].as_str() == Some(self.mode.as_str()),
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

    pub(crate) fn ready(&self, socket: &Path, deadline: Instant) -> bool {
        for endpoint in [
            ReadOnlyEndpoint::Configs,
            ReadOnlyEndpoint::Rules,
            ReadOnlyEndpoint::RuleProviders,
            ReadOnlyEndpoint::Proxies,
        ] {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            let Ok(response) = controller_get(
                socket,
                endpoint,
                remaining,
                omavless_mihomo::MAX_CONTROLLER_RESPONSE_BYTES,
            ) else {
                return false;
            };
            if response.status != 200 || !self.matches(endpoint, &response.payload) {
                return false;
            }
        }
        Instant::now() < deadline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
