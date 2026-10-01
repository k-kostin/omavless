// SPDX-License-Identifier: MIT

use crate::local_bus::{LocalBus, call as dbus_call};
use crate::{Error, Observation, Provenance, project_manager_environment};
use gio::glib::{self, prelude::ToVariant};
use gio::prelude::*;
use omavless_runtime::app_proxy::codec::{
    DesktopEntry, DesktopKey, DesktopSnapshot, DesktopValue, Override,
};

const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER_IFACE: &str = "org.freedesktop.systemd1.Manager";
const MAX_MANAGER_REPLY: usize = 256 * 1024;

const SCHEMAS: [(&str, &str, usize); 5] = [
    ("org.gnome.system.proxy", "/system/proxy/", 4),
    ("org.gnome.system.proxy.http", "/system/proxy/http/", 6),
    ("org.gnome.system.proxy.https", "/system/proxy/https/", 2),
    ("org.gnome.system.proxy.ftp", "/system/proxy/ftp/", 2),
    ("org.gnome.system.proxy.socks", "/system/proxy/socks/", 2),
];

pub(super) fn observe() -> Result<Observation, Error> {
    // The default schema/backend is used only when the caller has not replaced
    // it via the process environment. This is still not session provenance.
    if std::env::var_os("GSETTINGS_SCHEMA_DIR").is_some()
        || std::env::var_os("GSETTINGS_BACKEND").is_some()
        || std::env::var_os("GIO_EXTRA_MODULES").is_some()
        || std::env::var_os("GIO_MODULE_DIR").is_some()
        || std::env::var_os("DCONF_PROFILE").is_some()
    {
        return Err(Error::UnsupportedBackend);
    }
    let source = gio::SettingsSchemaSource::default().ok_or(Error::UnsupportedSchema)?;
    let schemas = validate_schemas(&source)?;
    let bus = LocalBus::connect()?;

    let desktop = read_desktop(&schemas)?;
    let manager = read_manager_environment(&bus.connection, bus.manager_owner())?;
    bus.revalidate()?;
    let desktop_again = read_desktop(&schemas)?;
    let manager_again = read_manager_environment(&bus.connection, bus.manager_owner())?;
    bus.revalidate()?;
    if desktop != desktop_again || manager != manager_again {
        return Err(Error::IncompleteObservation);
    }
    Ok(Observation {
        desktop,
        manager,
        provenance: Provenance::Unverified,
    })
}

fn validate_schemas(source: &gio::SettingsSchemaSource) -> Result<Vec<gio::SettingsSchema>, Error> {
    let mut schemas = Vec::with_capacity(SCHEMAS.len());
    for (id, path, key_count) in SCHEMAS {
        let schema = source.lookup(id, true).ok_or(Error::UnsupportedSchema)?;
        if schema.id() != id || schema.path().as_deref() != Some(path) {
            return Err(Error::UnsupportedSchema);
        }
        if schema.list_keys().len() != key_count {
            return Err(Error::UnsupportedSchema);
        }
        schemas.push(schema);
    }
    // Verify every expected key before any Settings object is constructed.
    for key in DesktopKey::ALL {
        let (schema_id, name, signature) = key.schema_key_type();
        let schema = schemas
            .iter()
            .find(|schema| schema.id() == schema_id)
            .ok_or(Error::UnsupportedSchema)?;
        if !schema.has_key(name) {
            return Err(Error::UnsupportedSchema);
        }
        let schema_key = schema.key(name);
        if schema_key.value_type().as_str() != signature {
            return Err(Error::UnsupportedSchema);
        }
        verify_range(key, &schema_key)?;
    }
    Ok(schemas)
}

fn verify_range(key: DesktopKey, schema_key: &gio::SettingsSchemaKey) -> Result<(), Error> {
    let range = schema_key.range();
    let (kind, bounds) = range
        .get::<(String, glib::Variant)>()
        .ok_or(Error::UnsupportedSchema)?;
    match key {
        DesktopKey::Mode => {
            let choices = bounds
                .get::<Vec<String>>()
                .ok_or(Error::UnsupportedSchema)?;
            if kind != "enum" || choices != ["none", "manual", "auto"] {
                return Err(Error::UnsupportedSchema);
            }
        }
        DesktopKey::HttpPort
        | DesktopKey::HttpsPort
        | DesktopKey::FtpPort
        | DesktopKey::SocksPort => {
            if kind != "range" || bounds.get::<(i32, i32)>() != Some((0, 65535)) {
                return Err(Error::UnsupportedSchema);
            }
        }
        _ => {
            if kind != "type" {
                return Err(Error::UnsupportedSchema);
            }
        }
    }
    Ok(())
}

fn read_desktop(schemas: &[gio::SettingsSchema]) -> Result<DesktopSnapshot, Error> {
    read_desktop_with_backend(schemas, None, true)
}

fn read_desktop_with_backend(
    schemas: &[gio::SettingsSchema],
    backend: Option<&gio::SettingsBackend>,
    require_dconf: bool,
) -> Result<DesktopSnapshot, Error> {
    let settings: Vec<_> = schemas
        .iter()
        .map(|schema| gio::Settings::new_full(schema, backend, None))
        .collect();
    // GIO's explicit backend is a private implementation detail; an unknown
    // backend cannot establish the same layered GSettings semantics.
    if require_dconf
        && settings.iter().any(|setting| {
            setting
                .backend()
                .is_none_or(|backend| backend.type_().name() != "DConfSettingsBackend")
        })
    {
        return Err(Error::UnsupportedBackend);
    }
    let mut entries = Vec::with_capacity(DesktopKey::ALL.len());
    for key in DesktopKey::ALL {
        let (schema_id, name, _) = key.schema_key_type();
        let index = schemas
            .iter()
            .position(|schema| schema.id() == schema_id)
            .ok_or(Error::UnsupportedSchema)?;
        let schema_key = schemas[index].key(name);
        let setting = &settings[index];
        let effective = setting.value(name);
        let default = setting
            .default_value(name)
            .ok_or(Error::UnsupportedSchema)?;
        let user = setting.user_value(name);
        if !schema_key.range_check(&effective)
            || !schema_key.range_check(&default)
            || user.as_ref().is_some_and(|v| !schema_key.range_check(v))
        {
            return Err(Error::InvalidValue);
        }
        entries.push(DesktopEntry {
            key,
            effective: typed_value(key, &effective)?,
            default: typed_value(key, &default)?,
            user: match user {
                Some(value) => Override::Present(typed_value(key, &value)?),
                None => Override::Absent,
            },
            writable: setting.is_writable(name),
        });
    }
    DesktopSnapshot::capture(entries).map_err(|_| Error::InvalidValue)
}

fn typed_value(key: DesktopKey, variant: &glib::Variant) -> Result<DesktopValue, Error> {
    match key.schema_key_type().2 {
        "s" => variant
            .get::<String>()
            .map(DesktopValue::String)
            .ok_or(Error::InvalidValue),
        "b" => variant
            .get::<bool>()
            .map(DesktopValue::Bool)
            .ok_or(Error::InvalidValue),
        "i" => variant
            .get::<i32>()
            .map(DesktopValue::Int)
            .ok_or(Error::InvalidValue),
        "as" => variant
            .get::<Vec<String>>()
            .map(DesktopValue::Strings)
            .ok_or(Error::InvalidValue),
        _ => Err(Error::UnsupportedSchema),
    }
}

fn read_manager_environment(
    bus: &gio::DBusConnection,
    owner: &str,
) -> Result<omavless_runtime::app_proxy::codec::EnvironmentSnapshot, Error> {
    let reply = dbus_call(
        bus,
        owner,
        MANAGER_PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        &(MANAGER_IFACE, "Environment").to_variant(),
    )?;
    if reply.size() > MAX_MANAGER_REPLY {
        return Err(Error::IncompleteObservation);
    }
    let (value,) = reply
        .get::<(glib::Variant,)>()
        .ok_or(Error::ManagerUnavailable)?;
    let assignments = value
        .get::<Vec<String>>()
        .ok_or(Error::ManagerUnavailable)?;
    project_manager_environment(&assignments)
}

#[cfg(test)]
mod persistent_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires public gsettings-desktop-schemas package"]
    fn installed_schema_supports_typed_memory_override() {
        let source = gio::SettingsSchemaSource::default().unwrap();
        let schemas = validate_schemas(&source).unwrap();
        let memory = gio::memory_settings_backend_new();
        let first = read_desktop_with_backend(&schemas, Some(&memory), false).unwrap();
        let mode = &first.entries()[0];
        assert!(matches!(mode.user, Override::Absent));
        let setting = gio::Settings::new_full(&schemas[0], Some(&memory), None);
        assert!(setting.set_value("mode", &"none".to_variant()).is_ok());
        let second = read_desktop_with_backend(&schemas, Some(&memory), false).unwrap();
        assert!(matches!(
            &second.entries()[0].user,
            Override::Present(DesktopValue::String(value)) if value == "none"
        ));
        assert_eq!(second.entries()[0].effective, second.entries()[0].default);
    }

    fn variant(value: &DesktopValue) -> glib::Variant {
        match value {
            DesktopValue::String(value) => value.to_variant(),
            DesktopValue::Bool(value) => value.to_variant(),
            DesktopValue::Int(value) => value.to_variant(),
            DesktopValue::Strings(value) => value.to_variant(),
        }
    }

    #[test]
    #[ignore = "requires public schemas; writes only an explicitly supplied memory backend"]
    fn installed_schema_all_fields_restore_exact_memory_override_presence() {
        let source = gio::SettingsSchemaSource::default().unwrap();
        let schemas = validate_schemas(&source).unwrap();
        for (index, key) in DesktopKey::ALL.into_iter().enumerate() {
            let (schema_id, name, signature) = key.schema_key_type();
            let schema = schemas
                .iter()
                .find(|schema| schema.id() == schema_id)
                .unwrap();
            for initial in 0..3 {
                // A fresh explicitly supplied backend for every case prevents
                // fallback to the current desktop's default/dconf database.
                let memory = gio::memory_settings_backend_new();
                let setting = gio::Settings::new_full(schema, Some(&memory), None);
                assert_eq!(setting.backend().as_ref(), Some(&memory));
                let default = setting.default_value(name).unwrap();
                let empty_or_default = match signature {
                    "s" if key != DesktopKey::Mode => "".to_variant(),
                    "as" => Vec::<String>::new().to_variant(),
                    _ => default.clone(),
                };
                match initial {
                    0 => assert!(setting.user_value(name).is_none()),
                    1 => assert!(setting.set_value(name, &default).is_ok()),
                    _ => assert!(setting.set_value(name, &empty_or_default).is_ok()),
                }
                let before = read_desktop_with_backend(&schemas, Some(&memory), false).unwrap();
                assert_eq!(
                    matches!(before.entries()[index].user, Override::Absent),
                    initial == 0
                );
                if initial == 1 {
                    assert_eq!(
                        before.entries()[index].effective,
                        before.entries()[index].default
                    );
                }
                let replacement = match key {
                    DesktopKey::Mode => "manual".to_variant(),
                    _ => match signature {
                        "s" => "synthetic literal = 'quoted'\nvalue".to_variant(),
                        "as" => vec!["synthetic.invalid", "localhost"].to_variant(),
                        "b" => (!default.get::<bool>().unwrap()).to_variant(),
                        "i" => 12345_i32.to_variant(),
                        _ => unreachable!(),
                    },
                };
                assert!(setting.set_value(name, &replacement).is_ok());
                let changed = read_desktop_with_backend(&schemas, Some(&memory), false).unwrap();
                for (position, entry) in changed.entries().iter().enumerate() {
                    if position == index {
                        assert_eq!(
                            entry.user,
                            Override::Present(typed_value(key, &replacement).unwrap())
                        );
                        assert_eq!(entry.effective, typed_value(key, &replacement).unwrap());
                        assert_eq!(entry.default, before.entries()[index].default);
                        assert_eq!(entry.writable, before.entries()[index].writable);
                    } else {
                        assert_eq!(entry, &before.entries()[position]);
                    }
                }
                match &before.entries()[index].user {
                    Override::Absent => setting.reset(name),
                    Override::Present(value) => {
                        assert!(setting.set_value(name, &variant(value)).is_ok())
                    }
                }
                let restored = read_desktop_with_backend(&schemas, Some(&memory), false).unwrap();
                assert_eq!(restored.encode().unwrap(), before.encode().unwrap());
                assert_eq!(restored, before);
            }
        }
    }

    #[test]
    #[ignore = "requires an installed Omarchy GSettings backend; reads no setting values"]
    fn installed_default_backend_is_the_supported_dconf_type() {
        let source = gio::SettingsSchemaSource::default().unwrap();
        let schemas = validate_schemas(&source).unwrap();
        let setting = gio::Settings::new_full(&schemas[0], None::<&gio::SettingsBackend>, None);
        assert_eq!(
            setting.backend().unwrap().type_().name(),
            "DConfSettingsBackend"
        );
    }
}
