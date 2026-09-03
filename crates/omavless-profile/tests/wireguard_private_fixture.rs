// SPDX-License-Identifier: MIT

//! Opt-in private-fixture and installed-core checks for P4. The fixture path is
//! supplied by the owner; fixture bytes and generated YAML never enter output.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use omavless_profile::wireguard::{
    AwgGeneration, WireGuardError, WireGuardFlavor, parse_amnezia_vpn_link, parse_wireguard_config,
};

fn private_file(variable: &str) -> Option<String> {
    let path = env::var_os(variable)?;
    let metadata = fs::symlink_metadata(&path).expect("private AWG fixture must be readable");
    assert!(
        metadata.is_file(),
        "private AWG fixture must be a regular file"
    );
    assert_eq!(
        metadata.permissions().mode() & 0o077,
        0,
        "private AWG fixture must not be accessible by group or other users"
    );
    Some(fs::read_to_string(path).expect("private AWG fixture must be valid UTF-8"))
}

fn private_fixture() -> Option<String> {
    private_file("OMAVLESS_AWG_FIXTURE")
}

#[test]
#[ignore = "requires an owner-supplied private 0600 AWG3 fixture"]
fn private_awg3_fixture_parses_without_disclosure() {
    let Some(config) = private_fixture() else {
        eprintln!("skipped: set OMAVLESS_AWG_FIXTURE to a private 0600 AWG config");
        return;
    };
    let profile = parse_wireguard_config(&config).expect("private AWG fixture must parse");
    assert_eq!(
        profile.facts().flavor,
        WireGuardFlavor::Amnezia(AwgGeneration::V3)
    );
    let debug = format!("{profile:?}");
    assert!(!config.lines().any(|line| {
        line.split_once('=').is_some_and(|(name, value)| {
            matches!(
                name.trim().to_ascii_lowercase().as_str(),
                "privatekey"
                    | "publickey"
                    | "presharedkey"
                    | "headerprotectionkey"
                    | "endpoint"
                    | "address"
                    | "dns"
            ) && !value.trim().is_empty()
                && debug.contains(value.trim())
        })
    }));
}

#[test]
#[ignore = "requires an owner-supplied private 0600 AWG3 fixture"]
fn private_awg3_fixture_survives_qcompress_guest_envelope() {
    let Some(config) = private_fixture() else {
        eprintln!("skipped: set OMAVLESS_AWG_FIXTURE to a private 0600 AWG config");
        return;
    };
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(8));
    encoder
        .write_all(config.as_bytes())
        .expect("private AWG fixture compression must succeed");
    let mut payload = (config.len() as u32).to_be_bytes().to_vec();
    payload.extend(
        encoder
            .finish()
            .expect("private AWG fixture compression must finish"),
    );
    let link = format!("vpn://{}", URL_SAFE_NO_PAD.encode(payload));
    let profile = parse_amnezia_vpn_link(&link).expect("private AWG guest link must parse");
    assert_eq!(
        profile.facts().flavor,
        WireGuardFlavor::Amnezia(AwgGeneration::V3)
    );
}

#[test]
#[ignore = "requires an owner-supplied private 0600 AWG3 fixture and Mihomo"]
fn private_awg3_fixture_is_accepted_by_installed_mihomo() {
    let (Some(config), Some(mihomo)) = (private_fixture(), env::var_os("OMAVLESS_MIHOMO")) else {
        eprintln!(
            "skipped: set OMAVLESS_AWG_FIXTURE and OMAVLESS_MIHOMO for installed-core validation"
        );
        return;
    };
    let profile = parse_wireguard_config(&config).expect("private AWG fixture must parse");
    let proxy = profile.render_mihomo_proxy("Private AWG validation", None);
    let full_config = format!(
        "mixed-port: 7890\nmode: rule\nlog-level: silent\nproxies:\n{proxy}\nproxy-groups:\n  - name: Validation\n    type: select\n    proxies:\n      - Private AWG validation\nrules:\n  - MATCH,Validation\n"
    );

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be valid")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "omavless-private-awg-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("private validation directory must be created");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .expect("private validation directory must stay private");
    let config_path = root.join("config.yaml");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&config_path)
        .expect("private validation config must be created");
    output
        .write_all(full_config.as_bytes())
        .expect("private validation config must be written");
    drop(output);

    let output = Command::new(Path::new(&mihomo))
        .args(["-t", "-f"])
        .arg(&config_path)
        .arg("-d")
        .arg(&root)
        .output()
        .expect("installed Mihomo must start");
    let _ = fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "installed Mihomo rejected the AWG3 mapping"
    );
}

#[test]
#[ignore = "requires an owner-supplied private 0600 Amnezia API guest key"]
fn private_api_guest_key_is_classified_without_disclosure() {
    let Some(link) = private_file("OMAVLESS_AWG_GUEST_FIXTURE") else {
        eprintln!("skipped: set OMAVLESS_AWG_GUEST_FIXTURE to a private 0600 guest key");
        return;
    };
    let error = parse_amnezia_vpn_link(&link).expect_err("API guest key must remain offline-only");
    assert_eq!(error, WireGuardError::UnsupportedVpnApiKey);
    assert_eq!(error.code(), "unsupported_vpn_api_key");
}
