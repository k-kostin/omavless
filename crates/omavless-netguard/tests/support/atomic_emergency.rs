// SPDX-License-Identifier: MIT
//! Developer-only bridge from the pure Rust encoder to isolated kernel evidence.
use super::*;

fn fixture() -> Scratch {
    let scratch = Scratch::new().unwrap();
    for (name, bytes) in [
        (
            "atomic_emergency.py",
            include_str!("atomic_emergency_fixture.py").as_bytes(),
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
            nft::render_create(Policy::Emergency).as_slice(),
        ),
    ] {
        scratch.create(name).unwrap().write_all(bytes).unwrap();
    }
    scratch
}

#[test]
#[ignore = "pure child encoder used only by explicit developer harness"]
fn encoder_child() {
    let number = |key| std::env::var(key).unwrap().parse::<u32>().unwrap();
    let value = omavless_netguard::emergency_wire::encode(
        number("K1_ATOMIC_GENERATION"),
        number("K1_ATOMIC_SEQUENCE"),
    )
    .unwrap();
    for (key, bytes) in [("BATCH", value.batch()), ("BARRIER", value.barrier())] {
        let hex: String = bytes.iter().map(|v| format!("{v:02x}")).collect();
        println!("K1_ATOMIC_{key}={hex}");
    }
}

fn command(scratch: &Scratch) -> Command {
    let mut cmd = Command::new("/usr/bin/python3");
    cmd.env_clear()
        .env("OMAVLESS_K1_ATOMIC_EXE", std::env::current_exe().unwrap())
        .arg("-I")
        .arg(scratch.0.join("atomic_emergency.py"));
    cmd
}

#[test]
fn atomic_emergency_pure_independent_codec_and_ack_refusal() {
    let scratch = fixture();
    let mut cmd = command(&scratch);
    cmd.arg("--self-test");
    let output = run(cmd, Stdio::null()).unwrap();
    assert!(output.success);
    assert_eq!(output.bytes, b"K1_ATOMIC_CODEC_PASS\n");
    let mut cmd = command(&scratch);
    cmd.arg("--isolated");
    let output = run(cmd, Stdio::null()).unwrap();
    assert!(!output.success);
    assert_eq!(
        output.bytes,
        b"K1_LIVE_OWNER_STAGE=namespace\nK1_ATOMIC_FAILED\n"
    );
}

#[test]
#[ignore = "VM-only atomic Emergency creation in disposable namespace"]
fn atomic_emergency_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_ATOMIC_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let scratch = fixture();
    let mut cmd = Command::new("/usr/bin/unshare");
    cmd.env_clear()
        .env("OMAVLESS_K1_ATOMIC_EXE", std::env::current_exe().unwrap())
        .env("OMAVLESS_K1_LIVE_OWNER_CHILD", "1")
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
        .arg(scratch.0.join("atomic_emergency.py"))
        .arg("--isolated");
    let result = run(cmd, Stdio::from(parent_fd.try_clone().unwrap()));
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let output = result.expect("isolated atomic gate unavailable");
    let safe_stages: Vec<_> = std::str::from_utf8(&output.bytes)
        .unwrap_or("")
        .lines()
        .filter(|line| {
            [
                "K1_LIVE_OWNER_STAGE=namespace",
                "K1_LIVE_OWNER_STAGE=rollback",
                "K1_LIVE_OWNER_STAGE=collision",
                "K1_LIVE_OWNER_STAGE=drift",
                "K1_LIVE_OWNER_STAGE=create",
                "K1_LIVE_OWNER_STAGE=readback",
                "K1_LIVE_OWNER_STAGE=foreign",
                "K1_LIVE_OWNER_STAGE=closed",
                "K1_LIVE_OWNER_STAGE=finished",
            ]
            .contains(line)
        })
        .collect();
    assert!(
        output.success && output.bytes.ends_with(b"K1_ATOMIC_PASS\n"),
        "atomic gate failed: {safe_stages:?}"
    );
    println!("K1_ATOMIC_PASS");
}
