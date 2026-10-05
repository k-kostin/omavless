//! Opt-in, empty-table socket lifetime experiment in a disposable VM namespace.
use super::*;

const FIXTURE: &str = include_str!("owner_fixture.py");
const CORE: &str = include_str!("capability_fixture.py");
const PASS: &str = "K1_OWNER_PASS";

fn fixture() -> Scratch {
    let scratch = Scratch::new().unwrap();
    for (name, contents) in [("owner.py", FIXTURE), ("capability.py", CORE)] {
        scratch
            .create(name)
            .unwrap()
            .write_all(contents.as_bytes())
            .unwrap();
    }
    scratch
}

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
            "sentinel",
            "owner_only",
            "owner_release",
            "persistent",
            "persistent_release",
            "adoption",
            "cleanup",
            "finished",
        ] {
            if line.strip_prefix("K1_OWNER_STAGE=") == Some(known) {
                current = known;
            }
        }
    }
    current
}

#[test]
fn owner_result_and_stage_output_are_strict_and_bounded() {
    assert!(accepted(true, PASS.as_bytes()));
    assert!(!accepted(false, PASS.as_bytes()));
    assert!(!accepted(true, b"no tests executed"));
    assert!(!accepted(true, format!("{PASS}\n{PASS}").as_bytes()));
    assert!(!accepted(true, b"\xff"));
    assert_eq!(
        stage(b"K1_OWNER_STAGE=adoption\nprivate-value\n"),
        "adoption"
    );
    assert_eq!(stage(b"K1_OWNER_STAGE=private-value\n"), "launch");
}

#[test]
fn owner_builders_and_direct_invocation_refuse_without_network() {
    let scratch = fixture();
    for (argument, expected, success) in [
        ("--self-test", "K1_OWNER_CODEC_PASS\n", true),
        (
            "--isolated",
            "K1_OWNER_STAGE=namespace\nK1_OWNER_FAILED\n",
            false,
        ),
    ] {
        let mut command = Command::new("/usr/bin/python3");
        command
            .env_clear()
            .arg("-I")
            .arg(scratch.0.join("owner.py"))
            .arg(argument);
        let output = run(command, Stdio::null()).unwrap();
        assert_eq!(output.success, success);
        assert_eq!(output.bytes, expected.as_bytes());
    }
}

#[test]
#[ignore = "explicit VM-only empty nft table owner/persist socket lifetime experiment"]
fn owner_lifetime_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_OWNER_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let scratch = fixture();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("OMAVLESS_K1_OWNER_CHILD", "1")
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
        .arg(scratch.0.join("owner.py"))
        .arg("--isolated");
    let result = run(command, Stdio::from(parent_fd.try_clone().unwrap()));
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let output = result.expect("isolated owner gate unavailable");
    assert!(
        accepted(output.success, &output.bytes),
        "isolated owner gate failed at stage: {}",
        stage(&output.bytes)
    );
    println!("{PASS}");
}
