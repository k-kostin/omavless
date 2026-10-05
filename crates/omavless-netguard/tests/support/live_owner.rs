//! Test-only complete Emergency-policy creator lifetime; no product authority.
use super::*;

const FIXTURE: &str = include_str!("live_owner_fixture.py");
const CORE: &str = include_str!("capability_fixture.py");
const PASS: &str = "K1_LIVE_OWNER_PASS";

fn fixture() -> Scratch {
    let scratch = Scratch::new().unwrap();
    for (name, bytes) in [
        ("live_owner.py", FIXTURE.as_bytes()),
        ("capability.py", CORE.as_bytes()),
        (
            "expected.json",
            nft::render_create(Policy::Emergency).as_slice(),
        ),
    ] {
        scratch.create(name).unwrap().write_all(bytes).unwrap();
    }
    scratch
}

fn stage(bytes: &[u8]) -> &'static str {
    let mut last = "launch";
    for line in std::str::from_utf8(bytes).unwrap_or("").lines() {
        for known in [
            "namespace",
            "collision",
            "create",
            "readback",
            "foreign",
            "drift",
            "closed",
            "finished",
        ] {
            if line.strip_prefix("K1_LIVE_OWNER_STAGE=") == Some(known) {
                last = known;
            }
        }
    }
    last
}

#[test]
fn live_owner_pure_codec_and_direct_invocation_refusal() {
    let scratch = fixture();
    for (arg, success, expected) in [
        ("--self-test", true, "K1_LIVE_OWNER_CODEC_PASS\n"),
        (
            "--isolated",
            false,
            "K1_LIVE_OWNER_STAGE=namespace\nK1_LIVE_OWNER_FAILED\n",
        ),
    ] {
        let mut command = Command::new("/usr/bin/python3");
        command
            .env_clear()
            .arg("-I")
            .arg(scratch.0.join("live_owner.py"))
            .arg(arg);
        let output = run(command, Stdio::null()).unwrap();
        assert_eq!(output.success, success);
        assert_eq!(output.bytes, expected.as_bytes());
    }
    assert_eq!(stage(b"K1_LIVE_OWNER_STAGE=private\n"), "launch");
}

#[test]
#[ignore = "VM-only live owner,persist Emergency policy in disposable namespace"]
fn live_emergency_owner_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_LIVE_OWNER_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let scratch = fixture();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
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
        .arg(scratch.0.join("live_owner.py"))
        .arg("--isolated");
    let result = run(command, Stdio::from(parent_fd.try_clone().unwrap()));
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let output = result.expect("isolated live owner gate unavailable");
    assert!(
        output.success
            && std::str::from_utf8(&output.bytes).is_ok_and(|s| s
                .lines()
                .filter(|v| *v == PASS)
                .count()
                == 1),
        "live owner gate failed at stage: {}",
        stage(&output.bytes)
    );
    println!("{PASS}");
}
