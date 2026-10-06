// SPDX-License-Identifier: MIT
//! Real local files exercising the same retained binding/digest/catalogue helpers.
//! No actor, manager admission, root path, namespace, socket or recovery effect.
use super::*;
use std::fs::{self, File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const PAIR: [&[u8]; 4] = [
    b"public-old-store",
    b"public-old-template",
    b"public-new-store",
    b"public-new-template",
];
struct Fixture {
    _root: tempfile::TempDir,
    config: PathBuf,
    state: PathBuf,
    stage: PathBuf,
    owner: Stage,
    intent: [u8; RECORD_BYTES],
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
        let config = root.path().join("config");
        let state = root.path().join("state");
        let stage = state.join(PENDING_DIRECTORY);
        for path in [&config, &state, &stage] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, Permissions::from_mode(0o700)).unwrap();
        }
        let plan = planned_stage_identity(PAIR).unwrap();
        let intent = DecisionRecord::intent(1, None, &plan, [7; 16])
            .unwrap()
            .encode();
        let mut names = vec![
            (Slot::Transaction, root.path().to_path_buf()),
            (Slot::Config, config.clone()),
            (Slot::State, state.clone()),
            (Slot::StageDirectory, stage.clone()),
        ];
        for (slot, name, bytes, parent) in [
            (LIVE[0].0, LIVE[0].1, PAIR[2], &config),
            (LIVE[1].0, LIVE[1].1, PAIR[1], &config),
            (Slot::StageOldStore, MEMBERS[0], PAIR[0], &stage),
            (Slot::StageOldTemplate, MEMBERS[1], PAIR[1], &stage),
            (Slot::StageNewStore, MEMBERS[2], PAIR[2], &stage),
            (Slot::StageNewTemplate, MEMBERS[3], PAIR[3], &stage),
            (Slot::Intent, INTENT, intent.as_slice(), &state),
        ] {
            let path = parent.join(name);
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            names.push((slot, path));
        }
        let ready = stage.join(READY_MEMBER);
        fs::write(&ready, ready_bytes(PAIR)).unwrap();
        fs::set_permissions(&ready, Permissions::from_mode(0o600)).unwrap();
        names.push((Slot::StageReady, ready));
        let mut owner = Stage::reserve_canonical();
        let mut files = Vec::new();
        for (slot, path) in names {
            let file = File::open(path).unwrap();
            owner.original[slot as usize] = Some(file.metadata().unwrap());
            files.push((slot, file));
        }
        owner.io = FileIo::local_files(files);
        Self {
            _root: root,
            config,
            state,
            stage,
            owner,
            intent,
        }
    }
    fn until() -> Instant {
        Instant::now() + std::time::Duration::from_secs(1)
    }
    fn state_catalogue(&mut self) -> Result<(), Unavailable> {
        self.owner
            .catalogue(Slot::State, &[PENDING_DIRECTORY, INTENT], Self::until())
    }
}

#[test]
fn optional_absence_drift_and_unknown_inventory_are_actual_directory_refusals() {
    for name in [TERMINAL, "foreign-record"] {
        let mut f = Fixture::new();
        f.state_catalogue().unwrap();
        fs::write(f.state.join(name), b"public prefix").unwrap();
        assert!(f.state_catalogue().is_err());
        assert!(f.owner.finish().is_err());
        assert!(
            f.owner
                .io
                .perform(
                    Slot::State,
                    || panic!("sealed gate"),
                    |_| panic!("sealed operation")
                )
                .is_err()
        );
    }
    let mut f = Fixture::new();
    f.owner
        .catalogue(Slot::Config, &[LIVE[0].1, LIVE[1].1], Fixture::until())
        .unwrap();
    fs::write(f.config.join(REPLACEMENTS[0].1), PAIR[2]).unwrap();
    assert!(
        f.owner
            .catalogue(Slot::Config, &[LIVE[0].1, LIVE[1].1], Fixture::until())
            .is_err()
    );
}

#[test]
fn every_captured_member_refuses_same_byte_replacement_without_new_adoption() {
    for index in 0..MEMBERS.len() {
        let mut f = Fixture::new();
        let path = f.stage.join(MEMBERS[index]);
        f.owner
            .verify_member(
                Slot::StageDirectory,
                STAGED[index],
                MEMBERS[index],
                PAIR[index],
                Fixture::until(),
            )
            .unwrap();
        fs::rename(&path, f.stage.join("displaced")).unwrap();
        fs::write(&path, PAIR[index]).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        assert!(
            f.owner
                .verify_member(
                    Slot::StageDirectory,
                    STAGED[index],
                    MEMBERS[index],
                    PAIR[index],
                    Fixture::until()
                )
                .is_err()
        );
        assert!(f.owner.finish().is_err());
    }
}

#[test]
fn held_live_mutation_mode_link_and_directory_identity_drift_refuse() {
    for cut in 0..4 {
        let mut f = Fixture::new();
        let live = f.config.join(LIVE[0].1);
        // The exact same fixed-component check must pass before mutation;
        // refusal of an invalid name would not prove directory drift.
        if cut == 3 {
            f.owner
                .binding(
                    Slot::Transaction,
                    Slot::Config,
                    "config",
                    true,
                    Fixture::until(),
                )
                .unwrap();
        } else {
            f.owner
                .verify_member(
                    Slot::Config,
                    LIVE[0].0,
                    LIVE[0].1,
                    PAIR[2],
                    Fixture::until(),
                )
                .unwrap();
        }
        match cut {
            0 => fs::write(&live, b"changed-public-bytes").unwrap(),
            1 => fs::set_permissions(&live, Permissions::from_mode(0o644)).unwrap(),
            2 => fs::hard_link(&live, f.config.join("extra-link")).unwrap(),
            _ => {
                fs::rename(&f.config, f._root.path().join("old-config")).unwrap();
                fs::create_dir(&f.config).unwrap();
            }
        }
        // File binding repeats held metadata and named inode, not just digest.
        let result = if cut == 3 {
            f.owner.binding(
                Slot::Transaction,
                Slot::Config,
                "config",
                true,
                Fixture::until(),
            )
        } else {
            f.owner.verify_member(
                Slot::Config,
                LIVE[0].0,
                LIVE[0].1,
                PAIR[2],
                Fixture::until(),
            )
        };
        assert!(result.is_err());
    }
}

#[test]
fn actual_record_reads_reject_stale_crossed_torn_and_partial_evidence() {
    let mut f = Fixture::new();
    let plan = planned_stage_identity(PAIR).unwrap();
    f.owner
        .verify_member(
            Slot::State,
            Slot::Intent,
            INTENT,
            &f.intent,
            Fixture::until(),
        )
        .unwrap();
    let (actual, terminal) = f.owner.inspection_records(false, Fixture::until()).unwrap();
    assert!(terminal.is_none());
    let chain = DecisionChain::decode(&actual, None).unwrap();
    let changed = DecisionRecord::intent(1, None, &plan, [8; 16])
        .unwrap()
        .encode();
    assert_ne!(actual, changed);
    assert!(
        f.owner
            .verify_member(
                Slot::State,
                Slot::Intent,
                INTENT,
                &changed,
                Fixture::until()
            )
            .is_err()
    );
    for cut in 0..3 {
        let mut f = Fixture::new();
        let mut bytes = f.intent.to_vec();
        match cut {
            0 => bytes[0] ^= 1,
            1 => {
                bytes.pop();
            }
            _ => bytes = changed.to_vec(),
        }
        fs::write(f.state.join(INTENT), bytes).unwrap();
        assert!(
            f.owner
                .verify_member(
                    Slot::State,
                    Slot::Intent,
                    INTENT,
                    &f.intent,
                    Fixture::until()
                )
                .is_err()
        );
    }
    let crossed = DecisionRecord::intent(1, None, &plan, [8; 16])
        .unwrap()
        .terminal(TerminalChoice::Commit)
        .unwrap()
        .encode();
    assert!(DecisionChain::decode(&actual, Some(&crossed)).is_err());
    assert_eq!(
        chain
            .active()
            .review_inspection(1, None, &plan, LivePairClass::Mixed),
        RecoveryReview::OldRollbackCandidate
    );
}

#[test]
fn actual_held_digests_classify_mixed_but_expose_no_rollback_method() {
    let mut f = Fixture::new();
    let store = f
        .owner
        .inspection_digest(LIVE[0].0, 64, Fixture::until())
        .unwrap();
    let template = f
        .owner
        .inspection_digest(LIVE[1].0, 64, Fixture::until())
        .unwrap();
    let class = class_from_matches(
        store == Sha256::digest(PAIR[0])[..],
        template == Sha256::digest(PAIR[1])[..],
        store == Sha256::digest(PAIR[2])[..],
        template == Sha256::digest(PAIR[3])[..],
    );
    assert_eq!(class, LivePairClass::Mixed);
    let plan = planned_stage_identity(PAIR).unwrap();
    let chain = DecisionChain::decode(&f.intent, None).unwrap();
    let result = Inspection {
        review: chain.active().review_inspection(1, None, &plan, class),
        class,
        phase: chain.active().phase(),
    };
    assert!(result.mixed_intent_candidate());
    assert_eq!(result.review, RecoveryReview::OldRollbackCandidate);
    // Observation has no effect callback or live mutation capability. Only
    // current private files are read; this is not a Canonical admission test.
    assert_eq!(fs::read(f.config.join(LIVE[0].1)).unwrap(), PAIR[2]);
    assert_eq!(fs::read(f.config.join(LIVE[1].1)).unwrap(), PAIR[1]);
    assert!(!f.state.join(TERMINAL).exists());
}
