// SPDX-License-Identifier: MIT
//! Fixed bundled policy transaction. No caller paths, YAML or shell input.
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::desired::{
    DesiredPaths, DesiredState, MAX_GENERATION, RoutingMode, read_desired, write_desired,
};
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use crate::private_store_transaction::{
    PreparedPrivateStoreWrite, PreparedWrite, PrivateStoreWriteError, prepare_private_store_write,
};
use crate::profile_transaction::StorePlan;
use omavless_store::{atomic_replace_private, read_private_utf8};
use serde_json::Value;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn bundled(preset: &str) -> Option<&'static str> {
    match preset {
        "roscomvpn-default" => Some(include_str!("../../../templates/default.yaml")),
        "china-cn-direct" => Some(include_str!("../../../templates/china.yaml")),
        "iran-ir-direct" => Some(include_str!("../../../templates/iran.yaml")),
        _ => None,
    }
}
pub(crate) struct PresetRequest {
    pub preset: String,
    pub keep_mode: bool,
    pub operation_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub digest: MutationDigest,
}
pub(crate) fn parse(request: &Value) -> Result<PresetRequest, MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "routing.set_preset" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(
        params,
        &["preset", "keepMode", "operationId", "expectedRevision"],
        &["preset"],
    ) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let meta = metadata(params)?;
    let preset = params["preset"]
        .as_str()
        .filter(|p| bundled(p).is_some())
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let keep_mode = params.get("keepMode").map_or(Ok(false), |v| {
        v.as_bool().ok_or(MutationProtocolError::InvalidArgument)
    })?;
    let mut bytes = b"omavless.control/routing-preset/v1\0".to_vec();
    append_field(&mut bytes, preset);
    bytes.push(u8::from(keep_mode));
    match meta.expected_revision {
        Some(revision) => {
            bytes.push(1);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        None => bytes.push(0),
    }
    Ok(PresetRequest {
        preset: preset.into(),
        keep_mode,
        operation_id: meta.operation_id.map(str::to_owned),
        expected_revision: meta.expected_revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

pub(crate) struct PresetPlan {
    store: PreparedPrivateStoreWrite,
    template: PathBuf,
    original: String,
    candidate: String,
    desired_paths: DesiredPaths,
    desired: DesiredState,
    target: DesiredState,
    rollback: DesiredState,
    uid: u32,
}
impl PresetPlan {
    pub fn prepare(
        store_path: &Path,
        desired_paths: &DesiredPaths,
        uid: u32,
        request: &PresetRequest,
    ) -> Result<Self, PrivateStoreWriteError> {
        let store = prepare_private_store_write(store_path, uid, |input| {
            let result =
                omavless_domain::private_store::apply_routing_preset(input, &request.preset)?;
            Ok((result.payload().to_vec(), result.changed))
        })?;
        let template = store_path
            .parent()
            .ok_or(PrivateStoreWriteError::UnsafeStore)?
            .join("route-template.yaml");
        let original = private_template(&template, uid)?;
        let desired =
            read_desired(desired_paths, uid).map_err(|_| PrivateStoreWriteError::StoreIo)?;
        let mode = if request.keep_mode {
            desired.mode
        } else {
            RoutingMode::Rule
        };
        let candidate = omavless_domain::routing::template_with_mode(
            bundled(&request.preset).ok_or(PrivateStoreWriteError::StoreIo)?,
            mode.as_str(),
        )
        .map_err(|_| PrivateStoreWriteError::StoreIo)?;
        let mut target = desired.clone();
        let mut rollback = desired.clone();
        if mode != desired.mode {
            target.mode = mode;
            target.generation = desired
                .generation
                .checked_add(1)
                .filter(|n| *n < MAX_GENERATION)
                .ok_or(PrivateStoreWriteError::StoreIo)?;
            rollback.generation = target.generation + 1;
        }
        Ok(Self {
            store,
            template,
            original,
            candidate,
            desired_paths: desired_paths.clone(),
            desired,
            target,
            rollback,
            uid,
        })
    }
    pub fn restart_required(&self) -> bool {
        self.original != self.candidate || self.desired != self.target
    }
    fn authorized(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<(), PrivateStoreWriteError> {
        if lock.authorizes(paths, self.uid) {
            Ok(())
        } else {
            Err(PrivateStoreWriteError::LockMismatch)
        }
    }
    fn desired_now(&self) -> Result<DesiredState, PrivateStoreWriteError> {
        read_desired(&self.desired_paths, self.uid).map_err(|_| PrivateStoreWriteError::StoreIo)
    }
    fn replace_template(&self, payload: &str) -> Result<(), PrivateStoreWriteError> {
        atomic_replace_private(&self.template, payload.as_bytes(), self.uid)
            .map_err(|_| PrivateStoreWriteError::StoreIo)?;
        if private_template(&self.template, self.uid)? != payload {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        Ok(())
    }
    fn replace_desired(&self, state: &DesiredState) -> Result<(), PrivateStoreWriteError> {
        write_desired(&self.desired_paths, self.uid, state)
            .map_err(|_| PrivateStoreWriteError::StoreIo)?;
        if self.desired_now()? != *state {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        Ok(())
    }
    fn restore_template(&self) -> Result<bool, PrivateStoreWriteError> {
        let current = private_template(&self.template, self.uid)?;
        if current == self.original {
            return Ok(false);
        }
        if current != self.candidate {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        self.replace_template(&self.original)?;
        Ok(true)
    }
    fn restore_desired(&self) -> Result<bool, PrivateStoreWriteError> {
        let current = self.desired_now()?;
        if current == self.desired || current == self.rollback {
            return Ok(false);
        }
        if current != self.target {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        self.replace_desired(&self.rollback)?;
        Ok(true)
    }
}
fn private_template(path: &Path, uid: u32) -> Result<String, PrivateStoreWriteError> {
    let parent = path
        .parent()
        .and_then(|p| std::fs::symlink_metadata(p).ok())
        .ok_or(PrivateStoreWriteError::UnsafeStore)?;
    if !parent.is_dir()
        || parent.file_type().is_symlink()
        || parent.uid() != uid
        || parent.permissions().mode() & 0o077 != 0
    {
        return Err(PrivateStoreWriteError::UnsafeStore);
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| PrivateStoreWriteError::StoreIo)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > omavless_domain::config::MAX_TEMPLATE_BYTES as u64
    {
        return Err(PrivateStoreWriteError::UnsafeStore);
    }
    read_private_utf8(path, uid).map_err(|_| PrivateStoreWriteError::StoreIo)
}
impl StorePlan for PresetPlan {
    fn changed(&self) -> bool {
        self.store.changed() || self.restart_required()
    }
    fn commit(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, PrivateStoreWriteError> {
        self.authorized(lock, paths)?;
        if private_template(&self.template, self.uid)? != self.original
            || self.desired_now()? != self.desired
        {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        self.store.commit_locked(lock, paths)?;
        if self.original != self.candidate {
            self.replace_template(&self.candidate)?;
        }
        if self.desired != self.target {
            self.replace_desired(&self.target)?;
        }
        Ok(if self.changed() {
            PreparedWrite::Changed
        } else {
            PreparedWrite::NoChange
        })
    }
    fn restore(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, PrivateStoreWriteError> {
        self.authorized(lock, paths)?;
        // Attempt every compensating member even if another is ambiguous.
        let desired = self.restore_desired();
        let template = self.restore_template();
        let store = self.store.restore_locked(lock, paths);
        let changed = desired? | template? | (store? == PreparedWrite::Changed);
        Ok(if changed {
            PreparedWrite::Changed
        } else {
            PreparedWrite::NoChange
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};
    fn request(preset: &str, keep_mode: bool) -> Value {
        json!({"api":"omavless.control","version":1,"id":"preset","method":"routing.set_preset","params":{"preset":preset,"keepMode":keep_mode}})
    }
    fn fixture() -> (PathBuf, PathBuf, DesiredPaths, CutoverPaths, u32) {
        let root = std::env::temp_dir().join(format!(
            "omavless-preset-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for path in [
            &root,
            &root.join("config"),
            &root.join("state"),
            &root.join("runtime"),
        ] {
            fs::create_dir_all(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let store = root.join("config/profiles.json");
        let desired = DesiredPaths::below(&root.join("state"));
        let cutover = CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid);
        (root, store, desired, cutover, uid)
    }
    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn routing_preset_exact_request_and_digest_bounds() {
        let baseline = request("china-cn-direct", true);
        let digest = parse(&baseline).unwrap().digest;
        for (key, value) in [
            ("path", json!("private-token")),
            ("preset", json!("private-token")),
            ("keepMode", json!("yes")),
            ("expectedRevision", json!(-1)),
        ] {
            let mut invalid = baseline.clone();
            invalid["params"][key] = value;
            let error = parse(&invalid).err().unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        for changed in [
            request("iran-ir-direct", true),
            request("china-cn-direct", false),
        ] {
            assert!(parse(&changed).unwrap().digest != digest);
        }
    }
    #[test]
    fn routing_preset_matches_actual_python_for_all_bundles_modes() {
        let (root, store, desired, cutover, uid) = fixture();
        let lock = MigrationLock::acquire(&cutover, uid).unwrap();
        let initial = json!({"version":3,"profiles":[],"subscriptions":[],"routingPreset":"custom","privateExtension":"private-token"});
        let mut cases = Vec::new();
        let mut actual = Vec::new();
        for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
            for keep_mode in [false, true] {
                for preset in ["roscomvpn-default", "china-cn-direct", "iran-ir-direct"] {
                    let template = omavless_domain::routing::template_with_mode(
                        bundled("roscomvpn-default").unwrap(),
                        mode.as_str(),
                    )
                    .unwrap();
                    write(&store, initial.to_string().as_bytes());
                    write(
                        &root.join("config/route-template.yaml"),
                        template.as_bytes(),
                    );
                    write_desired(
                        &desired,
                        uid,
                        &DesiredState {
                            mode,
                            ..DesiredState::default()
                        },
                    )
                    .unwrap();
                    let plan = PresetPlan::prepare(
                        &store,
                        &desired,
                        uid,
                        &parse(&request(preset, keep_mode)).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        plan.commit(&lock, &cutover).unwrap(),
                        PreparedWrite::Changed
                    );
                    let document: Value =
                        serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
                    let output = json!({"store":document,"template":fs::read_to_string(root.join("config/route-template.yaml")).unwrap()});
                    actual.push(format!(
                        "{:x}",
                        Sha256::digest(serde_json::to_vec(&output).unwrap())
                    ));
                    assert_eq!(
                        read_desired(&desired, uid).unwrap().mode,
                        if keep_mode { mode } else { RoutingMode::Rule }
                    );
                    cases.push(json!({"store":initial,"template":template,"preset":preset,"keepMode":keep_mode}));
                }
            }
        }
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut child = Command::new("python3")
            .arg(repo.join("tools/routing_preset_parity.py"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&cases).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "routing preset oracle failed");
        let expected: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(expected.len(), 18);
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert!(actual == &expected, "routing preset mismatch case {index}");
        }
        drop(lock);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn routing_preset_partial_commit_restores_members_and_rejects_external_edits() {
        for external in [false, true] {
            let (root, store, desired, cutover, uid) = fixture();
            let lock = MigrationLock::acquire(&cutover, uid).unwrap();
            let initial =
                json!({"version":3,"profiles":[],"subscriptions":[],"routingPreset":"custom"})
                    .to_string();
            write(&store, initial.as_bytes());
            let template_path = root.join("config/route-template.yaml");
            let template =
                b"mode: global\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
            write(&template_path, template);
            write_desired(
                &desired,
                uid,
                &DesiredState {
                    mode: RoutingMode::Global,
                    ..DesiredState::default()
                },
            )
            .unwrap();
            let plan = PresetPlan::prepare(
                &store,
                &desired,
                uid,
                &parse(&request("china-cn-direct", false)).unwrap(),
            )
            .unwrap();
            plan.commit(&lock, &cutover).unwrap();
            if external {
                write(&template_path, b"private-token-external-edit");
            }
            let result = plan.restore(&lock, &cutover);
            assert_eq!(result.is_err(), external);
            assert!(fs::read(&store).unwrap() == initial.as_bytes());
            let recovered = read_desired(&desired, uid).unwrap();
            assert_eq!(recovered.mode, RoutingMode::Global);
            assert_eq!(recovered.generation, 2);
            if external {
                assert!(fs::read(&template_path).unwrap() == b"private-token-external-edit");
            } else {
                assert!(fs::read(&template_path).unwrap() == template);
                assert_eq!(
                    plan.restore(&lock, &cutover).unwrap(),
                    PreparedWrite::NoChange
                );
            }
            drop(lock);
            fs::remove_dir_all(root).unwrap();
        }
    }

    struct Host {
        observation: crate::desired::OwnedObservation,
        fail_starts: usize,
        fail_stop: bool,
    }
    fn empty() -> crate::desired::OwnedObservation {
        crate::desired::OwnedObservation {
            service_active: false,
            controller_ready: false,
            core_count: 0,
            tun_count: 0,
            active_profile_matches: false,
        }
    }
    fn healthy() -> crate::desired::OwnedObservation {
        crate::desired::OwnedObservation {
            service_active: true,
            controller_ready: true,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        }
    }
    impl crate::lifecycle::LifecycleHost for Host {
        fn observe(
            &mut self,
            _: &DesiredState,
        ) -> Result<crate::desired::OwnedObservation, crate::lifecycle::HostStepError> {
            Ok(self.observation)
        }
        fn prepare(&mut self, _: &DesiredState) -> Result<(), crate::lifecycle::HostStepError> {
            Ok(())
        }
        fn start_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            if self.fail_starts > 0 {
                self.fail_starts -= 1;
                return Err(crate::lifecycle::HostStepError::Start);
            }
            self.observation = healthy();
            Ok(())
        }
        fn commit_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            Ok(())
        }
        fn stop_owned(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            if self.fail_stop {
                return Err(crate::lifecycle::HostStepError::Stop);
            }
            self.observation = empty();
            Ok(())
        }
        fn discard_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            Ok(())
        }
    }
    #[test]
    fn routing_preset_connected_failure_restores_policy_mode_and_owned_core() {
        use crate::profile_transaction::{ActionKind, ProfileTransactionError, apply_transaction};
        for (fail_starts, fail_stop, expected) in [
            (0, false, None),
            (
                1,
                false,
                Some(ProfileTransactionError::TransitionFailedRestored),
            ),
            (
                2,
                false,
                Some(ProfileTransactionError::ManualRecoveryRequired),
            ),
            (
                0,
                true,
                Some(ProfileTransactionError::ManualRecoveryRequired),
            ),
        ] {
            let (root, store, desired, cutover, uid) = fixture();
            let lock = MigrationLock::acquire(&cutover, uid).unwrap();
            let id = "00000000-0000-4000-8000-000000000001";
            let original =
                json!({"version":3,"profiles":[],"subscriptions":[],"routingPreset":"custom"})
                    .to_string();
            write(&store, original.as_bytes());
            let template = root.join("config/route-template.yaml");
            let original_template =
                b"mode: global\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
            write(&template, original_template);
            write_desired(
                &desired,
                uid,
                &DesiredState {
                    connected: true,
                    profile_id: id.into(),
                    mode: RoutingMode::Global,
                    ..DesiredState::default()
                },
            )
            .unwrap();
            let plan = PresetPlan::prepare(
                &store,
                &desired,
                uid,
                &parse(&request("china-cn-direct", false)).unwrap(),
            )
            .unwrap();
            let mut lifecycle = crate::lifecycle::LifecycleExecutor::new(
                Host {
                    observation: healthy(),
                    fail_starts,
                    fail_stop,
                },
                desired.clone(),
                uid,
            );
            let result = apply_transaction(
                &mut lifecycle,
                &plan,
                ActionKind::Replace,
                id,
                &lock,
                &cutover,
            );
            assert_eq!(result.err(), expected);
            let final_desired = read_desired(&desired, uid).unwrap();
            assert!(final_desired.connected);
            if expected.is_some() {
                assert!(fs::read(&store).unwrap() == original.as_bytes());
                assert!(fs::read(&template).unwrap() == original_template);
                assert_eq!(final_desired.mode, RoutingMode::Global);
            } else {
                assert_eq!(final_desired.mode, RoutingMode::Rule);
            }
            if expected == Some(ProfileTransactionError::TransitionFailedRestored) {
                assert_eq!(lifecycle.actual(), crate::lifecycle::ActualState::Connected);
                assert_eq!(final_desired.generation, 2);
            }
            drop(lock);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn routing_preset_cli_has_only_fixed_allowlisted_arguments() {
        for keep in [false, true] {
            let mut args = vec!["routing".into(), "preset".into(), "china-cn-direct".into()];
            if keep {
                args.push("keep-mode".into());
            }
            let (method, params) = crate::semantic_cli::parse_semantic_mutation(&args, None)
                .unwrap()
                .into_parts();
            assert_eq!(method, "routing.set_preset");
            assert_eq!(params, json!({"preset":"china-cn-direct","keepMode":keep}));
        }
        for args in [
            vec!["routing", "preset", "private-token"],
            vec!["routing", "preset", "china-cn-direct", "private-token"],
        ] {
            let error = crate::semantic_cli::parse_semantic_mutation(
                &args.into_iter().map(Into::into).collect::<Vec<_>>(),
                None,
            )
            .err()
            .unwrap();
            assert!(!error.to_string().contains("private-token"));
        }
    }

    #[test]
    fn routing_preset_store_only_partial_write_and_stale_snapshot_restore_safely() {
        let (root, store, desired, cutover, uid) = fixture();
        let lock = MigrationLock::acquire(&cutover, uid).unwrap();
        let initial =
            json!({"version":3,"profiles":[],"subscriptions":[],"routingPreset":"custom"})
                .to_string();
        write(&store, initial.as_bytes());
        let template = root.join("config/route-template.yaml");
        let original = b"mode: global\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
        write(&template, original);
        write_desired(
            &desired,
            uid,
            &DesiredState {
                mode: RoutingMode::Global,
                ..DesiredState::default()
            },
        )
        .unwrap();
        let plan = PresetPlan::prepare(
            &store,
            &desired,
            uid,
            &parse(&request("iran-ir-direct", false)).unwrap(),
        )
        .unwrap();
        plan.store.commit_locked(&lock, &cutover).unwrap();
        assert_eq!(
            plan.restore(&lock, &cutover).unwrap(),
            PreparedWrite::Changed
        );
        assert!(fs::read(&store).unwrap() == initial.as_bytes());
        assert!(fs::read(&template).unwrap() == original);
        assert_eq!(read_desired(&desired, uid).unwrap().generation, 0);
        let newer = DesiredState {
            generation: 1,
            mode: RoutingMode::Direct,
            ..DesiredState::default()
        };
        write_desired(&desired, uid, &newer).unwrap();
        assert_eq!(
            plan.commit(&lock, &cutover),
            Err(PrivateStoreWriteError::StoreChanged)
        );
        assert!(fs::read(&store).unwrap() == initial.as_bytes());
        assert!(fs::read(&template).unwrap() == original);
        drop(lock);
        fs::remove_dir_all(root).unwrap();
    }
}
