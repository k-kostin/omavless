// SPDX-License-Identifier: MIT
// Test-only real primary CLI and original-child completion under fixed Foot.
const REAL_CLI_PATH: &str = "/home/kdk_vm/.cache/t3-live-ui-r1/omavless";
const REAL_CLI_SIZE: u64 = 8571248;
const REAL_CLI_SHA: &str = "75263631e03f8b99bc8d1dd511901148cb7798fa5ad35b71797e9efdbed1a304";
const CLI_ZERO: &[u8] = b"T3_REAL_CLI_ORIGINAL_EXIT_ZERO\n";
const CLI_ZERO_LEAF: &str = "cli-original-zero";
const REAL_UI_PARENT: &str = "/home/kdk_vm/.cache/t3-live-ui-r1/tmp";

#[test]
#[ignore = "ROOT-reviewed private actual CLI/Foot in disposable VM namespace only"]
fn actual_owner_developer_pair_real_cli_foot_in_dev_vm() {
    assert!(std::env::var("OMAVLESS_CLOSE_DEVELOPER_REAL_UI_VM").as_deref() == Ok("1"));
    assert!(nix::unistd::getuid().as_raw() == 1000 && nix::unistd::getgid().as_raw() == 1000);
    assert_eq!(nix::unistd::getpid().as_raw(), 1);
    composed_core_selected_close_with_client(
        PathBuf::from("/var/lib/omavless-close-development-pair/mihomo"),
        true,
        false,
        true,
        false,
        true,
    );
}

fn wait_original_ui(child: &mut std::process::Child, until: Instant) -> Result<(), ()> {
    wait_ui_with(
        || {
            child
                .try_wait()
                .map(|s| s.map(|s| s.success()))
                .map_err(|_| ())
        },
        Instant::now,
        || std::thread::sleep(Duration::from_millis(20)),
        until,
    )
}
// Same sampled deadline around each original wait result; no new child lookup,
// signal, timeout kill or wait after refusal. A blocking backend is not preempted.
fn wait_ui_with(
    mut poll: impl FnMut() -> Result<Option<bool>, ()>,
    mut now: impl FnMut() -> Instant,
    mut pause: impl FnMut(),
    until: Instant,
) -> Result<(), ()> {
    loop {
        if now() >= until {
            return Err(());
        }
        let result = poll()?;
        if now() >= until {
            return Err(());
        }
        match result {
            Some(true) => return Ok(()),
            Some(false) => return Err(()),
            None => pause(),
        }
    }
}
fn real_ui_location(path: &Path) -> bool {
    path.parent() == Some(Path::new(REAL_UI_PARENT))
        && path.file_name().and_then(|s| s.to_str()).is_some_and(|s| {
            let Some(tail) = s.strip_prefix("ovt-owner-close-") else {
                return false;
            };
            let Some((pid, nonce)) = tail.split_once('-') else {
                return false;
            };
            !pid.is_empty()
                && pid.len() <= 10
                && pid.bytes().all(|b| b.is_ascii_digit())
                && pid.parse::<u32>().is_ok_and(|pid| pid > 0)
                && !nonce.is_empty()
                && nonce.len() <= 16
                && nonce
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}
fn cli_zero_bytes(bytes: &[u8]) -> bool {
    bytes == CLI_ZERO
}
fn real_ui_root(path: &Path) -> Result<(), ()> {
    if !real_ui_location(path) {
        return Err(());
    }
    let m = fs::symlink_metadata(path).map_err(|_| ())?;
    if !m.is_dir()
        || m.file_type().is_symlink()
        || (m.uid(), m.gid()) != (1000, 1000)
        || m.mode() & 0o7777 != 0o700
    {
        return Err(());
    }
    Ok(())
}
fn public_cli() -> Result<std::fs::File, ()> {
    use sha2::Digest;
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
        .open(REAL_CLI_PATH)
        .map_err(|_| ())?;
    let before = file.metadata().map_err(|_| ())?;
    if !before.is_file()
        || (
            before.uid(),
            before.gid(),
            before.nlink(),
            before.len(),
            before.mode() & 0o7777,
        ) != (1000, 1000, 1, REAL_CLI_SIZE, 0o500)
    {
        return Err(());
    }
    let mut data = Vec::new();
    Read::by_ref(&mut file)
        .take(REAL_CLI_SIZE + 1)
        .read_to_end(&mut data)
        .map_err(|_| ())?;
    if data.len() as u64 != REAL_CLI_SIZE
        || format!("{:x}", sha2::Sha256::digest(&data)) != REAL_CLI_SHA
    {
        return Err(());
    }
    let same = |a: &fs::Metadata, b: &fs::Metadata| {
        crate::restore_staging_candidate::same_member(a, b) && a.gid() == b.gid()
    };
    if !same(&before, &file.metadata().map_err(|_| ())?)
        || !same(
            &before,
            &fs::symlink_metadata(REAL_CLI_PATH).map_err(|_| ())?,
        )
    {
        return Err(());
    }
    Ok(file) // retained through original child wait; trusted admin immutable name
}

#[test]
#[ignore = "fixed Foot TTY child supervisor only, ordinary original CLI wait"]
fn real_cli_private_tty_child_supervisor() {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut original_child = None;
    let mut retained_binary = None;
    let mut retained_marker = None;
    let result = (|| -> Result<(), ()> {
        if nix::unistd::getuid().as_raw() != 1000 || nix::unistd::getgid().as_raw() != 1000 {
            return Err(());
        }
        let root = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").map_err(|_| ())?);
        real_ui_root(&root)?;
        let expected_ns = std::env::var("T3_UI_PID_NAMESPACE").map_err(|_| ())?;
        let actual_ns = fs::metadata("/proc/self/ns/pid").map_err(|_| ())?;
        if expected_ns != format!("{}:{}", actual_ns.dev(), actual_ns.ino()) {
            return Err(());
        }
        let locale = std::env::var("OMAVLESS_LOCALE").map_err(|_| ())?;
        if !["en", "ru"].contains(&locale.as_str()) {
            return Err(());
        }
        retained_binary = Some(public_cli()?);
        let admitted_binary = retained_binary
            .as_ref()
            .ok_or(())?
            .metadata()
            .map_err(|_| ())?;
        original_child = Some(
            std::process::Command::new(REAL_CLI_PATH)
                .args(["tui", "--developer-conditional-close"])
                .env_clear()
                .env("HOME", "/home/kdk_vm")
                .env("PATH", "/usr/bin:/bin")
                .env("LC_ALL", "C")
                .env("TERM", "xterm-256color")
                .env("OMAVLESS_LOCALE", locale)
                .env("XDG_RUNTIME_DIR", &root)
                .spawn()
                .map_err(|_| ())?,
        );
        // Original actual CLI child only. No kill, resend or restart on failure.
        wait_original_ui(
            original_child.as_mut().ok_or(())?,
            Instant::now() + Duration::from_secs(900),
        )?;
        let after_binary = retained_binary
            .as_ref()
            .ok_or(())?
            .metadata()
            .map_err(|_| ())?;
        let named_binary = fs::symlink_metadata(REAL_CLI_PATH).map_err(|_| ())?;
        if !crate::restore_staging_candidate::same_member(&admitted_binary, &after_binary)
            || !crate::restore_staging_candidate::same_member(&admitted_binary, &named_binary)
            || admitted_binary.gid() != after_binary.gid()
            || admitted_binary.gid() != named_binary.gid()
        {
            return Err(());
        }
        real_ui_root(&root)?;
        let until = Instant::now() + Duration::from_secs(5);
        let tick = || {
            if Instant::now() < until {
                Ok(())
            } else {
                Err(())
            }
        };
        let path = root.join(CLI_ZERO_LEAF);
        tick()?;
        retained_marker = Some(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
                .open(&path)
                .map_err(|_| ())?,
        );
        tick()?;
        let marker = retained_marker.as_mut().ok_or(())?;
        marker
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| ())?;
        tick()?;
        marker.write_all(CLI_ZERO).map_err(|_| ())?;
        tick()?;
        marker.sync_all().map_err(|_| ())?;
        tick()?;
        let m = marker.metadata().map_err(|_| ())?;
        tick()?;
        let named = fs::symlink_metadata(&path).map_err(|_| ())?;
        tick()?;
        if !m.is_file()
            || (m.uid(), m.gid(), m.nlink(), m.len(), m.mode() & 0o7777)
                != (1000, 1000, 1, CLI_ZERO.len() as u64, 0o600)
            || !crate::restore_staging_candidate::same_member(&m, &named)
            || m.gid() != named.gid()
        {
            return Err(());
        }
        tick()?;
        Ok(())
    })();
    if result.is_err() {
        eprintln!("T3_REAL_CLI_COMPLETION_UNAVAILABLE");
        // Keep the original child and reported handles. Exiting this helper on
        // failure could cause ordinary Foot PTY teardown to affect a live CLI.
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
}

fn exercise_real_cli_foot(
    fixture: &SocketFixture,
    clients: &mut [std::net::TcpStream],
    targets: [u16; 2],
    original_foot: &mut Option<std::process::Child>,
    completion_file: &mut Option<std::fs::File>,
) -> Result<(), ()> {
    use std::io::{Read, Write};
    if targets != [19180, 19181] || clients.len() != 2 {
        return Err(());
    }
    real_ui_root(&fixture.root)?;
    let before = fixture.desired_bytes();
    let locale = std::env::var("OMAVLESS_LOCALE").map_err(|_| ())?;
    if !["en", "ru"].contains(&locale.as_str()) {
        return Err(());
    }
    let namespace = fs::metadata("/proc/self/ns/pid").map_err(|_| ())?;
    let image = std::env::current_exe().map_err(|_| ())?;
    *original_foot = Some(std::process::Command::new("/usr/bin/foot")
        .args([
            "--config=/dev/null",
            "--log-level=none",
            "--app-id=OmaVLESS-T3-Live",
            "--title=OmaVLESS-T3-Live",
        ])
        .arg("/usr/bin/env")
        .arg(format!("XDG_RUNTIME_DIR={}", fixture.root.display()))
        .arg(format!(
            "T3_UI_PID_NAMESPACE={}:{}",
            namespace.dev(),
            namespace.ino()
        ))
        .arg(format!("OMAVLESS_LOCALE={locale}"))
        .arg(&image)
        .args([
            "--exact",
            "native_coordinator::connection_close::tests::real_cli_private_tty_child_supervisor",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("HOME", "/home/kdk_vm")
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("XDG_RUNTIME_DIR", "/run/user/1000")
        .env("WAYLAND_DISPLAY", "wayland-1")
        .spawn()
        .map_err(|_| ())?);
    wait_original_ui(
        original_foot.as_mut().ok_or(())?,
        Instant::now() + Duration::from_secs(900),
    )?;
    // Both prerequisites, never Foot0 alone: original Foot wait0 (ordinary
    // trusted Foot propagates the helper's exit status) AND the original CLI
    // wait0 marker. Helper late/failed I/O parks, so no original Foot wait0
    // authorizes an incomplete physical marker left by that refusal.
    let path = fixture.root.join(CLI_ZERO_LEAF);
    let until = Instant::now() + Duration::from_secs(5);
    let tick = || {
        if Instant::now() < until {
            Ok(())
        } else {
            Err(())
        }
    };
    tick()?;
    *completion_file = Some(
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| ())?,
    );
    tick()?;
    let file = completion_file.as_mut().ok_or(())?;
    let before_marker = file.metadata().map_err(|_| ())?;
    tick()?;
    if !before_marker.is_file()
        || (
            before_marker.uid(),
            before_marker.gid(),
            before_marker.nlink(),
            before_marker.len(),
            before_marker.mode() & 0o7777,
        ) != (1000, 1000, 1, CLI_ZERO.len() as u64, 0o600)
    {
        return Err(());
    }
    let mut bytes = Vec::new();
    Read::by_ref(file)
        .take(CLI_ZERO.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    tick()?;
    let held_after = file.metadata().map_err(|_| ())?;
    tick()?;
    let named_after = fs::symlink_metadata(&path).map_err(|_| ())?;
    tick()?;
    if !cli_zero_bytes(&bytes)
        || !crate::restore_staging_candidate::same_member(&before_marker, &held_after)
        || before_marker.gid() != held_after.gid()
        || !crate::restore_staging_candidate::same_member(&before_marker, &named_after)
        || before_marker.gid() != named_after.gid()
    {
        return Err(());
    }
    if fixture.desired_bytes() != before {
        return Err(());
    }
    clients[0].write_all(b"alive").map_err(|_| ())?;
    let mut alive = [0; 5];
    clients[0].read_exact(&mut alive).map_err(|_| ())?;
    if alive != *b"alive" {
        return Err(());
    }
    let mut byte = [0; 1];
    match clients[1].read(&mut byte) {
        Ok(0) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
            ) =>
        {
            Ok(())
        }
        _ => Err(()),
    }
}

#[test]
fn real_ui_fixed_artifact_binding() {
    assert_eq!(
        (REAL_CLI_SIZE, REAL_CLI_SHA),
        (
            8571248,
            "75263631e03f8b99bc8d1dd511901148cb7798fa5ad35b71797e9efdbed1a304"
        )
    );
    assert_eq!(CLI_ZERO.len(), 31);
}

#[test]
fn real_ui_private_binding_and_receipt_are_exact_not_arbitrary_runtime_or_text() {
    let parent = Path::new(REAL_UI_PARENT);
    assert!(real_ui_location(&parent.join("ovt-owner-close-1-a")));
    for leaf in [
        "ovt-owner-close-",
        "ovt-owner-close-0-a",
        "ovt-owner-close-1-A",
        "ovt-owner-close-1-a-more",
        "ovt-owner-close-1-",
        "ovt-owner-close-1-a/child",
        "ovt-owner-close-1-../other",
        "ovt-owner-close-1-\n",
    ] {
        assert!(!real_ui_location(&parent.join(leaf)));
    }
    assert!(!real_ui_location(Path::new(
        "/run/user/1000/ovt-owner-close-1-a"
    )));
    assert!(!real_ui_location(Path::new("ovt-owner-close-1-a")));
    assert!(cli_zero_bytes(CLI_ZERO));
    for bytes in [
        b"".as_slice(),
        b"T3_REAL_CLI_ORIGINAL_EXIT_ZERO".as_slice(),
        b"T3_REAL_CLI_ORIGINAL_EXIT_ZERO\n\n".as_slice(),
        b"T3_REAL_CLI_ORIGINAL_EXIT_NONZERO\n".as_slice(),
    ] {
        assert!(!cli_zero_bytes(bytes));
    }
}

#[test]
fn real_ui_original_wait_refuses_nonzero_timeout_late_and_error_without_retry() {
    use std::cell::Cell;
    let start = Instant::now();
    let until = start + Duration::from_secs(1);
    for outcome in [Ok(Some(false)), Err(())] {
        let calls = Cell::new(0);
        assert!(
            wait_ui_with(
                || {
                    calls.set(calls.get() + 1);
                    outcome
                },
                || start,
                || panic!("no pause after refusal"),
                until
            )
            .is_err()
        );
        assert_eq!(calls.get(), 1);
    }
    assert!(
        wait_ui_with(
            || panic!("expired before wait"),
            || until,
            || panic!("no pause"),
            until
        )
        .is_err()
    );
    let late = Cell::new(false);
    assert!(
        wait_ui_with(
            || {
                late.set(true);
                Ok(Some(true))
            },
            || if late.get() { until } else { start },
            || panic!("no pause"),
            until
        )
        .is_err()
    );
    assert!(wait_ui_with(|| Ok(Some(true)), || start, || panic!("no pause"), until).is_ok());
    let calls = Cell::new(0);
    assert!(
        wait_ui_with(
            || {
                calls.set(calls.get() + 1);
                Ok(if calls.get() == 1 { None } else { Some(true) })
            },
            || start,
            || (),
            until
        )
        .is_ok()
    );
    assert_eq!(calls.get(), 2);
}
