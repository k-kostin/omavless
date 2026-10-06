// SPDX-License-Identifier: MIT
//! Explicit ignored VM fixture renderer; no production entry point or admission.
use super::*;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use std::fmt::Write as _;
use std::os::fd::AsRawFd;

const INPUT_DIRECTORY: &str = "/run/omavless-k1-peer";
const IDENTITY: &str = "/run/omavless-k1-peer/identity.bin";
// A distinct one-shot publication preserves the old Global-policy matrix.
// This ignored test still calls the production renderer; it does not recreate
// configuration JSON or grant coverage/Arm authority.
const OUTPUT_DIRECTORY: &str = "/run/omavless-k1-rule-rendered";
const OUTPUT: &str = "/run/omavless-k1-rule-rendered/generated.json";
const CONTROLLER: &str = "/run/omavless-k1-rule-rendered/controller.sock";

fn canonical_peer(identity: &[u8; 16]) -> Result<Vec<u8>, PreparationError> {
    let mut uuid = String::with_capacity(36);
    for (index, byte) in identity.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            uuid.push('-');
        }
        write!(&mut uuid, "{byte:02x}").map_err(|_| PreparationError::Refused)?;
    }
    // Construct only the fixed fixture profile in memory, never argv or logs.
    let uri = format!("vless://{uuid}@10.77.0.2:24443?type=tcp&security=tls&sni=peer.k1.invalid");
    let profile = parse_canonical(&uri).map_err(|_| PreparationError::Refused)?;
    render(profile, Path::new(CONTROLLER))
}

fn publish_once() -> Result<(), ()> {
    let uid = nix::unistd::getuid().as_raw();
    if uid != 1000 || nix::unistd::geteuid().as_raw() != uid {
        return Err(());
    }
    let input_directory =
        HeldDirectory::capture(Path::new(INPUT_DIRECTORY), uid).map_err(|_| ())?;
    let identity = HeldFile::capture(Path::new(IDENTITY), uid, 0o400, 16).map_err(|_| ())?;
    if identity.metadata.len() != 16 {
        return Err(());
    }
    let mut bytes = [0; 16];
    identity.file.read_exact_at(&mut bytes, 0).map_err(|_| ())?;
    identity.recheck(Path::new(IDENTITY)).map_err(|_| ())?;
    let rendered = canonical_peer(&bytes).map_err(|_| ())?;
    let directory = HeldDirectory::capture(Path::new(OUTPUT_DIRECTORY), uid).map_err(|_| ())?;
    // Inspect the original held directory, not an independently resolved alias.
    let held_path = format!("/proc/self/fd/{}", directory.file.as_raw_fd());
    if fs::read_dir(held_path).map_err(|_| ())?.next().is_some() {
        return Err(());
    }
    input_directory
        .recheck(Path::new(INPUT_DIRECTORY))
        .map_err(|_| ())?;
    identity.recheck(Path::new(IDENTITY)).map_err(|_| ())?;
    directory
        .recheck(Path::new(OUTPUT_DIRECTORY))
        .map_err(|_| ())?;
    let descriptor = openat(
        &directory.file,
        "generated.json",
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )
    .map_err(|_| ())?;
    // Any failure from this point retains the exclusive file; no retry/unlink.
    let mut output = File::from(descriptor);
    output.write_all(&rendered).map_err(|_| ())?;
    output.sync_all().map_err(|_| ())?;
    directory.file.sync_all().map_err(|_| ())?;
    let captured = HeldFile::capture(Path::new(OUTPUT), uid, 0o600, MAX_CONFIG).map_err(|_| ())?;
    if !same(&output.metadata().map_err(|_| ())?, &captured.metadata)
        || captured.digest != <[u8; 32]>::from(Sha256::digest(&rendered))
    {
        return Err(());
    }
    // Publication changes directory times, but never its original identity.
    for current in [
        directory.file.metadata(),
        fs::symlink_metadata(OUTPUT_DIRECTORY),
    ] {
        let current = current.map_err(|_| ())?;
        if current.dev() != directory.metadata.dev()
            || current.ino() != directory.metadata.ino()
            || current.uid() != uid
            || current.mode() != directory.metadata.mode()
            || current.gid() != directory.metadata.gid()
            || current.nlink() != directory.metadata.nlink()
        {
            return Err(());
        }
    }
    input_directory
        .recheck(Path::new(INPUT_DIRECTORY))
        .map_err(|_| ())?;
    identity.recheck(Path::new(IDENTITY)).map_err(|_| ())?;
    captured.recheck(Path::new(OUTPUT)).map_err(|_| ())?;
    println!("K1_PEER_CONFIG_RENDERED");
    Ok(())
}

#[test]
#[ignore = "ROOT-selected private VM file publication only; no core or network"]
fn render_fixed_peer_config_once() {
    assert!(publish_once().is_ok(), "K1_PEER_CONFIG_REFUSED_OR_UNKNOWN");
}

#[test]
fn fixed_peer_uses_production_canonical_renderer() {
    let bytes = canonical_peer(&[0x11; 16]).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["proxies"][0]["server"], "10.77.0.2");
    assert_eq!(value["proxies"][0]["port"], 24443);
    assert_eq!(value["proxies"][0]["servername"], "peer.k1.invalid");
    assert_eq!(value["proxies"][0]["udp"], false);
    assert_eq!(value["mode"], "rule");
    assert_eq!(value["rules"], json!(["NETWORK,UDP,REJECT", "MATCH,PROXY"]));
    assert_eq!(value["tun"]["disable-icmp-forwarding"], true);
    assert_eq!(value["routing-mark"], omavless_netguard::nft::CORE_MARK);
    assert_eq!(value["dns"]["nameserver"][0], RESOLVER);
    assert_eq!(value["external-controller-unix"], CONTROLLER);
    assert!(value["proxies"][0].get("skip-cert-verify").is_none());
    assert!(matches!(
        issue_coverage([0; 32], None),
        Err(PreparationError::Unsupported)
    ));
}
