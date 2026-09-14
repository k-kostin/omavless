// SPDX-License-Identifier: MIT
//! Bounded shareable report, distinct from live rule/provider diagnostics.
use crate::desired::DesiredState;
use crate::lifecycle::{ActualState, NativeLocalObservation};
use crate::mutation_protocol::MutationProtocolError;
use omavless_control_protocol::validate_request;
use serde::Serialize;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Fixed-size shareable setup facts. None means unavailable, never healthy.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSupportFacts {
    core: Option<CoreFacts>,
    runtime_service: Option<ServiceFacts>,
    login_service: Option<ServiceFacts>,
    files: Option<FileFacts>,
    configured_policy: Option<PolicyFacts>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreFacts {
    installed: bool,
    file_network_capabilities: Option<bool>,
    tun_device_present: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceFacts {
    loaded: bool,
    active: bool,
    enabled: bool,
    owns_current_process: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileFacts {
    store: Option<bool>,
    template: Option<bool>,
    generated_config: Option<bool>,
    runtime_unit: Option<bool>,
    login_unit: Option<bool>,
}

#[derive(Serialize)]
struct PolicyFacts {
    basis: &'static str,
    rules: u32,
    providers: u32,
}

pub(crate) fn with_host(mut report: Value, host: Option<HostSupportFacts>) -> Value {
    let host = host.unwrap_or_default();
    report["schemaVersion"] = json!(3);
    report["coverage"]["coreSetupVerified"] =
        json!(host.core.as_ref().is_some_and(
            |c| c.file_network_capabilities.is_some() && c.tun_device_present.is_some()
        ));
    report["coverage"]["serviceEnablementVerified"] =
        json!(host.runtime_service.is_some() && host.login_service.is_some());
    report["coverage"]["fileReadiness"] = json!(host.files.as_ref().is_some_and(|f| {
        [
            f.store,
            f.template,
            f.generated_config,
            f.runtime_unit,
            f.login_unit,
        ]
        .iter()
        .all(Option::is_some)
    }));
    report["host"] = json!(host);
    report
}

// Reuses the existing bounded read-only child/pipe collector. These are the
// only two service names and the only properties this read can request.
fn service_facts(service: &str, runtime: bool) -> Option<ServiceFacts> {
    let mut command = Command::new("/usr/bin/systemctl");
    command.args([
        "--user",
        "show",
        service,
        "--no-pager",
        "--property=LoadState",
        "--property=ActiveState",
        "--property=UnitFileState",
        "--property=MainPID",
    ]);
    let raw =
        crate::production_observation::bounded_fixed_query(command, Duration::from_millis(250))
            .ok()?;
    parse_service_facts(&raw, runtime, std::process::id())
}

fn parse_service_facts(raw: &str, runtime: bool, current_pid: u32) -> Option<ServiceFacts> {
    let field = |name: &str| {
        let mut values = raw
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value);
        let result = values.next()?;
        if values.next().is_some() {
            None
        } else {
            Some(result)
        }
    };
    let loaded = match field("LoadState")? {
        "loaded" => true,
        "not-found" | "masked" => false,
        _ => return None,
    };
    let active = match field("ActiveState")? {
        "active" => true,
        "inactive" | "failed" => false,
        _ => return None, // transitional is not a stable support fact
    };
    let enabled = match field("UnitFileState")? {
        "enabled" | "enabled-runtime" => true,
        "disabled" | "static" | "indirect" | "masked" | "masked-runtime" | "" => false,
        _ => return None,
    };
    let pid: u32 = field("MainPID")?.parse().ok()?;
    if (!loaded && (active || enabled || pid != 0)) || (!active && pid != 0) {
        return None;
    }
    Some(ServiceFacts {
        loaded,
        active,
        enabled,
        owns_current_process: runtime.then_some(active && pid == current_pid),
    })
}

fn regular_file(path: &Path, uid: Option<u32>) -> Option<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) => Some(
            m.is_file()
                && !m.file_type().is_symlink()
                && uid.is_none_or(|uid| m.uid() == uid && m.permissions().mode() & 0o077 == 0),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    }
}

pub(crate) fn collect_host(
    paths: &crate::native_host::NativeHostPaths,
    uid: u32,
    connected: bool,
) -> HostSupportFacts {
    let installed = fs::symlink_metadata(&paths.core).is_ok_and(|m| {
        m.is_file() && !m.file_type().is_symlink() && m.permissions().mode() & 0o111 != 0
    });
    let capabilities = if installed {
        let mut command = Command::new("/usr/bin/getcap");
        command.arg(&paths.core);
        crate::production_observation::bounded_fixed_query(command, Duration::from_millis(250))
            .ok()
            .and_then(|output| {
                match crate::desktop_helpers::file_network_capabilities(output.as_bytes()) {
                    "present" => Some(true),
                    "missing" => Some(false),
                    _ => None,
                }
            })
    } else {
        Some(false)
    };
    let tun_device = match fs::symlink_metadata("/dev/net/tun") {
        Ok(m) => Some(
            m.file_type().is_char_device()
                && nix::sys::stat::major(m.rdev()) == 10
                && nix::sys::stat::minor(m.rdev()) == 200,
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    };
    let policy_path = if connected {
        &paths.active_config
    } else {
        &paths.template
    };
    let policy = if regular_file(policy_path, Some(uid)) == Some(true)
        && fs::metadata(policy_path)
            .is_ok_and(|m| m.len() <= omavless_domain::config::MAX_TEMPLATE_BYTES as u64)
    {
        omavless_store::read_private_utf8(policy_path, uid)
            .ok()
            .and_then(|text| policy_counts(&text, connected))
    } else {
        None
    };
    HostSupportFacts {
        core: Some(CoreFacts {
            installed,
            file_network_capabilities: capabilities,
            tun_device_present: tun_device,
        }),
        runtime_service: service_facts("omavless-runtime.service", true),
        login_service: service_facts("omavless-login-prepare.service", false),
        files: Some(FileFacts {
            store: regular_file(&paths.store, Some(uid)),
            template: regular_file(&paths.template, Some(uid)),
            generated_config: regular_file(&paths.active_config, Some(uid)),
            runtime_unit: regular_file(
                Path::new("/usr/lib/systemd/user/omavless-runtime.service"),
                None,
            ),
            login_unit: regular_file(
                Path::new("/usr/lib/systemd/user/omavless-login-prepare.service"),
                None,
            ),
        }),
        configured_policy: policy,
    }
}

// Reference: backend.routing_status's top-level block/sequence/mapping counts.
// These describe the private configuration file, not loaded or fetched rules.
fn policy_counts(text: &str, connected: bool) -> Option<PolicyFacts> {
    if text.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
        return None;
    }
    let mut section = "";
    let (mut rules, mut providers, mut provider_indent) = (0u32, 0u32, usize::MAX);
    let (mut seen_rules, mut seen_providers) = (false, false);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            section = "";
            if let Some((key, value)) = line.split_once(':')
                && matches!(key, "rules" | "rule-providers")
            {
                let seen = if key == "rules" {
                    &mut seen_rules
                } else {
                    &mut seen_providers
                };
                if *seen {
                    return None;
                }
                *seen = true;
                let value = value.trim();
                if !value.is_empty()
                    && !value.starts_with('#')
                    && value != (if key == "rules" { "[]" } else { "{}" })
                {
                    return None;
                }
                section = key;
            }
        } else if section == "rules" {
            if trimmed
                .strip_prefix('-')
                .is_some_and(|rest| rest.starts_with([' ', '\t']) && !rest.trim().is_empty())
            {
                rules += 1;
            }
        } else if section == "rule-providers"
            && !trimmed.starts_with('-')
            && let Some((key, value)) = trimmed.split_once(':')
        {
            if key.is_empty() {
                return None;
            }
            if !value.is_empty() && !value.starts_with(char::is_whitespace) {
                continue;
            }
            let indent = line
                .chars()
                .take_while(|c| matches!(c, ' ' | '\t'))
                .fold(0usize, |n, c| if c == '\t' { n + 2 - n % 2 } else { n + 1 });
            if indent < provider_indent {
                provider_indent = indent;
                providers = 1;
            } else if indent == provider_indent {
                providers += 1;
            }
        }
        if rules > 100_000 || providers > 1024 {
            return None;
        }
    }
    Some(PolicyFacts {
        basis: if connected {
            "active_config"
        } else {
            "template"
        },
        rules,
        providers,
    })
}

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "diagnostics.export" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn report(
    configuration: Value,
    desired: &DesiredState,
    actual: ActualState,
    pending: bool,
    observation: Option<NativeLocalObservation>,
) -> Value {
    // Saturated/inconsistent observations are unavailable, never rounded into
    // healthy counts. The same bounds are enforced by the presentation client.
    let observation = observation.filter(|o| {
        o.visible_mihomo_count <= 64
            && o.visible_tun_count <= 8
            && o.owned_auxiliary_mihomo_count <= 1
            && o.owned_auxiliary_mihomo_count <= o.visible_mihomo_count
            && (!o.desired_profile_matches_owned || (o.owned_core_running && desired.connected))
            && (!o.owned_controller_config_verified
                || (o.owned_core_running && o.desired_profile_matches_owned))
    });
    let actual = match actual {
        ActualState::Disconnected => "disconnected",
        ActualState::Starting => "starting",
        ActualState::Connected => "connected",
        ActualState::Reconnecting => "reconnecting",
        ActualState::Stopping => "stopping",
        ActualState::Failed => "failed",
        ActualState::ManualRecoveryRequired => "manual_recovery_required",
    };
    json!({
        "schemaVersion": 2,
        "scope": "native_support",
        "runtime": {
            "implementation": "rust",
            "version": env!("CARGO_PKG_VERSION"),
            "lastKnownState": actual,
            "routingTransactionPending": pending,
        },
        "configuration": configuration,
        "localObservation": {
            "availability": if observation.is_some() { "observed" } else { "unavailable" },
            "desired": { "connected": desired.connected, "mode": desired.mode.as_str() },
            "facts": observation.map(|o| json!({
                "ownedCoreRunning": o.owned_core_running,
                "visibleMihomoCount": o.visible_mihomo_count,
                "ownedAuxiliaryMihomoCount": o.owned_auxiliary_mihomo_count,
                "visibleTunCount": o.visible_tun_count,
                "ownedControllerConfigVerified": o.owned_controller_config_verified,
                "desiredProfileMatchesOwned": o.desired_profile_matches_owned,
            })),
            "verification": { "serviceOwnership": false, "tunOwnership": false,
                "routes": false, "dns": false, "internet": false },
        },
        "coverage": {
            "privateStoreValidated": true,
            "liveHostObservation": observation.is_some(),
            "controllerQuery": observation.is_some_and(|o| o.owned_controller_config_verified),
            "loginActivationVerified": false,
            "coreSetupVerified": false,
            "serviceEnablementVerified": false,
            "loadedPolicyCounts": false,
            "fileReadiness": false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_control_protocol::make_request;
    #[test]
    fn host_service_facts_are_typed_unknown_and_transition_safe() {
        let raw = "LoadState=loaded\nActiveState=active\nUnitFileState=enabled\nMainPID=42\n";
        let value = json!(parse_service_facts(raw, true, 42).unwrap());
        assert_eq!(
            value,
            json!({"loaded":true,"active":true,"enabled":true,"ownsCurrentProcess":true})
        );
        assert_eq!(
            json!(parse_service_facts(raw, true, 43).unwrap())["ownsCurrentProcess"],
            false
        );
        assert!(
            json!(parse_service_facts(raw, false, 42).unwrap())["ownsCurrentProcess"].is_null()
        );
        for bad in [
            raw.replace("active\n", "activating\n"),
            raw.replace("enabled", "private-token"),
            format!("{raw}MainPID=42\n"),
            raw.replace("loaded", "not-found"),
            raw.replace("active\n", "inactive\n"),
            raw.replace("MainPID=42\n", ""),
        ] {
            assert!(parse_service_facts(&bad, true, 42).is_none());
        }
        let absent = "LoadState=not-found\nActiveState=inactive\nUnitFileState=\nMainPID=0\n";
        assert_eq!(
            json!(parse_service_facts(absent, true, 42).unwrap())["loaded"],
            false
        );
    }

    #[test]
    fn host_policy_counts_are_configured_not_loaded_and_never_copy_payload() {
        let text = "# private-provider\nmode: rule\nrules:\n  - DOMAIN,private.invalid,PROXY\n  - MATCH,DIRECT\nrule-providers:\n  private-provider:\n    type: http\n    url: https://private.invalid/token\n  another:\n    payload:\n      - private.invalid\n";
        assert_eq!(
            json!(policy_counts(text, false).unwrap()),
            json!({"basis":"template","rules":2,"providers":2})
        );
        assert_eq!(
            json!(policy_counts(text, true).unwrap())["basis"],
            "active_config"
        );
        for invalid in [
            "rules:\nrules:\n",
            "rule-providers:\nrule-providers:\n",
            "rules: [private-token]\n",
            "rule-providers: {private: token}\n",
        ] {
            assert!(policy_counts(invalid, false).is_none());
        }
        assert_eq!(
            json!(policy_counts("rules: []\nrule-providers: {}\n", false).unwrap())["rules"],
            0
        );
        assert!(
            policy_counts(
                &"x".repeat(omavless_domain::config::MAX_TEMPLATE_BYTES + 1),
                false
            )
            .is_none()
        );
    }

    #[test]
    fn host_policy_counts_match_actual_python_reference_for_bundles_and_shapes() {
        use sha2::{Digest, Sha256};
        let cases = [
            include_str!("../../../templates/default.yaml"),
            include_str!("../../../templates/china.yaml"),
            include_str!("../../../templates/iran.yaml"),
            "rules:\n  - MATCH,DIRECT\n",
            "rules: []\nrule-providers: {}\n",
            "# comment\nrules:\n  # comment\n  - DOMAIN,example.invalid,DIRECT\nrule-providers:\n  public:\n    type: inline\n    payload:\n      - example.invalid\n",
        ];
        // Independently generated by the pinned Python archive, not by Rust.
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/parity_cases/support-policy-counts-v1.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["referenceCommit"],
            "aa5873783c019edc303a732e55ea8c85f1f0b090"
        );
        let expected = fixture["cases"].as_array().unwrap();
        assert_eq!(expected.len(), cases.len());
        for (case, expected) in cases.iter().zip(expected) {
            assert_eq!(
                format!("{:x}", Sha256::digest(case.as_bytes())),
                expected["sha256"]
            );
            let actual = policy_counts(case, false).unwrap();
            assert_eq!(json!([actual.rules, actual.providers]), expected["counts"]);
        }
    }

    #[test]
    fn host_file_readiness_refuses_symlinks_and_public_private_files() {
        let root = crate::test_temp::directory("support-file-facts").unwrap();
        let path = root.join("test");
        let uid = nix::unistd::Uid::current().as_raw();
        assert_eq!(regular_file(&path, Some(uid)), Some(false));
        fs::write(&path, b"synthetic").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(regular_file(&path, Some(uid)), Some(true));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(regular_file(&path, Some(uid)), Some(false));
        let link = root.join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert_eq!(regular_file(&link, None), Some(false));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn host_coverage_is_observation_not_success_or_network_verification() {
        let value = with_host(
            report(
                json!({}),
                &DesiredState::default(),
                ActualState::Disconnected,
                false,
                None,
            ),
            Some(HostSupportFacts {
                core: Some(CoreFacts {
                    installed: false,
                    file_network_capabilities: Some(false),
                    tun_device_present: Some(false),
                }),
                runtime_service: None,
                login_service: None,
                files: Some(FileFacts {
                    store: Some(false),
                    template: Some(false),
                    generated_config: None,
                    runtime_unit: Some(false),
                    login_unit: Some(false),
                }),
                configured_policy: None,
            }),
        );
        assert_eq!(value["schemaVersion"], 3);
        assert_eq!(value["coverage"]["coreSetupVerified"], true);
        assert_eq!(value["coverage"]["serviceEnablementVerified"], false);
        assert_eq!(value["coverage"]["fileReadiness"], false);
        assert_eq!(value["coverage"]["loadedPolicyCounts"], false);
        assert!(value["host"]["runtimeService"].is_null());
        assert!(
            value["localObservation"]["verification"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == false)
        );
        assert!(value.to_string().len() < 4096);
    }
    #[test]
    fn support_requests_reject_all_client_data_without_echo() {
        assert!(
            validate(&make_request("support", "diagnostics.export", json!({})).unwrap()).is_ok()
        );
        for params in [
            json!({"path":"private-token"}),
            json!({"raw":true}),
            json!({"operationId":"private-token"}),
            json!({"expectedRevision":0}),
        ] {
            let error = validate(&make_request("support", "diagnostics.export", params).unwrap())
                .unwrap_err();
            assert!(!format!("{error} {error:?}").contains("private-token"));
        }
        assert!(
            validate(&make_request("support", "diagnostics.summary", json!({})).unwrap()).is_err()
        );
    }

    #[test]
    fn support_state_is_explicitly_last_known_and_recovery_is_not_disconnected() {
        let store = omavless_domain::private_store::parse_private_store(
            r#"{"version":3,"profiles":[],"subscriptions":[]}"#,
        )
        .unwrap();
        let value = report(
            store.support_projection(),
            &DesiredState::default(),
            ActualState::ManualRecoveryRequired,
            true,
            None,
        );
        assert_eq!(
            value["runtime"]["lastKnownState"],
            "manual_recovery_required"
        );
        assert_eq!(value["runtime"]["routingTransactionPending"], true);
        assert_eq!(value["coverage"]["liveHostObservation"], false);
        assert!(value.to_string().len() < 4096);
    }

    #[test]
    fn support_observation_releases_only_typed_facts_not_profile_or_live_health() {
        let desired = DesiredState {
            connected: true,
            profile_id: "private-token".into(),
            ..DesiredState::default()
        };
        let observation = NativeLocalObservation {
            owned_core_running: true,
            visible_mihomo_count: 2,
            owned_auxiliary_mihomo_count: 1,
            visible_tun_count: 1,
            owned_controller_config_verified: true,
            desired_profile_matches_owned: true,
        };
        let store = omavless_domain::private_store::parse_private_store(
            r#"{"version":3,"profiles":[],"subscriptions":[]}"#,
        )
        .unwrap();
        let value = report(
            store.support_projection(),
            &desired,
            ActualState::Failed,
            false,
            Some(observation),
        );
        assert_eq!(value["localObservation"]["availability"], "observed");
        assert_eq!(value["runtime"]["lastKnownState"], "failed");
        assert_eq!(value["coverage"]["controllerQuery"], true);
        assert!(
            value["localObservation"]["verification"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == false)
        );
        for forbidden in [
            "private-token",
            "profileId",
            "instanceId",
            "generation",
            "192.0.2",
            "://",
        ] {
            assert!(!value.to_string().contains(forbidden));
        }
        assert!(value.to_string().len() < 4096);
    }

    #[test]
    fn inconsistent_and_saturated_support_facts_are_unavailable_not_rounded() {
        let mut cases = Vec::new();
        let empty = NativeLocalObservation {
            owned_core_running: false,
            visible_mihomo_count: 0,
            owned_auxiliary_mihomo_count: 0,
            visible_tun_count: 0,
            owned_controller_config_verified: false,
            desired_profile_matches_owned: false,
        };
        cases.push(NativeLocalObservation {
            visible_mihomo_count: 65,
            ..empty
        });
        cases.push(NativeLocalObservation {
            visible_tun_count: 9,
            ..empty
        });
        cases.push(NativeLocalObservation {
            owned_auxiliary_mihomo_count: 1,
            ..empty
        });
        cases.push(NativeLocalObservation {
            owned_controller_config_verified: true,
            ..empty
        });
        cases.push(NativeLocalObservation {
            desired_profile_matches_owned: true,
            ..empty
        });
        for observation in cases {
            let value = report(
                json!({}),
                &DesiredState::default(),
                ActualState::Disconnected,
                false,
                Some(observation),
            );
            assert_eq!(value["localObservation"]["availability"], "unavailable");
            assert!(value["localObservation"]["facts"].is_null());
            assert_eq!(value["coverage"]["liveHostObservation"], false);
        }
    }
}
