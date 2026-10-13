// SPDX-License-Identifier: MIT
//! Actual native IO on wholly synthetic HOME-backed files, not installed guard
//! acceptance. The private local gate is test data only, never production code.
use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const MEMBERS: [&[u8]; 4] = [b"OLD store", b"OLD template", b"NEW store", b"NEW template"];
const DESIRED: &[u8] =
    br#"{"schemaVersion":1,"generation":0,"connected":false,"profileId":"","mode":"rule"}"#;

fn write(path: &Path, bytes: &[u8]) -> File {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    file
}
struct Fixture {
    root: PathBuf,
    state: PathBuf,
    config: PathBuf,
    engine: NativeEngine,
    gate: LocalGate,
    live: [File; 2],
    lock: File,
    closure: Vec<u8>,
    new_output: bool,
    members: [Vec<u8>; 4],
}
struct LocalGate {
    dirs: Vec<(PathBuf, Metadata)>,
    members: Vec<(PathBuf, File, Metadata, Vec<u8>)>,
    calls: usize,
    fail: Option<usize>,
}
impl NativeGate for LocalGate {
    fn check(&mut self, view: NativeStageView<'_>) -> Result<(), FirstError> {
        self.calls += 1;
        if self.fail == Some(self.calls)
            || !view.recovery_exclusive()
            || !(view.own_completion_member(crate::restore_closure_model::CLOSURE_MEMBER)
                || view.engine.unlinked[Slot::Scratch3 as usize])
        {
            return Err(FirstError::Admission);
        }
        for (path, metadata) in &self.dirs {
            let now = fs::symlink_metadata(path).map_err(|_| FirstError::Admission)?;
            if !same_directory(metadata, &now) || metadata.gid() != now.gid() {
                return Err(FirstError::Admission);
            }
        }
        for (path, held, metadata, bytes) in &self.members {
            let named = fs::symlink_metadata(path).map_err(|_| FirstError::Admission)?;
            let now = held.metadata().map_err(|_| FirstError::Admission)?;
            let mut raw = vec![0; bytes.len() + 1];
            let count = held
                .read_at(&mut raw, 0)
                .map_err(|_| FirstError::Admission)?;
            if !same_member(metadata, &now)
                || !same_member(metadata, &named)
                || metadata.gid() != now.gid()
                || count != bytes.len()
                || raw[..count] != *bytes
            {
                return Err(FirstError::Admission);
            }
        }
        Ok(())
    }
}
impl Fixture {
    fn new(case: &str) -> Self {
        Self::from_members(case, false, MEMBERS)
    }
    fn new_committed(case: &str) -> Self {
        Self::from_members(case, true, MEMBERS)
    }
    fn from_members(case: &str, new_output: bool, members: [&[u8]; 4]) -> Self {
        let root = crate::test_temp::directory("final-old").unwrap();
        let config = root.join("c");
        let state = root.join("s");
        let run = root.join("r");
        for directory in [&config, &state, &run] {
            fs::DirBuilder::new().mode(0o700).create(directory).unwrap();
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(run.join("omavless"))
            .unwrap();
        let lock = write(&run.join("omavless").join(crate::OWNER_LOCK_NAME), b"");
        let intent = DecisionRecord::intent(
            2,
            Some(DESIRED),
            &planned_stage_identity(members).unwrap(),
            [31; 16],
        )
        .unwrap();
        let committed = if new_output {
            case != "wrong-phase"
        } else {
            case == "committed"
        };
        let terminal = intent
            .terminal(if committed {
                TerminalChoice::Commit
            } else {
                TerminalChoice::Abort
            })
            .unwrap();
        let receipt = if committed {
            crate::restore_retirement_candidate::RetirementReceipt::from_retained_committed(
                &terminal, members,
            )
            .unwrap()
        } else {
            crate::restore_retirement_candidate::RetirementReceipt::from_retained_terminal(
                &terminal, members,
            )
            .unwrap()
        };
        let closure = crate::restore_closure_model::ClosureRecord::from_verified_receipt(&receipt)
            .unwrap()
            .encode()
            .to_vec();
        drop(write(
            &state.join(crate::restore_closure_model::CLOSURE_MEMBER),
            if case == "torn" {
                &closure[..closure.len() - 1]
            } else {
                &closure
            },
        ));
        for name in match case {
            "history" => vec![native_history::NAMES[0]],
            "later-history" => vec![native_history::NAMES[7]],
            "ticket" => vec![crate::restore_disposition_ticket_model::TICKET_MEMBER],
            "complete" => vec![crate::restore_disposition_complete_model::COMPLETE_MEMBER],
            "receipt" => vec![crate::restore_retirement_candidate::RECEIPT_MEMBER],
            "intent" => vec![INTENT],
            "terminal" => vec![TERMINAL],
            "stage" => vec![PENDING_DIRECTORY],
            "unknown-prefix" => vec!["restore-foreign.pending"],
            "dot-state-prefix" => vec![".restore-foreign.pending"],
            "next" => vec![crate::restore_closure_model::NEXT_CLOSURE_MEMBER],
            "successor" => vec![crate::restore_successor_handoff_model::SUCCESSOR_MEMBER],
            "preset" => vec!["routing-preset.pending.json"],
            _ => vec![],
        } {
            drop(write(&state.join(name), b"foreign synthetic record"));
        }
        if case == "forward" {
            drop(write(&config.join(NATIVE_REPLACEMENTS[0].1), b"foreign"));
        }
        if case == "rollback" {
            drop(write(&config.join(NATIVE_ROLLBACKS[1].1), b"foreign"));
        }
        if case == "config-prefix" {
            drop(write(&config.join("restore-foreign"), b"foreign"));
        }
        if case == "dot-config-prefix" {
            drop(write(&config.join(".restore-foreign"), b"foreign"));
        }
        let offset = if new_output { 2 } else { 0 };
        let live = [
            write(
                &config.join(LIVE[0].1),
                if case == "wrong-old" || case == "wrong-store" {
                    b"other OLD"
                } else {
                    members[offset]
                },
            ),
            write(
                &config.join(LIVE[1].1),
                if case == "wrong-template" {
                    b"other template"
                } else {
                    members[offset + 1]
                },
            ),
        ];
        let mut engine = NativeEngine::reserve();
        engine.recovery = true;
        engine.recovery_completion = if new_output {
            RecoveryCompletionSelection::FinalNew
        } else {
            RecoveryCompletionSelection::FinalOld
        };
        engine.uid = Some(nix::unistd::getuid().as_raw());
        engine.gid = Some(nix::unistd::getgid().as_raw());
        engine.lower.io.native_admit().unwrap();
        let until = Instant::now() + std::time::Duration::from_secs(10);
        let mut gate = LocalGate {
            dirs: Vec::new(),
            members: Vec::new(),
            calls: 0,
            fail: None,
        };
        for (slot, directory) in [
            (Slot::Config, &config),
            (Slot::State, &state),
            (Slot::Run, &run),
        ] {
            let file = File::open(directory).unwrap();
            gate.dirs
                .push((directory.clone(), file.metadata().unwrap()));
            engine.clone_file(slot, &file, true, until).unwrap();
        }
        engine.capture_recovery_singleton(until).unwrap();
        for (index, (slot, _)) in LIVE.into_iter().enumerate() {
            engine.clone_file(slot, &live[index], false, until).unwrap();
        }
        for (slot, parent, name, bytes) in [
            (
                Slot::Owner,
                &state,
                "ownership.json",
                b"synthetic owner".as_slice(),
            ),
            (Slot::Desired, &state, "desired.json", DESIRED),
            (
                Slot::Login,
                &run,
                "omavless-login.receipt",
                b"synthetic login".as_slice(),
            ),
        ] {
            if case == "missing-login" && matches!(slot, Slot::Login) {
                continue;
            }
            let path = parent.join(name);
            let file = write(&path, bytes);
            engine.clone_file(slot, &file, false, until).unwrap();
            gate.members.push((
                path,
                file.try_clone().unwrap(),
                file.metadata().unwrap(),
                bytes.to_vec(),
            ));
        }
        Self {
            root,
            state,
            config,
            engine,
            gate,
            live,
            lock,
            closure,
            new_output,
            members: members.map(<[u8]>::to_vec),
        }
    }
    fn capture(&mut self, case: &str) -> Result<(), FirstError> {
        let new = if case == "wrong-new" {
            [b"other NEW".as_slice(), self.members[3].as_slice()]
        } else if case == "wrong-auth-template" {
            [self.members[2].as_slice(), b"other NEW template".as_slice()]
        } else {
            [self.members[2].as_slice(), self.members[3].as_slice()]
        };
        let desired = if case == "wrong-desired" {
            b"other desired"
        } else {
            DESIRED
        };
        let until = Instant::now() + std::time::Duration::from_secs(10);
        let result = if self.new_output {
            self.engine.capture_final_new_data(new, 2, desired, until)
        } else {
            self.engine.capture_final_old_data(new, 2, desired, until)
        };
        if result.is_err() {
            self.engine.revoke_native();
        }
        result
    }
    fn finish(&mut self, stop: Option<NativeStep>) -> Result<(), FirstError> {
        let until = Instant::now() + std::time::Duration::from_secs(10);
        let mut cut = |step| {
            if stop == Some(step) {
                Err(FirstError::StillFenced)
            } else {
                Ok(())
            }
        };
        let result = (|| {
            if self.new_output {
                self.engine
                    .qualify_final_new_inner(&mut self.gate, &mut cut, until)?;
            } else {
                self.engine
                    .qualify_final_old_inner(&mut self.gate, &mut cut, until)?;
            }
            self.engine
                .check_native_completed_inner(&mut self.gate, until)?;
            self.engine.dispose_completed_inner_cut(
                &mut self.gate,
                nix::unistd::getuid().as_raw(),
                2,
                DESIRED,
                &mut cut,
                until,
            )
        })();
        if result.is_err() {
            self.engine.revoke_native();
        }
        result
    }
    fn retained(&self) {
        assert!(self.engine.lower.io.test_retains_original(Slot::Scratch3));
        assert!(self.engine.lower.io.test_retains_original(Slot::Root));
        assert!(self.engine.lower.io.test_retains_original(Slot::Lock));
        assert!(!self.engine.lower.io.test_retains_original(Slot::Scratch2));
        assert!(!self.engine.unlinked[Slot::Scratch2 as usize]);
        assert!(
            rustix::fs::flock(
                &self.lock,
                rustix::fs::FlockOperation::NonBlockingLockExclusive
            )
            .is_err()
        );
        for (index, (_, name)) in LIVE.into_iter().enumerate() {
            assert_eq!(
                fs::metadata(self.config.join(name)).unwrap().ino(),
                self.live[index].metadata().unwrap().ino()
            );
            assert_eq!(
                fs::read(self.config.join(name)).unwrap(),
                self.members[index + if self.new_output { 2 } else { 0 }]
            );
        }
    }
    fn cleanup(self) {
        let root = self.root.clone();
        drop(self);
        fs::remove_dir_all(root).unwrap(); // only these completed local synthetic controls
    }
}

#[test]
fn native_final_old_capture_rejects_all_unsupported_data_before_qualification() {
    for case in [
        "committed",
        "torn",
        "wrong-new",
        "wrong-old",
        "wrong-desired",
        "history",
        "later-history",
        "ticket",
        "complete",
        "receipt",
        "intent",
        "terminal",
        "stage",
        "unknown-prefix",
        "dot-state-prefix",
        "config-prefix",
        "dot-config-prefix",
        "next",
        "successor",
        "preset",
        "forward",
        "rollback",
    ] {
        let mut f = Fixture::new(case);
        assert!(f.capture(case).is_err(), "public case {case}");
        assert_eq!(f.gate.calls, 0);
        assert!(f.engine.sealed && !f.engine.completed && f.engine.fresh_final_old.is_none());
        assert!(
            !f.state
                .join(crate::restore_disposition_complete_model::COMPLETE_MEMBER)
                .exists()
                || case == "complete"
        );
        f.cleanup();
    }
}

#[test]
fn native_final_old_capture_is_not_authority_and_positive_disposition_preserves_pair() {
    let mut f = Fixture::new("positive");
    assert!(
        f.engine
            .qualify_final_old_inner(&mut f.gate, &mut |_| Ok(()), Instant::now())
            .is_err()
    );
    f.capture("positive").unwrap();
    assert!(f.engine.final_old_observation_captured());
    assert!(!f.engine.completed_origin() && !f.engine.completed && !f.engine.final_old_qualified());
    assert!(
        f.engine
            .check_native_completed_inner(&mut f.gate, Instant::now())
            .is_err()
    );
    f.finish(None).unwrap();
    assert!(f.engine.final_old_qualified() && f.engine.disposition_ready());
    f.retained();
    assert!(!crate::pending_private_transaction::pending_at(&f.state));
    let (name, bytes) = f
        .engine
        .completed_history_data_inner(
            &mut f.gate,
            Instant::now() + std::time::Duration::from_secs(10),
        )
        .unwrap();
    assert_eq!(name, native_history::NAMES[0]);
    assert_eq!(fs::read(f.state.join(name)).unwrap(), bytes);
    let complete =
        crate::restore_disposition_complete_model::CompleteRecord::decode(&bytes).unwrap();
    let closure = crate::restore_closure_model::ClosureRecord::decode(&f.closure).unwrap();
    let ticket = crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
        &closure,
        nix::unistd::getuid().as_raw(),
        2,
        Some(DESIRED),
    )
    .unwrap();
    assert!(complete.matches_ticket(&ticket));
    assert!(closure.receipt().matches_pair(MEMBERS[0], MEMBERS[1]));
    f.cleanup();
}

#[test]
fn native_final_old_every_sync_publication_unlink_history_cut_retains_original_graph() {
    let mut steps = (0..10)
        .map(NativeStep::FinalOldSourceSynced)
        .collect::<Vec<_>>();
    steps.extend([
        NativeStep::EndpointSynced,
        NativeStep::EndpointRetired,
        NativeStep::DispositionTicket,
        NativeStep::DispositionComplete,
        NativeStep::DispositionClosureUnlinked,
        NativeStep::DispositionTicketUnlinked,
        NativeStep::HistoryRenamePrepared,
        NativeStep::HistoryRenamed,
        NativeStep::HistorySynced,
        NativeStep::History,
    ]);
    for stop in steps {
        let mut f = Fixture::new("positive");
        f.capture("positive").unwrap();
        assert!(f.finish(Some(stop)).is_err(), "public cut {stop:?}");
        assert!(f.engine.sealed && !f.engine.disposition_ready());
        f.retained();
        assert!(f.finish(None).is_err());
        f.cleanup();
    }
}

#[test]
fn native_final_old_rechecks_boundary_closure_pair_and_catalogue_substitution() {
    for case in [
        "closure",
        "live",
        "owner",
        "desired",
        "login",
        "state",
        "foreign",
        "receipt-fd",
    ] {
        let mut f = Fixture::new("positive");
        f.capture("positive").unwrap();
        let path = match case {
            "closure" => Some(f.state.join(crate::restore_closure_model::CLOSURE_MEMBER)),
            "live" => Some(f.config.join(LIVE[0].1)),
            "owner" => Some(f.gate.members[0].0.clone()),
            "desired" => Some(f.gate.members[1].0.clone()),
            "login" => Some(f.gate.members[2].0.clone()),
            _ => None,
        };
        if let Some(path) = path {
            let bytes = fs::read(&path).unwrap();
            fs::rename(&path, path.with_file_name("displaced")).unwrap();
            drop(write(&path, &bytes));
        } else if case == "state" {
            fs::rename(&f.state, f.root.join("displaced-state")).unwrap();
            fs::DirBuilder::new().mode(0o700).create(&f.state).unwrap();
        } else if case == "foreign" {
            drop(write(&f.state.join("foreign"), b"unknown"));
        } else {
            let file = write(&f.state.join("foreign-receipt"), b"foreign");
            f.engine
                .clone_file(
                    Slot::Scratch2,
                    &file,
                    false,
                    Instant::now() + std::time::Duration::from_secs(10),
                )
                .unwrap();
        }
        assert!(f.finish(None).is_err(), "public substitution {case}");
        assert!(f.engine.sealed && f.engine.fresh_final_old.is_none());
        assert!(f.engine.lower.io.test_retains_original(Slot::Scratch3));
        f.cleanup();
    }
}

#[test]
fn native_final_old_every_origin_gate_failure_retains_and_never_retries() {
    let mut positive = Fixture::new("positive");
    positive.capture("positive").unwrap();
    positive.finish(None).unwrap();
    let calls = positive.gate.calls;
    positive.cleanup();
    for fail in 1..=calls {
        let mut f = Fixture::new("positive");
        f.capture("positive").unwrap();
        f.gate.fail = Some(fail);
        assert!(f.finish(None).is_err(), "public origin gate {fail}");
        assert!(f.engine.sealed && !f.engine.disposition_ready());
        f.retained();
        assert!(f.finish(None).is_err());
        f.cleanup();
    }
}

#[test]
fn native_final_new_actual_authenticated_output_is_required_and_no_old_grant_exists() {
    let root = crate::test_temp::directory("final-new-auth").unwrap();
    let source = root.join("new.ovb");
    let password = b"synthetic new output passphrase";
    let portable = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"china-cn-direct","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
    drop(write(
        &source,
        &omavless_domain::private_backup::seal(
            portable,
            include_bytes!("../../../../templates/china.yaml"),
            password,
        )
        .unwrap(),
    ));
    let uid = nix::unistd::getuid().as_raw();
    assert!(
        crate::backup_destination_candidate::open_existing(
            &source,
            uid,
            b"wrong synthetic password"
        )
        .is_err()
    );
    let opened =
        crate::backup_destination_candidate::open_existing(&source, uid, password).unwrap();
    let off = opened.restore_store_off().unwrap();
    let members = [MEMBERS[0], MEMBERS[1], off.as_slice(), opened.template()];
    let mut f = Fixture::from_members("positive", true, members);
    f.capture("positive").unwrap();
    assert!(f.engine.final_new_observation_captured());
    assert!(!f.engine.completed_origin() && !f.engine.completed);
    assert!(f.engine.fresh_final_old.is_none() && f.engine.fresh_final_new.is_none());
    f.finish(None).unwrap();
    assert!(f.engine.final_new_qualified() && !f.engine.final_old_qualified());
    assert!(f.engine.fresh_final_old.is_none() && f.engine.disposition_ready());
    f.retained();
    let (name, bytes) = f
        .engine
        .completed_history_data_inner(
            &mut f.gate,
            Instant::now() + std::time::Duration::from_secs(10),
        )
        .unwrap();
    let closure = crate::restore_closure_model::ClosureRecord::decode(&f.closure).unwrap();
    assert_eq!(
        closure.receipt().terminal().phase(),
        DecisionPhase::Committed
    );
    let ticket = crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
        &closure,
        uid,
        2,
        Some(DESIRED),
    )
    .unwrap();
    assert!(
        crate::restore_disposition_complete_model::CompleteRecord::decode(&bytes)
            .unwrap()
            .matches_ticket(&ticket)
    );
    assert_eq!(fs::read(f.state.join(name)).unwrap(), bytes);
    assert!(!crate::pending_private_transaction::pending_at(&f.state));
    f.cleanup();
    drop(opened);
    fs::remove_dir_all(root).unwrap(); // completed private local auth fixture only
}

#[test]
fn native_final_new_wrong_phase_pair_archive_login_history_prefixes_refuse_before_qualification() {
    for case in [
        "wrong-phase",
        "torn",
        "wrong-new",
        "wrong-auth-template",
        "wrong-store",
        "wrong-template",
        "wrong-desired",
        "missing-login",
        "history",
        "later-history",
        "ticket",
        "complete",
        "receipt",
        "intent",
        "terminal",
        "stage",
        "unknown-prefix",
        "dot-state-prefix",
        "config-prefix",
        "dot-config-prefix",
        "next",
        "successor",
        "preset",
        "forward",
        "rollback",
    ] {
        let mut f = Fixture::new_committed(case);
        assert!(f.capture(case).is_err(), "public NEW refusal {case}");
        assert_eq!(f.gate.calls, 0);
        assert!(f.engine.sealed && !f.engine.completed);
        assert!(f.engine.fresh_final_new.is_none() && f.engine.fresh_final_old.is_none());
        f.cleanup();
    }
}

#[test]
fn native_final_new_every_sync_disposition_cut_retains_exact_current_new_graph() {
    let mut steps = (0..10)
        .map(NativeStep::FinalNewSourceSynced)
        .collect::<Vec<_>>();
    steps.extend([
        NativeStep::EndpointSynced,
        NativeStep::EndpointRetired,
        NativeStep::DispositionTicket,
        NativeStep::DispositionComplete,
        NativeStep::DispositionClosureUnlinked,
        NativeStep::DispositionTicketUnlinked,
        NativeStep::HistoryRenamePrepared,
        NativeStep::HistoryRenamed,
        NativeStep::HistorySynced,
        NativeStep::History,
    ]);
    for stop in steps {
        let mut f = Fixture::new_committed("positive");
        f.capture("positive").unwrap();
        assert!(f.finish(Some(stop)).is_err(), "public NEW cut {stop:?}");
        assert!(
            f.engine.sealed && !f.engine.disposition_ready() && f.engine.fresh_final_old.is_none()
        );
        f.retained();
        assert!(f.finish(None).is_err());
        f.cleanup();
    }
}

#[test]
fn native_final_new_every_original_gate_failure_retains_and_rejects_reuse() {
    let mut good = Fixture::new_committed("positive");
    good.capture("positive").unwrap();
    good.finish(None).unwrap();
    let calls = good.gate.calls;
    good.cleanup();
    for fail in 1..=calls {
        let mut f = Fixture::new_committed("positive");
        f.capture("positive").unwrap();
        f.gate.fail = Some(fail);
        assert!(f.finish(None).is_err(), "public NEW original gate {fail}");
        assert!(f.engine.sealed && !f.engine.disposition_ready());
        f.retained();
        assert!(f.finish(None).is_err());
        f.cleanup();
    }
}

#[test]
fn native_final_new_rechecks_current_inodes_boundaries_closure_catalogue_and_absent_receipt() {
    for case in [
        "closure",
        "live",
        "owner",
        "desired",
        "login",
        "foreign",
        "receipt-fd",
    ] {
        let mut f = Fixture::new_committed("positive");
        f.capture("positive").unwrap();
        let path = match case {
            "closure" => Some(f.state.join(crate::restore_closure_model::CLOSURE_MEMBER)),
            "live" => Some(f.config.join(LIVE[0].1)),
            "owner" => Some(f.gate.members[0].0.clone()),
            "desired" => Some(f.gate.members[1].0.clone()),
            "login" => Some(f.gate.members[2].0.clone()),
            _ => None,
        };
        if let Some(path) = path {
            let bytes = fs::read(&path).unwrap();
            fs::rename(&path, path.with_file_name("displaced")).unwrap();
            drop(write(&path, &bytes));
        } else if case == "foreign" {
            drop(write(&f.config.join(".restore-foreign"), b"foreign"));
        } else {
            let file = write(&f.state.join("foreign-receipt"), b"foreign");
            f.engine
                .clone_file(
                    Slot::Scratch2,
                    &file,
                    false,
                    Instant::now() + std::time::Duration::from_secs(10),
                )
                .unwrap();
        }
        assert!(f.finish(None).is_err(), "public NEW substitution {case}");
        assert!(
            f.engine.sealed
                && f.engine.fresh_final_new.is_none()
                && f.engine.fresh_final_old.is_none()
        );
        assert!(f.engine.lower.io.test_retains_original(Slot::Scratch3));
        f.cleanup();
    }
}
