// SPDX-License-Identifier: MIT
//! Developer-only seccomp discrimination probe, never namespace authority.
//! Only setns on a verified character-device /dev/null descriptor is attempted.
//! No valid namespace descriptor, unshare, socket or network operation exists.
use nix::{
    errno::Errno,
    sched::{CloneFlags, setns},
    unistd::geteuid,
};
use std::{
    fs::File,
    os::unix::fs::{FileTypeExt, MetadataExt},
};

fn discriminate(actual: nix::Result<()>, filtered: bool) -> bool {
    actual
        == Err(if filtered {
            Errno::EPERM
        } else {
            Errno::EINVAL
        })
}

fn probe(filtered: bool) -> bool {
    let Ok(file) = File::open("/dev/null") else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    // A substituted regular file or namespace/pidfd must never reach setns.
    if !meta.file_type().is_char_device()
        || nix::sys::stat::major(meta.rdev()) != 1
        || nix::sys::stat::minor(meta.rdev()) != 3
    {
        return false;
    }
    discriminate(setns(&file, CloneFlags::empty()), filtered)
}

fn main() {
    let mode = std::env::var("OMAVLESS_K1_NAMESPACE_FILTER_FIXTURE");
    let filtered = match mode.as_deref() {
        Ok("filtered") => true,
        Ok("control") => false,
        _ => std::process::exit(2),
    };
    if std::env::args_os().len() != 1 || !geteuid().is_root() || !probe(filtered) {
        println!("K1_NAMESPACE_FILTER_REFUSED");
        std::process::exit(2);
    }
    println!("K1_NAMESPACE_FILTER_CASE_PASS");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_and_filter_cannot_accept_each_others_result() {
        assert!(discriminate(Err(Errno::EPERM), true));
        assert!(discriminate(Err(Errno::EINVAL), false));
        for filtered in [false, true] {
            for actual in [Ok(()), Err(Errno::EBADF), Err(Errno::ENOSYS)] {
                assert!(!discriminate(actual, filtered));
            }
        }
        assert!(!discriminate(Err(Errno::EPERM), false));
        assert!(!discriminate(Err(Errno::EINVAL), true));
    }
}
