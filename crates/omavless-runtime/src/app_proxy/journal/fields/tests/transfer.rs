// SPDX-License-Identifier: MIT
//! Test-only durable transfer experiment. Synthetic provenance/receipt inputs
//! are not host authority. This format/API is absent from every non-test build.

use super::*;
use crate::app_proxy::takeover::{
    self, Admission, GracefulReceipt, HostScope, Predecessor, RecordIdentity, RecordState,
};
use sha2::{Digest, Sha256};

mod quiescence;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scope {
    manager: [u8; 16],
    bus: [u8; 16],
    settings: [u8; 16],
}

impl Scope {
    fn facts(&self) -> HostScope {
        HostScope {
            manager_incarnation: self.manager,
            bus_incarnation: self.bus,
            settings_profile: self.settings,
            shared_activation_environment: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    experiment: u8,
    sequence: u64,
    scope: Scope,
    fields: Record,
}

struct Model {
    journal: FieldJournal,
    sequence: u64,
    scope: Scope,
}

impl Model {
    fn bytes(
        binding: Binding,
        planner: &Planner,
        sequence: u64,
        scope: &Scope,
    ) -> Result<Vec<u8>, Error> {
        if sequence == 0
            || scope.manager == [0; 16]
            || scope.bus == [0; 16]
            || scope.settings == [0; 16]
        {
            return Err(Error::Invalid);
        }
        let envelope = Envelope {
            experiment: 1,
            sequence,
            scope: Scope {
                manager: scope.manager,
                bus: scope.bus,
                settings: scope.settings,
            },
            fields: serde_json::from_slice(&encode(binding, planner)?)
                .map_err(|_| Error::Invalid)?,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(Error::Invalid);
        }
        Ok(bytes)
    }

    fn create(temp: &Temp, planner: Planner) -> Self {
        let scope = Scope {
            manager: [21; 16],
            bus: [22; 16],
            settings: [23; 16],
        };
        let persisted = Self::bytes(binding(), &planner, 1, &scope).unwrap();
        let storage = Storage::acquire(&field_directory(&temp.0)).unwrap();
        storage.replace(None, &persisted).unwrap();
        Self {
            journal: FieldJournal {
                binding: binding(),
                planner,
                storage,
                persisted,
                recovered: true,
                poisoned: false,
            },
            sequence: 1,
            scope,
        }
    }

    fn open(temp: &Temp, expected: Binding) -> Result<Self, Error> {
        let storage = Storage::acquire(&field_directory(&temp.0))?;
        let persisted = storage.read()?.ok_or(Error::Missing)?;
        let envelope: Envelope = serde_json::from_slice(&persisted).map_err(|_| Error::Invalid)?;
        if envelope.experiment != 1 {
            return Err(Error::Invalid);
        }
        let fields = serde_json::to_vec(&envelope.fields).map_err(|_| Error::Invalid)?;
        let (binding, planner) = decode(&fields)?;
        if binding != expected {
            return Err(Error::BindingMismatch);
        }
        if Self::bytes(binding, &planner, envelope.sequence, &envelope.scope)? != persisted {
            return Err(Error::Invalid);
        }
        Ok(Self {
            journal: FieldJournal {
                binding,
                planner,
                storage,
                persisted,
                recovered: true,
                poisoned: false,
            },
            sequence: envelope.sequence,
            scope: envelope.scope,
        })
    }

    fn identity(&self) -> RecordIdentity {
        RecordIdentity {
            digest: Sha256::digest(&self.journal.persisted).into(),
            sequence: self.sequence,
        }
    }

    fn receipt(&self, successor: Binding) -> Predecessor {
        // Synthetic test input only, never an observation or minted authority.
        Predecessor::Graceful(GracefulReceipt {
            record: self.identity(),
            predecessor_instance: self.journal.binding.owner_instance,
            successor_instance: successor.owner_instance,
            workers_joined: true,
            effects_settled: true,
        })
    }

    fn publish(&mut self, binding: Binding, planner: Planner) -> Result<(), Error> {
        self.journal.check(self.journal.binding)?;
        let sequence = self.sequence.checked_add(1).ok_or(Error::Invalid)?;
        let bytes = Self::bytes(binding, &planner, sequence, &self.scope)?;
        if let Err(error) = self
            .journal
            .storage
            .replace(Some(&self.journal.persisted), &bytes)
        {
            self.journal.poisoned = true;
            return Err(error);
        }
        self.journal.binding = binding;
        self.journal.planner = planner;
        self.journal.persisted = bytes;
        self.sequence = sequence;
        Ok(())
    }

    fn transfer(
        &mut self,
        successor: Binding,
        scope: HostScope,
        receipt: Predecessor,
    ) -> Result<(), Error> {
        self.journal.check(self.journal.binding)?;
        takeover::evaluate(Admission {
            record_state: if self.journal.phase() == Phase::Released {
                RecordState::Released
            } else {
                RecordState::Restorable
            },
            record: self.identity(),
            predecessor: self.journal.binding,
            successor,
            old_scope: self.scope.facts(),
            new_scope: scope,
            native_owner_exclusive: true,
            quiescence: receipt,
        })
        .map_err(|_| Error::BindingMismatch)?;
        let mut planner = self.journal.planner.clone();
        planner.owner = successor.owner();
        self.publish(successor, planner)?;
        self.journal.recovered = true;
        Ok(())
    }

    fn restore(&mut self, binding: Binding, current: &mut State) -> Result<(), Error> {
        self.journal.check(binding)?;
        let mut planner = self.journal.planner.clone();
        planner
            .begin_restore(binding.owner(), current)
            .map_err(Error::Planner)?;
        self.publish(binding, planner)?;
        loop {
            let mut planner = self.journal.planner.clone();
            let effect = planner
                .begin_next(binding.owner(), current)
                .map_err(Error::Planner)?;
            self.publish(binding, planner)?;
            let Some(effect) = effect else {
                return Ok(());
            };
            *current = current
                .with(effect.field, effect.replacement)
                .map_err(Error::Planner)?;
            let mut planner = self.journal.planner.clone();
            planner
                .confirm(binding.owner(), current)
                .map_err(Error::Planner)?;
            self.publish(binding, planner)?;
        }
    }
}

fn successor() -> Binding {
    Binding {
        owner_instance: [41; 16],
        owner_generation: binding().owner_generation + 1,
        ..binding()
    }
}

fn partial(confirmed: usize, written: bool) -> (Planner, State, State) {
    let (original, intended) = states();
    let mut planner = Planner::prepare(binding().owner(), original.clone(), intended).unwrap();
    let mut current = original.clone();
    for _ in 0..confirmed {
        let effect = planner
            .begin_next(binding().owner(), &current)
            .unwrap()
            .unwrap();
        current = current.with(effect.field, effect.replacement).unwrap();
        planner.confirm(binding().owner(), &current).unwrap();
    }
    if let Some(effect) = planner.begin_next(binding().owner(), &current).unwrap()
        && written
    {
        current = current.with(effect.field, effect.replacement).unwrap();
    }
    (planner, current, original)
}

#[test]
fn transfer_preserves_every_partial_field_and_restores_exact_original() {
    for confirmed in 0..=FIELD_COUNT {
        for written in [false, true] {
            let temp = Temp::new();
            let (planner, mut current, original) = partial(confirmed, written);
            let old_bytes = encode(binding(), &planner).unwrap();
            let mut model = Model::create(&temp, planner);
            model
                .transfer(successor(), model.scope.facts(), model.receipt(successor()))
                .unwrap();
            assert_eq!(model.sequence, 2);
            assert_eq!(
                encode(binding(), &model.journal.planner).unwrap(),
                old_bytes
            );
            assert!(matches!(
                model.journal.begin_next(successor(), &current),
                Err(Error::RecoveryRequired)
            ));
            assert!(matches!(
                model.restore(binding(), &mut current),
                Err(Error::BindingMismatch)
            ));
            drop(model);
            assert!(matches!(
                Model::open(&temp, binding()),
                Err(Error::BindingMismatch)
            ));
            let mut reopened = Model::open(&temp, successor()).unwrap();
            reopened.restore(successor(), &mut current).unwrap();
            assert_eq!(current, original);
            assert_eq!(reopened.journal.phase(), Phase::Released);
        }
    }
}

#[test]
fn stale_receipt_scope_change_crash_and_foreign_bytes_cannot_transfer() {
    let temp = Temp::new();
    let (planner, _, _) = partial(4, true);
    let mut model = Model::create(&temp, planner);
    let initial = fs::read(temp.record()).unwrap();
    let receipt = model.receipt(successor());
    assert!(
        model
            .transfer(successor(), model.scope.facts(), Predecessor::Unknown)
            .is_err()
    );
    let mut scope = model.scope.facts();
    scope.manager_incarnation[0] ^= 1;
    assert!(model.transfer(successor(), scope, receipt).is_err());
    assert_eq!(fs::read(temp.record()).unwrap(), initial);
    assert!(matches!(Model::open(&temp, binding()), Err(Error::Busy)));
    model
        .transfer(successor(), model.scope.facts(), receipt)
        .unwrap();
    let transferred = fs::read(temp.record()).unwrap();
    let later = Binding {
        owner_instance: [42; 16],
        owner_generation: successor().owner_generation + 1,
        ..successor()
    };
    assert!(model.transfer(later, model.scope.facts(), receipt).is_err());
    assert_eq!(fs::read(temp.record()).unwrap(), transferred);
    // Even a correctly recomputed in-memory receipt cannot overwrite an
    // externally replaced record: Storage compares bytes under the held lock.
    fs::write(temp.record(), &initial).unwrap();
    assert!(matches!(
        model.transfer(later, model.scope.facts(), model.receipt(later)),
        Err(Error::ForeignChange)
    ));
    assert!(model.journal.poisoned);
    assert_eq!(fs::read(temp.record()).unwrap(), initial);
}

#[test]
fn transfer_publication_failures_never_return_a_successor_handle() {
    for checkpoint in [
        Checkpoint::Created,
        Checkpoint::Written,
        Checkpoint::FileSynced,
        Checkpoint::Renamed,
        Checkpoint::DirectorySynced,
    ] {
        let temp = Temp::new();
        let (planner, mut current, original) = partial(4, true);
        let mut model = Model::create(&temp, planner);
        model.journal.storage.fail_at.set(Some(checkpoint));
        assert!(
            model
                .transfer(successor(), model.scope.facts(), model.receipt(successor()))
                .is_err()
        );
        assert!(model.journal.poisoned);
        assert_eq!(model.sequence, 1);
        assert_eq!(model.journal.binding, binding());
        assert!(matches!(
            model.restore(binding(), &mut current),
            Err(Error::RecoveryRequired)
        ));
        drop(model);
        if matches!(
            checkpoint,
            Checkpoint::Created | Checkpoint::Written | Checkpoint::FileSynced
        ) {
            assert!(matches!(
                Model::open(&temp, binding()),
                Err(Error::Interrupted)
            ));
            assert!(matches!(
                Model::open(&temp, successor()),
                Err(Error::Interrupted)
            ));
        } else {
            assert!(matches!(
                Model::open(&temp, binding()),
                Err(Error::BindingMismatch)
            ));
            let mut recovered = Model::open(&temp, successor()).unwrap();
            assert!(matches!(
                recovered.journal.begin_next(successor(), &current),
                Err(Error::RecoveryRequired)
            ));
            recovered.restore(successor(), &mut current).unwrap();
            assert_eq!(current, original);
        }
    }
}

#[test]
fn missing_tombstone_overflow_and_invalid_envelopes_refuse() {
    let temp = Temp::new();
    assert!(matches!(Model::open(&temp, binding()), Err(Error::Missing)));
    let (planner, mut current, _) = partial(0, false);
    let mut model = Model::create(&temp, planner);
    model.sequence = u64::MAX;
    model.journal.persisted =
        Model::bytes(binding(), &model.journal.planner, u64::MAX, &model.scope).unwrap();
    fs::write(temp.record(), &model.journal.persisted).unwrap();
    drop(model);
    let mut model = Model::open(&temp, binding()).unwrap();
    let before = fs::read(temp.record()).unwrap();
    assert!(matches!(
        model.transfer(successor(), model.scope.facts(), model.receipt(successor())),
        Err(Error::Invalid)
    ));
    assert_eq!(fs::read(temp.record()).unwrap(), before);
    model.sequence = 1;
    model.journal.persisted =
        Model::bytes(binding(), &model.journal.planner, 1, &model.scope).unwrap();
    fs::write(temp.record(), &model.journal.persisted).unwrap();
    model.restore(binding(), &mut current).unwrap();
    assert!(
        model
            .transfer(successor(), model.scope.facts(), model.receipt(successor()))
            .is_err()
    );
    let valid = fs::read(temp.record()).unwrap();
    drop(model);
    for field in ["sequence", "experiment"] {
        let mut value: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        value[field] = serde_json::json!(0);
        fs::write(temp.record(), serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(Model::open(&temp, binding()), Err(Error::Invalid)));
    }
    let duplicate = [b"{\"sequence\":1,".as_slice(), &valid[1..]].concat();
    fs::write(temp.record(), duplicate).unwrap();
    assert!(matches!(Model::open(&temp, binding()), Err(Error::Invalid)));
}

const CHECKPOINTS: [Checkpoint; 5] = [
    Checkpoint::Created,
    Checkpoint::Written,
    Checkpoint::FileSynced,
    Checkpoint::Renamed,
    Checkpoint::DirectorySynced,
];

#[test]
fn transfer_crash_worker() {
    let Some(path) = std::env::var_os("OMAVLESS_S1_TRANSFER_FIXTURE") else {
        return;
    };
    let index: usize = std::env::var("OMAVLESS_S1_TRANSFER_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let temp = Temp(PathBuf::from(path));
    let mut model = Model::open(&temp, binding()).unwrap();
    model.journal.storage.crash_at.set(Some(CHECKPOINTS[index]));
    model
        .transfer(successor(), model.scope.facts(), model.receipt(successor()))
        .unwrap();
    panic!("synthetic crash checkpoint not reached");
}

#[test]
fn process_exit_at_every_transfer_boundary_retains_only_compensation() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    for (index, checkpoint) in CHECKPOINTS.into_iter().enumerate() {
        let temp = Temp::new();
        let (planner, mut current, original) = partial(4, true);
        drop(Model::create(&temp, planner));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app_proxy::journal::fields::tests::transfer::transfer_crash_worker",
            ])
            .env("OMAVLESS_S1_TRANSFER_FIXTURE", &temp.0)
            .env("OMAVLESS_S1_TRANSFER_POINT", index.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("synthetic transfer worker exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(status.code(), Some(71));
        if matches!(
            checkpoint,
            Checkpoint::Created | Checkpoint::Written | Checkpoint::FileSynced
        ) {
            assert!(matches!(
                Model::open(&temp, binding()),
                Err(Error::Interrupted)
            ));
            assert!(matches!(
                Model::open(&temp, successor()),
                Err(Error::Interrupted)
            ));
        } else {
            assert!(matches!(
                Model::open(&temp, binding()),
                Err(Error::BindingMismatch)
            ));
            let mut model = Model::open(&temp, successor()).unwrap();
            assert_eq!(model.sequence, 2);
            assert!(matches!(
                model.journal.begin_next(successor(), &current),
                Err(Error::RecoveryRequired)
            ));
            let mut changed = current.clone();
            let field = Field::UserManager(crate::app_proxy::codec::EnvironmentKey::NoUpper);
            changed = changed
                .with(
                    field,
                    crate::app_proxy::fields::Value::UserManager(
                        crate::app_proxy::codec::EnvironmentEntry {
                            key: crate::app_proxy::codec::EnvironmentKey::NoUpper,
                            value: crate::app_proxy::codec::EnvironmentValue::Present(
                                "synthetic-foreign".into(),
                            ),
                        },
                    ),
                )
                .unwrap();
            let before = fs::read(temp.record()).unwrap();
            assert!(matches!(
                model.restore(successor(), &mut changed),
                Err(Error::Planner(crate::app_proxy::Error::ForeignChange))
            ));
            assert_eq!(fs::read(temp.record()).unwrap(), before);
            model.restore(successor(), &mut current).unwrap();
            assert_eq!(current, original);
        }
    }
}
