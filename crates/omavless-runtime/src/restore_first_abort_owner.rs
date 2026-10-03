// SPDX-License-Identifier: MIT
//! Private recovery-only first-cycle Abort. Never constructs an ordinary owner.
use super::*;
use crate::backup_source_candidate::open_private_directory;
use crate::desired::read_desired_snapshot;
use crate::restore_decision_candidate::DecisionPhase;
use crate::restore_executor_candidate::{
    RetainedPair, abort_staged_pair_retained, verify_aborted_staged_pair_checked,
};
use crate::restore_staging_candidate::{
    MEMBERS, PENDING_DIRECTORY, READY_MEMBER, read_member, read_staged_pair, same_directory,
    same_member,
};
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use std::fs::{File, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use zeroize::Zeroizing;

const REFUSE: ProductionOwnerError = ProductionOwnerError::ManualRecoveryRequired;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    AbortedStillFenced,
}

struct Member {
    directory: usize,
    name: &'static str,
    limit: usize,
    held: Option<(File, Metadata, Zeroizing<Vec<u8>>)>,
}

/// Original to this recovery invocation, not the former process's creation.
struct Recovered {
    directories: Vec<(PathBuf, File, Metadata)>,
    members: Vec<Member>,
}

impl Recovered {
    fn read(
        &self,
        directory: usize,
        name: &'static str,
        limit: usize,
        uid: u32,
    ) -> Result<Member, ProductionOwnerError> {
        let parent = &self.directories[directory].1;
        let held = match openat(
            parent,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => {
                let file = File::from(fd);
                let before = file.metadata().map_err(|_| REFUSE)?;
                if !before.is_file()
                    || before.uid() != uid
                    || before.mode() & 0o7777 != 0o600
                    || before.nlink() != 1
                {
                    return Err(REFUSE);
                }
                let bytes = read_member(parent, name, uid, limit).map_err(|_| REFUSE)?;
                let after = File::from(
                    openat(
                        parent,
                        Path::new(name),
                        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| REFUSE)?,
                );
                if !same_member(&before, &file.metadata().map_err(|_| REFUSE)?)
                    || !same_member(&before, &after.metadata().map_err(|_| REFUSE)?)
                {
                    return Err(REFUSE);
                }
                Some((file, before, bytes))
            }
            Err(nix::errno::Errno::ENOENT) => None,
            Err(_) => return Err(REFUSE),
        };
        Ok(Member {
            directory,
            name,
            limit,
            held,
        })
    }

    fn capture(
        config: &Path,
        paths: &CutoverPaths,
        uid: u32,
        aborted: bool,
    ) -> Result<Self, ProductionOwnerError> {
        let mut this = Self {
            directories: Vec::new(),
            members: Vec::new(),
        };
        for path in [
            config.to_owned(),
            paths.state_directory.clone(),
            paths.runtime_base.clone(),
            paths.state_directory.join(PENDING_DIRECTORY),
        ] {
            let file = open_private_directory(&path, uid).map_err(|_| REFUSE)?;
            let meta = file.metadata().map_err(|_| REFUSE)?;
            this.directories.push((path, file, meta));
        }
        for (directory, name, limit, optional) in [
            (1, "ownership.json", 4096, false),
            (1, "desired.json", 65536, false),
            (2, "omavless-login.receipt", 1024, true),
            (
                1,
                "restore-decision.intent",
                crate::restore_decision_candidate::RECORD_BYTES,
                false,
            ),
            (
                3,
                READY_MEMBER,
                crate::restore_staging_candidate::READY_BYTES,
                false,
            ),
        ] {
            let member = this.read(directory, name, limit, uid)?;
            if !optional && member.held.is_none() {
                return Err(REFUSE);
            }
            this.members.push(member);
        }
        for (index, name) in MEMBERS.into_iter().enumerate() {
            let limit = if index % 2 == 0 {
                omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES
            } else {
                omavless_domain::config::MAX_TEMPLATE_BYTES
            };
            let member = this.read(3, name, limit, uid)?;
            if member.held.is_none() {
                return Err(REFUSE);
            }
            this.members.push(member);
        }
        if aborted {
            // Existing Abort is an immutable read-only pin, never CreatedAbort.
            let member = this.read(
                1,
                "restore-decision.terminal",
                crate::restore_decision_candidate::RECORD_BYTES,
                uid,
            )?;
            if member.held.is_none() {
                return Err(REFUSE);
            }
            this.members.push(member);
        }
        this.check(uid)?;
        Ok(this)
    }

    fn check(&self, uid: u32) -> Result<(), ProductionOwnerError> {
        for (path, file, before) in &self.directories {
            let current = open_private_directory(path, uid).map_err(|_| REFUSE)?;
            if !same_directory(before, &file.metadata().map_err(|_| REFUSE)?)
                || !same_directory(before, &current.metadata().map_err(|_| REFUSE)?)
            {
                return Err(REFUSE);
            }
        }
        for original in &self.members {
            let current = self.read(original.directory, original.name, original.limit, uid)?;
            match (&original.held, &current.held) {
                (None, None) => {}
                (Some((file, before, bytes)), Some((_, after, now)))
                    if bytes == now
                        && same_member(before, after)
                        && same_member(before, &file.metadata().map_err(|_| REFUSE)?) => {}
                _ => return Err(REFUSE),
            }
        }
        // The strict reader also rejects an unexpected entry in the stage.
        read_staged_pair(&self.directories[1].0, uid).map_err(|_| REFUSE)?;
        Ok(())
    }
}

fn no_unrelated(paths: &CutoverPaths) -> bool {
    ["routing-preset.pending.json", "restore-finalization.pending", crate::restore_closure_model::CLOSURE_MEMBER,
        crate::restore_closure_model::NEXT_CLOSURE_MEMBER, crate::restore_disposition_ticket_model::TICKET_MEMBER,
        crate::restore_disposition_complete_model::COMPLETE_MEMBER, crate::restore_successor_handoff_model::SUCCESSOR_MEMBER]
        .into_iter().all(|name| matches!(std::fs::symlink_metadata(paths.state_directory.join(name)), Err(e) if e.kind() == std::io::ErrorKind::NotFound))
}

fn matching_slots(
    config: &Path,
    uid: u32,
    staged: &crate::restore_staging_candidate::VerifiedStage,
) -> bool {
    let Ok(directory) = open_private_directory(config, uid) else {
        return false;
    };
    for (names, expected) in [
        (
            crate::restore_executor_candidate::OLD_SLOT,
            [staged.old_store(), staged.old_template()],
        ),
        (
            crate::restore_executor_candidate::NEW_SLOT,
            [staged.new_store(), staged.new_template()],
        ),
    ] {
        for (name, bytes) in names.into_iter().zip(expected) {
            match crate::restore_cleanup_candidate::read_optional(
                &directory,
                name,
                uid,
                bytes.len(),
            ) {
                Ok(None) => {}
                Ok(Some((raw, _))) if raw.as_slice() == bytes => {}
                _ => return false,
            }
        }
    }
    true
}

#[allow(dead_code)]
fn current(source: &Path, passphrase: &[u8]) -> Result<Outcome, ProductionOwnerError> {
    let uid = Uid::current().as_raw();
    let backup = crate::backup_destination_candidate::open_existing(source, uid, passphrase)
        .map_err(|_| REFUSE)?;
    let runtime = RuntimePaths::current().map_err(|_| REFUSE)?;
    let desired = DesiredPaths::current().map_err(|_| REFUSE)?;
    let paths = CutoverPaths::current(uid).map_err(|_| REFUSE)?;
    let host_paths = NativeHostPaths::current(&runtime.directory).map_err(|_| REFUSE)?;
    let store = host_paths.store.clone();
    let host =
        crate::native_host::ObservationOnlyNativeHost::new(host_paths, uid).map_err(|_| REFUSE)?;
    run(host, desired, &store, paths, uid, &backup)
}

fn run<H: LifecycleHost>(
    mut host: H,
    desired_paths: DesiredPaths,
    store: &Path,
    paths: CutoverPaths,
    uid: u32,
    backup: &omavless_domain::private_backup::OpenedBackup,
) -> Result<Outcome, ProductionOwnerError> {
    if desired_paths.directory != paths.state_directory
        || desired_paths.file != paths.state_directory.join("desired.json")
        || paths.ownership_marker != paths.state_directory.join("ownership.json")
        || paths.operation_lock != paths.runtime_base.join(format!("omavless.{uid}.lock"))
        || store.file_name() != Some("profiles.json".as_ref())
    {
        return Err(REFUSE);
    }
    let lock = MigrationLock::acquire_existing(&paths, uid).map_err(lock_error)?;
    let marker = read_marker_existing(&paths, uid).map_err(|_| REFUSE)?;
    let desired = read_desired_snapshot(&desired_paths, uid).map_err(|_| REFUSE)?;
    if marker.phase() != OwnershipPhase::Rust || desired.connected || !no_unrelated(&paths) {
        return Err(REFUSE);
    }
    let chain = crate::restore_journal_candidate::inspect_decision_journal(&paths, uid, &lock)
        .map_err(|_| REFUSE)?;
    // No gate, source sync or live effect may precede Commit refusal.
    let aborted = match chain.active().phase() {
        DecisionPhase::Intent => false,
        DecisionPhase::Aborted => true,
        DecisionPhase::Committed => return Err(REFUSE),
    };
    let config = store.parent().ok_or(REFUSE)?;
    let recovered = Recovered::capture(config, &paths, uid, aborted)?;
    let retained = std::cell::RefCell::new(RetainedPair::capture(config, uid).map_err(|_| REFUSE)?);
    let staged = read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let incoming = backup.restore_store_off().map_err(|_| REFUSE)?;
    if staged.new_store() != incoming.as_slice() || staged.new_template() != backup.template() {
        return Err(REFUSE);
    }
    let stable = || {
        lock.authorizes(&paths, uid)
            && recovered.check(uid).is_ok()
            && retained
                .try_borrow()
                .is_ok_and(|pair| pair.recheck(config, uid).is_ok())
            && no_unrelated(&paths)
            && matching_slots(config, uid, &staged)
            && read_marker_existing(&paths, uid).ok().as_ref() == Some(&marker)
            && read_desired_snapshot(&desired_paths, uid).ok().as_ref() == Some(&desired)
            && check_login_receipt_without_private_fence(
                &paths,
                uid,
                &lock,
                Some(marker.generation()),
            )
            .is_ok()
    };
    let mut gate = || {
        stable()
            && host.fresh_observation(&desired).is_ok_and(|o| {
                !o.owned_core_running
                    && o.owned_auxiliary_mihomo_count == 0
                    && o.managed_tun_count == 0
                    && !o.owned_controller_config_verified
                    && !o.desired_profile_matches_owned
            })
            && stable()
    };
    let mut preparing = || {
        gate()
            && (aborted
                || matches!(
        std::fs::symlink_metadata(paths.state_directory.join("restore-decision.terminal")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound))
    };
    if !preparing() {
        return Err(REFUSE);
    }
    // Same original descriptors from admission, not later recaptured sources.
    for member in &recovered.members {
        if let Some((file, _, _)) = &member.held {
            if !preparing() {
                return Err(REFUSE);
            }
            file.sync_all().map_err(|_| REFUSE)?;
            if !preparing() {
                return Err(REFUSE);
            }
        }
    }
    for (_, file, _) in recovered.directories.iter().rev() {
        if !preparing() {
            return Err(REFUSE);
        }
        file.sync_all().map_err(|_| REFUSE)?;
        if !preparing() {
            return Err(REFUSE);
        }
    }
    let created = if aborted {
        verify_aborted_staged_pair_checked(
            config,
            &paths,
            uid,
            marker.generation(),
            &lock,
            &mut gate,
        )
        .map_err(|_| REFUSE)?;
        None
    } else {
        let created = abort_staged_pair_retained(
            config,
            &paths,
            uid,
            marker.generation(),
            &lock,
            &mut gate,
            &retained,
        )
        .map_err(|_| REFUSE)?;
        Some(created)
    };
    if let Some(created) = &created {
        created.recheck(&paths, uid).map_err(|_| REFUSE)?;
    }
    if !gate() {
        return Err(REFUSE);
    }
    if let Some(created) = &created {
        created.recheck(&paths, uid).map_err(|_| REFUSE)?;
    }
    Ok(Outcome::AbortedStillFenced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::{DesiredState, OwnedObservation, write_desired};
    use crate::lifecycle::{HostStepError, NativeLocalObservation};
    use crate::restore_executor_candidate::{EffectStep, execute_with_hook};
    use crate::restore_successor_publication_candidate::tests::{Fixture, OLD, backup};
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::rc::Rc;

    struct Host<F>(F);
    impl<F: FnMut() -> bool> LifecycleHost for Host<F> {
        fn fresh_observation(
            &mut self,
            _: &DesiredState,
        ) -> Result<NativeLocalObservation, HostStepError> {
            Ok(NativeLocalObservation {
                owned_core_running: !(self.0)(),
                visible_mihomo_count: 2,
                owned_auxiliary_mihomo_count: 0,
                visible_tun_count: 1,
                managed_tun_count: 0,
                owned_controller_config_verified: false,
                desired_profile_matches_owned: false,
            })
        }
        fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            panic!("no lifecycle effects")
        }
        fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
            panic!("no lifecycle effects")
        }
        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no lifecycle effects")
        }
        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no lifecycle effects")
        }
        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            panic!("no lifecycle effects")
        }
        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no lifecycle effects")
        }
    }

    fn desired(f: &Fixture) -> DesiredPaths {
        DesiredPaths::below(&f.root.join("state"))
    }
    fn ready(stop: EffectStep) -> Fixture {
        let f = Fixture::new();
        fs::remove_file(
            f.paths
                .state_directory
                .join(crate::restore_closure_model::CLOSURE_MEMBER),
        )
        .unwrap();
        write_desired(&desired(&f), f.uid, &DesiredState::default()).unwrap();
        let incoming = backup().restore_store_off().unwrap();
        crate::restore_staging_candidate::stage_private_pair(
            &f.paths.state_directory,
            f.uid,
            OLD[0],
            OLD[1],
            &incoming,
            backup().template(),
        )
        .unwrap();
        let lock = f.lock();
        assert!(
            execute_with_hook(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                [61; 16],
                || true,
                |step| step != stop
            )
            .is_err()
        );
        drop(lock);
        f
    }
    fn invoke(f: &Fixture, host: impl LifecycleHost) -> Result<Outcome, ProductionOwnerError> {
        run(
            host,
            desired(f),
            &f.config.join("profiles.json"),
            f.paths.clone(),
            f.uid,
            backup(),
        )
    }
    fn fenced(f: &Fixture) {
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
        assert!(f.paths.state_directory.join(PENDING_DIRECTORY).is_dir());
        let lock = f.lock();
        assert!(check_startup_receipt(&f.paths, f.uid, &lock, Some(2)).is_err());
    }
    fn replace_same(path: &Path) {
        let bytes = fs::read(path).unwrap();
        let replacement = path.with_extension("swap");
        fs::write(&replacement, bytes).unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(replacement, path).unwrap();
    }

    #[test]
    fn first_abort_owner_restores_old_mixed_new_and_existing_abort_keeps_fences() {
        for stop in [
            EffectStep::Intent,
            EffectStep::Renamed(0),
            EffectStep::Renamed(1),
        ] {
            let f = ready(stop);
            for _ in 0..2 {
                let result = invoke(
                    &f,
                    Host(|| {
                        assert!(MigrationLock::acquire_existing(&f.paths, f.uid).is_err());
                        true
                    }),
                );
                assert_eq!(result, Ok(Outcome::AbortedStillFenced));
                assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), OLD[0]);
                assert_eq!(
                    fs::read(f.config.join("route-template.yaml")).unwrap(),
                    OLD[1]
                );
                fenced(&f);
            }
        }
    }

    #[test]
    fn first_abort_owner_live_identity_survives_every_host_callback_including_final() {
        let reference = ready(EffectStep::Renamed(1));
        let calls = Cell::new(0);
        assert_eq!(
            invoke(
                &reference,
                Host(|| {
                    calls.set(calls.get() + 1);
                    true
                })
            ),
            Ok(Outcome::AbortedStillFenced)
        );
        for name in ["profiles.json", "route-template.yaml"] {
            for fail_at in 1..=calls.get() {
                let f = ready(EffectStep::Renamed(1));
                let seen = Cell::new(0);
                assert!(
                    invoke(
                        &f,
                        Host(|| {
                            seen.set(seen.get() + 1);
                            if seen.get() == fail_at {
                                replace_same(&f.config.join(name));
                            }
                            true
                        })
                    )
                    .is_err(),
                    "{name} callback {fail_at}"
                );
                assert_eq!(seen.get(), fail_at);
                fenced(&f);
            }
        }
        // Existing Abort never advances a live identity either.
        let f = ready(EffectStep::Renamed(1));
        assert!(invoke(&f, Host(|| true)).is_ok());
        assert!(
            invoke(
                &f,
                Host(|| {
                    replace_same(&f.config.join("profiles.json"));
                    true
                })
            )
            .is_err()
        );
    }

    #[test]
    fn first_abort_owner_slots_are_pinned_before_admission_and_owned_link_callbacks() {
        for preexisting in [false, true] {
            for name in crate::restore_executor_candidate::OLD_SLOT {
                let reference = ready(EffectStep::Renamed(1));
                if preexisting {
                    let index = usize::from(name.contains("template"));
                    fs::write(reference.config.join(name), OLD[index]).unwrap();
                    fs::set_permissions(
                        reference.config.join(name),
                        fs::Permissions::from_mode(0o600),
                    )
                    .unwrap();
                }
                let slots_seen = Cell::new(0);
                assert!(
                    invoke(
                        &reference,
                        Host(|| {
                            if reference.config.join(name).exists() {
                                slots_seen.set(slots_seen.get() + 1);
                            }
                            true
                        })
                    )
                    .is_ok()
                );
                assert!(slots_seen.get() > 0);
                for fail_at in 1..=slots_seen.get() {
                    let f = ready(EffectStep::Renamed(1));
                    if preexisting {
                        let index = usize::from(name.contains("template"));
                        fs::write(f.config.join(name), OLD[index]).unwrap();
                        fs::set_permissions(f.config.join(name), fs::Permissions::from_mode(0o600))
                            .unwrap();
                    }
                    let seen = Cell::new(0);
                    assert!(
                        invoke(
                            &f,
                            Host(|| {
                                if f.config.join(name).exists() {
                                    seen.set(seen.get() + 1);
                                    if seen.get() == fail_at {
                                        replace_same(&f.config.join(name));
                                    }
                                }
                                true
                            })
                        )
                        .is_err(),
                        "{name} existing={preexisting} callback {fail_at}"
                    );
                    assert_eq!(seen.get(), fail_at);
                    fenced(&f);
                }
            }
        }
    }

    #[test]
    fn first_abort_owner_commit_refuses_before_host_or_sync() {
        let f = ready(EffectStep::Terminal);
        let before = fs::read(f.config.join("profiles.json")).unwrap();
        assert!(invoke(&f, Host(|| panic!("Commit refused before observation"))).is_err());
        assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), before);
        fenced(&f);
    }

    #[test]
    fn first_abort_owner_original_sources_survive_first_observation_swaps() {
        for member in [
            "old-profiles.json",
            "new-profiles.json",
            "ready.bin",
            "intent",
            "owner",
            "desired",
        ] {
            let f = ready(EffectStep::Renamed(0));
            let target = match member {
                "intent" => f.paths.state_directory.join("restore-decision.intent"),
                "owner" => f.paths.ownership_marker.clone(),
                "desired" => desired(&f).file,
                _ => f.paths.state_directory.join(PENDING_DIRECTORY).join(member),
            };
            let before = fs::read(f.config.join("profiles.json")).unwrap();
            let mut swapped = false;
            assert!(
                invoke(
                    &f,
                    Host(|| {
                        if !swapped {
                            replace_same(&target);
                            swapped = true;
                        }
                        true
                    })
                )
                .is_err()
            );
            assert!(swapped);
            assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), before);
            assert!(
                !f.paths
                    .state_directory
                    .join("restore-decision.terminal")
                    .exists()
            );
            fenced(&f);
        }
    }

    #[test]
    fn first_abort_owner_late_host_loss_and_foreign_fence_stop_actual_rollback() {
        for foreign in [false, true] {
            let f = ready(EffectStep::Renamed(0));
            let changed = Rc::new(Cell::new(false));
            let recorded = changed.clone();
            assert!(
                invoke(
                    &f,
                    Host(|| {
                        if fs::read(f.config.join("profiles.json")).unwrap() == OLD[0] {
                            recorded.set(true);
                            if foreign {
                                fs::write(
                                    f.paths.state_directory.join("restore-disposition.pending"),
                                    b"foreign",
                                )
                                .unwrap();
                            }
                            return foreign;
                        }
                        true
                    })
                )
                .is_err()
            );
            assert!(changed.get());
            assert!(
                !f.paths
                    .state_directory
                    .join("restore-decision.terminal")
                    .exists()
            );
            fenced(&f);
        }
    }

    #[test]
    fn first_abort_owner_every_original_source_sync_boundary_refuses_substitution() {
        for at in 2..=25 {
            let f = ready(EffectStep::Renamed(0));
            // Eight present pinned members and four directories, each with
            // pre/post-sync observations, after initial admission observation.
            let before = fs::read(f.config.join("profiles.json")).unwrap();
            let source = f
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join("old-profiles.json");
            let mut calls = 0;
            assert!(
                invoke(
                    &f,
                    Host(|| {
                        calls += 1;
                        if calls == at {
                            replace_same(&source);
                        }
                        true
                    })
                )
                .is_err()
            );
            assert_eq!(calls, at);
            assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), before);
            assert!(
                !f.paths
                    .state_directory
                    .join("restore-decision.terminal")
                    .exists()
            );
            fenced(&f);
        }
    }

    #[test]
    fn first_abort_owner_archive_lease_slot_login_and_connected_refuse_before_host() {
        for fault in 0..5 {
            let f = ready(EffectStep::Renamed(0));
            let before = fs::read(f.config.join("profiles.json")).unwrap();
            let mut archive = None;
            match fault {
                0 => {
                    let mut store: serde_json::Value =
                        serde_json::from_slice(backup().store()).unwrap();
                    store["onboardingComplete"] = true.into();
                    let pass = b"synthetic wrong archive passphrase";
                    let sealed = omavless_domain::private_backup::seal(
                        &serde_json::to_vec(&store).unwrap(),
                        backup().template(),
                        pass,
                    )
                    .unwrap();
                    archive = Some(omavless_domain::private_backup::open(&sealed, pass).unwrap());
                }
                1 => fs::remove_file(&f.paths.operation_lock).unwrap(),
                2 => {
                    let slot = f
                        .config
                        .join(crate::restore_executor_candidate::OLD_SLOT[0]);
                    fs::write(&slot, b"foreign slot").unwrap();
                    fs::set_permissions(slot, fs::Permissions::from_mode(0o600)).unwrap();
                }
                3 => {
                    let receipt = f.paths.runtime_base.join("omavless-login.receipt");
                    fs::write(&receipt, b"invalid login").unwrap();
                    fs::set_permissions(receipt, fs::Permissions::from_mode(0o600)).unwrap();
                }
                4 => {
                    let connected = DesiredState {
                        connected: true,
                        profile_id: "10000000-0000-4000-8000-000000000001".into(),
                        ..DesiredState::default()
                    };
                    write_desired(&desired(&f), f.uid, &connected).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                run(
                    Host(|| panic!("invalid admission before host")),
                    desired(&f),
                    &f.config.join("profiles.json"),
                    f.paths.clone(),
                    f.uid,
                    archive.as_ref().unwrap_or_else(|| backup())
                )
                .is_err()
            );
            assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), before);
            assert!(
                !f.paths
                    .state_directory
                    .join("restore-decision.terminal")
                    .exists()
            );
            if fault == 1 {
                assert!(!f.paths.operation_lock.exists());
            }
        }
    }

    #[test]
    fn first_abort_owner_existing_abort_inode_is_not_recreated_or_adopted() {
        let f = ready(EffectStep::Renamed(0));
        invoke(&f, Host(|| true)).unwrap();
        let terminal = f.paths.state_directory.join("restore-decision.terminal");
        let mut swapped = false;
        assert!(
            invoke(
                &f,
                Host(|| {
                    if !swapped {
                        replace_same(&terminal);
                        swapped = true;
                    }
                    true
                })
            )
            .is_err()
        );
        assert!(swapped);
        assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), OLD[0]);
        fenced(&f);
    }
}
