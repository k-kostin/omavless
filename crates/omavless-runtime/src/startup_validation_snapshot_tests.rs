// SPDX-License-Identifier: MIT
use super::*;
use crate::desired::RoutingMode;
use serde_json::json;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;

const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
const TEMPLATE: &str = "mixed-port: 0\nmode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n";
fn store() -> PrivateStore {
    parse_private_store(
        &json!({"version":3,"activeId":"","lastId":PROFILE,
        "profiles":[{"id":PROFILE,"name":"Synthetic","protocol":"vless",
        "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp",
        "subscriptionId":"","subscriptionKey":"","missing":false,"favorite":false}],
        "subscriptions":[],"customRules":[]})
        .to_string(),
    )
    .unwrap()
}
fn desired(mode: RoutingMode) -> DesiredState {
    DesiredState {
        connected: true,
        profile_id: PROFILE.to_owned(),
        mode,
        ..DesiredState::default()
    }
}
struct Fixture {
    root: PathBuf,
    paths: NativeHostPaths,
    uid: u32,
}
impl Fixture {
    fn new(body: &str) -> Self {
        let root = crate::test_temp::directory("startup-snapshot").unwrap();
        for child in ["data", "config", "runtime"] {
            fs::create_dir(root.join(child)).unwrap();
            fs::set_permissions(root.join(child), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let paths = NativeHostPaths::new(
            root.join("core"),
            root.join("data"),
            root.join("config"),
            root.join("runtime"),
            root.join("proc"),
            root.join("net"),
        );
        fs::write(&paths.core, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&paths.core, fs::Permissions::from_mode(0o700)).unwrap();
        thread::sleep(Duration::from_millis(20));
        Self {
            root,
            paths,
            uid: nix::unistd::Uid::current().as_raw(),
        }
    }
    fn check(&self) -> Result<(), HostStepError> {
        validate_snapshot(
            &self.paths,
            self.uid,
            &desired(RoutingMode::Rule),
            &store(),
            TEMPLATE,
        )
    }
    fn scratch(&self) -> PathBuf {
        self.paths.runtime_directory.join(".startup-check.yaml")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn snapshot_render_matches_canonical_all_modes_and_ignores_files() {
    let f = Fixture::new("exit 0");
    for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
        let desired = desired(mode);
        let store = store();
        let expected = store
            .prepare_config_mode(
                PROFILE,
                TEMPLATE,
                f.paths.controller_socket.to_str().unwrap(),
                mode.as_str(),
            )
            .unwrap();
        fs::write(&f.paths.store, b"malformed replaced store").unwrap();
        fs::write(&f.paths.template, b"malformed replaced template").unwrap();
        assert_eq!(
            render_snapshot(&desired, &store, TEMPLATE, &f.paths.controller_socket).unwrap(),
            expected
        );
        assert_eq!(
            validate_snapshot(&f.paths, f.uid, &desired, &store, TEMPLATE),
            Ok(())
        );
    }
    assert_eq!(
        fs::read(&f.paths.store).unwrap(),
        b"malformed replaced store"
    );
}

#[test]
fn invalid_snapshot_refuses_before_scratch_or_core_execution() {
    let f = Fixture::new("touch \"${0%/*}/called\"; exit 0");
    let store = store();
    let mut invalid = desired(RoutingMode::Rule);
    invalid.generation = u64::MAX;
    for value in [&DesiredState::default(), &invalid] {
        assert_eq!(
            validate_snapshot(&f.paths, f.uid, value, &store, TEMPLATE),
            Err(HostStepError::Prepare)
        );
    }
    let oversized = "x".repeat(omavless_domain::config::MAX_TEMPLATE_BYTES + 1);
    assert_eq!(
        validate_snapshot(
            &f.paths,
            f.uid,
            &desired(RoutingMode::Rule),
            &store,
            &oversized
        ),
        Err(HostStepError::Prepare)
    );
    assert!(!f.scratch().exists());
    assert!(!f.root.join("called").exists());
}

#[test]
fn scratch_is_private_fixed_argv_and_removed_on_success_or_core_rejection() {
    for exit in [0, 1] {
        let f = Fixture::new(&format!(
            "[ \"$#\" -eq 5 ] && [ \"$1\" = '-t' ] && [ \"$2\" = '-d' ] && [ \"$3\" = \"${{0%/*}}/data\" ] && [ \"$4\" = '-f' ] && [ \"$5\" = \"${{0%/*}}/runtime/.startup-check.yaml\" ] || exit 9\n[ \"$(stat -c %a \"$5\")\" = 600 ] || exit 8\nprintf checked > \"${{0%/*}}/checked\"\nexit {exit}"
        ));
        let result = f.check();
        assert_eq!(
            result,
            if exit == 0 {
                Ok(())
            } else {
                Err(HostStepError::Prepare)
            }
        );
        assert_eq!(fs::read(f.root.join("checked")).unwrap(), b"checked");
        assert!(!f.scratch().exists());
        assert!(!f.paths.active_config.exists());
        assert!(!f.paths.store.exists());
    }
}

#[test]
fn scratch_collision_and_symlink_are_preserved_without_core_execution() {
    for symlink_case in [false, true] {
        let f = Fixture::new("touch \"${0%/*}/called\"; exit 0");
        if symlink_case {
            symlink("missing", f.scratch()).unwrap();
        } else {
            fs::write(f.scratch(), b"existing").unwrap();
        }
        assert_eq!(f.check(), Err(HostStepError::Prepare));
        assert!(fs::symlink_metadata(f.scratch()).is_ok());
        assert!(!f.root.join("called").exists());
        if !symlink_case {
            assert_eq!(fs::read(f.scratch()).unwrap(), b"existing");
        }
    }
}

#[test]
fn core_replacement_of_scratch_is_not_unlinked_or_reported_success() {
    let f = Fixture::new(
        "mv \"$5\" \"$5.original\"; printf replacement > \"$5\"; chmod 600 \"$5\"; exit 0",
    );
    assert_eq!(f.check(), Err(HostStepError::Cleanup));
    assert_eq!(fs::read(f.scratch()).unwrap(), b"replacement");
    assert!(
        f.paths
            .runtime_directory
            .join(".startup-check.yaml.original")
            .exists()
    );
}

#[test]
fn installed_core_snapshot_validation_optin() {
    let Some(core) = std::env::var_os("OMAVLESS_TEST_MIHOMO") else {
        return;
    };
    let mut f = Fixture::new("exit 9");
    f.paths.core = PathBuf::from(core);
    assert!(f.paths.core.is_absolute());
    for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
        assert_eq!(
            validate_snapshot(&f.paths, f.uid, &desired(mode), &store(), TEMPLATE),
            Ok(())
        );
        assert!(!f.scratch().exists());
    }
    assert!(!f.paths.active_config.exists());
    assert!(!f.paths.store.exists());
}
