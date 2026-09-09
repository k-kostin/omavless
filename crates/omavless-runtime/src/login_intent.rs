// SPDX-License-Identifier: MIT

//! Pure first-login intent planning, separate from daemon restart recovery.
//!
//! This module does not determine whether a login is fresh. Before applying a
//! plan the future host must prove exact native ownership, hold the migration
//! and owner locks, validate the once-per-user-manager trigger, prove an empty
//! core/controller/TUN observation, validate generated config/host readiness,
//! and revalidate its desired/store snapshots. None of those proofs is supplied
//! by this pure function or by constructing `LoginTrigger`.
//!
//! A no-op desired plan does NOT mean the future host may skip consuming its
//! first-login trigger. Trigger consumption and desired persistence are separate
//! responsibilities. No service, CLI, IPC, filesystem or lifecycle effects are
//! implemented here, and the current Python owner is unchanged.

use crate::desired::{DesiredState, MAX_GENERATION, RoutingMode};
use omavless_domain::private_store::PrivateStore;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginTrigger {
    FirstLogin,
    DaemonRestart,
    AlreadyConsumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginIntentError {
    InvalidDesiredState,
    LegacyResolutionRequired,
    InvalidStartupSelection,
    GenerationExhausted,
}

impl fmt::Display for LoginIntentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDesiredState => "OmaVLESS desired state is invalid",
            Self::LegacyResolutionRequired => "OmaVLESS legacy login settings require resolution",
            Self::InvalidStartupSelection => "OmaVLESS login selection is unavailable",
            Self::GenerationExhausted => "OmaVLESS desired generation is exhausted",
        })
    }
}

impl std::error::Error for LoginIntentError {}

/// Return `None` to preserve the exact valid durable desired state without a
/// write, or a replacement with exactly one checked generation increment.
///
/// Restart/already-consumed paths never reapply login preferences, including
/// ambiguous legacy preferences. Fresh explicitly-disabled login clears old
/// connected intent while preserving the current routing mode. A valid enabled
/// choice uses the canonical store's last/explicit selection rules.
///
/// `PrivateStore` is already normalized: removed explicit selections and empty
/// last-selection stores are disabled by its accepted Python-parity contract.
/// Respect that normalized disabled state; do not resurrect or guess a profile.
/// Profile IDs in a returned desired state are private, never diagnostic text.
pub fn plan_login_intent(
    trigger: LoginTrigger,
    current: &DesiredState,
    store: &PrivateStore,
) -> Result<Option<DesiredState>, LoginIntentError> {
    current
        .validate()
        .map_err(|_| LoginIntentError::InvalidDesiredState)?;
    if trigger != LoginTrigger::FirstLogin {
        return Ok(None);
    }
    if !store.startup_is_configured() {
        // Python historically derives this case from existing user-unit state.
        // A pure planner must not equate an unresolved legacy choice to false.
        return Err(LoginIntentError::LegacyResolutionRequired);
    }
    let preferences = store.startup_preferences();
    let mut next = current.clone();
    if preferences.enabled {
        next.profile_id = store
            .resolve_startup_selection(&preferences)
            .map_err(|_| LoginIntentError::InvalidStartupSelection)?;
        next.mode = match preferences.mode.as_str() {
            "rule" => RoutingMode::Rule,
            "global" => RoutingMode::Global,
            _ => return Err(LoginIntentError::InvalidStartupSelection),
        };
        next.connected = true;
    } else {
        next.connected = false;
        next.profile_id.clear();
    }
    if next == *current {
        return Ok(None);
    }
    next.generation = current
        .generation
        .checked_add(1)
        .filter(|generation| *generation <= MAX_GENERATION)
        .ok_or(LoginIntentError::GenerationExhausted)?;
    next.validate()
        .map_err(|_| LoginIntentError::InvalidDesiredState)?;
    Ok(Some(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::MAX_PROFILE_ID_BYTES;
    use omavless_domain::private_store::parse_private_store;
    use serde_json::{Value, json};

    const FIRST: &str = "00000000-0000-4000-8000-000000000001";
    const SECOND: &str = "00000000-0000-4000-8000-000000000002";

    fn document() -> Value {
        json!({
            "version": 3, "activeId": "", "lastId": SECOND,
            "profiles": [
                {"id":FIRST,"name":"Synthetic first","protocol":"vless",
                 "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp",
                 "subscriptionId":"","subscriptionKey":"","favorite":false,"missing":false},
                {"id":SECOND,"name":"Synthetic second","protocol":"vless",
                 "uri":"vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp",
                 "subscriptionId":"","subscriptionKey":"","favorite":false,"missing":false}
            ],
            "subscriptions": [], "routingPreset": "custom", "customRules": [],
            "startupConfigured": true,
            "startup": {"enabled":true,"target":"last","profileId":"","mode":"global"}
        })
    }

    fn store(document: &Value) -> PrivateStore {
        parse_private_store(&document.to_string()).expect("valid synthetic store")
    }

    fn desired(connected: bool, mode: RoutingMode, generation: u64) -> DesiredState {
        DesiredState {
            connected,
            profile_id: if connected {
                SECOND.into()
            } else {
                String::new()
            },
            mode,
            generation,
            ..DesiredState::default()
        }
    }

    #[test]
    fn restart_and_consumed_never_reapply_preferences_or_legacy_ambiguity() {
        for trigger in [LoginTrigger::DaemonRestart, LoginTrigger::AlreadyConsumed] {
            for configured in [false, true] {
                for enabled in [false, true] {
                    let mut source = document();
                    source["startupConfigured"] = json!(configured);
                    source["startup"]["enabled"] = json!(enabled);
                    let store = store(&source);
                    for connected in [false, true] {
                        for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
                            let current = desired(connected, mode, MAX_GENERATION);
                            let original = current.clone();
                            assert!(
                                plan_login_intent(trigger, &current, &store)
                                    .unwrap()
                                    .is_none()
                            );
                            assert!(current == original);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn first_login_refuses_unresolved_legacy_even_when_stored_enabled_is_false() {
        for enabled in [false, true] {
            let mut source = document();
            source["startupConfigured"] = json!(false);
            source["startup"]["enabled"] = json!(enabled);
            let error = plan_login_intent(
                LoginTrigger::FirstLogin,
                &DesiredState::default(),
                &store(&source),
            )
            .unwrap_err();
            assert_eq!(error, LoginIntentError::LegacyResolutionRequired);
        }
    }

    #[test]
    fn disabled_first_login_clears_previous_session_intent_preserving_mode() {
        let mut source = document();
        source["startup"]["enabled"] = json!(false);
        let store = store(&source);
        for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
            let next = plan_login_intent(LoginTrigger::FirstLogin, &desired(true, mode, 7), &store)
                .unwrap()
                .unwrap();
            assert!(!next.connected && next.profile_id.is_empty());
            assert_eq!(next.mode, mode);
            assert_eq!(next.generation, 8);
            assert!(
                plan_login_intent(
                    LoginTrigger::FirstLogin,
                    &desired(false, mode, MAX_GENERATION),
                    &store
                )
                .unwrap()
                .is_none()
            );
        }
    }

    #[test]
    fn enabled_first_login_uses_canonical_last_or_explicit_selection_and_mode() {
        for (target, requested_id, expected_id) in [("last", "", SECOND), ("profile", FIRST, FIRST)]
        {
            for (mode, expected_mode) in
                [("rule", RoutingMode::Rule), ("global", RoutingMode::Global)]
            {
                let mut source = document();
                source["startup"]["target"] = json!(target);
                source["startup"]["profileId"] = json!(requested_id);
                source["startup"]["mode"] = json!(mode);
                let next = plan_login_intent(
                    LoginTrigger::FirstLogin,
                    &desired(false, RoutingMode::Direct, 7),
                    &store(&source),
                )
                .unwrap()
                .unwrap();
                assert!(next.connected && next.profile_id == expected_id);
                assert_eq!(next.mode, expected_mode);
                assert_eq!(next.generation, 8);
            }
        }
    }

    #[test]
    fn empty_or_stale_last_pointer_uses_canonical_first_profile_fallback() {
        for last in ["", "00000000-0000-4000-8000-000000000003"] {
            let mut source = document();
            source["lastId"] = json!(last);
            let next = plan_login_intent(
                LoginTrigger::FirstLogin,
                &DesiredState::default(),
                &store(&source),
            )
            .unwrap()
            .unwrap();
            assert!(next.profile_id == FIRST);
        }
    }

    #[test]
    fn removed_selection_and_empty_store_respect_canonical_disabled_normalization() {
        let mut removed = document();
        removed["startup"]["target"] = json!("profile");
        removed["startup"]["profileId"] = json!("00000000-0000-4000-8000-000000000003");
        let mut empty = document();
        empty["profiles"] = json!([]);
        for source in [removed, empty] {
            let store = store(&source);
            assert!(!store.startup_preferences().enabled);
            let next = plan_login_intent(
                LoginTrigger::FirstLogin,
                &desired(true, RoutingMode::Global, 3),
                &store,
            )
            .unwrap()
            .unwrap();
            assert!(!next.connected && next.profile_id.is_empty());
            assert_eq!(next.generation, 4);
        }
    }

    #[test]
    fn rule_without_preset_is_refused_but_global_does_not_require_one() {
        let mut source = document();
        source["routingPreset"] = json!("");
        source["startup"]["mode"] = json!("rule");
        assert_eq!(
            plan_login_intent(
                LoginTrigger::FirstLogin,
                &DesiredState::default(),
                &store(&source)
            )
            .unwrap_err(),
            LoginIntentError::InvalidStartupSelection
        );
        source["startup"]["mode"] = json!("global");
        assert!(
            plan_login_intent(
                LoginTrigger::FirstLogin,
                &DesiredState::default(),
                &store(&source)
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_desired_state_is_never_preserved_or_repaired_implicitly() {
        let store = store(&document());
        let mut invalid = vec![
            DesiredState {
                schema_version: 2,
                ..DesiredState::default()
            },
            desired(false, RoutingMode::Rule, MAX_GENERATION + 1),
            DesiredState {
                profile_id: "not-empty".into(),
                ..DesiredState::default()
            },
        ];
        for id in [
            String::new(),
            "x".repeat(MAX_PROFILE_ID_BYTES + 1),
            "line\nbreak".into(),
        ] {
            invalid.push(DesiredState {
                connected: true,
                profile_id: id,
                ..DesiredState::default()
            });
        }
        for current in invalid {
            for trigger in [
                LoginTrigger::FirstLogin,
                LoginTrigger::DaemonRestart,
                LoginTrigger::AlreadyConsumed,
            ] {
                assert_eq!(
                    plan_login_intent(trigger, &current, &store).unwrap_err(),
                    LoginIntentError::InvalidDesiredState
                );
            }
        }
        let maximum_id = DesiredState {
            connected: true,
            profile_id: "x".repeat(MAX_PROFILE_ID_BYTES),
            ..DesiredState::default()
        };
        assert!(
            plan_login_intent(LoginTrigger::DaemonRestart, &maximum_id, &store)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn checked_generation_increment_allows_maximum_and_identical_noop() {
        let store = store(&document());
        let before = desired(false, RoutingMode::Global, MAX_GENERATION - 1);
        let maximum = plan_login_intent(LoginTrigger::FirstLogin, &before, &store)
            .unwrap()
            .unwrap();
        assert_eq!(maximum.generation, MAX_GENERATION);
        assert!(
            plan_login_intent(LoginTrigger::FirstLogin, &maximum, &store)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            plan_login_intent(
                LoginTrigger::FirstLogin,
                &desired(false, RoutingMode::Global, MAX_GENERATION),
                &store
            )
            .unwrap_err(),
            LoginIntentError::GenerationExhausted
        );
    }

    #[test]
    fn public_errors_are_fixed_bounded_and_do_not_embed_private_state() {
        for error in [
            LoginIntentError::InvalidDesiredState,
            LoginIntentError::LegacyResolutionRequired,
            LoginIntentError::InvalidStartupSelection,
            LoginIntentError::GenerationExhausted,
        ] {
            let public = error.to_string();
            assert!(public.is_ascii() && public.len() <= 80);
            assert!(!public.contains(FIRST) && !public.contains(SECOND));
            assert!(!public.contains("vless://") && !public.contains("192.0.2."));
        }
    }
}
