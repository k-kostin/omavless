// SPDX-License-Identifier: MIT
//! Private desktop-only staged driver. No exported writer/activation path.
use super::*;
use omavless_runtime::app_proxy::{
    journal::staged::{Decision, Relationship, StagedJournal},
    staged::{Effect as StagedEffect, Stage, Step},
    transaction::Error,
};

const MODE: Field = Field::Desktop(DesktopKey::Mode);

fn replace(state: &State, effect: &Effect) -> State {
    let desktop = DesktopSnapshot::decode(&state.encode().unwrap()[0]).unwrap();
    let (Field::Desktop(key), Value::Desktop(replacement)) = (effect.field, &effect.replacement)
    else {
        panic!("desktop only")
    };
    let entries = desktop
        .entries()
        .iter()
        .map(|entry| {
            if entry.key == key {
                replacement.clone()
            } else {
                entry.clone()
            }
        })
        .collect();
    State::new(
        DesktopSnapshot::capture(entries).unwrap(),
        crate::project_manager_environment(&[]).unwrap(),
    )
}

fn assert_quiescent(state: &State) {
    let Value::Desktop(mode) = state.value(MODE) else {
        unreachable!()
    };
    assert!(mode.effective == DesktopValue::String("none".into()));
    assert!(mode.user == Override::Present(DesktopValue::String("none".into())));
}

fn staged_record(fixture: &Fixture) -> Vec<u8> {
    fs::read(fixture.root.join("app-proxy-staged/app-proxy-journal.json")).unwrap()
}

struct Driver {
    journal: StagedJournal,
    host: DconfHost,
    original: State,
    fenced: bool,
}
impl Driver {
    fn setup(role: &str) -> (Self, Rc<RefCell<Fixture>>) {
        let fixture = Rc::new(RefCell::new(Fixture::new()));
        fixture.borrow().run("seed");
        if role != "seed" {
            fixture.borrow().run(role);
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(fixture.borrow().root.join("app-proxy-staged"))
            .unwrap();
        let mut host = DconfHost::new(fixture.clone());
        let original = host.observe(binding()).unwrap();
        let journal = StagedJournal::create(
            &fixture.borrow().root,
            binding(),
            original.clone(),
            intended(&original),
        )
        .unwrap();
        (
            Self {
                journal,
                host,
                original,
                fenced: false,
            },
            fixture,
        )
    }
    fn intent(&mut self) -> Result<Option<StagedEffect>, Error> {
        if self.fenced {
            return Err(Error::DrainRequired);
        }
        let before = self.host.observe(binding()).map_err(Error::Host)?;
        let effect = self
            .journal
            .begin_next(binding(), &before)
            .map_err(Error::Journal)?;
        if let Some(effect) = &effect {
            if matches!(effect.step, Step::ApplyControl(_) | Step::RestoreControl(_)) {
                assert_quiescent(&before);
            }
            if effect.step == Step::RestoreMode {
                for key in DesktopKey::ALL
                    .into_iter()
                    .filter(|key| *key != DesktopKey::Mode)
                {
                    let field = Field::Desktop(key);
                    assert_eq!(before.value(field), self.original.value(field));
                }
            }
            if self.host.observe(binding()).map_err(Error::Host)? != before {
                return Err(Error::Host(HostError::Changed));
            }
            self.fenced = true;
        }
        Ok(effect)
    }
    fn step(&mut self) -> Result<Option<Step>, Error> {
        let Some(effect) = self.intent()? else {
            return Ok(None);
        };
        let _unknown_until_drained = self.host.write(binding(), &effect.change);
        self.host.drain(binding()).map_err(Error::Host)?;
        let observed = self.host.observe(binding()).map_err(Error::Host)?;
        self.journal
            .confirm(binding(), &observed)
            .map_err(Error::Journal)?;
        self.fenced = false;
        Ok(Some(effect.step))
    }
    fn reopen(self) -> Self {
        let Self {
            journal,
            host,
            original,
            ..
        } = self;
        let before = staged_record(&host.fixture.borrow());
        drop(journal);
        let journal = StagedJournal::open(&host.fixture.borrow().root, binding()).unwrap();
        assert_eq!(staged_record(&host.fixture.borrow()), before);
        Self {
            journal,
            host,
            original,
            fenced: true,
        }
    }
    fn begin_restore(&mut self) -> Result<(), Error> {
        self.fenced = true;
        self.host.drain(binding()).map_err(Error::Host)?;
        let observed = self.host.observe(binding()).map_err(Error::Host)?;
        self.journal
            .begin_restore(binding(), &observed)
            .map_err(Error::Journal)?;
        self.fenced = false;
        Ok(())
    }
    fn apply(&mut self) {
        while self.step().unwrap().is_some() {}
        assert_eq!(self.journal.stage(), Stage::Active);
    }
    fn restore(&mut self) {
        self.begin_restore().unwrap();
        while self.step().unwrap().is_some() {}
        assert_eq!(self.journal.stage(), Stage::Released);
        assert_eq!(self.host.observe(binding()).unwrap(), self.original);
    }
    fn before_mode(&mut self, step: Step) {
        match step {
            Step::QuiesceApply => (),
            Step::Enable => {
                for _ in 0..16 {
                    self.step().unwrap();
                }
            }
            Step::QuiesceRestore => {
                self.apply();
                self.begin_restore().unwrap();
            }
            Step::RestoreMode => {
                self.apply();
                self.begin_restore().unwrap();
                for _ in 0..16 {
                    self.step().unwrap();
                }
            }
            _ => panic!("fixed mode step"),
        }
    }
}

#[test]
#[ignore = "private installed dconf; all saved none/manual/PAC apply prefixes, no runtime activation"]
fn every_staged_apply_prefix_restores_exact_saved_manual_pac_and_none() {
    for role in ["seed", "seed-manual", "seed-auto"] {
        for confirmed in 0..=17 {
            let (mut driver, _) = Driver::setup(role);
            for _ in 0..confirmed {
                driver.step().unwrap();
            }
            let mut driver = driver.reopen();
            assert_eq!(driver.step(), Err(Error::DrainRequired));
            driver.restore();
        }
    }
}

#[test]
#[ignore = "private dconf; every saved manual/PAC/none restoration prefix retains original mode last"]
fn every_staged_restore_prefix_reenters_without_exposing_unrestored_controls() {
    for role in ["seed", "seed-manual", "seed-auto"] {
        // Reentry while the same port survives, after EACH compensation field.
        let (mut driver, _) = Driver::setup(role);
        driver.apply();
        driver.begin_restore().unwrap();
        while driver.step().unwrap().is_some() {
            if driver.journal.stage() != Stage::Released {
                driver = driver.reopen();
                driver.begin_restore().unwrap();
            }
        }
        assert_eq!(driver.journal.stage(), Stage::Released);
        assert_eq!(driver.host.observe(binding()).unwrap(), driver.original);
    }
}

#[test]
#[ignore = "private dconf; all four mode operations can commit after timeout, none may bypass origin drain"]
fn all_four_unknown_mode_transitions_require_retained_settlement() {
    for role in ["seed", "seed-manual", "seed-auto"] {
        for step in [
            Step::QuiesceApply,
            Step::Enable,
            Step::QuiesceRestore,
            Step::RestoreMode,
        ] {
            let (mut driver, fixture) = Driver::setup(role);
            driver.before_mode(step);
            let before = driver.host.observe(binding()).unwrap();
            fixture
                .borrow_mut()
                .service
                .as_mut()
                .unwrap()
                .signal(Signal::SIGSTOP);
            assert!(driver.step().is_err());
            let bytes = staged_record(&fixture.borrow());
            let review = driver.journal.recovery_review(binding(), &before).unwrap();
            assert_eq!(review.pending, Some(step));
            assert_eq!(review.relationship, Relationship::RecordedBefore);
            assert_eq!(review.decision, Decision::RetainUnsettledEvidence);
            assert_eq!(driver.step(), Err(Error::DrainRequired));
            assert!(driver.begin_restore().is_err());
            assert_eq!(staged_record(&fixture.borrow()), bytes);
            assert_eq!(driver.host.observe(binding()).unwrap(), before);
            fixture
                .borrow_mut()
                .service
                .as_mut()
                .unwrap()
                .signal(Signal::SIGCONT);
            driver.restore();
        }
    }
}

#[test]
#[ignore = "private dconf; staged external mode/control edits preserve values and durable evidence"]
fn staged_foreign_mode_and_control_edits_are_not_owned_third_values() {
    for (prefix, role) in [
        (0, "foreign-mode"),
        (0, "foreign-none"),
        (5, "foreign-host"),
        (17, "foreign-host"),
    ] {
        let (mut driver, fixture) = Driver::setup("seed-manual");
        for _ in 0..prefix {
            driver.step().unwrap();
        }
        let bytes = staged_record(&fixture.borrow());
        fixture.borrow().run(role);
        let external = driver.host.observe(binding()).unwrap();
        assert_eq!(
            driver
                .journal
                .recovery_review(binding(), &external)
                .unwrap()
                .decision,
            Decision::PreserveForeignEdits
        );
        assert!(driver.begin_restore().is_err());
        assert_eq!(driver.step(), Err(Error::DrainRequired));
        assert_eq!(staged_record(&fixture.borrow()), bytes);
        assert_eq!(driver.host.observe(binding()).unwrap(), external);
    }
}

pub(in crate::gio_host::persistent_tests) fn crash_child(
    root: &Path,
    schemas: &[gio::SettingsSchema],
) {
    let observed = State::new(
        read_desktop(schemas).unwrap(),
        crate::project_manager_environment(&[]).unwrap(),
    );
    let journal = StagedJournal::open(root, binding()).unwrap();
    let review = journal.recovery_review(binding(), &observed).unwrap();
    assert_eq!(review.relationship, Relationship::RecordedBefore);
    let step = review.pending.unwrap();
    assert!(matches!(
        step,
        Step::QuiesceApply | Step::Enable | Step::QuiesceRestore | Step::RestoreMode
    ));
    let target = DesktopSnapshot::decode(
        &omavless_runtime::app_proxy::Snapshot::new(Some(
            fs::read(root.join("staged-crash-target")).unwrap(),
        ))
        .unwrap(),
    )
    .unwrap();
    let entry = target
        .entries()
        .iter()
        .find(|entry| entry.key == DesktopKey::Mode)
        .unwrap();
    if matches!(step, Step::QuiesceApply | Step::QuiesceRestore) {
        assert!(entry.effective == DesktopValue::String("none".into()));
        assert!(entry.user == Override::Present(DesktopValue::String("none".into())));
    } else if step == Step::Enable {
        assert!(entry.effective == DesktopValue::String("manual".into()));
    }
    // This fixed private fixture file is synthetic test input, NOT installed
    // effect authority. Only mode on this fixture's own bus may be changed.
    let value = match &entry.user {
        Override::Absent => None,
        Override::Present(value) => Some(variant(value)),
    };
    send_mode_value_then_exit(root, value);
}

#[test]
#[ignore = "private dconf; all four dead-writer mode effects stay unsettled before/after delayed commit"]
fn dead_writer_each_mode_operation_never_inherits_a_new_connection_fence() {
    for role in ["seed", "seed-manual", "seed-auto"] {
        for step in [
            Step::QuiesceApply,
            Step::Enable,
            Step::QuiesceRestore,
            Step::RestoreMode,
        ] {
            let (mut driver, fixture) = Driver::setup(role);
            driver.before_mode(step);
            let before = driver.host.observe(binding()).unwrap();
            let effect = driver.intent().unwrap().unwrap();
            assert_eq!(effect.step, step);
            let after = replace(&before, &effect.change);
            private_file(
                &fixture.borrow().root.join("staged-crash-target"),
                after.encode().unwrap()[0].bytes().unwrap(),
            );
            drop(driver);
            fixture
                .borrow_mut()
                .service
                .as_mut()
                .unwrap()
                .signal(Signal::SIGSTOP);
            let mut child = fixture.borrow().child("staged-mode-crash");
            fixture
                .borrow()
                .wait_for(|| fixture.borrow().root.join("transaction-crashed").exists());
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    assert_eq!(status.code(), Some(33));
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "owned staged writer exit deadline"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let bytes = staged_record(&fixture.borrow());
            let mut host = DconfHost::new(fixture.clone());
            let mut reopened = StagedJournal::open(&fixture.borrow().root, binding()).unwrap();
            assert_eq!(host.observe(binding()).unwrap(), before);
            assert_eq!(
                reopened
                    .recovery_review(binding(), &before)
                    .unwrap()
                    .decision,
                Decision::RetainUnsettledEvidence
            );
            assert!(matches!(
                reopened.begin_next(binding(), &before),
                Err(omavless_runtime::app_proxy::journal::Error::RecoveryRequired)
            ));
            fixture
                .borrow_mut()
                .service
                .as_mut()
                .unwrap()
                .signal(Signal::SIGCONT);
            let deadline = Instant::now() + Duration::from_secs(5);
            while host.observe(binding()).unwrap() != after {
                assert!(
                    Instant::now() < deadline,
                    "private staged delayed commit deadline"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let review = reopened.recovery_review(binding(), &after).unwrap();
            assert_eq!(review.pending, Some(step));
            assert_eq!(review.relationship, Relationship::PendingAfter);
            assert_eq!(review.decision, Decision::RetainUnsettledEvidence);
            assert_eq!(staged_record(&fixture.borrow()), bytes);
            assert!(matches!(
                reopened.begin_next(binding(), &after),
                Err(omavless_runtime::app_proxy::journal::Error::RecoveryRequired)
            ));
        }
    }
}
