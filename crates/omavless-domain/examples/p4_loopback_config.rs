// SPDX-License-Identifier: MIT
//! Developer-only isolated WG fixture renderer. No daemon/IPC/installed caller.
use omavless_domain::private_store::parse_candidate_private_store;
use omavless_profile::wireguard::{
    AwgGeneration, MAX_WIREGUARD_CONFIG_BYTES, WireGuardFlavor, parse_wireguard_config,
};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

const ID: &str = "00000000-0000-0000-0000-000000000001";
const NAME: &str = "P4 loopback";
const TEMPLATE: &str = "mixed-port: 7898\nallow-lan: false\nbind-address: 127.0.0.1\nmode: rule\nlog-level: silent\nipv6: false\ntun:\n  enable: false\ndns:\n  enable: false\nproxies:\n{{OMAVLESS_PROXY}}\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies:\n      - P4 loopback\nrules:\n  - MATCH,PROXY\n";

fn render(root: &Path, phase: &str, generation: &str) -> Result<(), ()> {
    if !root.is_absolute() || !matches!(phase, "positive" | "negative") {
        return Err(());
    }
    let directory = fs::symlink_metadata(root).map_err(|_| ())?;
    if !directory.is_dir() || directory.file_type().is_symlink() || directory.mode() & 0o077 != 0 {
        return Err(());
    }
    // Linux fixture only: O_NOFOLLOW | O_NONBLOCK. Never acquire an installed profile.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(0x20000 | 0x800)
        .open(root.join(format!("{phase}.conf")))
        .map_err(|_| ())?;
    let before = file.metadata().map_err(|_| ())?;
    if !before.is_file()
        || before.uid() != directory.uid()
        || before.mode() & 0o077 != 0
        || before.nlink() != 1
        || before.len() > MAX_WIREGUARD_CONFIG_BYTES as u64
    {
        return Err(());
    }
    let mut input = String::new();
    Read::by_ref(&mut file)
        .take(MAX_WIREGUARD_CONFIG_BYTES as u64 + 1)
        .read_to_string(&mut input)
        .map_err(|_| ())?;
    let after = file.metadata().map_err(|_| ())?;
    if input.len() != before.len() as usize
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || before.uid() != after.uid()
        || before.mode() != after.mode()
        || before.nlink() != after.nlink()
    {
        return Err(());
    }
    let profile = parse_wireguard_config(&input).map_err(|_| ())?;
    let expected = match generation {
        "wireguard" => WireGuardFlavor::Standard,
        "3" => WireGuardFlavor::Amnezia(AwgGeneration::V3),
        "3.1" => WireGuardFlavor::Amnezia(AwgGeneration::V3_1),
        _ => return Err(()),
    };
    if profile.facts().flavor != expected {
        return Err(());
    }
    let store =
        parse_candidate_private_store("{\"version\":4,\"profiles\":[],\"subscriptions\":[]}")
            .map_err(|_| ())?
            .with_wireguard(ID, NAME, profile)
            .map_err(|_| ())?;
    let encoded = store.into_private_bytes().map_err(|_| ())?;
    let store = parse_candidate_private_store(std::str::from_utf8(&encoded).map_err(|_| ())?)
        .map_err(|_| ())?;
    let controller = root.join("mihomo.sock");
    let controller = controller.to_str().ok_or(())?;
    let prepared = store
        .prepare_private_config_mode(ID, TEMPLATE, controller, "rule")
        .map_err(|_| ())?;
    let export = store.export_private_native_credential(ID).map_err(|_| ())?;
    let reimported =
        parse_wireguard_config(std::str::from_utf8(export.expose_private_bytes()).map_err(|_| ())?)
            .map_err(|_| ())?;
    let restored =
        parse_candidate_private_store("{\"version\":4,\"profiles\":[],\"subscriptions\":[]}")
            .map_err(|_| ())?
            .with_wireguard(ID, NAME, reimported)
            .map_err(|_| ())?;
    let restored = restored
        .prepare_private_config_mode(ID, TEMPLATE, controller, "rule")
        .map_err(|_| ())?;
    if restored.expose_private_bytes() != prepared.expose_private_bytes() {
        return Err(());
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(format!("{phase}.yaml")))
        .map_err(|_| ())?;
    output
        .write_all(prepared.expose_private_bytes())
        .map_err(|_| ())?;
    output.sync_all().map_err(|_| ())?;
    println!("{{\"private_roundtrip\":true,\"flavor\":\"{generation}\"}}");
    Ok(())
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    let generation = arguments
        .get(3)
        .and_then(|value| value.to_str())
        .unwrap_or("wireguard");
    let okay = matches!(arguments.len(), 3 | 4)
        && arguments[2]
            .to_str()
            .is_some_and(|phase| render(Path::new(&arguments[1]), phase, generation).is_ok());
    if !okay {
        eprintln!("p4_loopback_render_refused");
        std::process::exit(2);
    }
}
