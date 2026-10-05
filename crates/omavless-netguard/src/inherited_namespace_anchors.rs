// SPDX-License-Identifier: MIT
//! Fixed manager-delivered namespace aliases, not environment authentication.
//! Import happens before bus/threads/other FD allocations. Source slots 0,3,4,5
//! stay open, CLOEXEC and unexported for this process lifetime; application code
//! never adopts/closes/replaces them. Owned aliases preserve the SAME OFDs.
use crate::effect_port::EffectError;
use nix::dir::Dir;
use nix::fcntl::{FcntlArg, FdFlag, OFlag, fcntl};
use nix::sys::stat::Mode;
use nix_netguard::sys::nsfs::{
    NamespaceType, duplicate_inherited_cloexec, namespace_id, namespace_type,
};
use std::fs::File;
use std::os::fd::{AsFd, AsRawFd};

const REFUSE: EffectError = EffectError::UnavailableOrUncertain;
type Result<T> = std::result::Result<T, EffectError>;
pub(crate) const OPEN_FILES: [(&str, &str, u64); 3] = [
    ("/proc/1/ns/user", "k1-manager-userns", 1),
    ("/proc/1/ns/mnt", "k1-manager-mntns", 1),
    ("/proc/1/ns/pid", "k1-manager-pidns", 1),
];
fn require(value: bool) -> Result<()> {
    if value { Ok(()) } else { Err(REFUSE) }
}

fn admitted_metadata(pid: &str, count: &str, names: &str, current: u32) -> bool {
    pid == current.to_string()
        && count == "3"
        && names == "k1-manager-userns:k1-manager-mntns:k1-manager-pidns"
}
fn fixed_environment(key: &str) -> Result<String> {
    let value = std::env::var(key).map_err(|_| REFUSE)?;
    require(value.len() <= 128)?;
    Ok(value)
}
fn admitted_slots(slots: &mut [i32], directory: i32) -> bool {
    slots.sort_unstable();
    directory >= 6 && slots == [0, 1, 2, 3, 4, 5]
}
fn namespace_match(
    kind: NamespaceType,
    manager: NamespaceType,
    current: NamespaceType,
    manager_id: u64,
    current_id: u64,
) -> bool {
    manager == kind && current == kind && manager_id != 0 && manager_id == current_id
}
fn original_slots_only() -> Result<()> {
    // The inventory directory is the only temporary FD permitted here. If an
    // inherited slot is missing, opening the directory into it cannot repair
    // admission: its own descriptor MUST lie above the fixed source slots.
    let mut directory = Dir::open(
        "/proc/self/fd",
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| REFUSE)?;
    let own = directory.as_raw_fd();
    require(own >= 6)?;
    let mut slots = Vec::new();
    for entry in directory.iter() {
        let entry = entry.map_err(|_| REFUSE)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        require(slots.len() < 16)?;
        let name = std::str::from_utf8(name).map_err(|_| REFUSE)?;
        let slot = name.parse::<i32>().map_err(|_| REFUSE)?;
        require(slot >= 0 && name == slot.to_string())?;
        if slot != own {
            slots.push(slot);
        }
    }
    require(admitted_slots(&mut slots, own))
}

pub(crate) struct ManagerNamespaceAnchors {
    user: File,
    mount: File,
    pid: File,
}
impl ManagerNamespaceAnchors {
    pub(crate) fn acquire() -> Result<(File, Self)> {
        // Metadata is consistency only, never trusted-manager provenance. No
        // name/number/path can authorize an arbitrary root invocation.
        require(admitted_metadata(
            &fixed_environment("LISTEN_PID")?,
            &fixed_environment("LISTEN_FDS")?,
            &fixed_environment("LISTEN_FDNAMES")?,
            std::process::id(),
        ))?;
        original_slots_only()?;
        // The minimum is ABOVE every source slot. A missing source cannot be
        // occupied by an earlier duplicate and masquerade as handed-off input.
        let network = File::from(duplicate_inherited_cloexec(0, 6).map_err(|_| REFUSE)?);
        let user = File::from(duplicate_inherited_cloexec(3, 6).map_err(|_| REFUSE)?);
        let mount = File::from(duplicate_inherited_cloexec(4, 6).map_err(|_| REFUSE)?);
        let pid = File::from(duplicate_inherited_cloexec(5, 6).map_err(|_| REFUSE)?);
        require(namespace_type(network.as_fd()).map_err(|_| REFUSE)? == NamespaceType::Network)?;
        let anchors = Self { user, mount, pid };
        anchors.recheck()?;
        Ok((network, anchors))
    }

    pub(crate) fn recheck(&self) -> Result<()> {
        for (manager, path, kind) in [
            (&self.user, "/proc/thread-self/ns/user", NamespaceType::User),
            (
                &self.mount,
                "/proc/thread-self/ns/mnt",
                NamespaceType::Mount,
            ),
            (&self.pid, "/proc/thread-self/ns/pid", NamespaceType::Pid),
        ] {
            require(
                fcntl(manager, FcntlArg::F_GETFD).map_err(|_| REFUSE)? == FdFlag::FD_CLOEXEC.bits(),
            )?;
            let current = File::open(path).map_err(|_| REFUSE)?;
            require(namespace_match(
                kind,
                namespace_type(manager.as_fd()).map_err(|_| REFUSE)?,
                namespace_type(current.as_fd()).map_err(|_| REFUSE)?,
                namespace_id(manager.as_fd()).map_err(|_| REFUSE)?,
                namespace_id(current.as_fd()).map_err(|_| REFUSE)?,
            ))?;
        }
        Ok(())
    }
}

pub(crate) fn admitted_open_files(values: &[(String, String, u64)]) -> bool {
    values.len() == OPEN_FILES.len()
        && values.iter().zip(OPEN_FILES).all(|(actual, expected)| {
            actual.0 == expected.0 && actual.1 == expected.1 && actual.2 == expected.2
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_kind_and_nonzero_full_id_equality_are_mandatory() {
        for kind in [
            NamespaceType::User,
            NamespaceType::Mount,
            NamespaceType::Pid,
        ] {
            assert!(namespace_match(kind, kind, kind, u64::MAX, u64::MAX));
            assert!(!namespace_match(kind, NamespaceType::Network, kind, 41, 41));
            assert!(!namespace_match(kind, kind, NamespaceType::Network, 41, 41));
            assert!(!namespace_match(kind, kind, kind, 0, 0));
            assert!(!namespace_match(kind, kind, kind, 41, 42));
        }
    }
    #[test]
    fn inherited_metadata_missing_extra_unknown_order_and_foreign_pid_refuse() {
        let names = "k1-manager-userns:k1-manager-mntns:k1-manager-pidns";
        assert!(admitted_metadata("41", "3", names, 41));
        for (pid, count, names) in [
            ("", "3", names),
            ("041", "3", names),
            ("42", "3", names),
            ("41", "", names),
            ("41", "4", names),
            ("41", "03", names),
            ("41", "3", ""),
            ("41", "3", "k1-manager-userns:k1-manager-mntns"),
            (
                "41",
                "3",
                "k1-manager-userns:k1-manager-mntns:k1-manager-pidns:extra",
            ),
            (
                "41",
                "3",
                "k1-manager-pidns:k1-manager-mntns:k1-manager-userns",
            ),
        ] {
            assert!(!admitted_metadata(pid, count, names, 41));
        }
    }
    #[test]
    fn inherited_directory_cannot_fill_missing_source_or_hide_extra_slots() {
        assert!(admitted_slots(&mut [5, 3, 0, 4, 1, 2], 6));
        for mut slots in [
            vec![0, 1, 2, 3, 4],
            vec![0, 1, 2, 3, 4, 5, 7],
            vec![0, 1, 2, 3, 3, 5],
            vec![0, 1, 2, 3, 4, 6],
        ] {
            assert!(!admitted_slots(&mut slots, 6));
        }
        assert!(!admitted_slots(&mut [0, 1, 2, 3, 4, 5], 4));
    }
    #[test]
    fn inherited_effective_open_files_require_whole_fixed_readonly_catalogue() {
        let values: Vec<_> = OPEN_FILES
            .into_iter()
            .map(|(p, n, f)| (p.into(), n.into(), f))
            .collect();
        assert!(admitted_open_files(&values));
        for index in 0..3 {
            let mut changed = values.clone();
            changed[index].2 = 9;
            assert!(!admitted_open_files(&changed));
            changed = values.clone();
            changed[index].0 = "/proc/self/ns/user".into();
            assert!(!admitted_open_files(&changed));
        }
        let mut extra = values.clone();
        extra.push(values[0].clone());
        assert!(!admitted_open_files(&extra));
        assert!(!admitted_open_files(&values[..2]));
    }
}
