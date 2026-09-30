//! Explicit VM-only integration harness. Ordinary workspace tests execute only
//! pure refusal/response tests. No command is ever run in the parent netns.
use omavless_netguard::kernel_observer::{LocalTablePresence, inspect_current_namespace};
use omavless_netguard::{nft, policy::Policy, transaction::Table};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::{
        fd::{AsFd, AsRawFd},
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PASS: &str = "K1_NFT_CHILD_PASS";
const LIMIT: u64 = 32768;
#[path = "support/capability.rs"]
mod capability;
#[path = "support/observer.rs"]
mod observer;
#[path = "support/owner.rs"]
mod owner;
#[path = "support/packet.rs"]
mod packet;
const DIAGNOSTIC: &str = "K1_NFT_SYNTHETIC_READBACK=";

// Only explicitly requested, bounded JSON from this harness's newly created
// fixture is forwarded. Never forward command stderr or arbitrary child output.
fn synthetic_diagnostic(enabled: bool, bytes: &[u8]) -> Option<String> {
    if !enabled || bytes.len() as u64 > LIMIT {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut candidates = text.lines().filter_map(|s| s.strip_prefix(DIAGNOSTIC));
    let candidate = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let value: Value = serde_json::from_str(candidate).ok()?;
    if value.as_object()?.len() != 1 || !value.get("nftables")?.is_array() {
        return None;
    }
    serde_json::to_string(&value).ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NamespaceIdentity {
    dev: u64,
    ino: u64,
}

fn namespace_label(label: &str, identity: NamespaceIdentity) -> bool {
    label == format!("net:[{}]", identity.ino)
}

fn fd_identity(fd: &File) -> Result<NamespaceIdentity, &'static str> {
    let metadata = fd.metadata().map_err(|_| "namespace_fstat_failed")?;
    let identity = NamespaceIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    };
    let label = fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd()))
        .map_err(|_| "namespace_kind_unavailable")?;
    if !label
        .to_str()
        .is_some_and(|label| namespace_label(label, identity))
    {
        return Err("not_network_namespace_fd");
    }
    Ok(identity)
}

fn only_loopback(dev: &str) -> bool {
    let mut lines = dev.lines();
    if !lines.next().is_some_and(|s| s.starts_with("Inter-|"))
        || !lines
            .next()
            .is_some_and(|s| s.trim_start().starts_with("face |"))
    {
        return false;
    }
    let entries: Vec<_> = lines.collect();
    if entries.len() != 1 {
        return false;
    }
    let Some((name, counters)) = entries[0].split_once(':') else {
        return false;
    };
    name.trim() == "lo"
        && counters.split_whitespace().count() == 16
        && counters
            .split_whitespace()
            .all(|n| n.parse::<u64>().is_ok())
}

fn isolated(
    current: NamespaceIdentity,
    parent: NamespaceIdentity,
    claimed_parent: NamespaceIdentity,
    dev: &str,
) -> bool {
    current.ino != 0
        && parent.ino != 0
        && current.dev == parent.dev
        && current != parent
        && parent == claimed_parent
        && only_loopback(dev)
}

fn bounded_read(path: &Path) -> Result<Vec<u8>, &'static str> {
    let mut file = File::open(path).map_err(|_| "read_failed")?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(&mut file, LIMIT + 1), &mut bytes)
        .map_err(|_| "read_failed")?;
    if bytes.len() as u64 > LIMIT {
        return Err("oversized_output");
    }
    Ok(bytes)
}

struct NamespaceGuard {
    parent_fd: File,
    claimed_parent: NamespaceIdentity,
}
impl NamespaceGuard {
    fn check_identity(&self) -> Result<NamespaceIdentity, &'static str> {
        let current =
            fd_identity(&File::open("/proc/self/ns/net").map_err(|_| "namespace_unavailable")?)?;
        let parent = fd_identity(&self.parent_fd)?;
        if current.ino == 0
            || parent.ino == 0
            || current.dev != parent.dev
            || current == parent
            || parent != self.claimed_parent
        {
            return Err("isolation_refused");
        }
        Ok(current)
    }
    fn check(&self) -> Result<u64, &'static str> {
        let current = self.check_identity()?;
        let dev = bounded_read(Path::new("/proc/net/dev"))?;
        if !only_loopback(std::str::from_utf8(&dev).map_err(|_| "invalid_interfaces")?) {
            return Err("isolation_refused");
        }
        Ok(current.ino)
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, &'static str> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "clock_failed")?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("omavless-k1-nft-{}-{nonce}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| "scratch_failed")?;
        Ok(Self(path))
    }
    fn create(&self, name: &str) -> Result<File, &'static str> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.0.join(name))
            .map_err(|_| "scratch_failed")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        for name in [
            "input.json",
            "stdout",
            "stderr",
            "packet.py",
            "capability.py",
            "owner.py",
        ] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

struct Output {
    success: bool,
    bytes: Vec<u8>,
}
fn run(mut command: Command, stdin: Stdio) -> Result<Output, &'static str> {
    let scratch = Scratch::new()?;
    command
        .stdin(stdin)
        .stdout(scratch.create("stdout")?)
        .stderr(scratch.create("stderr")?);
    let mut child = command.spawn().map_err(|_| "tool_unavailable")?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("wait_failed");
            }
        }
        if started.elapsed() > Duration::from_secs(15) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("tool_timeout");
        }
        // Bound retained file output too; the fixtures contain no secrets.
        if ["stdout", "stderr"]
            .iter()
            .any(|name| fs::metadata(scratch.0.join(name)).map_or(true, |m| m.len() > LIMIT))
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("oversized_output");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let bytes = bounded_read(&scratch.0.join("stdout"))?;
    bounded_read(&scratch.0.join("stderr"))?;
    Ok(Output {
        success: status.success(),
        bytes,
    })
}

fn nft_command(guard: &NamespaceGuard, args: &[&str]) -> Result<Output, &'static str> {
    guard.check()?;
    let mut command = Command::new("/usr/bin/nft");
    command.env_clear().env("LC_ALL", "C").args(args);
    run(command, Stdio::null())
}

fn child_result(success: bool, bytes: &[u8]) -> Result<(), &'static str> {
    let text = std::str::from_utf8(bytes).map_err(|_| "invalid_child_output")?;
    if success && text.lines().filter(|s| *s == PASS).count() == 1 {
        Ok(())
    } else {
        Err("child_failed_or_unavailable")
    }
}

fn last_stage(bytes: &[u8]) -> &'static str {
    let mut result = "launch";
    if let Ok(text) = std::str::from_utf8(bytes) {
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("K1_NFT_STAGE=") {
                for stage in [
                    "isolation",
                    "check",
                    "create",
                    "readback",
                    "classify",
                    "duplicate",
                    "cleanup",
                ] {
                    if value == stage {
                        result = stage;
                    }
                }
            }
        }
    }
    result
}

#[test]
#[ignore = "explicit disconnected child-network-namespace nft roundtrip inside disposable VM"]
fn nft_json_roundtrip_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_NFT_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").expect("pin parent namespace");
    let parent = fd_identity(&parent_fd).expect("parent namespace identity");
    let diagnostic = std::env::var("OMAVLESS_K1_NFT_DEBUG_SYNTHETIC").is_ok_and(|v| v == "1");
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .env("OMAVLESS_K1_NFT_CHILD", "1")
        .env("LC_ALL", "C")
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().expect("test executable"))
        .args(["--ignored", "--exact", "nft_roundtrip_child", "--nocapture"]);
    if diagnostic {
        command.env("OMAVLESS_K1_NFT_DEBUG_SYNTHETIC", "1");
    }
    let result = run(
        command,
        Stdio::from(parent_fd.try_clone().expect("parent namespace fd clone")),
    );
    assert_eq!(
        fd_identity(&parent_fd).expect("pinned parent identity"),
        parent
    );
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").expect("namespace observation"))
            .expect("current namespace identity"),
        parent
    );
    let result = result.expect("isolated child launch unavailable");
    if let Some(json) = synthetic_diagnostic(diagnostic, &result.bytes) {
        println!("{DIAGNOSTIC}{json}");
    }
    assert!(
        child_result(result.success, &result.bytes).is_ok(),
        "isolated nft gate failed at stage: {}",
        last_stage(&result.bytes)
    );
}

#[test]
#[ignore = "internal namespace child; direct invocation refuses"]
fn nft_roundtrip_child() {
    assert!(
        std::env::var("OMAVLESS_K1_NFT_CHILD").is_ok_and(|v| v == "1"),
        "child opt-in required"
    );
    let parent = NamespaceIdentity {
        dev: std::env::var("OMAVLESS_K1_NFT_PARENT_DEV")
            .ok()
            .and_then(|v| v.parse().ok())
            .expect("parent device required"),
        ino: std::env::var("OMAVLESS_K1_NFT_PARENT_INO")
            .ok()
            .and_then(|v| v.parse().ok())
            .expect("parent inode required"),
    };
    // stdin carries a pinned namespace descriptor, not data or a selectable path.
    // Duplicate it so subsequent nft children can independently have null stdin.
    let guard = NamespaceGuard {
        parent_fd: File::from(
            std::io::stdin()
                .as_fd()
                .try_clone_to_owned()
                .expect("inherited namespace fd"),
        ),
        claimed_parent: parent,
    };
    println!("K1_NFT_STAGE=isolation");
    let namespace = guard.check().expect("child isolation required");
    assert_eq!(inspect_current_namespace(), Ok(LocalTablePresence::Absent));
    // Test-only boot receipt. The namespace is freshly created by this harness;
    // production root receipt issuance/persistence is deliberately not exercised.
    for policy in [Policy::Emergency, Policy::FullVpn] {
        let scratch = Scratch::new().expect("private scratch");
        scratch
            .create("input.json")
            .unwrap()
            .write_all(&nft::render_create(policy))
            .unwrap();
        let input = scratch.0.join("input.json");
        let input = input.to_str().expect("fixed ASCII scratch");
        println!("K1_NFT_STAGE=check");
        assert!(
            nft_command(&guard, &["--json", "--check", "--file", input])
                .expect("nft check unavailable")
                .success,
            "nft check refused"
        );
        println!("K1_NFT_STAGE=create");
        assert!(
            nft_command(&guard, &["--json", "--file", input])
                .expect("nft create unavailable")
                .success,
            "nft create refused"
        );
        assert_eq!(guard.check().unwrap(), namespace);
        assert_eq!(
            inspect_current_namespace(),
            Ok(LocalTablePresence::PresentUntrusted),
            "a policy-shaped table is not observer ownership evidence"
        );
        println!("K1_NFT_STAGE=readback");
        let list = nft_command(
            &guard,
            &[
                "--json",
                "--handle",
                "--numeric",
                "--numeric-priority",
                "list",
                "table",
                "inet",
                "omavless_netguard",
            ],
        )
        .expect("nft readback unavailable");
        assert!(list.success, "nft readback failed");
        let value: Value = serde_json::from_slice(&list.bytes).expect("bounded synthetic JSON");
        let handle = value["nftables"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|v| v.get("table")?.get("handle")?.as_u64())
            .expect("table handle");
        let receipt = nft::TrustedTableIdentity {
            boot: [1; 16],
            netns_inode: namespace,
            table_handle: handle,
        };
        println!("K1_NFT_STAGE=classify");
        assert_eq!(
            nft::classify_readback(&list.bytes, [1; 16], namespace, None),
            Table::Foreign
        );
        let observed = nft::classify_readback(&list.bytes, [1; 16], namespace, Some(receipt));
        if observed != Table::OwnedVerified(policy)
            && std::env::var("OMAVLESS_K1_NFT_DEBUG_SYNTHETIC").is_ok_and(|v| v == "1")
        {
            // This dump is solely the fixed synthetic table created above in a
            // fresh loopback-only namespace, never a preexisting host ruleset.
            guard.check().expect("diagnostic isolation required");
            println!("{DIAGNOSTIC}{}", serde_json::to_string(&value).unwrap());
        }
        assert_eq!(
            observed,
            Table::OwnedVerified(policy),
            "installed readback differs from candidate; no protection claim"
        );
        println!("K1_NFT_STAGE=duplicate");
        assert!(
            !nft_command(&guard, &["--json", "--file", input])
                .expect("duplicate create unavailable")
                .success,
            "duplicate create unexpectedly accepted"
        );
        let after = nft_command(
            &guard,
            &[
                "--json",
                "--handle",
                "--numeric",
                "--numeric-priority",
                "list",
                "table",
                "inet",
                "omavless_netguard",
            ],
        )
        .unwrap();
        assert!(
            after.success && after.bytes == list.bytes,
            "duplicate create changed table"
        );
        println!("K1_NFT_STAGE=cleanup");
        // Test fixture cleanup only, after independent create and exact readback.
        assert!(
            nft_command(&guard, &["delete", "table", "inet", "omavless_netguard"])
                .unwrap()
                .success,
            "fixture cleanup failed"
        );
        assert_eq!(guard.check().unwrap(), namespace);
        assert_eq!(inspect_current_namespace(), Ok(LocalTablePresence::Absent));
    }
    println!("{PASS}");
}

const LO: &str = "Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n lo: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n";

#[test]
fn isolation_refuses_parent_namespace_extra_interfaces_and_bad_proc_facts() {
    let id = |ino| NamespaceIdentity { dev: 4, ino };
    assert!(isolated(id(20), id(10), id(10), LO));
    for (current, parent, claimed, dev) in [
        (10, 10, 10, LO),
        (20, 10, 11, LO),
        (20, 0, 0, LO),
        (0, 10, 10, LO),
        (20, 10, 10, ""),
    ] {
        assert!(!isolated(id(current), id(parent), id(claimed), dev));
    }
    assert!(!only_loopback(&format!(
        "{LO}eth0: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n"
    )));
    assert!(!only_loopback(&LO.replace("lo:", "eth0:")));
    assert!(!only_loopback(&LO.replace("0 0 0 0", "bad")));
    assert!(!isolated(
        id(20),
        NamespaceIdentity { dev: 5, ino: 10 },
        id(10),
        LO
    ));
    assert!(!isolated(
        id(20),
        id(10),
        NamespaceIdentity { dev: 5, ino: 10 },
        LO
    ));
    assert!(namespace_label("net:[10]", id(10)));
    for bad in [
        "user:[10]",
        "ipc:[10]",
        "net:[11]",
        "/tmp/net:[10]",
        "net:[10] (deleted)",
    ] {
        assert!(!namespace_label(bad, id(10)));
    }
}

#[test]
fn pinned_fd_identity_survives_clone_and_rejects_non_namespace_files() {
    let original = File::open("/proc/self/ns/net").unwrap();
    let expected = fd_identity(&original).unwrap();
    let clone = original.try_clone().unwrap();
    assert_eq!(fd_identity(&clone), Ok(expected));
    drop(original);
    assert_eq!(fd_identity(&clone), Ok(expected));
    let guard = NamespaceGuard {
        parent_fd: clone,
        claimed_parent: expected,
    };
    assert_eq!(guard.check(), Err("isolation_refused"));
    assert_eq!(
        fd_identity(&File::open("/dev/null").unwrap()),
        Err("not_network_namespace_fd")
    );
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/user").unwrap()),
        Err("not_network_namespace_fd")
    );
}

#[test]
fn failed_missing_or_duplicate_child_receipt_cannot_pass() {
    assert_eq!(
        child_result(true, format!("test output\n{PASS}\n").as_bytes()),
        Ok(())
    );
    assert!(child_result(false, PASS.as_bytes()).is_err());
    assert!(child_result(true, b"0 tests passed").is_err());
    assert!(child_result(true, format!("{PASS}\n{PASS}\n").as_bytes()).is_err());
    assert!(child_result(true, b"\xff").is_err());
    assert_eq!(
        last_stage(b"K1_NFT_STAGE=create\nK1_NFT_STAGE=private-data\n"),
        "create"
    );
    assert_eq!(last_stage(b"arbitrary stderr\n"), "launch");
}

#[test]
fn diagnostics_require_opt_in_and_one_bounded_json_record() {
    let record = format!("ignored\n{DIAGNOSTIC}{{\"nftables\":[]}}\n");
    assert_eq!(
        synthetic_diagnostic(true, record.as_bytes()),
        Some("{\"nftables\":[]}".into())
    );
    assert_eq!(synthetic_diagnostic(false, record.as_bytes()), None);
    for bad in [
        record.repeat(2),
        format!("{DIAGNOSTIC}not json"),
        format!("{DIAGNOSTIC}{{\"nftables\":null}}"),
        "stderr".into(),
        "x".repeat(LIMIT as usize + 1),
    ] {
        assert_eq!(synthetic_diagnostic(true, bad.as_bytes()), None);
    }
}

#[test]
fn scratch_files_are_private_exclusive_and_removed() {
    let scratch = Scratch::new().unwrap();
    let path = scratch.0.clone();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o700);
    let file = scratch.create("input.json").unwrap();
    assert_eq!(file.metadata().unwrap().mode() & 0o777, 0o600);
    assert!(scratch.create("input.json").is_err());
    drop(file);
    drop(scratch);
    assert!(!path.exists());
}
