// SPDX-License-Identifier: MIT
//! Source-only pre-SDK environment admission. No mutation or SDK initialization.
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

pub const XDG_SCOPES: [(&str, &str); 6] = [
    ("XDG_DATA_HOME", "sdk-data"),
    ("XDG_CONFIG_HOME", "sdk-config"),
    ("XDG_CACHE_HOME", "sdk-cache"),
    ("XDG_STATE_HOME", "sdk-state"),
    ("XDG_DATA_DIRS", "sdk-data"),
    ("XDG_CONFIG_DIRS", "sdk-config"),
];

pub fn admitted(home: &Path, value: impl Fn(&str) -> Option<OsString>) -> bool {
    // Exact singleton paths, not prefix tests or lists with an outside suffix.
    // Missing variables also refuse: never inherit SDK/platform defaults.
    home.is_absolute()
        && home
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
        && XDG_SCOPES
            .into_iter()
            .all(|(key, child)| value(key).as_deref() == Some(home.join(child).as_os_str()))
}

pub fn store_path(home: &Path, identity: &str) -> Option<PathBuf> {
    // Pinned runtime::app_data_dir/data_home layout. Before its create_dir_all,
    // exact empty sdk-data scope and safe bundle identity imply only new
    // descendants, never a preexisting symlink below that captured root.
    if identity.is_empty()
        || identity.len() > 128
        || identity.contains("..")
        || !identity.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        return None;
    }
    Some(home.join("sdk-data/gpui-shell/apps").join(identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn exact_storage_config_scopes_before_sdk() {
        let home = Path::new("/owned/public/g1h-fixture");
        let mut environment: BTreeMap<_, _> = XDG_SCOPES
            .into_iter()
            .map(|(key, child)| (key, home.join(child).into_os_string()))
            .collect();
        assert!(admitted(home, |key| environment.get(key).cloned()));
        for (key, child) in XDG_SCOPES {
            let expected = home.join(child).into_os_string();
            for bad in [
                OsString::new(),
                OsString::from("/outside"),
                OsString::from(format!("{}:/outside", home.join(child).display())),
                home.join(child).join("../outside").into_os_string(),
            ] {
                environment.insert(key, bad);
                assert!(!admitted(home, |key| environment.get(key).cloned()));
            }
            environment.remove(key);
            assert!(!admitted(home, |key| environment.get(key).cloned()));
            environment.insert(key, expected);
        }
        assert!(!admitted(
            Path::new("/owned/../public/g1h-fixture"),
            |key| environment.get(key).cloned()
        ));
        assert!(!admitted(Path::new("relative"), |key| environment
            .get(key)
            .cloned()));
        assert_eq!(
            store_path(home, "app-0123456789abcdef").unwrap(),
            home.join("sdk-data/gpui-shell/apps/app-0123456789abcdef")
        );
        for bad in [
            "",
            "..",
            "app/child",
            "/outside",
            "app:outside",
            "APP",
            "a..b",
        ] {
            assert!(store_path(home, bad).is_none());
        }
        assert!(store_path(home, &"a".repeat(129)).is_none());
    }
}
