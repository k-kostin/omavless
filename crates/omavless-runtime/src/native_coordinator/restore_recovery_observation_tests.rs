// SPDX-License-Identifier: MIT
//! Real fresh-observation/retained-origin seam on inert proc/sys/config roots.
//! No manager query, process launch, network effect or completion authority.
use super::*;
use crate::restore_successor_publication_candidate::tests::Fixture;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn with_origin(test: impl FnOnce(&mut NativeRecoveryOrigin<'_>, &Fixture, &NativeHostPaths)) {
    let f = Fixture::new();
    for name in ["data", "proc", "sys", "daemon"] {
        fs::create_dir(f.root.join(name)).unwrap();
        fs::set_permissions(f.root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    // Executable identity is required by the observer constructor, but this
    // sealed host exposes no lifecycle execution and never launches this file.
    private(&f.root.join("core"), b"#!/bin/sh\nexit 91\n");
    fs::set_permissions(f.root.join("core"), fs::Permissions::from_mode(0o700)).unwrap();
    private(
        &f.config.join("route-template.yaml"),
        b"tun:\n  enable: true\n  device: Meta\n",
    );
    let desired_paths = DesiredPaths::below(&f.root.join("state"));
    crate::desired::write_desired(&desired_paths, f.uid, &DesiredState::default()).unwrap();
    private(&f.paths.runtime_base.join("omavless-login.receipt"),
        br#"{"schemaVersion":1,"epochHash":"0000000000000000000000000000000000000000000000000000000000000000","ownershipGeneration":2,"phase":"consumed"}"#);
    let lock = f.lock();
    let make_paths = || {
        NativeHostPaths::new(
            f.root.join("core"),
            f.root.join("data"),
            f.config.clone(),
            f.root.join("daemon"),
            f.root.join("proc"),
            f.root.join("sys"),
        )
    };
    let host_paths = make_paths();
    let mut host = ObservationOnlyNativeHost::new(make_paths(), f.uid).unwrap();
    let mut boundary = None;
    Boundary::capture_native_paths(&f.paths, &host_paths.store, f.uid, &mut boundary).unwrap();
    let boundary = boundary.unwrap();
    let bytes = |index: usize, limit| {
        held_bytes(
            boundary.members[index]
                .held
                .as_ref()
                .map(|(f, _)| f)
                .unwrap(),
            limit,
        )
        .unwrap()
    };
    let mut origin = NativeRecoveryOrigin {
        lock: &lock,
        boundary: &boundary,
        host: &mut host,
        paths: &f.paths,
        desired_paths: &desired_paths,
        marker: read_marker_existing(&f.paths, f.uid).unwrap(),
        desired: read_desired_snapshot(&desired_paths, f.uid).unwrap(),
        marker_bytes: bytes(0, 1024),
        desired_bytes: bytes(1, 65536),
        login_bytes: Some(bytes(2, 1024)),
        uid: f.uid,
    };
    test(&mut origin, &f, &host_paths);
}

#[test]
fn recovery_origin_fresh_off_keeps_foreign_tun_and_retained_bindings() {
    for foreign in [false, true] {
        with_origin(|origin, f, paths| {
            let tun = paths.sys_class_net.join("foreign0");
            if foreign {
                fs::create_dir(&tun).unwrap();
                fs::write(tun.join("tun_flags"), b"0x1001\n").unwrap();
            }
            let marker = fs::read(&f.paths.ownership_marker).unwrap();
            let desired = fs::read(&origin.desired_paths.file).unwrap();
            let login = fs::read(f.paths.runtime_base.join("omavless-login.receipt")).unwrap();
            let store = fs::read(&paths.store).unwrap();
            assert_eq!(
                origin.bindings(false),
                Ok(()),
                "retained bindings, foreign={foreign}"
            );
            let observed = origin.host.fresh_observation(&origin.desired).unwrap();
            assert_eq!(observed.visible_tun_count, u8::from(foreign));
            assert_eq!(observed.managed_tun_count, 0);
            assert_eq!(
                origin.check_old_recovery_lease(),
                Ok(()),
                "foreign={foreign}"
            );
            assert_eq!(origin.check_observed_off(), Ok(()));
            assert!(origin.lock.authorizes(&f.paths, f.uid));
            assert_eq!(fs::read(&f.paths.ownership_marker).unwrap(), marker);
            assert_eq!(fs::read(&origin.desired_paths.file).unwrap(), desired);
            assert_eq!(
                fs::read(f.paths.runtime_base.join("omavless-login.receipt")).unwrap(),
                login
            );
            assert_eq!(fs::read(&paths.store).unwrap(), store);
            if foreign {
                assert_eq!(fs::read(tun.join("tun_flags")).unwrap(), b"0x1001\n");
            }
            assert!(!paths.active_config.exists() && !paths.staged_config.exists());
            assert!(!paths.controller_socket.exists());
        });
    }
}

#[test]
fn recovery_origin_refuses_managed_unknown_and_incomplete_observations() {
    for case in [
        "managed",
        "unknown-scope",
        "malformed-config",
        "malformed-flags",
        "missing-inventory",
        "missing-comm",
        "visible-mihomo",
    ] {
        with_origin(|origin, f, paths| {
            assert_eq!(origin.check_old_recovery_lease(), Ok(()));
            let tun = paths.sys_class_net.join(if case == "managed" {
                "Meta"
            } else {
                "foreign0"
            });
            fs::create_dir(&tun).unwrap();
            fs::write(tun.join("tun_flags"), b"0x1001\n").unwrap();
            match case {
                "unknown-scope" => private(&paths.active_config, b"tun: {enable: true}\n"),
                "malformed-config" => private(&paths.active_config, &[0xff]),
                "malformed-flags" => fs::write(tun.join("tun_flags"), b"invalid\n").unwrap(),
                "missing-inventory" => {
                    fs::rename(&paths.sys_class_net, f.root.join("old-sys")).unwrap()
                }
                "missing-comm" | "visible-mihomo" => {
                    let process = paths.proc_root.join("17");
                    fs::create_dir(&process).unwrap();
                    if case == "visible-mihomo" {
                        fs::write(process.join("comm"), b"mihomo\n").unwrap();
                    }
                }
                _ => {}
            }
            assert_eq!(
                origin.bindings(false),
                Ok(()),
                "{case}: original bindings remain valid"
            );
            assert_eq!(
                origin.check_observed_off(),
                Err(FirstError::Admission),
                "{case}: actual fresh host gate"
            );
            assert_eq!(
                origin.check_old_recovery_lease(),
                Err(FirstError::Admission),
                "{case}: retained origin refuses"
            );
            assert!(origin.lock.authorizes(&f.paths, f.uid));
            assert!(!paths.staged_config.exists() && !paths.controller_socket.exists());
        });
    }
}

#[test]
fn recovery_origin_foreign_tun_never_overrides_changed_retained_bindings() {
    for case in [
        "desired",
        "ownership",
        "login",
        "missing-login",
        "same-byte-template-replacement",
    ] {
        with_origin(|origin, f, paths| {
            let tun = paths.sys_class_net.join("foreign0");
            fs::create_dir(&tun).unwrap();
            fs::write(tun.join("tun_flags"), b"0x1001\n").unwrap();
            assert_eq!(origin.check_old_recovery_lease(), Ok(()));
            match case {
                "desired" => {
                    let desired = DesiredState {generation: 1, ..DesiredState::default()};
                    crate::desired::write_desired(origin.desired_paths, f.uid, &desired).unwrap();
                }
                "ownership" => private(&f.paths.ownership_marker, br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#),
                "login" => private(&f.paths.runtime_base.join("omavless-login.receipt"),
                    br#"{"schemaVersion":1,"epochHash":"0000000000000000000000000000000000000000000000000000000000000000","ownershipGeneration":2,"phase":"pending"}"#),
                "missing-login" => fs::remove_file(f.paths.runtime_base.join("omavless-login.receipt")).unwrap(),
                _ => {
                    let bytes = fs::read(&paths.template).unwrap();
                    fs::rename(&paths.template, f.root.join("old-template")).unwrap();
                    private(&paths.template, &bytes);
                }
            }
            // A valid foreign interface does not hide namespace/lease data drift.
            assert_eq!(origin.check_observed_off(), Ok(()));
            assert_eq!(
                origin.check_old_recovery_lease(),
                Err(FirstError::Admission),
                "{case}"
            );
            assert_eq!(fs::read(tun.join("tun_flags")).unwrap(), b"0x1001\n");
            assert!(!paths.active_config.exists() && !paths.staged_config.exists());
            assert!(!paths.controller_socket.exists());
        });
    }
}
