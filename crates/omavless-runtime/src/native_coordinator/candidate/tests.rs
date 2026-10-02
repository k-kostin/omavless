// SPDX-License-Identifier: MIT
use super::*;
use crate::cutover::CutoverPaths;
use crate::desired::{DesiredState, OwnedObservation, write_desired};
use crate::lifecycle::HostStepError;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use omavless_domain::private_store::{parse_candidate_private_store, parse_private_store};
use omavless_profile::wireguard::parse_wireguard_config;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

const URI_ID: &str = "00000000-0000-0000-0000-000000000001";
const WG_ID: &str = "00000000-0000-0000-0000-000000000002";
const NEW_ID: &str = "00000000-0000-0000-0000-000000000003";

struct Host {
    observation: OwnedObservation,
    observations: usize,
    effects: usize,
    during_observation: Option<(PathBuf, Vec<u8>)>,
}

impl LifecycleHost for Host {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.observations += 1;
        if let Some((path, bytes)) = self.during_observation.take() {
            fs::write(path, bytes).unwrap();
        }
        Ok(self.observation)
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        self.effects += 1;
        Err(HostStepError::Prepare)
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        self.effects += 1;
        Err(HostStepError::Start)
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        self.effects += 1;
        Err(HostStepError::Commit)
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        self.effects += 1;
        Err(HostStepError::Stop)
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        self.effects += 1;
        Err(HostStepError::Cleanup)
    }
}

fn wg(address: &str) -> CandidateProfileInput {
    CandidateProfileInput::WireGuard(parse_wireguard_config(&format!(
        "[Interface]\nPrivateKey={}\nAddress={address}\n[Peer]\nPublicKey={}\nAllowedIPs=0.0.0.0/0\nEndpoint=192.0.2.1:51820\n",
        STANDARD.encode([7_u8; 32]), STANDARD.encode([9_u8; 32]),
    )).unwrap())
}

fn import() -> CandidateProfileMutation {
    CandidateProfileMutation::Import {
        profile_id: NEW_ID.into(),
        name: "New WG".into(),
        input: wg("10.8.0.3/32"),
    }
}
fn replace() -> CandidateProfileMutation {
    CandidateProfileMutation::Replace {
        profile_id: WG_ID.into(),
        name: "Edited WG".into(),
        input: wg("10.8.0.4/32"),
    }
}
fn delete(id: &str) -> CandidateProfileMutation {
    CandidateProfileMutation::Delete {
        profile_id: id.into(),
    }
}

fn fixture() -> (PathBuf, PathBuf, OfflineNativeCoordinator<Host>) {
    let root = crate::test_temp::directory("p4-owner").unwrap();
    let config = root.join("config");
    let state = root.join("state");
    let runtime = root.join("runtime");
    for path in [&root, &config, &state, &runtime] {
        fs::create_dir_all(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let uid = fs::metadata(&root).unwrap().uid();
    let store = config.join("profiles.json");
    let candidate = parse_candidate_private_store(&serde_json::json!({
        "version":4, "activeId":URI_ID, "lastId":WG_ID, "extension":{"keep":true},
        "startup":{"enabled":true,"target":"profile","profileId":WG_ID,"mode":"rule"},
        "profiles":[{"id":URI_ID,"name":"URI","protocol":"trojan","uri":"trojan://invented-token@203.0.113.1:443","extension":"keep"}]
    }).to_string()).unwrap().with_profile(WG_ID,"WG",wg("10.8.0.2/32")).unwrap();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&store)
        .unwrap();
    output
        .write_all(&candidate.into_private_bytes().unwrap())
        .unwrap();
    drop(output);
    let paths = CutoverPaths::below(&runtime, &state, uid);
    let desired = DesiredPaths::below(&state);
    write_desired(&desired, uid, &DesiredState::default()).unwrap();
    fs::write(
        &paths.ownership_marker,
        br#"{"schemaVersion":1,"phase":"rust","generation":2}"#,
    )
    .unwrap();
    fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o600)).unwrap();
    let owner = OfflineNativeCoordinator::new_ownership_gated(
        Host {
            observation: OwnedObservation {
                service_active: false,
                controller_ready: false,
                core_count: 0,
                tun_count: 0,
                active_profile_matches: false,
            },
            observations: 0,
            effects: 0,
            during_observation: None,
        },
        desired,
        &store,
        paths,
        uid,
        2,
    );
    (root, store, owner)
}

fn outcome(
    value: NativeOwnerExecution,
) -> (
    CachedOutcome,
    Result<NativeMutationOutcome, NativeTransactionError>,
) {
    match value {
        NativeOwnerExecution::Applied { cached, outcome } => (cached, outcome),
        other => panic!("Unexpected fixed execution result {other:?}"),
    }
}

#[test]
fn active_store_and_desired_targets_refuse_before_publication_or_host_effects() {
    for use_desired in [false, true] {
        for replace_target in [false, true] {
            let (root, store, mut owner) = fixture();
            let target = if use_desired { WG_ID } else { URI_ID };
            if use_desired {
                write_desired(
                    owner.transaction.desired_paths(),
                    owner.uid(),
                    &DesiredState {
                        connected: true,
                        profile_id: target.into(),
                        ..DesiredState::default()
                    },
                )
                .unwrap();
            }
            let before = fs::read(&store).unwrap();
            let desired = fs::read(&owner.transaction.desired_paths().file).unwrap();
            let intent = if replace_target {
                CandidateProfileMutation::Replace {
                    profile_id: target.into(),
                    name: "Replacement".into(),
                    input: wg("10.8.0.4/32"),
                }
            } else {
                delete(target)
            };
            let (cached, result) = outcome(
                owner
                    .execute_candidate_profile(intent, "active-refusal", 0)
                    .unwrap(),
            );
            assert_eq!(cached.error, Some(StableErrorCode::Conflict));
            assert_eq!(
                result,
                Err(NativeTransactionError::Profile(
                    ProfileTransactionError::Conflict
                ))
            );
            assert!(fs::read(&store).unwrap() == before);
            assert!(fs::read(&owner.transaction.desired_paths().file).unwrap() == desired);
            assert_eq!(owner.host().observations, 0);
            assert_eq!(owner.host().effects, 0);
            assert_eq!(owner.revision(), 0);
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn busy_stale_ungated_and_revoked_ownership_have_no_effects() {
    let (root, store, mut owner) = fixture();
    let before = fs::read(&store).unwrap();
    assert_eq!(
        owner
            .execute_candidate_profile(import(), "stale", 1)
            .unwrap_err(),
        NativeOwnerError::Coordinator(CoordinatorError::RevisionConflict)
    );
    let lock = owner.transaction.acquire_lock().unwrap();
    assert_eq!(
        owner
            .execute_candidate_profile(import(), "busy", 0)
            .unwrap_err(),
        NativeOwnerError::OwnershipBusy
    );
    assert!(matches!(
        owner.candidate_native_export(0, WG_ID),
        Err(NativeOwnerError::OwnershipBusy)
    ));
    drop(lock);
    let fence = owner.required_ownership.take();
    assert_eq!(
        owner
            .execute_candidate_profile(import(), "ungated", 0)
            .unwrap_err(),
        NativeOwnerError::OwnershipUnavailable
    );
    assert!(matches!(
        owner.candidate_edit_input(0, WG_ID),
        Err(NativeOwnerError::OwnershipUnavailable)
    ));
    owner.required_ownership = fence;
    fs::write(
        &owner.transaction.cutover_paths().ownership_marker,
        br#"{"schemaVersion":1,"phase":"rust","generation":3}"#,
    )
    .unwrap();
    assert_eq!(
        owner
            .execute_candidate_profile(import(), "revoked", 0)
            .unwrap_err(),
        NativeOwnerError::OwnershipUnavailable
    );
    assert!(matches!(
        owner.candidate_native_export(0, WG_ID),
        Err(NativeOwnerError::OwnershipUnavailable)
    ));
    assert!(fs::read(&store).unwrap() == before);
    assert_eq!(owner.host().effects, 0);
    assert_eq!(owner.host().observations, 0);
    assert_eq!(owner.revision(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_editor_export_replace_import_delete_preserve_unrelated_mixed_state() {
    let (root, store, mut owner) = fixture();
    write_desired(
        owner.transaction.desired_paths(),
        owner.uid(),
        &DesiredState {
            connected: true,
            profile_id: URI_ID.into(),
            ..DesiredState::default()
        },
    )
    .unwrap();
    owner.host_mut().observation = OwnedObservation {
        service_active: true,
        controller_ready: true,
        core_count: 1,
        tun_count: 1,
        active_profile_matches: true,
    };
    let desired = fs::read(&owner.transaction.desired_paths().file).unwrap();
    let before: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    let seed = owner.candidate_edit_input(0, WG_ID).unwrap();
    assert_eq!(seed.private_name(), "WG");
    let exported = owner.candidate_native_export(0, WG_ID).unwrap();
    let native = std::str::from_utf8(exported.expose_private_bytes()).unwrap();
    let reimport = CandidateProfileInput::WireGuard(parse_wireguard_config(native).unwrap());
    let noop = CandidateProfileMutation::Replace {
        profile_id: WG_ID.into(),
        name: "WG".into(),
        input: reimport,
    };
    let inode = fs::metadata(&store).unwrap().ino();
    let (cached, result) = outcome(owner.execute_candidate_profile(noop, "noop", 0).unwrap());
    assert!(result.is_ok());
    assert_eq!(cached.revision, 0);
    assert_eq!(fs::metadata(&store).unwrap().ino(), inode);
    for (intent, op, rev) in [
        (import(), "import", 0),
        (replace(), "replace", 1),
        (delete(WG_ID), "delete", 2),
    ] {
        let (cached, result) = outcome(owner.execute_candidate_profile(intent, op, rev).unwrap());
        assert!(result.is_ok());
        assert_eq!(cached.revision, rev + 1);
    }
    let after: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    assert!(after["profiles"][0] == before["profiles"][0]);
    assert!(after["extension"] == before["extension"]);
    assert_eq!(after["activeId"], URI_ID);
    assert_eq!(after["lastId"], URI_ID);
    assert_eq!(after["startup"]["enabled"], false);
    assert_eq!(after["startup"]["profileId"], "");
    assert!(parse_private_store(std::str::from_utf8(&fs::read(&store).unwrap()).unwrap()).is_err());
    assert!(
        parse_candidate_private_store(std::str::from_utf8(&fs::read(&store).unwrap()).unwrap())
            .is_ok()
    );
    assert!(fs::read(&owner.transaction.desired_paths().file).unwrap() == desired);
    assert_eq!(owner.host().effects, 0);
    assert_eq!(owner.revision(), 3);
    assert_eq!(owner.actual(), ActualState::Connected);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_precedes_store_read_and_changed_input_cannot_reuse_operation() {
    let (root, store, mut owner) = fixture();
    let (cached, result) = outcome(
        owner
            .execute_candidate_profile(import(), "once", 0)
            .unwrap(),
    );
    assert!(result.is_ok());
    let observations = owner.host().observations;
    let applied = fs::read(&store).unwrap();
    fs::set_permissions(&store, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        owner
            .execute_candidate_profile(import(), "once", 0)
            .unwrap(),
        NativeOwnerExecution::Replay(cached)
    );
    assert_eq!(owner.host().observations, observations);
    assert_eq!(
        owner
            .execute_candidate_profile(replace(), "once", 1)
            .unwrap_err(),
        NativeOwnerError::Coordinator(CoordinatorError::OperationConflict)
    );
    assert!(matches!(
        owner.candidate_edit_input(0, WG_ID),
        Err(NativeOwnerError::Coordinator(
            CoordinatorError::RevisionConflict
        ))
    ));
    assert!(owner.candidate_edit_input(1, WG_ID).is_err());
    assert!(fs::read(&store).unwrap() == applied);
    fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(owner.candidate_edit_input(1, WG_ID).is_ok());
    assert_eq!(owner.revision(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn uncertain_published_write_compensates_exact_bytes_and_replays_failure() {
    let (root, store, mut owner) = fixture();
    let before = fs::read(&store).unwrap();
    let (cached, result) = outcome(
        owner
            .execute_candidate_with_commit(replace(), "uncertain", 0, |plan, lock| {
                assert_eq!(plan.commit_locked(lock).unwrap(), PreparedWrite::Changed);
                Err(CandidateStoreWriteError::Write(
                    PrivateStoreWriteError::StoreIo,
                ))
            })
            .unwrap(),
    );
    assert_eq!(
        result,
        Err(NativeTransactionError::Profile(
            ProfileTransactionError::Store
        ))
    );
    assert_eq!(cached.revision, 0);
    assert!(fs::read(&store).unwrap() == before);
    assert_eq!(
        fs::metadata(&store).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        owner
            .execute_candidate_profile(replace(), "uncertain", 0)
            .unwrap(),
        NativeOwnerExecution::Replay(cached)
    );
    assert_eq!(owner.host().effects, 0);
    assert!(owner.actual() != ActualState::ManualRecoveryRequired);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrelated_change_is_not_overwritten_and_unproved_compensation_blocks_owner() {
    let (root, store, mut owner) = fixture();
    let external = br#"{"version":4,"profiles":[],"extension":"external edit"}"#.to_vec();
    let (cached, result) = outcome(
        owner
            .execute_candidate_with_commit(replace(), "external", 0, |plan, lock| {
                plan.commit_locked(lock).unwrap();
                fs::write(&store, &external).unwrap();
                Err(CandidateStoreWriteError::Write(
                    PrivateStoreWriteError::StoreChanged,
                ))
            })
            .unwrap(),
    );
    assert_eq!(
        result,
        Err(NativeTransactionError::Profile(
            ProfileTransactionError::ManualRecoveryRequired
        ))
    );
    assert_eq!(cached.revision, 1);
    assert_eq!(owner.actual(), ActualState::ManualRecoveryRequired);
    assert!(fs::read(&store).unwrap() == external);
    assert!(matches!(
        owner.candidate_native_export(1, WG_ID),
        Err(NativeOwnerError::ManualRecoveryRequired)
    ));
    let (_, blocked) = outcome(
        owner
            .execute_candidate_profile(import(), "blocked", 1)
            .unwrap(),
    );
    assert_eq!(
        blocked,
        Err(NativeTransactionError::Profile(
            ProfileTransactionError::ManualRecoveryRequired
        ))
    );
    assert!(fs::read(&store).unwrap() == external);
    assert_eq!(owner.host().effects, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn desired_change_during_fresh_observation_refuses_before_commit() {
    let (root, store, mut owner) = fixture();
    let before = fs::read(&store).unwrap();
    let mut changed = owner.transaction.desired().unwrap();
    changed.generation += 1;
    owner.host_mut().during_observation = Some((
        owner.transaction.desired_paths().file.clone(),
        serde_json::to_vec(&changed).unwrap(),
    ));
    let (_, result) = outcome(
        owner
            .execute_candidate_profile(replace(), "changed-desired", 0)
            .unwrap(),
    );
    assert_eq!(
        result,
        Err(NativeTransactionError::Profile(
            ProfileTransactionError::Conflict
        ))
    );
    assert!(fs::read(&store).unwrap() == before);
    assert_eq!(owner.revision(), 0);
    assert_eq!(owner.host().effects, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ambiguous_live_state_blocks_without_store_or_lifecycle_effects() {
    let (root, store, mut owner) = fixture();
    let before = fs::read(&store).unwrap();
    owner.host_mut().observation.service_active = true;
    let (_, result) = outcome(
        owner
            .execute_candidate_profile(import(), "ambiguous", 0)
            .unwrap(),
    );
    assert_eq!(
        result,
        Err(NativeTransactionError::Profile(
            ProfileTransactionError::ManualRecoveryRequired
        ))
    );
    assert!(fs::read(&store).unwrap() == before);
    assert_eq!(owner.actual(), ActualState::ManualRecoveryRequired);
    assert_eq!(owner.host().effects, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_and_existing_methods_share_operation_namespace_and_revision() {
    let (root, store, mut owner) = fixture();
    outcome(
        owner
            .execute_candidate_profile(import(), "shared", 0)
            .unwrap(),
    )
    .1
    .unwrap();
    let applied = fs::read(&store).unwrap();
    let request = serde_json::json!({"api":"omavless.control","version":1,"id":"synthetic","method":"profiles.favorite",
        "params":{"profileId":URI_ID,"enabled":true,"operationId":"shared","expectedRevision":1}});
    assert_eq!(
        owner.execute_profile(&request).unwrap_err(),
        NativeOwnerError::Coordinator(CoordinatorError::OperationConflict)
    );
    let mut stale = request;
    stale["params"]["operationId"] = "other-family".into();
    stale["params"]["expectedRevision"] = 0.into();
    assert_eq!(
        owner.execute_profile(&stale).unwrap_err(),
        NativeOwnerError::Coordinator(CoordinatorError::RevisionConflict)
    );
    assert!(fs::read(&store).unwrap() == applied);
    assert_eq!(owner.revision(), 1);
    assert_eq!(owner.host().effects, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_inputs_collision_missing_target_and_unrelated_corruption_refuse() {
    let (root, store, mut owner) = fixture();
    let before = fs::read(&store).unwrap();
    let cases = [
        CandidateProfileMutation::Import {
            profile_id: WG_ID.into(),
            name: "Duplicate ID".into(),
            input: wg("10.8.0.3/32"),
        },
        CandidateProfileMutation::Import {
            profile_id: NEW_ID.into(),
            name: "URI".into(),
            input: wg("10.8.0.3/32"),
        },
        CandidateProfileMutation::Import {
            profile_id: NEW_ID.into(),
            name: "Invalid".into(),
            input: CandidateProfileInput::Uri("invalid invented token".into()),
        },
        delete(NEW_ID),
    ];
    for (index, intent) in cases.into_iter().enumerate() {
        let (_, result) = outcome(
            owner
                .execute_candidate_profile(intent, &format!("invalid-{index}"), 0)
                .unwrap(),
        );
        assert!(result.is_err());
        assert!(fs::read(&store).unwrap() == before);
        assert_eq!(owner.revision(), 0);
    }
    let mut corrupt: Value = serde_json::from_slice(&before).unwrap();
    corrupt["profiles"][0]["uri"] = "invalid invented token".into();
    let corrupt = serde_json::to_vec(&corrupt).unwrap();
    fs::write(&store, &corrupt).unwrap();
    assert!(owner.candidate_native_export(0, WG_ID).is_err());
    let (_, result) = outcome(
        owner
            .execute_candidate_profile(replace(), "corrupt", 0)
            .unwrap(),
    );
    assert!(result.is_err());
    assert!(fs::read(&store).unwrap() == corrupt);
    assert_eq!(owner.host().observations, 0);
    assert_eq!(owner.host().effects, 0);
    assert_eq!(owner.revision(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn revoked_generation_after_publication_preserves_bytes_and_requires_recovery() {
    let (root, store, mut owner) = fixture();
    let marker = owner.transaction.cutover_paths().ownership_marker.clone();
    let before = fs::read(&store).unwrap();
    let (cached, result) = outcome(
        owner
            .execute_candidate_with_commit(replace(), "revoked-write", 0, |plan, lock| {
                plan.commit_locked(lock).unwrap();
                fs::write(
                    &marker,
                    br#"{"schemaVersion":1,"phase":"rust","generation":3}"#,
                )
                .unwrap();
                Err(CandidateStoreWriteError::OwnershipUnavailable)
            })
            .unwrap(),
    );
    assert_eq!(
        result,
        Err(NativeTransactionError::Profile(
            ProfileTransactionError::ManualRecoveryRequired
        ))
    );
    assert_eq!(cached.revision, 1);
    let published = fs::read(&store).unwrap();
    assert!(published != before);
    assert!(parse_candidate_private_store(std::str::from_utf8(&published).unwrap()).is_ok());
    assert_eq!(owner.actual(), ActualState::ManualRecoveryRequired);
    assert_eq!(
        owner
            .execute_candidate_profile(replace(), "revoked-write", 0)
            .unwrap_err(),
        NativeOwnerError::OwnershipUnavailable
    );
    assert!(fs::read(&store).unwrap() == published);
    assert_eq!(owner.host().effects, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn v3_is_never_upgraded_and_input_bounds_refuse_before_admission() {
    let (root, store, mut owner) = fixture();
    let mut v3: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    v3["version"] = 3.into();
    v3["profiles"].as_array_mut().unwrap().truncate(1);
    let bytes = serde_json::to_vec(&v3).unwrap();
    fs::write(&store, &bytes).unwrap();
    let (_, result) = outcome(owner.execute_candidate_profile(import(), "v3", 0).unwrap());
    assert!(result.is_err());
    assert!(fs::read(&store).unwrap() == bytes);
    let huge = CandidateProfileMutation::Import {
        profile_id: NEW_ID.into(),
        name: "Huge".into(),
        input: CandidateProfileInput::Uri(
            "x".repeat(omavless_profile::MAX_CLASSIFICATION_INPUT_BYTES + 1),
        ),
    };
    assert_eq!(
        owner
            .execute_candidate_profile(huge, "oversize", 0)
            .unwrap_err(),
        NativeOwnerError::Protocol(MutationProtocolError::InvalidArgument)
    );
    assert!(!owner.coordinator.operation_id_in_use("oversize").unwrap());
    assert_eq!(owner.host().observations, 0);
    assert_eq!(owner.revision(), 0);
    fs::remove_dir_all(root).unwrap();
}
