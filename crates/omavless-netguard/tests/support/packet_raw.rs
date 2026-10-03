// SPDX-License-Identifier: MIT
//! Separate raw FullVpn matrix; the original JSON policy gate is unchanged.
use super::*;

const RAW_CHILD: &str = "packet::raw::raw_packet_child";

fn scratch() -> Scratch {
    let scratch = Scratch::new().unwrap();
    for (name, data) in [
        (
            "raw_packet.py",
            include_str!("packet_raw_fixture.py").as_bytes(),
        ),
        ("packet.py", FIXTURE.as_bytes()),
        (
            "atomic_full.py",
            include_str!("atomic_full_fixture.py").as_bytes(),
        ),
        (
            "live_owner.py",
            include_str!("live_owner_fixture.py").as_bytes(),
        ),
        (
            "capability.py",
            include_str!("capability_fixture.py").as_bytes(),
        ),
        (
            "expected.json",
            nft::render_create(Policy::FullVpn).as_slice(),
        ),
    ] {
        scratch.create(name).unwrap().write_all(data).unwrap();
    }
    scratch
}

#[test]
fn raw_worker_direct_invocation_and_guard_mutations_refuse() {
    let scratch = scratch();
    for (arg, success, expected) in [
        (
            "--self-test",
            true,
            b"K1_RAW_PACKET_CODEC_PASS\n".as_slice(),
        ),
        ("--isolated", false, b"K1_RAW_PACKET_FAILED\n".as_slice()),
    ] {
        let mut cmd = Command::new("/usr/bin/python3");
        cmd.env_clear()
            .arg("-I")
            .arg(scratch.0.join("raw_packet.py"))
            .arg(arg);
        let result = run(cmd, Stdio::null()).unwrap();
        assert_eq!(result.success, success);
        assert_eq!(result.bytes, expected);
    }
}

#[test]
#[ignore = "VM-only raw FullVpn53vectors in disposable veth namespace"]
fn raw_full_packet_enforcement_in_disposable_vm() {
    assert!(std::env::var("OMAVLESS_K1_RAW_PACKET_VM").is_ok_and(|v| v == "1"));
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .env("OMAVLESS_K1_RAW_PACKET_CHILD", "1")
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", RAW_CHILD, "--nocapture"]);
    let result = run_bounded(
        command,
        Stdio::from(parent_fd.try_clone().unwrap()),
        Duration::from_secs(45),
    );
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let result = result.expect("raw packet child unavailable");
    let text = std::str::from_utf8(&result.bytes).unwrap_or("");
    let stages: Vec<_> = text
        .lines()
        .filter(|line| {
            [
                "K1_RAW_PACKET_STAGE=baseline",
                "K1_RAW_PACKET_STAGE=full",
                "K1_RAW_PACKET_STAGE=interface",
            ]
            .contains(line)
        })
        .collect();
    assert!(
        result.success && text.lines().filter(|l| *l == "K1_RAW_PACKET_PASS").count() == 1,
        "raw packet gate failed at {stages:?}, case {:?}",
        last_case(&result.bytes)
    );
    println!("K1_RAW_PACKET_PASS");
}

#[test]
#[ignore = "internal raw packet child; direct invocation refuses"]
fn raw_packet_child() {
    assert!(std::env::var("OMAVLESS_K1_RAW_PACKET_CHILD").is_ok_and(|v| v == "1"));
    let base = NamespaceGuard {
        parent_fd: File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap()),
        claimed_parent: NamespaceIdentity {
            dev: std::env::var("OMAVLESS_K1_NFT_PARENT_DEV")
                .unwrap()
                .parse()
                .unwrap(),
            ino: std::env::var("OMAVLESS_K1_NFT_PARENT_INO")
                .unwrap()
                .parse()
                .unwrap(),
        },
    };
    base.check().expect("fresh loopback-only child required");
    let child = base.check_identity().unwrap();
    assert!(
        ip(
            &base,
            &["link", "add", OUT, "type", "veth", "peer", "name", PEER]
        )
        .success
    );
    let links = ip(&base, &["-j", "-d", "link", "show"]);
    assert!(links.success);
    let indexes = links_valid(&serde_json::from_slice(&links.bytes).unwrap(), OUT, None).unwrap();
    let mut guard = PacketGuard {
        base,
        child,
        indexes,
        output: OUT,
    };
    // Identical closed topology to the retained JSON fixture; no uplink/default.
    for args in [
        vec!["link", "set", "lo", "up"],
        vec!["link", "set", OUT, "address", "02:00:00:00:00:01"],
        vec!["link", "set", PEER, "address", "02:00:00:00:00:02"],
        vec!["addr", "add", "192.0.2.1/24", "dev", OUT],
        vec!["-6", "addr", "add", "2001:db8::1/64", "dev", OUT, "nodad"],
        vec!["-6", "addr", "add", "fe80::1/64", "dev", OUT, "nodad"],
        vec!["link", "set", OUT, "addrgenmode", "none"],
        vec!["link", "set", PEER, "addrgenmode", "none"],
        vec!["link", "set", OUT, "up"],
        vec!["link", "set", PEER, "up"],
        vec!["route", "add", "10.0.0.2/32", "dev", OUT],
        vec!["-6", "route", "add", "fd00::2/128", "dev", OUT],
    ] {
        assert!(guard.command("/usr/bin/ip", &args).success);
    }
    for address in ["192.0.2.2", "10.0.0.2", "2001:db8::2", "fd00::2", "fe80::2"] {
        assert!(
            guard
                .command(
                    "/usr/bin/ip",
                    &[
                        "neigh",
                        "replace",
                        address,
                        "lladdr",
                        "02:00:00:00:00:02",
                        "nud",
                        "permanent",
                        "dev",
                        OUT
                    ]
                )
                .success
        );
    }
    guard.check();
    // Use the already reviewed fixed foreign accept fixture in this raw-wire
    // test too: an earlier accept and a later accept must not bypass the
    // candidate's priority-300 drop. Both tables stay inside this child netns.
    guard.install_foreign_accepts();
    let foreign_before = guard.foreign_snapshot();
    let scratch = scratch();
    let mut command = Command::new("/usr/bin/python3");
    command
        .env_clear()
        .env("OMAVLESS_K1_RAW_PACKET_CHILD", "1")
        .env("OMAVLESS_K1_FULL_EXE", std::env::current_exe().unwrap())
        .env("K1_PARENT_DEV", guard.base.claimed_parent.dev.to_string())
        .env("K1_PARENT_INO", guard.base.claimed_parent.ino.to_string())
        .env("K1_CHILD_INO", guard.child.ino.to_string())
        .env("K1_OUT_INDEX", guard.indexes.0.to_string())
        .env("K1_PEER_INDEX", guard.indexes.1.to_string())
        .arg("-I")
        .arg(scratch.0.join("raw_packet.py"))
        .arg("--isolated");
    let result = run_bounded(
        command,
        Stdio::from(guard.base.parent_fd.try_clone().unwrap()),
        Duration::from_secs(30),
    )
    .unwrap();
    for line in std::str::from_utf8(&result.bytes).unwrap_or("").lines() {
        if [
            "K1_RAW_PACKET_STAGE=baseline",
            "K1_RAW_PACKET_STAGE=full",
            "K1_RAW_PACKET_STAGE=interface",
            "K1_RAW_PACKET_PASS",
        ]
        .contains(&line)
        {
            println!("{line}");
        }
    }
    if let Some(case) = last_case(&result.bytes) {
        println!("K1_PACKET_CASE={case}");
    }
    assert!(result.success, "guarded raw packet worker failed");
    // The worker renamed the fixed output at the end of its matrix.
    // Rebind the topology guard before any further child-namespace readback.
    guard.output = nft::TUN;
    guard.check();
    let foreign_after = guard.foreign_snapshot();
    assert_eq!(foreign_after, foreign_before);
    assert!(
        guard
            .command("/usr/bin/nft", &["delete", "table", "inet", FOREIGN])
            .success
    );
    assert!(!guard.foreign_snapshot_exists());
    // Worker closed its creator only after the full matrix. No target delete,
    // acquisition or effect after socket loss; child namespace exit reclaims it.
}
