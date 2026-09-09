// SPDX-License-Identifier: MIT
//! Compare only fixed path roots; never expose or log manager environment data.

use nix::unistd::{Uid, User};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

const KEYS: [&str; 6] = [
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "XDG_CACHE_HOME",
    "XDG_RUNTIME_DIR",
    "OMAVLESS_HOME",
];
const MAX_PATH_BYTES: usize = 4096;
type Values = [Option<OsString>; 6];

// No Debug/Serialize implementation: paths are private local account data.
#[derive(PartialEq, Eq)]
struct Roots([PathBuf; 5]);

fn normal_path(value: &std::ffi::OsStr) -> Result<PathBuf, ()> {
    let path = Path::new(value);
    if value.len() > MAX_PATH_BYTES
        || !path.is_absolute()
        || value
            .as_encoded_bytes()
            .iter()
            .any(|byte| byte.is_ascii_control())
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(());
    }
    Ok(path.components().collect())
}

fn roots(values: &Values, default_home: &Path, uid: u32) -> Result<Roots, ()> {
    if values[5].is_some() {
        return Err(());
    }
    let home = normal_path(values[0].as_deref().unwrap_or(default_home.as_os_str()))?;
    let mut paths = [
        home.clone(),
        home.join(".config"),
        home.join(".local/state"),
        home.join(".cache"),
        PathBuf::from(format!("/run/user/{uid}")),
    ];
    for index in 1..5 {
        if let Some(value) = &values[index] {
            // The runtime treats an explicitly empty XDG variable as a path,
            // not an unset default. Refuse it rather than claiming agreement.
            paths[index] = normal_path(value)?;
        }
    }
    Ok(Roots(paths))
}

fn manager_values(text: &str) -> Result<Values, ()> {
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(());
    }
    let mut values: Values = Default::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            // A malformed relevant assignment cannot disappear into defaults.
            if KEYS.iter().any(|key| line.contains(key)) {
                return Err(());
            }
            continue;
        };
        let bare_key = key.trim_start_matches(['\'', '"', '$']);
        let Some(index) = KEYS.iter().position(|expected| *expected == bare_key) else {
            if key
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .any(|part| KEYS.contains(&part))
            {
                return Err(());
            }
            // Ignore other values, including shell-quoted secrets, without
            // interpreting their contents or retaining them in the projection.
            continue;
        };
        if key != bare_key
            || values[index].is_some()
            || value.len() > MAX_PATH_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/_-.".contains(&byte))
        {
            return Err(());
        }
        values[index] = Some(value.into());
    }
    Ok(values)
}

fn check(account_home: &Path, uid: u32, current: &Values, manager: &str) -> Result<(), ()> {
    // CLI path constructors require HOME. A user-manager service gets the
    // account HOME when its manager environment does not explicitly supply one.
    if current[0].is_none() {
        return Err(());
    }
    let manager = manager_values(manager)?;
    (roots(current, account_home, uid)? == roots(&manager, account_home, uid)?)
        .then_some(())
        .ok_or(())
}

fn after_agreement<T>(
    account_home: &Path,
    uid: u32,
    current: &Values,
    manager: &str,
    activate: impl FnOnce() -> T,
) -> Result<T, ()> {
    check(account_home, uid, current, manager)?;
    Ok(activate())
}

pub(super) fn with_current<T>(activate: impl FnOnce() -> T) -> Result<T, ()> {
    let uid = Uid::current();
    let account = User::from_uid(uid).map_err(|_| ())?.ok_or(())?;
    let current: Values = KEYS.map(std::env::var_os);
    let manager = crate::production_observation::cutover_manager_environment().map_err(|_| ())?;
    after_agreement(&account.dir, uid.as_raw(), &current, &manager, activate)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn current() -> Values {
        [Some("/home/test".into()), None, None, None, None, None]
    }
    #[test]
    fn effective_defaults_and_explicit_matching_roots_agree() {
        for manager in [
            "",
            "HOME=/home/test\n",
            "HOME=/home/test\nXDG_CONFIG_HOME=/home/test/.config\nXDG_STATE_HOME=/home/test/.local/state\nXDG_CACHE_HOME=/home/test/.cache\nXDG_RUNTIME_DIR=/run/user/1000\n",
            "UNRELATED='private token'\n",
        ] {
            assert!(check(Path::new("/home/test"), 1000, &current(), manager).is_ok());
        }
        let mut cli = current();
        cli[2] = Some("/private/state".into());
        assert!(
            check(
                Path::new("/home/test"),
                1000,
                &cli,
                "XDG_STATE_HOME=/private/state\n"
            )
            .is_ok()
        );
    }
    #[test]
    fn differing_roots_overrides_and_ambiguous_encoding_refuse() {
        for manager in [
            "HOME=/another/home",
            "XDG_CONFIG_HOME=/another/config",
            "XDG_STATE_HOME=/another/state",
            "XDG_CACHE_HOME=/another/cache",
            "XDG_RUNTIME_DIR=/another/runtime",
            "OMAVLESS_HOME=/home/test",
            "HOME=/home/test\nHOME=/home/test",
            "XDG_STATE_HOME=",
            "XDG_STATE_HOME=relative",
            "XDG_STATE_HOME='/home/test/.local/state'",
            "'XDG_STATE_HOME=/home/test/.local/state'",
            "XDG_STATE_HOME=/home/test/../state",
            "XDG_STATE_HOME",
            "XDG_STATE_HOME=/home/test\\x20state",
        ] {
            assert!(
                check(
                    Path::new("/home/test"),
                    1000,
                    &current(),
                    &format!("{manager}\n")
                )
                .is_err()
            );
        }
        for index in 0..6 {
            let mut cli = current();
            cli[index] = Some("/another/root".into());
            assert!(check(Path::new("/home/test"), 1000, &cli, "").is_err());
        }
    }

    #[test]
    fn mismatched_or_malformed_roots_never_enter_transaction_effects() {
        let calls = std::cell::Cell::new(0);
        for manager in [
            "XDG_STATE_HOME=/another/state",
            "XDG_RUNTIME_DIR=/another/runtime",
            "export XDG_STATE_HOME=/another/state",
            "\"XDG_STATE_HOME\"=/another/state",
            "HOME=/home/test\nHOME=/home/test",
        ] {
            assert!(
                after_agreement(
                    Path::new("/home/test"),
                    1000,
                    &current(),
                    &format!("{manager}\n"),
                    || calls.set(calls.get() + 1)
                )
                .is_err()
            );
        }
        assert_eq!(calls.get(), 0);
        after_agreement(Path::new("/home/test"), 1000, &current(), "", || {
            calls.set(calls.get() + 1)
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
    }
}
