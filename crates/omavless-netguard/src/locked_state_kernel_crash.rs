// SPDX-License-Identifier: MIT
// VM-only test fixture. Never returns a verified policy or kernel identity.
use super::*;
use serde_json::{Value, json};
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const HOLDER: &str = "locked_state::tests::kernel_crash::isolated_holder";
const WRITER: &str = "locked_state::tests::kernel_crash::isolated_writer";
const LIMIT: u64 = 32768;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn identity(file: &File) -> (u64, u64) {
    let m = file.metadata().unwrap();
    assert_eq!(
        fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap(),
        PathBuf::from(format!("net:[{}]", m.ino()))
    );
    (m.dev(), m.ino())
}

struct Guard {
    parent: File,
    current: File,
    id: (u64, u64),
}
impl Guard {
    fn inherited() -> Self {
        let parent = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
        let current = File::open("/proc/self/ns/net").unwrap();
        let id = identity(&current);
        let guard = Self {
            parent,
            current,
            id,
        };
        guard.check();
        guard
    }
    fn check(&self) {
        assert_ne!(identity(&self.parent), self.id, "parent namespace refused");
        assert_eq!(identity(&self.current), self.id);
        assert_eq!(identity(&File::open("/proc/self/ns/net").unwrap()), self.id);
        let dev = fs::read_to_string("/proc/net/dev").unwrap();
        let lines: Vec<_> = dev.lines().collect();
        assert_eq!(lines.len(), 3);
        let (name, counters) = lines[2].split_once(':').unwrap();
        assert_eq!(name.trim(), "lo");
        assert_eq!(counters.split_whitespace().count(), 16);
        assert!(
            counters
                .split_whitespace()
                .all(|v| v.parse::<u64>().is_ok())
        );
    }
    fn epoch(&self) -> NamespaceObservation {
        // Synthetic test epoch, NOT canonical host/service provenance.
        NamespaceObservation::Canonical(HostEpoch {
            namespace_device: self.id.0,
            namespace_inode: self.id.1,
            ..EPOCH
        })
    }
}

fn bounded(file: &std::path::Path) -> Vec<u8> {
    use std::io::Read;
    let mut bytes = Vec::new();
    File::open(file)
        .unwrap()
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() as u64 <= LIMIT, "fixture output bound");
    bytes
}

fn run(mut command: Command, stdin: Stdio, scratch: &Fixture, seconds: u64) -> (bool, Vec<u8>) {
    let out = scratch.0.join("command-out");
    let err = scratch.0.join("command-err");
    let create = |path| {
        fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .unwrap()
    };
    let mut child = ChildGuard(
        command
            .stdin(stdin)
            .stdout(create(&out))
            .stderr(create(&err))
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "fixture child timeout");
        assert!(
            fs::metadata(&out).unwrap().len() <= LIMIT
                && fs::metadata(&err).unwrap().len() <= LIMIT
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    let bytes = bounded(&out);
    let _ = bounded(&err); // Never expose raw subprocess error text.
    (status.success(), bytes)
}

fn nft(guard: &Guard, scratch: &Fixture, args: &[&str]) -> (bool, Vec<u8>) {
    guard.check();
    let mut command = Command::new("/usr/bin/nft");
    command.env_clear().env("LC_ALL", "C").args(args);
    let result = run(command, Stdio::null(), scratch, 3);
    guard.check();
    result
}

fn table(guard: &Guard, scratch: &Fixture, name: &str) -> Option<Value> {
    let (ok, bytes) = nft(
        guard,
        scratch,
        &["--json", "--handle", "list", "tables", "inet"],
    );
    assert!(ok, "table inventory unavailable");
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let matches: Vec<_> = value["nftables"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.get("table"))
        .filter(|t| t["family"] == "inet" && t["name"] == name)
        .collect();
    assert!(matches.len() <= 1);
    matches.first().map(|v| {
        assert!(v["handle"].as_u64().is_some_and(|h| h > 0));
        (*v).clone()
    })
}

fn create(guard: &Guard, scratch: &Fixture, name: &str) {
    assert!(table(guard, scratch, name).is_none());
    let payload = json!({"nftables": [{"create": {"table": {"family":"inet", "name":name}}}]});
    let path = scratch.0.join("create.json");
    fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap()
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .unwrap();
    assert!(
        nft(
            guard,
            scratch,
            &["--json", "--file", path.to_str().unwrap()]
        )
        .0
    );
    assert!(table(guard, scratch, name).is_some());
}

fn pause(fixture: &Fixture) -> ! {
    fs::write(fixture.0.join("ready"), b"checkpoint").unwrap();
    let expected_parent: u32 = std::env::var("OMAVLESS_K1_RECEIPT_OWNER")
        .unwrap()
        .parse()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        // A killed namespace holder must not leave a parked writer retaining
        // the namespace. Reparenting revokes this fixture; never repair state.
        let parent = fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("PPid:")?.trim().parse::<u32>().ok())
            });
        if parent != Some(expected_parent) || Instant::now() >= deadline {
            std::process::exit(70);
        }
        std::thread::park_timeout(Duration::from_millis(5));
    }
}

struct UntrustedKernel<'a> {
    guard: &'a Guard,
    scratch: &'a Fixture,
    target: String,
    cut: u8,
    effects: usize,
}
impl EffectPort for UntrustedKernel<'_> {
    fn observe(&mut self) -> Result<EffectSnapshot, EffectError> {
        Ok(EffectSnapshot {
            table: if table(self.guard, self.scratch, &self.target).is_some() {
                Table::Foreign
            } else {
                Table::Absent
            },
            identity: None,
        })
    }
    fn create_if_absent(&mut self, _: Policy) -> Result<EffectIdentity, EffectError> {
        self.effects += 1;
        assert!(
            self.cut == 1 || self.cut == 2,
            "restart attempted an effect"
        );
        create(self.guard, self.scratch, &self.target);
        // nft has received kernel completion and readback confirms presence,
        // but the caller receives NO identity/ownership acknowledgement.
        if self.cut == 1 {
            pause(self.scratch);
        }
        Err(EffectError::UnavailableOrUncertain)
    }
    fn replace_owned(
        &mut self,
        _: EffectIdentity,
        _: Policy,
    ) -> Result<EffectIdentity, EffectError> {
        panic!("replace authority must not be requested")
    }
    fn delete_owned(&mut self, _: EffectIdentity) -> Result<(), EffectError> {
        panic!("delete authority must not be requested")
    }
}

fn names(owner: u32, cut: u8) -> (String, String) {
    assert!(owner != 0 && cut <= 2);
    (
        format!("ov_k1_crash_{owner}_{cut}"),
        format!("ov_k1_sentinel_{owner}_{cut}"),
    )
}

#[test]
#[ignore = "VM-only isolated kernel/file crash gate"]
fn kernel_receipt_crash_in_disposable_vm() {
    assert_eq!(std::env::var("OMAVLESS_K1_RECEIPT_VM").as_deref(), Ok("1"));
    let scratch = Fixture::new();
    let parent = File::open("/proc/self/ns/net").unwrap();
    let id = identity(&parent);
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("LC_ALL", "C")
        .env("TMPDIR", &scratch.0)
        .env("OMAVLESS_K1_RECEIPT_CHILD", "1")
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", HOLDER, "--nocapture"]);
    let (ok, output) = run(
        command,
        Stdio::from(parent.try_clone().unwrap()),
        &scratch,
        45,
    );
    assert_eq!(identity(&parent), id);
    assert_eq!(identity(&File::open("/proc/self/ns/net").unwrap()), id);
    assert!(
        ok && std::str::from_utf8(&output)
            .unwrap()
            .lines()
            .filter(|s| *s == "K1_RECEIPT_KERNEL_PASS")
            .count()
            == 1,
        "isolated kernel/file fixture failed"
    );
}

#[test]
#[ignore = "internal isolated holder; parent namespace refuses"]
fn isolated_holder() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_RECEIPT_CHILD").as_deref(),
        Ok("1")
    );
    let guard = Guard::inherited();
    for cut in 0..=2 {
        let fixture = Fixture::new();
        let (target, sentinel) = names(std::process::id(), cut);
        create(&guard, &fixture, &sentinel);
        let sentinel_before = table(&guard, &fixture, &sentinel).unwrap();
        let mut child = ChildGuard(
            Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", WRITER])
                .env("OMAVLESS_K1_RECEIPT_FIXTURE", &fixture.0)
                .env("OMAVLESS_K1_RECEIPT_OWNER", std::process::id().to_string())
                .env("OMAVLESS_K1_RECEIPT_CUT", cut.to_string())
                .stdin(Stdio::from(guard.parent.try_clone().unwrap()))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while fs::read(fixture.0.join("ready")).ok().as_deref() != Some(b"checkpoint") {
            assert!(child.0.try_wait().unwrap().is_none(), "writer exited early");
            assert!(Instant::now() < deadline, "writer checkpoint timeout");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(fixture.root(), Err(StateError::Busy)));
        child.0.kill().unwrap();
        assert_eq!(child.0.wait().unwrap().signal(), Some(9));
        let before = fixture.bytes();
        let present = table(&guard, &fixture, &target);
        assert_eq!(present.is_some(), cut != 0);
        let mut reopened = fixture.state();
        assert_eq!(record(&reopened).state(), ReceiptState::PendingCreate);
        assert!(
            before.0.is_none(),
            "lost acknowledgement must not publish Armed"
        );
        let mut kernel = UntrustedKernel {
            guard: &guard,
            scratch: &fixture,
            target: target.clone(),
            cut: 0,
            effects: 0,
        };
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(
                reopened.request(request, guard.epoch(), &mut kernel),
                Err(REFUSED)
            );
        }
        assert_eq!(kernel.effects, 0);
        assert_eq!(fixture.bytes(), before);
        assert_eq!(table(&guard, &fixture, &target), present);
        assert_eq!(
            table(&guard, &fixture, &sentinel),
            Some(sentinel_before.clone())
        );
        drop(reopened);
        // Fixture cleanup, not orphan recovery: holder witnessed exclusive
        // creation, controls this empty namespace, and checks exact metadata.
        for (name, expected) in [(&target, present), (&sentinel, Some(sentinel_before))] {
            if let Some(expected) = expected {
                assert_eq!(table(&guard, &fixture, name), Some(expected));
                assert!(nft(&guard, &fixture, &["delete", "table", "inet", name]).0);
                assert!(table(&guard, &fixture, name).is_none());
            }
        }
    }
    guard.check();
    println!("K1_RECEIPT_KERNEL_PASS");
}

#[test]
#[ignore = "internal isolated writer; parent namespace refuses"]
fn isolated_writer() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_RECEIPT_CHILD").as_deref(),
        Ok("1")
    );
    let guard = Guard::inherited();
    let fixture = std::mem::ManuallyDrop::new(Fixture(PathBuf::from(
        std::env::var_os("OMAVLESS_K1_RECEIPT_FIXTURE").unwrap(),
    )));
    let owner = std::env::var("OMAVLESS_K1_RECEIPT_OWNER")
        .unwrap()
        .parse()
        .unwrap();
    let cut = std::env::var("OMAVLESS_K1_RECEIPT_CUT")
        .unwrap()
        .parse()
        .unwrap();
    let (target, _) = names(owner, cut);
    let mut state = fixture.state();
    let mut kernel = UntrustedKernel {
        guard: &guard,
        scratch: &fixture,
        target,
        cut,
        effects: 0,
    };
    assert_eq!(
        state.request_with(ARM, guard.epoch(), &mut kernel, |point| {
            if cut == 0 && point == Point::BeforeKernel {
                pause(&fixture);
            }
            Ok(())
        }),
        Err(REFUSED)
    );
    assert_eq!(cut, 2);
    pause(&fixture);
}

#[test]
fn fixture_names_are_bounded_and_distinct() {
    for cut in 0..=2 {
        let (target, sentinel) = names(u32::MAX, cut);
        assert_ne!(target, sentinel);
        assert!(target.len() < 64 && sentinel.len() < 64);
        assert!(
            target
                .bytes()
                .chain(sentinel.bytes())
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
    }
}

#[test]
fn fixture_guard_refuses_the_parent_namespace_before_any_tool() {
    let parent = File::open("/proc/self/ns/net").unwrap();
    let current = parent.try_clone().unwrap();
    let guard = Guard {
        id: identity(&parent),
        parent,
        current,
    };
    assert!(std::panic::catch_unwind(|| guard.check()).is_err());
}
