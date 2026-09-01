// SPDX-License-Identifier: MIT

use omavless_domain::store::{
    NormalizedStoreState, ProfileState, StartupState, StoreError, StoreStateInput,
    SubscriptionState, normalize_store_state,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProfileCase {
    id: String,
    subscription_id: String,
    subscription_key: String,
    missing: bool,
    favorite: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SubscriptionCase {
    id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StartupCase {
    enabled: bool,
    target: String,
    profile_id: String,
    mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    id: String,
    store_version: u8,
    profiles: Vec<ProfileCase>,
    subscriptions: Vec<SubscriptionCase>,
    active_id: String,
    last_id: String,
    routing_preset: Option<String>,
    startup: Option<StartupCase>,
    startup_configured: Option<bool>,
    onboarding_complete: Option<bool>,
    classification: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u64,
    cases: Vec<Case>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Outcome {
    accepted: bool,
    classification: String,
    #[serde(default)]
    version: u8,
    #[serde(default = "missing_index")]
    active_index: i16,
    #[serde(default = "missing_index")]
    last_index: i16,
    #[serde(default)]
    routing_preset: String,
    #[serde(default)]
    startup_configured: bool,
    #[serde(default)]
    startup_enabled: bool,
    #[serde(default)]
    startup_target: String,
    #[serde(default = "missing_index")]
    startup_profile_index: i16,
    #[serde(default)]
    startup_mode: String,
    #[serde(default)]
    onboarding_complete: bool,
    #[serde(default)]
    profile_facts: Vec<(bool, bool, bool)>,
}

const fn missing_index() -> i16 {
    -1
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonCase {
    id: String,
    #[serde(flatten)]
    outcome: Outcome,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    cases: Vec<PythonCase>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn input(case: &Case) -> StoreStateInput {
    StoreStateInput {
        version: case.store_version,
        profiles: case
            .profiles
            .iter()
            .map(|profile| ProfileState {
                id: profile.id.clone(),
                subscription_id: profile.subscription_id.clone(),
                subscription_key: profile.subscription_key.clone(),
                missing: profile.missing,
                favorite: profile.favorite,
            })
            .collect(),
        subscriptions: case
            .subscriptions
            .iter()
            .map(|subscription| SubscriptionState {
                id: subscription.id.clone(),
            })
            .collect(),
        active_id: case.active_id.clone(),
        last_id: case.last_id.clone(),
        routing_preset: case.routing_preset.clone(),
        startup: case.startup.as_ref().map(|startup| StartupState {
            enabled: startup.enabled,
            target: startup.target.clone(),
            profile_id: startup.profile_id.clone(),
            mode: startup.mode.clone(),
        }),
        startup_configured: case.startup_configured,
        onboarding_complete: case.onboarding_complete,
    }
}

fn record_index(store: &NormalizedStoreState, id: &str) -> i16 {
    store
        .profiles
        .iter()
        .position(|profile| profile.id == id)
        .and_then(|index| i16::try_from(index).ok())
        .unwrap_or(-1)
}

fn accepted(store: NormalizedStoreState) -> Outcome {
    Outcome {
        accepted: true,
        classification: "accepted".to_owned(),
        version: store.version,
        active_index: record_index(&store, &store.active_id),
        last_index: record_index(&store, &store.last_id),
        routing_preset: store.routing_preset.clone(),
        startup_configured: store.startup_configured,
        startup_enabled: store.startup.enabled,
        startup_target: store.startup.target.clone(),
        startup_profile_index: record_index(&store, &store.startup.profile_id),
        startup_mode: store.startup.mode.clone(),
        onboarding_complete: store.onboarding_complete,
        profile_facts: store
            .profiles
            .iter()
            .map(|profile| {
                (
                    !profile.subscription_id.is_empty(),
                    if profile.subscription_id.is_empty() {
                        false
                    } else {
                        profile.missing
                    },
                    profile.favorite,
                )
            })
            .collect(),
    }
}

fn rejected(error: StoreError) -> Outcome {
    Outcome {
        accepted: false,
        classification: error.code().to_owned(),
        version: 0,
        active_index: -1,
        last_index: -1,
        routing_preset: String::new(),
        startup_configured: false,
        startup_enabled: false,
        startup_target: String::new(),
        startup_profile_index: -1,
        startup_mode: String::new(),
        onboarding_complete: false,
        profile_facts: Vec::new(),
    }
}

#[test]
fn python_and_rust_store_state_semantics_match() {
    let raw = std::fs::read_to_string(root().join("tests/parity_cases/store-state-v1.json"))
        .expect("read store corpus");
    let corpus: Corpus = serde_json::from_str(&raw).expect("valid store corpus");
    assert_eq!(corpus.version, 1);
    assert_eq!(corpus.cases.len(), 20);
    let mut child = Command::new("python3")
        .arg(root().join("tools/store_state_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python oracle");
    child
        .stdin
        .take()
        .expect("oracle stdin")
        .write_all(
            &serde_json::to_vec(&serde_json::json!({"cases": &corpus.cases}))
                .expect("oracle request"),
        )
        .expect("write oracle input");
    let output = child.wait_with_output().expect("oracle output");
    assert!(output.status.success(), "Python store oracle failed");
    let reference: Envelope = serde_json::from_slice(&output.stdout).expect("oracle response");
    assert_eq!(reference.cases.len(), corpus.cases.len());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE-NOT-A-KEY"));
    for (case, expected) in corpus.cases.iter().zip(reference.cases) {
        assert_eq!(case.id, expected.id);
        assert_eq!(
            case.classification, expected.outcome.classification,
            "Python {}",
            case.id
        );
        let rust = match normalize_store_state(input(case)) {
            Ok(store) => accepted(store),
            Err(error) => rejected(error),
        };
        assert_eq!(case.classification, rust.classification, "Rust {}", case.id);
        assert_eq!(rust, expected.outcome, "differential {}", case.id);
    }
}
