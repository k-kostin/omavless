//! Fixed opt-in installed-manager dependency and one-use readiness endpoint.
use super::*;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixDatagram;

const NM: &str = "NetworkManager.service";
const NM_FRAGMENT: &str = "/usr/lib/systemd/system/NetworkManager.service";
const DROPIN: &str = "/etc/systemd/system/NetworkManager.service.d/50-omavless-netguard-cold.conf";
const DROPIN_BYTES: &[u8] = include_bytes!("../systemd/NetworkManager-netguard-cold.conf");
const MODULES: &str = "/etc/modules-load.d/omavless-netguard-cold.conf";
const MODULES_BYTES: &[u8] = include_bytes!("../systemd/omavless-netguard-cold-modules.conf");
const NOTIFY: &str = "/run/systemd/notify";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Inspect,
    Cold,
    Prepared,
    Attempted,
    Active,
}
struct Pin {
    file: File,
    original: (u64, u64, u64, i64, i64),
    path: &'static str,
    bytes: &'static [u8],
}
impl Pin {
    fn acquire(path: &'static str, bytes: &'static [u8]) -> Result<Self> {
        let file = root_file(path)?;
        let pin = Self {
            original: identity(&file)?,
            file,
            path,
            bytes,
        };
        pin.recheck()?;
        Ok(pin)
    }
    fn recheck(&self) -> Result<()> {
        use std::os::unix::fs::FileExt;
        require(
            identity(&self.file)? == self.original
                && identity(&root_file(self.path)?)? == self.original,
        )?;
        let m = self.file.metadata().map_err(|_| REFUSE)?;
        require(m.mode() & 0o7777 == 0o644)?;
        let mut data = vec![0; self.bytes.len() + 1];
        let n = self.file.read_at(&mut data, 0).map_err(|_| REFUSE)?;
        require(&data[..n] == self.bytes)?;
        require(
            identity(&self.file)? == self.original
                && identity(&root_file(self.path)?)? == self.original,
        )
    }
}
pub(super) struct ColdOrigin {
    pub(super) phase: Phase,
    dependency: Pin,
    modules: Pin,
    nm_path: OwnedObjectPath,
    nm_fragment: File,
    nm_fragment_identity: (u64, u64, u64, i64, i64),
    notify_parent: File,
    notify_leaf: File,
    notify_identity: (u64, u64, u32, u32, u32, u64),
    notify: UnixDatagram,
}
fn notify_leaf() -> Result<File> {
    Ok(File::from(
        open(
            NOTIFY,
            OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| REFUSE)?,
    ))
}
fn socket_identity(file: &File) -> Result<(u64, u64, u32, u32, u32, u64)> {
    let m = file.metadata().map_err(|_| REFUSE)?;
    require(m.file_type().is_socket() && m.uid() == 0 && m.gid() == 0 && m.nlink() == 1)?;
    Ok((m.dev(), m.ino(), m.mode(), m.uid(), m.gid(), m.nlink()))
}
fn dependency(values: &Properties, key: &str, expected: &str) -> Result<()> {
    let items = values.get::<Vec<String>>(key)?;
    require(
        items
            .iter()
            .filter(|item| item.as_str() == expected)
            .count()
            == 1,
    )
}
fn before_network(unit: &Properties, nm: &Properties, nm_service: &Properties) -> Result<()> {
    unit.text("ActiveState", "activating")?;
    unit.text("SubState", "start")?;
    nm.text("ActiveState", "inactive")?;
    nm.text("SubState", "dead")?;
    nm_service.predicate("MainPID", nm_service.get::<u32>("MainPID")? == 0)?;
    nm_service.predicate("ExecMainPID", nm_service.get::<u32>("ExecMainPID")? == 0)?;
    nm_service.predicate(
        "ExecMainStartTimestampMonotonic",
        nm_service.get::<u64>("ExecMainStartTimestampMonotonic")? == 0,
    )
}
fn readiness_settings(service: &Properties) -> Result<()> {
    service.text("Type", "notify")?;
    service.text("NotifyAccess", "main")?;
    service.text("Restart", "no")?;
    service.predicate(
        "TimeoutStartUSec",
        service.get::<u64>("TimeoutStartUSec")? == u64::MAX,
    )
}
impl ColdOrigin {
    pub(super) fn acquire(bus: &Connection, owner: &str) -> Result<Self> {
        require(
            std::env::var_os("NOTIFY_SOCKET").as_deref() == Some(std::ffi::OsStr::new(NOTIFY)),
        )?;
        let dependency = Pin::acquire(DROPIN, DROPIN_BYTES)?;
        let modules = Pin::acquire(MODULES, MODULES_BYTES)?;
        let nm_path: OwnedObjectPath = decode(
            &bus.call_method(
                Some(owner),
                MANAGER_PATH,
                Some("org.freedesktop.systemd1.Manager"),
                "GetUnit",
                &(NM,),
            )
            .map_err(rpc_refusal)?,
        )?;
        // The unchanged normal OS fragment is an original trusted installed
        // resource, not a supplied unit/template or reconstructed manager grant.
        let nm_fragment = root_file(NM_FRAGMENT)?;
        let nm_fragment_identity = identity(&nm_fragment)?;
        let notify_parent = File::from(
            open(
                "/run/systemd",
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let notify_leaf = notify_leaf()?;
        let notify = UnixDatagram::unbound().map_err(|_| REFUSE)?;
        // Install the reported descriptor in the retained graph BEFORE connect
        // or any later cold effect. On error no readiness retry/cleanup occurs.
        let held = ManuallyDrop::new(Self {
            phase: Phase::Inspect,
            dependency,
            modules,
            nm_path,
            nm_fragment,
            nm_fragment_identity,
            notify_identity: socket_identity(&notify_leaf)?,
            notify_parent,
            notify_leaf,
            notify,
        });
        held.notify.set_nonblocking(true).map_err(|_| REFUSE)?;
        held.notify.connect(NOTIFY).map_err(|_| REFUSE)?;
        held.recheck_notify()?;
        Ok(ManuallyDrop::into_inner(held))
    }
    fn recheck_notify(&self) -> Result<()> {
        let m = self.notify_parent.metadata().map_err(|_| REFUSE)?;
        require(m.is_dir() && m.uid() == 0 && m.gid() == 0 && m.mode() & 0o022 == 0)?;
        let current = File::from(
            open(
                "/run/systemd",
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let n = current.metadata().map_err(|_| REFUSE)?;
        require((m.dev(), m.ino()) == (n.dev(), n.ino()))?;
        require(
            socket_identity(&self.notify_leaf)? == self.notify_identity
                && socket_identity(&notify_leaf()?)? == self.notify_identity,
        )
    }
    pub(super) fn recheck(
        &self,
        origin: &InstalledOrigin,
        unit: &Properties,
        service: &Properties,
    ) -> Result<()> {
        self.dependency.recheck()?;
        self.modules.recheck()?;
        self.recheck_notify()?;
        readiness_settings(service)?;
        dependency(unit, "Before", NM)?;
        dependency(unit, "Requires", "systemd-modules-load.service")?;
        dependency(unit, "After", "systemd-modules-load.service")?;
        let path: OwnedObjectPath = origin.call(
            &origin.manager_owner,
            MANAGER_PATH,
            "org.freedesktop.systemd1.Manager",
            "GetUnit",
            &(NM,),
        )?;
        require(path == self.nm_path)?;
        let nm: Properties = origin.call(
            &origin.manager_owner,
            path.as_str(),
            "org.freedesktop.DBus.Properties",
            "GetAll",
            &("org.freedesktop.systemd1.Unit",),
        )?;
        nm.text("Id", NM)?;
        nm.text("LoadState", "loaded")?;
        nm.boolean("Transient", false)?;
        nm.text("FragmentPath", NM_FRAGMENT)?;
        nm.text("SourcePath", "")?;
        require(
            identity(&self.nm_fragment)? == self.nm_fragment_identity
                && identity(&root_file(NM_FRAGMENT)?)? == self.nm_fragment_identity,
        )?;
        nm.predicate(
            "DropInPaths",
            nm.get::<Vec<String>>("DropInPaths")? == [DROPIN],
        )?;
        dependency(&nm, "Requires", UNIT_NAME)?;
        dependency(&nm, "After", UNIT_NAME)?;
        if self.phase == Phase::Cold {
            let nm_service: Properties = origin.call(
                &origin.manager_owner,
                path.as_str(),
                "org.freedesktop.DBus.Properties",
                "GetAll",
                &("org.freedesktop.systemd1.Service",),
            )?;
            before_network(unit, &nm, &nm_service)?;
        }
        Ok(())
    }
    pub(super) fn send_ready(&self) -> Result<()> {
        require(self.phase == Phase::Attempted)?;
        require(self.notify.send(b"READY=1").map_err(|_| REFUSE)? == 7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> OwnedValue {
        OwnedValue::try_from(zbus::zvariant::Value::new(value)).unwrap()
    }
    fn early() -> (Properties, Properties, Properties) {
        let ng = Properties(HashMap::from([
            ("ActiveState".into(), text("activating")),
            ("SubState".into(), text("start")),
        ]));
        let nm = Properties(HashMap::from([
            ("ActiveState".into(), text("inactive")),
            ("SubState".into(), text("dead")),
        ]));
        let service = Properties(HashMap::from([
            ("MainPID".into(), OwnedValue::from(0_u32)),
            ("ExecMainPID".into(), OwnedValue::from(0_u32)),
            (
                "ExecMainStartTimestampMonotonic".into(),
                OwnedValue::from(0_u64),
            ),
        ]));
        (ng, nm, service)
    }
    #[test]
    fn current_early_gate_uses_unit_substate_and_refuses_active_or_previously_started_nm() {
        let (ng, nm, service) = early();
        before_network(&ng, &nm, &service).unwrap();
        for (key, value, on_ng) in [
            ("ActiveState", "active", true),
            ("SubState", "running", true),
            ("ActiveState", "active", false),
            ("ActiveState", "activating", false),
            ("SubState", "start", false),
        ] {
            let (mut ng, mut nm, service) = early();
            if on_ng {
                ng.0.insert(key.into(), text(value));
            } else {
                nm.0.insert(key.into(), text(value));
            }
            assert!(before_network(&ng, &nm, &service).is_err());
        }
        for key in ["MainPID", "ExecMainPID", "ExecMainStartTimestampMonotonic"] {
            let (ng, nm, mut service) = early();
            service.0.insert(
                key.into(),
                if key.ends_with("Monotonic") {
                    OwnedValue::from(1_u64)
                } else {
                    OwnedValue::from(1_u32)
                },
            );
            assert!(before_network(&ng, &nm, &service).is_err());
            let (ng, nm, mut service) = early();
            service.0.remove(key);
            assert!(before_network(&ng, &nm, &service).is_err());
        }
    }
    #[test]
    fn early_service_and_unit_properties_require_exact_types_not_bool_or_width_coercion() {
        for key in ["MainPID", "ExecMainPID", "ExecMainStartTimestampMonotonic"] {
            let (ng, nm, mut service) = early();
            service.0.insert(key.into(), OwnedValue::from(false));
            assert!(before_network(&ng, &nm, &service).is_err());
            let (ng, nm, mut service) = early();
            service.0.insert(
                key.into(),
                if key.ends_with("Monotonic") {
                    OwnedValue::from(0_u32)
                } else {
                    OwnedValue::from(0_u64)
                },
            );
            assert!(before_network(&ng, &nm, &service).is_err());
        }
        let (mut ng, nm, service) = early();
        ng.0.insert("ActiveState".into(), OwnedValue::from(true));
        assert!(before_network(&ng, &nm, &service).is_err());
    }
    fn notify() -> Properties {
        Properties(HashMap::from([
            ("Type".into(), text("notify")),
            ("NotifyAccess".into(), text("main")),
            ("Restart".into(), text("no")),
            ("TimeoutStartUSec".into(), OwnedValue::from(u64::MAX)),
        ]))
    }
    #[test]
    fn only_main_notify_infinite_start_no_restart_can_admit_the_opt_in_bundle() {
        readiness_settings(&notify()).unwrap();
        for key in ["Type", "NotifyAccess", "Restart", "TimeoutStartUSec"] {
            let mut p = notify();
            p.0.remove(key);
            assert!(readiness_settings(&p).is_err());
            let mut p = notify();
            p.0.insert(key.into(), OwnedValue::from(true));
            assert!(readiness_settings(&p).is_err());
        }
        for (key, value) in [
            ("Type", "exec"),
            ("NotifyAccess", "all"),
            ("Restart", "on-failure"),
        ] {
            let mut p = notify();
            p.0.insert(key.into(), text(value));
            assert!(readiness_settings(&p).is_err());
        }
        for value in [0_u64, 1, 90_000_000] {
            let mut p = notify();
            p.0.insert("TimeoutStartUSec".into(), OwnedValue::from(value));
            assert!(readiness_settings(&p).is_err());
        }
    }
    #[test]
    fn effective_dependency_is_mandatory_unique_and_exact_typed_both_order_and_requirement() {
        for key in ["Requires", "After", "Before"] {
            for values in [vec![], vec!["foreign"], vec![UNIT_NAME, UNIT_NAME]] {
                let p = Properties(HashMap::from([(
                    key.into(),
                    OwnedValue::try_from(zbus::zvariant::Value::new(values)).unwrap(),
                )]));
                assert!(dependency(&p, key, UNIT_NAME).is_err());
            }
            let p = Properties(HashMap::from([(
                key.into(),
                OwnedValue::try_from(zbus::zvariant::Value::new(vec!["other", UNIT_NAME])).unwrap(),
            )]));
            dependency(&p, key, UNIT_NAME).unwrap();
            let p = Properties(HashMap::from([(
                key.into(),
                OwnedValue::try_from(zbus::zvariant::Value::new(vec![1_u64])).unwrap(),
            )]));
            assert!(dependency(&p, key, UNIT_NAME).is_err());
        }
    }
    #[test]
    fn opt_in_assets_are_separate_and_have_no_skip_kill_retry_or_default_activation_shortcut() {
        let unit = std::str::from_utf8(SERVICE_UNIT).unwrap();
        for line in [
            "Type=notify\n",
            "NotifyAccess=main\n",
            "TimeoutStartSec=infinity\n",
            "Restart=no\n",
            "Before=NetworkManager.service network-pre.target\n",
            "Requires=systemd-modules-load.service\n",
            "After=systemd-modules-load.service\n",
        ] {
            assert_eq!(unit.matches(line).count(), 1);
        }
        assert!(!unit.contains("Condition"));
        assert!(!unit.contains("ExecCondition"));
        assert!(!unit.contains("RestartSec="));
        assert_eq!(
            DROPIN_BYTES,
            b"[Unit]\nRequires=omavless-netguard.service\nAfter=omavless-netguard.service\n"
        );
        assert_eq!(MODULES_BYTES, b"nf_tables\n");
        let old = include_str!("../systemd/omavless-netguard.service");
        assert!(old.contains("Type=exec\n"));
        assert!(old.contains("Restart=on-failure\n"));
        assert!(!old.contains("NotifyAccess="));
    }
}
