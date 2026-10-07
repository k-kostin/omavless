// SPDX-License-Identifier: MIT
//! Standalone developer-only systemd OpenFile experiment, not a product API.
//! Never opens a socket or changes namespaces, files, policy or network state.
//! DescriptorMatch proves only agreement in a controlled launch: NOT kernel
//! namespace type/ID, socket cookie, canonical host or nft ownership.

use nix::sys::statfs::{NSFS_MAGIC, PROC_SUPER_MAGIC, fstatfs};
use nix::unistd::geteuid;
use std::fs::File;
use std::os::unix::fs::MetadataExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DescriptorIdentity {
    device: u64,
    inode: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct DescriptorMatch;
#[derive(Debug, PartialEq, Eq)]
struct Refused;

fn namespace_descriptor_identity(file: &File) -> Result<DescriptorIdentity, Refused> {
    // NSFS_MAGIC alone does not distinguish net from PID/mount/user namespace.
    if fstatfs(file).map_err(|_| Refused)?.filesystem_type() != NSFS_MAGIC {
        return Err(Refused);
    }
    let metadata = file.metadata().map_err(|_| Refused)?;
    if metadata.ino() == 0 {
        return Err(Refused);
    }
    Ok(DescriptorIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn compare(
    inherited_before: DescriptorIdentity,
    current_before: DescriptorIdentity,
    inherited_after: DescriptorIdentity,
    current_after: DescriptorIdentity,
) -> Result<DescriptorMatch, Refused> {
    if inherited_before == current_before
        && inherited_before == inherited_after
        && current_before == current_after
    {
        Ok(DescriptorMatch)
    } else {
        Err(Refused)
    }
}

fn inspect_with_checkpoint(checkpoint: impl FnOnce()) -> Result<DescriptorMatch, Refused> {
    // Open FD3 FIRST: a missing inherited FD must not be filled by our procfs
    // opener. Reopening procfs's fixed magic link safely retains its target;
    // no BorrowedFd::borrow_raw/from_raw_fd or unsafe syscall binding is used.
    let inherited = File::open("/proc/self/fd/3").map_err(|_| Refused)?;
    let before = namespace_descriptor_identity(&inherited)?;
    let proc = File::open("/proc/thread-self/ns").map_err(|_| Refused)?;
    if fstatfs(&proc).map_err(|_| Refused)?.filesystem_type() != PROC_SUPER_MAGIC {
        return Err(Refused);
    }
    let current = File::open("/proc/thread-self/ns/net").map_err(|_| Refused)?;
    let current_before = namespace_descriptor_identity(&current)?;
    checkpoint();
    let inherited_again = File::open("/proc/self/fd/3").map_err(|_| Refused)?;
    let current_again = File::open("/proc/thread-self/ns/net").map_err(|_| Refused)?;
    if namespace_descriptor_identity(&inherited)? != before
        || namespace_descriptor_identity(&current)? != current_before
    {
        return Err(Refused);
    }
    compare(
        before,
        current_before,
        namespace_descriptor_identity(&inherited_again)?,
        namespace_descriptor_identity(&current_again)?,
    )
}

fn main() {
    // An accidental invocation refuses. Neither this opt-in nor UID zero is
    // host provenance. LISTEN_PID/FDS/FDNAMES are deliberately not authority.
    let result = if std::env::args_os().len() == 1
        && std::env::var("OMAVLESS_K1_OPENFILE_FIXTURE").as_deref() == Ok("1")
        && geteuid().is_root()
    {
        inspect_with_checkpoint(|| {})
    } else {
        Err(Refused)
    };
    match result {
        Ok(DescriptorMatch) => println!("K1_OPENFILE_DESCRIPTOR_MATCH"),
        Err(Refused) => {
            println!("K1_OPENFILE_REFUSED");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_descriptor_agreement_only() {
        let a = DescriptorIdentity {
            device: 1,
            inode: 7,
        };
        let b = DescriptorIdentity {
            device: 1,
            inode: 8,
        };
        let c = DescriptorIdentity {
            device: 2,
            inode: 7,
        };
        assert_eq!(compare(a, a, a, a), Ok(DescriptorMatch));
        for different in [b, c] {
            assert_eq!(compare(a, different, a, different), Err(Refused));
            assert_eq!(compare(a, a, different, a), Err(Refused));
            assert_eq!(compare(a, a, a, different), Err(Refused));
            assert_eq!(compare(a, a, different, different), Err(Refused));
        }
    }

    #[test]
    fn ordinary_descriptor_is_not_nsfs() {
        assert_eq!(
            namespace_descriptor_identity(&File::open("/dev/null").unwrap()),
            Err(Refused)
        );
    }
}
