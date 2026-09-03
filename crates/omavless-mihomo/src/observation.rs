// SPDX-License-Identifier: MIT

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::Path;

pub const MAX_PROCESS_FAMILY: usize = 64;
pub const MAX_NAMED_PROCESSES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserServiceState {
    pub active: bool,
    pub main_pid: u32,
    pub exit_status: i32,
    pub result: String,
}

pub fn parse_systemd_show(value: &str) -> Option<UserServiceState> {
    if value.len() > 64 * 1024 {
        return None;
    }
    let (mut active, mut main_pid, mut exit_status, mut result) = (None, None, None, None);
    for line in value.lines() {
        let (key, item) = line.split_once('=')?;
        match key {
            "ActiveState" => {
                active = Some(match item {
                    "inactive" | "failed" => false,
                    "active" | "activating" | "deactivating" | "reloading" | "maintenance"
                    | "refreshing" => true,
                    _ => return None,
                })
            }
            "MainPID" => main_pid = item.parse().ok(),
            "ExecMainStatus" => exit_status = item.parse().ok(),
            "Result"
                if item.len() <= 80
                    && item
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)) =>
            {
                result = Some(item.to_owned())
            }
            _ => {}
        }
    }
    Some(UserServiceState {
        active: active?,
        main_pid: main_pid?,
        exit_status: exit_status?,
        result: result?,
    })
}

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

/// Return a bounded set of processes with one exact Linux `comm` name.
///
/// The name is supplied by trusted host policy, not IPC. Individual files are
/// capped before parsing so a synthetic or damaged procfs cannot allocate
/// unbounded memory.
pub fn processes_named(proc_root: &Path, name: &str) -> BTreeSet<u32> {
    if name.is_empty()
        || name.len() > 15
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return BTreeSet::new();
    }
    let Ok(entries) = fs::read_dir(proc_root) else {
        return BTreeSet::new();
    };
    let mut found = BTreeSet::new();
    for entry in entries.flatten().take(65_536) {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|pid| *pid > 0)
        else {
            continue;
        };
        let Ok(file) = fs::File::open(entry.path().join("comm")) else {
            continue;
        };
        let mut raw = String::new();
        if file.take(64).read_to_string(&mut raw).is_ok() && raw.trim_end() == name {
            found.insert(pid);
        }
        if found.len() >= MAX_NAMED_PROCESSES {
            break;
        }
    }
    found
}

/// Count all visible TUN devices, capped above the healthy singleton value.
#[must_use]
pub fn tun_interface_count(sys_class_net: &Path) -> u8 {
    let Ok(entries) = fs::read_dir(sys_class_net) else {
        return 0;
    };
    let mut count = 0_u8;
    for entry in entries.flatten().take(512) {
        if entry.path().join("tun_flags").is_file() {
            count = count.saturating_add(1);
            if count >= 8 {
                break;
            }
        }
    }
    count
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
    fn exact_process_name_scan_is_bounded_and_ignores_untrusted_names() {
        let root = root("named");
        for (pid, name) in [
            ("10", "mihomo\n"),
            ("11", "mihomo-helper\n"),
            ("12", "mihomo\n"),
        ] {
            let path = root.join(pid);
            fs::create_dir(&path).unwrap();
            fs::write(path.join("comm"), name).unwrap();
        }
        assert_eq!(processes_named(&root, "mihomo"), BTreeSet::from([10, 12]));
        assert!(processes_named(&root, "../mihomo").is_empty());
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
        assert_eq!(tun_interface_count(&root), 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn systemd_projection_is_bounded_and_typed() {
        let state = parse_systemd_show(
            "ActiveState=active\nMainPID=42\nExecMainStatus=0\nResult=success\n",
        )
        .unwrap();
        assert!(state.active);
        assert!(
            parse_systemd_show(
                "ActiveState=activating\nMainPID=0\nExecMainStatus=0\nResult=success\n"
            )
            .unwrap()
            .active
        );
        assert!(
            parse_systemd_show(
                "ActiveState=private\nMainPID=0\nExecMainStatus=0\nResult=success\n"
            )
            .is_none()
        );
        assert_eq!(state.main_pid, 42);
        assert!(
            parse_systemd_show(
                "ActiveState=active\nMainPID=private\nExecMainStatus=0\nResult=success\n"
            )
            .is_none()
        );
    }
}
