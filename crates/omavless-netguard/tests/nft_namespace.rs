//! Explicit VM-only integration harness. Ordinary workspace tests execute only
//! pure refusal/response tests. No command is ever run in the parent netns.
use omavless_netguard::{nft, policy::Policy, transaction::Table};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PASS: &str = "K1_NFT_CHILD_PASS";
const LIMIT: u64 = 32768;

fn parent_pid(status: &str) -> Option<u32> {
    let mut values = status.lines().filter_map(|s| s.strip_prefix("PPid:"));
    let value = values.next()?.trim().parse::<u32>().ok()?;
    (value > 1 && values.next().is_none()).then_some(value)
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

fn isolated(current: u64, parent: u64, claimed_parent: u64, dev: &str) -> bool {
    current != 0
        && parent != 0
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

fn namespace_guard(claimed_parent: u64) -> Result<u64, &'static str> {
    let current = fs::metadata("/proc/self/ns/net")
        .map_err(|_| "namespace_unavailable")?
        .ino();
    let status = bounded_read(Path::new("/proc/self/status"))?;
    let pid = parent_pid(std::str::from_utf8(&status).map_err(|_| "invalid_parent")?)
        .ok_or("invalid_parent")?;
    let parent = fs::metadata(format!("/proc/{pid}/ns/net"))
        .map_err(|_| "parent_namespace_unavailable")?
        .ino();
    let dev = bounded_read(Path::new("/proc/net/dev"))?;
    if !isolated(
        current,
        parent,
        claimed_parent,
        std::str::from_utf8(&dev).map_err(|_| "invalid_interfaces")?,
    ) {
        return Err("isolation_refused");
    }
    Ok(current)
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, &'static str> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "clock_failed")?
            .as_nanos();
        let path = PathBuf::from(format!(
            "/tmp/omavless-k1-nft-{}-{nonce}",
            std::process::id()
        ));
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
        for name in ["input.json", "stdout", "stderr"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

struct Output {
    success: bool,
    bytes: Vec<u8>,
}
fn run(mut command: Command) -> Result<Output, &'static str> {
    let scratch = Scratch::new()?;
    command
        .stdin(Stdio::null())
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

fn nft_command(parent: u64, args: &[&str]) -> Result<Output, &'static str> {
    namespace_guard(parent)?;
    let mut command = Command::new("/usr/bin/nft");
    command.env_clear().env("LC_ALL", "C").args(args);
    run(command)
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
    let parent = fs::metadata("/proc/self/ns/net")
        .expect("namespace observation")
        .ino();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_NFT_PARENT", parent.to_string())
        .env("OMAVLESS_K1_NFT_CHILD", "1")
        .env("LC_ALL", "C")
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().expect("test executable"))
        .args(["--ignored", "--exact", "nft_roundtrip_child", "--nocapture"]);
    let result = run(command).expect("isolated child launch unavailable");
    assert_eq!(
        fs::metadata("/proc/self/ns/net")
            .expect("namespace observation")
            .ino(),
        parent
    );
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
    let parent = std::env::var("OMAVLESS_K1_NFT_PARENT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .expect("parent identity required");
    println!("K1_NFT_STAGE=isolation");
    let namespace = namespace_guard(parent).expect("child isolation required");
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
            nft_command(parent, &["--json", "--check", "--file", input])
                .expect("nft check unavailable")
                .success,
            "nft check refused"
        );
        println!("K1_NFT_STAGE=create");
        assert!(
            nft_command(parent, &["--json", "--file", input])
                .expect("nft create unavailable")
                .success,
            "nft create refused"
        );
        println!("K1_NFT_STAGE=readback");
        let list = nft_command(
            parent,
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
        assert_eq!(
            nft::classify_readback(&list.bytes, [1; 16], namespace, Some(receipt)),
            Table::OwnedVerified(policy),
            "installed readback differs from candidate; no protection claim"
        );
        println!("K1_NFT_STAGE=duplicate");
        assert!(
            !nft_command(parent, &["--json", "--file", input])
                .expect("duplicate create unavailable")
                .success,
            "duplicate create unexpectedly accepted"
        );
        let after = nft_command(
            parent,
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
            nft_command(parent, &["delete", "table", "inet", "omavless_netguard"])
                .unwrap()
                .success,
            "fixture cleanup failed"
        );
    }
    println!("{PASS}");
}

const LO: &str = "Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n lo: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n";

#[test]
fn isolation_refuses_parent_namespace_extra_interfaces_and_bad_proc_facts() {
    assert!(isolated(20, 10, 10, LO));
    for (current, parent, claimed, dev) in [
        (10, 10, 10, LO),
        (20, 10, 11, LO),
        (20, 0, 0, LO),
        (0, 10, 10, LO),
        (20, 10, 10, ""),
    ] {
        assert!(!isolated(current, parent, claimed, dev));
    }
    assert!(!only_loopback(&format!(
        "{LO}eth0: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n"
    )));
    assert!(!only_loopback(&LO.replace("lo:", "eth0:")));
    assert!(!only_loopback(&LO.replace("0 0 0 0", "bad")));
    assert_eq!(parent_pid("Name:\ttest\nPPid:\t42\n"), Some(42));
    for bad in ["PPid: 1", "PPid: 0", "PPid: 42\nPPid: 43", "PPid: +x"] {
        assert_eq!(parent_pid(bad), None);
    }
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
