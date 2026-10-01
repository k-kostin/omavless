// SPDX-License-Identifier: MIT

//! Opt-in offline installed-core syntax check with public synthetic keys.
//! This does not start a proxy, connect to a server or establish a tunnel.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use omavless_profile::import::parse_import;

const PRIVATE: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
const PUBLIC: &str = "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=";
const HEADER: &str = "YGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn8=";

fn guest(container: &str, protocol: &str, config: &str) -> String {
    let value = serde_json::json!({
        "defaultContainer": container,
        "containers": [{
            "container": container,
            protocol: {"last_config": serde_json::json!({"config": config}).to_string()}
        }]
    })
    .to_string();
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
    encoder.write_all(value.as_bytes()).unwrap();
    let mut payload = (value.len() as u32).to_be_bytes().to_vec();
    payload.extend(encoder.finish().unwrap());
    format!("vpn://{}", URL_SAFE_NO_PAD.encode(payload))
}

#[test]
#[ignore = "opt-in: set OMAVLESS_MIHOMO to an installed core for offline syntax validation"]
fn synthetic_structured_wg_and_awg3_are_accepted_by_installed_mihomo() {
    let Some(mihomo) = env::var_os("OMAVLESS_MIHOMO") else {
        return;
    };
    let configs = [
        (
            "amnezia-wireguard",
            "wireguard",
            format!(
                "[Interface]\nPrivateKey = {PRIVATE}\nAddress = 10.8.0.2/32\nDNS = 1.1.1.1\n\n[Peer]\nPublicKey = {PUBLIC}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.1:51820\n"
            ),
        ),
        (
            "amnezia-awg2",
            "awg",
            format!(
                "[Interface]\nPrivateKey = {PRIVATE}\nAddress = 10.8.0.2/32\nJc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nS3 = 30\nS4 = 35\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\nHeaderProtectionKey = {HEADER}\nContentPaddingAddition = 10-100\nRekeyAfterTime = 100-120\nRekeyTimeout = 3-7\nRejectAfterTime = 150-180\nKeepaliveTimeout = 5-15\nMaxHandshakeAttempts = 15-20\n\n[Peer]\nPublicKey = {PUBLIC}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.1:51820\nPersistentKeepalive = 25-35\n"
            ),
        ),
    ];
    for (container, protocol, config) in configs {
        let parsed = parse_import(&guest(container, protocol, &config)).unwrap();
        let name = "Synthetic WG validation";
        let proxy = parsed.render_mihomo_proxy(name, None);
        let full = format!(
            "mixed-port: 7890\nmode: rule\nlog-level: silent\nproxies:\n{proxy}\nproxy-groups:\n  - name: Validation\n    type: select\n    proxies:\n      - {name}\nrules:\n  - MATCH,Validation\n"
        );
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "omavless-synthetic-p4-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let file = root.join("config.yaml");
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&file)
            .unwrap();
        output.write_all(full.as_bytes()).unwrap();
        drop(output);
        let result = Command::new(&mihomo)
            .args(["-t", "-f"])
            .arg(&file)
            .arg("-d")
            .arg(&root)
            .output()
            .unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(
            result.status.success(),
            "installed Mihomo rejected synthetic WG/AWG mapping"
        );
    }
}
