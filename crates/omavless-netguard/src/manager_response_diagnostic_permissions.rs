//! Pure developer-fixture permission checks. No bus calls or start authority.
//! Same-owner/version, raw unit/ELF FD continuity and phase remain caller duties.
use crate::manager_fixture_identity::Fixture;
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

pub(super) const UNIT: &str = "omavless-k1-retained-private-lifecycle.service";
pub(super) const STAGE: &str = "/run/omavless-k1-retained-private-lifecycle";
pub(super) const WRITER: &str =
    "kernel_observer::creator_lifecycle::response_diagnostic::manager_private_lifecycle";
type Facts = HashMap<String, OwnedValue>;
type Exec = (String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32);

fn put(values: &mut Facts, key: &str, value: impl Into<Value<'static>>) {
    // All expected values below are fixed local data, not received descriptors.
    values.insert(key.to_owned(), OwnedValue::try_from(value.into()).unwrap());
}

pub(super) fn expected_unit() -> Facts {
    expected_unit_for(Fixture::PrivateLifecycle)
}

pub(super) fn expected_unit_for(fixture: Fixture) -> Facts {
    let unit = fixture.unit();
    let mut values = Facts::new();
    for (key, value) in [
        ("Id", unit),
        ("Following", ""),
        ("LoadState", "loaded"),
        ("ActiveState", "inactive"),
        ("SubState", "dead"),
        ("FailureAction", "none"),
        ("SuccessAction", "none"),
    ] {
        put(&mut values, key, value);
    }
    put(&mut values, "Names", vec![unit.to_owned()]);
    put(&mut values, "FragmentPath", fixture.fragment().to_owned());
    for key in [
        "DropInPaths",
        "Wants",
        "BindsTo",
        "PartOf",
        "Upholds",
        "OnFailure",
        "OnSuccess",
        "TriggeredBy",
        "Requisite",
        "PropagatesStopTo",
        "StopPropagatedFrom",
        "JoinsNamespaceOf",
    ] {
        put(&mut values, key, Vec::<String>::new());
    }
    put(&mut values, "Conflicts", vec!["shutdown.target".to_owned()]);
    put(&mut values, "RefuseManualStart", false);
    put(&mut values, "RefuseManualStop", false);
    for key in ["JobTimeoutUSec", "JobRunningTimeoutUSec"] {
        put(&mut values, key, u64::MAX);
    }
    values
}

pub(super) fn expected_service() -> Facts {
    expected_service_for(Fixture::PrivateLifecycle)
}

pub(super) fn expected_service_for(fixture: Fixture) -> Facts {
    let mut values = Facts::new();
    for (key, value) in [
        ("Type", "oneshot"),
        ("User", "root"),
        ("Group", "root"),
        ("PrivatePIDs", "no"),
        ("Restart", "no"),
        ("KillMode", "none"),
        ("StandardInput", "null"),
        ("StandardOutput", "append"),
        ("StandardError", "append"),
        ("ProtectProc", "default"),
        ("ProcSubset", "all"),
        ("RootDirectory", ""),
        ("RootImage", ""),
        ("NetworkNamespacePath", ""),
        ("UserNamespacePath", ""),
        ("ControlGroup", ""),
    ] {
        put(&mut values, key, value);
    }
    for (key, value) in [
        ("RemainAfterExit", true),
        ("NoNewPrivileges", true),
        ("PrivateNetwork", true),
        ("PrivateUsers", false),
        ("Delegate", false),
        ("SendSIGKILL", false),
    ] {
        put(&mut values, key, value);
    }
    for (key, value) in [
        ("CapabilityBoundingSet", 0x1000_u64),
        ("AmbientCapabilities", 0),
        ("RestrictNamespaces", 0),
        ("TimeoutStartUSec", u64::MAX),
        ("TimeoutStopUSec", u64::MAX),
        ("RuntimeMaxUSec", u64::MAX),
        ("WatchdogUSec", u64::MAX),
        ("ExecMainStartTimestampMonotonic", 0),
    ] {
        put(&mut values, key, value);
    }
    for key in [
        "MainPID",
        "ControlPID",
        "ExecMainPID",
        "FileDescriptorStoreMax",
        "NFileDescriptorStore",
    ] {
        put(&mut values, key, 0_u32);
    }
    put(&mut values, "UMask", 0o77_u32);
    for key in [
        "PassEnvironment",
        "UnsetEnvironment",
        "SupplementaryGroups",
        "ExtraFileDescriptorNames",
        "ExtensionDirectories",
    ] {
        put(&mut values, key, Vec::<String>::new());
    }
    put(
        &mut values,
        "Environment",
        vec![fixture.environment().to_owned()],
    );
    put(
        &mut values,
        "EnvironmentFiles",
        Vec::<(String, bool)>::new(),
    );
    put(
        &mut values,
        "SystemCallFilter",
        (false, Vec::<String>::new()),
    );
    put(
        &mut values,
        "OpenFile",
        vec![(
            "/proc/1/ns/net".to_owned(),
            "k1-host-netns".to_owned(),
            1_u64,
        )],
    );
    for key in ["BindPaths", "BindReadOnlyPaths"] {
        put(&mut values, key, Vec::<(String, String, bool, u64)>::new());
    }
    put(
        &mut values,
        "TemporaryFileSystem",
        Vec::<(String, String)>::new(),
    );
    put(
        &mut values,
        "ExtensionImages",
        Vec::<(String, bool, Vec<(String, String)>)>::new(),
    );
    put(
        &mut values,
        "MountImages",
        Vec::<(String, String, bool, Vec<(String, String)>)>::new(),
    );
    for key in [
        "ExecCondition",
        "ExecStartPre",
        "ExecStartPost",
        "ExecReload",
        "ExecReloadPost",
        "ExecStop",
        "ExecStopPost",
    ] {
        put(&mut values, key, Vec::<Exec>::new());
    }
    let executable = format!("{}/probe", fixture.stage());
    let arguments = vec![
        executable.clone(),
        "--exact".into(),
        fixture.writer().into(),
        "--ignored".into(),
        "--nocapture".into(),
        "--test-threads=1".into(),
    ];
    put(
        &mut values,
        "ExecStart",
        vec![(
            executable, arguments, false, 0_u64, 0_u64, 0_u64, 0_u64, 0_u32, 0_i32, 0_i32,
        )],
    );
    values
}

fn exact_selected(actual: &Facts, expected: &Facts) -> Result<(), ()> {
    for (key, value) in expected {
        let observed = actual.get(key).ok_or(())?;
        // Explicit signature comparison also excludes wrongly typed empty arrays.
        if observed.value_signature() != value.value_signature() || observed != value {
            return Err(());
        }
    }
    Ok(())
}

pub(super) fn check_unit(unit: &Facts) -> Result<(), ()> {
    check_unit_for(Fixture::PrivateLifecycle, unit)
}

pub(super) fn check_unit_for(fixture: Fixture, unit: &Facts) -> Result<(), ()> {
    exact_selected(unit, &expected_unit_for(fixture))?;
    let value = unit.get("Requires").ok_or(())?;
    let first = OwnedValue::try_from(Value::from(vec!["sysinit.target", "system.slice"]))
        .map_err(|_| ())?;
    let second = OwnedValue::try_from(Value::from(vec!["system.slice", "sysinit.target"]))
        .map_err(|_| ())?;
    if value != &first && value != &second {
        return Err(());
    }
    Ok(())
}

pub(super) fn check(unit: &Facts, service: &Facts) -> Result<(), ()> {
    check_for(Fixture::PrivateLifecycle, unit, service)
}

pub(super) fn check_for(fixture: Fixture, unit: &Facts, service: &Facts) -> Result<(), ()> {
    check_unit_for(fixture, unit)?;
    exact_selected(service, &expected_service_for(fixture))
}

// Runtime phase validation is separate, not replacement values injected into
// the observed dictionary. Every unchanged security prerequisite still applies.
pub(super) fn check_lifecycle_stable(actual: &Facts) -> Result<(), ()> {
    check_lifecycle_stable_for(Fixture::PrivateLifecycle, actual)
}

pub(super) fn check_lifecycle_stable_for(fixture: Fixture, actual: &Facts) -> Result<(), ()> {
    let mut unit = expected_unit_for(fixture);
    unit.remove("ActiveState");
    unit.remove("SubState");
    exact_selected(actual, &unit)?;
    let requires = actual.get("Requires").ok_or(())?;
    let first = OwnedValue::try_from(Value::from(vec!["sysinit.target", "system.slice"]))
        .map_err(|_| ())?;
    let second = OwnedValue::try_from(Value::from(vec!["system.slice", "sysinit.target"]))
        .map_err(|_| ())?;
    if requires != &first && requires != &second {
        return Err(());
    }
    let mut service = expected_service_for(fixture);
    for runtime in [
        "ControlGroup",
        "WatchdogUSec",
        "ExecMainStartTimestampMonotonic",
        "MainPID",
        "ControlPID",
        "ExecMainPID",
        "ExecStart",
    ] {
        service.remove(runtime);
    }
    exact_selected(actual, &service)
}

pub(super) fn recorded(unit: &Facts, service: &Facts) -> Result<serde_json::Value, ()> {
    recorded_for(Fixture::PrivateLifecycle, unit, service)
}

pub(super) fn recorded_for(
    fixture: Fixture,
    unit: &Facts,
    service: &Facts,
) -> Result<serde_json::Value, ()> {
    check_for(fixture, unit, service)?;
    let mut selected_unit = expected_unit_for(fixture);
    put(&mut selected_unit, "Requires", Vec::<String>::new());
    let mut output = serde_json::Map::new();
    for (name, actual, selected) in [
        ("unit", unit, selected_unit),
        ("service", service, expected_service_for(fixture)),
    ] {
        let mut rows = serde_json::Map::new();
        for key in selected.keys() {
            rows.insert(
                key.clone(),
                serde_json::to_value(actual.get(key).ok_or(())?).map_err(|_| ())?,
            );
        }
        output.insert(name.to_owned(), serde_json::Value::Object(rows));
    }
    Ok(serde_json::Value::Object(output))
}

// Diagnostic projection only. Selecting a received value does not admit it.
pub(super) fn diagnostic(actual: &Facts, service: bool) -> Result<serde_json::Value, ()> {
    diagnostic_for(Fixture::PrivateLifecycle, actual, service)
}

pub(super) fn diagnostic_for(
    fixture: Fixture,
    actual: &Facts,
    service: bool,
) -> Result<serde_json::Value, ()> {
    let mut expected = if service {
        expected_service_for(fixture)
    } else {
        expected_unit_for(fixture)
    };
    if !service {
        put(
            &mut expected,
            "Requires",
            vec!["sysinit.target", "system.slice"],
        );
        put(
            &mut expected,
            "Job",
            (
                0_u32,
                zbus::zvariant::ObjectPath::try_from("/").map_err(|_| ())?,
            ),
        );
    }
    let mut selected = serde_json::Map::new();
    let mut mismatches = Vec::new();
    let mut keys: Vec<_> = expected.keys().collect();
    keys.sort();
    for key in keys {
        let value = actual.get(key);
        let matches = if !service && key == "Requires" {
            let reversed =
                OwnedValue::try_from(Value::from(vec!["system.slice", "sysinit.target"]))
                    .map_err(|_| ())?;
            value == expected.get(key) || value == Some(&reversed)
        } else {
            value.is_some_and(|v| {
                v.value_signature() == expected[key].value_signature() && v == &expected[key]
            })
        };
        if !matches {
            mismatches.push(key.clone());
        }
        selected.insert(key.clone(), match value {
            Some(value) => serde_json::json!({"present":true,"variant":serde_json::to_value(value).map_err(|_| ())?}),
            None => serde_json::json!({"present":false}),
        });
    }
    Ok(serde_json::json!({"selected":selected,"mismatch_fields":mismatches}))
}

#[test]
fn fresh_lease_permissions_are_not_legacy_fixture_admission() {
    assert_eq!(Fixture::PrivateLifecycle.writer(), WRITER);
    assert_eq!(Fixture::PrivateLifecycle.stage(), STAGE);
    assert_eq!(Fixture::PrivateLifecycle.unit(), UNIT);
    for fixture in [Fixture::PrivateLifecycle, Fixture::RetainedLease] {
        let mut unit = expected_unit_for(fixture);
        put(
            &mut unit,
            "Requires",
            vec!["system.slice", "sysinit.target"],
        );
        let service = expected_service_for(fixture);
        assert_eq!(check_for(fixture, &unit, &service), Ok(()));
        if fixture == Fixture::PrivateLifecycle {
            assert_eq!(check_unit(&unit), Ok(()));
            assert_eq!(
                diagnostic(&service, true),
                diagnostic_for(fixture, &service, true)
            );
        }
        let other = if fixture == Fixture::PrivateLifecycle {
            Fixture::RetainedLease
        } else {
            Fixture::PrivateLifecycle
        };
        assert!(check_for(other, &unit, &service).is_err());
        for key in ["Id", "Names", "FragmentPath"] {
            let mut crossed = expected_unit_for(fixture);
            put(
                &mut crossed,
                "Requires",
                vec!["system.slice", "sysinit.target"],
            );
            crossed.insert(key.into(), expected_unit_for(other).remove(key).unwrap());
            assert!(check_for(fixture, &crossed, &service).is_err());
        }
        for key in ["Environment", "ExecStart"] {
            let mut crossed = expected_service_for(fixture);
            crossed.insert(key.into(), expected_service_for(other).remove(key).unwrap());
            assert!(check_for(fixture, &unit, &crossed).is_err());
        }
        let records = recorded_for(fixture, &unit, &service).unwrap();
        assert_eq!(
            records["unit"]["Id"],
            serde_json::json!({"signature":"s","value":fixture.unit()})
        );
        let projected = diagnostic_for(fixture, &service, true).unwrap();
        assert_eq!(projected["mismatch_fields"], serde_json::json!([]));
    }
}

#[test]
fn exact_synthetic_permissions_are_not_runtime_or_namespace_proof() {
    let mut unit = expected_unit();
    put(
        &mut unit,
        "Requires",
        vec!["system.slice", "sysinit.target"],
    );
    assert_eq!(check(&unit, &expected_service()), Ok(()));
    let receipt = recorded(&unit, &expected_service()).unwrap();
    assert_eq!(
        receipt["service"]["CapabilityBoundingSet"],
        serde_json::json!({"signature": "t", "value": 4096})
    );
    assert_eq!(
        receipt["service"]["SystemCallFilter"],
        serde_json::json!({"signature": "(bas)", "value": [false, []]})
    );
    put(
        &mut unit,
        "Requires",
        vec!["sysinit.target", "system.slice"],
    );
    assert_eq!(check(&unit, &expected_service()), Ok(()));
    for bad in [
        vec![],
        vec!["system.slice"],
        vec!["system.slice", "system.slice"],
        vec!["sysinit.target", "system.slice", "other.target"],
    ] {
        put(&mut unit, "Requires", bad);
        assert!(check(&unit, &expected_service()).is_err());
    }
}

#[test]
fn supported_socket_and_alias_properties_are_required_not_missing_fallbacks() {
    let valid_unit = || {
        let mut values = expected_unit();
        put(
            &mut values,
            "Requires",
            vec!["sysinit.target", "system.slice"],
        );
        values
    };
    let unit = valid_unit();
    let service = expected_service();
    assert!(!service.contains_key("Sockets"));
    assert!(check(&unit, &service).is_ok());
    for key in ["Names", "Following", "TriggeredBy", "Wants"] {
        let mut missing = valid_unit();
        missing.remove(key);
        assert!(check(&missing, &service).is_err());
        let mut wrong_type = valid_unit();
        wrong_type.insert(key.into(), OwnedValue::from(false));
        assert!(check(&wrong_type, &service).is_err());
    }
    for (key, value) in [
        ("Names", vec![UNIT, "synthetic-alias.service"]),
        ("Names", vec![UNIT, UNIT]),
        ("Names", vec!["synthetic-other.service"]),
        ("TriggeredBy", vec!["synthetic.socket"]),
        ("Wants", vec!["synthetic.socket"]),
    ] {
        let mut wrong = valid_unit();
        put(&mut wrong, key, value);
        assert!(check(&wrong, &service).is_err());
    }
    let mut following = valid_unit();
    put(&mut following, "Following", "synthetic-other.service");
    assert!(check(&following, &service).is_err());
    let mut missing_service = expected_service();
    missing_service.remove("ExtraFileDescriptorNames");
    assert!(check(&unit, &missing_service).is_err());
}

#[test]
fn every_selected_field_is_required_and_wrong_type_refuses() {
    for expected in [expected_unit(), expected_service()] {
        for key in expected.keys() {
            let mut actual: Facts = expected
                .iter()
                .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
                .collect();
            actual.remove(key);
            assert!(exact_selected(&actual, &expected).is_err(), "missing {key}");
            put(&mut actual, key, Vec::<u8>::new());
            assert!(
                exact_selected(&actual, &expected).is_err(),
                "wrong signature {key}"
            );
        }
    }
}

#[test]
fn typed_but_unsafe_permissions_and_empty_container_lookalikes_refuse() {
    let expected = expected_service();
    let wrong: [(&str, Value<'static>); 12] = [
        ("CapabilityBoundingSet", Value::from(0x201000_u64)),
        ("AmbientCapabilities", Value::from(0x1000_u64)),
        ("PrivateNetwork", Value::from(false)),
        ("NoNewPrivileges", Value::from(false)),
        ("RestrictNamespaces", Value::from(0x40000000_u64)),
        ("WatchdogUSec", Value::from(0_u64)),
        ("ExecMainStartTimestampMonotonic", Value::from(1_u64)),
        (
            "SystemCallFilter",
            Value::from((true, Vec::<String>::new())),
        ),
        ("EnvironmentFiles", Value::from(Vec::<String>::new())),
        ("ExecStartPre", Value::from(Vec::<String>::new())),
        (
            "OpenFile",
            Value::from(vec![("/proc/other/ns/net", "k1-host-netns", 1_u64)]),
        ),
        ("PassEnvironment", Value::from(vec!["PYTHONPATH"])),
    ];
    for (key, value) in wrong {
        let mut actual: Facts = expected
            .iter()
            .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
            .collect();
        put(&mut actual, key, value);
        assert!(exact_selected(&actual, &expected).is_err(), "unsafe {key}");
    }
    let mut actual = expected_service();
    let executable = format!("{STAGE}/probe");
    put(
        &mut actual,
        "ExecStart",
        vec![(
            executable.clone(),
            vec![executable, "different-selector".into()],
            false,
            0_u64,
            0_u64,
            0_u64,
            0_u64,
            0_u32,
            0_i32,
            0_i32,
        )],
    );
    assert!(exact_selected(&actual, &expected).is_err());
}
