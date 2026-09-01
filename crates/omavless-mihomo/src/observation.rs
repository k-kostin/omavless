// SPDX-License-Identifier: MIT

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

pub const MAX_PROCESS_FAMILY: usize = 64;

pub fn process_family(root_pid: u32, proc_root: &Path) -> BTreeSet<u32> {
    if root_pid == 0 {
        return BTreeSet::new();
    }
    let mut found = BTreeSet::from([root_pid]);
    let mut pending = VecDeque::from([root_pid]);
    while let Some(pid) = pending.pop_front() {
        if found.len() >= MAX_PROCESS_FAMILY {
            break;
        }
        let path = proc_root
            .join(pid.to_string())
            .join("task")
            .join(pid.to_string())
            .join("children");
        let Ok(raw) = fs::read_to_string(path) else {
            continue;
        };
        for token in raw
            .chars()
            .take(4096)
            .collect::<String>()
            .split_whitespace()
        {
            let Ok(child) = token.parse::<u32>() else {
                continue;
            };
            if child > 0 && found.insert(child) {
                pending.push_back(child);
            }
            if found.len() >= MAX_PROCESS_FAMILY {
                break;
            }
        }
    }
    found
}

pub fn tun_interfaces(sys_class_net: &Path, own_device: &str, running: bool) -> Vec<String> {
    let Ok(entries) = fs::read_dir(sys_class_net) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten().take(512) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if (running && name == own_device)
            || name.len() > 32
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
        {
            continue;
        }
        if entry.path().join("tun_flags").is_file() {
            found.push(name);
        }
    }
    found.sort();
    found.truncate(8);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn root(label: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "omavless-observe-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn family_walk_is_bounded_and_ignores_invalid_tokens() {
        let root = root("proc");
        for (pid, children) in [("10", "11 bad 12"), ("11", "13"), ("12", ""), ("13", "")] {
            let p = root.join(pid).join("task").join(pid);
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("children"), children).unwrap();
        }
        assert_eq!(process_family(10, &root), BTreeSet::from([10, 11, 12, 13]));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn tun_scan_excludes_owned_and_bounds_names() {
        let root = root("net");
        for name in ["Meta", "wg0", "ordinary"] {
            fs::create_dir(root.join(name)).unwrap();
        }
        fs::write(root.join("Meta/tun_flags"), "1").unwrap();
        fs::write(root.join("wg0/tun_flags"), "1").unwrap();
        assert_eq!(tun_interfaces(&root, "Meta", true), ["wg0"]);
        fs::remove_dir_all(root).unwrap();
    }
}
