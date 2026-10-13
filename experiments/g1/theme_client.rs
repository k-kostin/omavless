// SPDX-License-Identifier: MIT
//! Fixed owned public client fixture preflight; no SDK initialization or writes.
use crate::{theme_environment, theme_fixture};
use std::os::unix::fs::MetadataExt;
use std::{fs, path::PathBuf, sync::Arc};
pub struct PublicClient {
    #[allow(dead_code)] // Embedding host uses app; Direct shares the same guard.
    pub app: PathBuf,
    pub sampler: Arc<theme_fixture::Sampler>,
    _scopes: Vec<theme_fixture::OwnedFixture>,
}
impl PublicClient {
    pub fn select() -> Option<Self> {
        Self::from_environment(|key| std::env::var_os(key))
    }
    fn from_environment(value: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<Self> {
        // Research opt-in only; never turn normal user HOME into a data source.
        if value("G1_PUBLIC_THEME_SOURCE").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return None;
        }
        let home = PathBuf::from(value("HOME")?);
        let uid = fs::metadata("/proc/self").ok()?.uid();
        if uid == 0
            || !home.is_absolute()
            || !home.file_name()?.to_str()?.starts_with("g1h-")
            || home.parent()?.file_name()? != "ovt"
            || home.parent()?.parent()?.file_name()? != ".cache"
            || fs::canonicalize(&home).ok()? != home
            || !theme_environment::admitted(&home, &value)
        {
            return None;
        }
        let mut scopes = Vec::new();
        for directory in [
            &home,
            &home.join(".cache"),
            &home.join(".cache/ovt"),
            &home.join("app"),
        ] {
            scopes.push(theme_fixture::OwnedFixture::open_root(directory).ok()?);
        }
        for child in ["sdk-data", "sdk-config", "sdk-cache", "sdk-state"] {
            scopes.push(theme_fixture::OwnedFixture::open_empty_root(&home.join(child)).ok()?);
        }
        if fs::symlink_metadata(home.join(".config")).is_ok()
            || fs::symlink_metadata(home.join(".local")).is_ok()
        {
            return None;
        }
        if !scopes[3].has_theme_manifest() {
            return None;
        }
        let fixture =
            theme_fixture::OwnedFixture::open_root(&home.join(".cache/ovt/g1-theme")).ok()?;
        Some(Self {
            app: home.join("app"),
            sampler: theme_fixture::Sampler::new(fixture),
            _scopes: scopes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        os::unix::fs::{DirBuilderExt, symlink},
    };
    #[test]
    fn actual_public_preflight_refuses_escapes_before_initialization() {
        let parent = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cache/ovt");
        drop(theme_fixture::OwnedFixture::open_root(&parent).unwrap());
        let home = (0..128)
            .find_map(|index| {
                let candidate = parent.join(format!("g1h-source-{}-{index}", std::process::id()));
                match fs::DirBuilder::new().mode(0o700).create(&candidate) {
                    Ok(()) => Some(candidate),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(_) => panic!("owned public allocation failed"),
                }
            })
            .expect("owned allocation bound");
        for child in [
            ".cache",
            ".cache/ovt",
            ".cache/ovt/g1-theme",
            "app",
            "sdk-data",
            "sdk-config",
            "sdk-cache",
            "sdk-state",
        ] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(home.join(child))
                .unwrap();
        }
        fs::write(
            home.join("app/gpui-shell.json"),
            include_bytes!("shell/theme-manifest.json"),
        )
        .unwrap();
        let mut environment = BTreeMap::from([
            ("HOME", home.clone().into_os_string()),
            ("G1_PUBLIC_THEME_SOURCE", "1".into()),
        ]);
        for (key, child) in theme_environment::XDG_SCOPES {
            environment.insert(key, home.join(child).into_os_string());
        }
        let source = PublicClient::from_environment(|key| environment.get(key).cloned()).unwrap();
        assert_eq!(source.app, home.join("app"));
        assert_eq!(
            source.sampler.reserve().unwrap().sample().unwrap().1.status,
            crate::theme_source::Status::Missing
        );
        drop(source);
        environment.insert("XDG_DATA_HOME", "/outside".into());
        assert!(PublicClient::from_environment(|key| environment.get(key).cloned()).is_none());
        environment.insert("XDG_DATA_HOME", home.join("sdk-data").into_os_string());
        fs::write(home.join("sdk-data/public-marker"), b"public").unwrap();
        assert!(PublicClient::from_environment(|key| environment.get(key).cloned()).is_none());
        fs::remove_file(home.join("sdk-data/public-marker")).unwrap();
        fs::remove_dir(home.join("sdk-data")).unwrap();
        symlink("sdk-config", home.join("sdk-data")).unwrap();
        assert!(PublicClient::from_environment(|key| environment.get(key).cloned()).is_none());
        fs::remove_file(home.join("sdk-data")).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(home.join("sdk-data"))
            .unwrap();
        fs::write(
            home.join("app/gpui-shell.json"),
            include_bytes!("shell/gpui-shell.json"),
        )
        .unwrap();
        assert!(PublicClient::from_environment(|key| environment.get(key).cloned()).is_none());
        // Known public fixture only; no SDK/selected process or user state.
        fs::remove_dir_all(home).unwrap();
    }
}
