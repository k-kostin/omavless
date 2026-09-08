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

const PENDING_BYTES: &[u8] = b"{\"schemaVersion\":1,\"kind\":\"routing-preset\"}\n";
fn pending_path(paths: &DesiredPaths) -> PathBuf {
    paths.directory.join("routing-preset.pending.json")
}
/// Existence, malformed shape, unsafe file or inaccessible state all block.
/// Never parse private payload or infer that a crashed transaction completed.
pub(crate) fn pending(paths: &DesiredPaths) -> bool {
    !matches!(std::fs::symlink_metadata(pending_path(paths)),Err(error) if error.kind()==std::io::ErrorKind::NotFound)
}

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
    original: Option<String>,
    candidate: String,
    desired_paths: DesiredPaths,
    desired: DesiredState,
    target: DesiredState,
    rollback: DesiredState,
    uid: u32,
    armed: std::cell::Cell<bool>,
    restored: std::cell::Cell<bool>,
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
            armed: std::cell::Cell::new(false),
            restored: std::cell::Cell::new(false),
        })
    }
    pub fn restart_required(&self) -> bool {
        self.original.as_deref() != Some(self.candidate.as_str()) || self.desired != self.target
    }
    fn arm(&self) -> Result<(), PrivateStoreWriteError> {
        if !self.changed() {
            return Ok(());
        }
        match omavless_store::atomic_create_private(
            &pending_path(&self.desired_paths),
            PENDING_BYTES,
            self.uid,
        )
        .map_err(|_| PrivateStoreWriteError::StoreIo)?
        {
            omavless_store::PrivateCreateOutcome::Created => {
                self.armed.set(true);
                Ok(())
            }
            omavless_store::PrivateCreateOutcome::AlreadyExists => {
                Err(PrivateStoreWriteError::StoreChanged)
            }
        }
    }
    fn clear_verified(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
        restored: bool,
    ) -> Result<(), PrivateStoreWriteError> {
        self.authorized(lock, paths)?;
        self.store.verify_outcome_locked(lock, paths, restored)?;
        let expected = if restored {
            self.original.as_deref()
        } else {
            Some(self.candidate.as_str())
        };
        if private_template(&self.template, self.uid)?.as_deref() != expected {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        let desired = self.desired_now()?;
        if (restored && desired != self.desired && desired != self.rollback)
            || (!restored && desired != self.target)
        {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        if !self.armed.get() {
            return if pending(&self.desired_paths) {
                Err(PrivateStoreWriteError::StoreChanged)
            } else {
                Ok(())
            };
        }
        let marker = pending_path(&self.desired_paths);
        let meta =
            std::fs::symlink_metadata(&marker).map_err(|_| PrivateStoreWriteError::StoreIo)?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.uid() != self.uid
            || meta.permissions().mode() & 0o7777 != 0o600
            || meta.len() != PENDING_BYTES.len() as u64
            || read_private_utf8(&marker, self.uid)
                .map_err(|_| PrivateStoreWriteError::StoreIo)?
                .as_bytes()
                != PENDING_BYTES
        {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        std::fs::remove_file(&marker).map_err(|_| PrivateStoreWriteError::StoreIo)?;
        if std::fs::File::open(&self.desired_paths.directory)
            .and_then(|directory| directory.sync_all())
            .is_err()
        {
            // Preserve a blocker if durable removal cannot be proven.
            let _ = omavless_store::atomic_create_private(&marker, PENDING_BYTES, self.uid);
            return Err(PrivateStoreWriteError::StoreIo);
        }
        self.armed.set(false);
        Ok(())
    }
    pub fn finish_outcome(
        &self,
        outcome: Result<
            crate::profile_transaction::ProfileMutationOutcome,
            crate::profile_transaction::ProfileTransactionError,
        >,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<
        crate::profile_transaction::ProfileMutationOutcome,
        crate::profile_transaction::ProfileTransactionError,
    > {
        use crate::profile_transaction::ProfileTransactionError;
        let safe = outcome.is_ok()
            || (self.restored.get()
                && outcome != Err(ProfileTransactionError::ManualRecoveryRequired));
        if safe {
            self.clear_verified(lock, paths, outcome.is_err())
                .map_err(|_| ProfileTransactionError::ManualRecoveryRequired)?;
        } else if self.armed.get() || pending(&self.desired_paths) {
            return Err(ProfileTransactionError::ManualRecoveryRequired);
        }
        outcome
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
        if private_template(&self.template, self.uid)?.as_deref() != Some(payload) {
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
        if current.as_deref() != Some(self.candidate.as_str()) {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        match &self.original {
            Some(original) => self.replace_template(original)?,
            None => {
                // Remove only the exact candidate created by this attempt,
                // under the matching migration lock and fixed template path.
                std::fs::remove_file(&self.template)
                    .map_err(|_| PrivateStoreWriteError::StoreIo)?;
                std::fs::File::open(
                    self.template
                        .parent()
                        .ok_or(PrivateStoreWriteError::UnsafeStore)?,
                )
                .and_then(|directory| directory.sync_all())
                .map_err(|_| PrivateStoreWriteError::StoreIo)?;
                if private_template(&self.template, self.uid)?.is_some() {
                    return Err(PrivateStoreWriteError::StoreChanged);
                }
            }
        }
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
fn private_template(path: &Path, uid: u32) -> Result<Option<String>, PrivateStoreWriteError> {
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
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(PrivateStoreWriteError::StoreIo),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > omavless_domain::config::MAX_TEMPLATE_BYTES as u64
    {
        return Err(PrivateStoreWriteError::UnsafeStore);
    }
    read_private_utf8(path, uid)
        .map(Some)
        .map_err(|_| PrivateStoreWriteError::StoreIo)
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
        self.store.verify_outcome_locked(lock, paths, true)?;
        self.arm()?;
        self.store.commit_locked(lock, paths)?;
        if self.original.as_deref() != Some(self.candidate.as_str()) {
            if self.original.is_none() {
                match omavless_store::atomic_create_private(
                    &self.template,
                    self.candidate.as_bytes(),
                    self.uid,
                )
                .map_err(|_| PrivateStoreWriteError::StoreIo)?
                {
                    omavless_store::PrivateCreateOutcome::Created => {}
                    omavless_store::PrivateCreateOutcome::AlreadyExists => {
                        return Err(PrivateStoreWriteError::StoreChanged);
                    }
                }
                if private_template(&self.template, self.uid)?.as_deref()
                    != Some(self.candidate.as_str())
                {
                    return Err(PrivateStoreWriteError::StoreChanged);
                }
            } else {
                self.replace_template(&self.candidate)?;
            }
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
        self.restored.set(true);
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
    fn request(preset: &str, keep_mode: bool) -> Value {
        json!({"api":"omavless.control","version":1,"id":"preset","method":"routing.set_preset","params":{"preset":preset,"keepMode":keep_mode}})
    }
    fn fixture() -> (PathBuf, PathBuf, DesiredPaths, CutoverPaths, u32) {
        let root = crate::test_temp::directory("preset").unwrap();
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
                    plan.clear_verified(&lock, &cutover, false).unwrap();
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
        for keep_mode in [false, true] {
            for preset in ["roscomvpn-default", "china-cn-direct", "iran-ir-direct"] {
                let template_path = root.join("config/route-template.yaml");
                fs::remove_file(&template_path).unwrap();
                write(&store, initial.to_string().as_bytes());
                write_desired(&desired, uid, &DesiredState::default()).unwrap();
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
                plan.clear_verified(&lock, &cutover, false).unwrap();
                let document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
                let output = json!({"store":document,"template":fs::read_to_string(&template_path).unwrap()});
                actual.push(format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&output).unwrap())
                ));
                cases.push(json!({"store":initial,"template":null,"preset":preset,"keepMode":keep_mode,"missingTemplate":true}));
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
        assert_eq!(expected.len(), 24);
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
                assert!(plan.clear_verified(&lock, &cutover, true).is_err());
                assert!(pending(&desired));
            } else {
                assert!(fs::read(&template_path).unwrap() == template);
                assert_eq!(
                    plan.restore(&lock, &cutover).unwrap(),
                    PreparedWrite::NoChange
                );
                plan.clear_verified(&lock, &cutover, true).unwrap();
                assert!(!pending(&desired));
            }
            drop(lock);
            fs::remove_dir_all(root).unwrap();
        }
    }

    struct Host {
        observation: crate::desired::OwnedObservation,
        fail_starts: usize,
        fail_stop: bool,
        calls: usize,
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
            self.calls += 1;
            Ok(self.observation)
        }
        fn prepare(&mut self, _: &DesiredState) -> Result<(), crate::lifecycle::HostStepError> {
            self.calls += 1;
            Ok(())
        }
        fn start_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            self.calls += 1;
            if self.fail_starts > 0 {
                self.fail_starts -= 1;
                return Err(crate::lifecycle::HostStepError::Start);
            }
            self.observation = healthy();
            Ok(())
        }
        fn commit_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            self.calls += 1;
            Ok(())
        }
        fn stop_owned(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            self.calls += 1;
            if self.fail_stop {
                return Err(crate::lifecycle::HostStepError::Stop);
            }
            self.observation = empty();
            Ok(())
        }
        fn discard_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
            self.calls += 1;
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
                    calls: 0,
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
            let result = plan.finish_outcome(result, &lock, &cutover);
            assert_eq!(result.err(), expected);
            // A failed old-core recovery keeps the durable blocker even when
            // all file members were restored. An uncertain initial stop made
            // no member writes and remains the existing in-memory hard gate.
            assert_eq!(pending(&desired), fail_starts == 2);
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
        plan.arm().unwrap();
        plan.store.commit_locked(&lock, &cutover).unwrap();
        assert_eq!(
            plan.restore(&lock, &cutover).unwrap(),
            PreparedWrite::Changed
        );
        assert!(fs::read(&store).unwrap() == initial.as_bytes());
        assert!(fs::read(&template).unwrap() == original);
        assert_eq!(read_desired(&desired, uid).unwrap().generation, 0);
        plan.clear_verified(&lock, &cutover, true).unwrap();
        assert!(!pending(&desired));
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

    #[test]
    fn routing_preset_missing_template_create_and_rollback_preserve_racing_files() {
        for race in [false, true] {
            let (root, store, desired, cutover, uid) = fixture();
            let lock = MigrationLock::acquire(&cutover, uid).unwrap();
            let initial =
                json!({"version":3,"profiles":[],"subscriptions":[],"routingPreset":"custom"})
                    .to_string();
            write(&store, initial.as_bytes());
            write_desired(&desired, uid, &DesiredState::default()).unwrap();
            let template = root.join("config/route-template.yaml");
            let plan = PresetPlan::prepare(
                &store,
                &desired,
                uid,
                &parse(&request("china-cn-direct", false)).unwrap(),
            )
            .unwrap();
            assert!(!template.exists());
            if race {
                write(&template, b"private-token-racing-winner");
                assert_eq!(
                    plan.commit(&lock, &cutover),
                    Err(PrivateStoreWriteError::StoreChanged)
                );
                assert!(fs::read(&store).unwrap() == initial.as_bytes());
                assert!(fs::read(&template).unwrap() == b"private-token-racing-winner");
            } else {
                plan.commit(&lock, &cutover).unwrap();
                assert!(template.is_file());
                assert_eq!(
                    fs::metadata(&template).unwrap().permissions().mode() & 0o777,
                    0o600
                );
                plan.restore(&lock, &cutover).unwrap();
                assert!(!template.exists());
                assert!(fs::read(&store).unwrap() == initial.as_bytes());
                assert_eq!(
                    plan.restore(&lock, &cutover).unwrap(),
                    PreparedWrite::NoChange
                );
                plan.clear_verified(&lock, &cutover, true).unwrap();
                assert!(!pending(&desired));
            }
            drop(lock);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn routing_preset_installed_mihomo_validates_all_native_candidates_without_tun() {
        use crate::lifecycle::LifecycleHost;
        let Some(core) = std::env::var_os("OMAVLESS_TEST_MIHOMO") else {
            return;
        };
        let (root, store, desired, cutover, uid) = fixture();
        let lock = MigrationLock::acquire(&cutover, uid).unwrap();
        for name in ["data", "proc", "sys-class-net"] {
            let directory = root.join(name);
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let id = "00000000-0000-4000-8000-000000000001";
        write(&store,json!({"version":3,"profiles":[{"id":id,"name":"Synthetic","protocol":"vless","uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic"}],"subscriptions":[],"routingPreset":"custom"}).to_string().as_bytes());
        write_desired(&desired, uid, &DesiredState::default()).unwrap();
        let paths = crate::native_host::NativeHostPaths::new(
            PathBuf::from(core),
            root.join("data"),
            root.join("config"),
            root.join("runtime"),
            root.join("proc"),
            root.join("sys-class-net"),
        );
        let mut host = crate::native_host::NativeLifecycleHost::new(paths, uid).unwrap();
        for preset in ["roscomvpn-default", "china-cn-direct", "iran-ir-direct"] {
            let plan = PresetPlan::prepare(
                &store,
                &desired,
                uid,
                &parse(&request(preset, false)).unwrap(),
            )
            .unwrap();
            plan.commit(&lock, &cutover).unwrap();
            let target = DesiredState {
                connected: true,
                profile_id: id.into(),
                ..read_desired(&desired, uid).unwrap()
            };
            assert!(
                host.prepare(&target).is_ok(),
                "installed Mihomo rejected bundled native candidate"
            );
            let candidate = fs::read_to_string(root.join("config/.config.candidate.yaml")).unwrap();
            assert!(candidate.contains("external-controller-unix:"));
            assert!(!candidate.contains("external-controller:"));
            assert!(!candidate.contains("{{OMAVLESS_PROXY}}"));
            host.discard_prepared().unwrap();
            assert!(host.core_pid().is_none());
            assert!(!root.join("runtime/mihomo.sock").exists());
            plan.clear_verified(&lock, &cutover, false).unwrap();
        }
        drop(lock);
        fs::remove_dir_all(root).unwrap();
    }

    fn initial_policy(store: &Path, desired: &DesiredPaths, uid: u32) {
        write(
            store,
            br#"{"version":3,"profiles":[],"subscriptions":[],"routingPreset":"custom"}"#,
        );
        write(
            &store.parent().unwrap().join("route-template.yaml"),
            b"mode: global\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
        );
        write_desired(
            desired,
            uid,
            &DesiredState {
                mode: RoutingMode::Global,
                ..DesiredState::default()
            },
        )
        .unwrap();
    }

    fn inert_host() -> Host {
        Host {
            observation: empty(),
            fail_starts: 0,
            fail_stop: false,
            calls: 0,
        }
    }

    // This child intentionally exits without unwinding: the parent exercises
    // real process restart, not a destructor-driven rollback simulation.
    #[test]
    fn routing_preset_crash_child() {
        let Some(root) = std::env::var_os("OMAVLESS_PRESET_TEST_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let stage: usize = std::env::var("OMAVLESS_PRESET_TEST_CRASH_STAGE")
            .unwrap()
            .parse()
            .unwrap();
        assert!(stage <= 3);
        let uid = fs::metadata(&root).unwrap().uid();
        let store = root.join("config/profiles.json");
        let desired = DesiredPaths::below(&root.join("state"));
        let paths = CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        let plan = PresetPlan::prepare(
            &store,
            &desired,
            uid,
            &parse(&request("china-cn-direct", false)).unwrap(),
        )
        .unwrap();
        plan.arm().unwrap();
        if stage >= 1 {
            plan.store.commit_locked(&lock, &paths).unwrap();
        }
        if stage >= 2 {
            plan.replace_template(&plan.candidate).unwrap();
        }
        if stage >= 3 {
            plan.replace_desired(&plan.target).unwrap();
        }
        std::process::exit(73);
    }

    #[test]
    fn routing_preset_crash_at_every_member_blocks_restart_and_other_mutations() {
        use crate::connection_transaction::ConnectionTransactionError;
        use crate::native_coordinator::{NativeOwnerError, OfflineNativeCoordinator};
        for stage in 0..=3 {
            let (root, store, desired, paths, uid) = fixture();
            initial_policy(&store, &desired, uid);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "routing_preset::tests::routing_preset_crash_child",
                    "--nocapture",
                ])
                .env("OMAVLESS_PRESET_TEST_CRASH_ROOT", &root)
                .env("OMAVLESS_PRESET_TEST_CRASH_STAGE", stage.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert_eq!(
                output.code(),
                Some(73),
                "crash child did not reach checkpoint"
            );
            let marker = pending_path(&desired);
            assert!(pending(&desired));
            let metadata = fs::symlink_metadata(&marker).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert_eq!(metadata.uid(), uid);
            assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
            assert_eq!(fs::read(&marker).unwrap(), PENDING_BYTES);
            let original_store = fs::read(&store).unwrap();
            let template = store.parent().unwrap().join("route-template.yaml");
            let original_template = fs::read(&template).unwrap();
            let original_desired = read_desired(&desired, uid).unwrap();
            let mut owner = OfflineNativeCoordinator::new(
                inert_host(),
                desired.clone(),
                &store,
                paths.clone(),
                uid,
            );
            assert_eq!(
                owner.reconcile_startup(),
                Err(ConnectionTransactionError::ManualRecoveryRequired)
            );
            // Also covers the constructor's continuously-held-lock path.
            let lock = MigrationLock::acquire(&paths, uid).unwrap();
            assert_eq!(
                owner.reconcile_startup_locked(&lock),
                Err(ConnectionTransactionError::ManualRecoveryRequired)
            );
            drop(lock);
            assert_eq!(
                owner.execute_routing_preset(&request("iran-ir-direct", true)),
                Err(NativeOwnerError::ManualRecoveryRequired)
            );
            let rename = json!({"api":"omavless.control","version":1,"id":"other","method":"profiles.rename","params":{"profileId":"00000000-0000-4000-8000-000000000001","name":"Synthetic"}});
            assert_eq!(
                owner.execute_profile(&rename),
                Err(NativeOwnerError::ManualRecoveryRequired)
            );
            assert_eq!(owner.host_mut().calls, 0, "blocked restart touched host");
            assert_eq!(owner.revision(), 0);
            assert!(fs::read(&store).unwrap() == original_store);
            assert!(fs::read(&template).unwrap() == original_template);
            assert_eq!(read_desired(&desired, uid).unwrap(), original_desired);
            assert_eq!(fs::read(&marker).unwrap(), PENDING_BYTES);
            write(
                &paths.ownership_marker,
                br#"{"schemaVersion":1,"generation":1,"phase":"rust"}"#,
            );
            assert_eq!(
                crate::production_owner::ProductionNativeOwner::initialize(
                    inert_host(),
                    desired.clone(),
                    &store,
                    paths.clone(),
                    uid
                )
                .err(),
                Some(crate::production_owner::ProductionOwnerError::ManualRecoveryRequired)
            );
            drop(owner);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn routing_preset_pending_blocks_cached_replay_without_host_or_state_effects() {
        use crate::native_coordinator::{NativeOwnerError, OfflineNativeCoordinator};
        let (root, store, desired, paths, uid) = fixture();
        initial_policy(&store, &desired, uid);
        let mut owner =
            OfflineNativeCoordinator::new(inert_host(), desired.clone(), &store, paths, uid);
        let mut intent = request("china-cn-direct", false);
        intent["params"]["operationId"] = json!("preset-replay");
        owner.execute_routing_preset(&intent).unwrap();
        assert_eq!(owner.revision(), 1);
        assert!(!pending(&desired));
        let calls = owner.host_mut().calls;
        write(&pending_path(&desired), PENDING_BYTES);
        assert_eq!(
            owner.execute_routing_preset(&intent),
            Err(NativeOwnerError::ManualRecoveryRequired)
        );
        assert_eq!(owner.revision(), 1);
        assert_eq!(owner.host_mut().calls, calls);
        assert!(pending(&desired));
        drop(owner);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn routing_preset_pending_never_infers_success_or_removes_foreign_marker() {
        use crate::profile_transaction::{ProfileMutationOutcome, ProfileTransactionError};
        for shape in [
            "malformed",
            "permissive",
            "symlink",
            "directory",
            "changed-store",
            "changed-template",
            "changed-desired",
        ] {
            let (root, store, desired, paths, uid) = fixture();
            initial_policy(&store, &desired, uid);
            let lock = MigrationLock::acquire(&paths, uid).unwrap();
            let plan = PresetPlan::prepare(
                &store,
                &desired,
                uid,
                &parse(&request("china-cn-direct", false)).unwrap(),
            )
            .unwrap();
            plan.commit(&lock, &paths).unwrap();
            let marker = pending_path(&desired);
            match shape {
                "malformed" => write(&marker, b"private-token"),
                "permissive" => {
                    fs::set_permissions(&marker, fs::Permissions::from_mode(0o644)).unwrap()
                }
                "symlink" => {
                    fs::remove_file(&marker).unwrap();
                    std::os::unix::fs::symlink(&store, &marker).unwrap();
                }
                "directory" => {
                    fs::remove_file(&marker).unwrap();
                    fs::create_dir(&marker).unwrap();
                }
                "changed-store" => write(&store, b"private-token"),
                "changed-template" => write(&plan.template, b"private-token"),
                "changed-desired" => {
                    let mut newer = plan.target.clone();
                    newer.generation += 1;
                    write_desired(&desired, uid, &newer).unwrap();
                }
                _ => unreachable!(),
            }
            let error = plan
                .finish_outcome(Ok(ProfileMutationOutcome { changed: true }), &lock, &paths)
                .unwrap_err();
            assert_eq!(error, ProfileTransactionError::ManualRecoveryRequired);
            assert!(!format!("{error}").contains("private-token"));
            assert!(pending(&desired));
            // Startup/admission only ask whether a blocker exists: they never
            // parse malformed marker contents or guess an automatic recovery.
            let mut owner = crate::native_coordinator::OfflineNativeCoordinator::new(
                inert_host(),
                desired.clone(),
                &store,
                paths.clone(),
                uid,
            );
            assert_eq!(owner.reconcile_startup_locked(&lock),Err(crate::connection_transaction::ConnectionTransactionError::ManualRecoveryRequired));
            assert_eq!(owner.host_mut().calls, 0);
            drop(owner);
            drop(lock);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn routing_preset_racing_pending_marker_prevents_first_member_write() {
        use crate::profile_transaction::{ProfileTransactionError, commit_store_only_profile};
        for symlink in [false, true] {
            let (root, store, desired, paths, uid) = fixture();
            initial_policy(&store, &desired, uid);
            let lock = MigrationLock::acquire(&paths, uid).unwrap();
            let plan = PresetPlan::prepare(
                &store,
                &desired,
                uid,
                &parse(&request("china-cn-direct", false)).unwrap(),
            )
            .unwrap();
            let original_store = fs::read(&store).unwrap();
            let original_template = fs::read(&plan.template).unwrap();
            let original_desired = read_desired(&desired, uid).unwrap();
            let marker = pending_path(&desired);
            if symlink {
                std::os::unix::fs::symlink(&store, &marker).unwrap();
            } else {
                write(&marker, PENDING_BYTES);
            }
            let outcome = commit_store_only_profile(&plan, &lock, &paths);
            assert_eq!(
                plan.finish_outcome(outcome, &lock, &paths),
                Err(ProfileTransactionError::ManualRecoveryRequired)
            );
            assert!(
                !plan.armed.get(),
                "attempt must not claim someone else's marker"
            );
            assert!(pending(&desired));
            assert_eq!(
                fs::symlink_metadata(&marker)
                    .unwrap()
                    .file_type()
                    .is_symlink(),
                symlink
            );
            assert!(fs::read(&store).unwrap() == original_store);
            assert!(fs::read(&plan.template).unwrap() == original_template);
            assert_eq!(read_desired(&desired, uid).unwrap(), original_desired);
            drop(lock);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn routing_preset_stale_store_refuses_before_publishing_pending_marker() {
        let (root, store, desired, paths, uid) = fixture();
        initial_policy(&store, &desired, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        let plan = PresetPlan::prepare(
            &store,
            &desired,
            uid,
            &parse(&request("china-cn-direct", false)).unwrap(),
        )
        .unwrap();
        write(&store, b"private-token-unexpected-store");
        assert_eq!(
            plan.commit(&lock, &paths),
            Err(PrivateStoreWriteError::StoreChanged)
        );
        assert!(!pending(&desired));
        assert!(fs::read(&store).unwrap() == b"private-token-unexpected-store");
        assert_eq!(read_desired(&desired, uid).unwrap(), plan.desired);
        assert!(fs::read_to_string(&plan.template).unwrap() == plan.original.unwrap());
        drop(lock);
        fs::remove_dir_all(root).unwrap();
    }
}
