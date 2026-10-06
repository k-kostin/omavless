//! Private same-owner gate. No daemon factory, IPC, bare executor extraction,
//! fresh host construction or accepted coverage constructor is exposed here.
use super::*;
use crate::lifecycle::{LifecycleError, LifecycleOutcome};
use crate::{RuntimeDispatcher, RuntimeServer};
use std::fs::{File, Metadata, OpenOptions};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};

/// Installed before any new lease acquisition. Every error/unwind retains the
/// whole original owner/singleton/lease graph; known Closed may retire normally.
struct Custody<T> {
    original: Option<T>,
}
impl<T> Drop for Custody<T> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            std::mem::forget(original);
        }
    }
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

struct Singleton {
    socket: File,
    socket_meta: Metadata,
    lock_meta: Metadata,
}
impl Singleton {
    fn capture(
        server: &RuntimeServer,
        paths: &CutoverPaths,
        uid: u32,
    ) -> Result<Self, LifecycleError> {
        let refuse = LifecycleError::ManualRecoveryRequired;
        if server.uid != uid
            || server.paths != RuntimePaths::below(&paths.runtime_base)
            || server
                .quit_requested
                .load(std::sync::atomic::Ordering::Acquire)
            || !server
                .dispatcher
                .lock()
                .is_ok_and(|d| matches!(*d, RuntimeDispatcher::ReadOnly))
        {
            return Err(refuse);
        }
        let socket = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_PATH | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(&server.paths.socket)
            .map_err(|_| refuse)?;
        let socket_meta = socket.metadata().map_err(|_| refuse)?;
        let lock_meta = server._owner._file.metadata().map_err(|_| refuse)?;
        if !socket_meta.file_type().is_socket()
            || !lock_meta.is_file()
            || [&socket_meta, &lock_meta]
                .into_iter()
                .any(|m| m.uid() != uid || m.nlink() != 1 || m.mode() & 0o7777 != 0o600)
        {
            return Err(refuse);
        }
        let this = Self {
            socket,
            socket_meta,
            lock_meta,
        };
        this.check(server)?;
        Ok(this)
    }
    fn check(&self, server: &RuntimeServer) -> Result<(), LifecycleError> {
        let refuse = LifecycleError::ManualRecoveryRequired;
        let socket = self.socket.metadata().map_err(|_| refuse)?;
        let named = std::fs::symlink_metadata(&server.paths.socket).map_err(|_| refuse)?;
        let lock = server._owner._file.metadata().map_err(|_| refuse)?;
        let named_lock = std::fs::symlink_metadata(&server.paths.owner_lock).map_err(|_| refuse)?;
        if !same(&self.socket_meta, &socket)
            || !same(&socket, &named)
            || !same(&self.lock_meta, &lock)
            || !same(&lock, &named_lock)
            || server
                .listener
                .local_addr()
                .ok()
                .and_then(|a| a.as_pathname().map(Path::to_path_buf))
                .as_deref()
                != Some(server.paths.socket.as_path())
        {
            return Err(refuse);
        }
        Ok(())
    }
}

impl ProductionNativeOwner<NativeLifecycleHost> {
    /// Only an explicit in-crate developer scenario may consume these originals.
    /// The closed issuer means current code refuses before validation or Arm.
    #[allow(dead_code)]
    pub(crate) fn protected_developer_roundtrip(
        self,
        server: RuntimeServer,
        profile: &str,
    ) -> Result<LifecycleOutcome, LifecycleError> {
        let mut custody = Custody {
            original: Some((self, server, None::<MigrationLock>)),
        };
        let (owner, server, lease) = custody
            .original
            .as_mut()
            .ok_or(LifecycleError::ManualRecoveryRequired)?;
        if !matches!(owner.ownership, ProductionOwnership::Committed { .. })
            || owner.actual() != ActualState::Disconnected
        {
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        let singleton = Singleton::capture(
            server,
            owner.coordinator.protected_native_paths(),
            owner.coordinator.uid(),
        )?;
        *lease = Some(owner.coordinator.protected_native_lock()?);
        let lock = lease
            .as_ref()
            .ok_or(LifecycleError::ManualRecoveryRequired)?;
        let result = owner
            .coordinator
            .protected_native_roundtrip(lock, profile, &mut || singleton.check(server))?;
        singleton.check(server)?;
        // Only a positively completed stop/empty/Disarm permits ordinary Drop.
        drop(custody.original.take());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    struct Count(Rc<Cell<u8>>);
    impl Drop for Count {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[test]
    fn original_graph_is_retained_on_return_and_unwind_only_closed_can_drop() {
        let dropped = Rc::new(Cell::new(0));
        drop(Custody {
            original: Some(Count(dropped.clone())),
        });
        assert_eq!(dropped.get(), 0);
        let count = dropped.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = Custody {
                original: Some(Count(count)),
            };
            panic!("fixed custody cut");
        }));
        assert_eq!(dropped.get(), 0);
        let mut guard = Custody {
            original: Some(Count(dropped.clone())),
        };
        drop(guard.original.take());
        assert_eq!(dropped.get(), 1);
    }
}

#[cfg(test)]
#[path = "protected_native_vm_tests.rs"]
mod native_vm_tests;
