//! Developer-only configured facts. No start, lifecycle or ownership admission.
use crate::manager_fixture_identity::Fixture;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Type};

const UNIT: &str = "omavless-k1-retained-private-lifecycle.service";
const STAGE: &str = "/run/omavless-k1-retained-private-lifecycle";
const FRAGMENT: &str = "/run/systemd/system/omavless-k1-retained-private-lifecycle.service";
const UNIT_PATH: &str =
    "/org/freedesktop/systemd1/unit/omavless_2dk1_2dretained_2dprivate_2dlifecycle_2eservice";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const TEST: &str = "manager_response_diagnostic_fixture::capture_effective_config";
const MAX_REPLY: usize = 1024 * 1024;
const REFUSE: &str = "K1_RETAINED_LIFECYCLE_UNCERTAIN_RETAINED";
type Result<T> = std::result::Result<T, &'static str>;

pub(super) struct Properties<const LIMIT: usize = 512>(pub(super) HashMap<String, OwnedValue>);

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

pub(super) fn decode<T: DeserializeOwned + Type>(message: &zbus::Message) -> Result<T> {
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
    identity_for(Fixture::PrivateLifecycle, values)
}

fn identity_for(fixture: Fixture, values: &HashMap<String, OwnedValue>) -> Result<Value> {
    require(text(values, "Id")? == fixture.unit())?;
    require(text(values, "LoadState")? == "loaded")?;
    require(text(values, "FragmentPath")? == fixture.fragment())?;
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
        json!({"Id": fixture.unit(), "LoadState": "loaded", "FragmentPath": fixture.fragment(),
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
pub(super) enum Request {
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

pub(super) trait FixedBus {
    fn request(&self, owner: &str, request: Request) -> Result<zbus::Message>;
    fn fixture(&self) -> Fixture {
        Fixture::PrivateLifecycle
    }
}

impl FixedBus for Connection {
    fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
        request_for(self, Fixture::PrivateLifecycle, owner, request)
    }
}

pub(super) struct FixedConnection {
    pub(super) connection: Connection,
    pub(super) fixture: Fixture,
}

impl FixedBus for FixedConnection {
    fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
        request_for(&self.connection, self.fixture, owner, request)
    }
    fn fixture(&self) -> Fixture {
        self.fixture
    }
}

fn request_for(
    connection: &Connection,
    fixture: Fixture,
    owner: &str,
    request: Request,
) -> Result<zbus::Message> {
    let unit = fixture.unit();
    let unit_path = fixture.unit_path();
    let reply = match request {
        Request::Owner => connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &("org.freedesktop.systemd1",),
        ),
        Request::Ref => connection.call_method(
            Some(owner),
            MANAGER_PATH,
            Some(MANAGER),
            "RefUnit",
            &(unit,),
        ),
        Request::VersionBefore | Request::VersionAfter => connection.call_method(
            Some(owner),
            MANAGER_PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(MANAGER, "Version"),
        ),
        Request::Unit => connection.call_method(
            Some(owner),
            unit_path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("org.freedesktop.systemd1.Unit",),
        ),
        Request::Service => connection.call_method(
            Some(owner),
            unit_path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("org.freedesktop.systemd1.Service",),
        ),
        Request::Dump => connection.call_method(
            Some(owner),
            MANAGER_PATH,
            Some(MANAGER),
            "DumpUnitsMatchingPatterns",
            &(vec![unit],),
        ),
        Request::Unref => connection.call_method(
            Some(owner),
            MANAGER_PATH,
            Some(MANAGER),
            "UnrefUnit",
            &(unit,),
        ),
        Request::PostUnrefAll => connection.call_method(
            Some(owner),
            unit_path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("",),
        ),
    };
    reply.map_err(|_| REFUSE)
}

pub(super) struct Held<B = Connection> {
    // Leaked before the first possible RefUnit. No destructor/Unref on failure.
    pub(super) connection: B,
    pub(super) stage: File,
}

impl<B: FixedBus> Held<B> {
    fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
        let fixture = self.connection.fixture();
        let (before, after) = match request {
            Request::Owner => ("rpc-00-before.json", "rpc-00-response.json"),
            Request::VersionBefore => ("rpc-01-before.json", "rpc-01-response.json"),
            Request::Ref => ("rpc-02-before.json", "rpc-02-response.json"),
            Request::Unit => ("rpc-03-before.json", "rpc-03-response.json"),
            Request::Service => ("rpc-04-before.json", "rpc-04-response.json"),
            Request::Dump => ("rpc-05-before.json", "rpc-05-response.json"),
            Request::VersionAfter => ("rpc-06-before.json", "rpc-06-response.json"),
            Request::Unref => ("rpc-07-before.json", "rpc-07-response.json"),
            Request::PostUnrefAll => ("rpc-08-before.json", "rpc-08-response.json"),
        };
        write_exclusive(
            &self.stage,
            before,
            b"{\"schema\":1,\"diagnostic\":true,\"boundary\":\"before-rpc\",\"admission\":false}\n",
        )?;
        let reply = self.connection.request(owner, request)?;
        let observation = (|| -> Result<(bool, Value)> {
            match request {
                Request::Owner => {
                    let value: String = decode(&reply)?;
                    Ok((
                        zbus::names::UniqueName::try_from(value.as_str()).is_ok(),
                        json!({"owner":value}),
                    ))
                }
                Request::VersionBefore | Request::VersionAfter => {
                    let value = manager_version(&reply)?;
                    Ok((value == "261.2-1-arch", json!({"version":value})))
                }
                Request::Ref | Request::Unref => {
                    decode::<()>(&reply)?;
                    Ok((true, json!({"empty_ack":true})))
                }
                Request::Unit => {
                    let fields = decode::<Properties>(&reply)?.0;
                    let facts = crate::manager_response_diagnostic_permissions::diagnostic_for(
                        fixture, &fields, false,
                    )
                    .map_err(|_| REFUSE)?;
                    Ok((
                        identity_for(fixture, &fields).is_ok()
                            && crate::manager_response_diagnostic_permissions::check_unit_for(
                                fixture, &fields,
                            )
                            .is_ok(),
                        facts,
                    ))
                }
                Request::Service => {
                    let fields = decode::<Properties>(&reply)?.0;
                    let facts = crate::manager_response_diagnostic_permissions::diagnostic_for(
                        fixture, &fields, true,
                    )
                    .map_err(|_| REFUSE)?;
                    let valid = selected_service(&fields).is_ok()
                        && facts["mismatch_fields"]
                            .as_array()
                            .is_some_and(Vec::is_empty);
                    Ok((valid, facts))
                }
                Request::Dump => {
                    let dump: String = decode(&reply)?;
                    require(dump.len() <= 64 * 1024)?;
                    let valid = crate::manager_response_diagnostic_dump::proposed_text_matches_for(
                        fixture,
                        "261.2-1-arch",
                        fixture.unit(),
                        fixture.fragment(),
                        fixture.unit_sha(),
                        &dump,
                    )
                    .is_ok();
                    Ok((
                        valid,
                        json!({"dump":dump,"mismatch_fields":if valid {vec![]} else {vec!["configured-dump"]}}),
                    ))
                }
                Request::PostUnrefAll => {
                    let fields = decode::<Properties<1024>>(&reply)?.0;
                    let unit = crate::manager_response_diagnostic_permissions::diagnostic_for(
                        fixture, &fields, false,
                    )
                    .map_err(|_| REFUSE)?;
                    let service = crate::manager_response_diagnostic_permissions::diagnostic_for(
                        fixture, &fields, true,
                    )
                    .map_err(|_| REFUSE)?;
                    Ok((
                        identity_for(fixture, &fields).is_ok()
                            && selected_service(&fields).is_ok()
                            && crate::manager_response_diagnostic_permissions::check_for(
                                fixture, &fields, &fields,
                            )
                            .is_ok(),
                        json!({"unit":unit,"service":service}),
                    ))
                }
            }
        })();
        let (valid, facts) =
            observation.unwrap_or((false, json!({"mismatch_fields":["decode-or-bounds"]})));
        let raw = serde_json::to_vec(&json!({"schema":1,"diagnostic":true,"admission":false,"validation_passed":valid,"facts":facts})).map_err(|_| REFUSE)?;
        require(raw.len() <= 2 * MAX_REPLY)?;
        write_exclusive(&self.stage, after, &raw)?;
        require(valid)?;
        Ok(reply)
    }
}

pub(super) fn write_exclusive(stage: &File, name: &str, bytes: &[u8]) -> Result<()> {
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

pub(super) fn admit_retaining_reference(
    held: &Held<impl FixedBus>,
    before_ref: impl FnOnce() -> std::result::Result<(), ()>,
) -> Result<String> {
    admitted_prefix_checked(held, before_ref)
}

fn admitted_prefix(held: &Held<impl FixedBus>) -> Result<String> {
    admitted_prefix_checked(held, || Ok(()))
}

fn admitted_prefix_checked(
    held: &Held<impl FixedBus>,
    before_ref: impl FnOnce() -> std::result::Result<(), ()>,
) -> Result<String> {
    let fixture = held.connection.fixture();
    // Pin the manager's unique bus identity. Never follow a replacement owner.
    let reply = held.request("", Request::Owner)?;
    let owner: String = decode(&reply)?;
    zbus::names::UniqueName::try_from(owner.as_str()).map_err(|_| REFUSE)?;
    let version_before = manager_version(&held.request(&owner, Request::VersionBefore)?)?;
    require(version_before == "261.2-1-arch")?;
    before_ref().map_err(|_| REFUSE)?;
    let reference = held.request(&owner, Request::Ref)?;
    decode::<()>(&reference)?;
    let unit_properties = decode::<Properties>(&held.request(&owner, Request::Unit)?)?;
    let unit = identity_for(fixture, &unit_properties.0)?;
    crate::manager_response_diagnostic_permissions::check_unit_for(fixture, &unit_properties.0)
        .map_err(|_| REFUSE)?;
    let service_properties = decode::<Properties>(&held.request(&owner, Request::Service)?)?;
    let service = selected_service(&service_properties.0)?;
    let permission_fields = crate::manager_response_diagnostic_permissions::recorded_for(
        fixture,
        &unit_properties.0,
        &service_properties.0,
    )
    .map_err(|_| REFUSE)?;
    let reply = held.request(&owner, Request::Dump)?;
    let dump: String = decode(&reply)?;
    require(!dump.is_empty() && dump.len() <= MAX_REPLY && !dump.contains('\0'))?;
    crate::manager_response_diagnostic_dump::proposed_text_matches_for(
        fixture,
        &version_before,
        fixture.unit(),
        fixture.fragment(),
        fixture.unit_sha(),
        &dump,
    )
    .map_err(|_| REFUSE)?;
    // An invalid configured dump is terminal BEFORE any subsequent RPC.
    let version_after = manager_version(&held.request(&owner, Request::VersionAfter)?)?;
    require(version_before == version_after)?;
    let version = json!({"schema": 2, "marker": "OBSERVED_VERSION_DATA_NOT_ADMISSION",
        "unique_owner": owner, "unit": fixture.unit(),
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
    Ok(owner)
}

fn capture(held: &Held<impl FixedBus>) -> Result<()> {
    let owner = admitted_prefix(held)?;
    // Only the complete known capture permits the sole explicit Unref.
    let reply = held.request(&owner, Request::Unref)?;
    decode::<()>(&reply)?;
    // One dispatch collects all interfaces. The exact object-path fallback can
    // load a unit collected after Unref; absence/missing facts are never success.
    // This is not another Ref and does not follow a replacement manager owner.
    let current = decode::<Properties<1024>>(&held.request(&owner, Request::PostUnrefAll)?)?;
    let post_state = json!({"schema": 2, "phase": "post-unref-single-getall",
        "unit": identity(&current.0)?, "service": selected_service(&current.0)?,
        "permission_fields": crate::manager_response_diagnostic_permissions::recorded(&current.0, &current.0).map_err(|_| REFUSE)?,
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
        std::env::var("OMAVLESS_K1_RETAINED_LIFECYCLE").as_deref(),
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
        .expect("K1_RETAINED_LIFECYCLE_STAGE_REFUSED");
    let meta = stage
        .metadata()
        .expect("K1_RETAINED_LIFECYCLE_STAGE_REFUSED");
    assert!(meta.is_dir() && meta.uid() == 0 && meta.gid() == 0 && meta.mode() & 0o7777 == 0o700);
    // Literal transport: DBUS_SYSTEM_BUS_ADDRESS and caller arguments cannot
    // redirect it. Construction/send stalls are bounded by the outer observer;
    // method_timeout bounds waiting for a reply, not the whole connection setup.
    let connection =
        zbus::blocking::connection::Builder::address("unix:path=/run/dbus/system_bus_socket")
            .expect("K1_RETAINED_LIFECYCLE_ADDRESS_REFUSED")
            .max_queued(8)
            .method_timeout(Duration::from_secs(5))
            .build()
            .expect("K1_RETAINED_LIFECYCLE_CONNECT_REFUSED");
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
    println!("K1_RETAINED_LIFECYCLE_CAPTURED_UNREF_ACKNOWLEDGED_NOT_ADMISSION");
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

    #[test]
    fn literal_object_path_belongs_only_to_this_unit() {
        let escaped = UNIT
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() {
                    char::from(byte).to_string()
                } else {
                    format!("_{byte:02x}")
                }
            })
            .collect::<String>();
        assert_eq!(
            UNIT_PATH,
            format!("/org/freedesktop/systemd1/unit/{escaped}")
        );
        assert_eq!(UNIT, crate::manager_response_diagnostic_permissions::UNIT);
        assert_eq!(STAGE, crate::manager_response_diagnostic_permissions::STAGE);
    }

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
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 14);
        std::fs::remove_dir_all(path).unwrap();

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
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 15);
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
            assert_eq!(
                std::fs::read_dir(&path).unwrap().count(),
                if version { 4 } else { 12 }
            );
            std::fs::remove_dir_all(path).unwrap();
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
        values.extend(crate::manager_response_diagnostic_permissions::expected_unit());
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
        values.extend(crate::manager_response_diagnostic_permissions::expected_service());
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
            assert_eq!(std::fs::read_dir(&path).unwrap().count(), end * 2);
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn supported_socket_and_alias_failures_stop_on_pre_or_post_reply() {
        struct Unsafe(Fake, Request, &'static str, bool);
        impl FixedBus for Unsafe {
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if request != self.1 {
                    return self.0.request(owner, request);
                }
                self.0.calls.borrow_mut().push(request);
                let mut facts = unit_values();
                if request == Request::PostUnrefAll {
                    facts.extend(service_values());
                }
                if self.3 {
                    facts.remove(self.2);
                } else if self.2 == "Following" {
                    facts.insert(self.2.into(), string("other.service"));
                } else {
                    let names = if self.2 == "Names" {
                        vec![UNIT.to_owned(), "alias.service".to_owned()]
                    } else {
                        vec!["synthetic.socket".to_owned()]
                    };
                    facts.insert(
                        self.2.into(),
                        OwnedValue::try_from(zbus::zvariant::Value::from(names)).unwrap(),
                    );
                }
                Ok(reply(&facts))
            }
        }
        for (request, end) in [(Request::Unit, 4), (Request::PostUnrefAll, 9)] {
            for field in ["Names", "Following", "TriggeredBy", "Wants"] {
                for missing in [false, true] {
                    let path = crate::test_temp::directory("k1-socket-refuse").unwrap();
                    let held = Held {
                        connection: Unsafe(
                            Fake {
                                calls: RefCell::new(Vec::new()),
                                fail: None,
                                malformed: false,
                            },
                            request,
                            field,
                            missing,
                        ),
                        stage: File::open(&path).unwrap(),
                    };
                    assert!(capture(&held).is_err());
                    assert_eq!(*held.connection.0.calls.borrow(), ORDER[..end]);
                    assert!(
                        !path
                            .join("private-admission-post-unref-state.json")
                            .exists()
                    );
                    // Pure fake bus: no live manager reference or child exists.
                    std::fs::remove_dir_all(path).unwrap();
                }
            }
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
                    reply(&crate::manager_response_diagnostic_dump::tests::synthetic())
                }
            })
        }
    }

    #[test]
    fn fresh_fixed_identity_uses_same_seven_rpc_prefix_and_crossed_replies_stop() {
        struct Fresh {
            fake: Fake,
            crossed: Option<Request>,
        }
        impl FixedBus for Fresh {
            fn fixture(&self) -> Fixture {
                Fixture::RetainedLease
            }
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if self.crossed == Some(request)
                    || !matches!(request, Request::Unit | Request::Service | Request::Dump)
                {
                    return self.fake.request(owner, request);
                }
                assert_eq!(owner, ":1.77");
                self.fake.calls.borrow_mut().push(request);
                Ok(match request {
                    Request::Unit => {
                        let mut facts = unit_values();
                        facts.extend(
                            crate::manager_response_diagnostic_permissions::expected_unit_for(
                                self.fixture(),
                            ),
                        );
                        reply(&facts)
                    }
                    Request::Service => reply(
                        &crate::manager_response_diagnostic_permissions::expected_service_for(
                            self.fixture(),
                        ),
                    ),
                    Request::Dump => reply(
                        &crate::manager_response_diagnostic_dump::tests::synthetic_for(
                            self.fixture(),
                        ),
                    ),
                    _ => unreachable!(),
                })
            }
        }
        for crossed in [
            None,
            Some(Request::Unit),
            Some(Request::Service),
            Some(Request::Dump),
        ] {
            let path = crate::test_temp::directory("k1-lease-prefix").unwrap();
            let held = Held {
                connection: Fresh {
                    fake: Fake {
                        calls: RefCell::new(Vec::new()),
                        fail: None,
                        malformed: false,
                    },
                    crossed,
                },
                stage: File::open(&path).unwrap(),
            };
            let result = admit_retaining_reference(&held, || Ok(()));
            if let Some(request) = crossed {
                assert!(result.is_err());
                let count = ORDER.iter().position(|value| *value == request).unwrap() + 1;
                assert_eq!(*held.connection.fake.calls.borrow(), ORDER[..count]);
            } else {
                assert_eq!(result.unwrap(), ":1.77");
                assert_eq!(*held.connection.fake.calls.borrow(), ORDER[..7]);
                let raw =
                    std::fs::read(path.join("private-admission-manager-version.json")).unwrap();
                let value: Value = serde_json::from_slice(&raw).unwrap();
                assert_eq!(value["unit"], Fixture::RetainedLease.unit());
            }
            // Pure injected bus, no actual manager reference or child exists.
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn failed_original_witness_recheck_prevents_ref_and_all_later_rpcs() {
        let path = crate::test_temp::directory("k1-pre-ref").unwrap();
        let held = Held {
            connection: Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            },
            stage: File::open(&path).unwrap(),
        };
        assert!(admitted_prefix_checked(&held, || Err(())).is_err());
        assert_eq!(*held.connection.calls.borrow(), ORDER[..2]);
        assert!(!path.join("rpc-02-before.json").exists());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn every_diagnostic_write_collision_stops_before_next_rpc() {
        for index in 0..ORDER.len() {
            for boundary in ["before", "response"] {
                let path = crate::test_temp::directory("k1-rpc-collision").unwrap();
                let name = format!("rpc-{index:02}-{boundary}.json");
                std::fs::write(path.join(&name), b"retained-original").unwrap();
                let held = Held {
                    connection: Fake {
                        calls: RefCell::new(Vec::new()),
                        fail: None,
                        malformed: false,
                    },
                    stage: File::open(&path).unwrap(),
                };
                assert!(capture(&held).is_err());
                let called = index + usize::from(boundary == "response");
                assert_eq!(*held.connection.calls.borrow(), ORDER[..called]);
                assert_eq!(
                    std::fs::read(path.join(name)).unwrap(),
                    b"retained-original"
                );
                // Only fake bus values exist: no real process/reference owner to clean.
                std::fs::remove_dir_all(path).unwrap();
            }
        }
    }

    #[test]
    fn diagnostic_wrong_type_is_private_exact_field_and_stops() {
        struct Wrong(Fake);
        impl FixedBus for Wrong {
            fn request(&self, owner: &str, request: Request) -> Result<zbus::Message> {
                if request == Request::Service {
                    self.0.calls.borrow_mut().push(request);
                    let mut facts = service_values();
                    facts.insert("PrivateNetwork".into(), OwnedValue::from(1_u32));
                    return Ok(reply(&facts));
                }
                self.0.request(owner, request)
            }
        }
        let path = crate::test_temp::directory("k1-rpc-type").unwrap();
        let held = Held {
            connection: Wrong(Fake {
                calls: RefCell::new(Vec::new()),
                fail: None,
                malformed: false,
            }),
            stage: File::open(&path).unwrap(),
        };
        assert!(capture(&held).is_err());
        assert_eq!(*held.connection.0.calls.borrow(), ORDER[..5]);
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(path.join("rpc-04-response.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["validation_passed"], false);
        assert_eq!(receipt["admission"], false);
        assert_eq!(
            receipt["facts"]["mismatch_fields"],
            json!(["PrivateNetwork"])
        );
        assert_eq!(
            receipt["facts"]["selected"]["PrivateNetwork"]["variant"]["signature"],
            "u"
        );
        assert!(!path.join("rpc-05-before.json").exists());
        std::fs::remove_dir_all(path).unwrap();
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
