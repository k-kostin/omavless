//! Test-only Linux capability experiments in a disposable child namespace.
use super::*;

const FIXTURE: &str = include_str!("capability_fixture.py");
const PASS: &str = "K1_CAPABILITY_PASS";

fn accepted(success: bool, bytes: &[u8]) -> bool {
    success
        && std::str::from_utf8(bytes)
            .is_ok_and(|text| text.lines().filter(|line| *line == PASS).count() == 1)
}

fn stage(bytes: &[u8]) -> &'static str {
    let mut current = "launch";
    for line in std::str::from_utf8(bytes).unwrap_or("").lines() {
        for known in [
            "namespace",
            "socket",
            "sentinel",
            "exclusive",
            "generation",
            "replace",
            "stale_handle",
            "rollback",
            "cleanup",
            "finished",
        ] {
            if line.strip_prefix("K1_CAPABILITY_STAGE=") == Some(known) {
                current = known;
            }
        }
    }
    current
}

#[test]
fn capability_result_requires_one_success_and_filters_stages() {
    assert!(accepted(true, PASS.as_bytes()));
    assert!(!accepted(false, PASS.as_bytes()));
    assert!(!accepted(true, format!("{PASS}\n{PASS}").as_bytes()));
    assert!(!accepted(true, b"0 tests executed"));
    assert!(!accepted(true, b"\xff"));
    assert_eq!(
        stage(b"K1_CAPABILITY_STAGE=rollback\nprivate\n"),
        "rollback"
    );
    assert_eq!(stage(b"K1_CAPABILITY_STAGE=untrusted\n"), "launch");
}

#[test]
fn capability_wire_codec_rejects_malformed_frames_without_network() {
    let scratch = Scratch::new().unwrap();
    scratch
        .create("capability.py")
        .unwrap()
        .write_all(FIXTURE.as_bytes())
        .unwrap();
    let mut command = Command::new("/usr/bin/python3");
    command
        .env_clear()
        .args(["-I"])
        .arg(scratch.0.join("capability.py"))
        .arg("--self-test");
    let output = run(command, Stdio::null()).expect("developer Python unavailable");
    assert!(output.success && output.bytes == b"K1_CAPABILITY_CODEC_PASS\n");
    // Direct invocation without the outer pinned-FD/opt-in context refuses at
    // Guard construction, before opening either an IP or netlink socket.
    let mut direct = Command::new("/usr/bin/python3");
    direct
        .env_clear()
        .args(["-I"])
        .arg(scratch.0.join("capability.py"))
        .arg("--isolated");
    let refused = run(direct, Stdio::null()).unwrap();
    assert!(!refused.success);
    assert_eq!(
        refused.bytes,
        b"K1_CAPABILITY_STAGE=namespace\nK1_CAPABILITY_FAILED\n"
    );
}

#[test]
#[ignore = "explicit VM-only user+network namespace kernel capability checks"]
fn kernel_capabilities_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_CAPABILITY_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let scratch = Scratch::new().unwrap();
    scratch
        .create("capability.py")
        .unwrap()
        .write_all(FIXTURE.as_bytes())
        .unwrap();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
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
        .arg(scratch.0.join("capability.py"))
        .arg("--isolated");
    let result = run(command, Stdio::from(parent_fd.try_clone().unwrap()));
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let output = result.expect("isolated capability launch unavailable");
    // Fixed capability categories only; kernel data and raw errors never escape.
    for line in std::str::from_utf8(&output.bytes).unwrap_or("").lines() {
        if matches!(
            line,
            "K1_NS_GET_ID=available" | "K1_NS_GET_ID=unavailable" | "K1_SO_NETNS_COOKIE=available"
        ) {
            println!("{line}");
        }
    }
    assert!(
        accepted(output.success, &output.bytes),
        "isolated capability gate failed at stage: {}",
        stage(&output.bytes)
    );
    println!("{PASS}");
}
