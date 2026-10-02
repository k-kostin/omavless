// SPDX-License-Identifier: MIT
//! Ordered writes are intentionally a private test driver, not a runtime
//! constructor. Only the fixture-created bus/service/database can reach it.
use super::*;
use omavless_runtime::app_proxy::{
    fields::Order,
    journal::fields::{ObservedRelationship, RecoveryDecision},
    transaction::Error,
};

const ORDER: Order = Order::ModeLastOriginalNone;
const MODE: Field = Field::Desktop(DesktopKey::Mode);

struct PrivateDriver {
    journal: FieldJournal,
    host: DconfHost,
    fenced: bool,
}

impl PrivateDriver {
    fn setup() -> (Self, State, Rc<RefCell<Fixture>>) {
        let fixture = Rc::new(RefCell::new(Fixture::new()));
        fixture.borrow().run("seed");
        let mut host = DconfHost::new(fixture.clone());
        let original = host.observe(binding()).unwrap();
        let journal = FieldJournal::create_ordered(
            &fixture.borrow().root,
            binding(),
            original.clone(),
            intended(&original),
            ORDER,
        )
        .unwrap();
        (
            Self {
                journal,
                host,
                fenced: false,
            },
            original,
            fixture,
        )
    }

    fn step(&mut self) -> Result<Option<Field>, Error> {
        if self.fenced {
            return Err(Error::DrainRequired);
        }
        let before = self.host.observe(binding()).map_err(Error::Host)?;
        let Some(effect) = self
            .journal
            .begin_next(binding(), &before)
            .map_err(Error::Journal)?
        else {
            return Ok(None);
        };
        if self.host.observe(binding()).map_err(Error::Host)? != before {
            return Err(Error::Host(HostError::Changed));
        }
        self.fenced = true;
        let _unknown_until_drained = self.host.write(binding(), &effect);
        self.host.drain(binding()).map_err(Error::Host)?;
        let observed = self.host.observe(binding()).map_err(Error::Host)?;
        self.journal
            .confirm(binding(), &observed)
            .map_err(Error::Journal)?;
        self.fenced = false;
        Ok(Some(effect.field))
    }

    fn reopen(self) -> Self {
        let Self { journal, host, .. } = self;
        let bytes = record(&host.fixture.borrow());
        drop(journal);
        let journal = FieldJournal::open(&host.fixture.borrow().root, binding()).unwrap();
        assert_eq!(record(&host.fixture.borrow()), bytes);
        assert_eq!(journal.order(), ORDER);
        Self {
            journal,
            host,
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
}

fn mode(fixture: &Fixture) -> Value {
    State::new(
        read_independently(fixture),
        crate::project_manager_environment(&[]).unwrap(),
    )
    .value(MODE)
}

#[test]
#[ignore = "private installed dconf; mode-last journal model at every partial prefix, no runtime admission"]
fn mode_last_every_prefix_restores_none_first_and_exact_layers() {
    let sequence: Vec<_> = ORDER
        .fields()
        .filter(|field| matches!(field, Field::Desktop(_)))
        .collect();
    assert_eq!(sequence.len(), 16);
    for confirmed in 0..=16 {
        let (mut driver, original, fixture) = PrivateDriver::setup();
        for expected in sequence.iter().take(confirmed) {
            if *expected != MODE {
                assert_eq!(mode(&fixture.borrow()), original.value(MODE));
            }
            assert_eq!(driver.step().unwrap(), Some(*expected));
        }
        let mut driver = driver.reopen();
        assert_eq!(driver.step(), Err(Error::DrainRequired));
        driver.begin_restore().unwrap();
        let mut restored = 0;
        if confirmed == 16 {
            // Only mode changes first. No endpoint/PAC/auth/bypass restoration
            // can start while the intended desktop mode is still enabled.
            assert_eq!(driver.step().unwrap(), Some(MODE));
            restored = 1;
            driver = driver.reopen();
            driver.begin_restore().unwrap();
        }
        while driver.journal.phase() != Phase::Released {
            assert_eq!(mode(&fixture.borrow()), original.value(MODE));
            let next = driver.step().unwrap();
            if confirmed == 16 && next.is_some() {
                assert_eq!(next, Some(sequence[15 - restored]));
                restored += 1;
                // Reenter at every confirmed compensation prefix while the
                // exact same port remains retained; not process-crash takeover.
                driver = driver.reopen();
                driver.begin_restore().unwrap();
            }
        }
        assert_eq!(
            read_independently(&fixture.borrow()).encode().unwrap(),
            original.encode().unwrap()[0]
        );
    }
}

#[test]
#[ignore = "private dconf; delayed LAST mode write settles on retained origin before none-first restore"]
fn delayed_last_mode_commit_does_not_become_a_recovery_receipt() {
    let (mut driver, original, fixture) = PrivateDriver::setup();
    for _ in 0..15 {
        driver.step().unwrap();
    }
    assert_eq!(mode(&fixture.borrow()), original.value(MODE));
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGSTOP);
    assert!(driver.step().is_err());
    let pending = record(&fixture.borrow());
    let observed = driver.host.observe(binding()).unwrap();
    let review = driver
        .journal
        .recovery_review(binding(), &observed)
        .unwrap();
    assert_eq!(review.pending_field, Some(MODE));
    assert_eq!(review.observed, ObservedRelationship::RecordedMixture);
    assert_eq!(review.decision, RecoveryDecision::RetainUnsettledEvidence);
    assert_eq!(review.attempted_fields, 16);
    assert_eq!(driver.step(), Err(Error::DrainRequired));
    assert!(driver.begin_restore().is_err());
    assert_eq!(record(&fixture.borrow()), pending);
    assert_eq!(mode(&fixture.borrow()), original.value(MODE));
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGCONT);
    driver.begin_restore().unwrap();
    assert_eq!(mode(&fixture.borrow()), intended(&original).value(MODE));
    assert_eq!(driver.step().unwrap(), Some(MODE));
    assert_eq!(mode(&fixture.borrow()), original.value(MODE));
    while driver.journal.phase() != Phase::Released {
        driver.step().unwrap();
    }
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
}

#[test]
#[ignore = "private dconf; unknown FIRST disable cannot admit endpoint restoration before same-origin drain"]
fn delayed_first_mode_disable_fences_every_remaining_restore_field() {
    let (mut driver, original, fixture) = PrivateDriver::setup();
    for _ in 0..16 {
        driver.step().unwrap();
    }
    driver.begin_restore().unwrap();
    let active = driver.host.observe(binding()).unwrap();
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGSTOP);
    assert!(driver.step().is_err());
    let bytes = record(&fixture.borrow());
    assert_eq!(driver.step(), Err(Error::DrainRequired));
    assert!(driver.begin_restore().is_err());
    assert_eq!(record(&fixture.borrow()), bytes);
    assert_eq!(driver.host.observe(binding()).unwrap(), active);
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGCONT);
    driver.begin_restore().unwrap();
    assert_eq!(mode(&fixture.borrow()), original.value(MODE));
    while driver.journal.phase() != Phase::Released {
        driver.step().unwrap();
    }
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
}

#[test]
#[ignore = "private dconf; prior manual/PAC is explicitly unsupported, with no record or effect"]
fn saved_manual_pac_are_refused_without_changing_any_private_value() {
    for role in ["seed-manual", "seed-auto"] {
        let fixture = Rc::new(RefCell::new(Fixture::new()));
        fixture.borrow().run("seed");
        fixture.borrow().run(role);
        let mut host = DconfHost::new(fixture.clone());
        let original = host.observe(binding()).unwrap();
        assert!(matches!(
            FieldJournal::create_ordered(
                &fixture.borrow().root,
                binding(),
                original.clone(),
                intended(&original),
                ORDER,
            ),
            Err(omavless_runtime::app_proxy::journal::Error::Planner(
                omavless_runtime::app_proxy::Error::UnsupportedOrder
            ))
        ));
        assert!(
            !fixture
                .borrow()
                .root
                .join("app-proxy-fields/app-proxy-journal.json")
                .exists()
        );
        assert_eq!(host.observe(binding()).unwrap(), original);
    }
}

#[test]
#[ignore = "private dconf; external mode/endpoint edits must not be overwritten by ordered compensation"]
fn ordered_restore_preserves_external_mode_and_endpoint_edits() {
    for (prefix, role) in [(5, "foreign-mode"), (16, "foreign-host")] {
        let (mut driver, _, fixture) = PrivateDriver::setup();
        for _ in 0..prefix {
            driver.step().unwrap();
        }
        let before = record(&fixture.borrow());
        fixture.borrow().run(role);
        let external = driver.host.observe(binding()).unwrap();
        assert_eq!(
            driver
                .journal
                .recovery_review(binding(), &external)
                .unwrap()
                .decision,
            RecoveryDecision::PreserveForeignEdits
        );
        assert!(driver.begin_restore().is_err());
        assert_eq!(driver.step(), Err(Error::DrainRequired));
        assert_eq!(record(&fixture.borrow()), before);
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
    let journal = FieldJournal::open(root, binding()).unwrap();
    assert_eq!(journal.order(), ORDER);
    let review = journal.recovery_review(binding(), &observed).unwrap();
    assert_eq!(review.pending_field, Some(MODE));
    assert_eq!(review.attempted_fields, 16);
    assert_eq!(review.different_from_original, 15);
    // The test-only child is deliberately acting on a durable pending intent,
    // not claiming that opening a journal grants production effect authority.
    send_mode_then_exit(root);
}

#[test]
#[ignore = "private dconf; dead writer's LAST mode request may enable after its process exits"]
fn dead_writer_delayed_mode_enable_stays_fenced_before_and_after_commit() {
    let (mut driver, original, fixture) = PrivateDriver::setup();
    for _ in 0..15 {
        driver.step().unwrap();
    }
    let observed = driver.host.observe(binding()).unwrap();
    let pending = driver
        .journal
        .begin_next(binding(), &observed)
        .unwrap()
        .unwrap();
    assert_eq!(pending.field, MODE);
    drop(driver);
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGSTOP);
    let mut writer = fixture.borrow().child("ordered-mode-crash");
    fixture
        .borrow()
        .wait_for(|| fixture.borrow().root.join("transaction-crashed").exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = writer.0.try_wait().unwrap() {
            assert_eq!(status.code(), Some(33));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "private ordered crash child timeout"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let bytes = record(&fixture.borrow());
    let mut host = DconfHost::new(fixture.clone());
    let journal = FieldJournal::open(&fixture.borrow().root, binding()).unwrap();
    let before = host.observe(binding()).unwrap();
    assert_eq!(before.value(MODE), original.value(MODE));
    assert_eq!(
        journal
            .recovery_review(binding(), &before)
            .unwrap()
            .decision,
        RecoveryDecision::RetainUnsettledEvidence
    );
    assert!(matches!(
        Transaction::new(journal, host, binding()),
        Err(Error::Journal(
            omavless_runtime::app_proxy::journal::Error::RecoveryRequired
        ))
    ));
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGCONT);
    fixture
        .borrow()
        .wait_for(|| mode(&fixture.borrow()) == intended(&original).value(MODE));
    assert_eq!(record(&fixture.borrow()), bytes);
    let journal = FieldJournal::open(&fixture.borrow().root, binding()).unwrap();
    let after = State::new(
        read_independently(&fixture.borrow()),
        crate::project_manager_environment(&[]).unwrap(),
    );
    let review = journal.recovery_review(binding(), &after).unwrap();
    assert_eq!(review.observed, ObservedRelationship::Intended);
    assert_eq!(review.different_from_original, 16);
    assert_eq!(review.pending_field, Some(MODE));
    assert_eq!(review.decision, RecoveryDecision::RetainUnsettledEvidence);
}
