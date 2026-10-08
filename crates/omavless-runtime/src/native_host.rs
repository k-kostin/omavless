// SPDX-License-Identifier: MIT

//! Fixed-purpose package host adapter for the native lifecycle executor.
//!
//! This module contains no IPC surface of its own. The production owner composes
//! private store/config preparation, Mihomo validation, parent-owned startup,
//! private-controller readiness, process/TUN observation, and cleanup behind
//! [`LifecycleHost`](crate::lifecycle::LifecycleHost).

use crate::core::OwnedCore;
use crate::core_readiness::ConfigReadiness;
use crate::desired::{DesiredState, OwnedObservation};
use crate::lifecycle::{HostStepError, LifecycleHost, NativeLocalObservation};
use crate::managed_pair::ManagedPair;
use omavless_domain::config::MAX_TEMPLATE_BYTES;
use omavless_domain::private_store::parse_private_store;
use omavless_mihomo::observation::{processes_named_strict, tun_interface_count_strict};
use omavless_mihomo::validate_config;
use omavless_store::{atomic_replace_private, read_private_utf8};
use std::env;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(20);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const OBSERVATION_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_PATH_BYTES: usize = 4096;

/// Captured only from the actual parent-owned host. All controller reads and
/// executable hashing happen after moving this out of the owner mutex.
/// Opaque retained observation for the fixed lifecycle host seam. No public
/// constructor, serialization, clone or access to its authority is available.
#[doc(hidden)]
pub struct CloseObservation {
    session: crate::conditional_close_candidate::Session,
    facts: CloseFacts,
}
pub(crate) struct CloseFacts {
    readiness: ConfigReadiness,
    proc_root: PathBuf,
    sys_class_net: PathBuf,
    config_directory: PathBuf,
    uid: u32,
    tun_identity: Option<(String, u64)>,
    auxiliary: std::sync::Arc<crate::auxiliary_core::AuxiliarySlot>,
    pid: u32,
    #[cfg(test)]
    fixture: Option<CloseFixture>,
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum CloseFixture {
    OwnedLoopback,
    PassiveOwnedLoopback,
    #[cfg(feature = "developer-image-witness")]
    PreparedImage,
}
#[cfg(all(test, feature = "developer-image-witness"))]
struct PreparedImageObservation {
    observation: CloseObservation,
    desired: DesiredState,
    config: String,
    store: String,
    core_path: PathBuf,
    config_path: PathBuf,
    store_path: PathBuf,
    controller: PathBuf,
    uid: u32,
}
enum CloseImageCapture {
    Direct,
    #[cfg(all(test, feature = "developer-image-witness"))]
    Witness,
    #[cfg(feature = "developer-image-witness")]
    InstalledRuntimeWitness,
    #[cfg(feature = "product-image-witness")]
    ProductWitness,
}

pub(crate) enum CloseImageSelection {
    Direct,
    #[cfg(feature = "developer-image-witness")]
    InstalledDevelopment,
    #[cfg(feature = "product-image-witness")]
    Product,
}

#[cfg(feature = "developer-image-witness")]
enum DevelopmentImageState {
    Disabled,
    Available,
    Consumed,
}

#[cfg(feature = "product-image-witness")]
struct ProductImageEpochs {
    issued: usize,
    // Nonevicting original lifetime/session identities, never an image cache.
    history: Vec<crate::conditional_close_candidate::Cancellation>,
    active: Option<crate::conditional_close_candidate::Cancellation>,
    poisoned: bool,
}
#[cfg(feature = "product-image-witness")]
impl ProductImageEpochs {
    const LIMIT: usize = 128;
    fn new() -> Result<Self, HostStepError> {
        let mut history = Vec::new();
        history
            .try_reserve_exact(Self::LIMIT)
            .map_err(|_| HostStepError::Prepare)?;
        Ok(Self {
            issued: 0,
            history,
            active: None,
            poisoned: false,
        })
    }
    fn admission(&self) -> crate::lifecycle::CloseEpochAdmission {
        use crate::lifecycle::CloseEpochAdmission;
        if self.poisoned
            || self.issued == Self::LIMIT
            || self
                .history
                .iter()
                .any(|original| !original.epoch_lifetime_available())
        {
            CloseEpochAdmission::Refused
        } else if self.active.is_some() {
            CloseEpochAdmission::Busy
        } else {
            CloseEpochAdmission::Ready
        }
    }
    fn reserve(&mut self) -> Result<(), HostStepError> {
        if self.admission() != crate::lifecycle::CloseEpochAdmission::Ready {
            return Err(HostStepError::Observation);
        }
        // Reserve/latch before original source/controller/child acquisition.
        self.issued += 1;
        self.poisoned = true;
        Ok(())
    }
    fn captured(&mut self, original: crate::conditional_close_candidate::Cancellation) {
        self.history.push(original.clone());
        self.active = Some(original);
        self.poisoned = false;
    }
    fn complete(&mut self, original: &crate::conditional_close_candidate::CloseEpochCompletion) {
        if !self.poisoned
            && self
                .active
                .as_ref()
                .is_some_and(|expected| original.admits(expected))
        {
            self.active = None;
        } else {
            // Wrong/late identity, refused/Unknown outcome, old lifetime poison
            // or missing real drain cannot renew this factory.
            self.poisoned = true;
            if let Some(active) = &self.active {
                active.cancel();
            }
        }
    }
    fn revoke_if_used(&mut self) {
        if self.issued != 0 {
            self.poisoned = true;
            if let Some(original) = &self.active {
                original.cancel();
            }
        }
    }
}

impl CloseObservation {
    pub(crate) fn session(&self) -> &crate::conditional_close_candidate::Session {
        &self.session
    }
    pub(crate) fn session_mut(&mut self) -> &mut crate::conditional_close_candidate::Session {
        &mut self.session
    }
    pub(crate) fn into_session(mut self) -> crate::conditional_close_candidate::Session {
        self.session.attach_observation(self.facts);
        self.session
    }
    #[cfg(test)]
    pub(crate) fn fixture_permit(
        &self,
    ) -> Option<crate::conditional_close_candidate::CandidateEffectPermit> {
        match self.facts.fixture {
            Some(CloseFixture::OwnedLoopback) => {
                Some(crate::conditional_close_candidate::CandidateEffectPermit::owned_fixture())
            }
            Some(CloseFixture::PassiveOwnedLoopback) | None => None,
            #[cfg(feature = "developer-image-witness")]
            Some(CloseFixture::PreparedImage) => None,
        }
    }

    pub(crate) fn observe(&mut self) -> Result<(), HostStepError> {
        self.facts.observe(&mut self.session)
    }
}
impl CloseFacts {
    pub(crate) fn observe(
        &self,
        session: &mut crate::conditional_close_candidate::Session,
    ) -> Result<(), HostStepError> {
        session
            .prepare_executable()
            .map_err(|_| HostStepError::Observation)?;
        #[cfg(feature = "developer-conditional-close")]
        session
            .prepare_developer_pair()
            .map_err(|_| HostStepError::Observation)?;
        #[cfg(feature = "developer-conditional-close")]
        session
            .prepare_qualified_pair(&self.config_directory)
            .map_err(|_| HostStepError::Observation)?;
        if !session.proves_live() {
            return Err(HostStepError::Observation);
        }
        let named = processes_named_strict(&self.proc_root, "mihomo")
            .map_err(|_| HostStepError::Observation)?;
        let auxiliary = self
            .auxiliary
            .verified_pid()
            .map_err(|_| HostStepError::Observation)?;
        if named
            .iter()
            .filter(|pid| **pid != self.pid && Some(**pid) != auxiliary)
            .count()
            != 0
        {
            return Err(HostStepError::Observation);
        }
        let inventory = crate::tun_scope::inventory(&self.sys_class_net)?;
        let configured = crate::tun_scope::configured_devices(&self.config_directory, self.uid)?;
        let scoped = configured.as_ref().map_or(inventory.len(), |devices| {
            inventory.intersection(devices).count()
        });
        if scoped != usize::from(self.tun_identity.is_some()) {
            return Err(HostStepError::Observation);
        }
        let mut config = None;
        if !self
            .readiness
            .ready_with(Instant::now() + Duration::from_secs(3), |endpoint| {
                let result = session.read_fixed(endpoint);
                if endpoint == omavless_mihomo::ReadOnlyEndpoint::Configs {
                    config = result.clone();
                }
                result
            })
        {
            return Err(HostStepError::Observation);
        }
        let config = config.ok_or(HostStepError::Observation)?;
        match &self.tun_identity {
            Some((device, index)) => {
                if crate::traffic::controller_device(&config) != Some(device)
                    || !inventory.contains(device)
                    || crate::tun_scope::device_index(&self.sys_class_net, device)? != *index
                {
                    return Err(HostStepError::Observation);
                }
            }
            None => {
                #[cfg(test)]
                if self.fixture.is_none() || config["tun"]["enable"] != false {
                    return Err(HostStepError::Observation);
                }
                #[cfg(not(test))]
                return Err(HostStepError::Observation);
            }
        }
        if !session.proves_live()
            || named
                != processes_named_strict(&self.proc_root, "mihomo")
                    .map_err(|_| HostStepError::Observation)?
            || inventory != crate::tun_scope::inventory(&self.sys_class_net)?
            || configured != crate::tun_scope::configured_devices(&self.config_directory, self.uid)?
        {
            return Err(HostStepError::Observation);
        }
        Ok(())
    }
}

/// Stable host paths resolved by package policy, never by an IPC request.
/// This type intentionally has no `Debug` implementation.
pub struct NativeHostPaths {
    pub core: PathBuf,
    pub data_directory: PathBuf,
    pub config_directory: PathBuf,
    pub runtime_directory: PathBuf,
    pub proc_root: PathBuf,
    pub sys_class_net: PathBuf,
    pub store: PathBuf,
    pub template: PathBuf,
    pub active_config: PathBuf,
    pub staged_config: PathBuf,
    pub controller_socket: PathBuf,
    managed_pair: Option<ManagedPair>,
    require_managed_pair: bool,
}

impl NativeHostPaths {
    #[must_use]
    pub fn new(
        core: PathBuf,
        data_directory: PathBuf,
        config_directory: PathBuf,
        runtime_directory: PathBuf,
        proc_root: PathBuf,
        sys_class_net: PathBuf,
    ) -> Self {
        Self {
            core,
            data_directory,
            store: config_directory.join("profiles.json"),
            template: config_directory.join("route-template.yaml"),
            active_config: config_directory.join("config.yaml"),
            staged_config: config_directory.join(".config.candidate.yaml"),
            controller_socket: runtime_directory.join("mihomo.sock"),
            managed_pair: None,
            require_managed_pair: false,
            config_directory,
            runtime_directory,
            proc_root,
            sys_class_net,
        }
    }

    /// Resolve the fixed package-owned host paths for the current user.
    ///
    /// The optional `OMAVLESS_HOME` override exists only for isolated package
    /// acceptance tests. No path is accepted from IPC, and the Mihomo/proc/sys
    /// entry points remain fixed by package policy.
    pub fn current(runtime_directory: &Path) -> Result<Self, HostStepError> {
        let home = env::var_os("OMAVLESS_HOME")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or(HostStepError::Prepare)?;
        if !valid_absolute(&home) || !valid_absolute(runtime_directory) {
            return Err(HostStepError::Prepare);
        }
        let config = home.join(".config/omavless");
        let managed_pair = ManagedPair::detect(&config, nix::unistd::getuid().as_raw())?;
        let core = if let Some(pair) = managed_pair.as_ref() {
            pair.core_path().to_path_buf()
        } else {
            resolve_core(&home, env::var_os("OMAVLESS_MIHOMO"), env::var_os("PATH"))?
        };
        let mut paths = Self::new(
            core,
            config.clone(),
            config,
            runtime_directory.to_path_buf(),
            PathBuf::from("/proc"),
            PathBuf::from("/sys/class/net"),
        );
        paths.managed_pair = managed_pair;
        paths.require_managed_pair = true;
        Ok(paths)
    }
}

/// Resolve the same stable Mihomo entry-point classes accepted by the current
/// plugin: an explicit absolute override, the current user's local binary, or
/// an absolute PATH entry. The resolved target is canonicalized before it is
/// retained so a later lifecycle step never executes a relative path or a
/// caller-controlled shell lookup.
fn resolve_core(
    home: &Path,
    override_path: Option<std::ffi::OsString>,
    search_path: Option<std::ffi::OsString>,
) -> Result<PathBuf, HostStepError> {
    let mut candidates = Vec::new();
    if let Some(value) = override_path {
        let candidate = PathBuf::from(value);
        if !valid_absolute(&candidate) {
            return Err(HostStepError::Prepare);
        }
        candidates.push(candidate);
    }
    candidates.push(home.join(".local/bin/mihomo"));
    if let Some(value) = search_path {
        for directory in env::split_paths(&value) {
            if valid_absolute(&directory) {
                candidates.push(directory.join("mihomo"));
            }
        }
    }
    for candidate in candidates {
        let Ok(canonical) = fs::canonicalize(candidate) else {
            continue;
        };
        if valid_absolute(&canonical) && executable(&canonical) {
            return Ok(canonical);
        }
    }
    Err(HostStepError::Prepare)
}

fn valid_absolute(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    path.is_absolute() && !bytes.is_empty() && bytes.len() <= MAX_PATH_BYTES && !bytes.contains(&0)
}

pub(crate) fn private_directory(path: &Path, uid: u32) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_dir()
            && metadata.uid() == uid
            && metadata.permissions().mode() & 0o077 == 0
    })
}

fn ordinary_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
}

fn executable(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
    })
}

fn remove_owned_file(path: &Path, uid: u32, socket: bool) -> Result<(), HostStepError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(HostStepError::Cleanup),
    };
    let expected_type = if socket {
        metadata.file_type().is_socket()
    } else {
        metadata.is_file() && !metadata.file_type().is_symlink()
    };
    if !expected_type || metadata.uid() != uid {
        return Err(HostStepError::Cleanup);
    }
    fs::remove_file(path).map_err(|_| HostStepError::Cleanup)
}

/// Contains private profile identity and potentially an owned child. It must
/// never be formatted or serialized.
pub struct NativeLifecycleHost {
    paths: NativeHostPaths,
    drop_paths: DropPaths,
    uid: u32,
    core: Option<OwnedCore>,
    core_diagnostics: Option<crate::core_diagnostics::DiagnosticReader>,
    profile_id: Option<String>,
    readiness: Option<ConfigReadiness>,
    previous_config: Option<Option<Vec<u8>>>,
    active_install_attempted: bool,
    ping_slot: std::sync::Arc<crate::tun_ping::PingSlot>,
    auxiliary: std::sync::Arc<crate::auxiliary_core::AuxiliarySlot>,
    // Retained across stop until disappearance is proved. A replacement at
    // the same configured name cannot silently become our connected device.
    tun_identity: Option<(String, u64)>,
    #[cfg(test)]
    close_fixture: Option<CloseFixture>,
    #[cfg(all(test, feature = "developer-image-witness"))]
    prepared_image: Option<PreparedImageObservation>,
    #[cfg(all(test, feature = "developer-image-witness"))]
    image_fixture_attempted: bool,
    #[cfg(feature = "developer-image-witness")]
    development_image: DevelopmentImageState,
    #[cfg(feature = "product-image-witness")]
    product_image: Option<ProductImageEpochs>,
    #[cfg(all(test, feature = "product-image-witness"))]
    product_preview_fixture: bool,
}

enum DropPaths {
    Cleanup,
    Preserve,
}

/// Sealed observation-only use of the actual native observer. No inner host,
/// auxiliary slot or resource handle escapes, and no lifecycle effect delegates.
/// Its fresh inner host never owns a core or staged file, so destruction must
/// not remove another operation's same-user controller/staging names.
pub(crate) struct ObservationOnlyNativeHost {
    inner: NativeLifecycleHost,
}

impl ObservationOnlyNativeHost {
    pub(crate) fn new(paths: NativeHostPaths, uid: u32) -> Result<Self, HostStepError> {
        let mut inner = NativeLifecycleHost::new(paths, uid)?;
        inner.drop_paths = DropPaths::Preserve;
        Ok(Self { inner })
    }
}

#[cfg(feature = "t4-manager-actor-service")]
impl NativeLifecycleHost {
    pub(crate) fn new_retained_completion(
        paths: NativeHostPaths,
        uid: u32,
    ) -> Result<Self, HostStepError> {
        let mut host = Self::new(paths, uid)?;
        host.drop_paths = DropPaths::Preserve;
        Ok(host)
    }
}

impl LifecycleHost for ObservationOnlyNativeHost {
    fn observe(&mut self, desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.inner.observe(desired)
    }
    fn fresh_observation(
        &mut self,
        desired: &DesiredState,
    ) -> Result<NativeLocalObservation, HostStepError> {
        self.inner.fresh_observation(desired)
    }
    fn connection_preflight(&mut self) -> Result<(), HostStepError> {
        Err(HostStepError::Prepare)
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        Err(HostStepError::Prepare)
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        Err(HostStepError::Prepare)
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        Err(HostStepError::Prepare)
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        Err(HostStepError::Cleanup)
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        Err(HostStepError::Cleanup)
    }
}

impl NativeLifecycleHost {
    #[cfg(all(test, feature = "product-image-witness"))]
    pub(crate) fn install_product_preview_fixture_for_test(&mut self) {
        assert!(matches!(
            self.close_fixture,
            Some(CloseFixture::OwnedLoopback)
        ));
        assert!(self.core.as_ref().and_then(OwnedCore::pid).is_some());
        self.product_image = Some(ProductImageEpochs::new().unwrap());
        self.product_preview_fixture = true;
    }
    #[cfg(all(test, feature = "product-image-witness"))]
    pub(crate) fn install_product_epoch_from_fixture_session(
        &mut self,
        original: crate::conditional_close_candidate::Cancellation,
    ) {
        // Memory provider callback tests only. Not root enrollment/package
        // evidence and not used by ordinary current() or an actual helper gate.
        let mut epochs = ProductImageEpochs::new().unwrap();
        epochs.reserve().unwrap();
        epochs.captured(original);
        self.product_image = Some(epochs);
    }

    #[cfg(all(test, feature = "product-image-witness"))]
    pub(crate) fn product_history_for_test(&self) -> (usize, usize) {
        let epochs = self.product_image.as_ref().unwrap();
        (epochs.issued, epochs.history.len())
    }
    #[cfg(test)]
    pub(crate) fn owned_close_fixture(
        paths: NativeHostPaths,
        uid: u32,
        core: OwnedCore,
    ) -> Result<Self, HostStepError> {
        let mut host = Self::new(paths, uid)?;
        host.install_owned_close_fixture(core)?;
        Ok(host)
    }

    #[cfg(test)]
    pub(crate) fn install_owned_close_fixture(
        &mut self,
        core: OwnedCore,
    ) -> Result<(), HostStepError> {
        if self.core.is_some()
            || self.profile_id.is_some()
            || self.readiness.is_some()
            || self.close_fixture.is_some()
        {
            return Err(HostStepError::Observation);
        }
        self.core = Some(core);
        self.profile_id = Some("00000000-0000-4000-8000-000000000001".into());
        self.readiness = Some(ConfigReadiness::new(
            crate::desired::RoutingMode::Direct,
            "DIRECT".into(),
        ));
        self.close_fixture = Some(CloseFixture::OwnedLoopback);
        Ok(())
    }

    #[cfg(all(test, feature = "developer-conditional-close"))]
    pub(crate) fn install_passive_owned_close_fixture(
        &mut self,
        core: OwnedCore,
    ) -> Result<(), HostStepError> {
        self.install_owned_close_fixture(core)?;
        self.close_fixture = Some(CloseFixture::PassiveOwnedLoopback);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn verify_close_fixture(
        &mut self,
        desired: &DesiredState,
    ) -> Result<(), HostStepError> {
        if self.close_fixture.is_none() {
            return Err(HostStepError::Observation);
        }
        #[cfg(feature = "developer-image-witness")]
        if matches!(self.close_fixture, Some(CloseFixture::PreparedImage)) {
            let Some(mut prepared) = self.prepared_image.take() else {
                self.revoke_prepared_image();
                return Err(HostStepError::Observation);
            };
            if self
                .validate_prepared_image(desired, &mut prepared)
                .is_err()
            {
                self.revoke_prepared_image();
                prepared.observation.session.revoke_prepared_witness();
                return Err(HostStepError::Observation);
            }
            self.prepared_image = Some(prepared);
            return Ok(());
        }
        self.capture_connection_close(desired)?.observe()
    }

    /// Fixed test-only ONE session, prepared before exposing this owner to
    /// concurrent callers. Caller holds no owner mutex or migration lease.
    /// No copied catalog/row, implicit reconnect or fixture permit is retained.
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn prepare_image_witness_close_fixture(
        &mut self,
        desired: &DesiredState,
    ) -> Result<(), HostStepError> {
        if self.image_fixture_attempted
            || !matches!(self.close_fixture, Some(CloseFixture::PassiveOwnedLoopback))
        {
            self.revoke_prepared_image();
            return Err(HostStepError::Observation);
        }
        self.image_fixture_attempted = true;
        self.close_fixture = Some(CloseFixture::PreparedImage);
        let result = (|| {
            let config_path = self.paths.config_directory.join("config.yaml");
            let config = read_private_utf8(&config_path, self.uid)
                .map_err(|_| HostStepError::Observation)?;
            let store = read_private_utf8(&self.paths.store, self.uid)
                .map_err(|_| HostStepError::Observation)?;
            let mut observation =
                self.capture_original_close(desired, CloseImageCapture::Witness)?;
            observation.observe()?;
            if observation.fixture_permit().is_some() {
                return Err(HostStepError::Observation);
            }
            let mut prepared = PreparedImageObservation {
                observation,
                desired: desired.clone(),
                config,
                store,
                core_path: self.paths.core.clone(),
                config_path,
                store_path: self.paths.store.clone(),
                controller: self.paths.controller_socket.clone(),
                uid: self.uid,
            };
            self.validate_prepared_image(desired, &mut prepared)?;
            self.prepared_image = Some(prepared);
            Ok(())
        })();
        if result.is_err() {
            self.revoke_prepared_image();
        }
        result
    }

    #[cfg(all(test, feature = "developer-image-witness"))]
    fn validate_prepared_image(
        &mut self,
        desired: &DesiredState,
        prepared: &mut PreparedImageObservation,
    ) -> Result<(), HostStepError> {
        if &prepared.desired != desired
            || !desired.connected
            || self.uid != prepared.uid
            || self.paths.core != prepared.core_path
            || self.paths.controller_socket != prepared.controller
            || self.paths.config_directory.join("config.yaml") != prepared.config_path
            || self.paths.store != prepared.store_path
            || self.profile_id.as_deref() != Some(desired.profile_id.as_str())
            || !self
                .readiness
                .as_ref()
                .is_some_and(|r| r.mode == desired.mode)
            || read_private_utf8(&prepared.config_path, self.uid)
                .ok()
                .as_ref()
                != Some(&prepared.config)
            || read_private_utf8(&self.paths.store, self.uid).ok().as_ref() != Some(&prepared.store)
            || !self
                .core
                .as_mut()
                .is_some_and(|core| prepared.observation.session.prepared_witness_origin(core))
        {
            return Err(HostStepError::Observation);
        }
        Ok(())
    }

    #[cfg(all(test, feature = "developer-image-witness"))]
    fn revoke_prepared_image(&mut self) {
        if matches!(self.close_fixture, Some(CloseFixture::PreparedImage)) {
            if let Some(core) = self.core.as_mut()
                && let Ok(lifetime) = core.conditional_lifetime()
            {
                lifetime.revoke();
            }
            if let Some(prepared) = self.prepared_image.take() {
                prepared.observation.session.revoke_prepared_witness();
            }
        }
    }

    /// Deliberately invalid preparation for a field-presence refusal control.
    /// This never creates helper provenance or qualified package evidence.
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn install_invalid_prepared_image_for_test(
        &mut self,
        desired: &DesiredState,
        observation: CloseObservation,
    ) {
        let config_path = self.paths.config_directory.join("config.yaml");
        self.prepared_image = Some(PreparedImageObservation {
            observation,
            desired: desired.clone(),
            config: read_private_utf8(&config_path, self.uid).unwrap(),
            store: read_private_utf8(&self.paths.store, self.uid).unwrap(),
            core_path: self.paths.core.clone(),
            config_path,
            store_path: self.paths.store.clone(),
            controller: self.paths.controller_socket.clone(),
            uid: self.uid,
        });
        self.image_fixture_attempted = true;
        self.close_fixture = Some(CloseFixture::PreparedImage);
    }

    #[cfg(test)]
    pub(crate) fn owned_rule_close_fixture(
        paths: NativeHostPaths,
        uid: u32,
        core: OwnedCore,
    ) -> Result<Self, HostStepError> {
        let mut host = Self::owned_close_fixture(paths, uid, core)?;
        host.readiness = Some(ConfigReadiness::new(
            crate::desired::RoutingMode::Rule,
            "DIRECT".into(),
        ));
        Ok(host)
    }
    #[cfg(test)]
    pub(crate) fn passive_owned_close_fixture(
        paths: NativeHostPaths,
        uid: u32,
        core: OwnedCore,
    ) -> Result<Self, HostStepError> {
        let mut host = Self::owned_close_fixture(paths, uid, core)?;
        host.close_fixture = Some(CloseFixture::PassiveOwnedLoopback);
        Ok(host)
    }

    pub fn new(paths: NativeHostPaths, uid: u32) -> Result<Self, HostStepError> {
        let all_paths_valid = [
            &paths.core,
            &paths.data_directory,
            &paths.config_directory,
            &paths.runtime_directory,
            &paths.proc_root,
            &paths.sys_class_net,
            &paths.store,
            &paths.template,
            &paths.active_config,
            &paths.staged_config,
            &paths.controller_socket,
        ]
        .into_iter()
        .all(|path| valid_absolute(path));
        if !all_paths_valid
            || !executable(&paths.core)
            || !private_directory(&paths.data_directory, uid)
            || !private_directory(&paths.config_directory, uid)
            || !private_directory(&paths.runtime_directory, uid)
            || !ordinary_directory(&paths.proc_root)
            || !ordinary_directory(&paths.sys_class_net)
        {
            return Err(HostStepError::Prepare);
        }
        Ok(Self {
            paths,
            drop_paths: DropPaths::Cleanup,
            uid,
            core: None,
            core_diagnostics: None,
            profile_id: None,
            readiness: None,
            previous_config: None,
            active_install_attempted: false,
            ping_slot: std::sync::Arc::default(),
            auxiliary: std::sync::Arc::default(),
            tun_identity: None,
            #[cfg(test)]
            close_fixture: None,
            #[cfg(all(test, feature = "developer-image-witness"))]
            prepared_image: None,
            #[cfg(all(test, feature = "developer-image-witness"))]
            image_fixture_attempted: false,
            #[cfg(feature = "developer-image-witness")]
            development_image: DevelopmentImageState::Disabled,
            #[cfg(feature = "product-image-witness")]
            product_image: None,
            #[cfg(all(test, feature = "product-image-witness"))]
            product_preview_fixture: false,
        })
    }

    /// Select data acquisition for ONE installed-development close epoch.
    /// A mode value does not qualify a package, grant a permit, or make RPCs.
    #[cfg(feature = "developer-image-witness")]
    pub(crate) fn new_development_image(
        paths: NativeHostPaths,
        uid: u32,
    ) -> Result<Self, HostStepError> {
        if uid != 1000
            || paths.core != Path::new(crate::managed_pair::RELEASE_CORE)
            || !paths.require_managed_pair
            || paths.managed_pair.is_none()
        {
            return Err(HostStepError::Prepare);
        }
        let mut host = Self::new(paths, uid)?;
        host.development_image = DevelopmentImageState::Available;
        Ok(host)
    }

    pub(crate) fn new_with_image_selection(
        paths: NativeHostPaths,
        uid: u32,
        selection: CloseImageSelection,
    ) -> Result<Self, HostStepError> {
        match selection {
            CloseImageSelection::Direct => Self::new(paths, uid),
            #[cfg(feature = "developer-image-witness")]
            CloseImageSelection::InstalledDevelopment => Self::new_development_image(paths, uid),
            #[cfg(feature = "product-image-witness")]
            CloseImageSelection::Product => Self::new_product_image(paths, uid),
        }
    }

    /// Default-off SOURCE selection. No runtime flag/enrollment writer grants
    /// this mode. The fixed helper independently authenticates its root UID.
    #[cfg(feature = "product-image-witness")]
    pub(crate) fn new_product_image(
        paths: NativeHostPaths,
        uid: u32,
    ) -> Result<Self, HostStepError> {
        if uid == 0
            || uid == u32::MAX
            || paths.core != Path::new(crate::managed_pair::RELEASE_CORE)
            || !paths.require_managed_pair
            || paths.managed_pair.is_none()
        {
            return Err(HostStepError::Prepare);
        }
        let epochs = ProductImageEpochs::new()?;
        let mut host = Self::new(paths, uid)?;
        host.product_image = Some(epochs);
        Ok(host)
    }

    #[cfg(feature = "product-image-witness")]
    fn revoke_product_image(&mut self) {
        if let Some(epochs) = &mut self.product_image {
            epochs.revoke_if_used();
        }
    }

    #[must_use]
    pub fn core_pid(&self) -> Option<u32> {
        self.core.as_ref().and_then(OwnedCore::pid)
    }

    pub(crate) fn capture_connection_close(
        &mut self,
        desired: &DesiredState,
    ) -> Result<CloseObservation, HostStepError> {
        #[cfg(feature = "product-image-witness")]
        if let Some(epochs) = &mut self.product_image {
            epochs.reserve()?;
            #[cfg(test)]
            let observation = if self.product_preview_fixture {
                let mut observation =
                    self.capture_original_close(desired, CloseImageCapture::Direct)?;
                let image = observation.session().original_image_for_test();
                observation
                    .session_mut()
                    .install_image_probe_for_test(Box::new(move |_| {
                        Ok(image.try_clone().unwrap())
                    }));
                observation
                    .session_mut()
                    .install_image_finish_probe_for_test(Box::new(|_| Ok(())));
                observation
            } else {
                self.capture_original_close(desired, CloseImageCapture::ProductWitness)?
            };
            #[cfg(not(test))]
            let observation =
                self.capture_original_close(desired, CloseImageCapture::ProductWitness)?;
            self.product_image
                .as_mut()
                .ok_or(HostStepError::Observation)?
                .captured(observation.session().cancellation());
            return Ok(observation);
        }
        #[cfg(all(test, feature = "developer-image-witness"))]
        if matches!(self.close_fixture, Some(CloseFixture::PreparedImage)) {
            let Some(mut prepared) = self.prepared_image.take() else {
                self.revoke_prepared_image();
                return Err(HostStepError::Observation);
            };
            if self
                .validate_prepared_image(desired, &mut prepared)
                .is_err()
            {
                self.revoke_prepared_image();
                prepared.observation.session.revoke_prepared_witness();
                return Err(HostStepError::Observation);
            }
            return Ok(prepared.observation);
        }
        #[cfg(feature = "developer-image-witness")]
        match self.development_image {
            DevelopmentImageState::Available => {
                // Consume BEFORE originals/constructor effects; failed or
                // cancelled discovery never silently reconnects or falls back.
                self.development_image = DevelopmentImageState::Consumed;
                return self
                    .capture_original_close(desired, CloseImageCapture::InstalledRuntimeWitness);
            }
            DevelopmentImageState::Consumed => return Err(HostStepError::Observation),
            DevelopmentImageState::Disabled => (),
        }
        self.capture_original_close(desired, CloseImageCapture::Direct)
    }

    fn capture_original_close(
        &mut self,
        desired: &DesiredState,
        image: CloseImageCapture,
    ) -> Result<CloseObservation, HostStepError> {
        if !desired.connected || self.profile_id.as_deref() != Some(desired.profile_id.as_str()) {
            return Err(HostStepError::Observation);
        }
        let readiness = self
            .readiness
            .as_ref()
            .filter(|expected| expected.mode == desired.mode)
            .ok_or(HostStepError::Observation)?
            .clone();
        let core = self.core.as_mut().ok_or(HostStepError::Observation)?;
        let pid = core.pid().ok_or(HostStepError::Observation)?;
        let mut session = crate::conditional_close_candidate::Session::bind(core, self.uid)
            .map_err(|_| HostStepError::Observation)?;
        match image {
            CloseImageCapture::Direct => session.capture_executable(&self.paths.core),
            #[cfg(all(test, feature = "developer-image-witness"))]
            CloseImageCapture::Witness => session.capture_executable_via_witness(&self.paths.core),
            #[cfg(feature = "developer-image-witness")]
            CloseImageCapture::InstalledRuntimeWitness => {
                session.capture_executable_via_runtime_witness(&self.paths.core)
            }
            #[cfg(feature = "product-image-witness")]
            CloseImageCapture::ProductWitness => {
                session.capture_executable_via_product_witness(&self.paths.core)
            }
        }
        .map_err(|_| HostStepError::Observation)?;
        Ok(CloseObservation {
            session,
            facts: CloseFacts {
                readiness,
                proc_root: self.paths.proc_root.clone(),
                sys_class_net: self.paths.sys_class_net.clone(),
                config_directory: self.paths.config_directory.clone(),
                uid: self.uid,
                tun_identity: self.tun_identity.clone(),
                auxiliary: self.auxiliary.clone(),
                pid,
                #[cfg(test)]
                fixture: self.close_fixture,
            },
        })
    }

    /// Startup only, while canonical runtime and migration ownership are held.
    /// No directory is removed unless cores and our configured TUN scope are empty.
    pub(crate) fn cleanup_probe_orphans(&self) -> Result<(), HostStepError> {
        crate::probe_executor::cleanup_orphans(&self.paths.runtime_directory, || {
            self.core.is_none()
                && processes_named_strict(&self.paths.proc_root, "mihomo")
                    .is_ok_and(|pids| pids.is_empty())
                && self.managed_tuns().is_ok_and(|count| count == 0)
        })
        .map(|_| ())
        .map_err(|_| HostStepError::Cleanup)
    }

    fn remove_controller(&self) -> Result<(), HostStepError> {
        remove_owned_file(&self.paths.controller_socket, self.uid, true)
    }

    fn read_previous_config(&self) -> Result<Option<Vec<u8>>, HostStepError> {
        match fs::symlink_metadata(&self.paths.active_config) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(HostStepError::Prepare),
            Ok(_) => read_private_utf8(&self.paths.active_config, self.uid)
                .map(String::into_bytes)
                .map(Some)
                .map_err(|_| HostStepError::Prepare),
        }
    }

    fn restore_previous_config(&mut self) -> Result<(), HostStepError> {
        let Some(previous) = self.previous_config.as_ref() else {
            self.active_install_attempted = false;
            return Ok(());
        };
        match previous {
            Some(payload) => {
                atomic_replace_private(&self.paths.active_config, payload, self.uid)
                    .map_err(|_| HostStepError::Cleanup)?;
            }
            None => remove_owned_file(&self.paths.active_config, self.uid, false)?,
        }
        self.previous_config = None;
        self.active_install_attempted = false;
        Ok(())
    }

    fn visible_core_count(
        &self,
        own_pid: Option<u32>,
        own_running: bool,
    ) -> Result<u8, HostStepError> {
        let named = processes_named_strict(&self.paths.proc_root, "mihomo")
            .map_err(|_| HostStepError::Observation)?;
        let mut count = named.len();
        let auxiliary = self
            .auxiliary
            .verified_pid()
            .map_err(|_| HostStepError::Observation)?;
        if auxiliary.is_some_and(|pid| named.contains(&pid)) {
            count -= 1;
        }
        if own_running && own_pid.is_some_and(|pid| !named.contains(&pid)) {
            count = count.saturating_add(1);
        }
        Ok(u8::try_from(count).unwrap_or(u8::MAX))
    }

    fn configured_devices(
        &self,
    ) -> Result<Option<std::collections::BTreeSet<String>>, HostStepError> {
        let Some(mut devices) =
            crate::tun_scope::configured_devices(&self.paths.config_directory, self.uid)?
        else {
            return Ok(None);
        };
        if let Some((name, _)) = &self.tun_identity {
            devices.insert(name.clone());
        }
        Ok(Some(devices))
    }

    fn managed_tuns(&self) -> Result<u8, HostStepError> {
        let inventory = crate::tun_scope::inventory(&self.paths.sys_class_net)?;
        if self.core.is_some()
            && let Some((name, pinned)) = &self.tun_identity
            && inventory.contains(name)
            && crate::tun_scope::device_index(&self.paths.sys_class_net, name)? != *pinned
        {
            return Err(HostStepError::Observation);
        }
        let count = match self.configured_devices()? {
            Some(devices) => inventory.intersection(&devices).count(),
            None => inventory.len(),
        };
        u8::try_from(count).map_err(|_| HostStepError::Observation)
    }

    fn verify_tun(&mut self, pid: u32) -> Result<bool, HostStepError> {
        let payload = crate::core_selector::read_configuration(
            &self.paths.controller_socket,
            pid,
            omavless_mihomo::ReadOnlyEndpoint::Configs,
            Instant::now() + OBSERVATION_TIMEOUT,
        )
        .ok_or(HostStepError::Observation)?;
        // A verified no-TUN core can have a ready controller. Keep its TUN
        // count zero: lifecycle adoption still requires one verified device.
        // Never use a disabled core to adopt a retained/foreign interface.
        if payload["tun"]["enable"] == false {
            return Ok(self.tun_identity.is_none() && self.managed_tuns()? == 0);
        }
        let Some(device) = crate::traffic::controller_device(&payload) else {
            return Ok(false);
        };
        if self
            .configured_devices()?
            .is_some_and(|names| !names.contains(device))
        {
            return Ok(false);
        }
        let index = crate::tun_scope::device_index(&self.paths.sys_class_net, device)?;
        if let Some((name, pinned)) = &self.tun_identity {
            return Ok(name == device && *pinned == index);
        }
        self.tun_identity = Some((device.to_owned(), index));
        Ok(true)
    }

    fn owned_connections_payload(
        &mut self,
        desired: &DesiredState,
    ) -> Result<serde_json::Value, HostStepError> {
        let deadline = Instant::now() + Duration::from_millis(750);
        if !desired.connected {
            return Err(HostStepError::Observation);
        }
        let valid = |facts: NativeLocalObservation| {
            facts.owned_core_running
                && facts.visible_mihomo_count == 1 + facts.owned_auxiliary_mihomo_count
                && facts.managed_tun_count == 1
                && facts.owned_controller_config_verified
                && facts.desired_profile_matches_owned
        };
        if !valid(self.fresh_observation(desired)?) {
            return Err(HostStepError::Observation);
        }
        let pid = self.core_pid().ok_or(HostStepError::Observation)?;
        // Private 0700 parent, same-UID exact owned PID authentication, 512-KiB
        // response bound, and one whole-exchange deadline are reused.
        let payload = crate::core_selector::read_configuration(
            &self.paths.controller_socket,
            pid,
            omavless_mihomo::ReadOnlyEndpoint::Connections,
            deadline,
        )
        .ok_or(HostStepError::Observation)?;
        if !valid(self.fresh_observation(desired)?)
            || Instant::now() >= deadline
            || !self
                .core
                .as_mut()
                .is_some_and(|c| c.pid() == Some(pid) && c.running().unwrap_or(false))
        {
            return Err(HostStepError::Observation);
        }
        Ok(payload)
    }
}

impl LifecycleHost for NativeLifecycleHost {
    #[cfg(feature = "developer-conditional-close")]
    fn close_registration(&self) -> crate::lifecycle::CloseRegistration {
        use crate::lifecycle::CloseRegistration;
        #[cfg(feature = "product-image-witness")]
        {
            if self.product_image.is_some() {
                return CloseRegistration::Product;
            }
            if !matches!(self.development_image, DevelopmentImageState::Disabled) {
                return CloseRegistration::Developer;
            }
            #[cfg(test)]
            if self.close_fixture.is_some() {
                return CloseRegistration::Developer;
            }
            CloseRegistration::Disabled
        }
        #[cfg(not(feature = "product-image-witness"))]
        {
            CloseRegistration::Developer
        }
    }
    #[cfg(feature = "product-image-witness")]
    fn matches_close_retirement(&self, original: &CloseObservation) -> bool {
        self.product_image.as_ref().is_some_and(|epochs| {
            !epochs.poisoned
                && epochs
                    .active
                    .as_ref()
                    .is_some_and(|expected| expected.same_epoch(&original.session().cancellation()))
        })
    }
    #[cfg(feature = "product-image-witness")]
    fn complete_close_retirement(
        &mut self,
        original: &crate::conditional_close_candidate::CloseEpochRetirement,
    ) {
        if let Some(epochs) = &mut self.product_image {
            if !epochs.poisoned
                && epochs
                    .active
                    .as_ref()
                    .is_some_and(|expected| original.admits(expected))
            {
                epochs.active = None;
            } else {
                epochs.revoke_if_used();
            }
        }
    }
    #[cfg(feature = "product-image-witness")]
    fn refuse_close_epoch(&mut self) {
        self.revoke_product_image();
    }
    #[cfg(feature = "product-image-witness")]
    fn close_epoch_admission(&self) -> crate::lifecycle::CloseEpochAdmission {
        self.product_image.as_ref().map_or(
            crate::lifecycle::CloseEpochAdmission::Legacy,
            ProductImageEpochs::admission,
        )
    }
    #[cfg(feature = "product-image-witness")]
    fn complete_close_epoch(
        &mut self,
        original: &crate::conditional_close_candidate::CloseEpochCompletion,
    ) {
        if let Some(epochs) = &mut self.product_image {
            epochs.complete(original);
        }
    }
    fn capture_connection_close(
        &mut self,
        desired: &DesiredState,
    ) -> Result<CloseObservation, HostStepError> {
        NativeLifecycleHost::capture_connection_close(self, desired)
    }
    fn core_diagnostics(&self) -> Option<crate::core_diagnostics::CoreDiagnostics> {
        self.core_diagnostics
            .as_ref()
            .map(|reader| reader.snapshot())
    }
    fn core_log_hints(&self) -> Option<crate::core_diagnostics::CoreLogHints> {
        self.core_diagnostics.as_ref().map(|reader| reader.hints())
    }
    fn support_facts(&self, connected: bool) -> Option<crate::lifecycle::HostSupportFacts> {
        Some(crate::support_diagnostics::collect_host(
            &self.paths,
            self.uid,
            connected,
        ))
    }
    fn auxiliary_slot(&self) -> Option<std::sync::Arc<crate::auxiliary_core::AuxiliarySlot>> {
        Some(std::sync::Arc::clone(&self.auxiliary))
    }
    fn probe_paths(&self) -> Option<(PathBuf, PathBuf)> {
        Some((
            self.paths.core.clone(),
            self.paths.runtime_directory.clone(),
        ))
    }
    fn ping_binding(
        &mut self,
        desired: &DesiredState,
        deadline: Instant,
    ) -> Result<crate::tun_ping::Binding, HostStepError> {
        let deadline = deadline.min(Instant::now() + Duration::from_millis(500));
        if Instant::now() >= deadline || !desired.connected {
            return Err(HostStepError::Observation);
        }
        let facts = self.fresh_observation(desired)?;
        if !facts.owned_core_running
            || facts.visible_mihomo_count != 1 + facts.owned_auxiliary_mihomo_count
            || facts.managed_tun_count != 1
            || !facts.owned_controller_config_verified
            || !facts.desired_profile_matches_owned
        {
            return Err(HostStepError::Observation);
        }
        let pid = self.core_pid().ok_or(HostStepError::Observation)?;
        let reported = crate::core_selector::read_configuration(
            &self.paths.controller_socket,
            pid,
            omavless_mihomo::ReadOnlyEndpoint::Configs,
            deadline,
        )
        .ok_or(HostStepError::Observation)?;
        let device =
            crate::traffic::controller_device(&reported).ok_or(HostStepError::Observation)?;
        let sample =
            crate::traffic::read_device(&self.paths.sys_class_net, pid, desired.generation, device)
                .ok_or(HostStepError::Observation)?;
        let after = crate::core_selector::read_configuration(
            &self.paths.controller_socket,
            pid,
            omavless_mihomo::ReadOnlyEndpoint::Configs,
            deadline,
        )
        .ok_or(HostStepError::Observation)?;
        if crate::traffic::controller_device(&after) != Some(device)
            || Instant::now() >= deadline
            || !self
                .core
                .as_mut()
                .is_some_and(|c| c.pid() == Some(pid) && c.running().unwrap_or(false))
        {
            return Err(HostStepError::Observation);
        }
        Ok(crate::tun_ping::Binding {
            device: device.to_owned(),
            identity: sample.identity,
            slot: std::sync::Arc::clone(&self.ping_slot),
            epoch: self.ping_slot.epoch().ok_or(HostStepError::Observation)?,
        })
    }
    fn traffic_counters(
        &mut self,
        desired: &DesiredState,
    ) -> Result<crate::traffic::TrafficCounters, HostStepError> {
        let deadline = Instant::now() + Duration::from_millis(500);
        if !desired.connected {
            return Err(HostStepError::Observation);
        }
        let valid = |facts: NativeLocalObservation| {
            facts.owned_core_running
                && facts.visible_mihomo_count == 1 + facts.owned_auxiliary_mihomo_count
                && facts.managed_tun_count == 1
                && facts.owned_controller_config_verified
                && facts.desired_profile_matches_owned
        };
        if !valid(self.fresh_observation(desired)?) {
            return Err(HostStepError::Observation);
        }
        let pid = self.core_pid().ok_or(HostStepError::Observation)?;
        // File capabilities deliberately make the core's fdinfo unreadable to
        // the user runtime. Do not change dumpability or guess the sole TUN.
        // Ask the exact PID-authenticated private controller for its device.
        let reported = crate::core_selector::read_configuration(
            &self.paths.controller_socket,
            pid,
            omavless_mihomo::ReadOnlyEndpoint::Configs,
            deadline,
        )
        .ok_or(HostStepError::Observation)?;
        let device =
            crate::traffic::controller_device(&reported).ok_or(HostStepError::Observation)?;
        let sample =
            crate::traffic::read_device(&self.paths.sys_class_net, pid, desired.generation, device)
                .ok_or(HostStepError::Observation)?;
        let after = crate::core_selector::read_configuration(
            &self.paths.controller_socket,
            pid,
            omavless_mihomo::ReadOnlyEndpoint::Configs,
            deadline,
        )
        .ok_or(HostStepError::Observation)?;
        if crate::traffic::controller_device(&after) != Some(device) {
            return Err(HostStepError::Observation);
        }
        let core_alive = self
            .core
            .as_mut()
            .is_some_and(|core| core.pid() == Some(pid) && core.running().unwrap_or(false));
        if !core_alive || Instant::now() >= deadline {
            return Err(HostStepError::Observation);
        }
        Ok(sample)
    }
    fn active_connection_count(&mut self, desired: &DesiredState) -> Result<u32, HostStepError> {
        let payload = self.owned_connections_payload(desired)?;
        crate::connections_summary::count(&payload).ok_or(HostStepError::Observation)
    }
    fn active_connection_overview(
        &mut self,
        desired: &DesiredState,
    ) -> Result<crate::connection_overview::ConnectionOverview, HostStepError> {
        let payload = self.owned_connections_payload(desired)?;
        crate::connection_overview::aggregate(&payload).ok_or(HostStepError::Observation)
    }
    fn active_connection_rows(
        &mut self,
        desired: &DesiredState,
    ) -> Result<crate::connection_rows::ConnectionRows, HostStepError> {
        let payload = self.owned_connections_payload(desired)?;
        crate::connection_rows::extract(&payload).ok_or(HostStepError::Observation)
    }
    fn fresh_observation(
        &mut self,
        desired: &DesiredState,
    ) -> Result<NativeLocalObservation, HostStepError> {
        desired.validate().map_err(|_| HostStepError::Observation)?;
        let named = processes_named_strict(&self.paths.proc_root, "mihomo")
            .map_err(|_| HostStepError::Observation)?;
        let auxiliary = self
            .auxiliary
            .verified_pid()
            .map_err(|_| HostStepError::Observation)?;
        let tun = tun_interface_count_strict(&self.paths.sys_class_net)
            .map_err(|_| HostStepError::Observation)?;
        let (pid, running) = match self.core.as_mut() {
            Some(core) => (
                core.pid(),
                core.running().map_err(|_| HostStepError::Observation)?,
            ),
            None => (None, false),
        };
        let profile_matches = running
            && desired.connected
            && self.profile_id.as_deref() == Some(desired.profile_id.as_str());
        let verified = profile_matches
            && self.readiness.as_ref().is_some_and(|expected| {
                expected.mode == desired.mode
                    && pid.is_some_and(|pid| {
                        expected.ready_for_pid(
                            &self.paths.controller_socket,
                            pid,
                            Instant::now() + OBSERVATION_TIMEOUT,
                        )
                    })
            });
        if verified
            && self.tun_identity.is_some()
            && !self.verify_tun(pid.ok_or(HostStepError::Observation)?)?
        {
            return Err(HostStepError::Observation);
        }
        let (after_pid, after_running) = match self.core.as_mut() {
            Some(core) => (
                core.pid(),
                core.running().map_err(|_| HostStepError::Observation)?,
            ),
            None => (None, false),
        };
        if (pid, running) != (after_pid, after_running)
            || self
                .auxiliary
                .verified_pid()
                .map_err(|_| HostStepError::Observation)?
                != auxiliary
            || processes_named_strict(&self.paths.proc_root, "mihomo")
                .map_err(|_| HostStepError::Observation)?
                != named
            || tun_interface_count_strict(&self.paths.sys_class_net)
                .map_err(|_| HostStepError::Observation)?
                != tun
        {
            return Err(HostStepError::Observation);
        }
        Ok(NativeLocalObservation {
            owned_core_running: running,
            visible_mihomo_count: u8::try_from(named.len())
                .map_err(|_| HostStepError::Observation)?,
            owned_auxiliary_mihomo_count: u8::from(
                auxiliary.is_some_and(|pid| named.contains(&pid)),
            ),
            visible_tun_count: tun,
            managed_tun_count: self.managed_tuns()?,
            owned_controller_config_verified: verified,
            desired_profile_matches_owned: profile_matches,
        })
    }
    fn route_core_identity(&mut self) -> Option<(u32, [u8; 32])> {
        use sha2::{Digest, Sha256};
        let core = self.core.as_mut()?;
        if !core.running().ok()? {
            return None;
        }
        let pid = core.pid()?;
        let config = self.read_previous_config().ok()??;
        Some((pid, Sha256::digest(config).into()))
    }
    fn validate_startup(&mut self, desired: &DesiredState) -> Result<(), HostStepError> {
        self.connection_preflight()?;
        crate::startup_validation::validate(&self.paths, self.uid, desired)
    }
    fn connection_preflight(&mut self) -> Result<(), HostStepError> {
        if !self.paths.require_managed_pair {
            return Ok(());
        }
        self.paths
            .managed_pair
            .as_ref()
            .ok_or(HostStepError::Prepare)?
            .verify()
    }
    fn observe(&mut self, desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        let (own_pid, own_running, controller_ready) = match self.core.as_mut() {
            Some(core) => {
                let running = core.running().map_err(|_| HostStepError::Observation)?;
                let ready = if running {
                    self.readiness.as_ref().is_some_and(|expected| {
                        expected.mode == desired.mode
                            && core.configured_ready(OBSERVATION_TIMEOUT, expected)
                    })
                } else {
                    false
                };
                (core.pid(), running, ready)
            }
            None => (None, false, false),
        };
        let tun_verified = if own_running && controller_ready {
            self.verify_tun(own_pid.ok_or(HostStepError::Observation)?)?
        } else {
            false
        };
        Ok(OwnedObservation {
            service_active: own_running,
            controller_ready: controller_ready && tun_verified,
            core_count: self.visible_core_count(own_pid, own_running)?,
            tun_count: self.managed_tuns()?,
            active_profile_matches: own_running
                && controller_ready
                && self.profile_id.as_deref() == Some(desired.profile_id.as_str()),
        })
    }

    fn prepare(&mut self, desired: &DesiredState) -> Result<(), HostStepError> {
        #[cfg(feature = "product-image-witness")]
        self.revoke_product_image();
        #[cfg(all(test, feature = "developer-image-witness"))]
        self.revoke_prepared_image();
        self.connection_preflight()?;
        if let Some(pair) = &self.paths.managed_pair {
            pair.verify()?;
        }
        if !self.auxiliary.mutation_safe() {
            return Err(HostStepError::Prepare);
        }
        if self.core.is_some() || self.profile_id.is_some() {
            return Err(HostStepError::Prepare);
        }
        remove_owned_file(&self.paths.staged_config, self.uid, false)?;
        self.previous_config = Some(self.read_previous_config()?);
        self.active_install_attempted = false;
        let store_text =
            read_private_utf8(&self.paths.store, self.uid).map_err(|_| HostStepError::Prepare)?;
        let template = read_private_utf8(&self.paths.template, self.uid)
            .map_err(|_| HostStepError::Prepare)?;
        if template.len() > MAX_TEMPLATE_BYTES {
            return Err(HostStepError::Prepare);
        }
        let store = parse_private_store(&store_text).map_err(|_| HostStepError::Prepare)?;
        let profile_name = store
            .list_projection()
            .profiles()
            .iter()
            .find(|profile| profile.id() == desired.profile_id)
            .ok_or(HostStepError::Prepare)?
            .name()
            .to_owned();
        let controller = self
            .paths
            .controller_socket
            .to_str()
            .ok_or(HostStepError::Prepare)?;
        // Presets and portable archives store policy DATA, not this machine's
        // DNS enrollment. Apply selected-host flags only to the rendered copy.
        let template =
            crate::managed_template::for_host(&template, self.paths.managed_pair.is_some())
                .ok_or(HostStepError::Prepare)?;
        let config = store
            .prepare_config_mode(
                &desired.profile_id,
                &template,
                controller,
                desired.mode.as_str(),
            )
            .map_err(|_| HostStepError::Prepare)?;
        let readiness = ConfigReadiness::from_generated_config(desired.mode, profile_name, &config)
            .ok_or(HostStepError::Prepare)?;
        if self.paths.managed_pair.is_some() && !readiness.managed_dns() {
            return Err(HostStepError::Prepare);
        }
        atomic_replace_private(&self.paths.staged_config, config.as_bytes(), self.uid)
            .map_err(|_| HostStepError::Prepare)?;
        if validate_config(
            &self.paths.core,
            &self.paths.data_directory,
            &self.paths.staged_config,
            VALIDATION_TIMEOUT,
        )
        .is_err()
        {
            let _ = remove_owned_file(&self.paths.staged_config, self.uid, false);
            return Err(HostStepError::Prepare);
        }
        self.profile_id = Some(desired.profile_id.clone());
        self.readiness = Some(readiness);
        Ok(())
    }

    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        #[cfg(feature = "product-image-witness")]
        self.revoke_product_image();
        #[cfg(all(test, feature = "developer-image-witness"))]
        self.revoke_prepared_image();
        if !self.auxiliary.mutation_safe() {
            return Err(HostStepError::Start);
        }
        if !self.ping_slot.revoke() {
            return Err(HostStepError::Start);
        }
        if self.core.is_some() || self.profile_id.is_none() {
            return Err(HostStepError::Start);
        }
        // Refuse an existing configured device before the child can touch it.
        // This is a collision, never permission to delete or adopt a foreign TUN.
        if self.managed_tuns()? != 0 {
            return Err(HostStepError::Start);
        }
        if let Some(names) = self.configured_devices()? {
            for name in names {
                match fs::symlink_metadata(self.paths.sys_class_net.join(name)) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    _ => return Err(HostStepError::Start),
                }
            }
        }
        self.tun_identity = None;
        self.remove_controller()?;
        let mut core = OwnedCore::spawn(
            &self.paths.core,
            &self.paths.data_directory,
            &self.paths.staged_config,
            &self.paths.controller_socket,
        )
        .map_err(|_| HostStepError::Start)?;
        self.core_diagnostics = Some(core.diagnostic_reader());
        let expected = self.readiness.as_ref().ok_or(HostStepError::Start)?;
        let ready = core.wait_configured(expected.startup_timeout(), expected);
        let private_controller = ready.is_ok()
            && core.pid().is_some_and(|pid| {
                crate::controller_permissions::secure_owned(
                    &self.paths.controller_socket,
                    pid,
                    self.uid,
                )
            })
            && core.running().unwrap_or(false);
        self.core = Some(core);
        if private_controller {
            Ok(())
        } else {
            Err(HostStepError::Start)
        }
    }

    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        #[cfg(feature = "product-image-witness")]
        self.revoke_product_image();
        #[cfg(all(test, feature = "developer-image-witness"))]
        self.revoke_prepared_image();
        if self.core.is_none() || self.profile_id.is_none() {
            return Err(HostStepError::Commit);
        }
        let config = read_private_utf8(&self.paths.staged_config, self.uid)
            .map_err(|_| HostStepError::Commit)?;
        remove_owned_file(&self.paths.staged_config, self.uid, false)
            .map_err(|_| HostStepError::Commit)?;
        self.active_install_attempted = true;
        if atomic_replace_private(&self.paths.active_config, config.as_bytes(), self.uid).is_err() {
            let _ = self.restore_previous_config();
            return Err(HostStepError::Commit);
        }
        self.previous_config = None;
        self.active_install_attempted = false;
        Ok(())
    }

    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        #[cfg(feature = "product-image-witness")]
        self.revoke_product_image();
        #[cfg(all(test, feature = "developer-image-witness"))]
        self.revoke_prepared_image();
        if !self.auxiliary.mutation_safe() {
            return Err(HostStepError::Stop);
        }
        if !self.ping_slot.revoke() {
            return Err(HostStepError::Stop);
        }
        let timeout = self
            .readiness
            .as_ref()
            .map_or(STOP_TIMEOUT, ConfigReadiness::stop_timeout);
        if let Some(mut core) = self.core.take()
            && core.stop(timeout).is_err()
        {
            self.core = Some(core);
            return Err(HostStepError::Stop);
        }
        self.remove_controller().map_err(|_| HostStepError::Stop)?;
        self.profile_id = None;
        self.readiness = None;
        Ok(())
    }

    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        #[cfg(feature = "product-image-witness")]
        self.revoke_product_image();
        #[cfg(all(test, feature = "developer-image-witness"))]
        self.revoke_prepared_image();
        remove_owned_file(&self.paths.staged_config, self.uid, false)?;
        if self.active_install_attempted {
            self.restore_previous_config()?;
        } else {
            self.previous_config = None;
        }
        if self.core.is_none() {
            self.profile_id = None;
            self.readiness = None;
        }
        Ok(())
    }
}

impl Drop for NativeLifecycleHost {
    fn drop(&mut self) {
        let _auxiliary_guard = self.auxiliary.quiesce();
        // Shutdown has no successor TUN. Cleanup is best effort in Drop;
        // ordinary stop/start instead refuse if synchronous reaping is unproven.
        let _ = self.ping_slot.revoke();
        if let Some(mut core) = self.core.take() {
            let timeout = self
                .readiness
                .as_ref()
                .map_or(STOP_TIMEOUT, ConfigReadiness::stop_timeout);
            let _ = core.stop(timeout);
        }
        if matches!(self.drop_paths, DropPaths::Cleanup) {
            let _ = self.remove_controller();
            let _ = remove_owned_file(&self.paths.staged_config, self.uid, false);
        }
        if self.active_install_attempted {
            let _ = self.restore_previous_config();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(feature = "product-image-witness")]
    #[test]
    fn product_compiled_ordinary_host_registration_is_disabled_until_explicit_selection() {
        use crate::lifecycle::CloseRegistration;
        let (root, mut host) = observation_fixture();
        assert!(host.close_registration() == CloseRegistration::Disabled);
        host.development_image = DevelopmentImageState::Available;
        assert!(host.close_registration() == CloseRegistration::Developer);
        host.development_image = DevelopmentImageState::Disabled;
        host.product_image = Some(ProductImageEpochs::new().unwrap());
        assert!(host.close_registration() == CloseRegistration::Product);
        // Selection publishes implementation only, never acquires/enrolls or
        // constructs a proof/current-image/conditional effect permission.
        assert_eq!(host.product_history_for_test(), (0, 0));
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature = "product-image-witness")]
    #[test]
    fn product_epoch_reservation_capacity_and_mutation_refusal_precede_acquisition() {
        use crate::lifecycle::CloseEpochAdmission;
        let mut epochs = ProductImageEpochs::new().unwrap();
        assert!(epochs.history.capacity() >= ProductImageEpochs::LIMIT);
        assert!(epochs.admission() == CloseEpochAdmission::Ready);
        // Initial ordinary connection setup does not spend a close epoch.
        epochs.revoke_if_used();
        assert!(epochs.admission() == CloseEpochAdmission::Ready);
        epochs.reserve().unwrap();
        assert_eq!(epochs.issued, 1);
        assert!(epochs.admission() == CloseEpochAdmission::Refused);
        assert!(epochs.reserve().is_err());
        assert_eq!(epochs.issued, 1);
        assert!(epochs.history.is_empty());
        let mut full = ProductImageEpochs::new().unwrap();
        full.issued = ProductImageEpochs::LIMIT;
        assert!(full.reserve().is_err());
        assert_eq!(full.issued, ProductImageEpochs::LIMIT);
        assert!(full.history.is_empty());
    }

    fn root(label: &str) -> (PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-native-host-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn executable_at(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn observation_fixture() -> (PathBuf, NativeLifecycleHost) {
        let root = crate::test_temp::directory("fresh-native").unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        for name in ["data", "config", "runtime", "proc", "sys"] {
            fs::create_dir(root.join(name)).unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
        }
        executable_at(&root.join("core"));
        let paths = NativeHostPaths::new(
            root.join("core"),
            root.join("data"),
            root.join("config"),
            root.join("runtime"),
            root.join("proc"),
            root.join("sys"),
        );
        let host = NativeLifecycleHost::new(paths, uid).unwrap();
        (root, host)
    }

    #[cfg(feature = "developer-image-witness")]
    #[test]
    fn installed_image_selection_is_not_proof_and_failure_spends_the_epoch() {
        let (root, mut host) = observation_fixture();
        assert!(matches!(
            host.development_image,
            DevelopmentImageState::Disabled
        ));
        host.development_image = DevelopmentImageState::Available;
        // No original child/package exists. Selection cannot adopt one or
        // mint a permit, and a failed first capture cannot use DirectProc.
        assert!(
            host.capture_connection_close(&DesiredState::default())
                .is_err()
        );
        assert!(matches!(
            host.development_image,
            DevelopmentImageState::Consumed
        ));
        assert!(
            host.capture_connection_close(&DesiredState::default())
                .is_err()
        );
        assert!(host.core.is_none());
        assert!(!host.paths.staged_config.exists());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature = "developer-image-witness")]
    #[test]
    fn installed_image_constructor_refuses_unqualified_caller_paths_and_uid() {
        let (root, host) = observation_fixture();
        let paths = NativeHostPaths::new(
            host.paths.core.clone(),
            host.paths.data_directory.clone(),
            host.paths.config_directory.clone(),
            host.paths.runtime_directory.clone(),
            host.paths.proc_root.clone(),
            host.paths.sys_class_net.clone(),
        );
        assert!(matches!(
            NativeLifecycleHost::new_development_image(paths, 1000),
            Err(HostStepError::Prepare)
        ));
        let paths = NativeHostPaths::new(
            PathBuf::from(crate::managed_pair::RELEASE_CORE),
            host.paths.data_directory.clone(),
            host.paths.config_directory.clone(),
            host.paths.runtime_directory.clone(),
            host.paths.proc_root.clone(),
            host.paths.sys_class_net.clone(),
        );
        assert!(matches!(
            NativeLifecycleHost::new_development_image(paths, 1001),
            Err(HostStepError::Prepare)
        ));
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn observation_only_host_preserves_unowned_names_and_refuses_all_effects() {
        use std::os::unix::net::{UnixListener, UnixStream};
        for refuse in [false, true] {
            let (root, initial) = observation_fixture();
            drop(initial);
            let uid = nix::unistd::Uid::current().as_raw();
            let paths = NativeHostPaths::new(
                root.join("core"),
                root.join("data"),
                root.join("config"),
                root.join("runtime"),
                root.join("proc"),
                root.join("sys"),
            );
            let staged = paths.staged_config.clone();
            let controller = paths.controller_socket.clone();
            fs::write(&staged, b"synthetic-unowned-staging").unwrap();
            let staged_identity = fs::metadata(&staged).unwrap();
            let listener = UnixListener::bind(&controller).unwrap();
            let socket_identity = fs::symlink_metadata(&controller).unwrap();
            let mut host = ObservationOnlyNativeHost::new(paths, uid).unwrap();
            assert!(host.auxiliary_slot().is_none());
            assert!(host.probe_paths().is_none());
            assert!(host.route_core_identity().is_none());
            assert!(host.connection_preflight().is_err());
            assert!(host.prepare(&DesiredState::default()).is_err());
            assert!(host.validate_startup(&DesiredState::default()).is_err());
            assert!(host.start_prepared().is_err());
            assert!(host.commit_prepared().is_err());
            assert!(host.stop_owned().is_err());
            assert!(host.discard_prepared().is_err());
            if refuse {
                fs::remove_dir(root.join("proc")).unwrap();
                assert!(host.fresh_observation(&DesiredState::default()).is_err());
            } else {
                assert!(
                    !host
                        .fresh_observation(&DesiredState::default())
                        .unwrap()
                        .owned_core_running
                );
                assert_eq!(
                    host.observe(&DesiredState::default()).unwrap().core_count,
                    0
                );
            }
            drop(host);
            assert_eq!(fs::read(&staged).unwrap(), b"synthetic-unowned-staging");
            assert!(crate::restore_staging_candidate::same_member(
                &staged_identity,
                &fs::metadata(&staged).unwrap()
            ));
            assert!(crate::restore_staging_candidate::same_member(
                &socket_identity,
                &fs::symlink_metadata(&controller).unwrap()
            ));
            let client = UnixStream::connect(&controller).unwrap();
            let peer = listener.accept().unwrap().0;
            drop((client, peer, listener));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn ordinary_host_drop_keeps_existing_path_cleanup_policy() {
        use std::os::unix::net::UnixListener;
        let (root, host) = observation_fixture();
        let staged = host.paths.staged_config.clone();
        let controller = host.paths.controller_socket.clone();
        fs::write(&staged, b"synthetic-staging").unwrap();
        let listener = UnixListener::bind(&controller).unwrap();
        drop(host);
        assert!(!staged.exists());
        assert!(!controller.exists());
        drop(listener);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_pair_required_host_refuses_legacy_path_before_staging() {
        let (root, mut host) = observation_fixture();
        host.paths.require_managed_pair = true;
        assert_eq!(host.connection_preflight(), Err(HostStepError::Prepare));
        assert_eq!(
            host.prepare(&DesiredState::default()),
            Err(HostStepError::Prepare)
        );
        assert!(!host.paths.staged_config.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn orphan_probe_cleanup_requires_strict_empty_host() {
        let (root, host) = observation_fixture();
        let orphan = host.paths.runtime_directory.join("probe-2147483647-0");
        assert!(!Path::new("/proc/2147483647").exists());
        fs::create_dir(&orphan).unwrap();
        fs::set_permissions(&orphan, fs::Permissions::from_mode(0o700)).unwrap();
        let marker = orphan.join(".omavless-probe-owner");
        fs::write(&marker, b"omavless-probe-scratch-v1\n2147483647\n").unwrap();
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o600)).unwrap();
        let process = host.paths.proc_root.join("4242");
        fs::create_dir(&process).unwrap();
        fs::write(process.join("comm"), b"mihomo\n").unwrap();
        assert_eq!(host.cleanup_probe_orphans(), Err(HostStepError::Cleanup));
        assert!(marker.exists());
        fs::remove_dir_all(process).unwrap();
        let tun = host.paths.sys_class_net.join("tun-test");
        fs::create_dir(&tun).unwrap();
        fs::write(tun.join("tun_flags"), b"0x1001\n").unwrap();
        assert_eq!(host.cleanup_probe_orphans(), Err(HostStepError::Cleanup));
        assert!(marker.exists());
        fs::remove_dir_all(tun).unwrap();
        host.cleanup_probe_orphans().unwrap();
        assert!(!orphan.exists());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn foreign_tun_is_not_our_cleanup_but_configured_collision_still_blocks() {
        let (root, mut host) = observation_fixture();
        fs::write(
            &host.paths.template,
            b"tun:\n  enable: true\n  device: Meta\n",
        )
        .unwrap();
        fs::set_permissions(&host.paths.template, fs::Permissions::from_mode(0o600)).unwrap();
        let foreign = host.paths.sys_class_net.join("tailscale0");
        fs::create_dir(&foreign).unwrap();
        fs::write(foreign.join("tun_flags"), b"0x1001\n").unwrap();
        let intent = DesiredState::default();
        let observed = host.observe(&intent).unwrap();
        assert_eq!(
            crate::desired::reconcile(&intent, observed),
            crate::desired::ReconcileAction::SettledDisconnected
        );
        let facts = host.fresh_observation(&intent).unwrap();
        assert_eq!((facts.visible_tun_count, facts.managed_tun_count), (1, 0));
        host.cleanup_probe_orphans().unwrap();
        host.stop_owned().unwrap();
        assert!(foreign.exists());
        let managed = host.paths.sys_class_net.join("Meta");
        fs::create_dir(&managed).unwrap();
        fs::write(managed.join("tun_flags"), b"0x1001\n").unwrap();
        assert_eq!(host.observe(&intent).unwrap().tun_count, 1);
        host.profile_id = Some("synthetic".into());
        assert!(host.start_prepared().is_err());
        assert!(managed.exists()); // Never delete a colliding device.
        host.stop_owned().unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 1);
        fs::remove_dir_all(&managed).unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 0);
        fs::create_dir(&managed).unwrap(); // Even a non-TUN name collision refuses spawn.
        host.profile_id = Some("synthetic".into());
        assert!(host.start_prepared().is_err());
        assert!(foreign.exists());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retained_device_blocks_cleanup_even_after_config_changes() {
        let (root, mut host) = observation_fixture();
        fs::write(
            &host.paths.template,
            b"tun:\n  enable: true\n  device: NewTun\n",
        )
        .unwrap();
        fs::set_permissions(&host.paths.template, fs::Permissions::from_mode(0o600)).unwrap();
        let managed = host.paths.sys_class_net.join("OldTun");
        fs::create_dir(&managed).unwrap();
        fs::write(managed.join("tun_flags"), b"0x1001\n").unwrap();
        fs::write(managed.join("ifindex"), b"7\n").unwrap();
        host.tun_identity = Some(("OldTun".into(), 7));
        assert_eq!(host.managed_tuns().unwrap(), 1);
        fs::copy(&host.paths.template, &host.paths.active_config).unwrap();
        fs::write(&host.paths.core, b"#!/bin/sh\nexec /usr/bin/sleep 10\n").unwrap();
        host.core = Some(
            OwnedCore::spawn(
                &host.paths.core,
                &host.paths.data_directory,
                &host.paths.active_config,
                &host.paths.controller_socket,
            )
            .unwrap(),
        );
        assert_eq!(host.managed_tuns().unwrap(), 1);
        fs::write(managed.join("ifindex"), b"8\n").unwrap();
        assert!(host.managed_tuns().is_err());
        host.stop_owned().unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 1);
        fs::remove_dir_all(managed).unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 0);
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_and_changed_configs_never_hide_a_tun() {
        let (root, host) = observation_fixture();
        let tun = host.paths.sys_class_net.join("foreign");
        fs::create_dir(&tun).unwrap();
        fs::write(tun.join("tun_flags"), b"0x1001\n").unwrap();
        fs::write(&host.paths.template, b"tun: {enable: true, device: Meta}\n").unwrap();
        fs::set_permissions(&host.paths.template, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 1);
        fs::write(
            &host.paths.template,
            b"tun:\n  enable: true\n  device: Meta\n",
        )
        .unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 0);
        fs::write(
            &host.paths.active_config,
            b"tun:\n  enable: true\n  device: foreign\n",
        )
        .unwrap();
        fs::set_permissions(&host.paths.active_config, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(host.managed_tuns().unwrap(), 1);
        fs::write(tun.join("tun_flags"), b"invalid\n").unwrap();
        assert!(host.managed_tuns().is_err());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_observation_empty_has_no_controller_or_vpn_health_claim() {
        let (root, mut host) = observation_fixture();
        let observed = host.fresh_observation(&DesiredState::default()).unwrap();
        assert_eq!(
            observed,
            NativeLocalObservation {
                owned_core_running: false,
                visible_mihomo_count: 0,
                owned_auxiliary_mihomo_count: 0,
                visible_tun_count: 0,
                managed_tun_count: 0,
                owned_controller_config_verified: false,
                desired_profile_matches_owned: false
            }
        );
        assert!(!host.paths.active_config.exists());
        assert!(!host.paths.store.exists());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ping_tickets_are_revoked_before_host_stop_and_start() {
        let (root, mut host) = observation_fixture();
        let first = host.ping_slot.epoch().unwrap();
        host.stop_owned().unwrap();
        assert!(host.ping_slot.epoch().unwrap() > first);
        let stopped = host.ping_slot.epoch().unwrap();
        assert!(host.start_prepared().is_err());
        assert!(host.ping_slot.epoch().unwrap() > stopped);
        host.ping_slot.poison_for_test();
        assert!(host.stop_owned().is_err());
        assert!(host.start_prepared().is_err());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_observation_counts_unowned_duplicates_without_adopting_them() {
        let (root, mut host) = observation_fixture();
        for index in [1, 2] {
            let process = host.paths.proc_root.join(index.to_string());
            fs::create_dir(&process).unwrap();
            fs::write(process.join("comm"), b"mihomo\n").unwrap();
            let interface = host.paths.sys_class_net.join(format!("tun{index}"));
            fs::create_dir(&interface).unwrap();
            fs::write(interface.join("tun_flags"), b"0x1001\n").unwrap();
        }
        let observed = host.fresh_observation(&DesiredState::default()).unwrap();
        assert_eq!(
            (observed.visible_mihomo_count, observed.visible_tun_count),
            (2, 2)
        );
        assert!(!observed.owned_core_running);
        assert!(!observed.owned_controller_config_verified);
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_observation_refuses_incomplete_inventory_and_invalid_desired() {
        for kind in [
            "missing-comm",
            "missing-net",
            "malformed-flags",
            "invalid-desired",
        ] {
            let (root, mut host) = observation_fixture();
            let mut desired = DesiredState::default();
            match kind {
                "missing-comm" => fs::create_dir(host.paths.proc_root.join("1")).unwrap(),
                "missing-net" => fs::remove_dir(&host.paths.sys_class_net).unwrap(),
                "malformed-flags" => {
                    let path = host.paths.sys_class_net.join("tun");
                    fs::create_dir(&path).unwrap();
                    fs::write(path.join("tun_flags"), b"private-invalid").unwrap();
                }
                _ => desired.generation = u64::MAX,
            }
            assert_eq!(
                host.fresh_observation(&desired),
                Err(HostStepError::Observation)
            );
            drop(host);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn fresh_controller_verification_rejects_socket_of_wrong_pid_without_repair() {
        use std::os::unix::net::UnixListener;
        let (root, mut host) = observation_fixture();
        fs::write(&host.paths.core, b"#!/bin/sh\nexec /usr/bin/sleep 5\n").unwrap();
        fs::write(&host.paths.active_config, b"synthetic config").unwrap();
        let listener = UnixListener::bind(&host.paths.controller_socket).unwrap();
        fs::set_permissions(
            &host.paths.controller_socket,
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        host.core = Some(
            OwnedCore::spawn(
                &host.paths.core,
                &host.paths.data_directory,
                &host.paths.active_config,
                &host.paths.controller_socket,
            )
            .unwrap(),
        );
        host.profile_id = Some("synthetic-id".into());
        host.readiness = Some(ConfigReadiness::new(
            crate::desired::RoutingMode::Rule,
            "Synthetic".into(),
        ));
        let desired = DesiredState {
            connected: true,
            profile_id: "synthetic-id".into(),
            ..DesiredState::default()
        };
        let observed = host.fresh_observation(&desired).unwrap();
        assert!(observed.owned_core_running);
        assert!(observed.desired_profile_matches_owned);
        assert!(!observed.owned_controller_config_verified);
        let disconnected = host.fresh_observation(&DesiredState::default()).unwrap();
        assert!(!disconnected.owned_controller_config_verified);
        assert!(!disconnected.desired_profile_matches_owned);
        assert!(host.paths.controller_socket.exists());
        assert_eq!(
            fs::read(&host.paths.active_config).unwrap(),
            b"synthetic config"
        );
        drop(listener);
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_mihomo_fresh_observation_authenticates_owned_child_without_tun() {
        let Some(binary) = std::env::var_os("OMAVLESS_TEST_MIHOMO") else {
            return;
        };
        let (root, mut host) = observation_fixture();
        host.paths.core = fs::canonicalize(binary).unwrap();
        host.paths.proc_root = PathBuf::from("/proc");
        host.paths.sys_class_net = PathBuf::from("/sys/class/net");
        let baseline_tun = tun_interface_count_strict(&host.paths.sys_class_net).unwrap();
        // Built-in DIRECT is the expected profile category. No remote endpoint,
        // provider, resolver, external TCP listener, TUN or auto-route is enabled.
        let config = format!(
            "mode: direct\nport: 0\nsocks-port: 0\nmixed-port: 0\nredir-port: 0\ntproxy-port: 0\nallow-lan: false\nlog-level: silent\nexternal-controller-unix: {}\ntun:\n  enable: false\n  auto-route: false\ndns:\n  enable: false\nproxies: []\nproxy-groups: []\nrules: []\n",
            serde_json::to_string(host.paths.controller_socket.to_str().unwrap()).unwrap()
        );
        fs::write(&host.paths.staged_config, &config).unwrap();
        fs::set_permissions(&host.paths.staged_config, fs::Permissions::from_mode(0o600)).unwrap();
        host.profile_id = Some("synthetic-direct".into());
        host.readiness = Some(ConfigReadiness::new(
            crate::desired::RoutingMode::Direct,
            "DIRECT".into(),
        ));
        host.start_prepared().unwrap();
        host.commit_prepared().unwrap();
        assert_eq!(
            fs::metadata(&host.paths.controller_socket).unwrap().mode() & 0o7777,
            0o600
        );
        let summary = crate::diagnostic_read::collect(
            &host.paths.runtime_directory,
            host.uid,
            "diagnostics.summary",
            &[],
        )
        .expect("strict diagnostics accepts the admitted private socket");
        assert_eq!(summary["version"], 1);
        assert!(summary["rules"]["items"].is_array());
        assert!(summary["providers"]["items"].is_array());
        let desired = DesiredState {
            connected: true,
            profile_id: "synthetic-direct".into(),
            mode: crate::desired::RoutingMode::Direct,
            ..DesiredState::default()
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        let observed = loop {
            if let Ok(observed) = host.fresh_observation(&desired)
                && observed.owned_controller_config_verified
            {
                break observed;
            }
            assert!(
                Instant::now() < deadline,
                "owned direct controller did not verify"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(observed.owned_core_running);
        assert!(observed.desired_profile_matches_owned);
        assert!(observed.visible_mihomo_count >= 1);
        assert_eq!(observed.visible_tun_count, baseline_tun);
        let mut wrong = desired.clone();
        wrong.mode = crate::desired::RoutingMode::Rule;
        // Ordinary unrelated process churn may invalidate a strict inventory;
        // it must never turn a mismatched desired config into verified health.
        if let Ok(observed) = host.fresh_observation(&wrong) {
            assert!(!observed.owned_controller_config_verified);
        }
        wrong = desired.clone();
        wrong.profile_id = "other-synthetic".into();
        if let Ok(observed) = host.fresh_observation(&wrong) {
            assert!(!observed.owned_controller_config_verified);
            assert!(!observed.desired_profile_matches_owned);
        }
        let pid = host.core.as_ref().unwrap().pid().unwrap();
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap()),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Ok(observed) = host.fresh_observation(&desired)
                && !observed.owned_core_running
            {
                assert!(!observed.owned_controller_config_verified);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "owned core death did not become observable"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            fs::read_to_string(&host.paths.active_config).unwrap(),
            config
        );
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn core_discovery_preserves_override_local_and_absolute_path_contract() {
        let (root, _uid) = root("core-discovery");
        let home = root.join("home");
        let local = home.join(".local/bin/mihomo");
        let path_core = root.join("bin/mihomo");
        let override_core = root.join("override/mihomo");
        executable_at(&local);
        executable_at(&path_core);
        executable_at(&override_core);

        assert_eq!(
            resolve_core(
                &home,
                Some(override_core.clone().into_os_string()),
                Some(root.join("bin").into_os_string()),
            )
            .unwrap(),
            override_core
        );
        assert_eq!(
            resolve_core(&home, None, Some(root.join("bin").into_os_string())).unwrap(),
            local
        );
        fs::remove_file(&local).unwrap();
        assert_eq!(
            resolve_core(&home, None, Some(root.join("bin").into_os_string())).unwrap(),
            path_core
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_explicit_override_and_relative_path_fail_closed() {
        let (root, _uid) = root("core-discovery-invalid");
        let home = root.join("home");
        let local = home.join(".local/bin/mihomo");
        executable_at(&local);
        assert_eq!(
            resolve_core(
                &home,
                Some(std::ffi::OsString::from("relative/mihomo")),
                None,
            ),
            Err(HostStepError::Prepare)
        );
        fs::remove_file(local).unwrap();
        assert_eq!(
            resolve_core(
                &home,
                None,
                Some(std::ffi::OsString::from("relative:also-relative")),
            ),
            Err(HostStepError::Prepare)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_directories_and_symlinked_executable_without_path_leaks() {
        let (root, uid) = root("unsafe");
        for name in ["data", "config", "runtime", "proc", "sys"] {
            fs::create_dir(root.join(name)).unwrap();
        }
        let real_core = root.join("real-core");
        fs::write(&real_core, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&real_core, fs::Permissions::from_mode(0o700)).unwrap();
        let core = root.join("private.example-password");
        symlink(real_core, &core).unwrap();
        let paths = NativeHostPaths::new(
            core,
            root.join("data"),
            root.join("config"),
            root.join("runtime"),
            root.join("proc"),
            root.join("sys"),
        );
        let error = match NativeLifecycleHost::new(paths, uid) {
            Ok(_) => panic!("unsafe host paths accepted"),
            Err(error) => error,
        };
        let public = format!("{error:?} {error}");
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
        fs::remove_dir_all(root).unwrap();
    }
}
