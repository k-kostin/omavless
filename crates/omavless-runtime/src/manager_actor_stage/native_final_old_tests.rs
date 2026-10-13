// SPDX-License-Identifier: MIT
//! Actual native IO on wholly synthetic HOME-backed files, not installed guard
//! acceptance. The private local gate is test data only, never production code.
use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink};
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
    new_ticket: bool,
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
    fn with_ticket(mut self) -> Self {
        self.new_ticket = true;
        self.engine.recovery_completion = RecoveryCompletionSelection::NewTicket;
        let closure = crate::restore_closure_model::ClosureRecord::decode(&self.closure).unwrap();
        let ticket = crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
            &closure,
            nix::unistd::getuid().as_raw(),
            2,
            Some(DESIRED),
        )
        .unwrap();
        drop(write(
            &self
                .state
                .join(crate::restore_disposition_ticket_model::TICKET_MEMBER),
            &ticket.encode(),
        ));
        self
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
            new_ticket: false,
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
        let result = if self.new_ticket {
            self.engine.capture_final_output_data(
                new,
                2,
                desired,
                FinalOutputChoice::NewTicketCommitted,
                until,
            )
        } else if self.new_output {
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
        let mut cut = |step| {
            if stop == Some(step) {
                Err(FirstError::StillFenced)
            } else {
                Ok(())
            }
        };
        self.finish_with(&mut cut)
    }
    fn finish_with(
        &mut self,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        let until = Instant::now() + std::time::Duration::from_secs(10);
        let result = (|| {
            if self.new_ticket {
                self.engine.qualify_final_output_inner(
                    &mut self.gate,
                    FinalOutputChoice::NewTicketCommitted,
                    cut,
                    until,
                )?;
            } else if self.new_output {
                self.engine
                    .qualify_final_new_inner(&mut self.gate, cut, until)?;
            } else {
                self.engine
                    .qualify_final_old_inner(&mut self.gate, cut, until)?;
            }
            self.engine
                .check_native_completed_inner(&mut self.gate, until)?;
            self.engine.dispose_completed_inner_cut(
                &mut self.gate,
                nix::unistd::getuid().as_raw(),
                2,
                DESIRED,
                cut,
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

// Previous complete records here are independently constructed synthetic DATA.
// Their generation, Desired, terminal choice and pair differ from current ones;
// no historical record supplies a qualification, lock, guard or durability fact.
fn past_audit(index: usize) -> Vec<u8> {
    let members = [
        b"past OLD".as_slice(),
        b"past template",
        b"past NEW",
        b"other template",
    ];
    let terminal = DecisionRecord::intent(
        1,
        None,
        &planned_stage_identity(members).unwrap(),
        [index as u8 + 1; 16],
    )
    .unwrap()
    .terminal(if index.is_multiple_of(2) {
        TerminalChoice::Abort
    } else {
        TerminalChoice::Commit
    })
    .unwrap();
    assert!(!terminal.matches_owner_desired(2, Some(DESIRED)));
    let receipt = if index.is_multiple_of(2) {
        crate::restore_retirement_candidate::RetirementReceipt::from_retained_terminal(
            &terminal, members,
        )
    } else {
        crate::restore_retirement_candidate::RetirementReceipt::from_retained_committed(
            &terminal, members,
        )
    }
    .unwrap();
    let closure =
        crate::restore_closure_model::ClosureRecord::from_verified_receipt(&receipt).unwrap();
    let ticket = crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
        &closure,
        nix::unistd::getuid().as_raw(),
        1,
        None,
    )
    .unwrap();
    crate::restore_disposition_complete_model::CompleteRecord::from_ticket(&ticket)
        .unwrap()
        .encode()
        .to_vec()
}
struct PastAudit {
    name: &'static str,
    file: File,
    metadata: Metadata,
    bytes: Vec<u8>,
}
fn prior_audits(state: &Path, count: usize) -> Vec<PastAudit> {
    native_history::NAMES[..count]
        .iter()
        .enumerate()
        .map(|(index, &name)| {
            let bytes = past_audit(index);
            let file = write(&state.join(name), &bytes);
            PastAudit {
                name,
                metadata: file.metadata().unwrap(),
                file,
                bytes,
            }
        })
        .collect()
}
fn assert_past_unchanged(state: &Path, past: &[PastAudit]) {
    for audit in past {
        let held = audit.file.metadata().unwrap();
        let named = fs::symlink_metadata(state.join(audit.name)).unwrap();
        assert!(same_member(&audit.metadata, &held) && same_member(&audit.metadata, &named));
        assert_eq!(audit.metadata.gid(), held.gid());
        assert_eq!(audit.metadata.gid(), named.gid());
        assert_eq!(fs::read(state.join(audit.name)).unwrap(), audit.bytes);
        let mut bytes = vec![0; audit.bytes.len() + 1];
        let count = audit.file.read_at(&mut bytes, 0).unwrap();
        assert_eq!(&bytes[..count], audit.bytes);
    }
}
fn final_fixture(new: bool) -> Fixture {
    if new {
        Fixture::new_committed("positive")
    } else {
        Fixture::new("positive")
    }
}

#[test]
fn native_final_history_old_and_new_append_after_all_seven_prior_prefixes() {
    for new in [false, true] {
        for count in 0..native_history::CAPACITY {
            let mut f = final_fixture(new);
            let past = prior_audits(&f.state, count);
            f.capture("positive").unwrap();
            assert_eq!(f.gate.calls, 0); // bounded audit/free target BEFORE qualification
            assert_eq!(
                f.engine.audit.target().unwrap(),
                native_history::NAMES[count]
            );
            assert!(
                !f.engine.completed
                    && !f.engine.final_old_qualified()
                    && !f.engine.final_new_qualified()
            );
            f.finish(None).unwrap();
            f.retained();
            assert!(f.engine.disposition_ready());
            assert_eq!(f.engine.final_new_qualified(), new);
            assert_eq!(f.engine.final_old_qualified(), !new);
            let (name, bytes) = f
                .engine
                .completed_history_data_inner(
                    &mut f.gate,
                    Instant::now() + std::time::Duration::from_secs(10),
                )
                .unwrap();
            assert_eq!(name, native_history::NAMES[count]);
            assert_eq!(fs::read(f.state.join(name)).unwrap(), bytes);
            let closure = crate::restore_closure_model::ClosureRecord::decode(&f.closure).unwrap();
            let ticket = crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
                &closure,
                nix::unistd::getuid().as_raw(),
                2,
                Some(DESIRED),
            )
            .unwrap();
            assert!(
                crate::restore_disposition_complete_model::CompleteRecord::decode(&bytes)
                    .unwrap()
                    .matches_ticket(&ticket)
            );
            assert_past_unchanged(&f.state, &past);
            assert!(!crate::pending_private_transaction::pending_at(&f.state));
            // This only releases the positively completed SOURCE fixture graph.
            // Actual FreshRecovery consuming lease/start remains its unchanged
            // production gate and separately selected installed acceptance.
            let Fixture {
                root,
                state,
                engine,
                gate,
                live,
                lock,
                ..
            } = f;
            drop((engine, gate, live));
            rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
            assert_past_unchanged(&state, &past);
            drop((past, lock));
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn native_final_history_exhausted_sparse_torn_unsafe_and_unknown_refuse_before_quiescence() {
    for new in [false, true] {
        for case in [
            "full",
            "sparse",
            "torn",
            "corrupt",
            "mode",
            "symlink",
            "directory",
            "hardlink",
            "xattr",
            "unknown",
            "suffix",
            "dot-prefix",
        ] {
            let mut f = final_fixture(new);
            let count = if case == "full" {
                native_history::CAPACITY
            } else {
                1
            };
            let past = if matches!(case, "symlink" | "directory") {
                vec![]
            } else {
                prior_audits(&f.state, count)
            };
            let path = f.state.join(native_history::NAMES[0]);
            match case {
                "full" => {}
                "sparse" => {
                    fs::rename(&path, f.state.join(native_history::NAMES[2])).unwrap();
                }
                "torn" => {
                    OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .set_len(1)
                        .unwrap();
                }
                "corrupt" => {
                    OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .write_all(b"x")
                        .unwrap();
                }
                "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
                "symlink" => symlink("missing", &path).unwrap(),
                "directory" => fs::DirBuilder::new().mode(0o700).create(&path).unwrap(),
                "hardlink" => fs::hard_link(&path, f.state.join("prior-alias")).unwrap(),
                "xattr" => rustix::fs::lsetxattr(
                    &path,
                    "user.t4_history_test",
                    b"public",
                    rustix::fs::XattrFlags::empty(),
                )
                .unwrap(),
                "unknown" => {
                    drop(write(&f.state.join("restore-unrelated"), b"public"));
                }
                "suffix" => {
                    drop(write(
                        &f.state.join("restore-disposition.history.8"),
                        &past_audit(2),
                    ));
                }
                "dot-prefix" => {
                    drop(write(
                        &f.state.join(".restore-disposition.history"),
                        &past_audit(2),
                    ));
                }
                _ => unreachable!(),
            }
            let before: Vec<_> = past
                .iter()
                .map(|p| {
                    (
                        p.file.metadata().unwrap(),
                        fs::read(f.state.join(if case == "sparse" {
                            native_history::NAMES[2]
                        } else {
                            p.name
                        }))
                        .unwrap(),
                    )
                })
                .collect();
            assert!(
                f.capture("positive").is_err(),
                "public history refusal {new}/{case}"
            );
            assert_eq!(f.gate.calls, 0);
            assert!(
                f.engine.sealed
                    && !f.engine.completed
                    && !f.engine.final_old_qualified()
                    && !f.engine.final_new_qualified()
            );
            assert!(f.engine.lower.io.test_retains_original(Slot::Scratch3));
            assert!(
                f.state
                    .join(crate::restore_closure_model::CLOSURE_MEMBER)
                    .exists()
            );
            for (p, (metadata, bytes)) in past.iter().zip(before) {
                assert!(same_member(&metadata, &p.file.metadata().unwrap()));
                assert_eq!(
                    fs::read(f.state.join(if case == "sparse" {
                        native_history::NAMES[2]
                    } else {
                        p.name
                    }))
                    .unwrap(),
                    bytes
                );
            }
            drop(past);
            f.cleanup();
        }
    }
}

#[test]
fn native_final_history_drift_and_target_collision_at_gates_seal_the_original() {
    for new in [false, true] {
        for step in [
            if new {
                NativeStep::FinalNewSourceSynced(0)
            } else {
                NativeStep::FinalOldSourceSynced(0)
            },
            NativeStep::DispositionTicket,
            NativeStep::DispositionComplete,
            NativeStep::HistoryRenamePrepared,
            NativeStep::HistorySynced,
        ] {
            for case in ["inode", "bytes", "mode", "xattr", "collision"] {
                let mut f = final_fixture(new);
                let past = prior_audits(&f.state, 2);
                f.capture("positive").unwrap();
                let state = f.state.clone();
                let path = state.join(native_history::NAMES[0]);
                let target = state.join(native_history::NAMES[2]);
                let mut reached = false;
                let result = f.finish_with(&mut |event| {
                    if event == step {
                        reached = true;
                        match case {
                            "inode" => {
                                fs::rename(&path, state.join("displaced-audit")).unwrap();
                                drop(write(&path, &past_audit(0)));
                            }
                            "bytes" => {
                                OpenOptions::new()
                                    .write(true)
                                    .open(&path)
                                    .unwrap()
                                    .write_all(b"x")
                                    .unwrap();
                            }
                            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
                                .unwrap(),
                            "xattr" => rustix::fs::lsetxattr(
                                &path,
                                "user.t4_history_test",
                                b"public",
                                rustix::fs::XattrFlags::empty(),
                            )
                            .unwrap(),
                            "collision" => {
                                // After rename this is a same-byte replacement of
                                // the published target, never recaptured as past.
                                if target.exists() {
                                    fs::rename(&target, state.join("displaced-target")).unwrap();
                                }
                                drop(write(&target, &past_audit(3)));
                            }
                            _ => unreachable!(),
                        }
                    }
                    Ok(())
                });
                assert!(
                    reached && result.is_err(),
                    "public drift {new}/{step:?}/{case}"
                );
                assert!(f.engine.sealed && !f.engine.disposition_ready());
                f.retained();
                assert!(f.finish(None).is_err()); // mutation cannot rearm or recapture
                assert_eq!(
                    fs::read(state.join(native_history::NAMES[1])).unwrap(),
                    past[1].bytes
                );
                assert!(same_member(
                    &past[1].metadata,
                    &past[1].file.metadata().unwrap()
                ));
                drop(past);
                f.cleanup();
            }
        }
    }
}

#[test]
fn native_final_history_all_sync_disposition_cuts_preserve_prior_audits() {
    for new in [false, true] {
        let mut steps: Vec<_> = (0..10)
            .map(|i| {
                if new {
                    NativeStep::FinalNewSourceSynced(i)
                } else {
                    NativeStep::FinalOldSourceSynced(i)
                }
            })
            .collect();
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
        for step in steps {
            let mut f = final_fixture(new);
            let past = prior_audits(&f.state, 7);
            f.capture("positive").unwrap();
            assert!(
                f.finish(Some(step)).is_err(),
                "public history cut {new}/{step:?}"
            );
            assert!(f.engine.sealed && !f.engine.disposition_ready());
            f.retained();
            assert_past_unchanged(&f.state, &past);
            assert!(f.finish(None).is_err());
            assert_past_unchanged(&f.state, &past);
            drop(past);
            f.cleanup();
        }
    }
}

#[test]
fn native_new_ticket_existing_observation_is_not_a_grant_and_all_prior_prefixes_append() {
    for count in 0..native_history::CAPACITY {
        let mut f = Fixture::new_committed("positive").with_ticket();
        let past = prior_audits(&f.state, count);
        let ticket_path = f
            .state
            .join(crate::restore_disposition_ticket_model::TICKET_MEMBER);
        let ticket = File::open(&ticket_path).unwrap();
        let old_ticket = ticket.metadata().unwrap();
        let bytes = fs::read(&ticket_path).unwrap();
        f.capture("positive").unwrap();
        assert_eq!(f.gate.calls, 0);
        assert!(f.engine.new_ticket_observation_captured() && !f.engine.new_ticket_qualified());
        assert!(!f.engine.completed && !f.engine.disposition && !f.engine.completed_origin());
        assert!(
            f.engine
                .check_native_completed_inner(&mut f.gate, Instant::now())
                .is_err()
        );
        assert_eq!(
            f.engine
                .lower
                .io
                .original(Slot::Scratch0)
                .unwrap()
                .metadata()
                .unwrap()
                .ino(),
            old_ticket.ino()
        );
        let mut reached = Vec::new();
        f.finish_with(&mut |event| {
            reached.push(event);
            Ok(())
        })
        .unwrap();
        assert!(f.engine.new_ticket_qualified() && f.engine.disposition_ready());
        assert!(!f.engine.final_old_qualified() && !f.engine.final_new_qualified());
        assert!(!reached.contains(&NativeStep::DispositionTicket)); // NEVER republishes/relabels it
        assert!(reached.contains(&NativeStep::NewTicketSourceSynced(10)));
        assert_eq!(ticket.metadata().unwrap().nlink(), 0);
        assert_eq!(ticket.metadata().unwrap().ino(), old_ticket.ino());
        let mut held_bytes = vec![0; bytes.len()];
        ticket.read_exact_at(&mut held_bytes, 0).unwrap();
        assert_eq!(held_bytes, bytes);
        let (name, result) = f
            .engine
            .completed_history_data_inner(
                &mut f.gate,
                Instant::now() + std::time::Duration::from_secs(10),
            )
            .unwrap();
        assert_eq!(name, native_history::NAMES[count]);
        let original_ticket =
            crate::restore_disposition_ticket_model::Ticket::decode(&bytes).unwrap();
        assert!(
            crate::restore_disposition_complete_model::CompleteRecord::decode(&result)
                .unwrap()
                .matches_ticket(&original_ticket)
        );
        f.retained();
        assert_past_unchanged(&f.state, &past);
        assert!(!crate::pending_private_transaction::pending_at(&f.state));
        assert!(f.finish(None).is_err());
        assert_past_unchanged(&f.state, &past);
        drop((past, ticket));
        f.cleanup();
    }
}

#[test]
fn native_new_ticket_wrong_current_or_competing_shapes_refuse_before_qualification() {
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
        let mut f = Fixture::new_committed(case).with_ticket();
        assert!(f.capture(case).is_err(), "public ticket shape {case}");
        assert_eq!(f.gate.calls, 0);
        assert!(
            f.engine.sealed
                && !f.engine.completed
                && !f.engine.disposition
                && !f.engine.new_ticket_qualified()
        );
        f.cleanup();
    }
}

#[test]
fn native_new_ticket_canonical_crossed_ticket_and_unsafe_members_never_supply_authority() {
    for case in [
        "missing",
        "torn",
        "corrupt",
        "uid",
        "generation-desired",
        "crossed-closure",
        "mode",
        "xattr",
        "hardlink",
        "symlink",
        "full-history",
        "sparse-history",
    ] {
        let mut f = Fixture::new_committed("positive").with_ticket();
        let path = f
            .state
            .join(crate::restore_disposition_ticket_model::TICKET_MEMBER);
        match case {
            "missing" => fs::remove_file(&path).unwrap(),
            "torn" => OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(1)
                .unwrap(),
            "corrupt" => OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .write_all(b"x")
                .unwrap(),
            "uid" | "generation-desired" | "crossed-closure" => {
                let raw = if case == "generation-desired" {
                    let raw = past_audit(1);
                    raw[8..8 + crate::restore_disposition_ticket_model::TICKET_BYTES].to_vec()
                } else {
                    let closure = if case == "crossed-closure" {
                        let other = Fixture::from_members(
                            "positive",
                            true,
                            [b"other OLD", b"other template", MEMBERS[2], MEMBERS[3]],
                        );
                        let closure =
                            crate::restore_closure_model::ClosureRecord::decode(&other.closure)
                                .unwrap();
                        other.cleanup();
                        closure
                    } else {
                        crate::restore_closure_model::ClosureRecord::decode(&f.closure).unwrap()
                    };
                    crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
                        &closure,
                        nix::unistd::getuid().as_raw() + u32::from(case == "uid"),
                        2,
                        Some(DESIRED),
                    )
                    .unwrap()
                    .encode()
                    .to_vec()
                };
                assert!(crate::restore_disposition_ticket_model::Ticket::decode(&raw).is_some());
                fs::remove_file(&path).unwrap();
                drop(write(&path, &raw));
            }
            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            "xattr" => rustix::fs::lsetxattr(
                &path,
                "user.t4_ticket_test",
                b"public",
                rustix::fs::XattrFlags::empty(),
            )
            .unwrap(),
            "hardlink" => fs::hard_link(&path, f.state.join("ticket-alias")).unwrap(),
            "symlink" => {
                fs::remove_file(&path).unwrap();
                symlink("missing", &path).unwrap();
            }
            "full-history" => {
                drop(prior_audits(&f.state, 8));
            }
            "sparse-history" => {
                drop(write(
                    &f.state.join(native_history::NAMES[1]),
                    &past_audit(1),
                ));
            }
            _ => unreachable!(),
        }
        assert!(
            f.capture("positive").is_err(),
            "public ticket refusal {case}"
        );
        assert_eq!(f.gate.calls, 0);
        assert!(f.engine.sealed && !f.engine.completed && !f.engine.disposition);
        assert!(f.engine.fresh_new_ticket.is_none());
        f.cleanup();
    }
}

#[test]
fn native_new_ticket_every_sync_and_actual_disposition_cut_retains_original_ticket_and_history() {
    let mut steps: Vec<_> = (0..11).map(NativeStep::NewTicketSourceSynced).collect();
    steps.extend([
        NativeStep::EndpointSynced,
        NativeStep::EndpointRetired,
        NativeStep::DispositionComplete,
        NativeStep::DispositionClosureUnlinked,
        NativeStep::DispositionTicketUnlinked,
        NativeStep::HistoryRenamePrepared,
        NativeStep::HistoryRenamed,
        NativeStep::HistorySynced,
        NativeStep::History,
    ]);
    for step in steps {
        let mut f = Fixture::new_committed("positive").with_ticket();
        let past = prior_audits(&f.state, 7);
        f.capture("positive").unwrap();
        let held_ticket = File::open(
            f.state
                .join(crate::restore_disposition_ticket_model::TICKET_MEMBER),
        )
        .unwrap();
        let before = f
            .engine
            .lower
            .io
            .original(Slot::Scratch0)
            .unwrap()
            .metadata()
            .unwrap()
            .ino();
        assert!(f.finish(Some(step)).is_err(), "public ticket cut {step:?}");
        assert!(f.engine.sealed && !f.engine.disposition_ready());
        assert!(f.engine.lower.io.test_retains_original(Slot::Scratch0));
        assert_eq!(held_ticket.metadata().unwrap().ino(), before);
        f.retained();
        assert_past_unchanged(&f.state, &past);
        assert!(f.finish(None).is_err());
        drop(past);
        drop(held_ticket);
        f.cleanup();
    }
}

#[test]
fn native_new_ticket_original_gate_failures_and_same_byte_ticket_swap_seal_once() {
    let mut good = Fixture::new_committed("positive").with_ticket();
    good.capture("positive").unwrap();
    good.finish(None).unwrap();
    let calls = good.gate.calls;
    good.cleanup();
    for fail in 1..=calls {
        let mut f = Fixture::new_committed("positive").with_ticket();
        f.capture("positive").unwrap();
        f.gate.fail = Some(fail);
        assert!(f.finish(None).is_err(), "public ticket origin gate {fail}");
        assert!(f.engine.sealed && !f.engine.disposition_ready());
        f.retained();
        assert!(f.engine.lower.io.test_retains_original(Slot::Scratch0));
        assert!(f.finish(None).is_err());
        f.cleanup();
    }
    for step in [
        NativeStep::NewTicketSourceSynced(0),
        NativeStep::NewTicketSourceSynced(10),
        NativeStep::DispositionComplete,
    ] {
        let mut f = Fixture::new_committed("positive").with_ticket();
        f.capture("positive").unwrap();
        let state = f.state.clone();
        assert!(
            f.finish_with(&mut |event| {
                if event == step {
                    let path = state.join(crate::restore_disposition_ticket_model::TICKET_MEMBER);
                    let bytes = fs::read(&path).unwrap();
                    fs::rename(&path, state.join("displaced-ticket")).unwrap();
                    drop(write(&path, &bytes));
                }
                Ok(())
            })
            .is_err()
        );
        assert!(f.engine.sealed && !f.engine.disposition_ready());
        f.retained();
        f.cleanup();
    }
}

#[test]
fn native_new_ticket_actual_authenticated_archive_output_and_missing_observed_role_controls() {
    let root = crate::test_temp::directory("new-ticket-auth").unwrap();
    let source = root.join("new.ovb");
    let password = b"synthetic ticket passphrase";
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
    assert!(crate::backup_destination_candidate::open_existing(&source, uid, b"wrong").is_err());
    let opened =
        crate::backup_destination_candidate::open_existing(&source, uid, password).unwrap();
    let off = opened.restore_store_off().unwrap();
    let members = [MEMBERS[0], MEMBERS[1], off.as_slice(), opened.template()];
    let mut good = Fixture::from_members("positive", true, members).with_ticket();
    good.capture("positive").unwrap();
    good.finish(None).unwrap();
    assert!(good.engine.new_ticket_qualified() && good.engine.disposition_ready());
    good.retained();
    good.cleanup();
    for role in [
        NativeTicketRole::None,
        NativeTicketRole::OwnedCut,
        NativeTicketRole::ObservedNew([0; 32]),
    ] {
        let mut f = Fixture::from_members("positive", true, members).with_ticket();
        f.capture("positive").unwrap();
        f.engine.ticket_role = role;
        assert!(f.finish(None).is_err());
        assert!(f.engine.sealed && f.engine.fresh_new_ticket.is_none());
        assert!(
            !f.state
                .join(crate::restore_disposition_complete_model::COMPLETE_MEMBER)
                .exists()
        );
        f.cleanup();
    }
    drop(opened);
    fs::remove_dir_all(root).unwrap(); // completed wholly synthetic authentication fixture
}
