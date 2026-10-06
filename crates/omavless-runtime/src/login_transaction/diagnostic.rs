//! Private test-only failure observation. No paths, payloads, errno numbers,
//! retries or authority. Only the original startup-check invocation activates it.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reason {
    NotEntered,
    NoFailure,
    StartupLock,
    Pending,
    ReceiptLock,
    MetadataBefore,
    MetadataUnsafe,
    MetadataAfter,
    MetadataChanged,
    StorePath,
    StoreOwner,
    StoreSize,
    StoreUtf8,
    StoreIo,
    Decode,
    Schema,
    Identity,
}
impl Reason {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::NoFailure => "no_failure",
            Self::StartupLock => "startup_lock",
            Self::Pending => "pending",
            Self::ReceiptLock => "receipt_lock",
            Self::MetadataBefore => "metadata_before",
            Self::MetadataUnsafe => "metadata_unsafe",
            Self::MetadataAfter => "metadata_after",
            Self::MetadataChanged => "metadata_changed",
            Self::StorePath => "store_path",
            Self::StoreOwner => "store_owner",
            Self::StoreSize => "store_size",
            Self::StoreUtf8 => "store_utf8",
            // The store API erases the underlying open/read errno. Do not
            // misrepresent this as EMFILE or perform another diagnostic read.
            Self::StoreIo => "store_io_opaque",
            Self::Decode => "decode",
            Self::Schema => "schema",
            Self::Identity => "phase_or_generation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Io {
    None,
    ProcessFdLimit,
    SystemFdLimit,
    Permission,
    Missing,
    Interrupted,
    Other,
}
impl Io {
    pub(crate) fn classify(error: &std::io::Error) -> Self {
        match error.raw_os_error() {
            Some(nix::libc::EMFILE) => Self::ProcessFdLimit,
            Some(nix::libc::ENFILE) => Self::SystemFdLimit,
            Some(nix::libc::EACCES | nix::libc::EPERM) => Self::Permission,
            Some(nix::libc::ENOENT) => Self::Missing,
            Some(nix::libc::EINTR) => Self::Interrupted,
            _ => Self::Other,
        }
    }
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ProcessFdLimit => "process_fd_limit",
            Self::SystemFdLimit => "system_fd_limit",
            Self::Permission => "permission",
            Self::Missing => "missing",
            Self::Interrupted => "interrupted",
            Self::Other => "other",
        }
    }
}

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static LAST: Cell<(Reason, Io)> = const { Cell::new((Reason::NotEntered, Io::None)) };
}
pub(crate) struct Observation;
impl Drop for Observation {
    fn drop(&mut self) {
        ACTIVE.set(false);
    }
}
pub(crate) fn begin() -> Observation {
    LAST.set((Reason::NoFailure, Io::None));
    ACTIVE.set(true);
    Observation
}
pub(crate) fn reset() {
    ACTIVE.set(false);
    LAST.set((Reason::NotEntered, Io::None));
}
pub(crate) fn last() -> (Reason, Io) {
    LAST.get()
}
pub(crate) fn record(reason: Reason, io: Io) {
    if ACTIVE.get() && LAST.get().0 == Reason::NoFailure {
        LAST.set((reason, io));
    }
}
pub(crate) fn store_failure(error: omavless_store::StoreIoError) {
    use omavless_store::StoreIoError;
    record(
        match error {
            StoreIoError::UnsafePath => Reason::StorePath,
            StoreIoError::WrongOwner => Reason::StoreOwner,
            StoreIoError::TooLarge => Reason::StoreSize,
            StoreIoError::InvalidUtf8 => Reason::StoreUtf8,
            StoreIoError::Io => Reason::StoreIo,
        },
        Io::None,
    );
}

#[test]
fn labels_are_closed_first_failure_scoped_resettable_and_thread_local() {
    reset();
    record(Reason::Decode, Io::None);
    assert_eq!(last(), (Reason::NotEntered, Io::None));
    {
        let _observation = begin();
        record(Reason::MetadataBefore, Io::ProcessFdLimit);
        record(Reason::Decode, Io::None);
        assert_eq!(last(), (Reason::MetadataBefore, Io::ProcessFdLimit));
        std::thread::spawn(|| assert_eq!(last(), (Reason::NotEntered, Io::None)))
            .join()
            .unwrap();
    }
    record(Reason::Decode, Io::None);
    assert_eq!(last(), (Reason::MetadataBefore, Io::ProcessFdLimit));
    assert_eq!(
        Io::classify(&std::io::Error::from_raw_os_error(nix::libc::EMFILE)),
        Io::ProcessFdLimit
    );
    assert_eq!(
        Io::classify(&std::io::Error::from_raw_os_error(nix::libc::ENFILE)),
        Io::SystemFdLimit
    );
    assert_eq!(
        Io::classify(&std::io::Error::other("private text")),
        Io::Other
    );
    reset();
    assert_eq!(last(), (Reason::NotEntered, Io::None));
    let _ = std::panic::catch_unwind(|| {
        let _observation = begin();
        record(Reason::Pending, Io::None);
        panic!("synthetic");
    });
    record(Reason::Decode, Io::None);
    assert_eq!(last(), (Reason::Pending, Io::None));
}

#[test]
fn diagnostic_tokens_and_original_read_call_sites_are_finite() {
    assert_eq!(
        [
            Reason::NotEntered,
            Reason::NoFailure,
            Reason::StartupLock,
            Reason::Pending,
            Reason::ReceiptLock,
            Reason::MetadataBefore,
            Reason::MetadataUnsafe,
            Reason::MetadataAfter,
            Reason::MetadataChanged,
            Reason::StorePath,
            Reason::StoreOwner,
            Reason::StoreSize,
            Reason::StoreUtf8,
            Reason::StoreIo,
            Reason::Decode,
            Reason::Schema,
            Reason::Identity
        ]
        .map(Reason::token),
        [
            "not_entered",
            "no_failure",
            "startup_lock",
            "pending",
            "receipt_lock",
            "metadata_before",
            "metadata_unsafe",
            "metadata_after",
            "metadata_changed",
            "store_path",
            "store_owner",
            "store_size",
            "store_utf8",
            "store_io_opaque",
            "decode",
            "schema",
            "phase_or_generation"
        ]
    );
    assert_eq!(
        [
            Io::None,
            Io::ProcessFdLimit,
            Io::SystemFdLimit,
            Io::Permission,
            Io::Missing,
            Io::Interrupted,
            Io::Other
        ]
        .map(Io::token),
        [
            "none",
            "process_fd_limit",
            "system_fd_limit",
            "permission",
            "missing",
            "interrupted",
            "other"
        ]
    );
    let source = include_str!("../login_transaction.rs");
    let reader = source
        .split("fn private_optional(")
        .nth(1)
        .unwrap()
        .split("\nfn required(")
        .next()
        .unwrap();
    assert_eq!(reader.matches("fs::symlink_metadata(path)").count(), 2);
    assert_eq!(reader.matches("read_private_utf8(path, uid)").count(), 1);
    let startup = source
        .split("pub(crate) fn check_startup_receipt(")
        .nth(1)
        .unwrap()
        .split("/// Restore recovery")
        .next()
        .unwrap();
    let checks = [
        "lock.authorizes(paths, uid)",
        "pending_private_transaction::pending_at(&paths.state_directory)",
        "check_login_receipt_without_private_fence(paths, uid, lock, committed_generation)",
    ];
    let mut end = 0;
    for check in checks {
        assert_eq!(startup.matches(check).count(), 1);
        let at = startup.find(check).unwrap();
        assert!(at > end);
        end = at;
    }
}
