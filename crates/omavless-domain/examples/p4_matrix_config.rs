// SPDX-License-Identifier: MIT
//! Developer-only isolated IP-family/MTU fixture renderer. No daemon/IPC/installed caller.
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
const TEMPLATE: &str = "mixed-port: 7898\nallow-lan: false\nbind-address: 127.0.0.1\nmode: rule\nlog-level: silent\nipv6: true\ntun:\n  enable: false\ndns:\n  enable: false\nproxies:\n{{OMAVLESS_PROXY}}\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies:\n      - P4 loopback\nrules:\n  - MATCH,PROXY\n";

fn render(
    root: &Path,
    phase: &str,
    generation: &str,
    outer: &str,
    inner: &str,
    mtu: &str,
) -> Result<(), ()> {
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
    // Strict fixed fixture inputs. These are not a new profile/parser API.
    let address = match inner {
        "4" => "10.203.0.2/32",
        "6" => "fd20:203::2/128",
        _ => return Err(()),
    };
    let allowed = match inner {
        "4" => "10.203.0.1/32",
        "6" => "fd20:203::1/128",
        _ => return Err(()),
    };
    let endpoint = match outer {
        "4" => "127.0.0.1:51888",
        "6" => "[::1]:51888",
        _ => return Err(()),
    };
    if !matches!(mtu, "1280" | "1420")
        || input
            .lines()
            .filter(|line| *line == format!("Address = {address}"))
            .count()
            != 1
        || input
            .lines()
            .filter(|line| *line == format!("AllowedIPs = {allowed}"))
            .count()
            != 1
        || input
            .lines()
            .filter(|line| *line == format!("Endpoint = {endpoint}"))
            .count()
            != 1
        || input
            .lines()
            .filter(|line| *line == format!("MTU = {mtu}"))
            .count()
            != 1
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
    let okay = arguments.len() == 7
        && (|| {
            let phase = arguments[2].to_str().ok_or(())?;
            let outer = arguments[4].to_str().ok_or(())?;
            let inner = arguments[5].to_str().ok_or(())?;
            let mtu = arguments[6].to_str().ok_or(())?;
            render(
                Path::new(&arguments[1]),
                phase,
                generation,
                outer,
                inner,
                mtu,
            )
        })()
        .is_ok();
    if !okay {
        eprintln!("p4_matrix_render_refused");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "p4-matrix-render-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn fixture(root: &Path, flavor: &str, outer: &str, inner: &str, mtu: &str) {
        let key = base64::engine::general_purpose::STANDARD.encode([1u8; 32]);
        let ip = if inner == "4" {
            "10.203.0.2/32"
        } else {
            "fd20:203::2/128"
        };
        let allowed = if inner == "4" {
            "10.203.0.1/32"
        } else {
            "fd20:203::1/128"
        };
        let endpoint = if outer == "4" {
            "127.0.0.1:51888"
        } else {
            "[::1]:51888"
        };
        let mut fields = String::new();
        if flavor != "wireguard" {
            fields = format!(
                "Jc = 4\nJmin = 64\nJmax = 66\nS1 = 24\nS2 = 32\nS3 = 40\nS4 = 48\nH1 = 101\nH2 = 202\nH3 = 303\nH4 = 404\nHeaderProtectionKey = {key}\nContentPaddingAddition = 37\nRekeyAfterTime = 2\nRekeyTimeout = 1\nRejectAfterTime = 30\nKeepaliveTimeout = 1\nMaxHandshakeAttempts = 4\n"
            );
            for n in 1..=5 {
                fields.push_str(&format!("I{n} = <b 0x{n:02x}>\n"));
            }
            if flavor == "3.1" {
                fields.push_str("RandomTrailers = true\nDisableCookies = true\n");
            }
        }
        let input = format!(
            "[Interface]\nPrivateKey = {key}\nAddress = {ip}\nMTU = {mtu}\n{fields}[Peer]\nPublicKey = {key}\nPresharedKey = {key}\nAllowedIPs = {allowed}\nEndpoint = {endpoint}\nPersistentKeepalive = 1\n"
        );
        for phase in ["positive", "negative"] {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(root.join(format!("{phase}.conf")))
                .unwrap();
            file.write_all(input.as_bytes()).unwrap();
        }
    }
    #[test]
    fn all_24_geometries_roundtrip_each_phase() {
        let mut rendered = 0;
        for flavor in ["wireguard", "3", "3.1"] {
            for outer in ["4", "6"] {
                for inner in ["4", "6"] {
                    for mtu in ["1280", "1420"] {
                        let root = Scratch::new();
                        fixture(&root.0, flavor, outer, inner, mtu);
                        for phase in ["positive", "negative"] {
                            assert!(render(&root.0, phase, flavor, outer, inner, mtu).is_ok());
                            let output =
                                fs::read_to_string(root.0.join(format!("{phase}.yaml"))).unwrap();
                            assert!(
                                output.contains("ipv6: true")
                                    && output.contains("udp: true")
                                    && !output.contains("DIRECT")
                                    && !output.contains("nameserver:")
                            );
                            assert!(output.contains(&format!("mtu: {mtu}")));
                            rendered += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(rendered, 48);
    }
    #[test]
    fn geometry_mismatch_invalid_modes_and_existing_output_refuse() {
        let root = Scratch::new();
        fixture(&root.0, "wireguard", "4", "4", "1280");
        for (phase, flavor, outer, inner, mtu) in [
            ("unknown", "wireguard", "4", "4", "1280"),
            ("positive", "unknown", "4", "4", "1280"),
            ("positive", "wireguard", "6", "4", "1280"),
            ("positive", "wireguard", "4", "6", "1280"),
            ("positive", "wireguard", "4", "4", "1420"),
            ("positive", "wireguard", "4", "4", "1500"),
        ] {
            assert!(render(&root.0, phase, flavor, outer, inner, mtu).is_err());
        }
        assert!(render(&root.0, "positive", "wireguard", "4", "4", "1280").is_ok());
        assert!(render(&root.0, "positive", "wireguard", "4", "4", "1280").is_err());
    }
}
