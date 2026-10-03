// SPDX-License-Identifier: MIT
use super::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

fn document(architecture: &str) -> serde_json::Value {
    serde_json::json!({
        "schema": "omavless-close-research-only-v1", "architecture": architecture,
        "abi": 1, "pair": "managed-dns-source-composition-only", "source": SOURCE,
        "dns_source": DNS_SOURCE, "mihomo_commit": MIHOMO, "sing_tun_commit": SING_TUN,
        "conditional_patch_sha256": CLOSE_PATCH, "dns_patch_sha256": DNS_PATCH,
        "tun_patch_sha256": TUN_PATCH,
        "build": {"go_version": "go1.27.0-X:nodwarf5", "tags": "with_gvisor",
            "cgo": false, "buildvcs": false, "dependency_mode": "vendor", "goos": "linux",
            "goarch": if architecture == "aarch64" { "arm64" } else { "amd64" }},
        "core_sha256": "3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544"
    })
}
fn parse(v: &serde_json::Value, arch: &str) -> Result<[u8; 32]> {
    decode_receipt(&serde_json::to_vec(v).unwrap(), arch)
}
fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("close-object-")
        .tempdir_in(std::env::var_os("HOME").unwrap())
        .unwrap()
}
fn write(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
#[test]
fn strict_receipt_is_only_syntax_and_expected_composition_not_authority() {
    for arch in ["x86_64", "aarch64"] {
        let original = document(arch);
        assert!(parse(&original, arch).is_ok());
        for key in [
            "schema",
            "architecture",
            "abi",
            "pair",
            "source",
            "dns_source",
            "mihomo_commit",
            "sing_tun_commit",
            "conditional_patch_sha256",
            "dns_patch_sha256",
            "tun_patch_sha256",
            "core_sha256",
        ] {
            let mut changed = original.clone();
            changed[key] = serde_json::json!("changed");
            assert!(parse(&changed, arch).is_err(), "field {key}");
            let mut missing = original.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(parse(&missing, arch).is_err());
        }
        for key in [
            "go_version",
            "tags",
            "cgo",
            "buildvcs",
            "dependency_mode",
            "goos",
            "goarch",
        ] {
            let mut changed = original.clone();
            changed["build"][key] = serde_json::json!("changed");
            assert!(parse(&changed, arch).is_err(), "build field {key}");
        }
        for hash in [
            "0".repeat(64),
            "F".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
        ] {
            let mut changed = original.clone();
            changed["core_sha256"] = hash.into();
            assert!(parse(&changed, arch).is_err());
        }
    }
    let raw = serde_json::to_string(&document("x86_64")).unwrap();
    for (from, to) in [
        ("\"abi\":1", "\"abi\":1,\"abi\":1"),
        ("\"abi\":1", "\"abi\":1,\"\\u0061bi\":1"),
        ("\"cgo\":false", "\"cgo\":false,\"cgo\":false"),
        ("\"abi\":1", "\"abi\":1,\"grant\":true"),
        ("\"cgo\":false", "\"cgo\":false,\"command\":\"unused\""),
    ] {
        assert!(raw.contains(from));
        assert!(decode_receipt(raw.replace(from, to).as_bytes(), "x86_64").is_err());
    }
    assert!(decode_receipt(&[b' '; MAX_RECEIPT as usize + 1], "x86_64").is_err());
    assert!(decode_receipt(b"\xff", "x86_64").is_err());
    assert!(decode_receipt(format!("{raw}{{}}").as_bytes(), "x86_64").is_err());
}
#[test]
fn user_owned_objects_are_never_root_package_evidence() {
    assert_ne!(
        nix::unistd::getuid().as_raw(),
        0,
        "ordinary gate must run unprivileged"
    );
    let root = scratch();
    write(
        &root.path().join(RECEIPT),
        &serde_json::to_vec(&document(std::env::consts::ARCH)).unwrap(),
        0o644,
    );
    write(&root.path().join(CORE), b"synthetic not ELF", 0o755);
    assert!(Objects::open_at(root.path()).is_err());
    let dir = File::open(root.path()).unwrap();
    assert!(member(&dir, RECEIPT, 0o644, MAX_RECEIPT).is_err());
    assert!(member(&dir, CORE, 0o755, MAX_CORE).is_err());
    assert!(DirectoryIdentity::capture(&dir.metadata().unwrap()).is_err());
    assert!(Objects::open_at(Path::new("relative")).is_err());
}
#[test]
fn member_open_refuses_links_fifo_directory_and_oversize_without_blocking() {
    let root = scratch();
    let dir = File::open(root.path()).unwrap();
    let path = root.path().join("member");
    symlink("missing", &path).unwrap();
    assert!(member(&dir, "member", 0o644, MAX_RECEIPT).is_err());
    fs::remove_file(&path).unwrap();
    nix::unistd::mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
    assert!(member(&dir, "member", 0o644, MAX_RECEIPT).is_err());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(member(&dir, "member", 0o644, MAX_RECEIPT).is_err());
    let oversize = root.path().join("oversize");
    write(&oversize, &[b'x'; MAX_RECEIPT as usize + 1], 0o644);
    assert!(member(&dir, "oversize", 0o644, MAX_RECEIPT).is_err());
}
#[test]
fn retained_identity_primitive_detects_replacement_mutation_and_hardlink() {
    // Primitive only: these user-owned objects never enter Objects or
    // PackageObjects. This is not a positive root-owned package fixture.
    let root = scratch();
    let path = root.path().join("member");
    write(&path, b"original", 0o644);
    let mut held = File::open(&path).unwrap();
    let identity = FileIdentity::capture(&held.metadata().unwrap()).unwrap();
    assert!(identity.matches(&held));
    assert_eq!(read_receipt(&mut held, identity).unwrap(), b"original");
    let next = root.path().join("next");
    write(&next, b"original", 0o644);
    fs::rename(&next, &path).unwrap();
    assert!(
        FileIdentity::capture(&File::open(&path).unwrap().metadata().unwrap()) != Some(identity)
    );
    assert!(!identity.matches(&held)); // unlinked original has nlink zero
    let live = File::open(&path).unwrap();
    let identity = FileIdentity::capture(&live.metadata().unwrap()).unwrap();
    fs::hard_link(&path, &next).unwrap();
    assert!(!identity.matches(&live));
    fs::remove_file(&next).unwrap();
    let identity = FileIdentity::capture(&live.metadata().unwrap()).unwrap();
    write(&path, b"changed!", 0o644);
    assert!(!identity.matches(&live));
}
#[test]
fn elf_header_uses_actual_machine_not_receipt_architecture() {
    let root = scratch();
    let path = root.path().join("elf");
    for (arch, machine) in [("x86_64", 62u16), ("aarch64", 183u16)] {
        let mut bytes = [0; 64];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes[20] = 1;
        bytes[52] = 64;
        write(&path, &bytes, 0o755);
        let mut file = File::open(&path).unwrap();
        assert!(elf_architecture(&mut file, arch).is_ok());
        let other = if arch == "x86_64" {
            "aarch64"
        } else {
            "x86_64"
        };
        assert_eq!(
            elf_architecture(&mut file, other),
            Err(Refusal::Architecture)
        );
        for offset in [0, 4, 5, 6, 16, 18, 20, 52] {
            let mut changed = bytes;
            changed[offset] = 0;
            write(&path, &changed, 0o755);
            assert!(elf_architecture(&mut file, arch).is_err());
        }
    }
    write(&path, b"#!/bin/sh\n", 0o755);
    assert!(elf_architecture(&mut File::open(&path).unwrap(), std::env::consts::ARCH).is_err());
}

#[test]
#[ignore = "requires exclusive Dev-VM lease and separately provisioned root-owned research objects"]
fn root_owned_package_objects_bind_actual_parent_owned_core_in_dev_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_CLOSE_OBJECTS_VM").as_deref(),
        Ok("1")
    );
    assert_ne!(nix::unistd::getuid().as_raw(), 0);
    // Provisioning is deliberately absent: the authorized VM owner must
    // stage exact reviewed core/receipt beforehand. No sudo/helper/path env.
    let mut objects = Objects::open().unwrap();
    let bytes = read_receipt(&mut objects.receipt, objects.receipt_identity).unwrap();
    let expected = decode_receipt(&bytes, std::env::consts::ARCH).unwrap();
    assert_eq!(
        ExecutableEvidence::hash(
            &mut objects.core,
            objects.core_identity,
            Instant::now() + BUDGET
        ),
        Some(expected)
    );
    let root = scratch();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let socket = root.path().join("c.sock");
    let config = root.path().join("config.yaml");
    write(&config, format!("mixed-port: 0\nexternal-controller-unix: {}\nallow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\ntun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n", socket.display()).as_bytes(), 0o600);
    let executable = Path::new(FIXTURE_DIRECTORY).join(CORE);
    let mut core =
        crate::core::OwnedCore::spawn(&executable, root.path(), &config, &socket).unwrap();
    core.wait_ready(Duration::from_secs(10)).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let mut session = Session::bind(&mut core, nix::unistd::getuid().as_raw()).unwrap();
    session.capture_executable(&executable).unwrap();
    let mut evidence = PackageObjects::capture(&mut session).unwrap();
    evidence.check(&mut session).unwrap();
    session.cancellation().cancel();
    assert_eq!(evidence.check(&mut session), Err(Refusal::Child));
    assert!(evidence.refused);
    drop(session);
    core.stop(Duration::from_secs(5)).unwrap();
    objects.recheck().unwrap();
}
