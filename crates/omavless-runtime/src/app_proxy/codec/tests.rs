// SPDX-License-Identifier: MIT

use super::*;

fn desktop_entries() -> Vec<DesktopEntry> {
    DesktopKey::ALL
        .into_iter()
        .map(|key| {
            let value = match key.schema_key_type().2 {
                "s" if key == DesktopKey::Mode => DesktopValue::String("none".into()),
                "s" => DesktopValue::String(String::new()),
                "b" => DesktopValue::Bool(false),
                "i" => DesktopValue::Int(0),
                "as" => DesktopValue::Strings(vec!["localhost".into(), "::1".into()]),
                _ => unreachable!(),
            };
            DesktopEntry {
                key,
                effective: value.clone(),
                default: value,
                user: Override::Absent,
                writable: true,
            }
        })
        .collect()
}

fn environment_entries() -> Vec<EnvironmentEntry> {
    EnvironmentKey::ALL
        .into_iter()
        .map(|key| EnvironmentEntry {
            key,
            value: EnvironmentValue::Absent,
        })
        .collect()
}

fn bytes(value: &str) -> Snapshot {
    Snapshot::new(Some(value.as_bytes().to_vec())).unwrap()
}

#[test]
fn effective_default_and_explicit_equal_default_are_distinct() {
    let initial = DesktopSnapshot::capture(desktop_entries()).unwrap();
    let mut entries = desktop_entries();
    entries[0].user = Override::Present(entries[0].default.clone());
    let explicit = DesktopSnapshot::capture(entries).unwrap();
    assert_ne!(initial, explicit);
    assert_ne!(initial.encode().unwrap(), explicit.encode().unwrap());
    for snapshot in [initial, explicit] {
        assert_eq!(snapshot.admit_writes(), Ok(()));
        assert_eq!(
            DesktopSnapshot::decode(&snapshot.encode().unwrap()).unwrap(),
            snapshot
        );
    }
}

#[test]
fn locked_override_is_captured_but_never_admitted_for_writing() {
    let mut entries = desktop_entries();
    entries[0].user = Override::Present(DesktopValue::String("manual".into()));
    entries[0].writable = false;
    let locked = DesktopSnapshot::capture(entries.clone()).unwrap();
    assert_eq!(locked.admit_writes(), Err(Error::NotWritable));
    assert_eq!(
        DesktopSnapshot::decode(&locked.encode().unwrap()).unwrap(),
        locked
    );
    entries[0].writable = true;
    assert_eq!(
        DesktopSnapshot::capture(entries).unwrap().admit_writes(),
        Err(Error::InconsistentReadback)
    );
}

#[test]
fn complete_allowlist_required_and_canonical_order_independent() {
    let mut entries = desktop_entries();
    let original = DesktopSnapshot::capture(entries.clone())
        .unwrap()
        .encode()
        .unwrap();
    entries.reverse();
    assert_eq!(
        DesktopSnapshot::capture(entries.clone())
            .unwrap()
            .encode()
            .unwrap(),
        original
    );
    entries.pop();
    assert_eq!(
        DesktopSnapshot::capture(entries),
        Err(Error::IncompleteSnapshot)
    );
    let mut entries = desktop_entries();
    entries[1] = entries[0].clone();
    assert_eq!(
        DesktopSnapshot::capture(entries),
        Err(Error::IncompleteSnapshot)
    );

    let mut entries = environment_entries();
    let original = EnvironmentSnapshot::capture(entries.clone())
        .unwrap()
        .encode()
        .unwrap();
    entries.reverse();
    assert_eq!(
        EnvironmentSnapshot::capture(entries.clone())
            .unwrap()
            .encode()
            .unwrap(),
        original
    );
    entries[1] = entries[0].clone();
    assert_eq!(
        EnvironmentSnapshot::capture(entries),
        Err(Error::IncompleteSnapshot)
    );
    assert_eq!(
        EnvironmentSnapshot::capture(Vec::new()),
        Err(Error::IncompleteSnapshot)
    );
}

#[test]
fn literal_environment_bytes_preserve_absent_empty_case_and_metacharacters() {
    let mut entries = environment_entries();
    entries[1].value = EnvironmentValue::Present(String::new());
    // Synthetic text stays data, including quote, newline, equals and shell syntax.
    entries[2].value = EnvironmentValue::Present("literal='x'=\n$(false);`false`\\é".into());
    let snapshot = EnvironmentSnapshot::capture(entries).unwrap();
    let encoded = snapshot.encode().unwrap();
    assert_eq!(EnvironmentSnapshot::decode(&encoded).unwrap(), snapshot);
    assert_ne!(snapshot.entries()[0].value, snapshot.entries()[1].value);
    let json: serde_json::Value = serde_json::from_slice(encoded.bytes().unwrap()).unwrap();
    assert_eq!(
        json["entries"][0],
        serde_json::json!({"key":"http_proxy","value":{"state":"absent"}})
    );
    assert_eq!(
        json["entries"][1],
        serde_json::json!({"key":"HTTP_PROXY","value":{"state":"present","value":""}})
    );
    assert_eq!(
        snapshot.admit_writes(ActivationEnvironment::SystemdBroker),
        Ok(())
    );
    for classification in [
        ActivationEnvironment::Unknown,
        ActivationEnvironment::SeparateDbusDaemon,
    ] {
        assert_eq!(
            snapshot.admit_writes(classification),
            Err(Error::UnsupportedActivation)
        );
    }
}

#[test]
fn desktop_types_ranges_and_embedded_nul_are_refused_without_normalization() {
    for (key, value) in [
        (DesktopKey::Mode, DesktopValue::String("MANUAL".into())),
        (DesktopKey::Mode, DesktopValue::String("unknown".into())),
        (DesktopKey::HttpPort, DesktopValue::Int(-1)),
        (DesktopKey::SocksPort, DesktopValue::Int(65536)),
        (DesktopKey::HttpHost, DesktopValue::Bool(false)),
        (DesktopKey::HttpHost, DesktopValue::String("x\0y".into())),
        (
            DesktopKey::HttpHost,
            DesktopValue::String("x".repeat(MAX_STRING + 1)),
        ),
        (
            DesktopKey::IgnoreHosts,
            DesktopValue::Strings(vec!["".into(); MAX_BYPASS_ENTRIES + 1]),
        ),
    ] {
        let mut entries = desktop_entries();
        entries
            .iter_mut()
            .find(|entry| entry.key == key)
            .unwrap()
            .effective = value;
        assert_eq!(
            DesktopSnapshot::capture(entries),
            Err(Error::InvalidSnapshot)
        );
    }
    for value in ["x\0y".to_owned(), "x".repeat(MAX_STRING + 1)] {
        let mut entries = environment_entries();
        entries[0].value = EnvironmentValue::Present(value);
        assert_eq!(
            EnvironmentSnapshot::capture(entries),
            Err(Error::InvalidSnapshot)
        );
    }
    let mut entries = desktop_entries();
    entries[2].default = DesktopValue::Strings(vec!["x".repeat(MAX_STRING); MAX_BYPASS_ENTRIES]);
    assert_eq!(
        DesktopSnapshot::capture(entries),
        Err(Error::InvalidSnapshot)
    );
}

#[test]
fn strict_document_decoder_rejects_duplicates_unknown_missing_and_wrong_surface() {
    let snapshot = EnvironmentSnapshot::capture(environment_entries())
        .unwrap()
        .encode()
        .unwrap();
    let text = String::from_utf8(snapshot.bytes().unwrap().to_vec()).unwrap();
    for broken in [
        text.replacen(
            "\"surface\":\"user_manager\"",
            "\"surface\":\"user_manager\",\"surface\":\"user_manager\"",
            1,
        ),
        text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        text.replacen("\"version\":1", "\"version\":2", 1),
        text.replacen("\"version\":1", "\"version\":1,\"command\":\"echo\"", 1),
        text.replacen("\"key\":\"http_proxy\"", "\"key\":\"PATH\"", 1),
        text.replacen(
            "\"key\":\"http_proxy\"",
            "\"key\":\"http_proxy\",\"key\":\"http_proxy\"",
            1,
        ),
        text.replacen(
            "\"state\":\"absent\"",
            "\"state\":\"absent\",\"state\":\"absent\"",
            1,
        ),
        text.replacen(
            "\"state\":\"absent\"",
            "\"state\":\"absent\",\"value\":\"ignored?\"",
            1,
        ),
        text.replacen("\"state\":\"absent\"", "\"state\":\"present\"", 1),
        text.replacen(
            "\"state\":\"absent\"",
            "\"state\":\"present\",\"value\":null",
            1,
        ),
        text.replacen("\"value\":{\"state\":\"absent\"}", "\"other\":false", 1),
        format!("{text} trailing"),
    ] {
        assert!(EnvironmentSnapshot::decode(&bytes(&broken)).is_err());
    }
    assert_eq!(
        DesktopSnapshot::decode(&snapshot),
        Err(Error::InvalidSnapshot)
    );
    assert_eq!(
        EnvironmentSnapshot::decode(&Snapshot::new(None).unwrap()),
        Err(Error::IncompleteSnapshot)
    );
    assert!(EnvironmentSnapshot::decode(&bytes("")).is_err());
    assert!(EnvironmentSnapshot::decode(&Snapshot::new(Some(vec![0xff])).unwrap()).is_err());
    let snapshot = DesktopSnapshot::capture(desktop_entries())
        .unwrap()
        .encode()
        .unwrap();
    let text = String::from_utf8(snapshot.bytes().unwrap().to_vec()).unwrap();
    for broken in [
        text.replacen("\"user\":{\"state\":\"absent\"},", "", 1),
        text.replacen(
            "\"writable\":true",
            "\"writable\":true,\"writable\":false",
            1,
        ),
        text.replacen(
            "\"type\":\"string\"",
            "\"type\":\"string\",\"type\":\"string\"",
            1,
        ),
        text.replacen(
            "\"value\":\"none\"",
            "\"value\":\"none\",\"value\":\"auto\"",
            1,
        ),
    ] {
        assert!(DesktopSnapshot::decode(&bytes(&broken)).is_err());
    }
}

#[test]
fn typed_pair_restores_through_lease_and_changed_default_refuses() {
    use crate::app_proxy::{Owner, ProxyLease};
    let owner = Owner {
        instance: [2; 16],
        generation: 9,
    };
    let desktop = DesktopSnapshot::capture(desktop_entries()).unwrap();
    let environment = EnvironmentSnapshot::capture(environment_entries()).unwrap();
    let original = [desktop.encode().unwrap(), environment.encode().unwrap()];
    let mut entries = desktop.entries().to_vec();
    entries[0].effective = DesktopValue::String("manual".into());
    entries[0].user = Override::Present(entries[0].effective.clone());
    let target_desktop = DesktopSnapshot::capture(entries).unwrap();
    let mut entries = environment.entries().to_vec();
    entries[0].value = EnvironmentValue::Present("http://127.0.0.1:17890".into());
    let target = [
        target_desktop.encode().unwrap(),
        EnvironmentSnapshot::capture(entries)
            .unwrap()
            .encode()
            .unwrap(),
    ];
    let mut current = original.clone();
    let mut lease = ProxyLease::prepare(owner, original.clone(), target.clone());
    while let Some(effect) = lease.begin_next(owner, &current).unwrap() {
        current[effect.surface.index()] = effect.replacement;
        lease.confirm(owner, &current).unwrap();
    }
    let mut foreign_entries = target_desktop.entries().to_vec();
    foreign_entries[0].default = DesktopValue::String("auto".into());
    let foreign = [
        DesktopSnapshot::capture(foreign_entries)
            .unwrap()
            .encode()
            .unwrap(),
        current[1].clone(),
    ];
    assert_eq!(
        lease.begin_restore(owner, &foreign),
        Err(crate::app_proxy::Error::ForeignChange)
    );
    lease.begin_restore(owner, &current).unwrap();
    while let Some(effect) = lease.begin_next(owner, &current).unwrap() {
        current[effect.surface.index()] = effect.replacement;
        lease.confirm(owner, &current).unwrap();
    }
    assert_eq!(DesktopSnapshot::decode(&current[0]).unwrap(), desktop);
    assert_eq!(
        EnvironmentSnapshot::decode(&current[1]).unwrap(),
        environment
    );
}

#[test]
fn snapshots_values_and_entries_never_debug_private_values() {
    let secret = "synthetic-sensitive-value";
    let mut entries = desktop_entries();
    entries[9].effective = DesktopValue::String(secret.into());
    entries[9].user = Override::Present(DesktopValue::String(secret.into()));
    let snapshot = DesktopSnapshot::capture(entries).unwrap();
    for debug in [
        format!("{snapshot:?}"),
        format!("{:?}", snapshot.entries()),
        format!("{:?}", snapshot.encode().unwrap()),
    ] {
        assert!(!debug.contains(secret));
    }
    let mut entries = environment_entries();
    entries[0].value = EnvironmentValue::Present(secret.into());
    let snapshot = EnvironmentSnapshot::capture(entries).unwrap();
    assert!(!format!("{snapshot:?} {:?}", snapshot.entries()).contains(secret));
}
