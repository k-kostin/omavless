// SPDX-License-Identifier: MIT
//! Five historical current43 originals and one separately selected history8
//! completion successor. No daemon lifecycle, lock creation, retry or cleanup.
use super::*;
use crate::restore_decision_candidate::{DecisionChain, DecisionPhase};
use crate::restore_staging_candidate::{MEMBERS, PENDING_DIRECTORY, READY_BYTES, READY_MEMBER};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use zeroize::Zeroizing;

const INPUT_ROOT: &str = "/home/kdk_vm/.cache/t4-installed-current-recovery-review43";
const REQUEST: &str = "/home/kdk_vm/.cache/t4-installed-current-recovery-review43/request.json";
const ARCHIVE: &str =
    "/home/kdk_vm/.cache/t4-installed-current-recovery-review43/actual-current.ovb";
const CONFIG: &str = "/home/kdk_vm/.config/omavless";
const STATE: &str = "/home/kdk_vm/.local/state/omavless";
const RUN_BASE: &str = "/run/user/1000";
const RUN_DIR: &str = "/run/user/1000/omavless";
const CURRENT_OPT_IN: &str = "OMAVLESS_TEST_T4_CURRENT_RECOVERY_VM";
const FILES: usize = 16;
const DIRECTORIES: usize = 6;
// Necessary only, not a global free-FD/kernel allocation promise. The factual
// graph has 16 retained readers + six directories, one RPC socket, and one
// terminal readback; the separately reserved recovery engine has its own guard.
const READ_FD_CEILING: u64 = 28;
const RECOVERY_FD_CEILING: u64 =
    READ_FD_CEILING + crate::manager_actor_service::NATIVE_RETAINED_ROLE_CEILING as u64 + 4;
fn sufficient_limit(limit: u64, recovery: bool) -> bool {
    limit
        >= if recovery {
            RECOVERY_FD_CEILING
        } else {
            READ_FD_CEILING
        }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema: u8,
    archive: String,
    #[serde(deserialize_with = "private_text")]
    passphrase: Zeroizing<String>,
    expected_instance_id: String,
    expected_revision: u64,
}
fn private_text<'de, D: serde::Deserializer<'de>>(
    input: D,
) -> std::result::Result<Zeroizing<String>, D::Error> {
    String::deserialize(input).map(Zeroizing::new)
}
fn input(raw: &[u8]) -> std::result::Result<Input, ()> {
    if raw.len() > crate::developer_current_restore::MAX_INPUT {
        return Err(());
    }
    let value: Input = serde_json::from_slice(raw).map_err(|_| ())?;
    if value.schema != 1
        || value.archive != ARCHIVE
        || !(12..=1024).contains(&value.passphrase.len())
        || value.expected_instance_id.is_empty()
        || value.expected_instance_id.len() > 128
        || !value
            .expected_instance_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        || value.expected_revision > omavless_control_protocol::MAX_REVISION
    {
        return Err(());
    }
    Ok(value)
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && (a.mtime(), a.mtime_nsec()) == (b.mtime(), b.mtime_nsec())
        && (a.ctime(), a.ctime_nsec()) == (b.ctime(), b.ctime_nsec())
}
struct Held {
    path: PathBuf,
    file: File,
    metadata: Metadata,
    bytes: Zeroizing<Vec<u8>>,
}
impl Held {
    fn open(path: &Path, limit: usize, directory: bool) -> std::result::Result<Self, ()> {
        // Installed VM readers retain their fixed expected owner. A generic
        // synthetic fixture cannot change this entry or supply VM authority.
        Self::open_for_owner(path, limit, directory, (UID, UID))
    }
    fn open_for_owner(
        path: &Path,
        limit: usize,
        directory: bool,
        owner: (u32, u32),
    ) -> std::result::Result<Self, ()> {
        let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
        if (metadata.uid(), metadata.gid()) != owner
            || metadata.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
            || (directory && !metadata.is_dir())
            || (!directory && (!metadata.is_file() || metadata.nlink() != 1))
            || (!directory && metadata.len() > limit as u64)
            || fs::canonicalize(path).ok().as_deref() != Some(path)
        {
            return Err(());
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                nix::libc::O_NOFOLLOW
                    | nix::libc::O_NONBLOCK
                    | if directory { nix::libc::O_DIRECTORY } else { 0 },
            )
            .open(path)
            .map_err(|_| ())?;
        let mut held = Self {
            path: path.to_owned(),
            file,
            metadata,
            bytes: Zeroizing::new(Vec::new()),
        };
        held.check_metadata()?;
        if !directory {
            held.bytes = held.read(limit)?;
        }
        held.check_metadata()?;
        Ok(held)
    }
    fn check_metadata(&self) -> std::result::Result<(), ()> {
        let mut attributes = [0_u8; 1];
        if !same(&self.metadata, &self.file.metadata().map_err(|_| ())?)
            || !same(
                &self.metadata,
                &fs::symlink_metadata(&self.path).map_err(|_| ())?,
            )
            || rustix::fs::flistxattr(&self.file, &mut attributes).map_err(|_| ())? != 0
        {
            return Err(());
        }
        Ok(())
    }
    fn read(&mut self, limit: usize) -> std::result::Result<Zeroizing<Vec<u8>>, ()> {
        self.check_metadata()?;
        self.file.seek(SeekFrom::Start(0)).map_err(|_| ())?;
        let mut bytes = Zeroizing::new(Vec::new());
        (&mut self.file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ())?;
        self.check_metadata()?;
        if bytes.len() > limit || bytes.len() as u64 != self.metadata.len() {
            return Err(());
        }
        Ok(bytes)
    }
    fn unchanged(&mut self) -> std::result::Result<(), ()> {
        if self.metadata.is_dir() {
            return self.check_metadata();
        }
        let bytes = self.read(self.bytes.len())?;
        if bytes.as_slice() != self.bytes.as_slice() {
            return Err(());
        }
        Ok(())
    }
}

const ABSENT: [&str; 10] = [
    "restore-decision.terminal",
    "restore-finalization.pending",
    crate::restore_closure_model::CLOSURE_MEMBER,
    "restore-closure.next",
    crate::restore_disposition_ticket_model::TICKET_MEMBER,
    "restore-disposition.complete",
    "restore-successor.pending",
    "routing-preset.pending.json",
    // These are config-directory names (checked separately below).
    crate::restore_executor_candidate::OLD_SLOT[0],
    crate::restore_executor_candidate::OLD_SLOT[1],
];
struct Originals {
    files: [Option<Held>; FILES],
    directories: [Option<Held>; DIRECTORIES],
    history: Option<Held>,
    socket: Option<Metadata>,
}
impl Originals {
    fn capture(live: bool) -> std::result::Result<Self, ()> {
        Self::capture_phase(live, false)
    }
    fn capture_aborted() -> std::result::Result<Self, ()> {
        Self::capture_phase(false, true)
    }
    fn capture_phase(live: bool, aborted: bool) -> std::result::Result<Self, ()> {
        let (limit, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| ())?;
        if !sufficient_limit(limit, false) {
            return Err(());
        }
        let mut graph = Self {
            files: std::array::from_fn(|_| None),
            directories: std::array::from_fn(|_| None),
            history: None,
            socket: match fs::symlink_metadata(Path::new(RUN_DIR).join("control.sock")) {
                Ok(metadata) => Some(metadata),
                Err(error) if !live && error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => return Err(()),
            },
        };
        use std::os::unix::fs::FileTypeExt;
        if let Some(socket) = &graph.socket
            && (!socket.file_type().is_socket()
                || socket.uid() != UID
                || socket.gid() != UID
                || socket.mode() & 0o7777 != 0o600
                || socket.nlink() != 1)
        {
            return Err(());
        }
        let stage = Path::new(STATE).join(PENDING_DIRECTORY);
        for (slot, path) in graph.directories.iter_mut().zip([
            PathBuf::from(CONFIG),
            PathBuf::from(STATE),
            PathBuf::from(RUN_BASE),
            PathBuf::from(RUN_DIR),
            PathBuf::from(INPUT_ROOT),
            stage.clone(),
        ]) {
            *slot = Some(Held::open(&path, 0, true)?);
        }
        let store = omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES;
        let template = omavless_domain::config::MAX_TEMPLATE_BYTES;
        let record = crate::restore_decision_candidate::RECORD_BYTES;
        let roles = [
            (Path::new(CONFIG).join("profiles.json"), store),
            (Path::new(CONFIG).join("route-template.yaml"), template),
            (Path::new(STATE).join("desired.json"), 65536),
            (Path::new(STATE).join("ownership.json"), 4096),
            (Path::new(RUN_BASE).join("omavless-login.receipt"), 4096),
            (
                PathBuf::from(REQUEST),
                crate::developer_current_restore::MAX_INPUT,
            ),
            (
                PathBuf::from(ARCHIVE),
                omavless_domain::private_backup::MAX_BACKUP_BYTES,
            ),
            (Path::new(STATE).join("restore-decision.intent"), record),
            (stage.join(MEMBERS[0]), store),
            (stage.join(MEMBERS[1]), template),
            (stage.join(MEMBERS[2]), store),
            (stage.join(MEMBERS[3]), template),
            (stage.join(READY_MEMBER), READY_BYTES),
            (Path::new(RUN_BASE).join("omavless.1000.lock"), 4096),
            (Path::new(RUN_DIR).join("owner.lock"), 4096),
            // No sixteenth file is assumed. The reserved slot is the terminal
            // readback after the separately selected recovery effect.
        ];
        for (slot, (path, limit)) in graph.files.iter_mut().zip(roles) {
            *slot = Some(Held::open(&path, limit, false)?);
        }
        let history = Path::new(STATE).join("restore-disposition.history");
        if !absent(&history) {
            graph.history = Some(Held::open(&history, 4096, false)?);
        }
        if aborted {
            graph.files[15] = Some(Held::open(
                &Path::new(STATE).join("restore-decision.terminal"),
                record,
                false,
            )?);
        }
        graph.check_absence(aborted)?;
        if graph.file(0)?.bytes != graph.file(8)?.bytes
            || graph.file(1)?.bytes != graph.file(9)?.bytes
            || !original_phase(
                &graph.file(7)?.bytes,
                if aborted {
                    Some(&graph.file(15)?.bytes)
                } else {
                    None
                },
                aborted,
            )
        {
            return Err(());
        }
        Ok(graph)
    }
    fn file(&self, index: usize) -> std::result::Result<&Held, ()> {
        self.files.get(index).and_then(Option::as_ref).ok_or(())
    }
    fn check_absence(&self, terminal: bool) -> std::result::Result<(), ()> {
        for name in ABSENT {
            let parent = if crate::restore_executor_candidate::OLD_SLOT.contains(&name) {
                CONFIG
            } else {
                STATE
            };
            if !(terminal && name == "restore-decision.terminal")
                && !absent(&Path::new(parent).join(name))
            {
                return Err(());
            }
        }
        for name in crate::restore_executor_candidate::NEW_SLOT {
            if !absent(&Path::new(CONFIG).join(name)) {
                return Err(());
            }
        }
        if self.history.is_none() && !absent(&Path::new(STATE).join("restore-disposition.history"))
        {
            return Err(());
        }
        Ok(())
    }
    fn unchanged(&mut self, terminal: bool) -> std::result::Result<(), ()> {
        let named = fs::symlink_metadata(Path::new(RUN_DIR).join("control.sock"));
        if !match (&self.socket, named) {
            (Some(original), Ok(current)) => same(original, &current),
            (None, Err(error)) => error.kind() == std::io::ErrorKind::NotFound,
            _ => false,
        } {
            return Err(());
        }
        for file in self.files.iter_mut().flatten() {
            file.unchanged()?;
        }
        if let Some(history) = &mut self.history {
            history.unchanged()?;
        }
        for (index, dir) in self.directories.iter_mut().enumerate() {
            // The production engine proves its own reported Terminal creation
            // and changed state catalogue. Do not claim state nine-tuple stays
            // identical after that known write, or waive production origin gates.
            if terminal && index == 1 {
                let held = dir.as_mut().ok_or(())?;
                let current = held.file.metadata().map_err(|_| ())?;
                let mut attrs = [0_u8; 1];
                if held.metadata.dev() != current.dev()
                    || held.metadata.ino() != current.ino()
                    || held.metadata.mode() != current.mode()
                    || held.metadata.uid() != current.uid()
                    || held.metadata.gid() != current.gid()
                    || held.metadata.nlink() != current.nlink()
                    || !same(&current, &fs::symlink_metadata(&held.path).map_err(|_| ())?)
                    || rustix::fs::flistxattr(&held.file, &mut attrs).map_err(|_| ())? != 0
                {
                    return Err(());
                }
            } else {
                dir.as_mut().ok_or(())?.unchanged()?;
            }
        }
        self.check_absence(terminal)
    }
}

fn original_phase(intent: &[u8], terminal: Option<&[u8]>, aborted: bool) -> bool {
    DecisionChain::decode(intent, terminal).is_ok_and(|chain| {
        chain.active().phase()
            == if aborted {
                DecisionPhase::Aborted
            } else {
                DecisionPhase::Intent
            }
    })
}

fn normal_paths() -> std::result::Result<(RuntimePaths, CutoverPaths, DesiredPaths), ()> {
    if !identity(
        Uid::current().as_raw(),
        nix::unistd::geteuid().as_raw(),
        std::env::var_os("HOME").as_deref(),
        std::env::var(CURRENT_OPT_IN).ok().as_deref(),
    ) || std::env::var_os("XDG_RUNTIME_DIR").as_deref() != Some(std::ffi::OsStr::new(RUN_BASE))
        || [
            "OMAVLESS_HOME",
            "OMAVLESS_MIHOMO",
            "XDG_CONFIG_HOME",
            "XDG_STATE_HOME",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
        ]
        .into_iter()
        .any(|key| std::env::var_os(key).is_some())
    {
        return Err(());
    }
    let runtime = RuntimePaths::current().map_err(|_| ())?;
    let cutover = CutoverPaths::current(UID).map_err(|_| ())?;
    let desired = DesiredPaths::current().map_err(|_| ())?;
    if runtime.directory != Path::new(RUN_DIR)
        || cutover.state_directory != Path::new(STATE)
        || cutover.runtime_base != Path::new(RUN_BASE)
        || desired.directory != Path::new(STATE)
    {
        return Err(());
    }
    Ok((runtime, cutover, desired))
}
#[derive(Clone, Copy)]
enum Refusal {
    Pause,
    Abort,
    Backup,
    Restore,
}
impl Refusal {
    fn method(self) -> &'static str {
        use crate::developer_current_restore as d;
        match self {
            Self::Pause => d::PAUSE_METHOD,
            Self::Abort => d::ABORT_METHOD,
            Self::Backup => d::BACKUP_METHOD,
            Self::Restore => d::METHOD,
        }
    }
    fn params(self, input: &Input, instance: &str, revision: u64) -> Value {
        let confirmation = match self {
            Self::Pause => "pause-current-private-intent",
            Self::Abort => "abort-current-private-intent",
            Self::Backup => "export-current-private-pair",
            Self::Restore => "replace-current-private-pair",
        };
        if matches!(self, Self::Abort) {
            json!({"schema":1,"confirmation":confirmation,"instanceId":instance,"expectedRevision":revision})
        } else {
            json!({"schema":1,"confirmation":confirmation,"instanceId":instance,"expectedRevision":revision,
                "archive":input.archive,"passphrase":input.passphrase.as_str()})
        }
    }
}
fn denied(value: &Value, revision: u64) -> bool {
    value["ok"] == false
        && value["revision"] == revision
        && value["error"]
            == json!({"code":"capability_unavailable",
            "message":"The requested capability is unavailable","retryable":false})
        && value.get("result").is_none()
}
fn off(value: &Value, revision: u64) -> bool {
    value["ok"] == true
        && value["revision"] == revision
        && value["result"]["runtimeOwnership"] == false
        && value["result"]["desired"] == "disconnected"
        && value["result"]["actual"] == "disconnected"
        && value["result"]["activeProfileId"] == ""
}
fn hello(value: &Value) -> std::result::Result<(&str, u64), ()> {
    let instance = value["result"]["instanceId"].as_str().ok_or(())?;
    let revision = value["revision"].as_u64().ok_or(())?;
    if value["ok"] != true
        || value["result"]["runtimeOwnership"] != false
        || instance.is_empty()
        || instance.len() > 128
        || !instance
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        || revision > omavless_control_protocol::MAX_REVISION
    {
        return Err(());
    }
    Ok((instance, revision))
}
fn call(
    paths: &RuntimePaths,
    method: &'static str,
    params: Value,
    until: Instant,
) -> std::result::Result<Value, ()> {
    let remaining = until.checked_duration_since(Instant::now()).ok_or(())?;
    let value = crate::call_with_timeout(
        paths,
        method,
        params,
        remaining.min(Duration::from_secs(10)),
    )
    .map_err(|_| ())?;
    if Instant::now() >= until {
        return Err(());
    }
    Ok(value)
}
fn refusal(case: Refusal) -> std::result::Result<(), ()> {
    let until = Instant::now() + Duration::from_secs(45);
    let (runtime, _, _) = normal_paths()?;
    let mut graph = Originals::capture(true)?; // before ANY RPC, private original request pinned
    let input = input(&graph.file(5)?.bytes)?;
    let h = call(&runtime, "system.hello", json!({"versions":[1]}), until)?;
    let (instance, revision) = hello(&h)?;
    if instance != input.expected_instance_id || revision != input.expected_revision {
        return Err(()); // correlation only: not permission from copied instance/revision
    }
    if !off(&call(&runtime, "status.get", json!({}), until)?, revision) {
        return Err(());
    }
    let profiles = call(&runtime, "profiles.list", json!({}), until)?;
    // A fenced owner may refuse list; compare the exact normal decoded result,
    // not assume an unavailable projection returns private profiles.
    graph.unchanged(false)?;
    let params = case.params(&input, instance, revision);
    if matches!(case, Refusal::Abort) {
        crate::developer_current_restore::AbortRequest::parse(&params).map_err(|_| ())?;
    } else {
        let parsed = crate::developer_current_restore::Request::parse(&params).map_err(|_| ())?;
        if !parsed.matches_method(case.method()) {
            return Err(());
        }
    }
    // ONE fixed original operation. Transport/malformed/non-capability outcome
    // returns immediately: no second operation or post-read grants continuation.
    if !denied(&call(&runtime, case.method(), params, until)?, revision) {
        return Err(());
    }
    graph.unchanged(false)?;
    let after = call(&runtime, "system.hello", json!({"versions":[1]}), until)?;
    if hello(&after)? != (instance, revision)
        || !off(&call(&runtime, "status.get", json!({}), until)?, revision)
        || call(&runtime, "profiles.list", json!({}), until)? != profiles
    {
        return Err(());
    }
    graph.unchanged(false)
}

#[test]
fn current43_fixed_input_and_exact_denial_are_not_generic_grants() {
    assert!(sufficient_limit(READ_FD_CEILING, false));
    assert!(!sufficient_limit(READ_FD_CEILING - 1, false));
    assert!(sufficient_limit(RECOVERY_FD_CEILING, true));
    assert!(!sufficient_limit(RECOVERY_FD_CEILING - 1, true));
    let raw = format!(
        r#"{{"schema":1,"archive":"{ARCHIVE}","passphrase":"public synthetic password","expectedInstanceId":"same-instance","expectedRevision":7}}"#
    );
    assert!(input(raw.as_bytes()).is_ok());
    assert!(input(&vec![b' '; crate::developer_current_restore::MAX_INPUT + 1]).is_err());
    assert!(input(&[0xff]).is_err());
    for bad in [
        raw.replace(ARCHIVE, "/arbitrary/archive"),
        raw.replace("1,", "2,"),
        raw.replace("public synthetic password", "short"),
        raw.replace("same-instance", ""),
        raw.replace("same-instance", "foreign/path"),
        raw.replace("\"expectedRevision\":7", "\"expectedRevision\":-1"),
        raw.replace("\"schema\":1", "\"schema\":1,\"schema\":1"),
        raw.replace("1,", "1,\"extra\":true,"),
        "not JSON".into(),
    ] {
        assert!(input(bad.as_bytes()).is_err());
    }
    let reply = omavless_control_protocol::error_response(
        "test",
        7,
        omavless_control_protocol::StableErrorCode::CapabilityUnavailable,
        false,
        None,
    )
    .unwrap();
    assert!(denied(&reply, 7));
    for bad in [
        json!({"ok":false,"revision":7,"error":{"code":"manual_recovery_required"}}),
        json!({"ok":true,"revision":7}),
        json!({"ok":false,"revision":7}),
    ] {
        assert!(!denied(&bad, 7));
    }
    assert!(!denied(&reply, 8));
    let input = input(raw.as_bytes()).unwrap();
    for case in [
        Refusal::Pause,
        Refusal::Abort,
        Refusal::Backup,
        Refusal::Restore,
    ] {
        let params = case.params(&input, "same-instance", 7);
        if matches!(case, Refusal::Abort) {
            assert!(crate::developer_current_restore::AbortRequest::parse(&params).is_ok());
        } else {
            assert!(
                crate::developer_current_restore::Request::parse(&params)
                    .unwrap()
                    .matches_method(case.method())
            );
        }
    }
}

#[test]
fn current43_original_readers_detect_byte_name_and_directory_drift() {
    use std::os::unix::fs::PermissionsExt;
    // Hosted runners need not have the development VM's UID/GID. Keep the
    // generic reader fixture tied to its own creator, not a copied VM identity.
    let owner = (
        Uid::current().as_raw(),
        nix::unistd::Gid::current().as_raw(),
    );
    let root = crate::test_temp::directory_under(
        Path::new(&std::env::var_os("HOME").unwrap()),
        "current43-reader",
    )
    .unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("private");
    fs::write(&path, b"public").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Held::open_for_owner(&path, 6, false, (owner.0 ^ 1, owner.1)).is_err());
    assert!(Held::open_for_owner(&path, 6, false, (owner.0, owner.1 ^ 1)).is_err());
    if owner != (UID, UID) {
        assert!(Held::open(&path, 6, false).is_err()); // VM entry stays fixed.
    }
    assert!(Held::open_for_owner(&path, 5, false, owner).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(Held::open_for_owner(&path, 6, false, owner).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let alias = root.join("alias");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(Held::open_for_owner(&alias, 6, false, owner).is_err());
    fs::remove_file(alias).unwrap();
    let mut file = Held::open_for_owner(&path, 6, false, owner).unwrap();
    let mut dir = Held::open_for_owner(&root, 0, true, owner).unwrap();
    file.unchanged().unwrap();
    dir.unchanged().unwrap();
    fs::rename(&path, root.join("held")).unwrap();
    fs::write(&path, b"public").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(file.unchanged().is_err() && dir.unchanged().is_err());
    assert_eq!(file.bytes.as_slice(), b"public");
    drop((file, dir));
    fs::remove_file(path).unwrap();
    fs::remove_file(root.join("held")).unwrap();
    fs::remove_dir(root).unwrap();
}

macro_rules! fixed_refusal {
    ($name:ident,$case:ident,$marker:literal) => {
        #[test]
        #[ignore = "ROOT-only SAME live sealed current43; independent original seal/epoch prerequisite"]
        fn $name() {
            assert!(refusal(Refusal::$case).is_ok(),"fixed_current43_refusal_unknown");
            println!($marker);
        }
    }
}
fixed_refusal!(
    installed_current43_pause_refused,
    Pause,
    "T4_CURRENT43_PAUSE_PREFLIGHT_REFUSED"
);
fixed_refusal!(
    installed_current43_abort_refused,
    Abort,
    "T4_CURRENT43_ABORT_PREFLIGHT_REFUSED"
);
fixed_refusal!(
    installed_current43_backup_refused,
    Backup,
    "T4_CURRENT43_BACKUP_PREFLIGHT_REFUSED"
);
fixed_refusal!(
    installed_current43_restore_refused,
    Restore,
    "T4_CURRENT43_RESTORE_PREFLIGHT_REFUSED"
);

#[test]
#[ignore = "ROOT-only NEW auth after known normal stop0; sameboot existing current43 locks, OLD/Intent"]
fn installed_current43_new_authenticated_old_intent_aborted() {
    let until = Instant::now() + Duration::from_secs(90);
    let (runtime, paths, desired) = normal_paths().expect("fixed_current43_identity_refused");
    let (limit, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
        .expect("fixed_current43_capacity_refused");
    assert!(
        sufficient_limit(limit, true),
        "fixed_current43_capacity_refused"
    );
    // Reserve the actual holder before even the factual reader acquisitions.
    let mut recovery = crate::native_coordinator::FreshRecovery::reserve()
        .expect("fixed_current43_recovery_reservation_refused");
    let mut graph = Originals::capture(false).expect("fixed_current43_originals_refused");
    let input = input(&graph.file(5).unwrap().bytes).expect("fixed_current43_input_refused");
    let result = NativeHostPaths::current(&runtime.directory)
        .map_err(|_| ())
        .and_then(|host| {
            recovery
                .reconcile_old_intent(
                    Path::new(ARCHIVE),
                    input.passphrase.as_bytes(),
                    paths.clone(),
                    desired,
                    host,
                    UID,
                )
                .map_err(|_| ())
        });
    let complete = (|| -> std::result::Result<(), ()> {
        result?;
        graph.files[15] = Some(Held::open(
            &Path::new(STATE).join("restore-decision.terminal"),
            crate::restore_decision_candidate::RECORD_BYTES,
            false,
        )?);
        let chain = DecisionChain::decode(&graph.file(7)?.bytes, Some(&graph.file(15)?.bytes))
            .map_err(|_| ())?;
        let busy = match MigrationLock::acquire_existing(&paths, UID) {
            Err(_) => true,
            Ok(unexpected) => {
                std::mem::forget(unexpected); // positively reported surprise retained on refusal
                false
            }
        };
        if chain.active().phase() != DecisionPhase::Aborted
            || !recovery.original_leases_held(&paths, UID)
            || !busy
            || !crate::pending_private_transaction::pending_at(&paths.state_directory)
            || Instant::now() >= until
        {
            return Err(());
        }
        graph.unchanged(true)?;
        Ok(())
    })()
    .is_ok();
    std::mem::forget(recovery); // before output/assert; SAME uncertain graph never released by this handle
    assert!(complete, "fixed_current43_recovery_unknown");
    println!("T4_CURRENT43_NEW_AUTHENTICATED_OLD_INTENT_ABORTED_STILL_FENCED");
    assert!(Instant::now() < until, "fixed_current43_deadline_refused");
}

#[test]
fn current43_completion_terminal_is_data_not_a_completion_grant() {
    let plan = crate::restore_staging_candidate::planned_stage_identity([
        b"old store".as_slice(),
        b"old template",
        b"new store",
        b"new template",
    ])
    .unwrap();
    let intent =
        crate::restore_decision_candidate::DecisionRecord::intent(2, None, &plan, [5; 16]).unwrap();
    let old = intent
        .terminal(crate::restore_decision_candidate::TerminalChoice::Abort)
        .unwrap()
        .encode();
    let new = intent
        .terminal(crate::restore_decision_candidate::TerminalChoice::Commit)
        .unwrap()
        .encode();
    assert!(original_phase(&intent.encode(), None, false));
    assert!(original_phase(&intent.encode(), Some(&old), true));
    assert!(!original_phase(&intent.encode(), Some(&old), false));
    assert!(!original_phase(&intent.encode(), None, true));
    assert!(!original_phase(&intent.encode(), Some(&new), true));
    let mut foreign = old;
    foreign[0] ^= 1;
    assert!(!original_phase(&intent.encode(), Some(&foreign), true));
}

#[test]
#[ignore = "ROOT-only after original current43 OLD/Aborted0; SAMEBOOT existing locks; history8 successor"]
fn installed_current43_aborted_history8_completion() {
    let until = Instant::now() + Duration::from_secs(90);
    let (runtime, paths, desired) = normal_paths().expect("fixed_current43_identity_refused");
    let (limit, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
        .expect("fixed_current43_capacity_refused");
    assert!(
        sufficient_limit(limit, true),
        "fixed_current43_capacity_refused"
    );
    // Both production holders and their entire eight-audit capacity exist
    // BEFORE factual file acquisition, authentication or new original leases.
    let mut recovery = crate::native_coordinator::FreshRecovery::reserve()
        .expect("fixed_current43_recovery_reservation_refused");
    let mut graph = Originals::capture_aborted().expect("fixed_current43_originals_refused");
    let mut successor = None;
    let complete = (|| -> std::result::Result<(), ()> {
        if graph.history.is_none()
            || !(1..8)
                .all(|n| absent(&Path::new(STATE).join(format!("restore-disposition.history.{n}"))))
        {
            return Err(());
        }
        let input = input(&graph.file(5)?.bytes)?;
        let original_store = omavless_domain::private_store::parse_private_store(
            std::str::from_utf8(&graph.file(0)?.bytes).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        let expected_list = crate::profile_list_json(&original_store.list_projection());
        graph.unchanged(true)?;
        recovery
            .complete_aborted(
                Path::new(ARCHIVE),
                input.passphrase.as_bytes(),
                paths.clone(),
                desired,
                NativeHostPaths::current(&runtime.directory).map_err(|_| ())?,
                UID,
            )
            .map_err(|_| ())?;
        recovery.dispose_and_transfer_completed().map_err(|_| ())?;
        recovery.activate_ordinary_owner().map_err(|_| ())?;
        let owner = recovery.ordinary_owner().ok_or(())?;
        let current_desired = owner.desired_for_status().map_err(|_| ())?;
        if owner.actual() != crate::lifecycle::ActualState::Disconnected
            || !owner.rust_ownership_available()
            || current_desired.connected || !current_desired.profile_id.is_empty()
            || owner.login_ready() // This loan is NOT a genuine current()/login issuer.
            || crate::profile_list_json(&owner.list_projection().map_err(|_| ())?) != expected_list
            || crate::pending_private_transaction::pending_at(&paths.state_directory)
        {
            return Err(());
        }
        // Untouched current pair, Desired, marker, login, archive, input and
        // original lease names retain full nine fields/EOF bytes. Production
        // owns all positive retirement role transitions; do not recapture its
        // unlinked Stage/Intent/Terminal or claim their original nlink remains1.
        for index in [0, 1, 2, 3, 4, 5, 6, 13, 14] {
            graph.files[index].as_mut().ok_or(())?.unchanged()?;
        }
        graph.history.as_mut().ok_or(())?.unchanged()?;
        for index in [0, 2, 3, 4] {
            graph.directories[index].as_mut().ok_or(())?.unchanged()?;
        }
        if !absent(&Path::new(RUN_DIR).join("control.sock"))
            || !absent(&Path::new(STATE).join(PENDING_DIRECTORY))
            || !absent(&Path::new(STATE).join("restore-decision.intent"))
            || !absent(&Path::new(STATE).join("restore-decision.terminal"))
            || Instant::now() >= until
        {
            return Err(());
        }
        // Data readback only; positive completion came from SAME engine,
        // not this decoded member. Slot0 history remains its original inode.
        successor = Some(Held::open(
            &Path::new(STATE).join("restore-disposition.history.1"),
            crate::restore_disposition_complete_model::COMPLETE_BYTES,
            false,
        )?);
        if crate::restore_disposition_complete_model::CompleteRecord::decode(
            &successor.as_ref().ok_or(())?.bytes,
        )
        .is_none()
        {
            return Err(());
        }
        let busy = match MigrationLock::acquire_existing(&paths, UID) {
            Err(_) => true,
            Ok(unexpected) => {
                std::mem::forget(unexpected);
                false
            }
        };
        if !busy {
            return Err(());
        }
        Ok(())
    })()
    .is_ok();
    std::mem::forget((recovery, graph, successor)); // before output/assert, including every uncertain prefix
    assert!(complete, "fixed_current43_history_completion_unknown");
    println!("T4_CURRENT43_ABORTED_HISTORY8_COMPLETED_RETAINED_ORDINARY_OFF");
    assert!(Instant::now() < until, "fixed_current43_deadline_refused");
    // ROOT independently starts the normal installed RuntimeServer only after
    // this original exits0. No listener/login/current grant is minted here.
}
