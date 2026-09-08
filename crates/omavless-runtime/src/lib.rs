// SPDX-License-Identifier: MIT

//! R5 runtime foundation: private IPC, lifetime ownership, and ownership-gated
//! semantic dispatch. A legacy/missing/invalid ownership marker keeps the
//! daemon read-only; only a successfully reconciled committed Rust owner can
//! register mutation methods.

use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg, OFlag};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use nix::unistd::Uid;
use omavless_control_protocol::{
    API_VERSION, FrameKind, MAX_REQUEST_FRAME_BYTES, MAX_RESPONSE_FRAME_BYTES, StableErrorCode,
    decode_request, decode_response, encode_request, encode_response, error_response, make_request,
    negotiate_version, read_unary_frame, success_response, write_unary_frame,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod batch_scheduler;
pub mod connection_transaction;
pub mod core;
mod core_readiness;
mod custom_rule_protocol;
pub mod cutover;
pub mod cutover_transaction;
pub mod desired;
pub mod desktop_helpers;
mod diagnostic_read;
pub mod frontend_bridge;
pub mod import_read_protocol;
pub mod lifecycle;
pub mod long_operation;
pub mod long_operation_protocol;
pub mod mutation;
pub mod mutation_binding;
pub mod mutation_protocol;
pub mod native_coordinator;
pub mod native_dispatch;
pub mod native_host;
mod onboarding_protocol;
pub mod owner;
pub mod private_store_transaction;
pub mod production_cutover;
pub mod production_observation;
pub mod production_owner;
pub mod profile_export_protocol;
pub mod profile_import_protocol;
pub mod profile_mutation;
pub mod profile_mutation_protocol;
pub mod profile_read_protocol;
pub mod profile_transaction;
pub mod provider_refresh;
pub mod remote_fetch;
mod route_check_protocol;
mod route_probe;
mod routing_preset;
pub mod routing_read_protocol;
pub mod semantic_cli;
pub mod startup_protocol;
mod startup_validation;
pub mod store_bootstrap;
pub mod store_preflight;
pub mod subscription_batch_work;
pub mod subscription_mutation;
pub mod subscription_mutation_protocol;
pub mod subscription_read_protocol;
pub mod subscription_refresh;
pub mod subscription_refresh_protocol;
pub mod subscription_transport;
mod support_diagnostics;

pub const SOCKET_NAME: &str = "control.sock";
pub const OWNER_LOCK_NAME: &str = "owner.lock";
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONCURRENT_CLIENTS: usize = 16;
#[cfg(test)]
use remote_fetch::MAX_CONCURRENT_REMOTE_FETCHES;

struct ActiveClient<'a>(&'a AtomicUsize);

impl Drop for ActiveClient<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn claim_slot(active: &AtomicUsize, maximum: usize) -> Option<ActiveClient<'_>> {
    active
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < maximum).then_some(count + 1)
        })
        .ok()
        .map(|_| ActiveClient(active))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    UnsafeRuntimeDirectory,
    AlreadyRunning,
    SocketUnavailable,
    PermissionDenied,
    Protocol,
    NativeOwnerUnavailable,
    Io,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeRuntimeDirectory => "OmaVLESS runtime directory is unsafe",
            Self::AlreadyRunning => "Another OmaVLESS runtime already owns this session",
            Self::SocketUnavailable => "OmaVLESS runtime socket is unavailable",
            Self::PermissionDenied => "OmaVLESS runtime peer is not permitted",
            Self::Protocol => "OmaVLESS control exchange is invalid",
            Self::NativeOwnerUnavailable => "OmaVLESS native owner is unavailable",
            Self::Io => "OmaVLESS runtime I/O failed",
        })
    }
}

impl std::error::Error for RuntimeError {}

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub directory: PathBuf,
    pub socket: PathBuf,
    pub owner_lock: PathBuf,
}

impl RuntimePaths {
    #[must_use]
    pub fn below(base: &Path) -> Self {
        let directory = base.join("omavless");
        Self {
            socket: directory.join(SOCKET_NAME),
            owner_lock: directory.join(OWNER_LOCK_NAME),
            directory,
        }
    }

    pub fn current() -> Result<Self> {
        let base = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", Uid::current().as_raw())));
        if !base.is_absolute() {
            return Err(RuntimeError::UnsafeRuntimeDirectory);
        }
        Ok(Self::below(&base))
    }
}

fn validate_directory(path: &Path, uid: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RuntimeError::UnsafeRuntimeDirectory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != uid {
        return Err(RuntimeError::UnsafeRuntimeDirectory);
    }
    Ok(())
}

fn prepare_runtime_directory(path: &Path, uid: u32) -> Result<()> {
    let parent = path.parent().ok_or(RuntimeError::UnsafeRuntimeDirectory)?;
    validate_directory(parent, uid)?;
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(RuntimeError::Io),
        }
    }
    validate_directory(path, uid)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| RuntimeError::Io)
}

struct OwnerLock {
    _file: Flock<File>,
}

impl OwnerLock {
    fn acquire(path: &Path, uid: u32) -> Result<Self> {
        if let Ok(metadata) = fs::symlink_metadata(path)
            && (metadata.file_type().is_symlink() || !metadata.is_file() || metadata.uid() != uid)
        {
            return Err(RuntimeError::UnsafeRuntimeDirectory);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(OFlag::O_NOFOLLOW.bits())
            .open(path)
            .map_err(|_| RuntimeError::Io)?;
        let file =
            Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_file, error)| {
                if matches!(error, Errno::EAGAIN) {
                    RuntimeError::AlreadyRunning
                } else {
                    RuntimeError::Io
                }
            })?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| RuntimeError::Io)?;
        Ok(Self { _file: file })
    }
}

pub struct RuntimeServer {
    listener: UnixListener,
    paths: RuntimePaths,
    uid: u32,
    instance_id: String,
    dispatcher: Arc<Mutex<RuntimeDispatcher>>,
    batch_scheduler: batch_scheduler::BatchScheduler,
    remote_fetches: remote_fetch::RemoteFetchPool,
    _owner: OwnerLock,
}

const READ_ONLY_METHODS: &[&str] = &["system.hello", "status.get", "capabilities.get"];
const NATIVE_READ_METHODS: &[&str] = &[
    "diagnostics.export",
    "diagnostics.summary",
    "diagnostics.rules",
    "diagnostics.providers",
    "routing.check",
    "routing.custom_rules.list",
    "profiles.edit_input",
    "profiles.export",
    "imports.classify",
    "profiles.list",
    "subscriptions.list",
    "subscriptions.edit_input",
];
// Remote subscription fetch uses the bounded concurrent client layer and a
// reservation-free preflight. Its final decode/commit re-enters this one
// serialized owner and rechecks revision plus exact durable ownership.
const NATIVE_MUTATION_METHODS: &[&str] = &[
    "onboarding.complete",
    "profiles.replace",
    "profiles.import",
    "routing.custom_rules.add",
    "routing.custom_rules.delete",
    "connection.connect",
    "connection.disconnect",
    "routing.set_mode",
    "routing.set_preset",
    "profiles.rename",
    "profiles.favorite",
    "profiles.delete",
    "subscriptions.add",
    "subscriptions.update",
    "subscriptions.delete",
    "subscriptions.refresh",
];

enum RuntimeDispatcher {
    ReadOnly,
    Native(Box<dyn NativeRuntimeOwner>),
}

trait NativeRuntimeOwner: Send {
    fn route_plan(
        &mut self,
        request: &Value,
    ) -> std::result::Result<route_probe::Plan, StableErrorCode>;
    fn support_report(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn diagnostic_snapshot(&mut self) -> std::result::Result<Vec<String>, StableErrorCode>;
    fn check_route(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn custom_rules(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn profile_edit_input(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn profile_export(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn import_preview(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn batch_control(
        &mut self,
        request: &Value,
        instance: &str,
    ) -> std::result::Result<
        (Value, Option<batch_scheduler::BatchWork>),
        native_coordinator::NativeOwnerError,
    >;
    fn batch_progress(&mut self, job: &native_coordinator::NativeSubscriptionBatch) -> bool;
    fn batch_finish(&mut self, job: native_coordinator::NativeSubscriptionBatch);
    fn batch_abort(&mut self, ticket: native_coordinator::NativeBatchTicket);
    fn batch_stop(&mut self);
    fn revision(&self) -> u64;
    fn runtime_ownership(&mut self) -> bool;
    fn status(&self, runtime_ownership: bool) -> Result<Value>;
    fn profiles(&self) -> Result<Value>;
    fn subscriptions(&self) -> Result<Value>;
    fn subscription_edit_input(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn bootstrap_generations(&self) -> Option<(u64, u64)>;
    fn mutate(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
    fn preflight_remote_subscription(
        &mut self,
        request: &Value,
    ) -> std::result::Result<NativeRemotePreflight, omavless_control_protocol::ProtocolError>;
    fn complete_remote_subscription(
        &mut self,
        request: &Value,
        preflight_revision: u64,
        completion: NativeRemoteCompletion,
        fetched: std::result::Result<
            omavless_domain::subscription_feed::PrivateSubscriptionBody,
            subscription_transport::SubscriptionTransportError,
        >,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError>;
}

trait NativeSubscriptionTransport:
    subscription_transport::SubscriptionTransport
    + subscription_batch_work::BudgetedSubscriptionTransport
    + Send
    + Sync
{
}
impl<T> NativeSubscriptionTransport for T where
    T: subscription_transport::SubscriptionTransport
        + subscription_batch_work::BudgetedSubscriptionTransport
        + Send
        + Sync
{
}

#[derive(Clone)]
struct SharedSubscriptionTransport(Arc<dyn NativeSubscriptionTransport>);

impl subscription_transport::SubscriptionTransport for SharedSubscriptionTransport {
    fn fetch(
        &self,
        url: &str,
    ) -> std::result::Result<
        omavless_domain::subscription_feed::PrivateSubscriptionBody,
        subscription_transport::SubscriptionTransportError,
    > {
        self.0.fetch(url)
    }
}

impl subscription_batch_work::BudgetedSubscriptionTransport for SharedSubscriptionTransport {
    fn fetch_with_budget(
        &self,
        url: &str,
        budget: Duration,
    ) -> std::result::Result<
        omavless_domain::subscription_feed::PrivateSubscriptionBody,
        subscription_transport::SubscriptionTransportError,
    > {
        self.0.fetch_with_budget(url, budget)
    }
}

enum NativeRemotePreflight {
    Fetch {
        url: String,
        transport: SharedSubscriptionTransport,
        revision: u64,
        completion: NativeRemoteCompletion,
    },
    Respond(Value),
}

enum NativeRemoteCompletion {
    Mutation,
    Refresh(native_coordinator::PreparedSubscriptionRefresh),
}

struct RecordIdGenerator {
    seed: Vec<u8>,
    counter: u128,
}

impl RecordIdGenerator {
    fn new(instance_id: &str) -> Self {
        Self {
            seed: instance_id.as_bytes().to_vec(),
            counter: 0,
        }
    }

    fn next(&mut self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"omavless/runtime-record-id/v1\0");
        hasher.update(&self.seed);
        hasher.update(self.counter.to_be_bytes());
        self.counter = self.counter.wrapping_add(1);
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15]
        )
    }
}

struct RegisteredNativeOwner<H> {
    owner: production_owner::ProductionNativeOwner<H>,
    transport: SharedSubscriptionTransport,
    record_ids: RecordIdGenerator,
    batch_initialized: bool,
}

fn profile_list_json(projection: &omavless_domain::private_store::StoreListProjection) -> Value {
    let profiles = projection
        .profiles()
        .iter()
        .map(|profile| {
            json!({
                "id": profile.id(),
                "name": profile.name(),
                "protocol": profile.protocol().as_str(),
                "subscriptionId": profile.subscription_id(),
                "missing": profile.missing(),
                "favorite": profile.favorite()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "profiles": profiles,
        "lastProfileId": projection.last_profile_id()
    })
}

fn subscription_list_json(
    projection: &omavless_domain::private_store::StoreListProjection,
) -> Value {
    let subscriptions = projection
        .subscriptions()
        .iter()
        .map(|subscription| {
            json!({
                "id": subscription.id(),
                "name": subscription.name(),
                "updatedAt": subscription.updated_at(),
                "profileCount": subscription.profile_count(),
                "staleCount": subscription.stale_count()
            })
        })
        .collect::<Vec<_>>();
    json!({"subscriptions": subscriptions})
}

impl<H> NativeRuntimeOwner for RegisteredNativeOwner<H>
where
    H: lifecycle::LifecycleHost + Send + 'static,
{
    fn route_plan(
        &mut self,
        request: &Value,
    ) -> std::result::Result<route_probe::Plan, StableErrorCode> {
        self.owner
            .route_plan(request)
            .map_err(|error| error.stable_code())
    }
    fn diagnostic_snapshot(&mut self) -> std::result::Result<Vec<String>, StableErrorCode> {
        self.owner
            .diagnostic_snapshot()
            .map_err(|error| error.stable_code())
    }

    fn check_route(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.check_route(request)
    }
    fn import_preview(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.import_preview(request)
    }

    fn batch_control(
        &mut self,
        request: &Value,
        instance: &str,
    ) -> std::result::Result<
        (Value, Option<batch_scheduler::BatchWork>),
        native_coordinator::NativeOwnerError,
    > {
        use native_coordinator::NativeOwnerError;
        if request["method"] == "subscriptions.refresh_all"
            && !self.owner.rust_ownership_available()
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let coordinator = self.owner.batch_coordinator();
        if !self.batch_initialized {
            coordinator.initialize_batch_operations(instance)?;
            self.batch_initialized = true;
        }
        let (job, accepted) = match request["method"].as_str() {
            Some("subscriptions.refresh_all") => {
                (coordinator.start_subscription_batch(request)?, None)
            }
            Some("operations.cancel") => {
                (None, Some(coordinator.cancel_subscription_batch(request)?))
            }
            Some("operations.get") => {
                return Ok((coordinator.subscription_batch_status(request)?, None));
            }
            _ => return Err(NativeOwnerError::Invariant),
        };
        let lookup = make_request(
            "batch-projection",
            "operations.get",
            json!({
                "instanceId": request["params"]["instanceId"],
                "operationId": request["params"]["operationId"]
            }),
        )
        .map_err(|_| NativeOwnerError::Invariant)?;
        let mut result = coordinator.subscription_batch_status(&lookup)?;
        if let Some(accepted) = accepted {
            result["accepted"] = json!(accepted);
        }
        let job = job.map(|job| batch_scheduler::BatchWork {
            job,
            transport: self.transport.clone(),
            record_ids: RecordIdGenerator::new(&self.record_ids.next()),
        });
        Ok((result, job))
    }

    fn batch_progress(&mut self, job: &native_coordinator::NativeSubscriptionBatch) -> bool {
        self.owner.rust_ownership_available()
            && self
                .owner
                .batch_coordinator()
                .publish_subscription_batch_progress(job)
                .is_ok()
    }

    fn batch_finish(&mut self, job: native_coordinator::NativeSubscriptionBatch) {
        let _ = self
            .owner
            .batch_coordinator()
            .complete_subscription_batch(job, || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64
            });
    }

    fn batch_abort(&mut self, ticket: native_coordinator::NativeBatchTicket) {
        let _ = self
            .owner
            .batch_coordinator()
            .abort_subscription_batch(ticket);
    }

    fn batch_stop(&mut self) {
        let _ = self.owner.batch_coordinator().stop_batch_operations();
    }

    fn revision(&self) -> u64 {
        self.owner.revision()
    }

    fn runtime_ownership(&mut self) -> bool {
        self.owner.rust_ownership_available()
    }

    fn status(&self, runtime_ownership: bool) -> Result<Value> {
        let desired = self
            .owner
            .desired()
            .map_err(|_| RuntimeError::NativeOwnerUnavailable)?;
        let actual = match self.owner.actual() {
            lifecycle::ActualState::Disconnected => "disconnected",
            lifecycle::ActualState::Starting => "starting",
            lifecycle::ActualState::Connected => "connected",
            lifecycle::ActualState::Reconnecting => "reconnecting",
            lifecycle::ActualState::Stopping => "stopping",
            lifecycle::ActualState::Failed => "failed",
            lifecycle::ActualState::ManualRecoveryRequired => "manualRecoveryRequired",
        };
        Ok(json!({
            "desired": if desired.connected { "connected" } else { "disconnected" },
            "actual": actual,
            "activeProfileId": if desired.connected { desired.profile_id.as_str() } else { "" },
            "mode": desired.mode.as_str(),
            "transition": self.owner.transition(),
            "runtimeOwnership": runtime_ownership
        }))
    }

    fn profiles(&self) -> Result<Value> {
        let projection = self
            .owner
            .list_projection()
            .map_err(|_| RuntimeError::NativeOwnerUnavailable)?;
        Ok(profile_list_json(&projection))
    }

    fn subscriptions(&self) -> Result<Value> {
        let projection = self
            .owner
            .list_projection()
            .map_err(|_| RuntimeError::NativeOwnerUnavailable)?;
        Ok(subscription_list_json(&projection))
    }

    fn subscription_edit_input(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.subscription_edit_input(request)
    }

    fn profile_edit_input(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.profile_edit_input(request)
    }

    fn custom_rules(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.custom_rules(request)
    }

    fn support_report(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.support_report(request)
    }

    fn profile_export(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        self.owner.profile_export(request)
    }

    fn bootstrap_generations(&self) -> Option<(u64, u64)> {
        self.owner.bootstrap_generations()
    }

    fn mutate(
        &mut self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let owner = &mut self.owner;
        let transport = &self.transport;
        let record_ids = &mut self.record_ids;
        owner.respond(
            request,
            transport,
            || record_ids.next(),
            || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX)
            },
        )
    }

    fn preflight_remote_subscription(
        &mut self,
        request: &Value,
    ) -> std::result::Result<NativeRemotePreflight, omavless_control_protocol::ProtocolError> {
        if request["method"] == "subscriptions.refresh" {
            match self.owner.preflight_remote_subscription_refresh(request)? {
                native_dispatch::RemoteSubscriptionRefreshPreflight::Fetch(prepared) => {
                    Ok(NativeRemotePreflight::Fetch {
                        url: prepared.private_url().to_owned(),
                        transport: self.transport.clone(),
                        revision: self.owner.revision(),
                        completion: NativeRemoteCompletion::Refresh(prepared),
                    })
                }
                native_dispatch::RemoteSubscriptionRefreshPreflight::Respond(response) => {
                    Ok(NativeRemotePreflight::Respond(response))
                }
            }
        } else {
            match self.owner.preflight_remote_subscription(request)? {
                native_dispatch::RemoteSubscriptionPreflight::Fetch(prepared) => {
                    Ok(NativeRemotePreflight::Fetch {
                        url: prepared.private_url().to_owned(),
                        transport: self.transport.clone(),
                        revision: self.owner.revision(),
                        completion: NativeRemoteCompletion::Mutation,
                    })
                }
                native_dispatch::RemoteSubscriptionPreflight::Respond(response) => {
                    Ok(NativeRemotePreflight::Respond(response))
                }
            }
        }
    }

    fn complete_remote_subscription(
        &mut self,
        request: &Value,
        preflight_revision: u64,
        completion: NativeRemoteCompletion,
        fetched: std::result::Result<
            omavless_domain::subscription_feed::PrivateSubscriptionBody,
            subscription_transport::SubscriptionTransportError,
        >,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let record_ids = &mut self.record_ids;
        let now = || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        };
        match completion {
            NativeRemoteCompletion::Mutation => self.owner.respond_to_fetched_subscription(
                request,
                preflight_revision,
                fetched,
                || record_ids.next(),
                now,
            ),
            NativeRemoteCompletion::Refresh(prepared) => {
                self.owner.respond_to_fetched_subscription_refresh(
                    request,
                    preflight_revision,
                    prepared,
                    fetched,
                    || record_ids.next(),
                    now,
                )
            }
        }
    }
}

impl RuntimeServer {
    pub fn bind(paths: RuntimePaths) -> Result<Self> {
        let uid = Uid::current().as_raw();
        prepare_runtime_directory(&paths.directory, uid)?;
        let owner = OwnerLock::acquire(&paths.owner_lock, uid)?;
        if let Ok(metadata) = fs::symlink_metadata(&paths.socket) {
            if !metadata.file_type().is_socket() || metadata.uid() != uid {
                return Err(RuntimeError::UnsafeRuntimeDirectory);
            }
            fs::remove_file(&paths.socket).map_err(|_| RuntimeError::Io)?;
        }
        let listener =
            UnixListener::bind(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600))
            .map_err(|_| RuntimeError::Io)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RuntimeError::Io)?
            .as_nanos();
        Ok(Self {
            listener,
            paths,
            uid,
            instance_id: format!("{:x}-{nonce:x}", std::process::id()),
            dispatcher: Arc::new(Mutex::new(RuntimeDispatcher::ReadOnly)),
            batch_scheduler: batch_scheduler::BatchScheduler::default(),
            remote_fetches: remote_fetch::RemoteFetchPool::default(),
            _owner: owner,
        })
    }

    /// Bind the production socket and register native mutations only after a
    /// committed Rust owner has been constructed and startup-reconciled. A
    /// non-Rust/missing/invalid marker intentionally leaves the daemon
    /// read-only; other owner-construction failures abort startup.
    pub fn bind_current(paths: RuntimePaths) -> Result<Self> {
        Self::bind_with_owner_factory(paths, |runtime_paths| {
            production_owner::ProductionNativeOwner::current(runtime_paths)
        })
    }

    fn bind_with_owner_factory<H, F>(paths: RuntimePaths, construct_owner: F) -> Result<Self>
    where
        H: lifecycle::LifecycleHost + Send + 'static,
        F: FnOnce(
            &RuntimePaths,
        ) -> std::result::Result<
            production_owner::ProductionNativeOwner<H>,
            production_owner::ProductionOwnerError,
        >,
    {
        let mut server = Self::bind(paths)?;
        match construct_owner(&server.paths) {
            Ok(owner) => server.register_native_owner(
                owner,
                subscription_transport::HttpsSubscriptionTransport::new(),
            ),
            Err(production_owner::ProductionOwnerError::OwnershipUnavailable) => {}
            Err(_) => return Err(RuntimeError::NativeOwnerUnavailable),
        }
        Ok(server)
    }

    fn register_native_owner<H, T>(
        &mut self,
        owner: production_owner::ProductionNativeOwner<H>,
        transport: T,
    ) where
        H: lifecycle::LifecycleHost + Send + 'static,
        T: NativeSubscriptionTransport + 'static,
    {
        let registered = RegisteredNativeOwner {
            owner,
            transport: SharedSubscriptionTransport(Arc::new(transport)),
            record_ids: RecordIdGenerator::new(&self.instance_id),
            batch_initialized: false,
        };
        self.dispatcher = Arc::new(Mutex::new(RuntimeDispatcher::Native(Box::new(registered))));
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.paths.socket
    }

    pub fn serve(self, maximum_connections: Option<usize>) -> Result<()> {
        let active = AtomicUsize::new(0);
        let server = &self;
        thread::scope(|scope| {
            for (handled, incoming) in self.listener.incoming().enumerate() {
                if let Ok(mut stream) = incoming
                    && let Some(slot) = claim_slot(&active, MAX_CONCURRENT_CLIENTS)
                {
                    // A malformed, slow, disconnected, or unauthorized client
                    // receives one bounded worker slot. Saturation closes the
                    // newly accepted stream instead of creating unbounded work.
                    scope.spawn(move || {
                        let _slot = slot;
                        let _ = server.handle(&mut stream);
                    });
                }
                if maximum_connections.is_some_and(|maximum| handled + 1 >= maximum) {
                    break;
                }
            }
            self.batch_scheduler.stop(&self.dispatcher);
        });
        Ok(())
    }

    pub fn serve_until(self, stop: &AtomicBool) -> Result<()> {
        self.listener
            .set_nonblocking(true)
            .map_err(|_| RuntimeError::Io)?;
        let active = AtomicUsize::new(0);
        let server = &self;
        thread::scope(|scope| {
            while !stop.load(Ordering::Relaxed) {
                match self.listener.accept() {
                    Ok((mut stream, _address)) => {
                        if let Some(slot) = claim_slot(&active, MAX_CONCURRENT_CLIENTS) {
                            scope.spawn(move || {
                                let _slot = slot;
                                let _ = server.handle(&mut stream);
                            });
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => {
                        self.batch_scheduler.stop(&self.dispatcher);
                        return Err(RuntimeError::Io);
                    }
                }
            }
            self.batch_scheduler.stop(&self.dispatcher);
            Ok(())
        })
    }

    fn handle(&self, stream: &mut UnixStream) -> Result<()> {
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(|_| RuntimeError::Io)?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(|_| RuntimeError::Io)?;
        let credentials =
            getsockopt(&*stream, PeerCredentials).map_err(|_| RuntimeError::PermissionDenied)?;
        if credentials.uid() != self.uid {
            return Err(RuntimeError::PermissionDenied);
        }
        let response = match read_unary_frame(stream, FrameKind::Request)
            .and_then(|frame| decode_request(&frame))
        {
            Ok(request) => self.dispatch(&request),
            Err(error) => error_response(
                "invalid",
                0,
                error.code(),
                false,
                (error.code() == StableErrorCode::UnsupportedVersion)
                    .then(|| json!({"supported": [API_VERSION]})),
            ),
        }
        .map_err(|_| RuntimeError::Protocol)?;
        let frame = encode_response(&response).map_err(|_| RuntimeError::Protocol)?;
        write_unary_frame(stream, &frame, FrameKind::Response).map_err(|_| RuntimeError::Io)
    }

    fn dispatch(
        &self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        if request["method"] == "routing.check" {
            return self.dispatch_route_check(request);
        }
        if diagnostic_read::METHODS.contains(&request["method"].as_str().unwrap_or("")) {
            return self.dispatch_diagnostics(request);
        }
        if batch_scheduler::METHODS.contains(&request["method"].as_str().unwrap_or_default()) {
            return self.batch_scheduler.dispatch(
                request,
                &self.instance_id,
                &self.dispatcher,
                &self.remote_fetches,
            );
        }
        if matches!(
            request["method"].as_str(),
            Some("subscriptions.add" | "subscriptions.update" | "subscriptions.refresh")
        ) {
            return self.dispatch_remote_subscription(request);
        }
        let mut dispatcher = match self.dispatcher.lock() {
            Ok(dispatcher) => dispatcher,
            Err(_) => {
                return error_response(
                    request["id"].as_str().unwrap_or("invalid"),
                    0,
                    StableErrorCode::InternalError,
                    false,
                    None,
                );
            }
        };
        if request["method"] == "runtime.transitionBootstrap" {
            return self.dispatch_transition_bootstrap(request, &mut dispatcher);
        }
        match &mut *dispatcher {
            RuntimeDispatcher::ReadOnly => dispatch_read_only(request, &self.instance_id),
            RuntimeDispatcher::Native(owner) => {
                dispatch_native(request, &self.instance_id, owner.as_mut())
            }
        }
    }

    fn dispatch_route_check(
        &self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let deadline = std::time::Instant::now() + route_probe::DEADLINE;
        let (revision, context) = {
            let mut dispatcher = match self.dispatcher.try_lock() {
                Ok(owner) => owner,
                Err(std::sync::TryLockError::WouldBlock) => {
                    return error_response(id, 0, StableErrorCode::Busy, true, None);
                }
                Err(_) => {
                    return error_response(id, 0, StableErrorCode::InternalError, false, None);
                }
            };
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                return dispatch_read_only(request, &self.instance_id);
            };
            let revision = owner.revision();
            match owner.route_plan(request) {
                Ok(route_probe::Plan::Fast(value)) => return success_response(id, revision, value),
                Ok(route_probe::Plan::Live(context)) => (revision, context),
                Err(code) => {
                    return error_response(id, revision, code, code == StableErrorCode::Busy, None);
                }
            }
        };
        let Some(_permit) = self.remote_fetches.try_acquire() else {
            return error_response(id, revision, StableErrorCode::Busy, true, None);
        };
        let collected = route_probe::collect(&self.paths.directory, self.uid, &context, deadline);
        let mut dispatcher = match self.dispatcher.try_lock() {
            Ok(owner) => owner,
            Err(std::sync::TryLockError::WouldBlock) => {
                return error_response(id, revision, StableErrorCode::Busy, true, None);
            }
            Err(_) => {
                return error_response(id, revision, StableErrorCode::InternalError, false, None);
            }
        };
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return error_response(
                id,
                revision,
                StableErrorCode::CapabilityUnavailable,
                false,
                None,
            );
        };
        let current = owner.revision();
        if current != revision {
            return error_response(id, current, StableErrorCode::Conflict, true, None);
        }
        match owner.route_plan(request) {
            Ok(route_probe::Plan::Live(now)) if now == context => (),
            Err(code) => {
                return error_response(id, current, code, code == StableErrorCode::Busy, None);
            }
            _ => return error_response(id, current, StableErrorCode::Conflict, true, None),
        }
        if std::time::Instant::now() >= deadline {
            return error_response(id, current, StableErrorCode::CoreRejected, false, None);
        }
        match collected {
            Ok(value) => success_response(id, current, value),
            Err(code) => error_response(id, current, code, false, None),
        }
    }

    fn dispatch_diagnostics(
        &self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        if !empty_params(request) {
            return error_response(id, 0, StableErrorCode::InvalidArgument, false, None);
        }
        let (revision, private) = {
            let mut dispatcher = self.dispatcher.lock().map_err(|_| {
                omavless_control_protocol::ProtocolError::new(StableErrorCode::InternalError)
            })?;
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                return dispatch_read_only(request, &self.instance_id);
            };
            let revision = owner.revision();
            match owner.diagnostic_snapshot() {
                Ok(private) => (revision, private),
                Err(code) => {
                    return error_response(id, revision, code, code == StableErrorCode::Busy, None);
                }
            }
        };
        // Share the existing four-work cap so reads + provider GETs cannot
        // consume all 16 client slots and starve status/urgent disconnect.
        let Some(_permit) = self.remote_fetches.try_acquire() else {
            return error_response(id, revision, StableErrorCode::Busy, true, None);
        };
        let started = std::time::Instant::now();
        let collected = diagnostic_read::collect(
            &self.paths.directory,
            self.uid,
            request["method"].as_str().unwrap_or(""),
            &private,
        );
        let mut dispatcher = self.dispatcher.lock().map_err(|_| {
            omavless_control_protocol::ProtocolError::new(StableErrorCode::InternalError)
        })?;
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return error_response(
                id,
                revision,
                StableErrorCode::CapabilityUnavailable,
                false,
                None,
            );
        };
        let current = owner.revision();
        if current != revision {
            return error_response(id, current, StableErrorCode::Conflict, true, None);
        }
        match owner.diagnostic_snapshot() {
            Err(code) => {
                return error_response(id, current, code, code == StableErrorCode::Busy, None);
            }
            Ok(current_private) if current_private != private => {
                return error_response(id, current, StableErrorCode::Conflict, true, None);
            }
            Ok(_) => (),
        }
        if started.elapsed() >= diagnostic_read::DEADLINE {
            return error_response(id, current, StableErrorCode::CoreRejected, false, None);
        }
        match collected {
            Ok(result) => success_response(id, current, result),
            Err(code) => error_response(id, current, code, false, None),
        }
    }

    fn dispatch_remote_subscription(
        &self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let preflight = {
            let mut dispatcher = match self.dispatcher.lock() {
                Ok(dispatcher) => dispatcher,
                Err(_) => {
                    return error_response(id, 0, StableErrorCode::InternalError, false, None);
                }
            };
            match &mut *dispatcher {
                RuntimeDispatcher::ReadOnly => {
                    return dispatch_read_only(request, &self.instance_id);
                }
                RuntimeDispatcher::Native(owner) => owner.preflight_remote_subscription(request)?,
            }
        };

        let (url, transport, revision, completion) = match preflight {
            NativeRemotePreflight::Fetch {
                url,
                transport,
                revision,
                completion,
            } => (url, transport, revision, completion),
            NativeRemotePreflight::Respond(response) => return Ok(response),
        };
        let Some(_remote_slot) = self.remote_fetches.try_acquire() else {
            return error_response(id, revision, StableErrorCode::Busy, true, None);
        };
        let fetched = subscription_transport::SubscriptionTransport::fetch(&transport, &url);

        let mut dispatcher = match self.dispatcher.lock() {
            Ok(dispatcher) => dispatcher,
            Err(_) => {
                return error_response(id, 0, StableErrorCode::InternalError, false, None);
            }
        };
        match &mut *dispatcher {
            RuntimeDispatcher::ReadOnly => {
                error_response(id, 0, StableErrorCode::CapabilityUnavailable, false, None)
            }
            RuntimeDispatcher::Native(owner) => {
                owner.complete_remote_subscription(request, revision, completion, fetched)
            }
        }
    }

    fn dispatch_transition_bootstrap(
        &self,
        request: &Value,
        dispatcher: &mut RuntimeDispatcher,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let Some(params) = request["params"].as_object() else {
            return error_response(id, 0, StableErrorCode::InvalidArgument, false, None);
        };
        if params.len() != 1 {
            return error_response(id, 0, StableErrorCode::InvalidArgument, false, None);
        }
        let Some(preparing_generation) = params.get("preparingGeneration").and_then(Value::as_u64)
        else {
            return error_response(id, 0, StableErrorCode::InvalidArgument, false, None);
        };

        if matches!(dispatcher, RuntimeDispatcher::ReadOnly) {
            let owner = match production_owner::ProductionNativeOwner::candidate_current(
                &self.paths,
                preparing_generation,
            ) {
                Ok(owner) => owner,
                Err(production_owner::ProductionOwnerError::Busy) => {
                    return error_response(id, 0, StableErrorCode::Busy, true, None);
                }
                Err(production_owner::ProductionOwnerError::OwnershipUnavailable) => {
                    return error_response(id, 0, StableErrorCode::Conflict, false, None);
                }
                Err(_) => {
                    return error_response(
                        id,
                        0,
                        StableErrorCode::ManualRecoveryRequired,
                        false,
                        None,
                    );
                }
            };
            let registered = RegisteredNativeOwner {
                owner,
                transport: SharedSubscriptionTransport(Arc::new(
                    subscription_transport::HttpsSubscriptionTransport::new(),
                )),
                record_ids: RecordIdGenerator::new(&self.instance_id),
                batch_initialized: false,
            };
            *dispatcher = RuntimeDispatcher::Native(Box::new(registered));
        }

        let RuntimeDispatcher::Native(owner) = dispatcher else {
            unreachable!("transition bootstrap installs a native candidate")
        };
        let Some((actual_preparing, rust_generation)) = owner.bootstrap_generations() else {
            return error_response(id, owner.revision(), StableErrorCode::Conflict, false, None);
        };
        if actual_preparing != preparing_generation {
            return error_response(id, owner.revision(), StableErrorCode::Conflict, false, None);
        }
        let runtime_ownership = owner.runtime_ownership();
        success_response(
            id,
            owner.revision(),
            json!({
                "instanceId": self.instance_id,
                "preparingGeneration": actual_preparing,
                "rustGeneration": rust_generation,
                "runtimeOwnership": runtime_ownership
            }),
        )
    }
}

impl Drop for RuntimeServer {
    fn drop(&mut self) {
        self.batch_scheduler.stop(&self.dispatcher);
        let _ = fs::remove_file(&self.paths.socket);
    }
}

fn empty_params(request: &Value) -> bool {
    request["params"]
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
}

fn dispatch_read_only(
    request: &Value,
    instance_id: &str,
) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
    let revision = 0;
    let id = request["id"].as_str().unwrap_or("invalid");
    let method = request["method"].as_str().unwrap_or_default();
    let result = match method {
        "system.hello" => {
            let params = request["params"].as_object();
            let versions = params.and_then(|value| value.get("versions"));
            if params.is_none_or(|value| value.len() != 1) || versions.is_none() {
                return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
            }
            if let Err(error) = negotiate_version(versions.unwrap_or(&Value::Null)) {
                return error_response(
                    id,
                    revision,
                    error.code(),
                    false,
                    (error.code() == StableErrorCode::UnsupportedVersion)
                        .then(|| json!({"supported": [API_VERSION]})),
                );
            }
            json!({
                "instanceId": instance_id,
                "version": API_VERSION,
                "versions": [API_VERSION],
                "limits": {
                    "requestFrameBytes": MAX_REQUEST_FRAME_BYTES,
                    "responseFrameBytes": MAX_RESPONSE_FRAME_BYTES
                },
                "runtimeOwnership": false
            })
        }
        "status.get" if empty_params(request) => json!({
            "desired": "disconnected",
            "actual": "disconnected",
            "activeProfileId": "",
            "mode": "rule",
            "transition": null,
            "runtimeOwnership": false
        }),
        "capabilities.get" if empty_params(request) => json!({
            "runtimeOwnership": false,
            "mutations": false,
            "methods": READ_ONLY_METHODS
        }),
        "status.get" | "capabilities.get" => {
            return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
        }
        _ => return error_response(id, revision, StableErrorCode::UnknownMethod, false, None),
    };
    success_response(id, revision, result)
}

fn dispatch_native(
    request: &Value,
    instance_id: &str,
    owner: &mut dyn NativeRuntimeOwner,
) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
    let id = request["id"].as_str().unwrap_or("invalid");
    let method = request["method"].as_str().unwrap_or_default();
    let revision = owner.revision();
    let runtime_ownership = owner.runtime_ownership();
    let result = match method {
        "system.hello" => {
            let params = request["params"].as_object();
            let versions = params.and_then(|value| value.get("versions"));
            if params.is_none_or(|value| value.len() != 1) || versions.is_none() {
                return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
            }
            if let Err(error) = negotiate_version(versions.unwrap_or(&Value::Null)) {
                return error_response(
                    id,
                    revision,
                    error.code(),
                    false,
                    (error.code() == StableErrorCode::UnsupportedVersion)
                        .then(|| json!({"supported": [API_VERSION]})),
                );
            }
            json!({
                "instanceId": instance_id,
                "version": API_VERSION,
                "versions": [API_VERSION],
                "limits": {
                    "requestFrameBytes": MAX_REQUEST_FRAME_BYTES,
                    "responseFrameBytes": MAX_RESPONSE_FRAME_BYTES
                },
                "runtimeOwnership": runtime_ownership
            })
        }
        "status.get" if empty_params(request) => match owner.status(runtime_ownership) {
            Ok(status) => status,
            Err(_) => {
                return error_response(id, revision, StableErrorCode::InternalError, false, None);
            }
        },
        "capabilities.get" if empty_params(request) => {
            let methods: Vec<_> = READ_ONLY_METHODS
                .iter()
                .chain(
                    runtime_ownership
                        .then_some(NATIVE_READ_METHODS)
                        .into_iter()
                        .flatten(),
                )
                .chain(
                    runtime_ownership
                        .then_some(NATIVE_MUTATION_METHODS)
                        .into_iter()
                        .flatten(),
                )
                .chain(
                    runtime_ownership
                        .then_some(batch_scheduler::METHODS)
                        .into_iter()
                        .flatten(),
                )
                .copied()
                .collect();
            json!({
                "runtimeOwnership": runtime_ownership,
                "mutations": runtime_ownership,
                "methods": methods
            })
        }
        "status.get" | "capabilities.get" => {
            return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
        }
        "profiles.list" if runtime_ownership && empty_params(request) => match owner.profiles() {
            Ok(profiles) => profiles,
            Err(_) => {
                return error_response(id, revision, StableErrorCode::InternalError, false, None);
            }
        },
        "subscriptions.list" if runtime_ownership && empty_params(request) => {
            match owner.subscriptions() {
                Ok(subscriptions) => subscriptions,
                Err(_) => {
                    return error_response(
                        id,
                        revision,
                        StableErrorCode::InternalError,
                        false,
                        None,
                    );
                }
            }
        }
        "subscriptions.edit_input" if runtime_ownership => {
            return owner.subscription_edit_input(request);
        }
        "imports.classify" if runtime_ownership => return owner.import_preview(request),
        "routing.custom_rules.list" if runtime_ownership => return owner.custom_rules(request),
        "diagnostics.export" if runtime_ownership => return owner.support_report(request),
        "routing.check" if runtime_ownership => return owner.check_route(request),
        "profiles.export" if runtime_ownership => return owner.profile_export(request),
        "profiles.edit_input" if runtime_ownership => return owner.profile_edit_input(request),
        _ if NATIVE_READ_METHODS.contains(&method) && !runtime_ownership => {
            return error_response(
                id,
                revision,
                StableErrorCode::CapabilityUnavailable,
                false,
                None,
            );
        }
        _ if NATIVE_READ_METHODS.contains(&method) => {
            return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
        }
        _ if NATIVE_MUTATION_METHODS.contains(&method) => return owner.mutate(request),
        _ => return error_response(id, revision, StableErrorCode::UnknownMethod, false, None),
    };
    success_response(id, revision, result)
}

pub fn call(paths: &RuntimePaths, method: &str, params: Value) -> Result<Value> {
    let uid = Uid::current().as_raw();
    validate_directory(&paths.directory, uid)?;
    let directory =
        fs::symlink_metadata(&paths.directory).map_err(|_| RuntimeError::UnsafeRuntimeDirectory)?;
    if directory.mode() & 0o7777 != 0o700 {
        return Err(RuntimeError::UnsafeRuntimeDirectory);
    }
    let socket =
        fs::symlink_metadata(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
    if !socket.file_type().is_socket() || socket.uid() != uid || socket.mode() & 0o7777 != 0o600 {
        return Err(RuntimeError::PermissionDenied);
    }
    let stream = UnixStream::connect(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
    call_stream(stream, uid, method, params)
}

fn call_stream(mut stream: UnixStream, uid: u32, method: &str, params: Value) -> Result<Value> {
    // Authenticate the connected peer, not just the path checked before
    // connect. In particular, no private editor/subscription input is written
    // before this check. Metadata checks alone cannot close replacement races.
    let credentials =
        getsockopt(&stream, PeerCredentials).map_err(|_| RuntimeError::PermissionDenied)?;
    if credentials.uid() != uid {
        return Err(RuntimeError::PermissionDenied);
    }
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| RuntimeError::Io)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| RuntimeError::Io)?;
    let id = format!("cli-{}", std::process::id());
    let request = make_request(&id, method, params).map_err(|_| RuntimeError::Protocol)?;
    let frame = encode_request(&request).map_err(|_| RuntimeError::Protocol)?;
    write_unary_frame(&mut stream, &frame, FrameKind::Request).map_err(|_| RuntimeError::Io)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| RuntimeError::Io)?;
    let response = read_unary_frame(&mut stream, FrameKind::Response)
        .and_then(|frame| decode_response(&frame))
        .map_err(|_| RuntimeError::Protocol)?;
    if response["id"].as_str() != Some(id.as_str()) {
        return Err(RuntimeError::Protocol);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::{CutoverPaths, OwnershipPhase};
    use crate::desired::{
        DesiredPaths, DesiredState, OwnedObservation, RoutingMode, write_desired,
    };
    use crate::lifecycle::HostStepError;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";
    const SUBSCRIPTION_ID: &str = "10000000-0000-4000-8000-000000000001";

    struct FakeHost {
        observation: OwnedObservation,
        calls: Arc<AtomicUsize>,
        route_config: Option<std::path::PathBuf>,
    }

    struct BlockingTransport {
        started: std::sync::mpsc::SyncSender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }

    impl subscription_transport::SubscriptionTransport for BlockingTransport {
        fn fetch(
            &self,
            _url: &str,
        ) -> std::result::Result<
            omavless_domain::subscription_feed::PrivateSubscriptionBody,
            subscription_transport::SubscriptionTransportError,
        > {
            self.started
                .send(())
                .map_err(|_| subscription_transport::SubscriptionTransportError::Unavailable)?;
            self.release
                .lock()
                .map_err(|_| subscription_transport::SubscriptionTransportError::Unavailable)?
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| subscription_transport::SubscriptionTransportError::Timeout)?;
            omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
                b"vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Managed".to_vec(),
            )
            .map_err(|_| subscription_transport::SubscriptionTransportError::Unavailable)
        }
    }

    impl subscription_batch_work::BudgetedSubscriptionTransport for BlockingTransport {
        fn fetch_with_budget(
            &self,
            url: &str,
            _budget: Duration,
        ) -> std::result::Result<
            omavless_domain::subscription_feed::PrivateSubscriptionBody,
            subscription_transport::SubscriptionTransportError,
        > {
            subscription_transport::SubscriptionTransport::fetch(self, url)
        }
    }

    impl lifecycle::LifecycleHost for FakeHost {
        fn route_core_identity(&mut self) -> Option<(u32, [u8; 32])> {
            use sha2::{Digest, Sha256};
            Some((
                std::process::id(),
                Sha256::digest(fs::read(self.route_config.as_ref()?).ok()?).into(),
            ))
        }
        fn observe(
            &mut self,
            _desired: &DesiredState,
        ) -> std::result::Result<OwnedObservation, HostStepError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> std::result::Result<(), HostStepError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn start_prepared(&mut self) -> std::result::Result<(), HostStepError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.observation = OwnedObservation {
                service_active: true,
                controller_ready: true,
                core_count: 1,
                tun_count: 1,
                active_profile_matches: true,
            };
            Ok(())
        }

        fn commit_prepared(&mut self) -> std::result::Result<(), HostStepError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn stop_owned(&mut self) -> std::result::Result<(), HostStepError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.observation = OwnedObservation {
                service_active: false,
                controller_ready: false,
                core_count: 0,
                tun_count: 0,
                active_profile_matches: false,
            };
            Ok(())
        }

        fn discard_prepared(&mut self) -> std::result::Result<(), HostStepError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    fn temporary_base(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = env::temp_dir().join(format!(
            "omavless-runtime-{label}-{}-{nonce}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&base).unwrap();
        base
    }

    #[test]
    fn native_client_correlates_success_and_error_replies() {
        for success in [true, false] {
            for matching in [true, false] {
                let (client, mut peer) = UnixStream::pair().unwrap();
                let worker = thread::spawn(move || {
                    let request = read_unary_frame(&mut peer, FrameKind::Request)
                        .and_then(|frame| decode_request(&frame))
                        .unwrap();
                    let id = if matching {
                        request["id"].as_str().unwrap()
                    } else {
                        "unrelated-private-value"
                    };
                    let response = if success {
                        success_response(id, 0, json!({"accepted": true})).unwrap()
                    } else {
                        error_response(id, 0, StableErrorCode::Busy, true, None).unwrap()
                    };
                    let frame = encode_response(&response).unwrap();
                    write_unary_frame(&mut peer, &frame, FrameKind::Response).unwrap();
                    peer.shutdown(std::net::Shutdown::Write).unwrap();
                });
                let result = call_stream(client, Uid::current().as_raw(), "status.get", json!({}));
                worker.join().unwrap();
                if matching {
                    assert_eq!(result.unwrap()["ok"], success);
                } else {
                    assert_eq!(result, Err(RuntimeError::Protocol));
                    assert!(!RuntimeError::Protocol.to_string().contains("private-value"));
                }
            }
        }
    }

    #[test]
    fn native_client_rejects_wrong_peer_before_sending_private_input() {
        use std::io::Read;
        let (client, mut peer) = UnixStream::pair().unwrap();
        let other_uid = Uid::current().as_raw().wrapping_add(1);
        assert_eq!(
            call_stream(
                client,
                other_uid,
                "subscriptions.add",
                json!({"name": "private-name", "url": "https://private.invalid/secret"})
            ),
            Err(RuntimeError::PermissionDenied)
        );
        // The client closes on rejection; the peer sees EOF without any bytes.
        let mut bytes = Vec::new();
        peer.read_to_end(&mut bytes).unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn native_client_rejects_unsafe_endpoint_without_connecting() {
        use std::os::unix::fs::symlink;
        let base = temporary_base("client-endpoint");
        let paths = RuntimePaths::below(&base);
        prepare_runtime_directory(&paths.directory, Uid::current().as_raw()).unwrap();
        let listener = UnixListener::bind(&paths.socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o660)).unwrap();
        assert_eq!(
            call(&paths, "status.get", json!({})),
            Err(RuntimeError::PermissionDenied)
        );
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o750)).unwrap();
        assert_eq!(
            call(&paths, "status.get", json!({})),
            Err(RuntimeError::UnsafeRuntimeDirectory)
        );
        fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
        let actual = paths.directory.join("actual.sock");
        fs::rename(&paths.socket, &actual).unwrap();
        symlink(&actual, &paths.socket).unwrap();
        assert_eq!(
            call(&paths, "status.get", json!({})),
            Err(RuntimeError::PermissionDenied)
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        fs::remove_file(&paths.socket).unwrap();
        fs::write(&paths.socket, b"not a socket").unwrap();
        assert_eq!(
            call(&paths, "status.get", json!({})),
            Err(RuntimeError::PermissionDenied)
        );
        fs::remove_dir_all(base).unwrap();
    }

    fn write_marker(paths: &CutoverPaths, phase: OwnershipPhase, generation: u64) {
        let marker = json!({
            "schemaVersion": 1,
            "generation": generation,
            "phase": phase.as_str(),
        });
        fs::write(
            &paths.ownership_marker,
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();
        fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn owner_fixture(
        base: &Path,
        phase: OwnershipPhase,
    ) -> (
        production_owner::ProductionNativeOwner<FakeHost>,
        CutoverPaths,
        Arc<AtomicUsize>,
    ) {
        owner_fixture_with_route(base, phase, false)
    }

    fn owner_fixture_with_route(
        base: &Path,
        phase: OwnershipPhase,
        route: bool,
    ) -> (
        production_owner::ProductionNativeOwner<FakeHost>,
        CutoverPaths,
        Arc<AtomicUsize>,
    ) {
        let runtime = base.join("runtime");
        let state = base.join("state");
        let config = base.join("config");
        for path in [&runtime, &state, &config] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(base).unwrap().uid();
        let desired = DesiredPaths::below(&state);
        write_desired(&desired, uid, &DesiredState::default()).unwrap();
        let cutover = CutoverPaths::below(&runtime, &state, uid);
        write_marker(&cutover, phase, 1);
        let store_path = config.join("profiles.json");
        let store = json!({
            "version": 3,
            "activeId": "",
            "lastId": "",
            "profiles": [{
                "id": PROFILE_ID,
                "name": "Example",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example",
                "protocol": "vless",
                "favorite": false
            }],
            "subscriptions": [{
                "id": "10000000-0000-4000-8000-000000000001",
                "name": "Example source",
                "url": "https://private.example/subscription-token",
                "updatedAt": 7
            }],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true
        });
        fs::write(&store_path, serde_json::to_vec(&store).unwrap()).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let host = FakeHost {
            route_config: if route {
                let path = config.join("route.yaml");
                fs::write(&path, b"synthetic-config").unwrap();
                Some(path)
            } else {
                None
            },
            observation: OwnedObservation {
                service_active: false,
                controller_ready: false,
                core_count: 0,
                tun_count: 0,
                active_profile_matches: false,
            },
            calls: Arc::clone(&calls),
        };
        let owner = if phase == OwnershipPhase::Rust {
            production_owner::ProductionNativeOwner::initialize(
                host,
                desired,
                &store_path,
                cutover.clone(),
                uid,
            )
        } else {
            production_owner::ProductionNativeOwner::initialize_candidate(
                host,
                desired,
                &store_path,
                cutover.clone(),
                uid,
                1,
            )
        }
        .unwrap();
        (owner, cutover, calls)
    }

    fn native_owner_fixture(
        base: &Path,
    ) -> (
        production_owner::ProductionNativeOwner<FakeHost>,
        CutoverPaths,
        Arc<AtomicUsize>,
    ) {
        owner_fixture(base, OwnershipPhase::Rust)
    }

    #[test]
    fn socket_and_owner_are_private_and_singleton() {
        let base = temporary_base("owner");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        assert_eq!(
            fs::metadata(&paths.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&paths.socket).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(matches!(
            RuntimeServer::bind(paths.clone()),
            Err(RuntimeError::AlreadyRunning)
        ));
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn trusted_runtime_record_ids_are_unique_and_store_compatible() {
        use std::collections::HashSet;

        let mut generator = RecordIdGenerator::new("instance-a");
        let generated: HashSet<_> = (0..1_025).map(|_| generator.next()).collect();
        assert_eq!(generated.len(), 1_025);
        assert!(
            generated
                .iter()
                .all(|value| omavless_domain::store::valid_record_id(value))
        );
        assert_ne!(
            RecordIdGenerator::new("instance-a").next(),
            RecordIdGenerator::new("instance-b").next()
        );
    }

    #[test]
    fn maximum_store_list_projections_fit_the_response_frame() {
        let profiles = (1..=omavless_domain::store::MAX_PROFILES)
            .map(|index| {
                json!({
                    "id": format!("00000000-0000-4000-8000-{index:012x}"),
                    "name": format!("{index:03}-{}", "界".repeat(76)),
                    "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp",
                    "protocol": "vless",
                    "favorite": index % 2 == 0
                })
            })
            .collect::<Vec<_>>();
        let subscriptions = (1..=omavless_domain::store::MAX_SUBSCRIPTIONS)
            .map(|index| {
                json!({
                    "id": format!("10000000-0000-4000-8000-{index:012x}"),
                    "name": format!("{index:03}-{}", "Я".repeat(76)),
                    "url": format!("https://private{index}.example/subscription-token"),
                    "updatedAt": index
                })
            })
            .collect::<Vec<_>>();
        let private = json!({
            "version": 3,
            "activeId": "",
            "lastId": "00000000-0000-4000-8000-000000000001",
            "profiles": profiles,
            "subscriptions": subscriptions,
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true
        });
        let private = private.to_string();
        let store = omavless_domain::private_store::parse_private_store(&private).unwrap();
        let projection = store.list_projection();
        let profiles = success_response("max-profiles", 1, profile_list_json(&projection)).unwrap();
        let subscriptions =
            success_response("max-subscriptions", 1, subscription_list_json(&projection)).unwrap();
        let profiles = encode_response(&profiles).unwrap();
        let subscriptions = encode_response(&subscriptions).unwrap();
        assert!(profiles.len() <= MAX_RESPONSE_FRAME_BYTES);
        assert!(subscriptions.len() <= MAX_RESPONSE_FRAME_BYTES);
        for output in [&profiles, &subscriptions] {
            let output = std::str::from_utf8(output).unwrap();
            for secret in ["vless://", "11111111", "192.0.2.1", "subscription-token"] {
                assert!(!output.contains(secret));
            }
        }
    }

    #[test]
    fn read_only_unary_api_round_trips() {
        let base = temporary_base("api");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        let worker = thread::spawn(move || server.serve(Some(3)).unwrap());
        for (method, params) in [
            ("system.hello", json!({"versions": [1]})),
            ("status.get", json!({})),
            ("capabilities.get", json!({})),
        ] {
            let response = call(&paths, method, params).unwrap();
            assert_eq!(response["ok"], true);
            assert_eq!(response["result"]["runtimeOwnership"], false);
        }
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn concurrent_client_slots_are_bounded_and_reusable() {
        let active = AtomicUsize::new(0);
        let slots = (0..MAX_CONCURRENT_CLIENTS)
            .map(|_| claim_slot(&active, MAX_CONCURRENT_CLIENTS).unwrap())
            .collect::<Vec<_>>();
        assert!(claim_slot(&active, MAX_CONCURRENT_CLIENTS).is_none());
        assert_eq!(active.load(Ordering::Acquire), MAX_CONCURRENT_CLIENTS);
        drop(slots);
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert!(claim_slot(&active, MAX_CONCURRENT_CLIENTS).is_some());

        let remote = AtomicUsize::new(0);
        let slots = (0..MAX_CONCURRENT_REMOTE_FETCHES)
            .map(|_| claim_slot(&remote, MAX_CONCURRENT_REMOTE_FETCHES).unwrap())
            .collect::<Vec<_>>();
        assert!(claim_slot(&remote, MAX_CONCURRENT_REMOTE_FETCHES).is_none());
        drop(slots);
        assert_eq!(remote.load(Ordering::Acquire), 0);
    }

    #[test]
    fn slow_client_does_not_monopolize_read_only_runtime() {
        use std::io::Write;
        use std::time::Instant;

        let base = temporary_base("slow-client");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        let worker = thread::spawn(move || server.serve(Some(2)).unwrap());

        let mut slow = UnixStream::connect(&paths.socket).unwrap();
        slow.write_all(b"{").unwrap();
        thread::sleep(Duration::from_millis(50));

        let started = Instant::now();
        let response = call(&paths, "status.get", json!({})).unwrap();
        let elapsed = started.elapsed();
        assert_eq!(response["ok"], true);
        assert!(
            elapsed < Duration::from_secs(2),
            "a slow peer delayed an independent status request"
        );

        drop(slow);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    fn batch_call(server: &RuntimeServer, method: &str, operation: &str) -> Value {
        server
            .dispatch(
                &make_request(
                    "batch-test",
                    method,
                    json!({
                        "instanceId": server.instance_id, "operationId": operation
                    }),
                )
                .unwrap(),
            )
            .unwrap()
    }

    fn wait_batch(server: &RuntimeServer, operation: &str) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let response = batch_call(server, "operations.get", operation);
            let state = response["result"]["operation"]["state"].as_str().unwrap();
            if matches!(state, "succeeded" | "failed" | "cancelled") {
                return response;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "batch did not terminalize"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn batch_socket_ack_retry_and_cancel_do_not_wait_for_network() {
        let base = temporary_base("batch-socket");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let store = base.join("config/profiles.json");
        let before = fs::read(&store).unwrap();
        let paths = RuntimePaths::below(&base.join("runtime"));
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(2);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(paths.clone()).unwrap();
        let instance = server.instance_id.clone();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || server.serve_until(&worker_stop).unwrap());
        let params = json!({"instanceId": instance, "operationId": "batch-1"});
        let started = std::time::Instant::now();
        assert_eq!(
            call(&paths, "subscriptions.refresh_all", params.clone()).unwrap()["ok"],
            true
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            call(&paths, "subscriptions.refresh_all", params.clone()).unwrap()["ok"],
            true
        );
        assert_eq!(call(&paths, "status.get", json!({})).unwrap()["ok"], true);
        let cancel = call(&paths, "operations.cancel", params.clone()).unwrap();
        assert_eq!(cancel["result"]["accepted"], true);
        release_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let response = call(&paths, "operations.get", params.clone()).unwrap();
            if response["result"]["operation"]["state"] == "cancelled" {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
        }
        assert!(started_rx.try_recv().is_err(), "retry downloaded twice");
        assert_eq!(fs::read(store).unwrap(), before);
        stop.store(true, Ordering::Release);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn batch_scheduler_commits_once_and_rejects_unknown_poll_fields() {
        let base = temporary_base("batch-success");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(2);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(RuntimePaths::below(&base.join("runtime"))).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "one")["ok"],
            true
        );
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
        let completed = wait_batch(&server, "one");
        assert_eq!(completed["result"]["operation"]["state"], "succeeded");
        assert_eq!(completed["revision"], 1);
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "one")["result"],
            completed["result"]
        );
        assert!(started_rx.try_recv().is_err());
        let bad = server
            .dispatch(
                &make_request(
                    "bad",
                    "operations.get",
                    json!({
                        "instanceId": server.instance_id, "operationId": "one", "unexpected": true
                    }),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(bad["error"]["code"], "invalid_argument");
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn batch_real_http_and_private_socket_preserve_success_cancel_and_error_boundaries() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        for outcome in ["success", "cancel", "error", "revoked"] {
            let base = temporary_base("batch-http-socket");
            let (owner, cutover, calls) = native_owner_fixture(&base);
            let host_calls = calls.load(Ordering::Relaxed);
            let store = base.join("config/profiles.json");
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut content: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
            content["subscriptions"][0]["url"] = json!(format!("http://{address}/synthetic-feed"));
            fs::write(&store, content.to_string()).unwrap();
            let before = fs::read(&store).unwrap();
            let (arrived_tx, arrived_rx) = std::sync::mpsc::sync_channel(1);
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
            let http = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                    assert!(headers.len() < 8192);
                }
                arrived_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                let body = if outcome == "error" {
                    "synthetic-private-error"
                } else {
                    "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Managed"
                };
                let status = if outcome == "error" {
                    "503 Unavailable"
                } else {
                    "200 OK"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            });
            let paths = RuntimePaths::below(&base.join("runtime"));
            let mut server = RuntimeServer::bind(paths.clone()).unwrap();
            let instance = server.instance_id.clone();
            server.register_native_owner(
                owner,
                subscription_transport::HttpsSubscriptionTransport::new(),
            );
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = thread::spawn(move || server.serve_until(&worker_stop).unwrap());
            let params = json!({"instanceId":instance,"operationId":"http-batch"});
            assert_eq!(
                call(&paths, "subscriptions.refresh_all", params.clone()).unwrap()["ok"],
                true
            );
            arrived_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(
                call(&paths, "subscriptions.refresh_all", params.clone()).unwrap()["ok"],
                true
            );
            assert_eq!(call(&paths, "status.get", json!({})).unwrap()["ok"], true);
            if outcome == "cancel" {
                assert_eq!(
                    call(&paths, "operations.cancel", params.clone()).unwrap()["result"]["accepted"],
                    true
                );
            } else if outcome == "revoked" {
                write_marker(&cutover, OwnershipPhase::RollbackPreparing, 2);
            }
            release_tx.send(()).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let terminal = loop {
                let response = call(&paths, "operations.get", params.clone()).unwrap();
                if matches!(
                    response["result"]["operation"]["state"].as_str(),
                    Some("succeeded" | "failed" | "cancelled")
                ) {
                    break response;
                }
                assert!(std::time::Instant::now() < deadline);
                thread::sleep(Duration::from_millis(5));
            };
            let expected = match outcome {
                "success" => "succeeded",
                "cancel" => "cancelled",
                _ => "failed",
            };
            assert_eq!(terminal["result"]["operation"]["state"], expected);
            for private in [
                "synthetic-feed",
                "synthetic-private-error",
                "vless://",
                "192.0.2.",
                "Managed",
            ] {
                assert!(!terminal.to_string().contains(private));
            }
            assert_eq!(fs::read(&store).unwrap() == before, outcome != "success");
            assert_eq!(
                terminal["revision"],
                if outcome == "success" { 1 } else { 0 }
            );
            assert_eq!(calls.load(Ordering::Relaxed), host_calls);
            stop.store(true, Ordering::Release);
            worker.join().unwrap();
            http.join().unwrap();
            assert!(!paths.socket.exists());
            fs::remove_dir_all(base).unwrap();
        }
    }

    #[test]
    fn batch_waits_for_shared_fetch_permit_and_shutdown_revokes_it() {
        let base = temporary_base("batch-pool-stop");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let store = base.join("config/profiles.json");
        let before = fs::read(&store).unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (_release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(RuntimePaths::below(&base.join("runtime"))).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        let permits: Vec<_> = (0..4)
            .map(|_| server.remote_fetches.try_acquire().unwrap())
            .collect();
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "one")["ok"],
            true
        );
        assert!(started_rx.recv_timeout(Duration::from_millis(80)).is_err());
        assert_eq!(batch_call(&server, "operations.get", "one")["ok"], true);
        server.batch_scheduler.stop(&server.dispatcher);
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "two")["error"]["code"],
            "daemon_restarting"
        );
        drop(permits);
        assert_eq!(fs::read(store).unwrap(), before);
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn batch_disconnect_during_fetch_prevents_late_commit() {
        let base = temporary_base("batch-disconnect");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let store = base.join("config/profiles.json");
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(RuntimePaths::below(&base.join("runtime"))).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        let connected = server
            .dispatch(
                &make_request(
                    "connect",
                    "connection.connect",
                    json!({
                        "profileId": PROFILE_ID, "mode": "global"
                    }),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(connected["ok"], true);
        assert_eq!(connected["revision"], 1);
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "one")["ok"],
            true
        );
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let disconnect = server
            .dispatch(&make_request("disconnect", "connection.disconnect", json!({})).unwrap())
            .unwrap();
        assert_eq!(disconnect["ok"], true);
        assert_eq!(disconnect["revision"], 2);
        let after_disconnect = fs::read(&store).unwrap();
        release_tx.send(()).unwrap();
        let completed = wait_batch(&server, "one");
        assert_eq!(completed["result"]["operation"]["state"], "failed");
        assert_eq!(
            completed["result"]["operation"]["error"]["code"],
            "conflict"
        );
        assert_eq!(fs::read(store).unwrap(), after_disconnect);
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn batch_shutdown_revokes_inflight_work_before_joining() {
        let base = temporary_base("batch-shutdown-inflight");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let store = base.join("config/profiles.json");
        let before = fs::read(&store).unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(RuntimePaths::below(&base.join("runtime"))).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        let server = Arc::new(server);
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "one")["ok"],
            true
        );
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let stopping_server = Arc::clone(&server);
        let shutdown = thread::spawn(move || {
            stopping_server
                .batch_scheduler
                .stop(&stopping_server.dispatcher);
        });
        let lookup = make_request(
            "inspect-shutdown",
            "operations.get",
            json!({
                "instanceId": server.instance_id, "operationId": "one"
            }),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let mut dispatcher = server.dispatcher.lock().unwrap();
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                panic!("native fixture");
            };
            let (result, _) = owner.batch_control(&lookup, &server.instance_id).unwrap();
            if result["operation"]["state"] == "failed" {
                assert_eq!(result["operation"]["error"]["code"], "daemon_restarting");
                break;
            }
            drop(dispatcher);
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !shutdown.is_finished(),
            "shutdown must join the in-flight fetch"
        );
        let started = std::time::Instant::now();
        assert_eq!(
            batch_call(&server, "operations.get", "one")["error"]["code"],
            "daemon_restarting"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "shutdown blocked a unary peer"
        );
        release_tx.send(()).unwrap();
        shutdown.join().unwrap();
        assert_eq!(fs::read(store).unwrap(), before);
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    struct PanickingBatchTransport;
    impl subscription_transport::SubscriptionTransport for PanickingBatchTransport {
        fn fetch(
            &self,
            _url: &str,
        ) -> std::result::Result<
            omavless_domain::subscription_feed::PrivateSubscriptionBody,
            subscription_transport::SubscriptionTransportError,
        > {
            panic!("synthetic worker failure");
        }
    }
    impl subscription_batch_work::BudgetedSubscriptionTransport for PanickingBatchTransport {
        fn fetch_with_budget(
            &self,
            url: &str,
            _budget: Duration,
        ) -> std::result::Result<
            omavless_domain::subscription_feed::PrivateSubscriptionBody,
            subscription_transport::SubscriptionTransportError,
        > {
            subscription_transport::SubscriptionTransport::fetch(self, url)
        }
    }

    #[test]
    fn batch_supervisor_reclaims_panicked_worker_and_allows_successor() {
        let base = temporary_base("batch-worker-panic");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let mut server = RuntimeServer::bind(RuntimePaths::below(&base.join("runtime"))).unwrap();
        server.register_native_owner(owner, PanickingBatchTransport);
        for operation in ["first", "successor"] {
            assert_eq!(
                batch_call(&server, "subscriptions.refresh_all", operation)["ok"],
                true
            );
            let failed = wait_batch(&server, operation);
            assert_eq!(failed["result"]["operation"]["state"], "failed");
            assert_eq!(
                failed["result"]["operation"]["error"]["code"],
                "internal_error"
            );
        }
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn batch_failed_spawn_terminalizes_without_fetch_and_retry_never_reschedules() {
        let base = temporary_base("batch-spawn-failure");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(2);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(RuntimePaths::below(&base.join("runtime"))).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        server
            .batch_scheduler
            .fail_next_spawn
            .store(true, Ordering::Release);
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "failed")["error"]["code"],
            "internal_error"
        );
        let terminal = wait_batch(&server, "failed");
        assert_eq!(terminal["result"]["operation"]["state"], "failed");
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "failed")["result"],
            terminal["result"]
        );
        assert!(started_rx.try_recv().is_err());
        assert_eq!(
            batch_call(&server, "subscriptions.refresh_all", "next")["ok"],
            true
        );
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
        assert_eq!(
            wait_batch(&server, "next")["result"]["operation"]["state"],
            "succeeded"
        );
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn remote_subscription_fetch_does_not_block_status_or_disconnect() {
        use std::time::Instant;

        let base = temporary_base("remote-fetch");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let mut server = RuntimeServer::bind(paths.clone()).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        let worker = thread::spawn(move || server.serve(Some(11)).unwrap());

        let connected = call(
            &paths,
            "connection.connect",
            json!({
                "profileId": PROFILE_ID,
                "mode": "global",
                "operationId": "connect-before-fetch",
                "expectedRevision": 0
            }),
        )
        .unwrap();
        assert_eq!(connected["ok"], true);
        assert_eq!(connected["revision"], 1);

        let refresh_paths = paths.clone();
        let refresh = thread::spawn(move || {
            call(
                &refresh_paths,
                "subscriptions.refresh",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "operationId": "subscription-refresh-conflict"
                }),
            )
            .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let started = Instant::now();
        let status = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(status["result"]["actual"], "connected");
        assert!(started.elapsed() < Duration::from_secs(2));

        let started = Instant::now();
        let disconnected = call(
            &paths,
            "connection.disconnect",
            json!({
                "operationId": "disconnect-during-fetch",
                "expectedRevision": 1
            }),
        )
        .unwrap();
        assert_eq!(disconnected["ok"], true);
        assert_eq!(disconnected["revision"], 2);
        assert!(started.elapsed() < Duration::from_secs(2));

        release_tx.send(()).unwrap();
        let rejected_refresh = refresh.join().unwrap();
        assert_eq!(rejected_refresh["ok"], false);
        assert_eq!(rejected_refresh["error"]["code"], "conflict");
        let rendered = rejected_refresh.to_string();
        for private in [
            "private.example",
            "subscription-token",
            "vless://",
            "192.0.2.2",
        ] {
            assert!(!rendered.contains(private));
        }

        let subscriptions = call(&paths, "subscriptions.list", json!({})).unwrap();
        assert_eq!(
            subscriptions["result"]["subscriptions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let add_paths = paths.clone();
        let add = thread::spawn(move || {
            call(
                &add_paths,
                "subscriptions.add",
                json!({
                    "name": "Second source",
                    "url": "https://second.invalid/subscription-token",
                    "operationId": "subscription-fetch-2"
                }),
            )
            .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
        let accepted_add = add.join().unwrap();
        assert_eq!(accepted_add["ok"], true);
        assert_eq!(accepted_add["revision"], 3);
        let subscriptions = call(&paths, "subscriptions.list", json!({})).unwrap();
        assert_eq!(
            subscriptions["result"]["subscriptions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let update_paths = paths.clone();
        let update = thread::spawn(move || {
            call(
                &update_paths,
                "subscriptions.update",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "name": "Updated source",
                    "url": "https://updated.invalid/subscription-token",
                    "operationId": "subscription-fetch-3"
                }),
            )
            .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
        let accepted_update = update.join().unwrap();
        assert_eq!(accepted_update["ok"], true);
        assert_eq!(accepted_update["revision"], 4);
        let subscriptions = call(&paths, "subscriptions.list", json!({})).unwrap();
        assert_eq!(
            subscriptions["result"]["subscriptions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            subscriptions["result"]["subscriptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|subscription| subscription["name"] == "Updated source")
        );

        let refresh_paths = paths.clone();
        let refresh = thread::spawn(move || {
            call(
                &refresh_paths,
                "subscriptions.refresh",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "operationId": "subscription-refresh-success"
                }),
            )
            .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
        let accepted_refresh = refresh.join().unwrap();
        assert_eq!(accepted_refresh["ok"], true);
        assert_eq!(accepted_refresh["revision"], 5);
        let subscriptions = call(&paths, "subscriptions.list", json!({})).unwrap();
        assert_eq!(
            subscriptions["result"]["subscriptions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn remote_fetch_saturation_preserves_an_urgent_disconnect_path() {
        use std::time::Instant;

        let base = temporary_base("fetch-cap");
        let (owner, _cutover, _calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(MAX_CONCURRENT_REMOTE_FETCHES);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(MAX_CONCURRENT_REMOTE_FETCHES);
        let mut server = RuntimeServer::bind(paths.clone()).unwrap();
        server.register_native_owner(
            owner,
            BlockingTransport {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        );
        let worker = thread::spawn(move || server.serve(Some(6)).unwrap());

        let mut fetches = Vec::new();
        for index in 0..MAX_CONCURRENT_REMOTE_FETCHES {
            let fetch_paths = paths.clone();
            fetches.push(thread::spawn(move || {
                call(
                    &fetch_paths,
                    "subscriptions.add",
                    json!({
                        "name": format!("Source {index}"),
                        "url": format!("https://provider{index}.invalid/private"),
                        "operationId": format!("parallel-fetch-{index}")
                    }),
                )
                .unwrap()
            }));
        }
        for _ in 0..MAX_CONCURRENT_REMOTE_FETCHES {
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        }

        let saturated = call(
            &paths,
            "subscriptions.add",
            json!({
                "name": "Saturated source",
                "url": "https://saturated.invalid/private",
                "operationId": "parallel-fetch-saturated"
            }),
        )
        .unwrap();
        assert_eq!(saturated["ok"], false);
        assert_eq!(saturated["error"]["code"], "busy");
        assert_eq!(saturated["error"]["retryable"], true);

        let started = Instant::now();
        let disconnected = call(
            &paths,
            "connection.disconnect",
            json!({
                "operationId": "disconnect-at-fetch-cap",
                "expectedRevision": 0
            }),
        )
        .unwrap();
        assert_eq!(disconnected["ok"], true);
        assert!(started.elapsed() < Duration::from_secs(2));

        for _ in 0..MAX_CONCURRENT_REMOTE_FETCHES {
            release_tx.send(()).unwrap();
        }
        let responses = fetches
            .into_iter()
            .map(|fetch| fetch.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            responses
                .iter()
                .filter(|response| response["ok"] == true)
                .count(),
            1
        );
        assert_eq!(
            responses
                .iter()
                .filter(|response| response["error"]["code"] == "conflict")
                .count(),
            MAX_CONCURRENT_REMOTE_FETCHES - 1
        );
        let rendered = format!("{saturated}{responses:?}");
        for private in ["provider", "saturated.invalid", "private", "vless://"] {
            assert!(!rendered.contains(private));
        }
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn bind_current_factory_registers_mutations_and_revocation_fails_closed() {
        let base = temporary_base("native-registration");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let expected_paths = paths.clone();
        let constructor_calls = Arc::new(AtomicUsize::new(0));
        let observed_calls = Arc::clone(&constructor_calls);
        let server = RuntimeServer::bind_with_owner_factory(paths.clone(), move |runtime_paths| {
            assert_eq!(runtime_paths, &expected_paths);
            observed_calls.fetch_add(1, Ordering::Relaxed);
            Ok(owner)
        })
        .unwrap();
        assert_eq!(constructor_calls.load(Ordering::Relaxed), 1);
        let worker = thread::spawn(move || server.serve(Some(20)).unwrap());

        let hello = call(&paths, "system.hello", json!({"versions": [1]})).unwrap();
        assert_eq!(hello["result"]["runtimeOwnership"], true);
        let capabilities = call(&paths, "capabilities.get", json!({})).unwrap();
        assert_eq!(capabilities["result"]["mutations"], true);
        assert!(
            capabilities["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method == "connection.connect")
        );
        assert!(
            capabilities["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method == "routing.set_mode")
        );
        for method in [
            "profiles.list",
            "subscriptions.list",
            "subscriptions.edit_input",
        ] {
            assert!(
                capabilities["result"]["methods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|candidate| candidate == method)
            );
        }
        for method in [
            "subscriptions.add",
            "subscriptions.update",
            "subscriptions.delete",
            "subscriptions.refresh",
        ] {
            assert!(
                capabilities["result"]["methods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|candidate| candidate == method)
            );
        }
        let status = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(status["result"]["actual"], "disconnected");
        let profiles = call(&paths, "profiles.list", json!({})).unwrap();
        assert_eq!(profiles["result"]["profiles"][0]["id"], PROFILE_ID);
        assert_eq!(profiles["result"]["profiles"][0]["protocol"], "vless");
        assert_eq!(profiles["result"]["profiles"][0]["favorite"], false);
        let subscriptions = call(&paths, "subscriptions.list", json!({})).unwrap();
        assert_eq!(
            subscriptions["result"]["subscriptions"][0]["profileCount"],
            0
        );
        assert_eq!(subscriptions["result"]["subscriptions"][0]["staleCount"], 0);
        let edit_input = call(
            &paths,
            "subscriptions.edit_input",
            json!({"subscriptionId": SUBSCRIPTION_ID}),
        )
        .unwrap();
        assert_eq!(edit_input["ok"], true);
        assert_eq!(edit_input["result"]["name"], "Example source");
        assert_eq!(
            edit_input["result"]["url"],
            "https://private.example/subscription-token"
        );
        let bad_edit = call(
            &paths,
            "subscriptions.edit_input",
            json!({"subscriptionId": SUBSCRIPTION_ID, "extra": true}),
        )
        .unwrap();
        assert_eq!(bad_edit["error"]["code"], "invalid_argument");
        assert!(!bad_edit.to_string().contains("private.example"));
        let rendered = format!("{profiles}{subscriptions}");
        for private in [
            "vless://",
            "11111111",
            "192.0.2.1",
            "private.example",
            "subscription-token",
        ] {
            assert!(!rendered.contains(private));
        }
        let bad_list = call(&paths, "profiles.list", json!({"extra": true})).unwrap();
        assert_eq!(bad_list["error"]["code"], "invalid_argument");

        let connected = call(
            &paths,
            "connection.connect",
            json!({
                "profileId": PROFILE_ID,
                "mode": RoutingMode::Global.as_str(),
                "operationId": "connect-1",
                "expectedRevision": 0
            }),
        )
        .unwrap();
        assert_eq!(connected["ok"], true);
        assert_eq!(connected["revision"], 1);
        let status = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(status["result"]["actual"], "connected");
        assert_eq!(status["result"]["mode"], "global");
        let mode = call(
            &paths,
            "routing.set_mode",
            json!({
                "mode": "direct",
                "operationId": "mode-1",
                "expectedRevision": 1
            }),
        )
        .unwrap();
        assert_eq!(mode["ok"], true);
        assert_eq!(mode["revision"], 2);
        let status = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(status["result"]["actual"], "connected");
        assert_eq!(status["result"]["mode"], "direct");
        let deleted = call(
            &paths,
            "subscriptions.delete",
            json!({
                "subscriptionId": SUBSCRIPTION_ID,
                "operationId": "subscription-delete-1",
                "expectedRevision": 2
            }),
        )
        .unwrap();
        assert_eq!(deleted["ok"], true);
        assert_eq!(deleted["revision"], 3);
        let subscriptions = call(&paths, "subscriptions.list", json!({})).unwrap();
        assert_eq!(
            subscriptions["result"]["subscriptions"]
                .as_array()
                .unwrap()
                .len(),
            0
        );

        write_marker(&cutover, OwnershipPhase::RollbackPreparing, 2);
        let calls_before_rejection = calls.load(Ordering::Relaxed);
        let capabilities = call(&paths, "capabilities.get", json!({})).unwrap();
        assert_eq!(capabilities["result"]["runtimeOwnership"], false);
        assert_eq!(capabilities["result"]["mutations"], false);
        assert!(
            capabilities["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .all(|method| method != "connection.connect")
        );
        let rejected_list = call(&paths, "profiles.list", json!({})).unwrap();
        assert_eq!(rejected_list["error"]["code"], "capability_unavailable");
        let rejected_edit = call(
            &paths,
            "subscriptions.edit_input",
            json!({"subscriptionId": SUBSCRIPTION_ID}),
        )
        .unwrap();
        assert_eq!(rejected_edit["error"]["code"], "capability_unavailable");
        assert!(!rejected_edit.to_string().contains("private.example"));
        let rejected = call(
            &paths,
            "connection.disconnect",
            json!({"operationId": "disconnect-1", "expectedRevision": 3}),
        )
        .unwrap();
        assert_eq!(rejected["error"]["code"], "capability_unavailable");
        assert_eq!(calls.load(Ordering::Relaxed), calls_before_rejection);

        write_marker(&cutover, OwnershipPhase::Rust, 3);
        let capabilities = call(&paths, "capabilities.get", json!({})).unwrap();
        assert_eq!(capabilities["result"]["runtimeOwnership"], false);
        assert_eq!(capabilities["result"]["mutations"], false);
        let stale_owner = call(
            &paths,
            "connection.disconnect",
            json!({"operationId": "disconnect-2", "expectedRevision": 1}),
        )
        .unwrap();
        assert_eq!(stale_owner["error"]["code"], "capability_unavailable");
        assert_eq!(calls.load(Ordering::Relaxed), calls_before_rejection);

        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn import_preview_uses_current_private_store_without_effects_and_fails_closed() {
        let base = temporary_base("import-preview");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let store_path = base.join("config/profiles.json");
        let original = fs::read(&store_path).unwrap();
        let calls_before = calls.load(Ordering::Relaxed);
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(10)).unwrap());
        let capabilities = call(&paths, "capabilities.get", json!({})).unwrap();
        assert!(
            capabilities["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method == "imports.classify")
        );
        let duplicate = call(
            &paths,
            "imports.classify",
            json!({"input": " \nhttps://private.example/subscription-token\n"}),
        )
        .unwrap();
        assert_eq!(duplicate["revision"], 0);
        assert_eq!(
            duplicate["result"],
            json!({
                "version": 1, "kind": "subscription", "duplicate": true,
                "suggestedName": "Subscription",
            })
        );
        let profile = call(
            &paths,
            "imports.classify",
            json!({
                "input": "trojan://synthetic-password@203.0.113.1:443#SyntheticLabel",
            }),
        )
        .unwrap();
        assert_eq!(profile["revision"], 0);
        assert_eq!(profile["result"]["kind"], "profile");
        assert_eq!(profile["result"]["profile"]["protocol"], "trojan");
        assert!(!profile.to_string().contains("synthetic-password"));
        for params in [
            json!({"input": "trojan://synthetic-password@bad"}),
            json!({"input": "https://private.example/subscription-token", "duplicate": false}),
        ] {
            let response = call(&paths, "imports.classify", params).unwrap();
            assert_eq!(response["error"]["code"], "invalid_argument");
            for marker in [
                "synthetic-password",
                "private.example",
                "subscription-token",
            ] {
                assert!(!response.to_string().contains(marker));
            }
        }
        let new = call(
            &paths,
            "imports.classify",
            json!({"input": "https://example.invalid/new-token"}),
        )
        .unwrap();
        assert_eq!(new["result"]["duplicate"], false);
        assert_eq!(fs::read(&store_path).unwrap(), original);
        assert_eq!(calls.load(Ordering::Relaxed), calls_before);

        fs::write(&store_path, b"invalid-private-store").unwrap();
        let corrupt = call(
            &paths,
            "imports.classify",
            json!({"input": "https://example.invalid/new-token"}),
        )
        .unwrap();
        assert_eq!(corrupt["error"]["code"], "internal_error");
        assert!(!corrupt.to_string().contains("invalid-private-store"));
        fs::write(&store_path, &original).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o644)).unwrap();
        let unsafe_store = call(
            &paths,
            "imports.classify",
            json!({"input": "https://example.invalid/new-token"}),
        )
        .unwrap();
        assert_eq!(unsafe_store["error"]["code"], "internal_error");
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            let response = call(
                &paths,
                "imports.classify",
                json!({"input": "https://private.example/subscription-token"}),
            )
            .unwrap();
            assert_eq!(response["error"]["code"], "capability_unavailable");
            assert!(!response.to_string().contains("subscription-token"));
        }
        assert_eq!(calls.load(Ordering::Relaxed), calls_before);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn explicit_profile_export_is_private_bounded_and_exact_owner_fenced() {
        let base = temporary_base("profile-export");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let store_path = base.join("config/profiles.json");
        let original = fs::read(&store_path).unwrap();
        let document: Value = serde_json::from_slice(&original).unwrap();
        let expected = document["profiles"][0]["uri"].as_str().unwrap().to_owned();
        let calls_before = calls.load(Ordering::Relaxed);
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(11)).unwrap());
        let params = json!({"profileId":PROFILE_ID,"purpose":"file"});
        for purpose in ["file", "qr"] {
            let response = call(
                &paths,
                "profiles.export",
                json!({"profileId":PROFILE_ID,"purpose":purpose}),
            )
            .unwrap();
            assert_eq!(response["ok"], true);
            assert_eq!(response["revision"], 0);
            assert_eq!(response["result"].as_object().unwrap().len(), 2);
            assert_eq!(response["result"]["format"], "uri");
            assert!(
                response["result"]["content"] == expected,
                "export content mismatch"
            );
        }
        let list = call(&paths, "profiles.list", json!({})).unwrap();
        assert!(!list.to_string().contains(&expected));
        let missing = call(
            &paths,
            "profiles.export",
            json!({"profileId":"00000000-0000-4000-8000-000000000099","purpose":"file"}),
        )
        .unwrap();
        assert_eq!(missing["error"]["code"], "not_found");
        let invalid = call(
            &paths,
            "profiles.export",
            json!({"profileId":PROFILE_ID,"purpose":"file","path":"private-token"}),
        )
        .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_argument");
        assert!(!invalid.to_string().contains("private-token"));
        assert!(fs::read(&store_path).unwrap() == original);
        assert_eq!(calls.load(Ordering::Relaxed), calls_before);
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o644)).unwrap();
        let unsafe_store = call(&paths, "profiles.export", params.clone()).unwrap();
        assert_eq!(unsafe_store["error"]["code"], "internal_error");
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&store_path, b"private-token-corrupt").unwrap();
        let corrupt = call(&paths, "profiles.export", params.clone()).unwrap();
        assert_eq!(corrupt["error"]["code"], "internal_error");
        assert!(!corrupt.to_string().contains("private-token"));
        fs::write(&store_path, &original).unwrap();
        let target = base.join("config/saved-store.json");
        fs::rename(&store_path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &store_path).unwrap();
        let symlink = call(&paths, "profiles.export", params.clone()).unwrap();
        assert_eq!(symlink["error"]["code"], "internal_error");
        fs::remove_file(&store_path).unwrap();
        fs::rename(&target, &store_path).unwrap();
        let mut large = document.clone();
        large["profiles"][0]["uri"] = json!(format!("{expected}{}", "x".repeat(32 * 1024)));
        fs::write(&store_path, serde_json::to_vec(&large).unwrap()).unwrap();
        let oversized = call(&paths, "profiles.export", params.clone()).unwrap();
        assert_eq!(oversized["error"]["code"], "internal_error");
        assert!(!oversized.to_string().contains(&expected));
        fs::write(&store_path, &original).unwrap();
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            let revoked = call(&paths, "profiles.export", params.clone()).unwrap();
            assert_eq!(revoked["error"]["code"], "capability_unavailable");
            assert!(!revoked.to_string().contains(&expected));
        }
        assert_eq!(calls.load(Ordering::Relaxed), calls_before);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn custom_rules_read_is_bounded_fenced_private_and_never_mutates() {
        let base = temporary_base("custom-rules");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let store_path = base.join("config/profiles.json");
        let mut document: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        let profile_uri = document["profiles"][0]["uri"].as_str().unwrap().to_owned();
        document["customRules"] = json!((0..omavless_domain::routing::MAX_CUSTOM_RULES).map(|i| {
            json!({"id":format!("00000000-0000-4000-8000-{i:012}"),"kind":"domain","action":"proxy","value":format!("r{i}.example.invalid"),"privateExtra":"private-token"})
        }).collect::<Vec<_>>());
        let original = serde_json::to_vec(&document).unwrap();
        fs::write(&store_path, &original).unwrap();
        let before_calls = calls.load(Ordering::Relaxed);
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(10)).unwrap());
        let method = "routing.custom_rules.list";
        let caps = call(&paths, "capabilities.get", json!({})).unwrap();
        assert!(
            caps["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == method)
        );
        let listed = call(&paths, method, json!({})).unwrap();
        assert_eq!(listed["revision"], 0);
        assert_eq!(listed["result"]["version"], 1);
        let rules = listed["result"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), omavless_domain::routing::MAX_CUSTOM_RULES);
        for (actual, expected) in rules
            .iter()
            .zip(document["customRules"].as_array().unwrap())
        {
            assert_eq!(actual.as_object().unwrap().len(), 4);
            for key in ["id", "kind", "action", "value"] {
                assert!(actual[key] == expected[key]);
            }
        }
        assert!(!listed.to_string().contains("private-token"));
        assert!(!listed.to_string().contains(&profile_uri));
        assert!(omavless_control_protocol::encode_response(&listed).is_ok());
        let status = call(&paths, "status.get", json!({})).unwrap();
        assert!(!status.to_string().contains("example.invalid"));
        let bad = call(&paths, method, json!({"path":"private-token"})).unwrap();
        assert_eq!(bad["error"]["code"], "invalid_argument");
        assert!(!bad.to_string().contains("private-token"));
        assert!(fs::read(&store_path).unwrap() == original);
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o644)).unwrap();
        let unsafe_store = call(&paths, method, json!({})).unwrap();
        assert_eq!(unsafe_store["error"]["code"], "internal_error");
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&store_path, b"private-token-corrupt").unwrap();
        let corrupt = call(&paths, method, json!({})).unwrap();
        assert_eq!(corrupt["error"]["code"], "internal_error");
        assert!(!corrupt.to_string().contains("private-token"));
        fs::write(&store_path, &original).unwrap();
        let target = base.join("config/temporary-store.json");
        fs::rename(&store_path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &store_path).unwrap();
        assert_eq!(
            call(&paths, method, json!({})).unwrap()["error"]["code"],
            "internal_error"
        );
        fs::remove_file(&store_path).unwrap();
        fs::rename(&target, &store_path).unwrap();
        document["customRules"] = json!([]);
        fs::write(&store_path, serde_json::to_vec(&document).unwrap()).unwrap();
        assert_eq!(
            call(&paths, method, json!({})).unwrap()["result"]["rules"],
            json!([])
        );
        fs::write(&store_path, &original).unwrap();
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            let revoked = call(&paths, method, json!({})).unwrap();
            assert_eq!(revoked["error"]["code"], "capability_unavailable");
            assert!(!revoked.to_string().contains("example.invalid"));
        }
        assert!(fs::read(&store_path).unwrap() == original);
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn support_report_socket_is_shareable_read_only_and_generation_fenced() {
        let base = temporary_base("support-report");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let store_path = base.join("config/profiles.json");
        let original = fs::read(&store_path).unwrap();
        let before_calls = calls.load(Ordering::Relaxed);
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(9)).unwrap());
        let method = "diagnostics.export";
        let caps = call(&paths, "capabilities.get", json!({})).unwrap();
        assert!(
            caps["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m == method)
        );
        let report = call(&paths, method, json!({})).unwrap();
        assert_eq!(report["ok"], true);
        assert_eq!(report["revision"], 0);
        assert_eq!(report["result"]["scope"], "native_configuration");
        assert_eq!(
            report["result"]["runtime"]["lastKnownState"],
            "disconnected"
        );
        assert_eq!(report["result"]["coverage"]["liveHostObservation"], false);
        assert_eq!(
            report["result"]["configuration"]["inventory"]["profiles"],
            1
        );
        assert!(
            omavless_control_protocol::encode_response(&report)
                .unwrap()
                .len()
                < 4096
        );
        for forbidden in [
            PROFILE_ID,
            "://",
            "192.0.2",
            "Example",
            "profileId",
            "private-token",
        ] {
            assert!(!report.to_string().contains(forbidden));
        }
        let bad = call(&paths, method, json!({"path":"private-token"})).unwrap();
        assert_eq!(bad["error"]["code"], "invalid_argument");
        assert!(!bad.to_string().contains("private-token"));
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            call(&paths, method, json!({})).unwrap()["error"]["code"],
            "internal_error"
        );
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&store_path, b"private-token-corrupt").unwrap();
        let corrupt = call(&paths, method, json!({})).unwrap();
        assert_eq!(corrupt["error"]["code"], "internal_error");
        assert!(!corrupt.to_string().contains("private-token"));
        fs::write(&store_path, &original).unwrap();
        let target = base.join("config/temporary-store.json");
        fs::rename(&store_path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &store_path).unwrap();
        assert_eq!(
            call(&paths, method, json!({})).unwrap()["error"]["code"],
            "internal_error"
        );
        fs::remove_file(&store_path).unwrap();
        fs::rename(&target, &store_path).unwrap();
        assert_eq!(
            call(&paths, method, json!({})).unwrap()["result"],
            report["result"]
        );
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            assert_eq!(
                call(&paths, method, json!({})).unwrap()["error"]["code"],
                "capability_unavailable"
            );
        }
        worker.join().unwrap();
        assert!(fs::read(&store_path).unwrap() == original);
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn diagnostics_do_not_block_disconnect_and_reject_stale_results() {
        use std::io::{Read, Write};
        use std::sync::mpsc;
        use std::time::Instant;
        let base = temporary_base("diagnostic-detached");
        let (owner, cutover, _) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let server = Arc::new(
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap(),
        );
        let request = |method, params| make_request("diagnostic-test", method, params).unwrap();
        let invalid = server
            .dispatch(&request(
                "diagnostics.rules",
                json!({"path":"private-token"}),
            ))
            .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_argument");
        assert!(!invalid.to_string().contains("private-token"));
        let disconnected = server
            .dispatch(&request("diagnostics.summary", json!({})))
            .unwrap();
        assert_eq!(disconnected["error"]["code"], "capability_unavailable");
        let connected = server
            .dispatch(&request(
                "connection.connect",
                json!({"profileId":PROFILE_ID}),
            ))
            .unwrap();
        assert_eq!(connected["ok"], true);
        let controller = paths.directory.join("mihomo.sock");
        let listener = UnixListener::bind(&controller).unwrap();
        fs::set_permissions(&controller, fs::Permissions::from_mode(0o600)).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let core = thread::spawn(move || {
            for round in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0; 1024];
                let size = stream.read(&mut buffer).unwrap();
                assert!(buffer[..size].starts_with(b"GET /rules HTTP/1.0\r\n"));
                if round == 1 {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                let payload = json!({"rules":[{"type":"DOMAIN","payload":"11111111-1111-4111-8111-111111111111","proxy":"private-group"}]}).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    payload.len(),
                    payload
                )
                .unwrap();
            }
        });
        let before = fs::read(base.join("config/profiles.json")).unwrap();
        let success = server
            .dispatch(&request("diagnostics.rules", json!({})))
            .unwrap();
        assert_eq!(success["ok"], true);
        assert_eq!(
            success["result"]["rules"]["items"][0]["payload"],
            "[private]"
        );
        assert!(!success.to_string().contains("private-group"));
        assert!(fs::read(base.join("config/profiles.json")).unwrap() == before);
        let reader_server = Arc::clone(&server);
        let reader = thread::spawn(move || {
            reader_server
                .dispatch(&make_request("slow", "diagnostics.rules", json!({})).unwrap())
                .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        let status = server.dispatch(&request("status.get", json!({}))).unwrap();
        assert_eq!(status["ok"], true);
        let disconnected = server
            .dispatch(&request("connection.disconnect", json!({})))
            .unwrap();
        assert_eq!(disconnected["ok"], true);
        assert!(started.elapsed() < Duration::from_secs(1));
        release_tx.send(()).unwrap();
        let stale = reader.join().unwrap();
        assert_eq!(stale["error"]["code"], "conflict");
        assert!(stale.get("result").is_none());
        core.join().unwrap();
        write_marker(&cutover, OwnershipPhase::RollbackPreparing, 2);
        let revoked = server
            .dispatch(&request("diagnostics.providers", json!({})))
            .unwrap();
        assert_eq!(revoked["error"]["code"], "capability_unavailable");
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn routing_preset_socket_is_fenced_replay_safe_and_noop_aware() {
        let base = temporary_base("routing-preset");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let template = base.join("config/route-template.yaml");
        fs::write(
            &template,
            b"mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
        )
        .unwrap();
        fs::set_permissions(&template, fs::Permissions::from_mode(0o600)).unwrap();
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(12)).unwrap());
        let params = json!({"preset":"china-cn-direct","keepMode":true,"operationId":"preset","expectedRevision":0});
        let first = call(&paths, "routing.set_preset", params.clone()).unwrap();
        assert_eq!(first["ok"], true);
        assert_eq!(first["revision"], 1);
        let store = base.join("config/profiles.json");
        let bytes = fs::read(&store).unwrap();
        let selected = fs::read(&template).unwrap();
        let before_calls = calls.load(Ordering::Relaxed);
        assert_eq!(
            call(&paths, "routing.set_preset", params.clone()).unwrap()["revision"],
            1
        );
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        let noop = call(
            &paths,
            "routing.set_preset",
            json!({"preset":"china-cn-direct","keepMode":true}),
        )
        .unwrap();
        assert_eq!(noop["ok"], true);
        assert_eq!(noop["revision"], 1);
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        let mut metadata: Value = serde_json::from_slice(&bytes).unwrap();
        metadata["routingPreset"] = json!("custom");
        fs::write(&store, serde_json::to_vec(&metadata).unwrap()).unwrap();
        let metadata_only = call(
            &paths,
            "routing.set_preset",
            json!({"preset":"china-cn-direct","keepMode":true}),
        )
        .unwrap();
        assert_eq!(metadata_only["ok"], true);
        assert_eq!(metadata_only["revision"], 2);
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        assert_eq!(
            call(
                &paths,
                "routing.set_preset",
                json!({"preset":"iran-ir-direct","expectedRevision":0})
            )
            .unwrap()["error"]["code"],
            "conflict"
        );
        let invalid = call(
            &paths,
            "routing.set_preset",
            json!({"preset":"china-cn-direct","path":"private-token"}),
        )
        .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_argument");
        assert!(!invalid.to_string().contains("private-token"));
        fs::set_permissions(&template, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            call(
                &paths,
                "routing.set_preset",
                json!({"preset":"iran-ir-direct"})
            )
            .unwrap()["error"]["code"],
            "internal_error"
        );
        fs::set_permissions(&template, fs::Permissions::from_mode(0o600)).unwrap();
        let pending = DesiredPaths::below(&base.join("state"))
            .directory
            .join("routing-preset.pending.json");
        fs::write(
            &pending,
            b"{\"schemaVersion\":1,\"kind\":\"routing-preset\"}\n",
        )
        .unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        let before_pending_calls = calls.load(Ordering::Relaxed);
        for (method, parameters) in [
            ("routing.set_preset", params.clone()),
            (
                "profiles.rename",
                json!({"profileId":PROFILE_ID,"name":"Synthetic"}),
            ),
            ("connection.disconnect", json!({})),
        ] {
            let response = call(&paths, method, parameters).unwrap();
            assert_eq!(response["error"]["code"], "manual_recovery_required");
            assert_eq!(response["revision"], 2);
        }
        assert_eq!(calls.load(Ordering::Relaxed), before_pending_calls);
        assert!(pending.exists());
        fs::remove_file(&pending).unwrap(); // Test fixture only, never runtime recovery.
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            assert_eq!(
                call(&paths, "routing.set_preset", params.clone()).unwrap()["error"]["code"],
                "capability_unavailable"
            );
        }
        assert!(fs::read(&store).unwrap() == bytes);
        assert!(fs::read(&template).unwrap() == selected);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn onboarding_socket_is_store_only_idempotent_and_fenced() {
        let base = temporary_base("onboarding-completion");
        let (owner, cutover, host_calls) = native_owner_fixture(&base);
        let store = base.join("config/profiles.json");
        let mut document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        document["onboardingComplete"] = json!(false);
        fs::write(&store, serde_json::to_vec(&document).unwrap()).unwrap();
        let initial_calls = host_calls.load(Ordering::Relaxed);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(11)).unwrap());
        let capabilities = call(&paths, "capabilities.get", json!({})).unwrap();
        assert!(capabilities.to_string().contains("onboarding.complete"));
        let params = json!({"operationId":"completion","expectedRevision":0});
        let first = call(&paths, "onboarding.complete", params.clone()).unwrap();
        assert_eq!(first["result"], json!({"accepted":true}));
        assert_eq!(first["revision"], 1);
        let bytes = fs::read(&store).unwrap();
        let saved: Value = serde_json::from_slice(&bytes).unwrap();
        document["onboardingComplete"] = json!(true);
        // Fixture normalization may fill legacy optional fields; compare
        // normalized documents, not newly invented settings or host intent.
        for (key, value) in document.as_object().unwrap() {
            if key != "profiles" {
                assert!(saved.get(key) == Some(value));
            }
        }
        assert_eq!(
            fs::metadata(&store).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            call(&paths, "onboarding.complete", params.clone()).unwrap()["revision"],
            1
        );
        let no_change = call(
            &paths,
            "onboarding.complete",
            json!({"operationId":"new-completion","expectedRevision":1}),
        )
        .unwrap();
        assert_eq!(no_change["ok"], true);
        assert_eq!(no_change["revision"], 1);
        assert_eq!(
            call(
                &paths,
                "connection.disconnect",
                json!({"operationId":"completion"})
            )
            .unwrap()["error"]["code"],
            "conflict"
        );
        assert_eq!(
            call(&paths, "onboarding.complete", json!({"expectedRevision":0})).unwrap()["error"]["code"],
            "conflict"
        );
        let invalid = call(
            &paths,
            "onboarding.complete",
            json!({"path":"private-marker"}),
        )
        .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_argument");
        assert!(!invalid.to_string().contains("private-marker"));
        let pending = DesiredPaths::below(&base.join("state"))
            .directory
            .join("routing-preset.pending.json");
        fs::write(
            &pending,
            b"{\"schemaVersion\":1,\"kind\":\"routing-preset\"}\n",
        )
        .unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            call(&paths, "onboarding.complete", params.clone()).unwrap()["error"]["code"],
            "manual_recovery_required"
        );
        fs::remove_file(&pending).unwrap(); // Synthetic fixture, not a recovery API.
        fs::set_permissions(&store, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            call(&paths, "onboarding.complete", json!({})).unwrap()["error"]["code"],
            "internal_error"
        );
        fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            assert_eq!(
                call(&paths, "onboarding.complete", params.clone()).unwrap()["error"]["code"],
                "capability_unavailable"
            );
        }
        assert_eq!(fs::read(&store).unwrap(), bytes);
        assert_eq!(host_calls.load(Ordering::Relaxed), initial_calls);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn custom_rule_mutation_socket_fences_private_values_and_replays_once() {
        let base = temporary_base("rule-mutations");
        let (owner, cutover, _calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(12)).unwrap());
        let params = json!({"kind":"domain","action":"direct","value":"example.invalid","operationId":"rule-add","expectedRevision":0});
        let first = call(&paths, "routing.custom_rules.add", params.clone()).unwrap();
        assert_eq!(first["ok"], true);
        assert_eq!(first["revision"], 1);
        assert_eq!(first["result"], json!({"accepted":true}));
        assert!(!first.to_string().contains("example.invalid"));
        let store_path = base.join("config/profiles.json");
        let bytes = fs::read(&store_path).unwrap();
        let payload: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload["customRules"].as_array().unwrap().len(), 1);
        let replay = call(&paths, "routing.custom_rules.add", params.clone()).unwrap();
        assert_eq!(replay["revision"], 1);
        assert!(fs::read(&store_path).unwrap() == bytes);
        let mut conflict = params.clone();
        conflict["action"] = json!("reject");
        assert_eq!(
            call(&paths, "routing.custom_rules.add", conflict).unwrap()["error"]["code"],
            "conflict"
        );
        let mut invalid = params.clone();
        invalid["path"] = json!("private-token");
        let error = call(&paths, "routing.custom_rules.add", invalid).unwrap();
        assert_eq!(error["error"]["code"], "invalid_argument");
        assert!(!error.to_string().contains("private-token"));
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            call(
                &paths,
                "routing.custom_rules.delete",
                json!({"ruleId":payload["customRules"][0]["id"]})
            )
            .unwrap()["error"]["code"],
            "internal_error"
        );
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let delete = json!({"ruleId":payload["customRules"][0]["id"],"operationId":"rule-delete","expectedRevision":1});
        assert_eq!(
            call(&paths, "routing.custom_rules.delete", delete.clone()).unwrap()["revision"],
            2
        );
        assert_eq!(
            call(&paths, "routing.custom_rules.delete", delete.clone()).unwrap()["revision"],
            2
        );
        assert_eq!(
            call(
                &paths,
                "routing.custom_rules.delete",
                json!({"ruleId":payload["customRules"][0]["id"]})
            )
            .unwrap()["error"]["code"],
            "not_found"
        );
        let after = fs::read(&store_path).unwrap();
        // A interrupted preset must also fence cached custom-rule replies.
        let pending = DesiredPaths::below(&base.join("state"))
            .directory
            .join("routing-preset.pending.json");
        fs::write(
            &pending,
            b"{\"schemaVersion\":1,\"kind\":\"routing-preset\"}\n",
        )
        .unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        let before_pending_calls = _calls.load(Ordering::Relaxed);
        for (method, cached) in [
            ("routing.custom_rules.add", params.clone()),
            ("routing.custom_rules.delete", delete),
        ] {
            let blocked = call(&paths, method, cached).unwrap();
            assert_eq!(blocked["error"]["code"], "manual_recovery_required");
            assert_eq!(blocked["revision"], 2);
        }
        assert_eq!(_calls.load(Ordering::Relaxed), before_pending_calls);
        assert!(fs::read(&store_path).unwrap() == after);
        assert!(pending.exists());
        fs::remove_file(&pending).unwrap(); // Synthetic fixture, not runtime recovery.
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            assert_eq!(
                call(&paths, "routing.custom_rules.add", params.clone()).unwrap()["error"]["code"],
                "capability_unavailable"
            );
        }
        assert!(fs::read(&store_path).unwrap() == after);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn route_check_fastpaths_are_private_fenced_and_do_not_invent_live_results() {
        let base = temporary_base("route-fast");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(11)).unwrap());
        let caps = call(&paths, "capabilities.get", json!({})).unwrap();
        assert!(
            caps["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method == "routing.check")
        );
        let before = fs::read(base.join("config/profiles.json")).unwrap();
        let host_before = calls.load(Ordering::Relaxed);
        let unknown = call(&paths, "routing.check", json!({"query":"PRIVATE.EXAMPLE."})).unwrap();
        assert_eq!(unknown["ok"], true);
        assert_eq!(unknown["result"]["query"], "private.example");
        assert_eq!(unknown["result"]["source"], "disconnected");
        assert_eq!(unknown["revision"], 0);
        for params in [
            json!({"query":"private.example","mode":"global"}),
            json!({"query":"https://private.example/private-token"}),
        ] {
            let rejected = call(&paths, "routing.check", params).unwrap();
            assert_eq!(rejected["error"]["code"], "invalid_argument");
            assert!(!rejected.to_string().contains("private.example"));
            assert!(!rejected.to_string().contains("private-token"));
        }
        assert!(fs::read(base.join("config/profiles.json")).unwrap() == before);
        assert_eq!(calls.load(Ordering::Relaxed), host_before);
        assert_eq!(
            call(&paths, "routing.set_mode", json!({"mode":"global"})).unwrap()["ok"],
            true
        );
        let mode = call(&paths, "routing.check", json!({"query":"private.example"})).unwrap();
        assert_eq!(mode["result"]["source"], "mode");
        assert_eq!(mode["result"]["outcome"], "vpn");
        assert_eq!(
            call(&paths, "routing.set_mode", json!({"mode":"rule"})).unwrap()["ok"],
            true
        );
        assert_eq!(
            call(
                &paths,
                "connection.connect",
                json!({"profileId":PROFILE_ID})
            )
            .unwrap()["ok"],
            true
        );
        let unavailable =
            call(&paths, "routing.check", json!({"query":"private.example"})).unwrap();
        assert_eq!(unavailable["error"]["code"], "capability_unavailable");
        assert!(unavailable.get("result").is_none());
        assert_eq!(
            call(&paths, "connection.disconnect", json!({})).unwrap()["ok"],
            true
        );
        write_marker(&cutover, OwnershipPhase::RollbackPreparing, 2);
        let revoked = call(&paths, "routing.check", json!({"query":"private.example"})).unwrap();
        assert_eq!(revoked["error"]["code"], "capability_unavailable");
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn detached_route_observation_rechecks_store_desired_config_and_owner() {
        use std::io::{Read, Write};
        for change in ["store", "desired", "config", "owner", "disconnect"] {
            let base = temporary_base("route-fence");
            let (owner, cutover, _calls) =
                owner_fixture_with_route(&base, OwnershipPhase::Rust, true);
            let paths = RuntimePaths::below(&base.join("runtime"));
            let server =
                RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
            let worker = thread::spawn(move || server.serve(Some(4)).unwrap());
            assert_eq!(
                call(
                    &paths,
                    "connection.connect",
                    json!({"profileId":PROFILE_ID})
                )
                .unwrap()["ok"],
                true
            );
            let socket = paths.directory.join("mihomo.sock");
            let controller = UnixListener::bind(&socket).unwrap();
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
            let (started, waiting) = std::sync::mpsc::channel();
            let (release, released) = std::sync::mpsc::channel();
            let http = thread::spawn(move || {
                let (mut stream, _) = controller.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = [0; 512];
                assert!(stream.read(&mut bytes).unwrap() > 0);
                started.send(()).unwrap();
                released.recv_timeout(Duration::from_secs(3)).unwrap();
                let body = b"{\"mixed-port\":0,\"mode\":\"rule\"}";
                write!(
                    stream,
                    "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            });
            let query_paths = paths.clone();
            let query = thread::spawn(move || {
                call(
                    &query_paths,
                    "routing.check",
                    json!({"query":"never-sent.example"}),
                )
                .unwrap()
            });
            waiting.recv_timeout(Duration::from_secs(2)).unwrap();
            // Status remains admitted while the controller is deliberately blocked.
            assert_eq!(call(&paths, "status.get", json!({})).unwrap()["ok"], true);
            match change {
                "store" => {
                    let path = base.join("config/profiles.json");
                    let mut store: Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    store["onboardingComplete"] = json!(false);
                    fs::write(path, serde_json::to_vec(&store).unwrap()).unwrap();
                }
                "desired" => {
                    let desired_paths = DesiredPaths::below(&base.join("state"));
                    let mut desired =
                        crate::desired::read_desired(&desired_paths, Uid::current().as_raw())
                            .unwrap();
                    desired.connected = false;
                    desired.profile_id.clear();
                    write_desired(&desired_paths, Uid::current().as_raw(), &desired).unwrap();
                }
                "config" => fs::write(base.join("config/route.yaml"), b"changed").unwrap(),
                "owner" => write_marker(&cutover, OwnershipPhase::RollbackPreparing, 2),
                "disconnect" => {
                    assert_eq!(
                        call(&paths, "connection.disconnect", json!({})).unwrap()["ok"],
                        true
                    );
                }
                _ => unreachable!(),
            }
            release.send(()).unwrap();
            let response = query.join().unwrap();
            let expected = if matches!(change, "owner" | "desired") {
                "capability_unavailable"
            } else {
                "conflict"
            };
            assert_eq!(response["error"]["code"], expected, "{change}");
            assert!(!response.to_string().contains("never-sent.example"));
            if change != "disconnect" {
                let _ = call(&paths, "connection.disconnect", json!({})).unwrap();
            }
            http.join().unwrap();
            worker.join().unwrap();
            fs::remove_dir_all(base).unwrap();
        }
    }

    #[test]
    fn profile_editor_read_is_fenced_private_and_never_mutates() {
        let base = temporary_base("profile-editor");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let store_path = base.join("config/profiles.json");
        let original = fs::read(&store_path).unwrap();
        let document: Value = serde_json::from_slice(&original).unwrap();
        let expected = document["profiles"][0]["uri"].as_str().unwrap();
        let before_calls = calls.load(Ordering::Relaxed);
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(11)).unwrap());
        let params = json!({"profileId":PROFILE_ID});
        let caps = call(&paths, "capabilities.get", json!({})).unwrap();
        assert!(
            caps["result"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "profiles.edit_input")
        );
        let input = call(&paths, "profiles.edit_input", params.clone()).unwrap();
        assert_eq!(input["revision"], 0);
        assert!(
            input["result"] == json!({"name":document["profiles"][0]["name"],"input":expected}),
            "editor response mismatch"
        );
        let list = call(&paths, "profiles.list", json!({})).unwrap();
        assert!(!list.to_string().contains(expected));
        let missing = call(
            &paths,
            "profiles.edit_input",
            json!({"profileId":"00000000-0000-4000-8000-000000000099"}),
        )
        .unwrap();
        assert_eq!(missing["error"]["code"], "not_found");
        let bad = call(
            &paths,
            "profiles.edit_input",
            json!({"profileId":PROFILE_ID,"path":"private-token"}),
        )
        .unwrap();
        assert_eq!(bad["error"]["code"], "invalid_argument");
        assert!(!bad.to_string().contains("private-token"));
        assert!(fs::read(&store_path).unwrap() == original);
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        let mut managed = document.clone();
        managed["profiles"][0]["subscriptionId"] = managed["subscriptions"][0]["id"].clone();
        managed["profiles"][0]["subscriptionKey"] = json!("a".repeat(64));
        fs::write(&store_path, serde_json::to_vec(&managed).unwrap()).unwrap();
        let rejected = call(&paths, "profiles.edit_input", params.clone()).unwrap();
        assert_eq!(rejected["error"]["code"], "invalid_argument");
        assert!(!rejected.to_string().contains(expected));
        fs::write(&store_path, &original).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o644)).unwrap();
        let unsafe_store = call(&paths, "profiles.edit_input", params.clone()).unwrap();
        assert_eq!(unsafe_store["error"]["code"], "internal_error");
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&store_path, b"private-token-corrupt").unwrap();
        let corrupt = call(&paths, "profiles.edit_input", params.clone()).unwrap();
        assert_eq!(corrupt["error"]["code"], "internal_error");
        assert!(!corrupt.to_string().contains("private-token"));
        fs::write(&store_path, &original).unwrap();
        let target = base.join("config/temporary-store.json");
        fs::rename(&store_path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &store_path).unwrap();
        let symlink = call(&paths, "profiles.edit_input", params.clone()).unwrap();
        assert_eq!(symlink["error"]["code"], "internal_error");
        fs::remove_file(&store_path).unwrap();
        fs::rename(&target, &store_path).unwrap();
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            let revoked = call(&paths, "profiles.edit_input", params.clone()).unwrap();
            assert_eq!(revoked["error"]["code"], "capability_unavailable");
            assert!(!revoked.to_string().contains(expected));
        }
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn profile_import_commits_once_without_host_work_and_rejects_replay_conflicts() {
        let base = temporary_base("profile-import");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let before_calls = calls.load(Ordering::Relaxed);
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(8)).unwrap());
        let params = json!({
            "name": "Imported",
            "input": "trojan://synthetic-password@203.0.113.1:443#Synthetic",
            "operationId": "import-1", "expectedRevision": 0,
        });
        let first = call(&paths, "profiles.import", params.clone()).unwrap();
        assert_eq!(first["ok"], true);
        assert_eq!(first["revision"], 1);
        assert_eq!(first["result"], json!({"accepted": true}));
        let store_path = base.join("config/profiles.json");
        let original = fs::read(&store_path).unwrap();
        let store: Value = serde_json::from_slice(&original).unwrap();
        assert_eq!(store["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(store["profiles"][1]["protocol"], "trojan");
        assert_eq!(store["activeId"], "");
        assert_eq!(store["lastId"], PROFILE_ID);
        assert!(omavless_domain::store::valid_record_id(
            store["profiles"][1]["id"].as_str().unwrap()
        ));
        assert_eq!(
            fs::metadata(&store_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let replay = call(&paths, "profiles.import", params.clone()).unwrap();
        assert_eq!(replay["revision"], 1);
        assert_eq!(replay["result"], first["result"]);
        let mut conflict = params.clone();
        conflict["name"] = json!("Different");
        let conflict = call(&paths, "profiles.import", conflict).unwrap();
        assert_eq!(conflict["error"]["code"], "conflict");
        let mut duplicate = params.clone();
        duplicate["operationId"] = json!("import-2");
        duplicate["expectedRevision"] = json!(1);
        let duplicate = call(&paths, "profiles.import", duplicate).unwrap();
        assert_eq!(duplicate["ok"], false);
        let invalid = call(
            &paths,
            "profiles.import",
            json!({
                "name": "Invalid", "input": "https://example.invalid/private-token",
            }),
        )
        .unwrap();
        assert_eq!(invalid["ok"], false);
        let extra = call(
            &paths,
            "profiles.import",
            json!({
                "name": "Invalid", "input": "private-token", "profileId": PROFILE_ID,
            }),
        )
        .unwrap();
        assert_eq!(extra["error"]["code"], "invalid_argument");
        write_marker(&cutover, OwnershipPhase::RollbackPreparing, 2);
        let revoked = call(&paths, "profiles.import", params.clone()).unwrap();
        assert_eq!(revoked["error"]["code"], "capability_unavailable");
        write_marker(&cutover, OwnershipPhase::Rust, 3);
        let stale = call(&paths, "profiles.import", params).unwrap();
        assert_eq!(stale["error"]["code"], "capability_unavailable");
        assert_eq!(fs::read(&store_path).unwrap(), original);
        assert_eq!(calls.load(Ordering::Relaxed), before_calls);
        for response in [
            first, replay, conflict, duplicate, invalid, extra, revoked, stale,
        ] {
            for private in [
                "synthetic-password",
                "203.0.113.1",
                "private-token",
                "Imported",
            ] {
                assert!(!response.to_string().contains(private));
            }
        }
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn profile_replacement_dispatch_replays_without_second_transition_and_revokes() {
        let base = temporary_base("profile-replace");
        let (owner, cutover, calls) = native_owner_fixture(&base);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let server =
            RuntimeServer::bind_with_owner_factory(paths.clone(), move |_| Ok(owner)).unwrap();
        let worker = thread::spawn(move || server.serve(Some(7)).unwrap());
        let connected = call(
            &paths,
            "connection.connect",
            json!({"profileId": PROFILE_ID, "mode": "global"}),
        )
        .unwrap();
        assert_eq!(connected["ok"], true);
        let params = json!({"profileId": PROFILE_ID, "name": "Replaced",
            "input": "trojan://synthetic-password@203.0.113.1:443",
            "operationId": "replace-1", "expectedRevision": 1});
        let changed = call(&paths, "profiles.replace", params.clone()).unwrap();
        assert_eq!(changed["ok"], true);
        assert_eq!(changed["revision"], 2);
        let count = calls.load(Ordering::Relaxed);
        let replay = call(&paths, "profiles.replace", params.clone()).unwrap();
        assert_eq!(replay["revision"], 2);
        let mut noop = params.clone();
        noop["operationId"] = json!("replace-noop");
        noop["expectedRevision"] = json!(2);
        let noop = call(&paths, "profiles.replace", noop).unwrap();
        assert_eq!(noop["ok"], true);
        assert_eq!(noop["revision"], 2);
        assert_eq!(calls.load(Ordering::Relaxed), count);
        let store_path = base.join("config/profiles.json");
        let before = fs::read(&store_path).unwrap();
        let store: Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(store["activeId"], PROFILE_ID);
        assert_eq!(store["profiles"][0]["protocol"], "trojan");
        let invalid = call(
            &paths,
            "profiles.replace",
            json!({
                "profileId": PROFILE_ID, "name": "New", "input": "https://example.invalid/token",
            }),
        )
        .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_argument");
        for (phase, generation) in [
            (OwnershipPhase::RollbackPreparing, 2),
            (OwnershipPhase::Rust, 3),
        ] {
            write_marker(&cutover, phase, generation);
            let rejected = call(&paths, "profiles.replace", params.clone()).unwrap();
            assert_eq!(rejected["error"]["code"], "capability_unavailable");
            assert!(!rejected.to_string().contains("synthetic-password"));
        }
        assert!(fs::read(&store_path).unwrap() == before);
        assert_eq!(calls.load(Ordering::Relaxed), count);
        for response in [changed, replay, noop, invalid] {
            assert!(!response.to_string().contains("synthetic-password"));
            assert!(!response.to_string().contains("203.0.113.1"));
        }
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn transition_candidate_is_read_only_then_promotes_in_the_same_runtime() {
        let base = temporary_base("candidate");
        let (owner, cutover, calls) = owner_fixture(&base, OwnershipPhase::CutoverPreparing);
        let paths = RuntimePaths::below(&base.join("runtime"));
        let mut server = RuntimeServer::bind(paths.clone()).unwrap();
        server.register_native_owner(
            owner,
            subscription_transport::HttpsSubscriptionTransport::new(),
        );
        let worker = thread::spawn(move || server.serve(Some(10)).unwrap());

        let hello_before = call(&paths, "system.hello", json!({"versions": [1]})).unwrap();
        let instance_id = hello_before["result"]["instanceId"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(hello_before["result"]["runtimeOwnership"], false);
        let status_before = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(status_before["result"]["actual"], "disconnected");
        assert_eq!(status_before["result"]["transition"], "cutoverPreparing");
        let capabilities_before = call(&paths, "capabilities.get", json!({})).unwrap();
        assert_eq!(capabilities_before["result"]["mutations"], false);
        let calls_before_rejection = calls.load(Ordering::Relaxed);
        let rejected = call(
            &paths,
            "connection.connect",
            json!({
                "profileId": PROFILE_ID,
                "mode": RoutingMode::Global.as_str(),
                "operationId": "candidate-connect",
                "expectedRevision": 0
            }),
        )
        .unwrap();
        assert_eq!(rejected["error"]["code"], "capability_unavailable");
        assert_eq!(calls.load(Ordering::Relaxed), calls_before_rejection);

        let bootstrap = call(
            &paths,
            "runtime.transitionBootstrap",
            json!({"preparingGeneration": 1}),
        )
        .unwrap();
        assert_eq!(bootstrap["ok"], true);
        assert_eq!(bootstrap["result"]["instanceId"], instance_id);
        assert_eq!(bootstrap["result"]["rustGeneration"], 2);
        let wrong_bootstrap = call(
            &paths,
            "runtime.transitionBootstrap",
            json!({"preparingGeneration": 2}),
        )
        .unwrap();
        assert_eq!(wrong_bootstrap["error"]["code"], "conflict");

        write_marker(&cutover, OwnershipPhase::Rust, 2);
        let capabilities_after = call(&paths, "capabilities.get", json!({})).unwrap();
        assert_eq!(capabilities_after["result"]["mutations"], true);
        let hello_after = call(&paths, "system.hello", json!({"versions": [1]})).unwrap();
        assert_eq!(hello_after["result"]["instanceId"], instance_id);
        assert_eq!(hello_after["result"]["runtimeOwnership"], true);
        let status_after = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(status_after["result"]["transition"], Value::Null);
        let connected = call(
            &paths,
            "connection.connect",
            json!({
                "profileId": PROFILE_ID,
                "mode": RoutingMode::Global.as_str(),
                "operationId": "promoted-connect",
                "expectedRevision": 0
            }),
        )
        .unwrap();
        assert_eq!(connected["ok"], true);

        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn owner_constructor_failure_aborts_and_removes_socket() {
        let base = temporary_base("native-constructor-failure");
        let paths = RuntimePaths::below(&base);
        let result = RuntimeServer::bind_with_owner_factory::<FakeHost, _>(paths.clone(), |_| {
            Err(production_owner::ProductionOwnerError::HostUnavailable)
        });
        assert!(matches!(result, Err(RuntimeError::NativeOwnerUnavailable)));
        assert!(!paths.socket.exists());

        let server = RuntimeServer::bind(paths.clone()).unwrap();
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn errors_are_stable_and_do_not_echo_private_input() {
        let request = make_request("safe", "private.example/password", json!({})).unwrap();
        let response = dispatch_read_only(&request, "instance").unwrap();
        let rendered = serde_json::to_string(&response).unwrap();
        assert_eq!(response["error"]["code"], "unknown_method");
        assert!(!rendered.contains("private.example"));
        assert!(!rendered.contains("password"));
    }

    #[test]
    fn malformed_client_does_not_terminate_runtime() {
        use std::io::{Read, Write};

        let base = temporary_base("malformed");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        let worker = thread::spawn(move || server.serve(Some(2)).unwrap());

        let mut invalid = UnixStream::connect(&paths.socket).unwrap();
        invalid.write_all(b"private.example/password\n").unwrap();
        invalid.shutdown(std::net::Shutdown::Write).unwrap();
        let mut safe_error = String::new();
        invalid.read_to_string(&mut safe_error).unwrap();
        assert!(safe_error.contains("invalid_request"));
        assert!(!safe_error.contains("private.example"));
        assert!(!safe_error.contains("password"));

        let response = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(response["ok"], true);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }
}
