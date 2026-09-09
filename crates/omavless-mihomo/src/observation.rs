// SPDX-License-Identifier: MIT

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// Incomplete or unsafe host inventory. No observed names or paths are exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrictObservationError;

impl std::fmt::Display for StrictObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Host inventory could not be verified")
    }
}
impl std::error::Error for StrictObservationError {}

type StrictResult<T> = Result<T, StrictObservationError>;

fn fixed_process_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// Unlike the tolerant display projection, every incomplete scan is an error.
/// An empty result proves only this bounded observation, not atomic host state.
pub fn processes_named_strict(proc_root: &Path, name: &str) -> StrictResult<BTreeSet<u32>> {
    processes_named_strict_bounded(proc_root, name, 65_536, MAX_NAMED_PROCESSES)
}

fn processes_named_strict_bounded(
    proc_root: &Path,
    name: &str,
    entries_limit: usize,
    matches_limit: usize,
) -> StrictResult<BTreeSet<u32>> {
    if !fixed_process_name(name) {
        return Err(StrictObservationError);
    }
    let root = fs::symlink_metadata(proc_root).map_err(|_| StrictObservationError)?;
    if !root.is_dir() {
        return Err(StrictObservationError);
    }
    let mut found = BTreeSet::new();
    for (index, entry) in fs::read_dir(proc_root)
        .map_err(|_| StrictObservationError)?
        .enumerate()
    {
        if index >= entries_limit {
            return Err(StrictObservationError);
        }
        let entry = entry.map_err(|_| StrictObservationError)?;
        let raw = entry.file_name();
        let bytes = raw.as_encoded_bytes();
        if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
            continue;
        }
        let text = raw.to_str().ok_or(StrictObservationError)?;
        let pid = text.parse::<u32>().map_err(|_| StrictObservationError)?;
        if pid == 0 || pid.to_string() != text {
            return Err(StrictObservationError);
        }
        let directory = entry.path();
        let before_dir = fs::symlink_metadata(&directory).map_err(|_| StrictObservationError)?;
        if !before_dir.is_dir() {
            return Err(StrictObservationError);
        }
        let path = directory.join("comm");
        let before = fs::symlink_metadata(&path).map_err(|_| StrictObservationError)?;
        if !before.is_file() {
            return Err(StrictObservationError);
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| StrictObservationError)?;
        let opened = file.metadata().map_err(|_| StrictObservationError)?;
        if !opened.is_file() || opened.dev() != before.dev() || opened.ino() != before.ino() {
            return Err(StrictObservationError);
        }
        let mut raw = Vec::new();
        file.take(65)
            .read_to_end(&mut raw)
            .map_err(|_| StrictObservationError)?;
        if raw.len() > 64 || raw.contains(&0) {
            return Err(StrictObservationError);
        }
        // Linux comm is a byte string, not necessarily UTF-8 or nonempty.
        // Unrelated valid names must not make safe inventory unavailable.
        let comm = raw.strip_suffix(b"\n").ok_or(StrictObservationError)?;
        let after_dir = fs::symlink_metadata(&directory).map_err(|_| StrictObservationError)?;
        let after = fs::symlink_metadata(&path).map_err(|_| StrictObservationError)?;
        if !after_dir.is_dir()
            || after_dir.dev() != before_dir.dev()
            || after_dir.ino() != before_dir.ino()
            || !after.is_file()
            || after.dev() != opened.dev()
            || after.ino() != opened.ino()
        {
            return Err(StrictObservationError);
        }
        if comm == name.as_bytes() {
            found.insert(pid);
            if found.len() > matches_limit {
                return Err(StrictObservationError);
            }
        }
    }
    Ok(found)
}

/// Fail-closed TUN count. Real sysfs interface symlinks are followed, but broken
/// targets, unreadable entries and overflow are never reported as zero.
pub fn tun_interface_count_strict(sys_class_net: &Path) -> StrictResult<u8> {
    tun_interface_count_strict_bounded(sys_class_net, 512, 8)
}

fn tun_interface_count_strict_bounded(
    root: &Path,
    entries_limit: usize,
    count_limit: u8,
) -> StrictResult<u8> {
    if !fs::metadata(root)
        .map_err(|_| StrictObservationError)?
        .is_dir()
    {
        return Err(StrictObservationError);
    }
    let mut count = 0u8;
    for (index, entry) in fs::read_dir(root)
        .map_err(|_| StrictObservationError)?
        .enumerate()
    {
        if index >= entries_limit {
            return Err(StrictObservationError);
        }
        let path = entry.map_err(|_| StrictObservationError)?.path();
        let before = fs::metadata(&path).map_err(|_| StrictObservationError)?;
        if !before.is_dir() {
            return Err(StrictObservationError);
        }
        match fs::symlink_metadata(path.join("tun_flags")) {
            Ok(flags) if flags.is_file() => {
                let file = fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
                    .open(path.join("tun_flags"))
                    .map_err(|_| StrictObservationError)?;
                let opened = file.metadata().map_err(|_| StrictObservationError)?;
                if !opened.is_file() || opened.dev() != flags.dev() || opened.ino() != flags.ino() {
                    return Err(StrictObservationError);
                }
                let mut raw = String::new();
                file.take(33)
                    .read_to_string(&mut raw)
                    .map_err(|_| StrictObservationError)?;
                if raw.len() > 32 {
                    return Err(StrictObservationError);
                }
                let value = raw.trim();
                let parsed = if let Some(hex) = value.strip_prefix("0x") {
                    u32::from_str_radix(hex, 16)
                } else {
                    value.parse::<u32>()
                };
                if parsed.is_err() {
                    return Err(StrictObservationError);
                }
                count = count.checked_add(1).ok_or(StrictObservationError)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(StrictObservationError),
        }
        let after = fs::metadata(&path).map_err(|_| StrictObservationError)?;
        if !after.is_dir()
            || before.dev() != after.dev()
            || before.ino() != after.ino()
            || count > count_limit
        {
            return Err(StrictObservationError);
        }
    }
    Ok(count)
}

#[cfg(test)]
#[path = "observation_strict_tests.rs"]
mod strict_tests;

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
    fn root(_label: &str) -> std::path::PathBuf {
        crate::test_temp::directory("observe").unwrap()
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
