//! Developer-only configured facts. No start, lifecycle or ownership admission.
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Type};

const UNIT: &str = "omavless-k1-private-lifecycle-admission.service";
const STAGE: &str = "/run/omavless-k1-private-lifecycle-admission";
const FRAGMENT: &str = "/run/systemd/system/omavless-k1-private-lifecycle-admission.service";
const UNIT_PATH: &str =
    "/org/freedesktop/systemd1/unit/omavless_2dk1_2dprivate_2dlifecycle_2dadmission_2eservice";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const TEST: &str = "manager_lifecycle_admission_fixture::capture_effective_config";
const MAX_REPLY: usize = 1024 * 1024;
const REFUSE: &str = "K1_PRIVATE_ADMISSION_UNCERTAIN_RETAINED";
type Result<T> = std::result::Result<T, &'static str>;

struct Properties<const LIMIT: usize = 512>(HashMap<String, OwnedValue>);

impl<const LIMIT: usize> Type for Properties<LIMIT> {
    const SIGNATURE: &'static zbus::zvariant::Signature = <HashMap<String, OwnedValue>>::SIGNATURE;
}

impl<'de, const LIMIT: usize> serde::Deserialize<'de> for Properties<LIMIT> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Unique<const LIMIT: usize>;
        impl<'de, const LIMIT: usize> serde::de::Visitor<'de> for Unique<LIMIT> {
            type Value = Properties<LIMIT>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("bounded unique D-Bus property dictionary")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Properties<LIMIT>, M::Error> {
                let mut values = HashMap::new();
                while let Some((key, value)) = map.next_entry::<String, OwnedValue>()? {
                    if values.len() >= LIMIT
                        || key.is_empty()
                        || key.len() > 128
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || values.contains_key(&key)
                    {
                        return Err(serde::de::Error::custom("invalid property dictionary"));
                    }
                    values.insert(key, value);
                }
                Ok(Properties(values))
            }
        }
        deserializer.deserialize_map(Unique::<LIMIT>)
    }
}

fn require(ok: bool) -> Result<()> {
    if ok { Ok(()) } else { Err(REFUSE) }
}

fn decode<T: DeserializeOwned + Type>(message: &zbus::Message) -> Result<T> {
    // The library has already received the message (upstream limit 128 MiB).
    // This is a tighter decoding/output bound, not a preallocation bound.
    require(message.data().len() <= MAX_REPLY && message.data().fds().is_empty())?;
    message.body().deserialize().map_err(|_| REFUSE)
}

fn text(values: &HashMap<String, OwnedValue>, key: &str) -> Result<String> {
    let value = values.get(key).ok_or(REFUSE)?;
    <&str>::try_from(value)
        .map(str::to_owned)
        .map_err(|_| REFUSE)
}

fn number(values: &HashMap<String, OwnedValue>, key: &str) -> Result<u64> {
    u64::try_from(values.get(key).ok_or(REFUSE)?).map_err(|_| REFUSE)
}

fn identity(values: &HashMap<String, OwnedValue>) -> Result<Value> {
    require(text(values, "Id")? == UNIT)?;
    require(text(values, "LoadState")? == "loaded")?;
    require(text(values, "FragmentPath")? == FRAGMENT)?;
    require(text(values, "ActiveState")? == "inactive")?;
    require(text(values, "SubState")? == "dead")?;
    // Empty containers can convert without checking element types. Inspect the
    // variant signature before conversion, so an empty `at` cannot stand for `as`.
    require(
        values.get("DropInPaths").ok_or(REFUSE)?.value_signature() == Vec::<String>::SIGNATURE,
    )?;
    let drops = Vec::<String>::try_from(
        values
            .get("DropInPaths")
            .ok_or(REFUSE)?
            .try_clone()
            .map_err(|_| REFUSE)?,
    )
    .map_err(|_| REFUSE)?;
    require(drops.is_empty())?;
    let job_value = values.get("Job").ok_or(REFUSE)?;
    require(job_value.value_signature() == <(u32, zbus::zvariant::OwnedObjectPath)>::SIGNATURE)?;
    let job = <(u32, zbus::zvariant::OwnedObjectPath)>::try_from(
        job_value.try_clone().map_err(|_| REFUSE)?,
    )
    .map_err(|_| REFUSE)?;
    require(job.0 == 0 && job.1.as_str() == "/")?;
    Ok(
        json!({"Id": UNIT, "LoadState": "loaded", "FragmentPath": FRAGMENT,
        "ActiveState": "inactive", "SubState": "dead", "DropInPaths": [],
        "Job": {"type": "(uo)", "data": [0, "/"]}}),
    )
}

fn selected_service(values: &HashMap<String, OwnedValue>) -> Result<Value> {
    // Capture exact typed facts; in particular, never equate the pre-start
    // watchdog value with the configured WatchdogSec value in the dump.
    let mut result = serde_json::Map::new();
    for key in ["StandardOutput", "StandardError", "Type", "User", "Group"] {
        result.insert(key.to_owned(), json!(text(values, key)?));
    }
    for key in ["WatchdogUSec", "ExecMainStartTimestampMonotonic"] {
        result.insert(key.to_owned(), json!(number(values, key)?));
    }
    for key in ["MainPID", "ControlPID", "ExecMainPID"] {
        let pid = u32::try_from(values.get(key).ok_or(REFUSE)?).map_err(|_| REFUSE)?;
        require(pid == 0)?;
        result.insert(key.to_owned(), json!(pid));
    }
    require(number(values, "ExecMainStartTimestampMonotonic")? == 0)?;
    require(text(values, "ControlGroup")?.is_empty())?;
    result.insert("ControlGroup".to_owned(), json!(""));
    Ok(Value::Object(result))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    Owner,
    VersionBefore,
    Ref,
    Unit,
    Service,
    Dump,
    VersionAfter,
    Unref,
    PostUnrefAll,
}

trait FixedBus {
    fn request(&self, owner: &str, request: Request) -> Result<zbus::Message>;
}

impl FixedBus for Connection {
    fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
        let reply = match request {
            Request::Owner => self.call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetNameOwner",
                &("org.freedesktop.systemd1",),
            ),
            Request::Ref => self.call_method(
                Some(owner),
                MANAGER_PATH,
                Some(MANAGER),
                "RefUnit",
                &(UNIT,),
            ),
            Request::VersionBefore | Request::VersionAfter => self.call_method(
                Some(owner),
                MANAGER_PATH,
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(MANAGER, "Version"),
            ),
            Request::Unit => self.call_method(
                Some(owner),
                UNIT_PATH,
                Some("org.freedesktop.DBus.Properties"),
                "GetAll",
                &("org.freedesktop.systemd1.Unit",),
            ),
            Request::Service => self.call_method(
                Some(owner),
                UNIT_PATH,
                Some("org.freedesktop.DBus.Properties"),
                "GetAll",
                &("org.freedesktop.systemd1.Service",),
            ),
            Request::Dump => self.call_method(
                Some(owner),
                MANAGER_PATH,
                Some(MANAGER),
                "DumpUnitsMatchingPatterns",
                &(vec![UNIT],),
            ),
            Request::Unref => self.call_method(
                Some(owner),
                MANAGER_PATH,
                Some(MANAGER),
                "UnrefUnit",
                &(UNIT,),
            ),
            Request::PostUnrefAll => self.call_method(
                Some(owner),
                UNIT_PATH,
                Some("org.freedesktop.DBus.Properties"),
                "GetAll",
                &("",),
            ),
        };
        reply.map_err(|_| REFUSE)
    }
}

struct Held<B = Connection> {
    // Leaked before the first possible RefUnit. No destructor/Unref on failure.
    connection: B,
    stage: File,
}

fn write_exclusive(stage: &File, name: &'static str, bytes: &[u8]) -> Result<()> {
    use nix::fcntl::{OFlag, openat};
    use nix::sys::stat::Mode;
    let fd = openat(
        stage,
        name,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(|_| REFUSE)?;
    let mut output = File::from(fd);
    output.write_all(bytes).map_err(|_| REFUSE)?;
    output.sync_all().map_err(|_| REFUSE)?;
    stage.sync_all().map_err(|_| REFUSE)
}

fn manager_version(message: &zbus::Message) -> Result<String> {
    let value: OwnedValue = decode(message)?;
    let text = <&str>::try_from(&value).map_err(|_| REFUSE)?;
    require(!text.is_empty() && text.len() <= 256 && !text.chars().any(char::is_control))?;
    // Unknown complete distro strings are metadata, never normalized/admitted.
    Ok(text.to_owned())
}

fn capture(held: &Held<impl FixedBus>) -> Result<()> {
    // Pin the manager's unique bus identity. Never follow a replacement owner.
    let reply = held.connection.request("", Request::Owner)?;
    let owner: String = decode(&reply)?;
    zbus::names::UniqueName::try_from(owner.as_str()).map_err(|_| REFUSE)?;
    let version_before =
        manager_version(&held.connection.request(&owner, Request::VersionBefore)?)?;
    require(version_before == "261.2-1-arch")?;
    let reference = held.connection.request(&owner, Request::Ref)?;
    decode::<()>(&reference)?;
    let unit_properties = decode::<Properties>(&held.connection.request(&owner, Request::Unit)?)?;
    let unit = identity(&unit_properties.0)?;
    crate::manager_lifecycle_permissions::check_unit(&unit_properties.0).map_err(|_| REFUSE)?;
    let service_properties =
        decode::<Properties>(&held.connection.request(&owner, Request::Service)?)?;
    let service = selected_service(&service_properties.0)?;
    let permission_fields =
        crate::manager_lifecycle_permissions::recorded(&unit_properties.0, &service_properties.0)
            .map_err(|_| REFUSE)?;
    let reply = held.connection.request(&owner, Request::Dump)?;
    let dump: String = decode(&reply)?;
    require(!dump.is_empty() && dump.len() <= MAX_REPLY && !dump.contains('\0'))?;
    crate::manager_lifecycle_admission_dump::proposed_text_matches(
        &version_before,
        UNIT,
        FRAGMENT,
        crate::manager_lifecycle_admission_dump::UNIT_SHA,
        &dump,
    )
    .map_err(|_| REFUSE)?;
    // An invalid configured dump is terminal BEFORE any subsequent RPC.
    let version_after = manager_version(&held.connection.request(&owner, Request::VersionAfter)?)?;
    require(version_before == version_after)?;
    let version = json!({"schema": 2, "marker": "OBSERVED_VERSION_DATA_NOT_ADMISSION",
        "unique_owner": owner, "unit": UNIT,
        "before_ref": {"type": "s", "data": [version_before]},
        "while_ref_after_dump": {"type": "s", "data": [version_after]},
        "admission": false});
    let version_bytes = serde_json::to_vec(&version).map_err(|_| REFUSE)?;
    require(version_bytes.len() <= 4096)?;
    write_exclusive(
        &held.stage,
        "private-admission-manager-version.json",
        &version_bytes,
    )?;
    // These are configured text facts only. Unit original-FD continuity remains
    // an outer requirement; runtime WatchdogUSec is separately phase-specific.
    let evidence = json!({"schema": 2, "marker": "OBSERVED_CONFIGURED_FACTS_NOT_LIFECYCLE_ADMISSION",
        "unit": unit, "service": service, "permission_fields": permission_fields,
        "dump": {"type": "s", "data": [dump]}});
    let bytes = serde_json::to_vec(&evidence).map_err(|_| REFUSE)?;
    require(bytes.len() <= 2 * MAX_REPLY)?;
    write_exclusive(&held.stage, "private-admission-config-data.json", &bytes)?;
    // Only the complete known capture permits the sole explicit Unref.
    let reply = held.connection.request(&owner, Request::Unref)?;
    decode::<()>(&reply)?;
    // One dispatch collects all interfaces. The exact object-path fallback can
    // load a unit collected after Unref; absence/missing facts are never success.
    // This is not another Ref and does not follow a replacement manager owner.
    let current =
        decode::<Properties<1024>>(&held.connection.request(&owner, Request::PostUnrefAll)?)?;
    let post_state = json!({"schema": 2, "phase": "post-unref-single-getall",
        "unit": identity(&current.0)?, "service": selected_service(&current.0)?,
        "permission_fields": crate::manager_lifecycle_permissions::recorded(&current.0, &current.0).map_err(|_| REFUSE)?,
        "admission": false});
    let post_bytes = serde_json::to_vec(&post_state).map_err(|_| REFUSE)?;
    require(post_bytes.len() <= 16384)?;
    write_exclusive(
        &held.stage,
        "private-admission-post-unref-state.json",
        &post_bytes,
    )?;
    write_exclusive(
        &held.stage,
        "private-admission-unref-ack.json",
        b"{\"schema\":2,\"unref_acknowledged\":true,\"admission\":false}\n",
    )?;
    Ok(())
}

#[test]
#[ignore = "root-reviewed fixed metadata capture; dedicated VM lease only, never ordinary cargo"]
fn capture_effective_config() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_PRIVATE_ADMISSION").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        [
            "--exact",
            TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1"
        ]
    );
    assert_eq!(nix::unistd::getuid().as_raw(), 0);
    assert_eq!(nix::unistd::geteuid().as_raw(), 0);
    let stage = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(STAGE)
        .expect("K1_PRIVATE_ADMISSION_STAGE_REFUSED");
    let meta = stage
        .metadata()
        .expect("K1_PRIVATE_ADMISSION_STAGE_REFUSED");
    assert!(meta.is_dir() && meta.uid() == 0 && meta.gid() == 0 && meta.mode() & 0o7777 == 0o700);
    // Literal transport: DBUS_SYSTEM_BUS_ADDRESS and caller arguments cannot
    // redirect it. Construction/send stalls are bounded by the outer observer;
    // method_timeout bounds waiting for a reply, not the whole connection setup.
    let connection =
        zbus::blocking::connection::Builder::address("unix:path=/run/dbus/system_bus_socket")
            .expect("K1_PRIVATE_ADMISSION_ADDRESS_REFUSED")
            .max_queued(8)
            .method_timeout(Duration::from_secs(5))
            .build()
            .expect("K1_PRIVATE_ADMISSION_CONNECT_REFUSED");
    let held = Box::leak(Box::new(Held { connection, stage }));
    if !matches!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| capture(held))),
        Ok(Ok(()))
    ) {
        eprintln!("{REFUSE}");
        loop {
            std::thread::park();
        }
    }
    println!("K1_PRIVATE_ADMISSION_CAPTURED_UNREF_ACKNOWLEDGED_NOT_ADMISSION");
}

#[test]
fn missing_and_wrong_typed_identity_refuse() {
    assert!(identity(&HashMap::new()).is_err());
    let mut values = HashMap::new();
    values.insert("Id".to_owned(), OwnedValue::from(7_u64));
    assert!(identity(&values).is_err());
}

#[test]
fn pid_width_and_start_history_refuse() {
    assert!(selected_service(&HashMap::new()).is_err());
    let mut values = HashMap::new();
    values.insert("MainPID".to_owned(), OwnedValue::from(0_u64));
    assert!(u32::try_from(values.get("MainPID").unwrap()).is_err());
}

#[cfg(test)]
mod controls {
    use super::*;
    use std::cell::RefCell;
    use zbus::zvariant::{DynamicType, Str};

    const ORDER: [Request; 9] = [
        Request::Owner,
        Request::VersionBefore,
        Request::Ref,
        Request::Unit,
        Request::Service,
        Request::Dump,
        Request::VersionAfter,
        Request::Unref,
        Request::PostUnrefAll,
    ];

    fn reply<T: serde::Serialize + DynamicType>(body: &T) -> zbus::Message {
        let call = zbus::Message::method_call("/fixture", "Fixture")
            .unwrap()
            .build(&())
            .unwrap();
        zbus::Message::method_return(&call.header())
            .unwrap()
            .build(body)
            .unwrap()
    }

    fn string(value: &'static str) -> OwnedValue {
        OwnedValue::from(Str::from(value))
    }

    #[test]
    fn version_is_exact_bounded_typed_metadata_not_an_admitted_release() {
        for text in [
            "unknown-distribution 999.7+vendor",
            "261.2-custom",
            "synthetic-λ",
        ] {
            assert_eq!(manager_version(&reply(&string(text))).unwrap(), text);
        }
        assert!(
            manager_version(&reply(&"261.2")).is_err(),
            "must be variant(s), not s"
        );
        assert!(manager_version(&reply(&OwnedValue::from(261_u64))).is_err());
        for text in ["", "a\0b", "a\nb", "a\rb", "a\tb", "a\u{7f}b", "a\u{85}b"] {
            assert!(manager_version(&reply(&string(text))).is_err());
        }
        for text in ["x".repeat(257), "λ".repeat(129)] {
            let value = OwnedValue::from(Str::from(text.as_str()));
            assert!(manager_version(&reply(&value)).is_err());
        }
        let boundary = "x".repeat(256);
        let value = OwnedValue::from(Str::from(boundary.as_str()));
        assert_eq!(manager_version(&reply(&value)).unwrap(), boundary);
    }

    #[test]
    fn version_drift_or_output_collision_stops_before_unref() {
        struct Changed(Fake);
        impl FixedBus for Changed {
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if request == Request::VersionAfter {
                    assert_eq!(owner, ":1.77");
                    self.0.calls.borrow_mut().push(request);
                    Ok(reply(&string("SYNTHETIC-version-with-distro-CHANGED")))
                } else {
                    self.0.request(owner, request)
                }
            }
        }
        let path = crate::test_temp::directory("k1-version-drift").unwrap();
        let held = Held {
            connection: Changed(Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            }),
            stage: File::open(&path).unwrap(),
        };
        assert!(capture(&held).is_err());
        assert_eq!(*held.connection.0.calls.borrow(), ORDER[..7]);
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        std::fs::remove_dir(path).unwrap();

        let path = crate::test_temp::directory("k1-version-collision").unwrap();
        std::fs::write(
            path.join("private-admission-manager-version.json"),
            b"retained",
        )
        .unwrap();
        let held = Held {
            connection: Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            },
            stage: File::open(&path).unwrap(),
        };
        assert!(capture(&held).is_err());
        assert_eq!(*held.connection.calls.borrow(), ORDER[..7]);
        assert_eq!(
            std::fs::read(path.join("private-admission-manager-version.json")).unwrap(),
            b"retained"
        );
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 1);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn unapproved_version_stops_before_ref_and_malformed_dump_before_unref() {
        struct Changed(Fake, bool);
        impl FixedBus for Changed {
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if (self.1 && request == Request::VersionBefore)
                    || (!self.1 && request == Request::Dump)
                {
                    self.0.calls.borrow_mut().push(request);
                    return Ok(if self.1 {
                        reply(&string("261.2"))
                    } else {
                        reply(&"SYNTHETIC_OPAQUE_NOT_CONFIGURED_GRAMMAR")
                    });
                }
                self.0.request(owner, request)
            }
        }
        for version in [true, false] {
            let path = crate::test_temp::directory("k1-configured-reject").unwrap();
            let held = Held {
                connection: Changed(
                    Fake {
                        calls: RefCell::new(Vec::new()),
                        fail: None,
                        malformed: false,
                    },
                    version,
                ),
                stage: File::open(&path).unwrap(),
            };
            assert!(capture(&held).is_err());
            assert_eq!(
                *held.connection.0.calls.borrow(),
                ORDER[..if version { 2 } else { 6 }]
            );
            assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
            std::fs::remove_dir(path).unwrap();
        }
    }

    fn unit_values() -> HashMap<String, OwnedValue> {
        let mut values: HashMap<_, _> = [
            ("Id", UNIT),
            ("LoadState", "loaded"),
            ("FragmentPath", FRAGMENT),
            ("ActiveState", "inactive"),
            ("SubState", "dead"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), string(v)))
        .collect();
        values.insert(
            "DropInPaths".to_owned(),
            OwnedValue::try_from(zbus::zvariant::Value::from(Vec::<String>::new())).unwrap(),
        );
        values.insert(
            "Job".to_owned(),
            OwnedValue::try_from(zbus::zvariant::Value::from((
                0_u32,
                zbus::zvariant::ObjectPath::try_from("/").unwrap(),
            )))
            .unwrap(),
        );
        values.extend(crate::manager_lifecycle_permissions::expected_unit());
        values.insert(
            "Requires".to_owned(),
            OwnedValue::try_from(zbus::zvariant::Value::from(vec![
                "sysinit.target",
                "system.slice",
            ]))
            .unwrap(),
        );
        values
    }

    fn service_values() -> HashMap<String, OwnedValue> {
        let mut values: HashMap<_, _> = [
            ("StandardOutput", "append"),
            ("StandardError", "append"),
            ("Type", "oneshot"),
            ("User", "root"),
            ("Group", "root"),
            ("ControlGroup", ""),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), string(v)))
        .collect();
        for name in ["MainPID", "ControlPID", "ExecMainPID"] {
            values.insert(name.to_owned(), OwnedValue::from(0_u32));
        }
        values.insert(
            "ExecMainStartTimestampMonotonic".to_owned(),
            OwnedValue::from(0_u64),
        );
        values.insert("WatchdogUSec".to_owned(), OwnedValue::from(u64::MAX));
        values.extend(crate::manager_lifecycle_permissions::expected_service());
        values
    }

    struct Fake {
        calls: RefCell<Vec<Request>>,
        fail: Option<Request>,
        malformed: bool,
    }

    #[test]
    fn unsafe_unit_or_service_stops_at_that_reply_without_next_call_or_unref() {
        struct Unsafe(Fake, Request);
        impl FixedBus for Unsafe {
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if request == self.1 {
                    self.0.calls.borrow_mut().push(request);
                    let mut facts = if request == Request::Unit {
                        unit_values()
                    } else {
                        service_values()
                    };
                    let key = if request == Request::Unit {
                        "RefuseManualStart"
                    } else {
                        "PrivateNetwork"
                    };
                    facts.insert(key.to_owned(), OwnedValue::from(request == Request::Unit));
                    Ok(reply(&facts))
                } else {
                    self.0.request(owner, request)
                }
            }
        }
        for (bad, end) in [(Request::Unit, 4), (Request::Service, 5)] {
            let path = crate::test_temp::directory("k1-perm-refuse").unwrap();
            let held = Held {
                connection: Unsafe(
                    Fake {
                        calls: RefCell::new(Vec::new()),
                        fail: None,
                        malformed: false,
                    },
                    bad,
                ),
                stage: File::open(&path).unwrap(),
            };
            assert!(capture(&held).is_err());
            assert_eq!(*held.connection.0.calls.borrow(), ORDER[..end]);
            assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
            std::fs::remove_dir(path).unwrap();
        }
    }

    impl FixedBus for Fake {
        fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
            assert_eq!(
                owner,
                if request == Request::Owner {
                    ""
                } else {
                    ":1.77"
                }
            );
            self.calls.borrow_mut().push(request);
            if self.fail == Some(request) {
                return if self.malformed {
                    Ok(reply(&17_u64))
                } else {
                    Err(REFUSE)
                };
            }
            Ok(match request {
                Request::Owner => reply(&":1.77"),
                Request::VersionBefore | Request::VersionAfter => reply(&string("261.2-1-arch")),
                Request::Ref | Request::Unref => reply(&()),
                Request::Unit => reply(&unit_values()),
                Request::Service => reply(&service_values()),
                Request::PostUnrefAll => {
                    let mut all = unit_values();
                    all.extend(service_values());
                    reply(&all)
                }
                Request::Dump => {
                    reply(&crate::manager_lifecycle_admission_dump::tests::synthetic())
                }
            })
        }
    }

    #[test]
    fn actual_capture_stops_at_every_error_or_malformed_reply_without_compensating_unref() {
        for (index, fail) in ORDER.into_iter().enumerate() {
            for malformed in [false, true] {
                let path = crate::test_temp::directory("k1-ref-error").unwrap();
                let held = Held {
                    connection: Fake {
                        calls: RefCell::new(Vec::new()),
                        fail: Some(fail),
                        malformed,
                    },
                    stage: File::open(&path).unwrap(),
                };
                assert!(capture(&held).is_err());
                assert_eq!(*held.connection.calls.borrow(), ORDER[..=index]);
                assert!(!path.join("private-admission-unref-ack.json").exists());
                std::fs::remove_dir_all(path).unwrap();
            }
        }
    }

    #[test]
    fn actual_capture_success_preserves_typed_infinity_without_admitting_it() {
        let path = crate::test_temp::directory("k1-ref-good").unwrap();
        let held = Held {
            connection: Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            },
            stage: File::open(&path).unwrap(),
        };
        capture(&held).unwrap();
        assert_eq!(*held.connection.calls.borrow(), ORDER);
        let version: Value = serde_json::from_slice(
            &std::fs::read(path.join("private-admission-manager-version.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(version["schema"], 2);
        assert_eq!(version["unique_owner"], ":1.77");
        assert_eq!(version["unit"], UNIT);
        assert_eq!(version["before_ref"], version["while_ref_after_dump"]);
        assert_eq!(
            version["before_ref"],
            json!({"type": "s", "data": ["261.2-1-arch"]})
        );
        assert_eq!(version["admission"], false);
        let data: Value = serde_json::from_slice(
            &std::fs::read(path.join("private-admission-config-data.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(data["service"]["WatchdogUSec"], json!(u64::MAX));
        assert_eq!(
            data["marker"],
            "OBSERVED_CONFIGURED_FACTS_NOT_LIFECYCLE_ADMISSION"
        );
        assert_eq!(
            std::fs::metadata(path.join("private-admission-config-data.json"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        assert!(path.join("private-admission-unref-ack.json").exists());
        let post: Value = serde_json::from_slice(
            &std::fs::read(path.join("private-admission-post-unref-state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(post["phase"], "post-unref-single-getall");
        assert_eq!(
            post["unit"]["Job"],
            json!({"type": "(uo)", "data": [0, "/"]})
        );
        assert_eq!(post["service"]["ControlGroup"], "");
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn existing_output_prevents_unref_and_is_not_overwritten() {
        let path = crate::test_temp::directory("k1-ref-exists").unwrap();
        std::fs::write(path.join("private-admission-config-data.json"), b"sentinel").unwrap();
        let held = Held {
            connection: Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            },
            stage: File::open(&path).unwrap(),
        };
        assert!(capture(&held).is_err());
        assert_eq!(*held.connection.calls.borrow(), ORDER[..7]);
        assert_eq!(
            std::fs::read(path.join("private-admission-config-data.json")).unwrap(),
            b"sentinel"
        );
        assert!(!path.join("private-admission-unref-ack.json").exists());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn duplicate_and_oversized_property_dictionary_refuse() {
        struct Entries(Vec<(String, u64)>);
        impl Type for Entries {
            const SIGNATURE: &'static zbus::zvariant::Signature = Properties::<512>::SIGNATURE;
        }
        impl serde::Serialize for Entries {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for (key, value) in &self.0 {
                    map.serialize_entry(key, &OwnedValue::from(*value))?;
                }
                map.end()
            }
        }
        for entries in [
            vec![
                ("WatchdogUSec".to_owned(), 0),
                ("WatchdogUSec".to_owned(), u64::MAX),
            ],
            (0..513).map(|i| (format!("Field{i}"), 0)).collect(),
            vec![("x".repeat(129), 0)],
            vec![("wrong.name".to_owned(), 0)],
        ] {
            assert!(decode::<Properties>(&reply(&Entries(entries))).is_err());
        }
        assert_eq!(
            decode::<Properties>(&reply(&Entries(vec![("Field".to_owned(), 7)])))
                .unwrap()
                .0
                .len(),
            1
        );
        let combined = Entries((0..1024).map(|i| (format!("Field{i}"), 0)).collect());
        assert_eq!(
            decode::<Properties<1024>>(&reply(&combined))
                .unwrap()
                .0
                .len(),
            1024
        );
        assert!(decode::<Properties>(&reply(&combined)).is_err());
        let excess = Entries((0..1025).map(|i| (format!("Field{i}"), 0)).collect());
        assert!(decode::<Properties<1024>>(&reply(&excess)).is_err());
        let duplicate = Entries(vec![("Job".to_owned(), 0), ("Job".to_owned(), 0)]);
        assert!(decode::<Properties<1024>>(&reply(&duplicate)).is_err());
        let oversized = reply(&"x".repeat(MAX_REPLY));
        assert!(decode::<String>(&oversized).is_err());
    }

    #[test]
    fn wrong_unit_dropins_and_prior_start_refuse() {
        let mut values = unit_values();
        values.insert("Id".to_owned(), string("other.service"));
        assert!(identity(&values).is_err());
        values = unit_values();
        values.insert(
            "DropInPaths".to_owned(),
            OwnedValue::try_from(zbus::zvariant::Value::from(Vec::<u64>::new())).unwrap(),
        );
        assert!(identity(&values).is_err());
        let mut service = service_values();
        service.insert(
            "ExecMainStartTimestampMonotonic".to_owned(),
            OwnedValue::from(1_u64),
        );
        assert!(selected_service(&service).is_err());
    }

    #[test]
    fn post_unref_invalid_current_state_stops_without_any_further_call_or_success_ack() {
        struct PostFake {
            inner: Fake,
            current: HashMap<String, OwnedValue>,
        }
        impl FixedBus for PostFake {
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if request == Request::PostUnrefAll {
                    assert_eq!(owner, ":1.77");
                    self.inner.calls.borrow_mut().push(request);
                    Ok(reply(&self.current))
                } else {
                    self.inner.request(owner, request)
                }
            }
        }
        for bad in 0..6 {
            let mut current = unit_values();
            current.extend(service_values());
            match bad {
                0 => {
                    current.remove("Job");
                }
                1 => {
                    current.insert("LoadState".to_owned(), string("not-found"));
                }
                2 => {
                    current.insert("MainPID".to_owned(), OwnedValue::from(9_u32));
                }
                3 => {
                    current.insert(
                        "ControlGroup".to_owned(),
                        string("/system.slice/other.service"),
                    );
                }
                4 => {
                    current.insert(
                        "Job".to_owned(),
                        OwnedValue::try_from(zbus::zvariant::Value::from((0_u32, "/"))).unwrap(),
                    );
                }
                _ => {
                    current.insert(
                        "Job".to_owned(),
                        OwnedValue::try_from(zbus::zvariant::Value::from((
                            1_u32,
                            zbus::zvariant::ObjectPath::try_from("/").unwrap(),
                        )))
                        .unwrap(),
                    );
                }
            }
            let path = crate::test_temp::directory("k1-ref-postbad").unwrap();
            let held = Held {
                connection: PostFake {
                    inner: Fake {
                        calls: RefCell::new(Vec::new()),
                        fail: None,
                        malformed: false,
                    },
                    current,
                },
                stage: File::open(&path).unwrap(),
            };
            assert!(capture(&held).is_err());
            assert_eq!(*held.connection.inner.calls.borrow(), ORDER);
            assert!(path.join("private-admission-config-data.json").exists());
            assert!(
                !path
                    .join("private-admission-post-unref-state.json")
                    .exists()
            );
            assert!(!path.join("private-admission-unref-ack.json").exists());
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn post_unref_output_collision_retains_prior_evidence_without_retry() {
        let path = crate::test_temp::directory("k1-ref-postexists").unwrap();
        std::fs::write(
            path.join("private-admission-post-unref-state.json"),
            b"sentinel",
        )
        .unwrap();
        let held = Held {
            connection: Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            },
            stage: File::open(&path).unwrap(),
        };
        assert!(capture(&held).is_err());
        assert_eq!(*held.connection.calls.borrow(), ORDER);
        assert_eq!(
            std::fs::read(path.join("private-admission-post-unref-state.json")).unwrap(),
            b"sentinel"
        );
        assert!(!path.join("private-admission-unref-ack.json").exists());
        std::fs::remove_dir_all(path).unwrap();
    }
}
