// SPDX-License-Identifier: MIT
//! Fixed trusted installed-manager launch, with read-only consistency fences.
//! Authority assumes root's admitted package/install and system manager launch;
//! matching bytes, environment or namespace IDs alone DO NOT authenticate it.
use super::*;
use crate::kernel_observer::service_creator::LiveCreator;
use nix::fcntl::{OFlag, open};
use nix::sys::stat::Mode;
use nix::unistd::{geteuid, getppid};
use nix_netguard::sys::nsfs::{NamespaceType, namespace_id, namespace_type};
use nix_netguard::sys::socket::{getsockopt, sockopt::NetnsCookie};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::time::Duration;
use zbus::blocking::{Connection, connection::Builder};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Type};

pub(crate) const SERVICE_UNIT: &[u8] = include_bytes!("../systemd/omavless-netguard.service");
const FRAGMENT: &str = "/usr/lib/systemd/system/omavless-netguard.service";
const UNIT_NAME: &str = "omavless-netguard.service";
const MANAGER: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const REFUSE: EffectError = EffectError::UnavailableOrUncertain;
type Result<T> = std::result::Result<T, EffectError>;
type ExecCommand = (String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32);
fn require(value: bool) -> Result<()> {
    if value { Ok(()) } else { Err(REFUSE) }
}

fn read_bounded(path: &str, limit: u64) -> Result<Vec<u8>> {
    let fd = open(
        path,
        OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| REFUSE)?;
    let mut bytes = Vec::new();
    File::from(fd)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| REFUSE)?;
    require(bytes.len() as u64 <= limit)?;
    Ok(bytes)
}
fn root_file(path: &str) -> Result<File> {
    let file = File::from(
        open(
            path,
            OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| REFUSE)?,
    );
    let m = file.metadata().map_err(|_| REFUSE)?;
    require(
        m.is_file() && m.uid() == 0 && m.gid() == 0 && m.nlink() == 1 && m.mode() & 0o022 == 0,
    )?;
    Ok(file)
}
fn identity(file: &File) -> Result<(u64, u64, u64, i64, i64)> {
    let m = file.metadata().map_err(|_| REFUSE)?;
    Ok((m.dev(), m.ino(), m.len(), m.ctime(), m.ctime_nsec()))
}
fn read_unit(file: &File) -> Result<()> {
    use std::os::unix::fs::FileExt;
    let mut bytes = vec![0; SERVICE_UNIT.len() + 1];
    let n = file.read_at(&mut bytes, 0).map_err(|_| REFUSE)?;
    require(&bytes[..n] == SERVICE_UNIT)
}

fn decode<T: DeserializeOwned + Type>(message: &zbus::Message) -> Result<T> {
    // zbus receives under its own upstream allocation limit; this is the
    // tighter decoding bound, not a claim of pre-allocation packet rejection.
    require(message.data().len() <= 128 * 1024 && message.data().fds().is_empty())?;
    message.body().deserialize().map_err(|_| REFUSE)
}

// Reject duplicate keys before extracting the few fixed effective settings.
struct Properties(HashMap<String, OwnedValue>);
impl Type for Properties {
    const SIGNATURE: &'static zbus::zvariant::Signature = <HashMap<String, OwnedValue>>::SIGNATURE;
}
impl<'de> serde::Deserialize<'de> for Properties {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Unique;
        impl<'de> serde::de::Visitor<'de> for Unique {
            type Value = Properties;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded unique effective properties")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut out = HashMap::new();
                while let Some((key, value)) = map.next_entry::<String, OwnedValue>()? {
                    if out.len() >= 512 || key.len() > 128 || out.contains_key(&key) {
                        return Err(serde::de::Error::custom("invalid properties"));
                    }
                    out.insert(key, value);
                }
                Ok(Properties(out))
            }
        }
        d.deserialize_map(Unique)
    }
}
impl Properties {
    fn get<T>(&self, key: &str) -> Result<T>
    where
        T: TryFrom<OwnedValue>,
        T: Type,
    {
        let value = self.0.get(key).ok_or(REFUSE)?;
        require(value.value_signature() == T::SIGNATURE)?;
        T::try_from(value.try_clone().map_err(|_| REFUSE)?).map_err(|_| REFUSE)
    }
    fn text(&self, key: &str, expected: &str) -> Result<()> {
        require(self.get::<String>(key)? == expected)
    }
    fn empty(&self, key: &str) -> Result<()> {
        require(self.get::<Vec<String>>(key)?.is_empty())
    }
    fn boolean(&self, key: &str, expected: bool) -> Result<()> {
        require(self.get::<bool>(key)? == expected)
    }
}

struct InstalledOrigin {
    bus: Connection,
    manager_owner: String,
    unit_path: OwnedObjectPath,
    invocation: Vec<u8>,
    fragment: File,
    executable: File,
    fragment_identity: (u64, u64, u64, i64, i64),
    executable_identity: (u64, u64, u64, i64, i64),
    namespace_id: u64,
}
impl InstalledOrigin {
    fn call<T: serde::Serialize + Type, R: DeserializeOwned + Type>(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        args: &T,
    ) -> Result<R> {
        decode(
            &self
                .bus
                .call_method(Some(destination), path, Some(interface), method, args)
                .map_err(|_| REFUSE)?,
        )
    }
    fn properties(&self, interface: &str) -> Result<Properties> {
        self.call(
            &self.manager_owner,
            self.unit_path.as_str(),
            "org.freedesktop.DBus.Properties",
            "GetAll",
            &(interface,),
        )
    }
    fn recheck_installed(&self) -> Result<()> {
        require(geteuid().as_raw() == 0 && getppid().as_raw() == 1)?;
        require(
            identity(&self.fragment)? == self.fragment_identity
                && identity(&root_file(FRAGMENT)?)? == self.fragment_identity,
        )?;
        require(
            identity(&self.executable)? == self.executable_identity
                && identity(&root_file(EXECUTABLE)?)? == self.executable_identity,
        )?;
        read_unit(&self.fragment)?;
        require(
            identity(&File::open("/proc/self/exe").map_err(|_| REFUSE)?)?
                == self.executable_identity,
        )?;
        require(
            read_bounded("/proc/self/cmdline", 256)? == format!("{EXECUTABLE}\0serve\0").as_bytes(),
        )?;
        require(
            read_bounded("/proc/self/cgroup", 256)?
                == b"0::/system.slice/omavless-netguard.service\n",
        )?;
        let owner: String = self.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            &(MANAGER,),
        )?;
        require(owner == self.manager_owner)?;
        let manager_pid: u32 = self.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionUnixProcessID",
            &(owner.as_str(),),
        )?;
        let manager_uid: u32 = self.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionUnixUser",
            &(owner.as_str(),),
        )?;
        require(manager_pid == 1 && manager_uid == 0)?;
        let path: OwnedObjectPath = self.call(
            &owner,
            MANAGER_PATH,
            "org.freedesktop.systemd1.Manager",
            "GetUnitByPID",
            &(std::process::id(),),
        )?;
        require(path == self.unit_path)?;
        let unit = self.properties("org.freedesktop.systemd1.Unit")?;
        unit.text("Id", UNIT_NAME)?;
        unit.text("LoadState", "loaded")?;
        unit.text("FragmentPath", FRAGMENT)?;
        unit.text("SourcePath", "")?;
        unit.boolean("Transient", false)?;
        unit.empty("DropInPaths")?;
        // Type=exec enters the application before activation completes; this
        // is the SAME invocation during activating or active, never dead.
        require(matches!(
            unit.get::<String>("ActiveState")?.as_str(),
            "activating" | "active"
        ))?;
        require(unit.get::<Vec<u8>>("InvocationID")? == self.invocation)?;
        let service = self.properties("org.freedesktop.systemd1.Service")?;
        service.text("Type", "exec")?;
        service.text("User", "root")?;
        service.text("Group", "root")?;
        let commands = service.get::<Vec<ExecCommand>>("ExecStart")?;
        require(
            commands.len() == 1
                && commands[0].0 == EXECUTABLE
                && commands[0].1 == [EXECUTABLE, "serve"]
                && !commands[0].2,
        )?;
        for key in [
            "ExecStartPre",
            "ExecStartPost",
            "ExecReload",
            "ExecStop",
            "ExecStopPost",
        ] {
            require(service.get::<Vec<ExecCommand>>(key)?.is_empty())?;
        }
        service.text("ControlGroup", "/system.slice/omavless-netguard.service")?;
        require(
            service.get::<u32>("MainPID")? == std::process::id()
                && service.get::<u32>("ExecMainPID")? == std::process::id(),
        )?;
        service.text("StandardInput", "file")?;
        service.text("StandardInputFile", "/proc/1/ns/net")?;
        service.text("StandardOutput", "null")?;
        service.text("StandardError", "null")?;
        service.text("KillMode", "control-group")?;
        service.text("RuntimeDirectoryPreserve", "no")?;
        require(service.get::<Vec<String>>("RuntimeDirectory")? == ["omavless-netguard"])?;
        require(service.get::<u32>("RuntimeDirectoryMode")? == 0o700)?;
        service.boolean("NoNewPrivileges", true)?;
        // RestrictNamespaces=yes is reported as the allowed-kind bitmask 0.
        require(service.get::<u64>("RestrictNamespaces")? == 0)?;
        require(service.get::<u64>("CapabilityBoundingSet")? == 1 << 12)?;
        require(service.get::<u64>("AmbientCapabilities")? == 0)?;
        service.boolean("PrivateNetwork", false)?;
        service.boolean("PrivateMounts", false)?;
        // Newer systemd uses enum-string PrivateUsers, not boolean.
        service.text("PrivateUsers", "no")?;
        service.boolean("Delegate", false)?;
        require(
            service.get::<u32>("FileDescriptorStoreMax")? == 0
                && service.get::<u64>("WatchdogUSec")? == 0,
        )?;
        for key in [
            "RootDirectory",
            "RootImage",
            "NetworkNamespacePath",
            "PAMName",
        ] {
            service.text(key, "")?;
        }
        unit.empty("JoinsNamespaceOf")?;
        for key in [
            "ReadWritePaths",
            "ReadOnlyPaths",
            "InaccessiblePaths",
            "ExtensionDirectories",
        ] {
            service.empty(key)?;
        }
        for key in ["BindPaths", "BindReadOnlyPaths"] {
            require(
                service
                    .get::<Vec<(String, String, bool, u64)>>(key)?
                    .is_empty(),
            )?;
        }
        require(
            service
                .get::<Vec<(String, String)>>("TemporaryFileSystem")?
                .is_empty(),
        )?;
        service.text("ProtectProc", "default")?;
        service.text("ProcSubset", "all")?;
        // Fixed bytes also forbid mounts/binds, extra execs and namespace
        // overrides; effective unit + actual namespace fences are mandatory.
        for name in ["user", "mnt", "pid"] {
            let manager = File::open(format!("/proc/1/ns/{name}")).map_err(|_| REFUSE)?;
            let current = File::open(format!("/proc/thread-self/ns/{name}")).map_err(|_| REFUSE)?;
            require(
                namespace_type(manager.as_fd()).map_err(|_| REFUSE)?
                    == namespace_type(current.as_fd()).map_err(|_| REFUSE)?
                    && namespace_id(manager.as_fd()).map_err(|_| REFUSE)?
                        == namespace_id(current.as_fd()).map_err(|_| REFUSE)?,
            )?;
        }
        Ok(())
    }
}
impl OriginalVerifier for InstalledOrigin {
    fn recheck(&mut self, originals: LaunchBorrow<'_>) -> Result<()> {
        self.recheck_installed()?;
        for fd in [originals.anchor, originals.thread_namespace] {
            require(
                namespace_type(fd).map_err(|_| REFUSE)? == NamespaceType::Network
                    && namespace_id(fd).map_err(|_| REFUSE)? == self.namespace_id,
            )?;
        }
        let current = File::open("/proc/thread-self/ns/net").map_err(|_| REFUSE)?;
        let manager = File::open("/proc/1/ns/net").map_err(|_| REFUSE)?;
        for file in [current, manager] {
            require(
                namespace_type(file.as_fd()).map_err(|_| REFUSE)? == NamespaceType::Network
                    && namespace_id(file.as_fd()).map_err(|_| REFUSE)? == self.namespace_id,
            )?;
        }
        require(
            getsockopt(&originals.creator_socket, NetnsCookie).map_err(|_| REFUSE)?
                == self.namespace_id,
        )?;
        // FD0 is a manager-delivered anchor only UNDER admitted installation
        // and this original fixed invocation. Descriptor equality is a fence.
        require(
            namespace_type(std::io::stdin().as_fd()).map_err(|_| REFUSE)? == NamespaceType::Network
                && namespace_id(std::io::stdin().as_fd()).map_err(|_| REFUSE)? == self.namespace_id,
        )?;
        Ok(())
    }
}

fn uuid(bytes: &[u8]) -> Result<[u8; 16]> {
    require(bytes.len() == 36 && [8, 13, 18, 23].iter().all(|i| bytes[*i] == b'-'))?;
    let hex: Vec<_> = bytes.iter().copied().filter(|b| *b != b'-').collect();
    require(hex.len() == 32 && hex.iter().all(u8::is_ascii_hexdigit))?;
    let mut out = [0; 16];
    for (n, pair) in hex.as_chunks::<2>().0.iter().enumerate() {
        out[n] = u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| REFUSE)?, 16)
            .map_err(|_| REFUSE)?;
    }
    require(out != [0; 16])?;
    Ok(out)
}

pub(crate) fn acquire_fixed_service() -> Result<AcquiredCreator<LiveCreator>> {
    require(geteuid().as_raw() == 0 && getppid().as_raw() == 1)?;
    let anchor = File::from(
        std::io::stdin()
            .as_fd()
            .try_clone_to_owned()
            .map_err(|_| REFUSE)?,
    );
    require(namespace_type(anchor.as_fd()).map_err(|_| REFUSE)? == NamespaceType::Network)?;
    let ns_id = namespace_id(anchor.as_fd()).map_err(|_| REFUSE)?;
    let meta = anchor.metadata().map_err(|_| REFUSE)?;
    let mut boot = read_bounded("/proc/sys/kernel/random/boot_id", 37)?;
    require(boot.pop() == Some(b'\n'))?;
    let mut ns_epoch = [0; 16];
    ns_epoch[..8].copy_from_slice(&ns_id.to_be_bytes());
    let epoch = HostEpoch {
        boot: uuid(&boot)?,
        namespace_epoch: ns_epoch,
        namespace_device: meta.dev(),
        namespace_inode: meta.ino(),
    };
    let bus = Builder::system()
        .map_err(|_| REFUSE)?
        .method_timeout(Duration::from_secs(3))
        .build()
        .map_err(|_| REFUSE)?;
    let reply = bus
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &(MANAGER,),
        )
        .map_err(|_| REFUSE)?;
    let owner: String = decode(&reply)?;
    zbus::names::UniqueName::try_from(owner.as_str()).map_err(|_| REFUSE)?;
    let path: OwnedObjectPath = decode(
        &bus.call_method(
            Some(owner.as_str()),
            MANAGER_PATH,
            Some("org.freedesktop.systemd1.Manager"),
            "GetUnitByPID",
            &(std::process::id(),),
        )
        .map_err(|_| REFUSE)?,
    )?;
    let invocation: OwnedValue = decode(
        &bus.call_method(
            Some(owner.as_str()),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.systemd1.Unit", "InvocationID"),
        )
        .map_err(|_| REFUSE)?,
    )?;
    require(invocation.value_signature() == Vec::<u8>::SIGNATURE)?;
    let invocation = Vec::<u8>::try_from(invocation).map_err(|_| REFUSE)?;
    require(invocation.len() == 16 && invocation.iter().any(|b| *b != 0))?;
    let fragment = root_file(FRAGMENT)?;
    let executable = root_file(EXECUTABLE)?;
    let verifier = InstalledOrigin {
        bus,
        manager_owner: owner,
        unit_path: path,
        invocation,
        fragment_identity: identity(&fragment)?,
        executable_identity: identity(&executable)?,
        fragment,
        executable,
        namespace_id: ns_id,
    };
    verifier.recheck_installed()?;
    let creator = LiveCreator::open(epoch).map_err(|_| REFUSE)?;
    let (thread_namespace, creator_socket) = creator.original_aliases().map_err(|_| REFUSE)?;
    let mut acquired = AcquiredCreator {
        retained: ManuallyDrop::new(Retained {
            originals: Originals {
                anchor,
                thread_namespace,
                creator_socket,
                owner_thread: thread::current().id(),
                verifier: Box::new(verifier),
            },
            creator,
        }),
        sealed: false,
        _same_thread: PhantomData,
    };
    // Once assembled, even admission refusal/unwind retains every original.
    acquired.retained_epoch(Boundary::Admission)?;
    Ok(acquired)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effective_properties_require_exact_types_even_for_empty_arrays() {
        let mut values = HashMap::new();
        values.insert(
            "DropInPaths".into(),
            OwnedValue::try_from(zbus::zvariant::Value::new(Vec::<String>::new())).unwrap(),
        );
        let good = Properties(values);
        assert!(good.empty("DropInPaths").is_ok());
        assert!(good.get::<Vec<u64>>("DropInPaths").is_err());
        assert!(good.get::<bool>("DropInPaths").is_err());
        assert!(good.get::<Vec<String>>("Absent").is_err());
        let mut values = HashMap::new();
        values.insert(
            "DropInPaths".into(),
            OwnedValue::try_from(zbus::zvariant::Value::new(Vec::<u64>::new())).unwrap(),
        );
        assert!(Properties(values).empty("DropInPaths").is_err());
    }
    #[test]
    fn boot_uuid_is_exact_nonzero_not_a_namespace_authority_token() {
        assert!(uuid(b"12345678-1234-1234-1234-123456789abc").is_ok());
        for bytes in [
            b"00000000-0000-0000-0000-000000000000".as_slice(),
            b"12345678x1234-1234-1234-123456789abc",
            b"12345678-1234-1234-1234-123456789abc\n",
        ] {
            assert!(uuid(bytes).is_err());
        }
    }
}
