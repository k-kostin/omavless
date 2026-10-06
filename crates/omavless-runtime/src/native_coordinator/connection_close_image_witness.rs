// SPDX-License-Identifier: MIT
// Passive ignored gate only. This does not inject helper facts into Session.
#[test]
#[ignore = "ROOT-reviewed isolated original cap-child image helper namespace only"]
fn actual_original_cap_child_image_witness_in_dev_vm() {
    use omavless_image_witness::{Client, Error};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek};
    assert_eq!(
        std::env::var("OMAVLESS_IMAGE_WITNESS_VM").as_deref(),
        Ok("1")
    );
    assert_eq!(nix::unistd::getuid().as_raw(), 1000);
    assert_eq!(nix::unistd::getgid().as_raw(), 1000);
    // ROOT's namespace-init supervisor retains BOTH original helper/test
    // children and their exit identities; the test is not an exec-away reaper.
    assert_eq!(nix::unistd::getppid().as_raw(), 1);
    assert!(nix::unistd::getpid().as_raw() > 1);
    let own_pid = u32::try_from(nix::unistd::getpid().as_raw()).unwrap();
    let case = std::env::var("OMAVLESS_IMAGE_WITNESS_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "positive" | "wrong-parent" | "dead" | "source-drift"
    ));
    let _fixtures = FIXTURES.lock().unwrap();
    let parent = original_cap_image_status(own_pid).expect("image_witness_parent_unavailable");
    assert_eq!(parent.uids, [1000; 4]);
    assert_eq!(parent.permitted, 0);
    assert_eq!(parent.effective, 0);
    assert!(!parent.no_new_privs);
    // Root stages this exact test image at the helper's sole developer-client
    // path. No installed runtime or caller-selected executable is adopted.
    assert_eq!(
        std::env::current_exe().unwrap(),
        Path::new("/usr/lib/omavless-image/development-runtime-tests")
    );
    let executable = PathBuf::from(crate::managed_pair::RELEASE_CORE);
    let expected = std::env::var("OMAVLESS_CLOSE_QUALIFIED_CORE_SHA").unwrap();
    assert!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let source = fs::File::open(&executable).unwrap();
    let identity = source.metadata().unwrap();
    assert!(identity.is_file());
    assert_eq!(identity.uid(), 0);
    assert_eq!(identity.nlink(), 1);
    assert_eq!(identity.mode() & 0o7777, 0o755);
    let root = crate::test_temp::directory("original-image-witness").unwrap();
    let socket = root.join("mihomo.sock");
    let config = root.join("config.yaml");
    write(&config,format!("external-controller-unix: {}\nallow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\nprofile:\n  store-selected: false\n  store-fake-ip: false\ntun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n",socket.display()).as_bytes(),0o600);
    let mut core = OwnedCore::spawn(&executable, &root, &config, &socket).unwrap();
    core.wait_ready(Duration::from_secs(10)).unwrap();
    assert_eq!(core.running(), Ok(true));
    let pid = core.pid().unwrap();
    let status = original_cap_image_status(pid).expect("image_witness_child_status_unavailable");
    assert_eq!(status.uids, [1000; 4]);
    assert_eq!(status.parent, own_pid);
    assert_eq!(status.permitted, 0x3400);
    assert_eq!(status.effective, 0x3400);
    assert!(!status.no_new_privs);
    assert_eq!(core.running(), Ok(true));
    eprintln!("image_witness_original_cap_child_verified");
    let target = if case == "wrong-parent" { own_pid } else { pid };
    let process = rustix::process::Pid::from_raw(i32::try_from(target).unwrap()).unwrap();
    let pidfd =
        rustix::process::pidfd_open(process, rustix::process::PidfdFlags::NONBLOCK).unwrap();
    assert_eq!(core.running(), Ok(true));
    if case == "dead" {
        core.stop(Duration::from_secs(2)).unwrap();
    }
    let bound = Client::bind_original(pidfd, Instant::now() + Duration::from_secs(2));
    if case != "positive" {
        assert!(matches!(
            bound,
            Err(Error::Refused | Error::Unavailable | Error::ChannelLost)
        ));
        eprintln!("image_witness_negative_original_binding_refused");
        if case == "source-drift" {
            // ROOT alone replaced the fixed source with a same-byte/cap
            // different inode after ORIGINAL helper source capture, before
            // this live original child's launch. No native mutation or retry.
            assert_eq!(core.running(), Ok(true));
            assert_eq!(core.pid(), Some(pid));
            eprintln!("image_witness_original_source_drift_refused");
        }
        if case != "dead" {
            core.stop(Duration::from_secs(2)).unwrap();
        }
        return;
    }
    let mut client = bound.expect("image_witness_original_binding_unavailable");
    let until = Instant::now() + Duration::from_secs(5);
    for _ in 0..3 {
        assert!(Instant::now() < until);
        assert_eq!(core.running(), Ok(true));
        let mut image = client
            .observe(until)
            .expect("image_witness_current_image_unavailable");
        let m = image.metadata().unwrap();
        let current = source.metadata().unwrap();
        let named = fs::symlink_metadata(&executable).unwrap();
        let full = |m: &fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.uid(),
                m.gid(),
                m.mode(),
                m.nlink(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        assert_eq!(full(&m), full(&identity));
        assert_eq!(full(&current), full(&identity));
        assert_eq!(full(&named), full(&identity));
        image.rewind().unwrap();
        let mut hash = Sha256::new();
        let mut bytes = [0; 65536];
        let mut count = 0;
        loop {
            assert!(Instant::now() < until);
            let n = image.read(&mut bytes).unwrap();
            if n == 0 {
                break;
            }
            count += n as u64;
            assert!(count <= identity.len());
            hash.update(&bytes[..n]);
        }
        assert_eq!(count, identity.len());
        assert_eq!(format!("{:x}", hash.finalize()), expected);
        assert_eq!(core.running(), Ok(true));
        assert_eq!(core.pid(), Some(pid));
    }
    eprintln!("image_witness_three_current_images_matched");
    client
        .finish(until)
        .expect("image_witness_positive_finish_unavailable");
    assert!(client.observe(until).is_err());
    core.stop(Duration::from_secs(2)).unwrap();
    eprintln!("image_witness_original_positive_completed");
    // No adoption/permit/controller POST or desired-state mutation. Ordinary
    // OwnedCore stop/Drop and namespace teardown are not UNKNOWN cleanup proof.
}
