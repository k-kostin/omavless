//! Fixed developer-only bus/file adapter; no production IPC or caller paths.
use super::*;
use crate::manager_response_diagnostic_fixture as admission;
use crate::manager_response_diagnostic_permissions as permissions;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Type};

const STAGE: &str = "/run/omavless-k1-retained-private-lifecycle";
const UNIT: &str = "omavless-k1-retained-private-lifecycle.service";
const LINK: &str = "/run/systemd/system/omavless-k1-retained-private-lifecycle.service";
const UNIT_PATH: &str =
    "/org/freedesktop/systemd1/unit/omavless_2dk1_2dretained_2dprivate_2dlifecycle_2eservice";
const CGROUP: &str = "/sys/fs/cgroup/system.slice/omavless-k1-retained-private-lifecycle.service";
const WRITER: &str =
    "kernel_observer::creator_lifecycle::response_diagnostic::manager_private_lifecycle";
const TEST: &str = "manager_retained_lifecycle::adapter::run_private_lifecycle";
const UNIT_BYTES: &[u8] =
    include_bytes!("../tests/fixtures/omavless-k1-retained-private-lifecycle.service");
type Facts = HashMap<String, OwnedValue>;
type Exec = (String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32);

fn ensure(value: bool) -> Result<()> {
    if value { Ok(()) } else { Err(()) }
}

fn meta(m: &Metadata) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.nlink(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}

fn directory_meta(m: &Metadata) -> (u64, u64, u32, u32, u32) {
    (m.dev(), m.ino(), m.mode(), m.uid(), m.gid())
}

struct Pin {
    file: File,
    path: PathBuf,
    initial: Metadata,
    hash: [u8; 32],
}

impl Pin {
    fn open(path: PathBuf, mode: u32, bound: u64) -> Result<Self> {
        Self::open_owned(path, mode, bound, (0, 0))
    }
    fn open_owned(path: PathBuf, mode: u32, bound: u64, owner: (u32, u32)) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC | nix::libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| ())?;
        let initial = file.metadata().map_err(|_| ())?;
        ensure(
            initial.is_file()
                && (initial.uid(), initial.gid()) == owner
                && initial.mode() & 0o7777 == mode
                && initial.nlink() == 1
                && initial.len() > 0
                && initial.len() <= bound,
        )?;
        let mut pin = Self {
            file,
            path,
            initial,
            hash: [0; 32],
        };
        pin.hash = pin.digest()?;
        pin.recheck(true)?;
        Ok(pin)
    }
    fn digest(&self) -> Result<[u8; 32]> {
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 65536];
        let mut offset = 0;
        while offset < self.initial.len() {
            let length = self
                .file
                .read_at(
                    &mut buffer[..(self.initial.len() - offset).min(65536) as usize],
                    offset,
                )
                .map_err(|_| ())?;
            ensure(length > 0)?;
            hash.update(&buffer[..length]);
            offset += length as u64;
        }
        ensure(
            self.file
                .read_at(&mut buffer[..1], offset)
                .map_err(|_| ())?
                == 0,
        )?;
        Ok(hash.finalize().into())
    }
    fn recheck(&self, contents: bool) -> Result<()> {
        ensure(
            meta(&self.initial) == meta(&self.file.metadata().map_err(|_| ())?)
                && meta(&self.initial)
                    == meta(&std::fs::symlink_metadata(&self.path).map_err(|_| ())?),
        )?;
        if contents {
            ensure(self.digest()? == self.hash)?;
        }
        ensure(
            meta(&self.initial) == meta(&self.file.metadata().map_err(|_| ())?)
                && meta(&self.initial)
                    == meta(&std::fs::symlink_metadata(&self.path).map_err(|_| ())?),
        )
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        ensure(self.initial.len() <= 16384)?;
        self.recheck(true)?;
        let mut bytes = vec![0; self.initial.len() as usize];
        self.file.read_exact_at(&mut bytes, 0).map_err(|_| ())?;
        self.recheck(true)?;
        Ok(bytes)
    }
}

struct Directory {
    file: File,
    path: PathBuf,
    initial: Metadata,
}
impl Directory {
    fn open(path: &Path, private: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| ())?;
        let initial = file.metadata().map_err(|_| ())?;
        ensure(
            initial.is_dir()
                && initial.uid() == 0
                && initial.gid() == 0
                && initial.mode() & 0o022 == 0
                && (!private || initial.mode() & 0o7777 == 0o700),
        )?;
        let held = Self {
            file,
            path: path.to_owned(),
            initial,
        };
        held.recheck()?;
        Ok(held)
    }
    fn recheck(&self) -> Result<()> {
        ensure(
            directory_meta(&self.initial) == directory_meta(&self.file.metadata().map_err(|_| ())?)
                && directory_meta(&self.initial)
                    == directory_meta(&std::fs::symlink_metadata(&self.path).map_err(|_| ())?),
        )
    }
}

fn string(f: &Facts, k: &str) -> Result<String> {
    <&str>::try_from(f.get(k).ok_or(())?)
        .map(str::to_owned)
        .map_err(|_| ())
}
fn u64_field(f: &Facts, k: &str) -> Result<u64> {
    u64::try_from(f.get(k).ok_or(())?).map_err(|_| ())
}
fn u32_field(f: &Facts, k: &str) -> Result<u32> {
    u32::try_from(f.get(k).ok_or(())?).map_err(|_| ())
}
fn i32_field(f: &Facts, k: &str) -> Result<i32> {
    i32::try_from(f.get(k).ok_or(())?).map_err(|_| ())
}
fn job_field(f: &Facts) -> Result<(u32, String)> {
    let value = f.get("Job").ok_or(())?;
    ensure(value.value_signature() == <(u32, OwnedObjectPath)>::SIGNATURE)?;
    let (id, path) =
        <(u32, OwnedObjectPath)>::try_from(value.try_clone().map_err(|_| ())?).map_err(|_| ())?;
    Ok((id, path.to_string()))
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct Execution {
    invocation: Vec<u8>,
    pid: u32,
    start: u64,
    exit: u64,
    command_start: u64,
    command_exit: u64,
}

fn execution(f: &Facts, finished: bool) -> Result<Execution> {
    let value = f.get("ExecStart").ok_or(())?;
    ensure(value.value_signature() == Vec::<Exec>::SIGNATURE)?;
    let values = Vec::<Exec>::try_from(value.try_clone().map_err(|_| ())?).map_err(|_| ())?;
    ensure(values.len() == 1)?;
    let e = &values[0];
    let probe = format!("{STAGE}/probe");
    ensure(
        e.0 == probe
            && e.1
                == [
                    probe,
                    "--exact".into(),
                    WRITER.into(),
                    "--ignored".into(),
                    "--nocapture".into(),
                    "--test-threads=1".into(),
                ]
            && !e.2,
    )?;
    let value = f.get("InvocationID").ok_or(())?;
    ensure(value.value_signature() == Vec::<u8>::SIGNATURE)?;
    let invocation = Vec::<u8>::try_from(value.try_clone().map_err(|_| ())?).map_err(|_| ())?;
    ensure(invocation.len() == 16 && invocation.iter().any(|v| *v != 0))?;
    let pid = u32_field(f, "ExecMainPID")?;
    let start = u64_field(f, "ExecMainStartTimestampMonotonic")?;
    let exit = u64_field(f, "ExecMainExitTimestampMonotonic")?;
    ensure(
        pid > 1
            && pid == e.7
            && start > 0
            && start < u64::MAX
            && e.3 > 0
            && e.3 < u64::MAX
            && e.4 > 0
            && e.4 < u64::MAX,
    )?;
    if finished {
        ensure(
            exit >= start
                && exit < u64::MAX
                && e.5 < u64::MAX
                && e.6 < u64::MAX
                && e.5 >= e.3
                && e.6 >= e.4
                && i32_field(f, "ExecMainCode")? == 1
                && i32_field(f, "ExecMainStatus")? == 0
                && e.8 == 1
                && e.9 == 0
                && u32_field(f, "MainPID")? == 0,
        )?;
    } else {
        ensure(
            exit == 0
                && e.5 == 0
                && e.6 == 0
                && e.8 == 0
                && e.9 == 0
                && i32_field(f, "ExecMainCode")? == 0
                && i32_field(f, "ExecMainStatus")? == 0
                && u32_field(f, "MainPID")? == pid,
        )?;
    }
    ensure(u32_field(f, "ControlPID")? == 0 && u64_field(f, "WatchdogUSec")? == 0)?;
    Ok(Execution {
        invocation,
        pid,
        start,
        exit,
        command_start: e.4,
        command_exit: e.6,
    })
}

fn common_phase(f: &Facts, job: &Job) -> Result<bool> {
    permissions::check_lifecycle_stable(f)?;
    ensure(string(f, "Result")? == "success" && u32_field(f, "NRestarts")? == 0)?;
    let cg = string(f, "ControlGroup")?;
    ensure(cg.is_empty() || cg == format!("/system.slice/{UNIT}"))?;
    let current = job_field(f)?;
    if current == (0, "/".into()) {
        Ok(false)
    } else {
        ensure(current == (job.id, job.path.clone()))?;
        Ok(true)
    }
}

fn never_started(f: &Facts) -> Result<()> {
    permissions::check(f, f)?;
    ensure(
        string(f, "Result")? == "success"
            && u32_field(f, "NRestarts")? == 0
            && u64_field(f, "ExecMainExitTimestampMonotonic")? == 0
            && i32_field(f, "ExecMainCode")? == 0
            && i32_field(f, "ExecMainStatus")? == 0,
    )?;
    let value = f.get("InvocationID").ok_or(())?;
    ensure(value.value_signature() == Vec::<u8>::SIGNATURE)?;
    let invocation = Vec::<u8>::try_from(value.try_clone().map_err(|_| ())?).map_err(|_| ())?;
    // v261 bus_property_get_id128 emits an empty typed ay for a null ID,
    // not an array of sixteen zeros. Missing or alternative encodings refuse.
    ensure(invocation.is_empty())
}

fn start_phase(f: &Facts, job: &Job) -> Result<Option<Execution>> {
    let pending_job = common_phase(f, job)?;
    match (
        string(f, "ActiveState")?.as_str(),
        string(f, "SubState")?.as_str(),
    ) {
        ("inactive", "dead") => {
            ensure(pending_job)?;
            never_started(f)?;
            Ok(None)
        }
        ("activating", "start") => {
            ensure(pending_job)?;
            execution(f, false)?;
            Ok(None)
        }
        ("active", "exited") => {
            let complete = execution(f, true)?;
            if pending_job {
                Ok(None)
            } else {
                Ok(Some(complete))
            }
        }
        _ => Err(()),
    }
}

fn stop_phase(f: &Facts, job: &Job, prior: &Execution) -> Result<bool> {
    let pending_job = common_phase(f, job)?;
    ensure(execution(f, true)? == *prior)?;
    match (
        string(f, "ActiveState")?.as_str(),
        string(f, "SubState")?.as_str(),
    ) {
        ("inactive", "dead") => Ok(!pending_job),
        ("active", "exited")
        | ("deactivating", "stop")
        | ("deactivating", "stop-sigterm")
        | ("deactivating", "stop-post")
        | ("deactivating", "final-sigterm") => {
            ensure(pending_job)?;
            Ok(false)
        }
        _ => Err(()),
    }
}

struct Real {
    witness: Option<crate::manager_negative_witness::RootWitness>,
    admitted: admission::Held,
    dirs: Vec<Directory>,
    pins: Vec<Pin>,
    link: Metadata,
    owner: String,
    sequence: usize,
    completed: Option<Execution>,
    seen: Option<Execution>,
    start: Option<Job>,
    stop: Option<Job>,
    native_verified: bool,
    stopped_verified: bool,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct NativeReceipt {
    schema: u32,
    synthetic_epoch: bool,
    effects: u32,
    full_inventory: bool,
    second_socket_untrusted: bool,
    absent: bool,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ArmedReceipt {
    version: u32,
    policy_version: u32,
    enrolled_uid: u32,
    generation: u64,
    armed: bool,
    flags: u32,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RetiredReceipt {
    version: u32,
    enrolled_uid: u32,
    boot: [u8; 16],
    host_netns_epoch: [u8; 16],
    netns_device: u64,
    netns_inode: u64,
    operation: u64,
    phase: String,
    table_handle: u64,
}

fn native_json(index: usize, raw: &[u8]) -> Result<Value> {
    // Struct deserialization rejects duplicate and unknown fields, wrong
    // integer/boolean types and malformed fixed-size epoch arrays.
    match index {
        0 => serde_json::to_value(serde_json::from_slice::<NativeReceipt>(raw).map_err(|_| ())?),
        1 => serde_json::to_value(serde_json::from_slice::<ArmedReceipt>(raw).map_err(|_| ())?),
        2 => serde_json::to_value(serde_json::from_slice::<RetiredReceipt>(raw).map_err(|_| ())?),
        _ => return Err(()),
    }
    .map_err(|_| ())
}

impl Real {
    fn recheck(&self, deep: bool) -> Result<()> {
        if let Some(witness) = &self.witness {
            witness.recheck()?;
        }
        for dir in &self.dirs {
            dir.recheck()?;
        }
        for pin in &self.pins {
            pin.recheck(deep)?;
        }
        ensure(
            meta(&self.link) == meta(&std::fs::symlink_metadata(LINK).map_err(|_| ())?)
                && std::fs::read_link(LINK).map_err(|_| ())?
                    == Path::new(STAGE).join("fixture.service"),
        )
    }
    fn write(&self, name: &str, value: &Value) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|_| ())?;
        ensure(bytes.len() <= 65536)?;
        admission::write_exclusive(&self.admitted.stage, name, &bytes).map_err(|_| ())
    }
    fn rpc<T: serde::ser::Serialize + zbus::zvariant::DynamicType>(
        &mut self,
        label: &'static str,
        path: &'static str,
        interface: &'static str,
        method: &'static str,
        args: &T,
    ) -> Result<zbus::Message> {
        ensure(self.sequence < 2 * MAX_POLLS + 16 && !self.owner.is_empty())?;
        self.recheck(false)?;
        let index = self.sequence;
        self.sequence += 1;
        self.write(
            &format!("lifecycle-{index:03}-before.json"),
            &json!({"schema":1,"boundary":"before-rpc","phase":label}),
        )?;
        self.admitted
            .connection
            .call_method(
                Some(self.owner.as_str()),
                path,
                Some(interface),
                method,
                args,
            )
            .map_err(|_| ())
    }
    fn snapshot(&mut self, label: &'static str) -> Result<Facts> {
        let reply = self.rpc(
            label,
            UNIT_PATH,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            &("",),
        )?;
        let fields = admission::decode::<admission::Properties<1024>>(&reply)
            .map(|v| v.0)
            .map_err(|_| ())?;
        let mut selected = serde_json::Map::new();
        for key in [
            "ActiveState",
            "SubState",
            "Job",
            "InvocationID",
            "Result",
            "NRestarts",
            "MainPID",
            "ControlPID",
            "ExecMainPID",
            "ExecMainCode",
            "ExecMainStatus",
            "ExecMainStartTimestampMonotonic",
            "ExecMainExitTimestampMonotonic",
            "WatchdogUSec",
            "ControlGroup",
            "ExecStart",
        ] {
            selected.insert(
                key.into(),
                serde_json::to_value(fields.get(key)).map_err(|_| ())?,
            );
        }
        // Private bounded observation BEFORE predicate evaluation: no diagnostic
        // inference or acceptance, and no further RPC after malformed/failed data.
        self.write(
            &format!("lifecycle-{:03}-observed.json", self.sequence - 1),
            &json!({"schema":1,"phase":label,"validated":false,"selected":selected}),
        )?;
        Ok(fields)
    }
    fn empty_cgroup(&self) -> Result<()> {
        let parents = [
            "/sys",
            "/sys/fs",
            "/sys/fs/cgroup",
            "/sys/fs/cgroup/system.slice",
        ]
        .iter()
        .map(|p| Directory::open(Path::new(p), false))
        .collect::<Result<Vec<_>>>()?;
        match std::fs::symlink_metadata(CGROUP) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                for dir in &parents {
                    dir.recheck()?;
                }
                ensure(
                    matches!(std::fs::symlink_metadata(CGROUP), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
                )
            }
            Err(_) => Err(()),
            Ok(_) => {
                let directory = Directory::open(Path::new(CGROUP), false)?;
                for _ in 0..2 {
                    for (name, require_empty) in [("cgroup.procs", true), ("cgroup.events", false)]
                    {
                        let path = Path::new(CGROUP).join(name);
                        let file = OpenOptions::new()
                            .read(true)
                            .custom_flags(
                                nix::libc::O_NOFOLLOW
                                    | nix::libc::O_CLOEXEC
                                    | nix::libc::O_NONBLOCK,
                            )
                            .open(&path)
                            .map_err(|_| ())?;
                        let before = file.metadata().map_err(|_| ())?;
                        ensure(before.is_file() && before.uid() == 0 && before.gid() == 0)?;
                        let mut bytes = Vec::new();
                        (&file).take(4097).read_to_end(&mut bytes).map_err(|_| ())?;
                        ensure(
                            bytes.len() <= 4096
                                && meta(&before) == meta(&file.metadata().map_err(|_| ())?)
                                && meta(&before)
                                    == meta(&std::fs::symlink_metadata(path).map_err(|_| ())?),
                        )?;
                        if require_empty {
                            ensure(bytes.is_empty())?;
                        } else {
                            let text = std::str::from_utf8(&bytes).map_err(|_| ())?;
                            let rows: Vec<_> = text.lines().collect();
                            ensure(rows == ["populated 0", "frozen 0"])?;
                        }
                    }
                    directory.recheck()?;
                    for dir in &parents {
                        dir.recheck()?;
                    }
                }
                Ok(())
            }
        }
    }
    fn native(&mut self) -> Result<()> {
        for directory in [
            format!("{STAGE}/state"),
            format!("{STAGE}/state/omavless-netguard"),
        ] {
            self.dirs
                .push(Directory::open(Path::new(&directory), true)?);
        }
        let names = [
            "native-result.json",
            "state/omavless-netguard/armed-v1.json",
            "state/omavless-netguard/table-receipt-v1.json",
        ];
        let mut values = Vec::new();
        for (index, name) in names.into_iter().enumerate() {
            self.pins
                .push(Pin::open(Path::new(STAGE).join(name), 0o600, 16384)?);
            let raw = self.pins.last().ok_or(())?.bytes()?;
            values.push(native_json(index, &raw)?);
        }
        ensure(
            values[0]
                == json!({"schema":1,"synthetic_epoch":true,"effects":2,"full_inventory":true,"second_socket_untrusted":true,"absent":true}),
        )?;
        ensure(
            values[1]
                == json!({"version":1,"policy_version":1,"enrolled_uid":1001,"generation":7,"armed":false,"flags":0}),
        )?;
        let receipt = values[2].as_object().ok_or(())?;
        ensure(
            receipt.len() == 9
                && receipt.get("version") == Some(&json!(1))
                && receipt.get("enrolled_uid") == Some(&json!(1001))
                && receipt.get("boot") == Some(&json!(vec![0x31; 16]))
                && receipt.get("host_netns_epoch") == Some(&json!(vec![0x32; 16]))
                && receipt.get("operation") == Some(&json!(2))
                && receipt.get("phase") == Some(&json!("retired"))
                && receipt.get("table_handle") == Some(&json!(0))
                && receipt
                    .get("netns_device")
                    .and_then(Value::as_u64)
                    .is_some_and(|v| v > 0)
                && receipt
                    .get("netns_inode")
                    .and_then(Value::as_u64)
                    .is_some_and(|v| v > 0),
        )?;
        self.empty_cgroup()?;
        self.recheck(true)
    }
}

impl FixedLifecycle for Real {
    fn admit_retaining_reference(&mut self) -> Result<()> {
        self.recheck(true)?;
        ensure(self.witness.is_none())?;
        self.witness = Some(crate::manager_negative_witness::RootWitness::capture()?);
        self.recheck(true)?;
        self.owner = admission::admit_retaining_reference(&self.admitted, || self.recheck(true))
            .map_err(|_| ())?;
        Ok(())
    }
    fn start_once(&mut self) -> Result<Job> {
        self.recheck(true)?;
        let current = self.snapshot("pre-start")?;
        never_started(&current)?;
        ensure(job_field(&current)? == (0, "/".into()))?;
        self.recheck(true)?;
        let reply = self.rpc(
            "start",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StartUnit",
            &(UNIT, "fail"),
        )?;
        let path: OwnedObjectPath = admission::decode(&reply).map_err(|_| ())?;
        let job = Job::from_path(path.as_str())?;
        self.start = Some(job.clone());
        Ok(job)
    }
    fn observe_start(&mut self, job: &Job) -> Result<StartObservation> {
        let current = self.snapshot("observe-start")?;
        let completed = start_phase(&current, job)?;
        let observed = if string(&current, "ActiveState")? == "activating" {
            Some(execution(&current, false)?)
        } else if string(&current, "ActiveState")? == "active" {
            Some(execution(&current, true)?)
        } else {
            None
        };
        if let Some(observed) = observed {
            if let Some(prior) = &self.seen {
                ensure(
                    prior.invocation == observed.invocation
                        && prior.pid == observed.pid
                        && prior.start == observed.start
                        && prior.command_start == observed.command_start,
                )?;
            }
            self.seen = Some(observed);
        } else {
            ensure(self.seen.is_none())?;
        }
        if let Some(execution) = completed {
            self.completed = Some(execution);
            Ok(StartObservation::Completed)
        } else {
            Ok(StartObservation::Pending)
        }
    }
    fn prove_native_completion(&mut self) -> Result<()> {
        self.native()?;
        self.native_verified = true;
        self.write(
            "lifecycle-native-proof.json",
            &json!({"schema":1,"execution":self.completed,
            "effects":2,"closed_retired":true,"absent":true,"synthetic_epoch":true}),
        )
    }
    fn stop_once(&mut self) -> Result<Job> {
        ensure(self.native_verified)?;
        self.recheck(true)?;
        let current = self.snapshot("pre-stop")?;
        ensure(start_phase(&current, self.start.as_ref().ok_or(())?)? == self.completed)?;
        self.empty_cgroup()?;
        let reply = self.rpc(
            "stop",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StopUnit",
            &(UNIT, "fail"),
        )?;
        let path: OwnedObjectPath = admission::decode(&reply).map_err(|_| ())?;
        let job = Job::from_path(path.as_str())?;
        ensure(Some(&job) != self.start.as_ref())?;
        self.stop = Some(job.clone());
        Ok(job)
    }
    fn observe_stop(&mut self, job: &Job) -> Result<StopObservation> {
        let current = self.snapshot("observe-stop")?;
        if stop_phase(&current, job, self.completed.as_ref().ok_or(())?)? {
            Ok(StopObservation::Completed)
        } else {
            Ok(StopObservation::Pending)
        }
    }
    fn prove_stopped(&mut self) -> Result<()> {
        self.empty_cgroup()?;
        self.recheck(true)?;
        self.stopped_verified = true;
        self.write(
            "lifecycle-stopped-proof.json",
            &json!({"schema":1,"execution":self.completed,
            "inactive_dead":true,"zero_pids":true,"no_job":true,"empty_cgroup":true}),
        )
    }
    fn unref_once(&mut self) -> Result<()> {
        ensure(self.stopped_verified)?;
        let reply = self.rpc(
            "unref",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "UnrefUnit",
            &(UNIT,),
        )?;
        admission::decode::<()>(&reply).map_err(|_| ())
    }
    fn publish_terminal_receipt(&mut self) -> Result<()> {
        self.recheck(true)?;
        self.write("lifecycle-result.json", &json!({"schema":1,"unit":UNIT,"unique_owner":self.owner,
            "start_job":{"id":self.start.as_ref().ok_or(())?.id,"path":self.start.as_ref().ok_or(())?.path},
            "stop_job":{"id":self.stop.as_ref().ok_or(())?.id,"path":self.stop.as_ref().ok_or(())?.path},
            "execution":self.completed,"effects":2,"closed_retired":true,"absent":true,
            "stopped":true,"unref_acknowledged":true,"synthetic_epoch":true,"production_admission":false}))
    }
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn pause_between_known_pending(&mut self) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "fixed private manager lifecycle; root review and exclusive VM lease required"]
fn run_private_lifecycle() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_RETAINED_LIFECYCLE").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        [
            "--exact",
            TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1"
        ]
    );
    assert_eq!(nix::unistd::getuid().as_raw(), 0);
    assert_eq!(nix::unistd::geteuid().as_raw(), 0);
    let build = || -> Result<Real> {
        let mut dirs = Vec::new();
        for path in ["/", "/run", "/run/systemd", "/run/systemd/system", STAGE] {
            dirs.push(Directory::open(Path::new(path), path == STAGE)?);
        }
        let unit = Pin::open(Path::new(STAGE).join("fixture.service"), 0o600, 16384)?;
        ensure(unit.bytes()? == UNIT_BYTES)?;
        let probe = Pin::open(Path::new(STAGE).join("probe"), 0o500, 128 * 1024 * 1024)?;
        let own = File::open("/proc/self/exe").map_err(|_| ())?;
        ensure(meta(&own.metadata().map_err(|_| ())?) == meta(&probe.initial))?;
        let link = std::fs::symlink_metadata(LINK).map_err(|_| ())?;
        ensure(
            link.file_type().is_symlink()
                && link.uid() == 0
                && link.gid() == 0
                && link.nlink() == 1,
        )?;
        let stage = dirs.last().ok_or(())?.file.try_clone().map_err(|_| ())?;
        let connection =
            zbus::blocking::connection::Builder::address("unix:path=/run/dbus/system_bus_socket")
                .map_err(|_| ())?
                .max_queued(8)
                .method_timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| ())?;
        Ok(Real {
            witness: None,
            admitted: admission::Held { connection, stage },
            dirs,
            pins: vec![unit, probe],
            link,
            owner: String::new(),
            sequence: 0,
            completed: None,
            seen: None,
            start: None,
            stop: None,
            native_verified: false,
            stopped_verified: false,
        })
    };
    // Construction has made no Ref/Start or namespace/nft effect. Once built,
    // all owners are leaked BEFORE any bus admission and survive error/panic.
    let held = Box::leak(Box::new(Coordinator::new(
        build().expect("K1_LIFECYCLE_PRE_EFFECT_REFUSED"),
    )));
    if !matches!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| held.once())),
        Ok(Ok(()))
    ) {
        eprintln!("K1_RETAINED_PRIVATE_UNCERTAIN_PARKED");
        loop {
            std::thread::park();
        }
    }
    println!("K1_RETAINED_PRIVATE_STOPPED_UNREF_NOT_PRODUCTION");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use zbus::zvariant::Value as BusValue;

    macro_rules! field {
        ($facts:expr, $key:expr, $value:expr) => {
            $facts.insert(
                $key.into(),
                OwnedValue::try_from(BusValue::from($value)).unwrap(),
            )
        };
    }

    fn queued(job: &Job) -> Facts {
        let mut f = permissions::expected_unit();
        f.extend(permissions::expected_service());
        field!(f, "Requires", vec!["sysinit.target", "system.slice"]);
        field!(
            f,
            "Job",
            (job.id, OwnedObjectPath::try_from(job.path.clone()).unwrap())
        );
        field!(f, "InvocationID", Vec::<u8>::new());
        field!(f, "ExecMainExitTimestampMonotonic", 0_u64);
        field!(f, "ExecMainCode", 0_i32);
        field!(f, "ExecMainStatus", 0_i32);
        field!(f, "Result", "success");
        field!(f, "NRestarts", 0_u32);
        f
    }

    fn running(job: &Job, finished: bool) -> Facts {
        let mut f = queued(job);
        field!(
            f,
            "ActiveState",
            if finished { "active" } else { "activating" }
        );
        field!(f, "SubState", if finished { "exited" } else { "start" });
        field!(f, "InvocationID", vec![7_u8; 16]);
        field!(f, "ExecMainPID", 123_u32);
        field!(f, "MainPID", if finished { 0_u32 } else { 123_u32 });
        field!(f, "ExecMainStartTimestampMonotonic", 100_u64);
        field!(
            f,
            "ExecMainExitTimestampMonotonic",
            if finished { 200_u64 } else { 0_u64 }
        );
        field!(f, "ExecMainCode", if finished { 1_i32 } else { 0_i32 });
        field!(f, "WatchdogUSec", 0_u64);
        let probe = format!("{STAGE}/probe");
        field!(
            f,
            "ExecStart",
            vec![(
                probe.clone(),
                vec![
                    probe,
                    "--exact".into(),
                    WRITER.into(),
                    "--ignored".into(),
                    "--nocapture".into(),
                    "--test-threads=1".into()
                ],
                false,
                1000_u64,
                100_u64,
                if finished { 2000_u64 } else { 0_u64 },
                if finished { 200_u64 } else { 0_u64 },
                123_u32,
                if finished { 1_i32 } else { 0_i32 },
                0_i32
            )]
        );
        f
    }

    fn completed(job: &Job) -> Facts {
        let mut f = running(job, true);
        field!(f, "Job", (0_u32, OwnedObjectPath::try_from("/").unwrap()));
        f
    }

    #[test]
    fn never_started_requires_exact_upstream_empty_byte_array_not_zero_id_aliases() {
        let job = Job::from_path("/org/freedesktop/systemd1/job/17").unwrap();
        assert!(never_started(&queued(&job)).is_ok());
        for bytes in [vec![0_u8; 16], vec![1_u8; 16], vec![0_u8], vec![1_u8; 17]] {
            let mut facts = queued(&job);
            field!(facts, "InvocationID", bytes);
            assert!(never_started(&facts).is_err());
        }
        let mut facts = queued(&job);
        facts.remove("InvocationID");
        assert!(never_started(&facts).is_err());
        field!(facts, "InvocationID", Vec::<u64>::new());
        assert!(never_started(&facts).is_err());
        field!(facts, "InvocationID", "");
        assert!(never_started(&facts).is_err());
        let mut after = completed(&job);
        for bytes in [vec![], vec![0_u8; 16]] {
            field!(after, "InvocationID", bytes);
            assert!(execution(&after, true).is_err());
        }
    }

    #[test]
    fn typed_start_phases_bind_one_job_and_fresh_normal_execution() {
        let job = Job::from_path("/org/freedesktop/systemd1/job/17").unwrap();
        assert_eq!(start_phase(&queued(&job), &job), Ok(None));
        assert_eq!(start_phase(&running(&job, false), &job), Ok(None));
        assert_eq!(start_phase(&running(&job, true), &job), Ok(None));
        assert!(start_phase(&completed(&job), &job).unwrap().is_some());
        let mut wrong = queued(&job);
        field!(
            wrong,
            "Job",
            (0_u32, OwnedObjectPath::try_from("/").unwrap())
        );
        assert!(start_phase(&wrong, &job).is_err());
        let foreign = Job::from_path("/org/freedesktop/systemd1/job/18").unwrap();
        assert!(start_phase(&running(&foreign, false), &job).is_err());
    }

    #[test]
    fn every_stable_security_field_missing_or_wrong_type_refuses_after_start() {
        let job = Job::from_path("/org/freedesktop/systemd1/job/17").unwrap();
        for key in completed(&job).keys() {
            // Some runtime fields are independently covered below; no stable
            // field is exempt merely because it is not an admission-phase field.
            let mut missing = completed(&job);
            missing.remove(key);
            assert!(start_phase(&missing, &job).is_err(), "missing {key}");
            let mut wrong = completed(&job);
            field!(wrong, key, 5_u8);
            assert!(start_phase(&wrong, &job).is_err(), "wrong type {key}");
        }
    }

    #[test]
    fn failures_signals_pid_mismatch_hooks_and_aliases_are_not_pending() {
        let job = Job::from_path("/org/freedesktop/systemd1/job/17").unwrap();
        for (key, value) in [
            ("ActiveState", "failed"),
            ("SubState", "failed"),
            ("Result", "exit-code"),
            ("ControlGroup", "/system.slice/foreign.service"),
        ] {
            let mut f = completed(&job);
            field!(f, key, value);
            assert!(start_phase(&f, &job).is_err());
        }
        for key in ["MainPID", "ControlPID", "ExecMainPID", "NRestarts"] {
            let mut f = completed(&job);
            field!(f, key, 55_u32);
            assert!(start_phase(&f, &job).is_err());
        }
        for key in ["ExecMainCode", "ExecMainStatus"] {
            let mut f = completed(&job);
            field!(f, key, 9_i32);
            assert!(start_phase(&f, &job).is_err());
        }
        for key in [
            "WatchdogUSec",
            "ExecMainStartTimestampMonotonic",
            "ExecMainExitTimestampMonotonic",
        ] {
            let mut f = completed(&job);
            field!(f, key, u64::MAX);
            assert!(start_phase(&f, &job).is_err());
        }
        for key in ["DropInPaths", "Wants", "TriggeredBy", "Names"] {
            let mut f = completed(&job);
            field!(f, key, vec!["foreign.service"]);
            assert!(start_phase(&f, &job).is_err());
        }
    }

    #[test]
    fn stop_requires_the_same_completed_invocation_and_exact_stop_job() {
        let start = Job::from_path("/org/freedesktop/systemd1/job/17").unwrap();
        let stop = Job::from_path("/org/freedesktop/systemd1/job/23").unwrap();
        let prior = start_phase(&completed(&start), &start).unwrap().unwrap();
        let mut pending = completed(&start);
        field!(
            pending,
            "Job",
            (
                stop.id,
                OwnedObjectPath::try_from(stop.path.clone()).unwrap()
            )
        );
        assert_eq!(stop_phase(&pending, &stop, &prior), Ok(false));
        let mut done = completed(&start);
        field!(done, "ActiveState", "inactive");
        field!(done, "SubState", "dead");
        assert_eq!(stop_phase(&done, &stop, &prior), Ok(true));
        field!(done, "InvocationID", vec![8_u8; 16]);
        assert!(stop_phase(&done, &stop, &prior).is_err());
        assert!(stop_phase(&pending, &start, &prior).is_err());
    }

    #[test]
    fn strict_native_records_reject_duplicates_unknown_fields_and_boolean_numbers() {
        let raw = br#"{"schema":1,"synthetic_epoch":true,"effects":2,"full_inventory":true,"second_socket_untrusted":true,"absent":true}"#;
        assert!(native_json(0, raw).is_ok());
        for altered in [
            String::from_utf8(raw.to_vec())
                .unwrap()
                .replace("\"schema\":1", "\"schema\":1,\"schema\":1"),
            String::from_utf8(raw.to_vec())
                .unwrap()
                .replace("\"effects\":2", "\"effects\":true"),
            String::from_utf8(raw.to_vec())
                .unwrap()
                .replace("\"effects\":2", "\"effects\":2,\"extra\":0"),
        ] {
            assert!(native_json(0, altered.as_bytes()).is_err());
        }
        assert!(native_json(3, raw).is_err());
    }

    #[test]
    fn real_original_fd_replacement_reversion_mutation_links_modes_and_bounds_refuse() {
        let root = crate::test_temp::directory("k1-held-file").unwrap();
        let path = root.join("record");
        std::fs::write(&path, b"same").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let owner = (
            nix::unistd::getuid().as_raw(),
            nix::unistd::getgid().as_raw(),
        );
        let pin = Pin::open_owned(path.clone(), 0o600, 4, owner).unwrap();
        assert_eq!(pin.bytes().unwrap(), b"same");
        assert!(Pin::open_owned(path.clone(), 0o500, 4, owner).is_err());
        assert!(Pin::open_owned(path.clone(), 0o600, 3, owner).is_err());
        std::fs::rename(&path, root.join("old")).unwrap();
        std::fs::write(&path, b"same").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(pin.recheck(true).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(root.join("old"), &path).unwrap();
        assert!(pin.recheck(true).is_err()); // ctime catches rename-away/back.
        let fresh = Pin::open_owned(path.clone(), 0o600, 4, owner).unwrap();
        std::fs::write(&path, b"else").unwrap();
        assert!(fresh.recheck(true).is_err());
        symlink(&path, root.join("alias")).unwrap();
        assert!(Pin::open_owned(root.join("alias"), 0o600, 4, owner).is_err());
        std::fs::hard_link(&path, root.join("hard")).unwrap();
        assert!(Pin::open_owned(path, 0o600, 4, owner).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
