// SPDX-License-Identifier: MIT
use super::*;
use std::os::unix::fs::PermissionsExt;

struct Fixture {
    root: PathBuf,
    uid: u32,
}
impl Fixture {
    fn new() -> Self {
        let root = crate::test_temp::directory("up082").unwrap();
        let f = Self {
            root,
            uid: Uid::current().as_raw(),
        };
        let scope = f.scope();
        for directory in [
            &scope.runtime.directory,
            &scope.cutover.runtime_base,
            &scope.cutover.state_directory,
            &scope.config,
        ] {
            fs::create_dir_all(directory).unwrap();
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        f.put(&scope.runtime.owner_lock, b"");
        f.put(&scope.cutover.operation_lock, b"");
        f.put(
            &scope.cutover.ownership_marker,
            br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
        );
        f.put(
            &scope.cutover.state_directory.join("frontend-bridge.target"),
            b"rust:1\n",
        );
        f.put(&scope.cutover.state_directory.join("desired.json"), br#"{"schemaVersion":1,"generation":7,"connected":false,"profileId":"","mode":"global"}"#);
        f.put(&scope.config.join("profiles.json"), br#"{"version":3,"activeId":"","lastId":"","profiles":[],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":false}"#);
        f.put(
            &scope.config.join("route-template.yaml"),
            include_bytes!("../../../../templates/default.yaml"),
        );
        f
    }
    fn scope(&self) -> Scope {
        let runtime = self.root.join("r");
        Scope {
            runtime: RuntimePaths::below(&runtime),
            cutover: CutoverPaths::below(&runtime, &self.root.join("s"), self.uid),
            config: self.root.join("c"),
        }
    }
    fn put(&self, path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn acquire(&self) -> Result<UpgradeContext> {
        UpgradeContext::acquire(
            self.scope(),
            self.uid,
            ["synthetic-runtime".into(), "synthetic-login".into()],
        )
    }
    fn contends(&self, path: &Path) -> bool {
        let status = Command::new("/usr/bin/flock")
            .args(["--nonblock", "--exclusive"])
            .arg(path)
            .arg("/usr/bin/true")
            .status()
            .unwrap();
        !status.success()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn self_consistent_local_rebuild_of_public_source_does_not_earn_admission() {
    let f = Fixture::new();
    let binary = f.root.join("app");
    let receipt = f.root.join("identity");
    fs::write(&binary, b"inert fixture binary").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let digest = format!("{:x}", Sha256::digest(b"inert fixture binary"));
    let good = format!(
        "schemaVersion=3\nsourceCommit={}\nbinarySha256={digest}\narchitecture={}\nprovenance=caller-supplied-prebuilt\nproductVersion=0.9.8\n",
        PUBLIC_098_SOURCES[0],
        std::env::consts::ARCH
    );
    fs::write(&receipt, &good).unwrap();
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(predecessor098_at(&receipt, &binary, f.uid, std::env::consts::ARCH, None).is_err());
    for wrong in [
        good.replace(PUBLIC_098_SOURCES[0], &"a".repeat(40)),
        good.replace("productVersion=0.9.8", "productVersion=0.9.7"),
        format!("{good}sourceCommit={}\n", PUBLIC_098_SOURCES[0]),
        good.replace("architecture=", "unknownArchitecture="),
    ] {
        fs::write(&receipt, wrong).unwrap();
        assert!(predecessor098_at(&receipt, &binary, f.uid, std::env::consts::ARCH, None).is_err());
    }
    fs::write(&receipt, good).unwrap();
    fs::write(&binary, b"modified fixture").unwrap();
    assert!(predecessor098_at(&receipt, &binary, f.uid, std::env::consts::ARCH, None).is_err());
}

#[test]
fn published_predecessor_pins_are_architecture_and_source_specific() {
    let table: PublishedPairs =
        serde_json::from_str(include_str!("../../../../plugin/published-pairs.json")).unwrap();
    assert_eq!(table.pairs.len(), PUBLIC_098_SOURCES.len() * 2);
    for source in PUBLIC_098_SOURCES {
        for arch in ["x86_64", "aarch64"] {
            let pair = published098(source, arch).unwrap();
            for digest in [
                pair.app_sha,
                pair.core_sha,
                pair.broker_sha,
                pair.dns_receipt_sha,
            ] {
                assert_eq!(digest.len(), 64);
                assert!(
                    digest
                        .bytes()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                );
            }
        }
    }
    assert!(published098(&"a".repeat(40), "x86_64").is_err());
    assert!(published098(PUBLIC_098_SOURCES[0], "unknown").is_err());
}

#[test]
fn fix7_is_retained_as_a_supported_predecessor_for_the_next_delivery() {
    let source = "d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b";
    assert!(PUBLIC_098_SOURCES.contains(&source));
    assert_eq!(
        published098(source, "x86_64").unwrap().app_sha,
        "7c7b0f0c350b8903353d2fed3c8effb693aaaca0c271279b85efa51dfebb8881"
    );
    assert_eq!(
        published098(source, "aarch64").unwrap().app_sha,
        "6949d9473a714e36824e93437c61aff1064c619b9960e8a9b31bb49834176004"
    );
}

#[test]
fn managed_selector_backup_and_their_absence_are_retained_without_conversion() {
    let f = Fixture::new();
    let scope = f.scope();
    let selector = scope.config.join(crate::managed_pair::SELECTOR);
    let backup = scope.config.join("route-template.pre-managed-dns.yaml");
    let managed =
        crate::managed_template::for_host(include_str!("../../../../templates/default.yaml"), true)
            .unwrap();
    f.put(
        &scope.config.join("route-template.yaml"),
        managed.as_bytes(),
    );
    f.put(&selector, crate::managed_pair::SELECTION_BYTES);
    f.put(&backup, b"unchanged fixture backup");
    let mut context = f.acquire().unwrap();
    context.capture_managed_members().unwrap();
    assert!(context.recheck_private().is_ok());
    assert_eq!(fs::read(&backup).unwrap(), b"unchanged fixture backup");
    fs::write(&selector, b"changed").unwrap();
    assert!(matches!(
        context.recheck_private(),
        Err(Blocker::StateChanged)
    ));
}

#[test]
fn absent_managed_member_appearing_after_capture_blocks_package_submission() {
    let f = Fixture::new();
    let mut context = f.acquire().unwrap();
    context.capture_managed_members().unwrap();
    f.put(
        &f.scope().config.join(crate::managed_pair::SELECTOR),
        crate::managed_pair::SELECTION_BYTES,
    );
    assert!(matches!(
        context.recheck_private(),
        Err(Blocker::StateChanged)
    ));
}

#[test]
fn invalid_selector_and_pending_candidate_never_gain_managed_update_admission() {
    for name in [
        crate::managed_pair::SELECTOR,
        ".managed-dns-selection.candidate",
    ] {
        let f = Fixture::new();
        let mut context = f.acquire().unwrap();
        f.put(&f.scope().config.join(name), b"invalid fixture");
        assert!(context.capture_managed_members().is_err());
    }
}

#[test]
fn stopped_post_reboot_owner_acquires_fresh_volatile_exclusions() {
    let f = Fixture::new();
    let scope = f.scope();
    let profiles = fs::read(scope.config.join("profiles.json")).unwrap();
    let desired = fs::read(scope.cutover.state_directory.join("desired.json")).unwrap();
    fs::remove_file(&scope.runtime.owner_lock).unwrap();
    fs::remove_dir(&scope.runtime.directory).unwrap();
    fs::remove_file(&scope.cutover.operation_lock).unwrap();
    let mut context = f.acquire().expect("cold stopped owner must be supported");
    assert!(f.contends(&scope.runtime.owner_lock));
    assert!(f.contends(&scope.cutover.operation_lock));
    context.recheck_private().unwrap();
    assert_eq!(
        fs::read(scope.config.join("profiles.json")).unwrap(),
        profiles
    );
    assert_eq!(
        fs::read(scope.cutover.state_directory.join("desired.json")).unwrap(),
        desired
    );
}

#[test]
fn unrelated_state_siblings_are_not_upgrade_inputs_and_are_preserved() {
    let f = Fixture::new();
    let scope = f.scope();
    let ancillary = scope.cutover.state_directory.join("developer-notes");
    fs::create_dir(&ancillary).unwrap();
    f.put(&ancillary.join("retained.txt"), b"synthetic-private-note");
    let note = scope.cutover.state_directory.join("unrelated-note.txt");
    f.put(&note, b"synthetic-note");
    let profiles = fs::read(scope.config.join("profiles.json")).unwrap();
    let mut context = f
        .acquire()
        .expect("unrelated siblings are not state inputs");
    let later = scope.cutover.state_directory.join("later-notes");
    fs::create_dir(&later).unwrap();
    f.put(&later.join("note.txt"), b"later-synthetic-note");
    context.recheck_private().unwrap();
    assert_eq!(fs::read(&note).unwrap(), b"synthetic-note");
    assert_eq!(
        fs::read(ancillary.join("retained.txt")).unwrap(),
        b"synthetic-private-note"
    );
    assert_eq!(
        fs::read(later.join("note.txt")).unwrap(),
        b"later-synthetic-note"
    );
    assert_eq!(
        fs::read(scope.config.join("profiles.json")).unwrap(),
        profiles
    );
}

#[test]
fn cold_connected_or_startup_on_refuses_before_volatile_creation() {
    for connected in [false, true] {
        let f = Fixture::new();
        let scope = f.scope();
        fs::remove_file(&scope.runtime.owner_lock).unwrap();
        fs::remove_dir(&scope.runtime.directory).unwrap();
        fs::remove_file(&scope.cutover.operation_lock).unwrap();
        let path = if connected {
            scope.cutover.state_directory.join("desired.json")
        } else {
            scope.config.join("profiles.json")
        };
        let before = fs::read_to_string(&path).unwrap();
        let mut after = before.replace(
            if connected {
                "\"connected\":false"
            } else {
                "\"enabled\":false"
            },
            if connected {
                "\"connected\":true"
            } else {
                "\"enabled\":true"
            },
        );
        if connected {
            after = after.replace("\"profileId\":\"\"", "\"profileId\":\"synthetic\"");
        }
        f.put(&path, after.as_bytes());
        assert!(matches!(
            f.acquire(),
            Err(Blocker::DesiredConnected | Blocker::StartupRequiresOff)
        ));
        assert!(!scope.runtime.directory.exists());
        assert!(!scope.cutover.operation_lock.exists());
        assert_eq!(fs::read(&path).unwrap(), after.as_bytes());
    }
}

#[test]
fn existing_unsafe_exclusions_are_never_repaired_or_adopted() {
    for migration in [false, true] {
        for kind in ["mode", "symlink", "hardlink"] {
            let f = Fixture::new();
            let scope = f.scope();
            let path = if migration {
                &scope.cutover.operation_lock
            } else {
                &scope.runtime.owner_lock
            };
            let alias = f.root.join("alias");
            match kind {
                "mode" => fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap(),
                "symlink" => {
                    fs::rename(path, &alias).unwrap();
                    std::os::unix::fs::symlink(&alias, path).unwrap();
                }
                _ => fs::hard_link(path, &alias).unwrap(),
            }
            let before = fs::symlink_metadata(path).unwrap();
            assert!(matches!(f.acquire(), Err(Blocker::ExclusionUnavailable)));
            assert!(same(&before, &fs::symlink_metadata(path).unwrap()));
        }
    }
}

#[test]
fn original_two_flocks_exclude_another_process_until_context_drop() {
    let f = Fixture::new();
    let scope = f.scope();
    let before = fs::read(scope.config.join("profiles.json")).unwrap();
    let mut context = f.acquire().expect("private context admitted");
    assert!(f.contends(&scope.runtime.owner_lock));
    assert!(f.contends(&scope.cutover.operation_lock));
    // An unrelated runtime-base member must not invalidate stable directory identity.
    f.put(&scope.cutover.runtime_base.join("unrelated"), b"synthetic");
    context.recheck_private().unwrap();
    assert_eq!(
        fs::read(scope.config.join("profiles.json")).unwrap(),
        before
    );
    drop(context);
    assert!(!f.contends(&scope.runtime.owner_lock));
    assert!(!f.contends(&scope.cutover.operation_lock));
}

#[test]
fn busy_existing_and_replaced_original_locks_refuse_without_repair() {
    for migration in [false, true] {
        let f = Fixture::new();
        let scope = f.scope();
        let path = if migration {
            &scope.cutover.operation_lock
        } else {
            &scope.runtime.owner_lock
        };
        let held = Flock::lock(File::open(path).unwrap(), FlockArg::LockExclusiveNonblock).unwrap();
        assert!(matches!(f.acquire(), Err(Blocker::Busy)));
        drop(held);
        fs::remove_file(path).unwrap();
        let context = f
            .acquire()
            .expect("partial absent volatile lock is created natively");
        assert!(f.contends(path));
        drop(context);
    }
    for migration in [false, true] {
        let f = Fixture::new();
        let scope = f.scope();
        let mut context = f.acquire().ok().unwrap();
        let path = if migration {
            &scope.cutover.operation_lock
        } else {
            &scope.runtime.owner_lock
        };
        fs::rename(path, f.root.join("retained-original-lock")).unwrap();
        f.put(path, b"");
        assert_eq!(context.recheck_private(), Err(Blocker::StateChanged));
    }
}

#[test]
fn desired_must_exist_be_strict_and_disconnected() {
    for (bytes, expected) in [
        (None, Blocker::DesiredUnavailable),
        (Some(b"{}".as_slice()), Blocker::DesiredUnavailable),
        (Some(br#"{"schemaVersion":1,"generation":7,"connected":true,"profileId":"synthetic","mode":"rule"}"#.as_slice()), Blocker::DesiredConnected),
        (Some(br#"{"schemaVersion":1,"generation":7,"connected":false,"connected":true,"profileId":"","mode":"rule"}"#.as_slice()), Blocker::DesiredUnavailable),
    ] {
        let f = Fixture::new();
        let path = f.scope().cutover.state_directory.join("desired.json");
        if let Some(bytes) = bytes { f.put(&path, bytes); } else { fs::remove_file(&path).unwrap(); }
        assert!(matches!(f.acquire(), Err(error) if error == expected));
    }
}

#[test]
fn startup_on_unconfigured_store_and_stale_bridge_refuse() {
    for (needle, value) in [
        ("\"enabled\":false", "\"enabled\":true"),
        ("\"startupConfigured\":true", "\"startupConfigured\":false"),
    ] {
        let f = Fixture::new();
        let path = f.scope().config.join("profiles.json");
        let raw = fs::read_to_string(&path).unwrap();
        f.put(&path, raw.replace(needle, value).as_bytes());
        assert!(matches!(f.acquire(), Err(Blocker::StartupRequiresOff)));
    }
    let f = Fixture::new();
    f.put(
        &f.scope()
            .cutover
            .state_directory
            .join("frontend-bridge.target"),
        b"rust:0\n",
    );
    assert!(matches!(f.acquire(), Err(Blocker::Ownership)));
}

#[test]
fn duplicate_oversized_and_semantically_repaired_store_refuse() {
    for kind in ["duplicate", "escaped-duplicate", "oversized", "repair"] {
        let f = Fixture::new();
        let path = f.scope().config.join("profiles.json");
        let raw = fs::read_to_string(&path).unwrap();
        let changed = match kind {
            "duplicate" => raw.replacen('{', "{\"version\":3,", 1),
            "escaped-duplicate" => raw.replacen('{', "{\"vers\\u0069on\":3,", 1),
            "oversized" => "x".repeat(MAX_PRIVATE_STORE_BYTES + 1),
            _ => raw.replace(
                "\"activeId\":\"\"",
                "\"activeId\":\"stale-synthetic-record\"",
            ),
        };
        f.put(&path, changed.as_bytes());
        assert!(matches!(f.acquire(), Err(Blocker::PrivateState)));
        assert_eq!(fs::read(&path).unwrap(), changed.as_bytes());
    }
}

#[test]
fn pending_stale_or_new_login_receipt_never_becomes_off_admission() {
    for phase in ["pending", "consumed"] {
        let f = Fixture::new();
        let receipt = f
            .scope()
            .cutover
            .runtime_base
            .join("omavless-login.receipt");
        let bytes = format!(
            "{{\"schemaVersion\":1,\"epochHash\":\"{}\",\"ownershipGeneration\":99,\"phase\":\"{phase}\"}}",
            "a".repeat(64)
        );
        f.put(&receipt, bytes.as_bytes());
        assert!(matches!(f.acquire(), Err(Blocker::PendingRecovery)));
        assert_eq!(fs::read(&receipt).unwrap(), bytes.as_bytes());
    }
    let f = Fixture::new();
    let mut context = f.acquire().ok().unwrap();
    f.put(
        &f.scope()
            .cutover
            .runtime_base
            .join("omavless-login.receipt"),
        b"new pending receipt",
    );
    assert_eq!(context.recheck_private(), Err(Blocker::StateChanged));
}

#[test]
fn pending_custom_partial_and_occupied_template_members_refuse() {
    for name in [
        "routing-preset.pending.json",
        "restore-decision.intent",
        "restore-finalization.pending",
    ] {
        let f = Fixture::new();
        f.put(
            &f.scope().cutover.state_directory.join(name),
            b"synthetic-private-secret",
        );
        assert!(matches!(f.acquire(), Err(Blocker::PendingRecovery)));
    }
    for suffix in ["# custom\n", "  omavless-dns-broker: true\n"] {
        let f = Fixture::new();
        let path = f.scope().config.join("route-template.yaml");
        let raw = fs::read_to_string(&path).unwrap();
        f.put(&path, format!("{raw}{suffix}").as_bytes());
        assert!(matches!(f.acquire(), Err(Blocker::TemplateRequiresReview)));
    }
    for name in [
        "route-template.pre-managed-dns.yaml",
        ".route-template.managed-dns.candidate",
    ] {
        let f = Fixture::new();
        f.put(&f.scope().config.join(name), b"retained");
        assert!(f.acquire().is_err());
    }
}

#[test]
fn changed_private_name_bytes_and_directory_refuse() {
    for replaced in [false, true] {
        let f = Fixture::new();
        let mut context = f.acquire().ok().unwrap();
        let path = f.scope().config.join("profiles.json");
        if replaced {
            fs::rename(&path, f.root.join("retained-original-store")).unwrap();
        }
        f.put(&path, b"synthetic-private-secret");
        assert_eq!(context.recheck_private(), Err(Blocker::StateChanged));
        assert!(!format!("{}", Blocker::StateChanged).contains("synthetic-private-secret"));
    }
    let f = Fixture::new();
    let mut context = f.acquire().ok().unwrap();
    fs::set_permissions(&f.scope().config, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(context.recheck_private(), Err(Blocker::StateChanged));
}

#[test]
fn unsafe_canonical_files_refuse_without_changes() {
    for kind in ["mode", "symlink", "hardlink"] {
        let f = Fixture::new();
        let path = f.scope().config.join("profiles.json");
        match kind {
            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            "symlink" => {
                fs::remove_file(&path).unwrap();
                std::os::unix::fs::symlink("missing", &path).unwrap();
            }
            _ => fs::hard_link(&path, f.root.join("linked")).unwrap(),
        }
        assert!(matches!(f.acquire(), Err(Blocker::PrivateState)));
    }
}

#[test]
fn all_reserved_transaction_names_refuse_before_and_during_update() {
    for name in [
        "routing-preset.pending.json",
        "restore-pair.pending",
        "restore-decision.intent",
        "restore-decision.terminal",
        "restore-finalization.pending",
        crate::restore_closure_model::CLOSURE_MEMBER,
        crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
        crate::restore_disposition_ticket_model::TICKET_MEMBER,
        crate::restore_disposition_complete_model::COMPLETE_MEMBER,
        crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
    ] {
        for kind in ["file", "directory", "dangling-symlink"] {
            for after_acquisition in [false, true] {
                let f = Fixture::new();
                let mut context = after_acquisition.then(|| f.acquire().ok().unwrap());
                let path = f.scope().cutover.state_directory.join(name);
                match kind {
                    "file" => f.put(&path, b"synthetic-pending"),
                    "directory" => fs::create_dir(&path).unwrap(),
                    _ => std::os::unix::fs::symlink("missing", &path).unwrap(),
                }
                let before = fs::symlink_metadata(&path).unwrap();
                if let Some(context) = context.as_mut() {
                    assert_eq!(context.recheck_private(), Err(Blocker::PendingRecovery));
                } else {
                    assert!(matches!(f.acquire(), Err(Blocker::PendingRecovery)));
                }
                assert!(same(&before, &fs::symlink_metadata(&path).unwrap()));
            }
        }
    }
}

#[test]
fn predecessor_synthetic_bytes_require_exact_schema_source_arch_and_digest() {
    let f = Fixture::new();
    let binary = f.root.join("synthetic-public-binary");
    let receipt = f.root.join("synthetic-public-receipt");
    fs::write(&binary, b"synthetic-model-not-public-ELF").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let digest = format!("{:x}", Sha256::digest(fs::read(&binary).unwrap()));
    let bytes = format!(
        "schemaVersion=3\nsourceCommit=22e23e64c49b8110088b6be3f063b5e641ba0853\nbinarySha256={digest}\narchitecture=x86_64\nprovenance=caller-supplied-prebuilt\nproductVersion=0.8.2\n"
    );
    fs::write(&receipt, &bytes).unwrap();
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        predecessor_at(&receipt, &binary, f.uid, "x86_64", &digest),
        Ok(())
    );
    assert_eq!(
        predecessor_at(&receipt, &binary, f.uid, "aarch64", &digest),
        Err(Blocker::Predecessor)
    );
    fs::write(&binary, b"modified").unwrap();
    assert_eq!(
        predecessor_at(&receipt, &binary, f.uid, "x86_64", &digest),
        Err(Blocker::Predecessor)
    );
}

#[test]
fn unit_gate_requires_settled_fixed_unmodified_loaded_units() {
    for name in ["omavless-runtime.service", "omavless-login-prepare.service"] {
        let enabled = if name == "omavless-runtime.service" {
            "disabled"
        } else {
            "static"
        };
        let command = if name == "omavless-runtime.service" {
            "daemon"
        } else {
            "login-prepare"
        };
        let raw = format!(
            "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nJob=\nFragmentPath=/usr/lib/systemd/user/{name}\nDropInPaths=\nUnitFileState={enabled}\nNeedDaemonReload=no\nExecStart={{ path=/usr/bin/omavless ; argv[]=/usr/bin/omavless {command} ; ignore_errors=no ; pid=0 }}\n"
        );
        assert_eq!(validate_unit(&raw, name), Ok(()));
        for changed in [
            raw.replace("MainPID=0", "MainPID=17"),
            raw.replace("Job=\n", "Job=2\n"),
            raw.replace("DropInPaths=\n", "DropInPaths=/private\n"),
            raw.replace("LoadState=loaded", "LoadState=not-found"),
            format!("{raw}Job=\n"),
            raw.replace("NeedDaemonReload=no", "NeedDaemonReload=yes"),
            raw.replace("path=/usr/bin/omavless", "path=/untrusted/helper"),
            raw.replace("ignore_errors=no", "ignore_errors=yes"),
        ] {
            assert_eq!(validate_unit(&changed, name), Err(Blocker::RuntimeActive));
        }
    }
}

#[cfg(any(feature = "developer-upgrade082", feature = "setup-upgrade082"))]
#[test]
fn real_package_consumer_retains_exclusions_through_settlement_and_verification() {
    use std::sync::atomic::{AtomicBool, Ordering};
    for (program, success, cancellation) in [
        ("/usr/bin/true", true, false),
        ("/usr/bin/false", false, false),
        ("/usr/bin/sleep", true, true),
    ] {
        let f = Fixture::new();
        let scope = f.scope();
        let mut context = f.acquire().unwrap();
        let cancel = AtomicBool::new(false);
        let verified = AtomicBool::new(false);
        let result = super::package_phase::consume(
            &mut context,
            &cancel,
            || {
                let mut command = Command::new(program);
                if cancellation {
                    command.arg("0.1");
                }
                let child = command.spawn().unwrap();
                if cancellation {
                    cancel.store(true, Ordering::Release);
                }
                assert!(f.contends(&scope.runtime.owner_lock));
                assert!(f.contends(&scope.cutover.operation_lock));
                Ok(child)
            },
            |context| {
                assert!(f.contends(&scope.runtime.owner_lock));
                assert!(f.contends(&scope.cutover.operation_lock));
                context.recheck_private()?;
                verified.store(true, Ordering::Release);
                Ok(())
            },
        );
        assert!(verified.load(Ordering::Acquire)); // includes nonzero child exit
        assert_eq!(
            result,
            if success {
                Ok(())
            } else {
                Err(Blocker::PackageOutcomeUnverified)
            }
        );
        assert!(f.contends(&scope.runtime.owner_lock));
        drop(context);
        assert!(!f.contends(&scope.runtime.owner_lock));
        assert!(!f.contends(&scope.cutover.operation_lock));
    }
}

#[cfg(any(feature = "developer-upgrade082", feature = "setup-upgrade082"))]
#[test]
fn package_consumer_refuses_before_submit_and_marks_failed_postreadback_unknown() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let f = Fixture::new();
    let mut context = f.acquire().unwrap();
    let submitted = AtomicBool::new(false);
    let cancelled = AtomicBool::new(true);
    let result = super::package_phase::consume(
        &mut context,
        &cancelled,
        || {
            submitted.store(true, Ordering::Release);
            Ok(Command::new("/usr/bin/true").spawn().unwrap())
        },
        |_| Ok(()),
    );
    assert_eq!(result, Err(Blocker::CancelledBeforeSubmission));
    assert!(!submitted.load(Ordering::Acquire));
    cancelled.store(false, Ordering::Release);
    let result = super::package_phase::consume(
        &mut context,
        &cancelled,
        || Ok(Command::new("/usr/bin/true").spawn().unwrap()),
        |_| Err(Blocker::StateChanged),
    );
    assert_eq!(result, Err(Blocker::PackageOutcomeUnverified));
    f.put(
        &f.scope().config.join("profiles.json"),
        b"synthetic changed before submission",
    );
    let result = super::package_phase::consume(
        &mut context,
        &cancelled,
        || {
            submitted.store(true, Ordering::Release);
            Ok(Command::new("/usr/bin/true").spawn().unwrap())
        },
        |_| Ok(()),
    );
    assert_eq!(result, Err(Blocker::StateChanged));
    assert!(!submitted.load(Ordering::Acquire));
}
