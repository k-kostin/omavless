use super::*;
use std::cell::{Cell, RefCell};
use std::io::Read;
use std::os::unix::net::UnixStream;

fn fd() -> OwnedFd {
    UnixStream::pair().unwrap().0.into()
}

struct Fake {
    calls: RefCell<Vec<&'static str>>,
    fail: Cell<Option<&'static str>>,
    flags: Cell<u16>,
    name: Cell<[u8; 16]>,
    index: Cell<u32>,
    namespace_mismatch: Cell<bool>,
    namespace_calls: Cell<u32>,
}

impl Default for Fake {
    fn default() -> Self {
        let mut name = [0; 16];
        name[..4].copy_from_slice(b"Meta");
        Self {
            calls: RefCell::new(Vec::new()),
            fail: Cell::new(None),
            flags: Cell::new(0x1001),
            name: Cell::new(name),
            index: Cell::new(7),
            namespace_mismatch: Cell::new(false),
            namespace_calls: Cell::new(0),
        }
    }
}

impl Fake {
    fn step(&self, name: &'static str) -> Result<(), Error> {
        self.calls.borrow_mut().push(name);
        if self.fail.get() == Some(name) {
            Err(Error::KernelUnavailable)
        } else {
            Ok(())
        }
    }
}

impl Kernel for Fake {
    fn current_namespace(&self) -> Result<OwnedFd, Error> {
        self.step("current")?;
        Ok(fd())
    }
    fn namespace_id(&self, _: &OwnedFd) -> Result<NamespaceId, Error> {
        self.step("namespace_id")?;
        let count = self.namespace_calls.get();
        self.namespace_calls.set(count + 1);
        Ok(NamespaceId {
            device: 7,
            inode: if self.namespace_mismatch.get() {
                u64::from(count)
            } else {
                9
            },
        })
    }
    fn device(&self, _: &OwnedFd) -> Result<(), Error> {
        self.step("device")
    }
    fn info(&self, _: &OwnedFd) -> Result<InterfaceInfo, Error> {
        self.step("info")?;
        Ok(InterfaceInfo {
            name: self.name.get(),
            flags: self.flags.get(),
        })
    }
    fn device_namespace(&self, _: &OwnedFd) -> Result<OwnedFd, Error> {
        self.step("device_namespace")?;
        Ok(fd())
    }
    fn query_socket(&self) -> Result<OwnedFd, Error> {
        self.step("socket")?;
        Ok(fd())
    }
    fn index(&self, _: &OwnedFd) -> Result<u32, Error> {
        self.step("index")?;
        Ok(self.index.get())
    }
}

#[test]
fn admitted_object_is_retained_until_drop() {
    let (input, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let held = HeldTun::admit_with(input.into(), &Fake::default()).unwrap();
    assert_eq!(held.interface_index(), 7);
    assert_eq!(
        peer.read(&mut [0]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(format!("{held:?}"), "HeldTun { .. }");
    drop(held);
    assert_eq!(peer.read(&mut [0]).unwrap(), 0);
}

#[test]
fn failures_close_received_descriptor_at_every_stage() {
    for stage in [
        "device",
        "current",
        "info",
        "device_namespace",
        "namespace_id",
        "socket",
        "index",
    ] {
        let kernel = Fake::default();
        kernel.fail.set(Some(stage));
        let (input, mut peer) = UnixStream::pair().unwrap();
        assert!(HeldTun::admit_with(input.into(), &kernel).is_err());
        peer.set_nonblocking(true).unwrap();
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
    }
}

#[test]
fn driver_identity_precedes_any_ioctl_or_namespace_access() {
    let kernel = Fake::default();
    kernel.fail.set(Some("device"));
    assert!(HeldTun::admit_with(fd(), &kernel).is_err());
    assert_eq!(*kernel.calls.borrow(), ["device"]);
}

#[test]
fn refuses_wrong_type_multiqueue_persistence_and_detachment() {
    for flags in [0, 2, 3, 0x1002, 0x1101, 0x1201, 0x1401, 0x1801] {
        let kernel = Fake::default();
        kernel.flags.set(flags);
        assert_eq!(
            HeldTun::admit_with(fd(), &kernel).unwrap_err(),
            Error::InvalidTun
        );
        assert!(!kernel.calls.borrow().contains(&"device_namespace"));
    }
}

#[test]
fn name_requires_exact_fixed_value_and_termination() {
    for name in [*b"MetaX\0\0\0\0\0\0\0\0\0\0\0", [b'A'; 16], [0; 16]] {
        let kernel = Fake::default();
        kernel.name.set(name);
        assert_eq!(
            HeldTun::admit_with(fd(), &kernel).unwrap_err(),
            Error::InvalidTun
        );
    }
}

#[test]
fn no_pi_is_not_claimed_from_ambiguous_nofilter_bit() {
    let kernel = Fake::default();
    kernel.flags.set(1);
    assert!(HeldTun::admit_with(fd(), &kernel).is_ok());
}

#[test]
fn foreign_namespace_is_refused_before_name_lookup() {
    let kernel = Fake::default();
    kernel.namespace_mismatch.set(true);
    assert_eq!(
        HeldTun::admit_with(fd(), &kernel).unwrap_err(),
        Error::NamespaceMismatch
    );
    assert!(!kernel.calls.borrow().contains(&"index"));
}

#[test]
fn indices_are_positive_and_fit_resolved_signed_type() {
    for index in [0, i32::MAX as u32 + 1, u32::MAX] {
        let kernel = Fake::default();
        kernel.index.set(index);
        assert_eq!(
            HeldTun::admit_with(fd(), &kernel).unwrap_err(),
            Error::InvalidTun
        );
    }
}

#[test]
fn every_effect_recheck_refuses_changed_kernel_facts() {
    for changed in ["namespace", "index", "flags", "name", "device_namespace"] {
        let kernel = Fake::default();
        let held = HeldTun::admit_with(fd(), &kernel).unwrap();
        match changed {
            "namespace" => kernel.namespace_mismatch.set(true),
            "index" => kernel.index.set(8),
            "flags" => kernel.flags.set(0x1801),
            "name" => kernel.name.set([0; 16]),
            _ => kernel.fail.set(Some("device_namespace")),
        }
        assert!(held.recheck_with(&kernel).is_err());
    }
}

#[test]
fn public_errors_never_include_kernel_or_descriptor_data() {
    for error in [
        Error::UnsupportedPlatform,
        Error::InvalidDescriptor,
        Error::InvalidTun,
        Error::NamespaceMismatch,
        Error::Changed,
        Error::KernelUnavailable,
    ] {
        let text = error.to_string();
        assert!(text.is_ascii() && text.len() < 100);
        assert!(!text.contains("Meta") && !text.contains("/proc") && !text.contains("fd="));
    }
}

#[test]
fn actual_regular_file_and_socket_refuse_without_tun_access() {
    let file = std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    assert_eq!(
        HeldTun::admit(file.into()).unwrap_err(),
        Error::InvalidDescriptor
    );
    assert_eq!(HeldTun::admit(fd()).unwrap_err(), Error::InvalidDescriptor);
}
