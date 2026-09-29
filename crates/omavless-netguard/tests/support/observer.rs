//! Read-only Rust observer opt-in: no nft executable, table creation or mutation.
use super::*;
use omavless_netguard::kernel_observer::{LocalTablePresence, inspect_current_namespace};
const PASS: &str = "K1_OBSERVER_PASS";

#[test]
#[ignore = "explicit VM-only read-only observer in fresh user/network namespace"]
fn read_only_observer_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_OBSERVER_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_OBSERVER_CHILD", "1")
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "observer::read_only_observer_child",
            "--nocapture",
        ]);
    let output = run(command, Stdio::from(parent_fd.try_clone().unwrap())).unwrap();
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    assert!(
        output.success
            && std::str::from_utf8(&output.bytes).is_ok_and(|s| s
                .lines()
                .filter(|v| *v == PASS)
                .count()
                == 1),
        "isolated observer refused or failed"
    );
    println!("{PASS}");
}

#[test]
#[ignore = "internal namespace child; direct invocation refuses before observer"]
fn read_only_observer_child() {
    assert!(
        std::env::var("OMAVLESS_K1_OBSERVER_CHILD").is_ok_and(|v| v == "1"),
        "child opt-in required"
    );
    let parent = NamespaceIdentity {
        dev: std::env::var("OMAVLESS_K1_NFT_PARENT_DEV")
            .unwrap()
            .parse()
            .unwrap(),
        ino: std::env::var("OMAVLESS_K1_NFT_PARENT_INO")
            .unwrap()
            .parse()
            .unwrap(),
    };
    let guard = NamespaceGuard {
        parent_fd: File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap()),
        claimed_parent: parent,
    };
    let child = guard.check().expect("isolated loopback namespace required");
    assert_eq!(inspect_current_namespace(), Ok(LocalTablePresence::Absent));
    assert_eq!(inspect_current_namespace(), Ok(LocalTablePresence::Absent));
    assert_eq!(guard.check().unwrap(), child);
    println!("{PASS}");
}

#[test]
fn observer_child_direct_invocation_refuses() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .args(["--ignored", "--exact", "observer::read_only_observer_child"]);
    let output = run(command, Stdio::null()).unwrap();
    assert!(!output.success);
    assert!(
        !std::str::from_utf8(&output.bytes)
            .unwrap_or("")
            .lines()
            .any(|line| line == PASS)
    );
}
