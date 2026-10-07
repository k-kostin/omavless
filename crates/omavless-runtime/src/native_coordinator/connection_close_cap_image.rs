// SPDX-License-Identifier: MIT
// Separate ignored compatibility/refusal gate. Never weakens image admission.
const CAP_STATUS_LIMIT: usize = 16384;
#[derive(Clone, Copy, PartialEq, Eq)]
struct CapImageStatus {
    pid: u32,
    parent: u32,
    uids: [u32; 4],
    permitted: u64,
    effective: u64,
    no_new_privs: bool,
}

fn cap_status_decimal(raw: &str) -> Option<u32> {
    if raw.is_empty()
        || raw.len() > 10
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    raw.parse().ok()
}

fn parse_cap_image_status(raw: &[u8]) -> Option<CapImageStatus> {
    if raw.is_empty()
        || raw.len() > CAP_STATUS_LIMIT
        || !raw.ends_with(b"\n")
        || raw.iter().any(|b| !b.is_ascii() || *b == b'\r' || *b == 0)
    {
        return None;
    }
    let text = std::str::from_utf8(raw).ok()?;
    let mut fields = [None; 6];
    for line in text.split_terminator('\n') {
        let (key, value) = line.split_once(':')?;
        let index = match key {
            "Pid" => 0,
            "PPid" => 1,
            "Uid" => 2,
            "CapPrm" => 3,
            "CapEff" => 4,
            "NoNewPrivs" => 5,
            _ => continue,
        };
        if fields[index]
            .replace(value.trim_matches([' ', '\t']))
            .is_some()
        {
            return None;
        }
    }
    let uids: Vec<_> = fields[2]?
        .split_ascii_whitespace()
        .map(cap_status_decimal)
        .collect();
    if uids.len() != 4 {
        return None;
    }
    let hex = |s: &str| {
        (s.len() == 16
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| u64::from_str_radix(s, 16).ok())
        .flatten()
    };
    Some(CapImageStatus {
        pid: cap_status_decimal(fields[0]?)?,
        parent: cap_status_decimal(fields[1]?)?,
        uids: [uids[0]?, uids[1]?, uids[2]?, uids[3]?],
        permitted: hex(fields[3]?)?,
        effective: hex(fields[4]?)?,
        no_new_privs: match fields[5]? {
            "0" => false,
            "1" => true,
            _ => return None,
        },
    })
}

fn original_cap_image_status(pid: u32) -> Option<CapImageStatus> {
    use nix::fcntl::{OFlag, open, openat};
    use nix::sys::stat::{Mode, fstatat};
    // The PID comes only from the still-owned unreaped Child (or this parent),
    // not a name census, controller response or caller-controlled parameter.
    let path = PathBuf::from(format!("/proc/{pid}"));
    let directory = fs::File::from(
        open(
            &path,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .ok()?,
    );
    let before = directory.metadata().ok()?;
    let same_named = || {
        let named = fstatat(
            nix::fcntl::AT_FDCWD,
            &path,
            nix::fcntl::AtFlags::AT_SYMLINK_NOFOLLOW,
        )
        .ok()?;
        let held = directory.metadata().ok()?;
        Some(
            held.is_dir()
                && held.dev() == before.dev()
                && held.ino() == before.ino()
                && named.st_dev == held.dev()
                && named.st_ino == held.ino(),
        )
    };
    if same_named() != Some(true) {
        return None;
    }
    let file = fs::File::from(
        openat(
            &directory,
            "status",
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .ok()?,
    );
    let mut bytes = Vec::new();
    // Read the extra byte: never accept an unconsumed/overflowed prefix as
    // complete status evidence. Ordinary proc read semantics, not custody.
    file.take((CAP_STATUS_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    let result = parse_cap_image_status(&bytes)?;
    (result.pid == pid && same_named() == Some(true)).then_some(result)
}

#[test]
fn cap_image_status_parser_is_exact_unique_complete_and_bounded() {
    let valid = b"Name:\tmihomo\nPid:\t42\nPPid:\t1\nUid:\t1000\t1000\t1000\t1000\nCapPrm:\t0000000000003400\nCapEff:\t0000000000003400\nNoNewPrivs:\t0\n";
    let status = parse_cap_image_status(valid).unwrap();
    assert_eq!(status.pid, 42);
    assert_eq!(status.parent, 1);
    assert_eq!(status.uids, [1000; 4]);
    assert_eq!(status.permitted, 0x3400);
    assert_eq!(status.effective, 0x3400);
    assert!(!status.no_new_privs);
    let text = std::str::from_utf8(valid).unwrap();
    for key in ["Pid", "PPid", "Uid", "CapPrm", "CapEff", "NoNewPrivs"] {
        let mut missing = String::new();
        for line in text.lines().filter(|s| !s.starts_with(&format!("{key}:"))) {
            missing.push_str(line);
            missing.push('\n');
        }
        assert!(parse_cap_image_status(missing.as_bytes()).is_none());
        let duplicate = format!(
            "{text}{}\n",
            text.lines()
                .find(|s| s.starts_with(&format!("{key}:")))
                .unwrap()
        );
        assert!(parse_cap_image_status(duplicate.as_bytes()).is_none());
    }
    for (old, new) in [
        ("Pid:\t42", "Pid:\t042"),
        ("Pid:\t42", "Pid:\t4294967296"),
        ("Uid:\t1000\t1000\t1000\t1000", "Uid:\t1000\t1000\t1000"),
        (
            "Uid:\t1000\t1000\t1000\t1000",
            "Uid:\t1000\t1000\t1000\t1000\t1000",
        ),
        ("0000000000003400", "000000000000340G"),
        ("0000000000003400", "3400"),
        ("NoNewPrivs:\t0", "NoNewPrivs:\t2"),
    ] {
        assert!(parse_cap_image_status(text.replace(old, new).as_bytes()).is_none());
    }
    assert!(parse_cap_image_status(&valid[..valid.len() - 1]).is_none());
    assert!(parse_cap_image_status(text.replace('\n', "\r\n").as_bytes()).is_none());
    let mut overflow = valid.to_vec();
    overflow.extend(vec![b' '; CAP_STATUS_LIMIT]);
    overflow.push(b'\n');
    assert!(parse_cap_image_status(&overflow).is_none());
}

#[test]
#[ignore = "ROOT-reviewed fresh cap-enabled package-image namespace only"]
fn actual_owner_qualified_cap_image_refusal_in_dev_vm() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        std::env::var("OMAVLESS_CLOSE_CAP_IMAGE_VM").as_deref(),
        Ok("1")
    );
    assert_eq!(nix::unistd::getuid().as_raw(), 1000);
    assert_eq!(nix::unistd::getgid().as_raw(), 1000);
    assert_eq!(nix::unistd::getpid().as_raw(), 1);
    let _fixtures = FIXTURES.lock().unwrap();
    let parent = original_cap_image_status(1).expect("cap_image_parent_status_unavailable");
    assert_eq!(parent.uids, [1000; 4], "cap_image_parent_uid_refused");
    assert_eq!(parent.permitted, 0, "cap_image_parent_caps_refused");
    assert_eq!(parent.effective, 0, "cap_image_parent_caps_refused");
    assert!(!parent.no_new_privs, "cap_image_parent_nnp_refused");
    let executable = PathBuf::from(crate::managed_pair::RELEASE_CORE);
    let source_before = fs::symlink_metadata(&executable).unwrap();
    assert!(source_before.is_file() && !source_before.file_type().is_symlink());
    assert_eq!(source_before.uid(), 0);
    assert_eq!(source_before.nlink(), 1);
    assert_eq!(source_before.mode() & 0o7777, 0o755);
    assert!(source_before.len() > 0 && source_before.len() <= 128 * 1024 * 1024);
    let expected = std::env::var("OMAVLESS_CLOSE_QUALIFIED_CORE_SHA").unwrap();
    assert!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(fs::read(&executable).unwrap())),
        expected
    );
    let source_after = fs::symlink_metadata(&executable).unwrap();
    assert_eq!(
        (
            source_before.dev(),
            source_before.ino(),
            source_before.len(),
            source_before.ctime(),
            source_before.ctime_nsec()
        ),
        (
            source_after.dev(),
            source_after.ino(),
            source_after.len(),
            source_after.ctime(),
            source_after.ctime_nsec()
        )
    );
    let (root, owner) = fixture_parts("ok", None);
    let mut fixture = Fixture {
        root,
        owner: FixtureCoordinator(Some(owner)),
    };
    fixture.owner.invalidate_connection_close();
    fixture.owner.host_mut().stop_owned().unwrap();
    let runtime = fixture.root.join("r");
    let config = fixture.root.join("c");
    write(
        &config.join(crate::managed_pair::SELECTOR),
        crate::managed_pair::SELECTION_BYTES,
        0o600,
    );
    let socket = runtime.join("mihomo.sock");
    let paths = NativeHostPaths::new(
        executable.clone(),
        config.clone(),
        config.clone(),
        runtime.clone(),
        PathBuf::from("/proc"),
        PathBuf::from("/sys/class/net"),
    );
    *fixture.owner.host_mut() = NativeLifecycleHost::new(paths, fixture.owner.uid()).unwrap();
    write(&config.join("config.yaml"), format!("external-controller-unix: {}\nallow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\ntun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n", socket.display()).as_bytes(), 0o600);
    let mut core =
        OwnedCore::spawn(&executable, &runtime, &config.join("config.yaml"), &socket).unwrap();
    core.wait_ready(Duration::from_secs(10)).unwrap();
    eprintln!("cap_image_before_original_status");
    assert_eq!(core.running(), Ok(true), "cap_image_original_not_live");
    let pid = core.pid().expect("cap_image_original_pid_unavailable");
    let gained = original_cap_image_status(pid).expect("cap_image_original_status_unavailable");
    assert_eq!(core.running(), Ok(true), "cap_image_original_not_live");
    assert_eq!(core.pid(), Some(pid), "cap_image_original_changed");
    assert_eq!(gained.uids, [1000; 4], "cap_image_gain_uid_refused");
    assert_eq!(gained.parent, 1, "cap_image_gain_parent_refused");
    assert_eq!(gained.permitted, 0x3400, "cap_image_gain_caps_refused");
    assert_eq!(gained.effective, 0x3400, "cap_image_gain_caps_refused");
    assert!(!gained.no_new_privs, "cap_image_gain_nnp_refused");
    eprintln!("cap_image_original_gain_verified");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    fixture
        .owner
        .host_mut()
        .install_passive_owned_close_fixture(core)
        .unwrap();
    eprintln!("cap_image_before_one_adoption");
    let result = fixture
        .owner
        .transaction
        .lifecycle_mut()
        .adopt_owned_close_fixture();
    assert_eq!(
        result,
        Err(crate::lifecycle::HostStepError::Observation),
        "cap_image_adoption_was_not_refused"
    );
    eprintln!("cap_image_adoption_observation_refused");
    // No second adoption/proc-exe query/permit/close. Ordinary Fixture Drop and
    // namespace exit are NOT a recovery or UNKNOWN-resource custody receipt.
}
