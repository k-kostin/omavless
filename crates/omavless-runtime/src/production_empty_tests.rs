// SPDX-License-Identifier: MIT
use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;

struct Fixture {
    root: PathBuf,
    paths: ProductionObservationPaths,
    uid: u32,
}
impl Fixture {
    fn new(body: &str) -> Self {
        let root = crate::test_temp::directory("empty-host").unwrap();
        let uid = Uid::current().as_raw();
        let paths = ProductionObservationPaths::below(
            root.join("systemctl"),
            &root.join("home"),
            &root.join("runtime"),
            root.join("proc"),
            root.join("net"),
            uid,
        );
        for path in [&paths.runtime_base, &paths.proc_root, &paths.sys_class_net] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        // Fixed synthetic output executable checks argv, never invokes any
        // service mutation and writes only inside its private fixture.
        let script = format!(
            "#!/bin/sh\ncase \"$1/$2/$3\" in --user/show/omavless.service|--user/show/omavless-runtime.service) ;; *) exit 9;; esac\n{body}\n"
        );
        fs::write(&paths.systemctl, script).unwrap();
        fs::set_permissions(&paths.systemctl, fs::Permissions::from_mode(0o700)).unwrap();
        thread::sleep(Duration::from_millis(20));
        Self { root, paths, uid }
    }
    fn empty() -> Self {
        Self::new(
            "printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n'",
        )
    }
    fn observer(&self) -> ProductionOwnershipObserver {
        ProductionOwnershipObserver::new(self.paths.clone(), self.uid).unwrap()
    }
    fn runtime_child(&self) {
        fs::create_dir(self.paths.runtime_base.join("omavless")).unwrap();
        fs::set_permissions(
            self.paths.runtime_base.join("omavless"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn disconnected_native_fixture(native: &str, legacy: &str) -> Fixture {
    let f = Fixture::new(&format!(
        "if [ \"$3\" = omavless-runtime.service ]; then printf '{native}'; else printf '{legacy}'; fi"
    ));
    f.runtime_child();
    drop(UnixListener::bind(&f.paths.rust_control_socket).unwrap());
    fs::set_permissions(
        &f.paths.rust_control_socket,
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    f
}

const ACTIVE_NATIVE: &str =
    "ActiveState=active\\nMainPID=42\\nExecMainStatus=0\\nResult=success\\n";
const INACTIVE_LEGACY: &str =
    "ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n";

#[test]
fn disconnected_native_requires_complete_final_inventories() {
    for kind in [
        "valid",
        "missingcomm",
        "core",
        "tun",
        "missingproc",
        "missingnet",
    ] {
        let f = disconnected_native_fixture(ACTIVE_NATIVE, INACTIVE_LEGACY);
        let observer = f.observer();
        match kind {
            "missingproc" => fs::remove_dir(&f.paths.proc_root).unwrap(),
            "missingnet" => fs::remove_dir(&f.paths.sys_class_net).unwrap(),
            "tun" => {
                fs::create_dir(f.paths.sys_class_net.join("tun")).unwrap();
                fs::write(f.paths.sys_class_net.join("tun/tun_flags"), b"0x1001\n").unwrap();
            }
            "core" | "missingcomm" => {
                fs::create_dir(f.paths.proc_root.join("42")).unwrap();
                if kind == "core" {
                    fs::write(f.paths.proc_root.join("42/comm"), b"mihomo\n").unwrap();
                }
            }
            _ => {}
        }
        let result = observer.verify_disconnected_native();
        if kind == "valid" {
            let result = result.unwrap();
            assert!(result.rust_owner_active);
            assert_eq!((result.core_count, result.tun_count), (0, 0));
        } else {
            assert!(result.is_err());
        }
    }
}

#[test]
fn disconnected_native_refuses_zero_pid_transitional_and_legacy_owners() {
    for native in [
        ACTIVE_NATIVE.replace("42", "0"),
        ACTIVE_NATIVE.replace("=active", "=inactive"),
        ACTIVE_NATIVE.replace("=active", "=activating"),
        ACTIVE_NATIVE.replace("=active", "=deactivating"),
        ACTIVE_NATIVE.replace("success", "exit-code"),
    ] {
        let f = disconnected_native_fixture(&native, INACTIVE_LEGACY);
        assert!(f.observer().verify_disconnected_native().is_err());
    }
    for legacy in [
        INACTIVE_LEGACY.replace("MainPID=0", "MainPID=2"),
        INACTIVE_LEGACY.replace("=inactive", "=active"),
    ] {
        let f = disconnected_native_fixture(ACTIVE_NATIVE, &legacy);
        assert!(f.observer().verify_disconnected_native().is_err());
    }
}

#[test]
fn verified_empty_does_not_require_or_create_store_or_config() {
    let f = Fixture::empty();
    assert_eq!(f.observer().verify_empty(), Ok(()));
    assert!(!f.paths.store.exists());
    assert!(!f.paths.active_config.exists());
    assert!(!f.paths.rust_control_socket.exists());
}

#[test]
fn active_or_nonzero_pid_and_duplicate_service_state_refuse() {
    for output in [
        "ActiveState=active\\nMainPID=0\\n",
        "ActiveState=activating\\nMainPID=0\\n",
        "ActiveState=deactivating\\nMainPID=0\\n",
        "ActiveState=reloading\\nMainPID=0\\n",
        "ActiveState=inactive\\nMainPID=2\\n",
        "ActiveState=active\\nActiveState=inactive\\nMainPID=0\\n",
    ] {
        let f = Fixture::new(&format!(
            "printf '{output}ExecMainStatus=0\\nResult=success\\n'"
        ));
        assert!(f.observer().verify_empty().is_err());
    }
}

#[test]
fn failed_but_stopped_service_is_empty_not_healthy() {
    let f = Fixture::new(
        "printf 'ActiveState=failed\\nMainPID=0\\nExecMainStatus=1\\nResult=exit-code\\n'",
    );
    assert_eq!(f.observer().verify_empty(), Ok(()));
}

#[test]
fn stdout_eof_does_not_replace_successful_child_exit() {
    let f = Fixture::new("exec 1>&-\nexec sleep 1");
    let started = Instant::now();
    assert_eq!(
        service_state_with_timeout(
            &f.paths.systemctl,
            LEGACY_SERVICE,
            Duration::from_millis(80)
        ),
        Err(ProductionObservationError::ServiceQuery)
    );
    assert!(started.elapsed() < Duration::from_millis(800));
}

#[test]
fn every_controller_presence_including_dead_socket_is_a_blocker() {
    for index in 0..3 {
        for kind in ["file", "symlink", "socket"] {
            let f = Fixture::empty();
            f.runtime_child();
            let path = [
                &f.paths.legacy_controller,
                &f.paths.rust_controller,
                &f.paths.rust_control_socket,
            ][index];
            match kind {
                "file" => fs::write(path, b"private").unwrap(),
                "symlink" => symlink("missing", path).unwrap(),
                _ => drop(UnixListener::bind(path).unwrap()),
            }
            assert_eq!(
                f.observer().verify_empty(),
                Err(ProductionObservationError::HostNotEmpty)
            );
            assert!(fs::symlink_metadata(path).is_ok());
        }
    }
}

#[test]
fn dangling_or_unsafe_parent_is_not_absence() {
    for kind in ["symlink", "file", "mode"] {
        let f = Fixture::empty();
        let child = f.paths.runtime_base.join("omavless");
        match kind {
            "symlink" => symlink("missing", &child).unwrap(),
            "file" => fs::write(&child, b"").unwrap(),
            _ => {
                f.runtime_child();
                fs::set_permissions(&child, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        assert_eq!(
            f.observer().verify_empty(),
            Err(ProductionObservationError::UnsafePath)
        );
    }
}

#[test]
fn incomplete_or_nonempty_inventory_refuses() {
    for kind in ["missingcomm", "core", "tun", "missingproc", "missingnet"] {
        let f = Fixture::empty();
        let observer = f.observer();
        match kind {
            "missingproc" => fs::remove_dir(&f.paths.proc_root).unwrap(),
            "missingnet" => fs::remove_dir(&f.paths.sys_class_net).unwrap(),
            "tun" => {
                fs::create_dir(f.paths.sys_class_net.join("tun")).unwrap();
                fs::write(f.paths.sys_class_net.join("tun/tun_flags"), b"0x1001\n").unwrap();
            }
            _ => {
                fs::create_dir(f.paths.proc_root.join("42")).unwrap();
                if kind == "core" {
                    fs::write(f.paths.proc_root.join("42/comm"), b"mihomo\n").unwrap();
                }
            }
        }
        assert!(observer.verify_empty().is_err());
    }
}

#[test]
fn symlinked_runtime_ancestor_is_not_trusted_empty_host() {
    let f = Fixture::empty();
    let alias = f.root.join("alias");
    symlink(&f.root, &alias).unwrap();
    let paths = ProductionObservationPaths::below(
        f.paths.systemctl.clone(),
        &f.root.join("home"),
        &alias.join("runtime"),
        f.paths.proc_root.clone(),
        f.paths.sys_class_net.clone(),
        f.uid,
    );
    let observer = ProductionOwnershipObserver::new(paths, f.uid).unwrap();
    assert_eq!(
        observer.verify_empty(),
        Err(ProductionObservationError::UnsafePath)
    );
}

#[test]
fn second_observation_rejects_transition() {
    let f = Fixture::new(
        "here=${0%/*}; n=0; [ ! -f \"$here/count\" ] || read -r n < \"$here/count\"; n=$((n+1)); printf '%s' \"$n\" > \"$here/count\"; if [ \"$n\" -ge 3 ]; then s=active; else s=inactive; fi; printf 'ActiveState=%s\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' \"$s\"",
    );
    assert_eq!(
        f.observer().verify_empty(),
        Err(ProductionObservationError::HostNotEmpty)
    );
}

#[test]
fn retained_descendant_stdout_cannot_extend_query_deadline() {
    let f = Fixture::new(
        "sleep 1 &\nprintf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n'",
    );
    let started = Instant::now();
    assert_eq!(
        service_state_with_timeout(
            &f.paths.systemctl,
            LEGACY_SERVICE,
            Duration::from_millis(80)
        ),
        Err(ProductionObservationError::ServiceQuery)
    );
    assert!(started.elapsed() < Duration::from_millis(800));
    // The deliberately unowned helper self-terminates; never send broad kills.
    thread::sleep(Duration::from_millis(1100));
}
