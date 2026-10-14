// SPDX-License-Identifier: MIT
//! Explicit administrator preparation for the independently pinned Idle-gate
//! public broker. Never signal a live broker or unlink its socket/journal.
//! Closing its socket access causes its ORIGINAL Idle recheck to exit naturally;
//! already admitted sessions must finish under their original broker owner.
use super::{Blocker, Captured, Result, same};
use nix::unistd::Uid;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::fs;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SOCKET: &str = "/run/omavless-dns/control.sock";
const BROKER: &str = "/usr/lib/omavless-dns/omavless-dns-broker";
const UNIT: &str = "omavless-dns-broker.service";
const GUARD: &str = "/usr/lib/omavless-dns/package-guard";
const WAIT: Duration = Duration::from_secs(35);

fn refused<T>() -> Result<T> {
    Err(Blocker::BrokerPreparationRequired)
}
fn unknown<T>() -> Result<T> {
    Err(Blocker::MaintenanceOutcomeUnknown)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrollment {
    schema: u32,
    uid: u32,
    policy: String,
}

fn protected(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path).map_err(|_| Blocker::BrokerPreparationRequired)?;
    if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
        return refused();
    }
    Ok(())
}
fn empty() -> Result<()> {
    for path in [
        "/",
        "/run",
        "/run/omavless-dns",
        "/run/omavless-dns/private",
    ] {
        protected(Path::new(path))?;
    }
    // Inert census: never open/create a Journal or inspect private entries.
    let mut entries = fs::read_dir("/run/omavless-dns/private")
        .map_err(|_| Blocker::BrokerPreparationRequired)?;
    if entries.next().is_some() {
        return refused();
    }
    if !super::absent(Path::new("/sys/class/net/Meta")) {
        return refused();
    }
    Ok(())
}
fn query() -> Result<String> {
    let mut command = Command::new("/usr/bin/systemctl");
    command.env_clear().env("PATH", "/usr/bin:/bin").env("LC_ALL", "C")
        .args(["--system", "show", UNIT, "--no-pager",
            "--property=LoadState,ActiveState,SubState,MainPID,NFileDescriptorStore,DropInPaths,FragmentPath,NeedDaemonReload,Job,ExecMainCode,ExecMainStatus,Result"]);
    crate::production_observation::bounded_fixed_query(command, Duration::from_secs(3))
        .map_err(|_| Blocker::BrokerPreparationRequired)
}
fn fields(raw: &str) -> Result<std::collections::BTreeMap<&str, &str>> {
    let allowed = [
        "LoadState",
        "ActiveState",
        "SubState",
        "MainPID",
        "NFileDescriptorStore",
        "DropInPaths",
        "FragmentPath",
        "NeedDaemonReload",
        "Job",
        "ExecMainCode",
        "ExecMainStatus",
        "Result",
    ];
    let mut values = std::collections::BTreeMap::new();
    for line in raw.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or(Blocker::BrokerPreparationRequired)?;
        if !allowed.contains(&key) || values.insert(key, value).is_some() {
            return refused();
        }
    }
    if values.len() != allowed.len()
        || values.get("LoadState") != Some(&"loaded")
        || values.get("DropInPaths") != Some(&"")
        || values.get("Job") != Some(&"")
        || values.get("NeedDaemonReload") != Some(&"no")
        || values.get("FragmentPath")
            != Some(&"/usr/lib/systemd/system/omavless-dns-broker.service")
        || values.get("NFileDescriptorStore") != Some(&"0")
    {
        return refused();
    }
    Ok(values)
}
fn prepared(raw: &str) -> bool {
    fields(raw).is_ok_and(|v| {
        v.get("ActiveState") == Some(&"inactive")
            && v.get("SubState") == Some(&"dead")
            && v.get("MainPID") == Some(&"0")
    })
}
fn clean_exit(raw: &str) -> bool {
    fields(raw).is_ok_and(|v| {
        v.get("ActiveState") == Some(&"failed")
            && v.get("SubState") == Some(&"failed")
            && v.get("MainPID") == Some(&"0")
            && v.get("ExecMainCode") == Some(&"1")
            && v.get("ExecMainStatus") == Some(&"1")
            && v.get("Result") == Some(&"exit-code")
    })
}
fn original_dead(pidfd: &OwnedFd) -> Result<bool> {
    let mut rows = [PollFd::new(pidfd, PollFlags::IN)];
    poll(
        &mut rows,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    let events = rows[0].revents();
    if events.intersects(PollFlags::ERR | PollFlags::NVAL) {
        return unknown();
    }
    Ok(events.contains(PollFlags::IN))
}
fn pin_broker() -> Result<super::package_phase::Archive> {
    pin_broker_at(Path::new(BROKER), 0, std::env::consts::ARCH)
}
fn idle_broker_pins(arch: &str) -> Result<&'static [&'static str]> {
    // These released binaries have next_session's serialized Idle access gate.
    // Original v0.9.8 lacks it and MUST NOT use this preparation.
    // fix.5 keeps production broker/access/channel inputs unchanged; these
    // exact architecture-specific payloads were re-inspected against its
    // immutable public archives. Never trust arbitrary installed receipt pins.
    match arch {
        "x86_64" => Ok(&[
            "ba61e433d89baa3ae4fd1af5ed275f769654c19fbf6a7b22592869e617b277d5",
            "1855ed920a4b064c811ed14134b5ae14e3c5ece0a26695a84afc1ccfd711dc93",
        ]),
        "aarch64" => Ok(&[
            "8f250d4fce7e784477c91204579ed6f8792006713ccc895ac357e23faa03df26",
            "c764c30ba24844f6972e2b19c600c068c92682d9fe1e1bd94de5896356f93c5e",
        ]),
        _ => refused(),
    }
}
// Private inert test seam; production always supplies the fixed root path/UID.
fn pin_broker_at(path: &Path, owner: u32, arch: &str) -> Result<super::package_phase::Archive> {
    for digest in idle_broker_pins(arch)? {
        if let Ok(captured) = super::package_phase::Archive::open(path.into(), owner, digest, 0o755)
        {
            return Ok(captured);
        }
    }
    refused()
}
fn exact_acl(uid: u32) -> [u8; 44] {
    let mut output = [0; 44];
    output[..4].copy_from_slice(&2_u32.to_le_bytes());
    for (position, (tag, permission, id)) in [
        (1_u16, 6_u16, u32::MAX),
        (2, 6, uid),
        (4, 0, u32::MAX),
        (16, 6, u32::MAX),
        (32, 0, u32::MAX),
    ]
    .into_iter()
    .enumerate()
    {
        let start = 4 + position * 8;
        output[start..start + 2].copy_from_slice(&tag.to_le_bytes());
        output[start + 2..start + 4].copy_from_slice(&permission.to_le_bytes());
        output[start + 4..start + 8].copy_from_slice(&id.to_le_bytes());
    }
    output
}
fn acl_matches(path: &Path, expected: &[u8; 44]) -> bool {
    let mut actual = [0; 44];
    rustix::fs::lgetxattr(path, "system.posix_acl_access", &mut actual[..])
        .is_ok_and(|count| count == expected.len() && actual == *expected)
}
fn close_socket(
    path: &Path,
    original: &fs::Metadata,
    observe: impl FnOnce() -> std::io::Result<fs::Metadata>,
) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    let now = match observe() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return unknown(),
    };
    if !now.file_type().is_socket()
        || (
            now.dev(),
            now.ino(),
            now.uid(),
            now.nlink(),
            now.mode() & 0o7777,
        ) != (original.dev(), original.ino(), original.uid(), 1, 0o600)
    {
        return unknown();
    }
    Ok(())
}

trait Host {
    fn before(&mut self) -> Result<()>;
    fn close_admission(&mut self) -> Result<()>;
    fn await_original_exit(&mut self) -> Result<()>;
    fn clean_after(&mut self) -> Result<()>;
    fn normalize_stopped(&mut self) -> Result<()>;
    fn final_guard(&mut self) -> Result<()>;
}
fn prepare(host: &mut impl Host) -> Result<()> {
    host.before()?;
    // No implicit recovery or automatic reversal after this explicit effect.
    host.close_admission()
        .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    host.await_original_exit()
        .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    host.clean_after()
        .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    host.normalize_stopped()
        .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    host.final_guard()
        .map_err(|_| Blocker::MaintenanceOutcomeUnknown)
}

struct Live {
    pid: u32,
    original: OwnedFd,
    socket: fs::Metadata,
    enrollment: Captured,
    broker: super::package_phase::Archive,
    unit: Captured,
    guard: Captured,
    acl: [u8; 44],
}
impl Live {
    fn capture(uid: u32, raw: &str) -> Result<Self> {
        let values = fields(raw)?;
        if values.get("ActiveState") != Some(&"active")
            || values.get("SubState") != Some(&"running")
        {
            return refused();
        }
        let pid: u32 = values
            .get("MainPID")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v > 1 && *v <= i32::MAX as u32)
            .ok_or(Blocker::BrokerPreparationRequired)?;
        let original = pidfd_open(
            Pid::from_raw(pid as i32).ok_or(Blocker::BrokerPreparationRequired)?,
            PidfdFlags::NONBLOCK,
        )
        .map_err(|_| Blocker::BrokerPreparationRequired)?;
        if original_dead(&original)? {
            return refused();
        }
        for path in [
            "/etc",
            "/etc/omavless-dns",
            "/usr",
            "/usr/lib",
            "/usr/lib/omavless-dns",
            "/usr/lib/systemd",
            "/usr/lib/systemd/system",
        ] {
            protected(Path::new(path))?;
        }
        let enrollment = Captured::read(
            Path::new("/etc/omavless-dns/release-enrollment.json"),
            0,
            0o600,
            256,
        )?;
        let entry: Enrollment = serde_json::from_slice(&enrollment.bytes)
            .map_err(|_| Blocker::BrokerPreparationRequired)?;
        if entry.schema != 1 || entry.uid != uid || entry.policy != "meta-ipv4-release-v1" {
            return refused();
        }
        let broker = pin_broker()?;
        let proc =
            fs::metadata(format!("/proc/{pid}")).map_err(|_| Blocker::BrokerPreparationRequired)?;
        let executable = fs::metadata(format!("/proc/{pid}/exe"))
            .map_err(|_| Blocker::BrokerPreparationRequired)?;
        let binary =
            fs::symlink_metadata(BROKER).map_err(|_| Blocker::BrokerPreparationRequired)?;
        if proc.uid() != 0 || !same(&executable, &binary) || original_dead(&original)? {
            return refused();
        }
        let unit = Captured::read(
            Path::new("/usr/lib/systemd/system/omavless-dns-broker.service"),
            0,
            0o644,
            16384,
        )?;
        let original_unit =
            include_str!("../../../../tests/dns_broker_host/omavless-dns-broker.service");
        let expected_unit = original_unit
            .replace(
                "# REVIEW-ONLY opt-in candidate. Not installed or enabled by normal packages.\n",
                "# Installation alone does not enroll, enable, or start this service.\n",
            )
            .replace(
                "ExecStart=/usr/lib/omavless/omavless-dns-broker --serve",
                "ExecStart=/usr/lib/omavless-dns/omavless-dns-broker --serve",
            );
        if unit.bytes != expected_unit.as_bytes() {
            return refused();
        }
        let guard = Captured::read(Path::new(GUARD), 0, 0o755, 16384)?;
        if guard.bytes != include_bytes!("../../../../tests/dns_broker_host/package/package-guard")
        {
            return refused();
        }
        empty()?;
        let socket =
            fs::symlink_metadata(SOCKET).map_err(|_| Blocker::BrokerPreparationRequired)?;
        if !socket.file_type().is_socket()
            || socket.uid() != 0
            || socket.nlink() != 1
            || socket.mode() & 0o7777 != 0o660
        {
            return refused();
        }
        let acl = exact_acl(uid);
        if !acl_matches(Path::new(SOCKET), &acl) {
            return refused();
        }
        Ok(Self {
            pid,
            original,
            socket,
            enrollment,
            broker,
            unit,
            guard,
            acl,
        })
    }
    fn identities(&mut self) -> Result<()> {
        self.enrollment.recheck()?;
        self.broker.recheck()?;
        self.unit.recheck()?;
        self.guard.recheck()
    }
}
impl Host for Live {
    fn before(&mut self) -> Result<()> {
        self.identities()?;
        empty()?;
        let raw = query()?;
        let values = fields(&raw)?;
        if values.get("ActiveState") != Some(&"active")
            || values.get("SubState") != Some(&"running")
            || values.get("MainPID").and_then(|v| v.parse::<u32>().ok()) != Some(self.pid)
            || original_dead(&self.original)?
        {
            return refused();
        }
        let socket =
            fs::symlink_metadata(SOCKET).map_err(|_| Blocker::BrokerPreparationRequired)?;
        if !same(&self.socket, &socket) || !acl_matches(Path::new(SOCKET), &self.acl) {
            return refused();
        }
        Ok(())
    }
    fn close_admission(&mut self) -> Result<()> {
        // root-protected ancestors prevent ordinary UID path replacement.
        // chmod changes the POSIX ACL mask, denying the enrolled UID. The old
        // broker notices the exact mode change only on its original Idle gate.
        // Its natural Drop can win after chmod. Absence earns nothing alone:
        // await_original_exit and every clean postcheck always follow.
        close_socket(Path::new(SOCKET), &self.socket, || {
            fs::symlink_metadata(SOCKET)
        })
    }
    fn await_original_exit(&mut self) -> Result<()> {
        let until = Instant::now() + WAIT;
        while !original_dead(&self.original)? {
            if Instant::now() >= until {
                return unknown();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
    fn clean_after(&mut self) -> Result<()> {
        self.identities()
            .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        empty().map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        if !original_dead(&self.original)? || !super::absent(Path::new(SOCKET)) {
            return unknown();
        }
        // pidfd exit can precede the manager's final state publication.
        // Observe boundedly; do not resend an effect or signal the service.
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            let state = query()?;
            if clean_exit(&state) {
                break;
            }
            let values = fields(&state).map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
            if values
                .get("MainPID")
                .and_then(|v| v.parse::<u32>().ok())
                .is_none_or(|pid| pid != 0 && pid != self.pid)
                || Instant::now() >= until
            {
                return unknown();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }
    fn normalize_stopped(&mut self) -> Result<()> {
        // Only after the original broker has exited and clean checks passed.
        // This does not stop, kill, enable or start a broker.
        let result = Command::new("/usr/bin/systemctl")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .args(["--system", "reset-failed", UNIT])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !result.is_ok_and(|s| s.success()) {
            return unknown();
        }
        Ok(())
    }
    fn final_guard(&mut self) -> Result<()> {
        self.identities()
            .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        empty().map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        if !prepared(&query()?) || !super::absent(Path::new(SOCKET)) {
            return unknown();
        }
        guard().map_err(|_| Blocker::MaintenanceOutcomeUnknown)
    }
}
fn guard() -> Result<()> {
    let result = Command::new(GUARD)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !result.is_ok_and(|s| s.success()) {
        return refused();
    }
    Ok(())
}
pub fn run_root(raw_uid: &std::ffi::OsStr) -> Result<serde_json::Value> {
    if !Uid::current().is_root() || !Uid::effective().is_root() {
        return refused();
    }
    let uid = raw_uid
        .to_str()
        .filter(|s| !s.is_empty() && s.len() <= 10 && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|v| *v > 0 && *v < u32::MAX)
        .ok_or(Blocker::BrokerPreparationRequired)?;
    // Only fixed root paths/commands. No runtime user data or environment targets.
    let raw = query()?;
    let mut live = Live::capture(uid, &raw)?;
    prepare(&mut live)?;
    Ok(
        serde_json::json!({"schemaVersion":1,"scope":"explicit_098_broker_preparation","outcome":"broker_prepared","packagesChanged":false,"enrollmentChanged":false,"vpnStarted":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_pins_are_exact_architecture_specific_and_exclude_original_brokers() {
        let original = [
            "8caba1a57b44b2856b98b0d0213cc545905ba5a031aac144fcce2420957b0d2e",
            "33121a6f4c4eb7b644da034fc521e221d7f292d61bdaffb837ed82c015dbe755",
        ];
        for (arch, other) in [("x86_64", "aarch64"), ("aarch64", "x86_64")] {
            let pins = idle_broker_pins(arch).unwrap();
            assert_eq!(pins.len(), 2);
            let pair = super::super::published098("99c556e5c966e1e1dae04f2d8babd2ca2dec9375", arch)
                .unwrap();
            assert!(pins.contains(&pair.broker_sha.as_str()));
            for denied in idle_broker_pins(other)
                .unwrap()
                .iter()
                .chain(original.iter())
            {
                assert!(!pins.contains(denied));
            }
        }
        assert!(idle_broker_pins("unknown").is_err());
    }
    #[test]
    fn self_consistent_or_modified_unknown_binary_never_earns_preparation() {
        let directory = crate::test_temp::directory("idlepin").unwrap();
        let path = directory.join("b");
        fs::write(&path, b"inert locally rebuilt broker; never execute").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        for arch in ["x86_64", "aarch64", "unknown"] {
            assert!(pin_broker_at(&path, Uid::current().as_raw(), arch).is_err());
        }
        fs::write(&path, b"modified inert broker").unwrap();
        assert!(pin_broker_at(&path, Uid::current().as_raw(), "x86_64").is_err());
        fs::remove_dir_all(directory).unwrap();
    }
    struct Fixture {
        trace: Vec<&'static str>,
        fail: Option<&'static str>,
    }
    impl Fixture {
        fn step(&mut self, name: &'static str) -> Result<()> {
            self.trace.push(name);
            if self.fail == Some(name) {
                refused()
            } else {
                Ok(())
            }
        }
    }
    impl Host for Fixture {
        fn before(&mut self) -> Result<()> {
            self.step("before")
        }
        fn close_admission(&mut self) -> Result<()> {
            self.step("close")
        }
        fn await_original_exit(&mut self) -> Result<()> {
            self.step("await")
        }
        fn clean_after(&mut self) -> Result<()> {
            self.step("after")
        }
        fn normalize_stopped(&mut self) -> Result<()> {
            self.step("normalize")
        }
        fn final_guard(&mut self) -> Result<()> {
            self.step("guard")
        }
    }
    #[test]
    fn each_failure_blocks_every_dependent_effect_without_restore_kill_or_cleanup() {
        let order = ["before", "close", "await", "after", "normalize", "guard"];
        for (index, failure) in order.iter().enumerate() {
            let mut host = Fixture {
                trace: vec![],
                fail: Some(failure),
            };
            assert_eq!(
                prepare(&mut host),
                Err(if index == 0 {
                    Blocker::BrokerPreparationRequired
                } else {
                    Blocker::MaintenanceOutcomeUnknown
                })
            );
            assert_eq!(host.trace, order[..=index]);
        }
        let mut host = Fixture {
            trace: vec![],
            fail: None,
        };
        prepare(&mut host).unwrap();
        assert_eq!(host.trace, order);
    }
    fn state(active: &str, sub: &str, pid: &str, code: &str, status: &str, result: &str) -> String {
        format!(
            "LoadState=loaded\nActiveState={active}\nSubState={sub}\nMainPID={pid}\nNFileDescriptorStore=0\nDropInPaths=\nFragmentPath=/usr/lib/systemd/system/omavless-dns-broker.service\nNeedDaemonReload=no\nJob=\nExecMainCode={code}\nExecMainStatus={status}\nResult={result}\n"
        )
    }
    #[test]
    fn failure_exit_is_not_itself_clean_or_stopped_evidence() {
        let exited = state("failed", "failed", "0", "1", "1", "exit-code");
        assert!(clean_exit(&exited));
        assert!(!prepared(&exited));
        assert!(prepared(&state(
            "inactive", "dead", "0", "1", "1", "success"
        )));
        for bad in [
            exited.replace("ExecMainCode=1", "ExecMainCode=2"),
            exited.replace("Result=exit-code", "Result=oom-kill"),
            exited.replace("NFileDescriptorStore=0", "NFileDescriptorStore=1"),
            exited.replace("Job=\n", "Job=123\n"),
            exited.replace("DropInPaths=\n", "DropInPaths=changed\n"),
            format!("{exited}MainPID=0\n"),
            exited.replace("NeedDaemonReload=no", "NeedDaemonReload=yes"),
        ] {
            assert!(!clean_exit(&bad));
        }
    }
    #[test]
    fn nonroot_invocation_never_observes_or_changes_root_state() {
        if !Uid::current().is_root() {
            assert!(run_root(std::ffi::OsStr::new("1000")).is_err());
        }
    }
    #[test]
    fn original_pidfd_is_alive_until_actual_owned_child_exit() {
        let mut child = Command::new("/usr/bin/sleep").arg("30").spawn().unwrap();
        let fd = pidfd_open(
            Pid::from_raw(child.id() as i32).unwrap(),
            PidfdFlags::NONBLOCK,
        )
        .unwrap();
        assert!(!original_dead(&fd).unwrap());
        // Only this inert test-owned child, never a product broker/helper.
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(original_dead(&fd).unwrap());
        assert!(original_dead(&fd).unwrap());
    }
    #[test]
    fn actual_linux_socket_acl_mask_closes_the_expected_old_access_gate() {
        use std::os::unix::net::UnixListener;
        let parent = crate::test_temp::home_parent().unwrap();
        let directory = tempfile::tempdir_in(parent).unwrap();
        let path = directory.path().join("s");
        let listener = UnixListener::bind(&path).unwrap();
        let expected = exact_acl(42424);
        rustix::fs::lsetxattr(
            &path,
            "system.posix_acl_access",
            &expected,
            rustix::fs::XattrFlags::empty(),
        )
        .unwrap();
        assert!(acl_matches(&path, &expected));
        let original = fs::symlink_metadata(&path).unwrap();
        assert_eq!(original.mode() & 0o7777, 0o660);
        close_socket(&path, &original, || fs::symlink_metadata(&path)).unwrap();
        assert!(!acl_matches(&path, &expected));
        let changed = fs::symlink_metadata(&path).unwrap();
        assert_eq!(
            (original.dev(), original.ino()),
            (changed.dev(), changed.ino())
        );
        assert_eq!(changed.mode() & 0o7777, 0o600);
        drop(listener);
    }
    #[test]
    fn original_socket_disappearance_after_chmod_advances_only_to_remaining_checks() {
        use std::os::unix::net::UnixListener;
        let directory = tempfile::tempdir_in(crate::test_temp::home_parent().unwrap()).unwrap();
        let path = directory.path().join("s");
        let listener = UnixListener::bind(&path).unwrap();
        let original = fs::symlink_metadata(&path).unwrap();
        close_socket(&path, &original, || {
            // Test-only simulation of old Listener's same-inode Drop.
            drop(listener);
            fs::remove_file(&path)?;
            fs::symlink_metadata(&path)
        })
        .unwrap();
        assert!(!path.exists());
        let mut host = Fixture {
            trace: vec![],
            fail: Some("await"),
        };
        assert_eq!(prepare(&mut host), Err(Blocker::MaintenanceOutcomeUnknown));
        assert_eq!(host.trace, ["before", "close", "await"]);
    }
    #[test]
    fn replaced_socket_after_chmod_is_unknown_and_never_deleted_or_adopted() {
        use std::os::unix::net::UnixListener;
        let directory = tempfile::tempdir_in(crate::test_temp::home_parent().unwrap()).unwrap();
        let path = directory.path().join("s");
        let original_listener = UnixListener::bind(&path).unwrap();
        let original = fs::symlink_metadata(&path).unwrap();
        let moved = directory.path().join("original");
        let mut replacement = None;
        assert_eq!(
            close_socket(&path, &original, || {
                fs::rename(&path, &moved)?;
                replacement = Some(UnixListener::bind(&path)?);
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
                fs::symlink_metadata(&path)
            }),
            Err(Blocker::MaintenanceOutcomeUnknown)
        );
        assert!(path.exists());
        assert!(moved.exists());
        drop(replacement);
        drop(original_listener);
    }
}
