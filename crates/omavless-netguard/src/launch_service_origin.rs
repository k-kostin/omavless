// SPDX-License-Identifier: MIT
//! Fixed trusted installed-manager launch, with read-only consistency fences.
//! Authority assumes root's admitted package/install and system manager launch;
//! matching bytes, environment or namespace IDs alone DO NOT authenticate it.
use super::*;
use crate::inherited_namespace_anchors::{ManagerNamespaceAnchors, admitted_open_files};
use crate::kernel_observer::service_creator::LiveCreator;
use crate::package_group_candidate::PackageGroup;
use crate::startup_trace::{self as trace, Event, Phase};
use nix::fcntl::{OFlag, open};
use nix::sys::stat::Mode;
use nix::unistd::{geteuid, getgroups, getppid};
use nix_netguard::sys::nsfs::{NamespaceType, namespace_id, namespace_type};
use nix_netguard::sys::socket::{getsockopt, sockopt::NetnsCookie};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::time::Duration;
use zbus::blocking::{Connection, connection::Builder};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Type};

#[cfg(not(feature = "netguard-cold-bootstrap"))]
pub(crate) const SERVICE_UNIT: &[u8] = include_bytes!("../systemd/omavless-netguard.service");
#[cfg(feature = "netguard-cold-bootstrap")]
pub(crate) const SERVICE_UNIT: &[u8] = include_bytes!("../systemd/omavless-netguard-cold.service");
#[cfg(feature = "netguard-cold-bootstrap")]
#[path = "launch_cold_origin.rs"]
mod cold;
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
    if message.data().len() > 128 * 1024 || !message.data().fds().is_empty() {
        trace::emit(Phase::Rpc, Event::Decode, None);
        return Err(REFUSE);
    }
    message.body().deserialize().map_err(|_| {
        trace::emit(Phase::Rpc, Event::Decode, None);
        REFUSE
    })
}
fn rpc_refusal(error: zbus::Error) -> EffectError {
    let event = match &error {
        zbus::Error::InputOutput(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            Event::Deadline
        }
        _ => Event::Unavailable,
    };
    trace::emit(Phase::Rpc, event, None);
    REFUSE
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
        let value = self.0.get(key).ok_or_else(|| {
            trace::emit(Phase::EffectiveUnit, Event::Missing, Some(key));
            REFUSE
        })?;
        if value.value_signature() != T::SIGNATURE {
            trace::emit(Phase::EffectiveUnit, Event::Type, Some(key));
            return Err(REFUSE);
        }
        T::try_from(value.try_clone().map_err(|_| {
            trace::emit(Phase::EffectiveUnit, Event::Unavailable, Some(key));
            REFUSE
        })?)
        .map_err(|_| {
            trace::emit(Phase::EffectiveUnit, Event::Decode, Some(key));
            REFUSE
        })
    }
    fn predicate(&self, key: &str, matches: bool) -> Result<()> {
        if !matches {
            trace::emit(Phase::EffectiveUnit, Event::Mismatch, Some(key));
        }
        require(matches)
    }
    fn text(&self, key: &str, expected: &str) -> Result<()> {
        self.predicate(key, self.get::<String>(key)? == expected)
    }
    fn empty(&self, key: &str) -> Result<()> {
        self.predicate(key, self.get::<Vec<String>>(key)?.is_empty())
    }
    fn boolean(&self, key: &str, expected: bool) -> Result<()> {
        self.predicate(key, self.get::<bool>(key)? == expected)
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
    package_group: PackageGroup,
    manager_namespaces: ManagerNamespaceAnchors,
    #[cfg(feature = "netguard-cold-bootstrap")]
    cold: cold::ColdOrigin,
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
                .map_err(rpc_refusal)?,
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
        self.package_group.validate().map_err(|_| REFUSE)?;
        let groups: Vec<_> = getgroups()
            .map_err(|_| REFUSE)?
            .into_iter()
            .map(|group| group.as_raw())
            .collect();
        require(admitted_groups(&groups, self.package_group.gid()))?;
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
        unit.predicate(
            "ActiveState",
            admitted_active_state(&unit.get::<String>("ActiveState")?),
        )?;
        unit.predicate(
            "InvocationID",
            unit.get::<Vec<u8>>("InvocationID")? == self.invocation,
        )?;
        let service = self.properties("org.freedesktop.systemd1.Service")?;
        #[cfg(not(feature = "netguard-cold-bootstrap"))]
        service.text("Type", "exec")?;
        #[cfg(feature = "netguard-cold-bootstrap")]
        service.text("Type", "notify")?;
        service.text("User", "root")?;
        service.text("Group", "root")?;
        service.predicate(
            "SupplementaryGroups",
            service.get::<Vec<String>>("SupplementaryGroups")? == ["omavless-netguard"],
        )?;
        let commands = service.get::<Vec<ExecCommand>>("ExecStart")?;
        service.predicate(
            "ExecStart",
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
            service.predicate(key, service.get::<Vec<ExecCommand>>(key)?.is_empty())?;
        }
        service.text("ControlGroup", "/system.slice/omavless-netguard.service")?;
        current_main_pids(&service, std::process::id())?;
        service.text("StandardInput", "file")?;
        // systemd v261 exposes StandardInputFile only as a transient SETTER,
        // not a readable property. FileDescriptorName describes named-FD
        // input, not this path. Path custody comes from the admitted manager
        // launch, retained exact installed unit/no drop-ins, and original FD0
        // namespace fences below; no matching getter authenticates delivery.
        service.text("StandardOutput", "null")?;
        service.text("StandardError", "journal")?;
        service.predicate(
            "OpenFile",
            admitted_open_files(&service.get::<Vec<(String, String, u64)>>("OpenFile")?),
        )?;
        service.empty("ExtraFileDescriptorNames")?;
        service.text("KillMode", "control-group")?;
        service.text("RuntimeDirectoryPreserve", "no")?;
        service.predicate(
            "RuntimeDirectory",
            service.get::<Vec<String>>("RuntimeDirectory")? == ["omavless-netguard"],
        )?;
        service.predicate(
            "RuntimeDirectoryMode",
            service.get::<u32>("RuntimeDirectoryMode")? == 0o700,
        )?;
        service.boolean("NoNewPrivileges", true)?;
        // RestrictNamespaces=yes is reported as the allowed-kind bitmask 0.
        service.predicate(
            "RestrictNamespaces",
            service.get::<u64>("RestrictNamespaces")? == 0,
        )?;
        service.predicate(
            "CapabilityBoundingSet",
            service.get::<u64>("CapabilityBoundingSet")? == 1 << 12,
        )?;
        service.predicate(
            "AmbientCapabilities",
            service.get::<u64>("AmbientCapabilities")? == 0,
        )?;
        service.boolean("PrivateNetwork", false)?;
        service.boolean("PrivateMounts", false)?;
        // The legacy name is STILL boolean. PrivateUsersEx is a separate
        // newer enum-string property, not assumed supported on every version.
        service.boolean("PrivateUsers", false)?;
        service.boolean("Delegate", false)?;
        service.predicate(
            "FileDescriptorStoreMax",
            service.get::<u32>("FileDescriptorStoreMax")? == 0,
        )?;
        service.predicate("WatchdogUSec", active_watchdog_disabled(&service)?)?;
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
            service.predicate(
                key,
                service
                    .get::<Vec<(String, String, bool, u64)>>(key)?
                    .is_empty(),
            )?;
        }
        service.predicate(
            "TemporaryFileSystem",
            service
                .get::<Vec<(String, String)>>("TemporaryFileSystem")?
                .is_empty(),
        )?;
        service.text("ProtectProc", "default")?;
        service.text("ProcSubset", "all")?;
        // Fixed bytes also forbid mounts/binds, extra execs and namespace
        // overrides; effective unit + actual namespace fences are mandatory.
        trace::emit(Phase::OriginalNamespaces, Event::Begin, None);
        self.manager_namespaces.recheck()?;
        trace::emit(Phase::OriginalNamespaces, Event::Pass, None);
        #[cfg(feature = "netguard-cold-bootstrap")]
        self.cold.recheck(self, &unit, &service)?;
        Ok(())
    }
}
fn admitted_active_state(state: &str) -> bool {
    matches!(state, "activating" | "active")
}
fn current_main_pids(service: &Properties, pid: u32) -> Result<()> {
    service.predicate("MainPID", service.get::<u32>("MainPID")? == pid)?;
    service.predicate("ExecMainPID", service.get::<u32>("ExecMainPID")? == pid)
}
fn active_watchdog_disabled(service: &Properties) -> Result<bool> {
    // v261 initializes the never-started original timeout to USEC_INFINITY,
    // then copies configured WatchdogSec into it before this invocation.
    // This gate admits activating/active ONLY, so infinity is not accepted.
    Ok(service.get::<u64>("WatchdogUSec")? == 0)
}
impl OriginalVerifier for InstalledOrigin {
    #[cfg(feature = "netguard-cold-bootstrap")]
    fn begin_cold_create(&mut self, originals: LaunchBorrow<'_>) -> Result<()> {
        require(self.cold.phase == cold::Phase::Inspect)?;
        self.cold.phase = cold::Phase::Cold;
        self.recheck(originals)
    }
    #[cfg(feature = "netguard-cold-bootstrap")]
    fn consume_startup(&mut self, originals: LaunchBorrow<'_>) -> Result<()> {
        require(matches!(
            self.cold.phase,
            cold::Phase::Inspect | cold::Phase::Cold
        ))?;
        self.recheck(originals)?;
        self.cold.phase = cold::Phase::Prepared;
        Ok(())
    }
    #[cfg(feature = "netguard-cold-bootstrap")]
    fn notify_ready(&mut self, originals: LaunchBorrow<'_>) -> Result<()> {
        require(self.cold.phase == cold::Phase::Prepared)?;
        // Normal original guards, not re-admission of NM-inactive permission.
        self.recheck(originals)?;
        self.cold.phase = cold::Phase::Attempted;
        self.cold.send_ready()?;
        self.cold.phase = cold::Phase::Active;
        // Positive READY cannot be retracted: no fallible post-send admission.
        Ok(())
    }
    fn recheck(&mut self, originals: LaunchBorrow<'_>) -> Result<()> {
        trace::step(Phase::EffectiveUnit, || self.recheck_installed())?;
        for fd in [originals.anchor, originals.thread_namespace] {
            require(
                namespace_type(fd).map_err(|_| REFUSE)? == NamespaceType::Network
                    && namespace_id(fd).map_err(|_| REFUSE)? == self.namespace_id,
            )?;
        }
        let current = File::open("/proc/thread-self/ns/net").map_err(|_| REFUSE)?;
        require(
            namespace_type(current.as_fd()).map_err(|_| REFUSE)? == NamespaceType::Network
                && namespace_id(current.as_fd()).map_err(|_| REFUSE)? == self.namespace_id,
        )?;
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

fn admitted_groups(groups: &[u32], package_gid: u32) -> bool {
    package_gid != 0
        && groups.contains(&package_gid)
        && groups.len() <= 2
        && groups.iter().all(|gid| *gid == 0 || *gid == package_gid)
        && groups
            .iter()
            .enumerate()
            .all(|(index, gid)| !groups[..index].contains(gid))
}

pub(crate) fn acquire_fixed_service() -> Result<AcquiredCreator<LiveCreator>> {
    trace::emit(Phase::Anchor, Event::Begin, None);
    require(geteuid().as_raw() == 0 && getppid().as_raw() == 1)?;
    // Import fixed inherited slots BEFORE any bus/threads or other FD opens.
    // No reopening PID1 namespace paths under reduced runtime capabilities.
    let (anchor, manager_namespaces) =
        trace::step(Phase::InheritedAnchors, ManagerNamespaceAnchors::acquire)?;
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
    trace::emit(Phase::Anchor, Event::Pass, None);
    trace::emit(Phase::BusOwner, Event::Begin, None);
    let bus = Builder::system()
        .map_err(rpc_refusal)?
        .method_timeout(Duration::from_secs(3))
        .build()
        .map_err(rpc_refusal)?;
    let reply = bus
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &(MANAGER,),
        )
        .map_err(rpc_refusal)?;
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
        .map_err(rpc_refusal)?,
    )?;
    let invocation: OwnedValue = decode(
        &bus.call_method(
            Some(owner.as_str()),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.systemd1.Unit", "InvocationID"),
        )
        .map_err(rpc_refusal)?,
    )?;
    require(invocation.value_signature() == Vec::<u8>::SIGNATURE)?;
    let invocation = Vec::<u8>::try_from(invocation).map_err(|_| REFUSE)?;
    require(invocation.len() == 16 && invocation.iter().any(|b| *b != 0))?;
    trace::emit(Phase::BusOwner, Event::Pass, None);
    trace::emit(Phase::OriginFiles, Event::Begin, None);
    let fragment = root_file(FRAGMENT)?;
    let executable = root_file(EXECUTABLE)?;
    let package_group = PackageGroup::open_fixed().map_err(|_| REFUSE)?;
    #[cfg(feature = "netguard-cold-bootstrap")]
    let cold = cold::ColdOrigin::acquire(&bus, &owner)?;
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
        package_group,
        manager_namespaces,
        #[cfg(feature = "netguard-cold-bootstrap")]
        cold,
    };
    trace::emit(Phase::OriginFiles, Event::Pass, None);
    trace::step(Phase::EffectiveUnit, || verifier.recheck_installed())?;
    let creator = trace::step(Phase::CreatorOpen, || {
        LiveCreator::open(epoch).map_err(|_| REFUSE)
    })?;
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
    trace::emit(Phase::CreatorAssembled, Event::Pass, None);
    // Once assembled, even admission refusal/unwind retains every original.
    acquired.retained_epoch(Boundary::Admission)?;
    Ok(acquired)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activating_and_active_never_admit_watchdog_infinity_or_bad_typed_value() {
        for state in ["activating", "active"] {
            assert!(admitted_active_state(state));
            for (value, expected) in [(0, true), (1, false), (u64::MAX, false)] {
                let mut values = HashMap::new();
                values.insert("WatchdogUSec".into(), OwnedValue::from(value));
                assert_eq!(
                    active_watchdog_disabled(&Properties(values)).unwrap(),
                    expected
                );
            }
        }
        for state in ["", "inactive", "failed", "deactivating", "reloading"] {
            assert!(!admitted_active_state(state));
        }
    }
    #[test]
    fn initial_zero_other_and_wrong_typed_main_pids_are_not_grants() {
        for (main, exec, expected) in [
            (41, 41, true),
            (0, 0, false),
            (41, 0, false),
            (0, 41, false),
            (41, 42, false),
        ] {
            let mut values = HashMap::new();
            values.insert("MainPID".into(), OwnedValue::from(main as u32));
            values.insert("ExecMainPID".into(), OwnedValue::from(exec as u32));
            assert_eq!(current_main_pids(&Properties(values), 41).is_ok(), expected);
        }
        let mut values = HashMap::new();
        values.insert("MainPID".into(), OwnedValue::from(41_u64));
        values.insert("ExecMainPID".into(), OwnedValue::from(41_u32));
        assert!(current_main_pids(&Properties(values), 41).is_err());
        assert!(current_main_pids(&Properties(HashMap::new()), 41).is_err());
    }
    #[test]
    fn developer_stderr_unit_and_typed_gate_agree() {
        let unit = std::str::from_utf8(SERVICE_UNIT).unwrap();
        assert_eq!(unit.matches("StandardError=journal\n").count(), 1);
        let mut values = HashMap::new();
        values.insert(
            "StandardError".into(),
            OwnedValue::try_from(zbus::zvariant::Value::new("journal")).unwrap(),
        );
        assert!(Properties(values).text("StandardError", "journal").is_ok());
    }
    #[test]
    fn active_watchdog_requires_exact_unsigned_zero_not_prestart_infinity() {
        for (value, expected) in [(0, true), (1, false), (3_000_000, false), (u64::MAX, false)] {
            let mut values = HashMap::new();
            values.insert("WatchdogUSec".into(), OwnedValue::from(value));
            assert_eq!(
                active_watchdog_disabled(&Properties(values)).unwrap(),
                expected
            );
        }
        let mut values = HashMap::new();
        values.insert("WatchdogUSec".into(), OwnedValue::from(0_u32));
        assert!(active_watchdog_disabled(&Properties(values)).is_err());
        assert!(active_watchdog_disabled(&Properties(HashMap::new())).is_err());
    }
    #[test]
    fn filepath_delivery_requires_literal_unit_without_unsupported_getter() {
        let unit = std::str::from_utf8(SERVICE_UNIT).unwrap();
        assert_eq!(
            unit.matches("StandardInput=file:/proc/1/ns/net\n").count(),
            1
        );
        let source = include_str!("launch_service_origin.rs");
        assert!(!source.contains("service.text(\"StandardInputFile\","));
        assert!(!source.contains("service.text(\"StandardInputFileDescriptorName\","));
    }
    #[test]
    fn only_fixed_package_supplementary_membership_is_admitted() {
        assert!(admitted_groups(&[71], 71));
        assert!(admitted_groups(&[0, 71], 71));
        assert!(admitted_groups(&[71, 0], 71));
        for groups in [
            &[][..],
            &[0],
            &[70],
            &[0, 70],
            &[71, 72],
            &[71, 71],
            &[0, 0, 71],
        ] {
            assert!(!admitted_groups(groups, 71));
        }
        assert!(!admitted_groups(&[0], 0));
    }

    #[test]
    fn private_users_legacy_property_is_boolean_false_not_enum_text() {
        let mut values = HashMap::new();
        values.insert("PrivateUsers".into(), OwnedValue::from(false));
        assert!(Properties(values).boolean("PrivateUsers", false).is_ok());
        let mut values = HashMap::new();
        values.insert("PrivateUsers".into(), OwnedValue::from(true));
        assert!(Properties(values).boolean("PrivateUsers", false).is_err());
        let mut values = HashMap::new();
        values.insert(
            "PrivateUsers".into(),
            OwnedValue::try_from(zbus::zvariant::Value::new("no")).unwrap(),
        );
        assert!(Properties(values).boolean("PrivateUsers", false).is_err());
    }
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
