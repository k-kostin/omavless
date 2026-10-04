//! Developer-only metadata capture. No start, lifecycle or admission result.
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Type};

const UNIT: &str = "omavless-k1-effective-config-reference.service";
const STAGE: &str = "/run/omavless-k1-effective-config-reference";
const FRAGMENT: &str = "/run/systemd/system/omavless-k1-effective-config-reference.service";
const UNIT_PATH: &str =
    "/org/freedesktop/systemd1/unit/omavless_2dk1_2deffective_2dconfig_2dreference_2eservice";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const TEST: &str = "manager_config_reference_fixture::capture_effective_config";
const MAX_REPLY: usize = 1024 * 1024;
const REFUSE: &str = "K1_CONFIG_REFERENCE_UNCERTAIN_RETAINED";
type Result<T> = std::result::Result<T, &'static str>;

struct Properties(HashMap<String, OwnedValue>);

impl Type for Properties {
    const SIGNATURE: &'static zbus::zvariant::Signature = <HashMap<String, OwnedValue>>::SIGNATURE;
}

impl<'de> serde::Deserialize<'de> for Properties {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Unique;
        impl<'de> serde::de::Visitor<'de> for Unique {
            type Value = Properties;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("bounded unique D-Bus property dictionary")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Properties, M::Error> {
                let mut values = HashMap::new();
                while let Some((key, value)) = map.next_entry::<String, OwnedValue>()? {
                    if values.len() >= 512
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
        deserializer.deserialize_map(Unique)
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
    Ok(
        json!({"Id": UNIT, "LoadState": "loaded", "FragmentPath": FRAGMENT,
        "ActiveState": "inactive", "SubState": "dead", "DropInPaths": []}),
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
    Ok(Value::Object(result))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    Owner,
    Ref,
    Unit,
    Service,
    Dump,
    Unref,
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

fn capture(held: &Held<impl FixedBus>) -> Result<()> {
    // Pin the manager's unique bus identity. Never follow a replacement owner.
    let reply = held.connection.request("", Request::Owner)?;
    let owner: String = decode(&reply)?;
    zbus::names::UniqueName::try_from(owner.as_str()).map_err(|_| REFUSE)?;
    let reference = held.connection.request(&owner, Request::Ref)?;
    decode::<()>(&reference)?;
    let unit =
        identity(&decode::<Properties>(&held.connection.request(&owner, Request::Unit)?)?.0)?;
    let service = selected_service(
        &decode::<Properties>(&held.connection.request(&owner, Request::Service)?)?.0,
    )?;
    let reply = held.connection.request(&owner, Request::Dump)?;
    let dump: String = decode(&reply)?;
    require(!dump.is_empty() && dump.len() <= MAX_REPLY && !dump.contains('\0'))?;
    // Deliberately NO grammar-based configured-path or watchdog acceptance.
    // Root must review the actual private shape before a successor parser exists.
    let evidence = json!({"schema": 1, "marker": "OBSERVED_CONFIG_DATA_NOT_ADMISSION",
        "unit": unit, "service": service, "dump": {"type": "s", "data": [dump]}});
    let bytes = serde_json::to_vec(&evidence).map_err(|_| REFUSE)?;
    require(bytes.len() <= 2 * MAX_REPLY)?;
    write_exclusive(&held.stage, "reference-config-data.json", &bytes)?;
    // Only the complete known capture permits the sole explicit Unref.
    let reply = held.connection.request(&owner, Request::Unref)?;
    decode::<()>(&reply)?;
    write_exclusive(
        &held.stage,
        "reference-unref-ack.json",
        b"{\"schema\":1,\"unref_acknowledged\":true,\"admission\":false}\n",
    )?;
    Ok(())
}

#[test]
#[ignore = "root-reviewed fixed metadata capture; dedicated VM lease only, never ordinary cargo"]
fn capture_effective_config() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_CONFIG_REFERENCE").as_deref(),
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
        .expect("K1_CONFIG_REFERENCE_STAGE_REFUSED");
    let meta = stage.metadata().expect("K1_CONFIG_REFERENCE_STAGE_REFUSED");
    assert!(meta.is_dir() && meta.uid() == 0 && meta.gid() == 0 && meta.mode() & 0o7777 == 0o700);
    // Literal transport: DBUS_SYSTEM_BUS_ADDRESS and caller arguments cannot
    // redirect it. Construction/send stalls are bounded by the outer observer;
    // method_timeout bounds waiting for a reply, not the whole connection setup.
    let connection =
        zbus::blocking::connection::Builder::address("unix:path=/run/dbus/system_bus_socket")
            .expect("K1_CONFIG_REFERENCE_ADDRESS_REFUSED")
            .max_queued(8)
            .method_timeout(Duration::from_secs(5))
            .build()
            .expect("K1_CONFIG_REFERENCE_CONNECT_REFUSED");
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
    println!("K1_CONFIG_REFERENCE_CAPTURED_UNREF_ACKNOWLEDGED_NOT_ADMISSION");
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

    const ORDER: [Request; 6] = [
        Request::Owner,
        Request::Ref,
        Request::Unit,
        Request::Service,
        Request::Dump,
        Request::Unref,
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
        values
    }

    fn service_values() -> HashMap<String, OwnedValue> {
        let mut values: HashMap<_, _> = [
            ("StandardOutput", "append"),
            ("StandardError", "append"),
            ("Type", "oneshot"),
            ("User", "root"),
            ("Group", "root"),
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
        values
    }

    struct Fake {
        calls: RefCell<Vec<Request>>,
        fail: Option<Request>,
        malformed: bool,
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
                Request::Ref | Request::Unref => reply(&()),
                Request::Unit => reply(&unit_values()),
                Request::Service => reply(&service_values()),
                // Not a proposed systemd grammar or a configured-proof parser.
                Request::Dump => reply(&"SYNTHETIC_OPAQUE_CAPTURE_ONLY"),
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
                assert!(!path.join("reference-unref-ack.json").exists());
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
        let data: Value = serde_json::from_slice(
            &std::fs::read(path.join("reference-config-data.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(data["service"]["WatchdogUSec"], json!(u64::MAX));
        assert_eq!(data["marker"], "OBSERVED_CONFIG_DATA_NOT_ADMISSION");
        assert_eq!(
            std::fs::metadata(path.join("reference-config-data.json"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        assert!(path.join("reference-unref-ack.json").exists());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn existing_output_prevents_unref_and_is_not_overwritten() {
        let path = crate::test_temp::directory("k1-ref-exists").unwrap();
        std::fs::write(path.join("reference-config-data.json"), b"sentinel").unwrap();
        let held = Held {
            connection: Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            },
            stage: File::open(&path).unwrap(),
        };
        assert!(capture(&held).is_err());
        assert_eq!(*held.connection.calls.borrow(), ORDER[..5]);
        assert_eq!(
            std::fs::read(path.join("reference-config-data.json")).unwrap(),
            b"sentinel"
        );
        assert!(!path.join("reference-unref-ack.json").exists());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn duplicate_and_oversized_property_dictionary_refuse() {
        struct Entries(Vec<(String, u64)>);
        impl Type for Entries {
            const SIGNATURE: &'static zbus::zvariant::Signature = Properties::SIGNATURE;
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
}
