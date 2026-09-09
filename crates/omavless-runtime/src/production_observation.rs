// SPDX-License-Identifier: MIT

//! Read-only production-host observation for the R5 ownership preflight.
//!
//! This adapter executes only fixed `systemctl --user show` queries, reads the
//! private current store/config, probes the two fixed Mihomo Unix sockets, and
//! inspects bounded procfs/sysfs facts. It cannot start, stop, enable, disable,
//! or mutate either runtime owner.

use crate::RuntimePaths;
use crate::core_readiness::ConfigReadiness;
use crate::cutover::{
    CutoverError, CutoverPaths, CutoverReadiness, MigrationLock, OwnershipMarker,
    OwnershipObservation, evaluate_cutover, read_marker,
};
use crate::desired::RoutingMode;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::unistd::Uid;
use omavless_domain::config::MAX_TEMPLATE_BYTES;
use omavless_domain::private_store::parse_private_store;
use omavless_mihomo::observation::{
    UserServiceState, parse_systemd_show, process_family, processes_named, processes_named_strict,
    tun_interface_count, tun_interface_count_strict,
};
use omavless_mihomo::{MAX_CONTROLLER_HEADER_BYTES, ReadOnlyEndpoint, controller_get};
use omavless_store::read_private_utf8;
use serde_json::json;
use std::env;
use std::fmt;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const LEGACY_SERVICE: &str = "omavless.service";
pub(crate) const RUST_SERVICE: &str = "omavless-runtime.service";
pub(crate) const SERVICE_QUERY_TIMEOUT: Duration = Duration::from_secs(3);
const CONTROLLER_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_SERVICE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionObservationError {
    UnsafePath,
    ServiceQuery,
    ServiceResponse,
    PrivateState,
    HostNotEmpty,
    IncompleteInventory,
    Cutover(CutoverError),
}

impl fmt::Display for ProductionObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafePath => "OmaVLESS ownership observation path is unsafe",
            Self::ServiceQuery => "OmaVLESS service state could not be observed",
            Self::ServiceResponse => "OmaVLESS service state is invalid",
            Self::PrivateState => "OmaVLESS active private state could not be verified",
            Self::HostNotEmpty => "OmaVLESS host is not empty",
            Self::IncompleteInventory => "OmaVLESS host inventory could not be verified",
            Self::Cutover(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for ProductionObservationError {}

impl From<CutoverError> for ProductionObservationError {
    fn from(value: CutoverError) -> Self {
        Self::Cutover(value)
    }
}

#[derive(Clone)]
pub struct ProductionObservationPaths {
    pub systemctl: PathBuf,
    pub config_directory: PathBuf,
    pub runtime_base: PathBuf,
    pub proc_root: PathBuf,
    pub sys_class_net: PathBuf,
    pub legacy_controller: PathBuf,
    pub rust_controller: PathBuf,
    pub rust_control_socket: PathBuf,
    pub store: PathBuf,
    pub template: PathBuf,
    pub active_config: PathBuf,
}

impl ProductionObservationPaths {
    #[must_use]
    pub fn below(
        systemctl: PathBuf,
        home: &Path,
        runtime_base: &Path,
        proc_root: PathBuf,
        sys_class_net: PathBuf,
        uid: u32,
    ) -> Self {
        let config_directory = home.join(".config/omavless");
        let rust_runtime = RuntimePaths::below(runtime_base);
        Self {
            systemctl,
            store: config_directory.join("profiles.json"),
            template: config_directory.join("route-template.yaml"),
            active_config: config_directory.join("config.yaml"),
            legacy_controller: runtime_base.join(format!("omavless.{uid}.controller.sock")),
            rust_controller: rust_runtime.directory.join("mihomo.sock"),
            rust_control_socket: rust_runtime.socket,
            config_directory,
            runtime_base: runtime_base.to_owned(),
            proc_root,
            sys_class_net,
        }
    }

    pub fn current(uid: u32) -> Result<Self, ProductionObservationError> {
        let home = env::var_os("OMAVLESS_HOME")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or(ProductionObservationError::UnsafePath)?;
        let runtime_base = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")));
        let systemctl = PathBuf::from("/usr/bin/systemctl");
        if !home.is_absolute() || !runtime_base.is_absolute() || !systemctl.is_absolute() {
            return Err(ProductionObservationError::UnsafePath);
        }
        Ok(Self::below(
            systemctl,
            &home,
            &runtime_base,
            PathBuf::from("/proc"),
            PathBuf::from("/sys/class/net"),
            uid,
        ))
    }
}

fn valid_path(path: &Path) -> bool {
    let value = path.as_os_str().as_encoded_bytes();
    path.is_absolute() && !value.is_empty() && value.len() <= MAX_PATH_BYTES && !value.contains(&0)
}

fn executable_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
    })
}

fn ordinary_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
        && fs::read_dir(path).is_ok()
}

fn private_runtime_directory(path: &Path, uid: u32) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_dir()
            && metadata.uid() == uid
            && metadata.permissions().mode() & 0o077 == 0
    })
}

pub(crate) fn service_state_with_timeout(
    systemctl: &Path,
    service: &str,
    timeout: Duration,
) -> Result<UserServiceState, ProductionObservationError> {
    if !matches!(service, LEGACY_SERVICE | RUST_SERVICE)
        || timeout.is_zero()
        || timeout > SERVICE_QUERY_TIMEOUT
    {
        return Err(ProductionObservationError::ServiceQuery);
    }
    let mut child = Command::new(systemctl)
        .args([
            "--user",
            "show",
            service,
            "--property=ActiveState",
            "--property=MainPID",
            "--property=ExecMainStatus",
            "--property=Result",
            "--no-pager",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ProductionObservationError::ServiceQuery)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or(ProductionObservationError::ServiceQuery)?;
    let deadline = Instant::now() + timeout;
    let result = (|| {
        fcntl(&stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
            .map_err(|_| ProductionObservationError::ServiceQuery)?;
        let mut output = Vec::new();
        let mut eof = false;
        let mut status = None;
        loop {
            if Instant::now() >= deadline {
                return Err(ProductionObservationError::ServiceQuery);
            }
            if !eof {
                let mut buffer = [0u8; 4096];
                match stdout.read(&mut buffer) {
                    Ok(0) => eof = true,
                    Ok(count) => {
                        output.extend_from_slice(&buffer[..count]);
                        if output.len() > MAX_SERVICE_OUTPUT_BYTES {
                            return Err(ProductionObservationError::ServiceQuery);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => return Err(ProductionObservationError::ServiceQuery),
                }
            }
            if status.is_none() {
                status = child
                    .try_wait()
                    .map_err(|_| ProductionObservationError::ServiceQuery)?;
            }
            if let Some(status) = status {
                if !status.success() {
                    return Err(ProductionObservationError::ServiceQuery);
                }
                if eof {
                    return Ok(output);
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    })();
    if result.is_err() {
        // Only our direct query child is owned. Descendants cannot hold an
        // unbounded reader join open; their pipe is dropped on return.
        let _ = child.kill();
        let _ = child.wait();
    }
    let output = result?;
    let text =
        std::str::from_utf8(&output).map_err(|_| ProductionObservationError::ServiceResponse)?;
    for field in ["ActiveState", "MainPID", "ExecMainStatus", "Result"] {
        if text
            .lines()
            .filter(|line| line.split_once('=').is_some_and(|(key, _)| key == field))
            .count()
            != 1
        {
            return Err(ProductionObservationError::ServiceResponse);
        }
    }
    parse_systemd_show(text).ok_or(ProductionObservationError::ServiceResponse)
}

fn socket_presence(
    path: &Path,
    uid: u32,
    require_private_mode: bool,
) -> Result<bool, ProductionObservationError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ProductionObservationError::UnsafePath),
        Ok(metadata)
            if metadata.file_type().is_socket()
                && metadata.uid() == uid
                && (!require_private_mode || metadata.permissions().mode() & 0o077 == 0)
                && path
                    .parent()
                    .is_some_and(|parent| private_runtime_directory(parent, uid)) =>
        {
            Ok(true)
        }
        Ok(_) => Err(ProductionObservationError::UnsafePath),
    }
}

fn controller_ready(path: &Path, uid: u32) -> Result<bool, ProductionObservationError> {
    if !socket_presence(path, uid, false)? {
        return Ok(false);
    }
    Ok(controller_get(
        path,
        ReadOnlyEndpoint::Version,
        CONTROLLER_TIMEOUT,
        MAX_CONTROLLER_HEADER_BYTES,
    )
    .is_ok_and(|response| response.has_live_version()))
}

fn active_config_matches(
    paths: &ProductionObservationPaths,
    uid: u32,
    controller_path: &Path,
    core_pid: u32,
) -> Result<bool, ProductionObservationError> {
    let store_text = read_private_utf8(&paths.store, uid)
        .map_err(|_| ProductionObservationError::PrivateState)?;
    let template = read_private_utf8(&paths.template, uid)
        .map_err(|_| ProductionObservationError::PrivateState)?;
    let active = read_private_utf8(&paths.active_config, uid)
        .map_err(|_| ProductionObservationError::PrivateState)?;
    if template.len() > MAX_TEMPLATE_BYTES {
        return Err(ProductionObservationError::PrivateState);
    }
    let store =
        parse_private_store(&store_text).map_err(|_| ProductionObservationError::PrivateState)?;
    let controller = controller_path
        .to_str()
        .ok_or(ProductionObservationError::UnsafePath)?;
    let Some(mode) = store
        .matched_active_mode(&template, controller, &active)
        .map_err(|_| ProductionObservationError::PrivateState)?
    else {
        return Ok(false);
    };
    let mode = match mode.as_str() {
        "global" => RoutingMode::Global,
        "rule" => RoutingMode::Rule,
        "direct" => RoutingMode::Direct,
        _ => return Ok(false),
    };
    let projection = store.list_projection();
    let Some(profile) = projection
        .profiles()
        .iter()
        .find(|profile| Some(profile.id()) == store.active_profile_id())
    else {
        return Ok(false);
    };
    if !ConfigReadiness::new(mode, profile.name().to_owned()).ready_for_pid(
        controller_path,
        core_pid,
        Instant::now() + CONTROLLER_TIMEOUT,
    ) {
        return Ok(false);
    }
    // A result collected while an external writer replaced private state must
    // never authorize adoption of that replacement. No write or repair here.
    for (path, before) in [
        (&paths.store, &store_text),
        (&paths.template, &template),
        (&paths.active_config, &active),
    ] {
        if read_private_utf8(path, uid).map_err(|_| ProductionObservationError::PrivateState)?
            != *before
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub struct ProductionOwnershipObserver {
    paths: ProductionObservationPaths,
    uid: u32,
}

#[cfg(test)]
#[path = "production_empty_tests.rs"]
mod empty_tests;

impl ProductionOwnershipObserver {
    /// Verify two empty observations in this trusted procfs/sysfs/user-manager
    /// view. This is not an atomic reservation, namespace-completeness proof,
    /// login trigger or permission to start a core. No private config is read.
    pub fn verify_empty(&self) -> Result<(), ProductionObservationError> {
        for _ in 0..2 {
            if !private_runtime_directory(&self.paths.runtime_base, self.uid)
                || fs::canonicalize(&self.paths.runtime_base).ok().as_deref()
                    != Some(self.paths.runtime_base.as_path())
            {
                return Err(ProductionObservationError::UnsafePath);
            }
            for service in [LEGACY_SERVICE, RUST_SERVICE] {
                let state = service_state_with_timeout(
                    &self.paths.systemctl,
                    service,
                    SERVICE_QUERY_TIMEOUT,
                )?;
                if state.active || state.main_pid != 0 {
                    return Err(ProductionObservationError::HostNotEmpty);
                }
            }
            for socket in [
                &self.paths.legacy_controller,
                &self.paths.rust_controller,
                &self.paths.rust_control_socket,
            ] {
                self.require_absent_socket(socket)?;
            }
            let cores = processes_named_strict(&self.paths.proc_root, "mihomo")
                .map_err(|_| ProductionObservationError::IncompleteInventory)?;
            let tun = tun_interface_count_strict(&self.paths.sys_class_net)
                .map_err(|_| ProductionObservationError::IncompleteInventory)?;
            if !cores.is_empty() || tun != 0 {
                return Err(ProductionObservationError::HostNotEmpty);
            }
        }
        Ok(())
    }

    fn require_absent_socket(&self, path: &Path) -> Result<(), ProductionObservationError> {
        let relative = path
            .strip_prefix(&self.paths.runtime_base)
            .map_err(|_| ProductionObservationError::UnsafePath)?;
        let parent = relative
            .parent()
            .ok_or(ProductionObservationError::UnsafePath)?;
        let mut current = self.paths.runtime_base.clone();
        for component in parent.components() {
            let std::path::Component::Normal(component) = component else {
                return Err(ProductionObservationError::UnsafePath);
            };
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(_) if private_runtime_directory(&current, self.uid) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err(ProductionObservationError::UnsafePath),
            }
        }
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ProductionObservationError::UnsafePath),
            Ok(_) => Err(ProductionObservationError::HostNotEmpty),
        }
    }

    pub fn new(
        paths: ProductionObservationPaths,
        uid: u32,
    ) -> Result<Self, ProductionObservationError> {
        let paths_valid = [
            &paths.systemctl,
            &paths.config_directory,
            &paths.runtime_base,
            &paths.proc_root,
            &paths.sys_class_net,
            &paths.legacy_controller,
            &paths.rust_controller,
            &paths.rust_control_socket,
            &paths.store,
            &paths.template,
            &paths.active_config,
        ]
        .into_iter()
        .all(|path| valid_path(path));
        if !paths_valid
            || !executable_file(&paths.systemctl)
            || !private_runtime_directory(&paths.runtime_base, uid)
            || !ordinary_directory(&paths.proc_root)
            || !ordinary_directory(&paths.sys_class_net)
        {
            return Err(ProductionObservationError::UnsafePath);
        }
        Ok(Self { paths, uid })
    }

    pub fn current() -> Result<Self, ProductionObservationError> {
        let uid = Uid::current().as_raw();
        Self::new(ProductionObservationPaths::current(uid)?, uid)
    }

    pub fn observe(&self) -> Result<OwnershipObservation, ProductionObservationError> {
        let legacy = service_state_with_timeout(
            &self.paths.systemctl,
            LEGACY_SERVICE,
            SERVICE_QUERY_TIMEOUT,
        )?;
        let rust =
            service_state_with_timeout(&self.paths.systemctl, RUST_SERVICE, SERVICE_QUERY_TIMEOUT)?;
        let core_pids = processes_named(&self.paths.proc_root, "mihomo");
        let core_count = u8::try_from(core_pids.len()).unwrap_or(u8::MAX);
        let core_pid = core_pids.first().copied().unwrap_or(0);
        let legacy_family = process_family(legacy.main_pid, &self.paths.proc_root);
        let rust_family = process_family(rust.main_pid, &self.paths.proc_root);
        let exactly_one_legacy_core = legacy.active
            && core_pids.len() == 1
            && core_pids.iter().all(|pid| legacy_family.contains(pid));
        let exactly_one_rust_core = rust.active
            && core_pids.len() == 1
            && core_pids.iter().all(|pid| rust_family.contains(pid));
        let rust_control_present =
            socket_presence(&self.paths.rust_control_socket, self.uid, true)?;
        let active_profile_matches = if exactly_one_legacy_core {
            active_config_matches(
                &self.paths,
                self.uid,
                &self.paths.legacy_controller,
                core_pid,
            )?
        } else if exactly_one_rust_core {
            active_config_matches(&self.paths, self.uid, &self.paths.rust_controller, core_pid)?
        } else {
            false
        };
        let active_profile_matches = active_profile_matches
            && processes_named(&self.paths.proc_root, "mihomo") == core_pids
            && if exactly_one_legacy_core {
                process_family(legacy.main_pid, &self.paths.proc_root).contains(&core_pid)
            } else if exactly_one_rust_core {
                process_family(rust.main_pid, &self.paths.proc_root).contains(&core_pid)
            } else {
                false
            };
        Ok(OwnershipObservation {
            legacy_owner_active: legacy.active,
            rust_owner_active: rust.active || rust_control_present,
            legacy_controller_ready: controller_ready(&self.paths.legacy_controller, self.uid)?,
            rust_controller_ready: controller_ready(&self.paths.rust_controller, self.uid)?,
            core_count,
            tun_count: tun_interface_count(&self.paths.sys_class_net),
            active_profile_matches,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCutoverPreflight {
    pub marker: OwnershipMarker,
    pub observation: OwnershipObservation,
    pub readiness: CutoverReadiness,
}

impl ProductionCutoverPreflight {
    #[must_use]
    pub fn public_json(&self) -> serde_json::Value {
        json!({
            "schemaVersion": 1,
            "markerGeneration": self.marker.generation(),
            "markerPhase": self.marker.phase().as_str(),
            "readiness": self.readiness.as_str(),
            "legacyOwnerActive": self.observation.legacy_owner_active,
            "rustOwnerActive": self.observation.rust_owner_active,
            "legacyControllerReady": self.observation.legacy_controller_ready,
            "rustControllerReady": self.observation.rust_controller_ready,
            "coreCount": self.observation.core_count,
            "tunCount": self.observation.tun_count,
            "activeProfileMatches": self.observation.active_profile_matches,
        })
    }
}

pub fn current_cutover_preflight() -> Result<ProductionCutoverPreflight, ProductionObservationError>
{
    let uid = Uid::current().as_raw();
    let cutover_paths = CutoverPaths::current(uid)?;
    let _lock = MigrationLock::acquire(&cutover_paths, uid)?;
    let marker = read_marker(&cutover_paths, uid)?;
    let observation = ProductionOwnershipObserver::current()?.observe()?;
    let readiness = evaluate_cutover(&marker, observation);
    Ok(ProductionCutoverPreflight {
        marker,
        observation,
        readiness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;
    use std::sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(label: &str) -> (PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "omavless-production-observe-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn executable(root: &Path, body: &str) -> PathBuf {
        let path = root.join("systemctl");
        let staged = root.join(".systemctl.staged");
        fs::write(&staged, body).unwrap();
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(staged, &path).unwrap();
        // Some overlay-backed test runners briefly report ETXTBSY when an
        // executable is spawned immediately after publication. Production
        // calls the stable systemctl binary, so keep this bounded settling
        // delay confined to the ephemeral test fixture.
        thread::sleep(Duration::from_millis(20));
        path
    }

    fn observer_paths(root: &Path, uid: u32, systemctl: PathBuf) -> ProductionObservationPaths {
        let home = root.join("home");
        let config = home.join(".config/omavless");
        let runtime = root.join("runtime");
        let proc_root = root.join("proc");
        let sys_class_net = root.join("sys/class/net");
        fs::create_dir_all(&config).unwrap();
        fs::create_dir(&runtime).unwrap();
        fs::create_dir(&proc_root).unwrap();
        fs::create_dir_all(&sys_class_net).unwrap();
        for path in [&config, &runtime] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        ProductionObservationPaths::below(systemctl, &home, &runtime, proc_root, sys_class_net, uid)
    }

    fn inactive_systemctl(root: &Path) -> PathBuf {
        executable(
            root,
            "#!/bin/sh\nprintf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n'\n",
        )
    }

    fn configured_controller(
        listener: UnixListener,
        mode: &'static str,
        store: PathBuf,
    ) -> (Arc<AtomicU8>, thread::JoinHandle<()>) {
        let stage = Arc::new(AtomicU8::new(0));
        let serving = Arc::clone(&stage);
        listener.set_nonblocking(true).unwrap();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            while serving.load(Ordering::Acquire) != 255 && Instant::now() < deadline {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut bytes = [0; 1024];
                let Ok(n) = stream.read(&mut bytes) else {
                    continue;
                };
                if n == 0 {
                    continue;
                }
                let request = std::str::from_utf8(&bytes[..n]).unwrap();
                assert!(request.starts_with("GET "), "preflight must never mutate");
                let fault = serving.load(Ordering::Acquire);
                let payload = if request.starts_with("GET /version ") {
                    json!({"version":"synthetic"})
                } else if request.starts_with("GET /configs ") {
                    json!({"mode": if fault == 1 {"unknown"} else {mode}})
                } else if request.starts_with("GET /rules ") {
                    if fault == 2 {
                        json!({"rules":null})
                    } else {
                        json!({"rules":[]})
                    }
                } else if request.starts_with("GET /providers/rules ") {
                    if fault == 3 {
                        json!({"providers":null})
                    } else {
                        json!({"providers":{}})
                    }
                } else if request.starts_with("GET /proxies ") {
                    if fault == 6 {
                        let text = fs::read_to_string(&store).unwrap();
                        fs::write(&store, format!("{text}\n")).unwrap();
                    }
                    json!({"proxies": {
                        "Private": {"type":"Vless"},
                        "PROXY": {"type":"Selector", "now":if fault == 4 {"DIRECT"} else {"Private"}, "all":["Private"]},
                        "GLOBAL": {"type":"Selector", "now":if fault == 5 {"DIRECT"} else {"PROXY"}, "all":["PROXY"]}
                    }})
                } else {
                    panic!("unexpected preflight endpoint");
                };
                let body = payload.to_string();
                let response = format!(
                    "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (stage, worker)
    }

    #[test]
    fn service_query_is_fixed_bounded_and_typed() {
        let (root, _uid) = root("systemd");
        let systemctl = executable(
            &root,
            "#!/bin/sh\n[ \"$1 $2 $3\" = \"--user show omavless.service\" ] || exit 9\nprintf 'ActiveState=active\\nMainPID=42\\nExecMainStatus=0\\nResult=success\\n'\n",
        );
        // Use the production query budget here. Under a fully parallel
        // workspace test run the helper process can be descheduled long enough
        // for a shorter test-only deadline to expire before it executes.
        let state =
            service_state_with_timeout(&systemctl, LEGACY_SERVICE, SERVICE_QUERY_TIMEOUT).unwrap();
        assert!(state.active);
        assert_eq!(state.main_pid, 42);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn service_query_timeout_and_private_output_are_safe() {
        let (root, _uid) = root("timeout");
        let systemctl = executable(
            &root,
            "#!/bin/sh\nprintf 'private.example/password'\nexec sleep 2\n",
        );
        let started = Instant::now();
        let error =
            service_state_with_timeout(&systemctl, LEGACY_SERVICE, Duration::from_millis(50))
                .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(1));
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("private.example"));
        assert!(!rendered.contains("password"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_type_or_symlinked_socket_fails_closed() {
        let (root, uid) = root("socket");
        let private = root.join("private.example-password");
        fs::write(&private, "unchanged").unwrap();
        let socket = root.join("control.sock");
        symlink(&private, &socket).unwrap();
        let error = socket_presence(&socket, uid, true).unwrap_err();
        assert_eq!(error, ProductionObservationError::UnsafePath);
        assert!(!error.to_string().contains("private.example"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn controller_liveness_requires_success_and_a_nonempty_version() {
        // Actual private Unix responses exercise the observation adapter, not
        // just a newly invented projection. This matches OwnedCore's accepted
        // version predicate; config/selector readiness is explicitly separate.
        for (status, body, live) in [
            ("200 OK", r#"{"version":"synthetic"}"#, true),
            ("404 Not Found", r#"{"version":"synthetic"}"#, false),
            ("503 Unavailable", r#"{"version":"synthetic"}"#, false),
            ("204 No Content", "", false),
            ("200 OK", "{}", false),
            ("200 OK", "null", false),
            ("200 OK", "[]", false),
            ("200 OK", r#"{"version":""}"#, false),
            ("200 OK", r#"{"version":null}"#, false),
            ("200 OK", r#"{"version":1}"#, false),
            ("200 OK", r#"{"version":false}"#, false),
            ("200 OK", "malformed-private-error", false),
        ] {
            let root = crate::test_temp::directory("version-liveness").unwrap();
            let uid = fs::metadata(&root).unwrap().uid();
            let socket = root.join("mihomo.sock");
            let listener = UnixListener::bind(&socket).unwrap();
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
            let worker = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut request = [0; 512];
                let count = stream.read(&mut request).unwrap();
                assert!(request[..count].starts_with(b"GET /version HTTP/1.0\r\n"));
                let response = format!(
                    "HTTP/1.0 {status}\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            });
            let observed = controller_ready(&socket, uid).unwrap();
            assert_eq!(observed, live);
            let readiness = evaluate_cutover(
                &OwnershipMarker::default(),
                OwnershipObservation {
                    legacy_owner_active: true,
                    legacy_controller_ready: observed,
                    core_count: 1,
                    tun_count: 1,
                    active_profile_matches: true,
                    ..OwnershipObservation::disconnected()
                },
            );
            assert_eq!(readiness == CutoverReadiness::ReadyToAdopt, live);
            worker.join().unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn disconnected_host_is_ready_without_reading_private_profile_state() {
        let (root, uid) = root("disconnected");
        let systemctl = inactive_systemctl(&root);
        let paths = observer_paths(&root, uid, systemctl);
        let observation = ProductionOwnershipObserver::new(paths, uid)
            .unwrap()
            .observe()
            .unwrap();
        assert_eq!(observation, OwnershipObservation::disconnected());
        assert_eq!(
            evaluate_cutover(&OwnershipMarker::default(), observation),
            CutoverReadiness::ReadyDisconnected
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transitioning_legacy_service_cannot_look_disconnected() {
        let (root, uid) = root("transitioning");
        let systemctl = executable(
            &root,
            "#!/bin/sh\ncase \"$3\" in\n  omavless.service) printf 'ActiveState=activating\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' ;;\n  omavless-runtime.service) printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' ;;\n  *) exit 9 ;;\nesac\n",
        );
        let paths = observer_paths(&root, uid, systemctl);
        let observation = ProductionOwnershipObserver::new(paths, uid)
            .unwrap()
            .observe()
            .unwrap();
        assert!(observation.legacy_owner_active);
        assert_eq!(
            evaluate_cutover(&OwnershipMarker::default(), observation),
            CutoverReadiness::Blocked(crate::cutover::CutoverBlocker::InconsistentHostState)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_native_control_socket_blocks_legacy_cutover() {
        let (root, uid) = root("native-present");
        let systemctl = inactive_systemctl(&root);
        let paths = observer_paths(&root, uid, systemctl);
        let rust_directory = paths.rust_control_socket.parent().unwrap();
        fs::create_dir(rust_directory).unwrap();
        fs::set_permissions(rust_directory, fs::Permissions::from_mode(0o700)).unwrap();
        let listener = UnixListener::bind(&paths.rust_control_socket).unwrap();
        fs::set_permissions(
            &paths.rust_control_socket,
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let observation = ProductionOwnershipObserver::new(paths, uid)
            .unwrap()
            .observe()
            .unwrap();
        assert!(observation.rust_owner_active);
        assert_eq!(
            evaluate_cutover(&OwnershipMarker::default(), observation),
            CutoverReadiness::Blocked(crate::cutover::CutoverBlocker::RustLifecycleAlreadyActive)
        );
        drop(listener);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_exact_legacy_owner_and_private_config_are_ready_to_adopt() {
        let (root, uid) = root("legacy");
        let systemctl = executable(
            &root,
            "#!/bin/sh\ncase \"$3\" in\n  omavless.service) printf 'ActiveState=active\\nMainPID=42\\nExecMainStatus=0\\nResult=success\\n' ;;\n  omavless-runtime.service) printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' ;;\n  *) exit 9 ;;\nesac\n",
        );
        let paths = observer_paths(&root, uid, systemctl);
        let child_task = paths.proc_root.join("42/task/42");
        fs::create_dir_all(&child_task).unwrap();
        let core_pid = std::process::id().to_string();
        fs::write(child_task.join("children"), &core_pid).unwrap();
        fs::create_dir(paths.proc_root.join(&core_pid)).unwrap();
        fs::write(paths.proc_root.join(&core_pid).join("comm"), "mihomo\n").unwrap();
        fs::create_dir(paths.sys_class_net.join("Meta")).unwrap();
        fs::write(paths.sys_class_net.join("Meta/tun_flags"), "1\n").unwrap();

        let profile_id = "00000000-0000-0000-0000-000000000001";
        let private_uri = "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private";
        let store_text = format!(
            r#"{{"version":3,"activeId":"{profile_id}","lastId":"{profile_id}","profiles":[{{"id":"{profile_id}","name":"Private","uri":"{private_uri}","protocol":"vless"}}],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{{"enabled":false,"target":"last","profileId":"","mode":"rule"}},"onboardingComplete":true}}"#
        );
        let template = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
        let store = parse_private_store(&store_text).unwrap();
        let active = store
            .prepare_config_mode(
                profile_id,
                template,
                paths.legacy_controller.to_str().unwrap(),
                "global",
            )
            .unwrap();
        for (path, value) in [
            (&paths.store, store_text.as_str()),
            (&paths.template, template),
            (&paths.active_config, active.as_str()),
        ] {
            fs::write(path, value).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        let listener = UnixListener::bind(&paths.legacy_controller).unwrap();
        fs::set_permissions(&paths.legacy_controller, fs::Permissions::from_mode(0o600)).unwrap();
        let (stage, controller) = configured_controller(listener, "global", paths.store.clone());
        let observer = ProductionOwnershipObserver::new(paths.clone(), uid).unwrap();
        let observation = observer.observe().unwrap();
        for fault in 1..=6 {
            stage.store(fault, Ordering::Release);
            let rejected = observer.observe().unwrap();
            assert!(rejected.legacy_controller_ready);
            assert!(!rejected.active_profile_matches);
            assert_ne!(
                evaluate_cutover(&OwnershipMarker::default(), rejected),
                CutoverReadiness::ReadyToAdopt
            );
            fs::write(&paths.store, &store_text).unwrap();
        }
        stage.store(0, Ordering::Release);
        assert!(observer.observe().unwrap().active_profile_matches);
        stage.store(255, Ordering::Release);
        controller.join().unwrap();
        assert_eq!(observation.core_count, 1);
        assert_eq!(observation.tun_count, 1);
        assert!(observation.legacy_owner_active);
        assert!(observation.legacy_controller_ready);
        assert!(observation.active_profile_matches);
        assert_eq!(
            evaluate_cutover(&OwnershipMarker::default(), observation),
            CutoverReadiness::ReadyToAdopt
        );
        let rendered = serde_json::to_string(
            &ProductionCutoverPreflight {
                marker: OwnershipMarker::default(),
                observation,
                readiness: CutoverReadiness::ReadyToAdopt,
            }
            .public_json(),
        )
        .unwrap();
        for private in [profile_id, "11111111", "203.0.113.1", "Private"] {
            assert!(!rendered.contains(private));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_exact_rust_owner_matches_config_against_native_controller() {
        let (root, uid) = root("rust-owner");
        let systemctl = executable(
            &root,
            "#!/bin/sh\ncase \"$3\" in\n  omavless.service) printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' ;;\n  omavless-runtime.service) printf 'ActiveState=active\\nMainPID=52\\nExecMainStatus=0\\nResult=success\\n' ;;\n  *) exit 9 ;;\nesac\n",
        );
        let paths = observer_paths(&root, uid, systemctl);
        let child_task = paths.proc_root.join("52/task/52");
        fs::create_dir_all(&child_task).unwrap();
        let core_pid = std::process::id().to_string();
        fs::write(child_task.join("children"), &core_pid).unwrap();
        fs::create_dir(paths.proc_root.join(&core_pid)).unwrap();
        fs::write(paths.proc_root.join(&core_pid).join("comm"), "mihomo\n").unwrap();
        fs::create_dir(paths.sys_class_net.join("Meta")).unwrap();
        fs::write(paths.sys_class_net.join("Meta/tun_flags"), "1\n").unwrap();

        let profile_id = "00000000-0000-0000-0000-000000000001";
        let private_uri = "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private";
        let store_text = format!(
            r#"{{"version":3,"activeId":"{profile_id}","lastId":"{profile_id}","profiles":[{{"id":"{profile_id}","name":"Private","uri":"{private_uri}","protocol":"vless"}}],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{{"enabled":false,"target":"last","profileId":"","mode":"rule"}},"onboardingComplete":true}}"#
        );
        let template = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
        let store = parse_private_store(&store_text).unwrap();
        let active = store
            .prepare_config_mode(
                profile_id,
                template,
                paths.rust_controller.to_str().unwrap(),
                "rule",
            )
            .unwrap();
        for (path, value) in [
            (&paths.store, store_text.as_str()),
            (&paths.template, template),
            (&paths.active_config, active.as_str()),
        ] {
            fs::write(path, value).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        let rust_runtime = paths.rust_control_socket.parent().unwrap();
        fs::create_dir(rust_runtime).unwrap();
        fs::set_permissions(rust_runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let listener = UnixListener::bind(&paths.rust_controller).unwrap();
        fs::set_permissions(&paths.rust_controller, fs::Permissions::from_mode(0o600)).unwrap();
        let (stage, controller) = configured_controller(listener, "rule", paths.store.clone());
        let observation = ProductionOwnershipObserver::new(paths, uid)
            .unwrap()
            .observe()
            .unwrap();
        stage.store(255, Ordering::Release);
        controller.join().unwrap();
        assert!(observation.rust_owner_active);
        assert!(observation.rust_controller_ready);
        assert!(observation.active_profile_matches);
        assert_eq!(observation.core_count, 1);
        assert_eq!(observation.tun_count, 1);
        fs::remove_dir_all(root).unwrap();
    }
}
