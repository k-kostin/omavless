// SPDX-License-Identifier: MIT
//! Conservative device scope, not a general YAML parser or ownership claim.
//! Unsupported YAML keeps the old whole-host guard. A recognized configured
//! name is only a collision/cleanup boundary; live attribution also needs the
//! owned PID-authenticated controller and a stable kernel interface identity.
use crate::lifecycle::HostStepError;
use omavless_mihomo::observation::tun_interface_count_strict;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConfiguredTun {
    Disabled,
    Device(String),
}

pub(crate) fn configured_tun(text: &str) -> Option<ConfiguredTun> {
    if text.len() > omavless_domain::config::MAX_TEMPLATE_BYTES || text.contains('\t') {
        return None;
    }
    let mut roots = BTreeSet::new();
    let mut children = BTreeSet::new();
    let mut in_tun = false;
    let mut enabled = None;
    let mut device = None;
    let mut scalar_child = false;
    let mut root_sequence_allowed = false;
    for line in text.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            // YAML permits an unindented block sequence as a mapping value
            // (our generated proxies list uses this style).
            if line.starts_with("- ") && root_sequence_allowed && !in_tun {
                continue;
            }
            if line == omavless_domain::config::PROFILE_MARKER && !in_tun {
                continue;
            }
            let (key, value) = line.split_once(':')?;
            if !plain_key(key) || !roots.insert(key) {
                return None;
            }
            in_tun = key == "tun";
            root_sequence_allowed = value.trim().is_empty() || value.trim().starts_with('#');
            if in_tun && !value.trim().is_empty() && !value.trim().starts_with('#') {
                return None;
            }
        } else if in_tun {
            // Accept the explicit two-space block mapping emitted by our
            // templates; aliases, merges, tags and alternate key spellings
            // must not acquire a narrower cleanup proof.
            let child = line.strip_prefix("  ")?;
            if child.starts_with(' ') {
                if scalar_child {
                    return None;
                }
                continue;
            }
            let (key, value) = child.split_once(':')?;
            if !plain_key(key) || !children.insert(key) {
                return None;
            }
            let scalar = value.split(" #").next()?.trim();
            scalar_child = matches!(key, "enable" | "device");
            match key {
                "enable" => {
                    enabled = Some(match scalar {
                        "true" => true,
                        "false" => false,
                        _ => return None,
                    });
                }
                "device" => {
                    let scalar = if let Some(s) = scalar.strip_prefix('"') {
                        s.strip_suffix('"')?
                    } else if let Some(s) = scalar.strip_prefix('\'') {
                        s.strip_suffix('\'')?
                    } else {
                        // YAML numeric coercion must not give the core a name
                        // different from the byte spelling we are scoping.
                        if !scalar
                            .bytes()
                            .next()
                            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                        {
                            return None;
                        }
                        scalar
                    };
                    if !valid_device(scalar) {
                        return None;
                    }
                    device = Some(scalar.to_owned());
                }
                _ => {}
            }
        }
    }
    match enabled? {
        true => device.map(ConfiguredTun::Device),
        false => Some(ConfiguredTun::Disabled),
    }
}

fn plain_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn valid_device(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub(crate) fn device_index(root: &Path, name: &str) -> Result<u64, HostStepError> {
    if !valid_device(name) {
        return Err(HostStepError::Observation);
    }
    let path = root.join(name);
    let before = fs::metadata(&path).map_err(|_| HostStepError::Observation)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path.join("ifindex"))
        .map_err(|_| HostStepError::Observation)?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(HostStepError::Observation);
    }
    let mut value = String::new();
    file.take(33)
        .read_to_string(&mut value)
        .map_err(|_| HostStepError::Observation)?;
    if value.len() > 32 || !value.trim().bytes().all(|b| b.is_ascii_digit()) {
        return Err(HostStepError::Observation);
    }
    let index: u64 = value
        .trim()
        .parse()
        .map_err(|_| HostStepError::Observation)?;
    let after = fs::metadata(&path).map_err(|_| HostStepError::Observation)?;
    if index == 0
        || index > u64::from(u32::MAX)
        || before.ino() != after.ino()
        || before.dev() != after.dev()
    {
        return Err(HostStepError::Observation);
    }
    Ok(index)
}

/// Preserve complete strict inventory checks even when only configured names
/// count towards OmaVLESS cleanup. A missing/broken sysfs is never empty.
pub(crate) fn inventory(root: &Path) -> Result<BTreeSet<String>, HostStepError> {
    let before = tun_interface_count_strict(root).map_err(|_| HostStepError::Observation)?;
    let mut names = BTreeSet::new();
    for (index, entry) in fs::read_dir(root)
        .map_err(|_| HostStepError::Observation)?
        .enumerate()
    {
        if index >= 512 {
            return Err(HostStepError::Observation);
        }
        let entry = entry.map_err(|_| HostStepError::Observation)?;
        if entry
            .path()
            .join("tun_flags")
            .try_exists()
            .map_err(|_| HostStepError::Observation)?
        {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| HostStepError::Observation)?;
            if !valid_device(&name) {
                return Err(HostStepError::Observation);
            }
            names.insert(name);
        }
    }
    let after = tun_interface_count_strict(root).map_err(|_| HostStepError::Observation)?;
    if before != after || usize::from(after) != names.len() {
        return Err(HostStepError::Observation);
    }
    Ok(names)
}

pub(crate) fn configured_devices(
    config: &Path,
    uid: u32,
) -> Result<Option<BTreeSet<String>>, HostStepError> {
    let mut devices = BTreeSet::new();
    let mut recognized = false;
    for name in [
        "config.yaml",
        ".config.candidate.yaml",
        "route-template.yaml",
    ] {
        let path = config.join(name);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(HostStepError::Observation),
            Ok(_) => {}
        }
        let text = omavless_store::read_private_utf8(&path, uid)
            .map_err(|_| HostStepError::Observation)?;
        let Some(tun) = configured_tun(&text) else {
            return Ok(None);
        };
        recognized = true;
        if let ConfiguredTun::Device(device) = tun {
            devices.insert(device);
        }
    }
    Ok(recognized.then_some(devices))
}

pub(crate) fn count_scoped(
    root: &Path,
    devices: Option<&BTreeSet<String>>,
) -> Result<u8, HostStepError> {
    let inventory = inventory(root)?;
    let count = devices.map_or(inventory.len(), |d| inventory.intersection(d).count());
    u8::try_from(count).map_err(|_| HostStepError::Observation)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configured_device(text: &str) -> Option<String> {
        match configured_tun(text)? {
            ConfiguredTun::Device(name) => Some(name),
            ConfiguredTun::Disabled => None,
        }
    }
    #[test]
    fn explicit_disabled_tun_has_empty_scope_not_an_unknown_device() {
        assert_eq!(
            configured_tun("tun:\n  enable: false\n"),
            Some(ConfiguredTun::Disabled)
        );
        assert_eq!(configured_tun("tun:\n  device: Meta\n"), None);
    }
    #[test]
    #[ignore = "read-only private configured scope on an explicitly supplied disconnected host"]
    fn installed_private_scope_with_foreign_tun() {
        let path = std::env::var_os("OMAVLESS_TEST_TUN_CONFIG").expect("explicit fixture required");
        let devices =
            configured_devices(Path::new(&path), nix::unistd::Uid::current().as_raw()).unwrap();
        assert!(
            devices.is_some(),
            "configuration requires conservative fallback"
        );
        let root = Path::new("/sys/class/net");
        assert!(
            !inventory(root).unwrap().is_empty(),
            "foreign TUN fixture required"
        );
        assert_eq!(
            count_scoped(root, devices.as_ref()).unwrap(),
            0,
            "configured TUN is not absent"
        );
    }

    #[test]
    fn kernel_index_read_is_bounded_and_rejects_symlinks() {
        let root = crate::test_temp::directory("tun-index").unwrap();
        let dev = root.join("Meta");
        fs::create_dir(&dev).unwrap();
        for invalid in ["0", "-1", "4294967296", "private text", "1\n2"] {
            fs::write(dev.join("ifindex"), invalid).unwrap();
            assert!(device_index(&root, "Meta").is_err());
        }
        fs::write(dev.join("ifindex"), "17\n").unwrap();
        assert_eq!(device_index(&root, "Meta").unwrap(), 17);
        fs::remove_file(dev.join("ifindex")).unwrap();
        std::os::unix::fs::symlink("/dev/null", dev.join("ifindex")).unwrap();
        assert!(device_index(&root, "Meta").is_err());
        assert!(device_index(&root, "../Meta").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn explicit_bundled_device_and_rendered_block_are_recognized() {
        for text in [
            include_str!("../../../templates/default.yaml"),
            include_str!("../../../templates/china.yaml"),
            include_str!("../../../templates/iran.yaml"),
        ] {
            assert_eq!(configured_device(text).as_deref(), Some("Meta"));
        }
        for value in ["Meta", "'Meta'", "\"Meta\"", "Meta # comment"] {
            assert_eq!(
                configured_device(&format!("tun:\n  enable: true\n  device: {value}\n")).as_deref(),
                Some("Meta")
            );
        }
        assert_eq!(configured_device("proxies:\n- name: Synthetic\n  type: direct\ntun:\n  enable: true\n  device: Meta\nrules:\n- MATCH,DIRECT\n").as_deref(), Some("Meta"));
    }
    #[test]
    fn unsupported_yaml_does_not_gain_scoped_cleanup() {
        for text in [
            "tun: {enable: true, device: Meta}",
            "tun:\n  enable: true\n  device: Meta\n  device: other\n",
            "tun:\n  enable: true\n  device: Meta\ntun:\n  device: other\n",
            "'tun':\n  enable: true\n  device: Meta\n",
            "tun:\n  enable: true\n  <<: *other\n  device: Meta\n",
            "tun:\n  enable: true\n  device: &name Meta\n",
            "tun:\n  enable: true\n  device: '../private'\n",
            "tun:\n  enable: false\n  device: Meta\n",
            "tun:\n  enable: true\n  device: |\n    Meta\n",
            "tun:\n  enable: true\n  device: \"M\\u0065ta\"\n",
            "tun:\n  enable: true\n  device: 0123\n",
            "tun:\n  enable: true\n  device: Meta\n    folded\n",
            "tun:\n  enable: true\n",
            "",
        ] {
            assert!(configured_device(text).is_none());
        }
    }
}
