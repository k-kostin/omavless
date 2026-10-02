//! Read-only Rust observer opt-in: no nft executable, table creation or mutation.
use super::*;
use omavless_netguard::kernel_observer::{
    LocalChainInventory, LocalReadSession, LocalTablePresence, inspect_current_namespace,
};
const PASS: &str = "K1_OBSERVER_PASS";
const PRESENT_PASS: &str = "K1_OBSERVER_PRESENT_PASS";
const CHAIN_PASS: &str = "K1_OBSERVER_CHAIN_PASS";

fn last_chain_stage(bytes: &[u8]) -> &'static str {
    let mut stage = "launch";
    if let Ok(text) = std::str::from_utf8(bytes) {
        for line in text.lines() {
            stage = match line.strip_prefix("K1_CHAIN_STAGE=") {
                Some("isolated") => "isolated",
                Some("absent") => "absent",
                Some("table") => "table",
                Some("added") => "added",
                Some("other") => "other",
                Some("error") => "error",
                Some("expected") => "expected",
                Some("foreign") => "foreign",
                Some("foreign_added") => "foreign_added",
                Some("foreign_refused") => "foreign_refused",
                Some("foreign_removed") => "foreign_removed",
                Some("extra") => "extra",
                Some("priority") => "priority",
                Some("hook") => "hook",
                Some("policy") => "policy",
                Some("removed") => "removed",
                _ => stage,
            };
        }
    }
    stage
}

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
    let mut retained = LocalReadSession::open().unwrap();
    assert_eq!(retained.inspect(), Ok(LocalTablePresence::Absent));
    assert_eq!(retained.inspect(), Ok(LocalTablePresence::Absent));
    assert_eq!(guard.check().unwrap(), child);
    // The retained socket must not be usable after this very thread moves to
    // another disposable network namespace. No return to the old namespace is
    // attempted; the isolated test process exits immediately afterwards.
    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET).unwrap();
    assert_ne!(
        fd_identity(&File::open("/proc/thread-self/ns/net").unwrap())
            .unwrap()
            .ino,
        child
    );
    assert!(retained.inspect().is_err());
    println!("{PASS}");
}

#[test]
#[ignore = "explicit VM-only foreign empty table in fresh user/network namespace"]
fn present_untrusted_observer_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_OBSERVER_PRESENT_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_OBSERVER_PRESENT_CHILD", "1")
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "observer::present_untrusted_observer_child",
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
                .filter(|v| *v == PRESENT_PASS)
                .count()
                == 1),
        "isolated present-table observer refused or failed"
    );
    println!("{PRESENT_PASS}");
}

#[test]
#[ignore = "internal namespace child; direct invocation refuses before nft"]
fn present_untrusted_observer_child() {
    assert!(
        std::env::var("OMAVLESS_K1_OBSERVER_PRESENT_CHILD").is_ok_and(|v| v == "1"),
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
    let mut retained = LocalReadSession::open().unwrap();
    assert_eq!(retained.inspect(), Ok(LocalTablePresence::Absent));
    assert!(
        nft_command(&guard, &["add", "table", "inet", "omavless_netguard"])
            .unwrap()
            .success
    );
    assert_eq!(guard.check().unwrap(), child);
    assert_eq!(retained.inspect(), Ok(LocalTablePresence::PresentUntrusted));
    assert_eq!(retained.inspect(), Ok(LocalTablePresence::PresentUntrusted));
    assert!(
        nft_command(&guard, &["delete", "table", "inet", "omavless_netguard"])
            .unwrap()
            .success
    );
    assert_eq!(retained.inspect(), Ok(LocalTablePresence::Absent));
    assert_eq!(guard.check().unwrap(), child);
    println!("{PRESENT_PASS}");
}

#[test]
#[ignore = "explicit VM-only chain inventory in fresh user/network namespace"]
fn chain_inventory_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_CHAIN_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_CHAIN_CHILD", "1")
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "observer::chain_inventory_child",
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
                .filter(|v| *v == CHAIN_PASS)
                .count()
                == 1),
        "isolated chain inventory refused or failed after {}",
        last_chain_stage(&output.bytes)
    );
    println!("{CHAIN_PASS}");
}

#[test]
#[ignore = "internal namespace child; direct invocation refuses before nft"]
fn chain_inventory_child() {
    assert!(
        std::env::var("OMAVLESS_K1_CHAIN_CHILD").is_ok_and(|v| v == "1"),
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
    println!("K1_CHAIN_STAGE=isolated");
    let mut retained = LocalReadSession::open().unwrap();
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::TableAbsent)
    );
    println!("K1_CHAIN_STAGE=absent");
    let nft_ok = |args: &[&str]| assert!(nft_command(&guard, args).unwrap().success);
    nft_ok(&["add", "table", "inet", "omavless_netguard"]);
    assert_eq!(retained.inspect_chains(), Ok(LocalChainInventory::Empty));
    println!("K1_CHAIN_STAGE=table");
    nft_ok(&[
        "add",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
        "{ type filter hook output priority 300 ; policy drop ; }",
    ]);
    println!("K1_CHAIN_STAGE=added");
    let observed = retained.inspect_chains();
    println!(
        "K1_CHAIN_STAGE={}",
        match observed {
            Ok(LocalChainInventory::OtherUntrusted) => "other",
            Err(_) => "error",
            _ => "added",
        }
    );
    assert_eq!(
        observed,
        Ok(LocalChainInventory::ExpectedOutputChainUntrusted)
    );
    println!("K1_CHAIN_STAGE=expected");
    nft_ok(&["add", "table", "inet", "unrelated_synthetic"]);
    nft_ok(&["add", "chain", "inet", "unrelated_synthetic", "foreign"]);
    println!("K1_CHAIN_STAGE=foreign_added");
    assert!(retained.inspect_chains().is_err());
    println!("K1_CHAIN_STAGE=foreign_refused");
    assert!(retained.inspect().is_err());
    nft_ok(&["delete", "table", "inet", "unrelated_synthetic"]);
    println!("K1_CHAIN_STAGE=foreign_removed");
    assert!(retained.inspect_chains().is_err());
    retained = LocalReadSession::open().unwrap();
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::ExpectedOutputChainUntrusted)
    );
    println!("K1_CHAIN_STAGE=foreign");
    nft_ok(&["add", "chain", "inet", "omavless_netguard", "extra"]);
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::OtherUntrusted)
    );
    println!("K1_CHAIN_STAGE=extra");
    nft_ok(&["delete", "chain", "inet", "omavless_netguard", "extra"]);
    nft_ok(&[
        "delete",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
    ]);
    nft_ok(&[
        "add",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
        "{ type filter hook output priority 299 ; policy drop ; }",
    ]);
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::OtherUntrusted)
    );
    println!("K1_CHAIN_STAGE=priority");
    nft_ok(&[
        "delete",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
    ]);
    nft_ok(&[
        "add",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
        "{ type filter hook input priority 300 ; policy drop ; }",
    ]);
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::OtherUntrusted)
    );
    println!("K1_CHAIN_STAGE=hook");
    nft_ok(&[
        "delete",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
    ]);
    nft_ok(&[
        "add",
        "chain",
        "inet",
        "omavless_netguard",
        "output_guard",
        "{ type filter hook output priority 300 ; policy accept ; }",
    ]);
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::OtherUntrusted)
    );
    println!("K1_CHAIN_STAGE=policy");
    nft_ok(&["delete", "table", "inet", "omavless_netguard"]);
    assert_eq!(
        retained.inspect_chains(),
        Ok(LocalChainInventory::TableAbsent)
    );
    println!("K1_CHAIN_STAGE=removed");
    assert_eq!(guard.check().unwrap(), child);
    println!("{CHAIN_PASS}");
}

#[test]
fn observer_child_direct_invocation_refuses() {
    for (child, pass) in [
        ("observer::read_only_observer_child", PASS),
        ("observer::present_untrusted_observer_child", PRESENT_PASS),
        ("observer::chain_inventory_child", CHAIN_PASS),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.env_clear().args(["--ignored", "--exact", child]);
        let output = run(command, Stdio::null()).unwrap();
        assert!(!output.success);
        assert!(
            !std::str::from_utf8(&output.bytes)
                .unwrap_or("")
                .lines()
                .any(|line| line == pass)
        );
    }
}
