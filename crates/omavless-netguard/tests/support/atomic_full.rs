// SPDX-License-Identifier: MIT
//! Opt-in FullVpn wire validation in a new loopback-only VM namespace.
use super::*;
use std::io::Read;

#[test]
#[ignore = "pure raw-reply decoder child used only by explicit developer harness"]
fn decoder_child() {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(32 * 1024 + 16 * 16 + 17)
        .read_to_end(&mut bytes)
        .unwrap();
    let mut rest = bytes.as_slice();
    let next = |rest: &mut &[u8]| -> u32 {
        let (value, tail) = rest.split_at(4);
        *rest = tail;
        u32::from_ne_bytes(value.try_into().unwrap())
    };
    let generation = next(&mut rest);
    let first = next(&mut rest);
    let port = next(&mut rest);
    let count = next(&mut rest);
    assert!((1..=16).contains(&count));
    let mut collector =
        omavless_netguard::full_vpn_wire::reply::FullVpnTranscript::new(generation, first, port)
            .unwrap();
    for _ in 0..count {
        let length = usize::try_from(next(&mut rest)).unwrap();
        let sender_port = next(&mut rest);
        let sender_groups = next(&mut rest);
        let flags = next(&mut rest);
        let (data, tail) = rest.split_at(length);
        rest = tail;
        collector
            .push_datagram(data, sender_port, sender_groups, flags)
            .unwrap();
    }
    assert!(rest.is_empty());
    assert!(collector.finish().is_ok());
    println!("K1_FULL_RAW_REPLY_PASS");
}

fn fixture() -> Scratch {
    let scratch = Scratch::new().unwrap();
    for (name, bytes) in [
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
        scratch.create(name).unwrap().write_all(bytes).unwrap();
    }
    scratch
}

#[test]
#[ignore = "pure encoder child for opt-in FullVpn fixture"]
fn encoder_child() {
    let number = |key| std::env::var(key).unwrap().parse::<u32>().unwrap();
    let wire = omavless_netguard::full_vpn_wire::encode(
        number("K1_FULL_GENERATION"),
        number("K1_FULL_SEQUENCE"),
    )
    .unwrap();
    for (name, data) in [("BATCH", wire.batch()), ("BARRIER", wire.barrier())] {
        let hex: String = data.iter().map(|v| format!("{v:02x}")).collect();
        println!("K1_FULL_{name}={hex}");
    }
}

#[test]
fn pure_reply_checks_and_unisolated_invocation_refuse() {
    let scratch = fixture();
    for (arg, success, expected) in [
        ("--self-test", true, b"K1_FULL_CODEC_PASS\n".as_slice()),
        (
            "--isolated",
            false,
            b"K1_FULL_STAGE=namespace\nK1_FULL_FAILED\n".as_slice(),
        ),
    ] {
        let mut cmd = Command::new("/usr/bin/python3");
        cmd.env_clear()
            .arg("-I")
            .arg(scratch.0.join("atomic_full.py"))
            .arg(arg);
        let output = run(cmd, Stdio::null()).unwrap();
        assert_eq!(output.success, success);
        assert_eq!(output.bytes, expected);
    }
}

#[test]
#[ignore = "VM-only FullVpn atomic bytes, readback, rollback and loopback packets"]
fn atomic_full_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_FULL_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let scratch = fixture();
    let mut cmd = Command::new("/usr/bin/unshare");
    cmd.env_clear()
        .env("OMAVLESS_K1_FULL_EXE", std::env::current_exe().unwrap())
        .env("OMAVLESS_K1_FULL_CHILD", "1")
        .env("OMAVLESS_K1_FULL_RAW_REPLY_VM", "1")
        .env("OMAVLESS_K1_CAPABILITY_CHILD", "1")
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .args([
            "--user",
            "--map-root-user",
            "--net",
            "--",
            "/usr/bin/python3",
            "-I",
        ])
        .arg(scratch.0.join("atomic_full.py"))
        .arg("--isolated");
    let result = run(cmd, Stdio::from(parent_fd.try_clone().unwrap()));
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let output = result.expect("FullVpn isolated gate unavailable");
    let stages: Vec<_> = std::str::from_utf8(&output.bytes)
        .unwrap_or("")
        .lines()
        .filter(|line| {
            [
                "K1_FULL_STAGE=namespace",
                "K1_FULL_STAGE=rollback",
                "K1_FULL_STAGE=create",
                "K1_FULL_STAGE=readback",
                "K1_FULL_STAGE=packets",
                "K1_FULL_STAGE=closed",
            ]
            .contains(line)
        })
        .collect();
    // This opt-in diagnostic can only be produced by this newly created fixture;
    // ordinary test runs never forward a host firewall dump.
    if let Some(readback) = synthetic_diagnostic(
        std::env::var("OMAVLESS_K1_FULL_DIAGNOSTIC").is_ok_and(|v| v == "1"),
        &output.bytes,
    ) {
        println!("{DIAGNOSTIC}{readback}");
    }
    assert!(
        output.success && output.bytes.ends_with(b"K1_FULL_PASS\n"),
        "FullVpn gate failed: {stages:?}"
    );
    assert_eq!(
        output
            .bytes
            .split(|b| *b == b'\n')
            .filter(|line| *line == b"K1_FULL_RAW_REPLY_PASS")
            .count(),
        1,
        "raw FullVpn transcript was not checked exactly once"
    );
    println!("K1_FULL_RAW_REPLY_PASS");
    println!("K1_FULL_PASS");
}
