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

pub mod connection_transaction;
pub mod core;
pub mod cutover;
pub mod cutover_transaction;
pub mod desired;
pub mod frontend_bridge;
pub mod lifecycle;
pub mod mutation;
pub mod mutation_binding;
pub mod mutation_protocol;
pub mod native_coordinator;
pub mod native_dispatch;
pub mod native_host;
pub mod owner;
pub mod private_store_transaction;
pub mod production_cutover;
pub mod production_observation;
pub mod production_owner;
pub mod profile_mutation;
pub mod profile_mutation_protocol;
pub mod profile_transaction;
pub mod semantic_cli;
pub mod store_bootstrap;
pub mod store_preflight;
pub mod subscription_mutation;
pub mod subscription_mutation_protocol;
pub mod subscription_refresh;
pub mod subscription_refresh_protocol;
pub mod subscription_transport;

pub const SOCKET_NAME: &str = "control.sock";
pub const OWNER_LOCK_NAME: &str = "owner.lock";
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONCURRENT_CLIENTS: usize = 16;
const MAX_CONCURRENT_REMOTE_FETCHES: usize = 4;

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
    dispatcher: Mutex<RuntimeDispatcher>,
    remote_fetches: AtomicUsize,
    _owner: OwnerLock,
}

const READ_ONLY_METHODS: &[&str] = &["system.hello", "status.get", "capabilities.get"];
const NATIVE_READ_METHODS: &[&str] = &["profiles.list", "subscriptions.list"];
// Remote subscription fetch uses the bounded concurrent client layer and a
// reservation-free preflight. Its final decode/commit re-enters this one
// serialized owner and rechecks revision plus exact durable ownership.
const NATIVE_MUTATION_METHODS: &[&str] = &[
    "connection.connect",
    "connection.disconnect",
    "routing.set_mode",
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
    fn revision(&self) -> u64;
    fn runtime_ownership(&mut self) -> bool;
    fn status(&self, runtime_ownership: bool) -> Result<Value>;
    fn profiles(&self) -> Result<Value>;
    fn subscriptions(&self) -> Result<Value>;
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

#[derive(Clone)]
struct SharedSubscriptionTransport(
    Arc<dyn subscription_transport::SubscriptionTransport + Send + Sync>,
);

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
            dispatcher: Mutex::new(RuntimeDispatcher::ReadOnly),
            remote_fetches: AtomicUsize::new(0),
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
        T: subscription_transport::SubscriptionTransport + Send + Sync + 'static,
    {
        let registered = RegisteredNativeOwner {
            owner,
            transport: SharedSubscriptionTransport(Arc::new(transport)),
            record_ids: RecordIdGenerator::new(&self.instance_id),
        };
        self.dispatcher = Mutex::new(RuntimeDispatcher::Native(Box::new(registered)));
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
                    Err(_) => return Err(RuntimeError::Io),
                }
            }
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
        let Some(_remote_slot) = claim_slot(&self.remote_fetches, MAX_CONCURRENT_REMOTE_FETCHES)
        else {
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
    let mut stream =
        UnixStream::connect(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
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

    impl lifecycle::LifecycleHost for FakeHost {
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
        let worker = thread::spawn(move || server.serve(Some(17)).unwrap());

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
        for method in ["profiles.list", "subscriptions.list"] {
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
