// SPDX-License-Identifier: MIT
//! Pure counterexamples, not a systemd writer, drain API or admission proof.
//! The pinned systemd Environment property merges transient and client layers;
//! Set/UnsetEnvironment modify only the client layer. No host bus is accessed.
use super::*;
use crate::app_proxy::codec::{EnvironmentEntry, EnvironmentKey, EnvironmentValue};
use crate::app_proxy::fields::Value;

struct LayeredManager {
    desktop: State,
    transient: [EnvironmentValue; 10],
    client: [EnvironmentValue; 10],
    effects: usize,
}

fn index(key: EnvironmentKey) -> usize {
    EnvironmentKey::ALL
        .iter()
        .position(|item| *item == key)
        .unwrap()
}

fn present(value: &str) -> EnvironmentValue {
    EnvironmentValue::Present(value.into())
}

impl LayeredManager {
    fn new(key: EnvironmentKey, transient: EnvironmentValue, client: EnvironmentValue) -> Self {
        let mut host = Self {
            desktop: states().0,
            transient: std::array::from_fn(|_| EnvironmentValue::Absent),
            client: std::array::from_fn(|_| EnvironmentValue::Absent),
            effects: 0,
        };
        host.transient[index(key)] = transient;
        host.client[index(key)] = client;
        host
    }

    fn effective(&self, key: EnvironmentKey) -> EnvironmentValue {
        match &self.client[index(key)] {
            EnvironmentValue::Absent => self.transient[index(key)].clone(),
            value => value.clone(),
        }
    }

    fn snapshot(&self) -> State {
        let mut state = self.desktop.clone();
        for key in EnvironmentKey::ALL {
            state = state
                .with(
                    Field::UserManager(key),
                    Value::UserManager(EnvironmentEntry {
                        key,
                        value: self.effective(key),
                    }),
                )
                .unwrap();
        }
        state
    }
}

impl FixedHost for LayeredManager {
    fn observe(&mut self, _: Binding) -> Result<State, HostError> {
        Ok(self.snapshot())
    }

    fn write(&mut self, _: Binding, effect: &Effect) -> Result<(), HostError> {
        let (Field::UserManager(key), Value::UserManager(entry)) =
            (effect.field, &effect.replacement)
        else {
            panic!("counterexample must never write desktop fields");
        };
        assert!(entry.key == key);
        // Model SetEnvironment for Present (including empty), UnsetEnvironment
        // for Absent. This changes a test array, never a manager or process env.
        self.client[index(key)] = entry.value.clone();
        self.effects += 1;
        Ok(())
    }

    fn drain(&mut self, _: Binding) -> Result<(), HostError> {
        // Only this model's synchronous array assignment is already complete.
        // This deliberately says nothing about real D-Bus request ordering.
        Ok(())
    }
}

fn transaction(
    temp: &Temp,
    host: LayeredManager,
    key: EnvironmentKey,
) -> Transaction<LayeredManager> {
    let original = host.snapshot();
    let intended = original
        .with(
            Field::UserManager(key),
            Value::UserManager(EnvironmentEntry {
                key,
                value: present("synthetic-owned-proxy"),
            }),
        )
        .unwrap();
    let journal = FieldJournal::create(&temp.0, binding(), original, intended).unwrap();
    Transaction::new(journal, host, binding()).unwrap()
}

fn apply(txn: &mut Transaction<LayeredManager>) {
    assert_eq!(txn.step(), Ok(Phase::Applying));
    assert_eq!(txn.step(), Ok(Phase::Active));
    assert_eq!(txn.host.effects, 1);
}

fn restore(txn: &mut Transaction<LayeredManager>) {
    txn.begin_restore().unwrap();
    assert_eq!(txn.step(), Ok(Phase::Restoring));
    assert_eq!(txn.step(), Ok(Phase::Released));
    assert_eq!(txn.host.effects, 2);
}

#[test]
fn effective_aliases_restore_equal_values_but_lose_original_client_absence() {
    for key in EnvironmentKey::ALL {
        for saved in ["synthetic-prior-proxy", ""] {
            let transient_only = LayeredManager::new(key, present(saved), EnvironmentValue::Absent);
            let client_only = LayeredManager::new(key, EnvironmentValue::Absent, present(saved));
            assert!(transient_only.snapshot() == client_only.snapshot());
            let original = transient_only.snapshot();
            let temp_a = Temp::new();
            let temp_b = Temp::new();
            let mut a = transaction(&temp_a, transient_only, key);
            let mut b = transaction(&temp_b, client_only, key);
            apply(&mut a);
            apply(&mut b);
            restore(&mut a);
            restore(&mut b);
            assert!(a.host.snapshot() == original && b.host.snapshot() == original);
            assert!(a.host.client[index(key)] == present(saved));
            // Counterexample PASS: effective Released is NOT layer restoration.
            assert!(!matches!(
                a.host.client[index(key)],
                EnvironmentValue::Absent
            ));
            a.host.transient[index(key)] = present("synthetic-later-generator-value");
            assert!(a.host.effective(key) == present(saved));
            assert!(a.host.effective(key) != a.host.transient[index(key)]);
        }
    }
}

#[test]
fn known_original_client_override_is_restored_in_the_model() {
    for key in EnvironmentKey::ALL {
        for saved in ["synthetic-client-override", ""] {
            let temp = Temp::new();
            let host = LayeredManager::new(key, present("synthetic-transient"), present(saved));
            let mut txn = transaction(&temp, host, key);
            apply(&mut txn);
            restore(&mut txn);
            assert!(txn.host.client[index(key)] == present(saved));
            assert!(txn.host.transient[index(key)] == present("synthetic-transient"));
        }
    }
}

#[test]
fn initially_effective_absent_restores_both_absent_only_with_unchanged_layers() {
    for key in EnvironmentKey::ALL {
        let temp = Temp::new();
        let host = LayeredManager::new(key, EnvironmentValue::Absent, EnvironmentValue::Absent);
        let mut txn = transaction(&temp, host, key);
        apply(&mut txn);
        restore(&mut txn);
        assert!(matches!(
            txn.host.client[index(key)],
            EnvironmentValue::Absent
        ));
        assert!(matches!(
            txn.host.transient[index(key)],
            EnvironmentValue::Absent
        ));
        assert!(matches!(txn.host.effective(key), EnvironmentValue::Absent));
    }
}

#[test]
fn masked_transient_drift_prevents_released_confirmation_after_unset() {
    for key in EnvironmentKey::ALL {
        let temp = Temp::new();
        let host = LayeredManager::new(key, EnvironmentValue::Absent, EnvironmentValue::Absent);
        let mut txn = transaction(&temp, host, key);
        apply(&mut txn);
        let active = txn.host.snapshot();
        txn.host.transient[index(key)] = present("synthetic-foreign-transient");
        assert!(txn.host.snapshot() == active);
        txn.begin_restore().unwrap();
        assert!(matches!(
            txn.step(),
            Err(Error::Journal(JournalError::Planner(
                PlannerError::UnconfirmedEffect
            )))
        ));
        assert_eq!(txn.phase(), Phase::Restoring);
        assert!(txn.host.effective(key) == present("synthetic-foreign-transient"));
        let pending = temp.record();
        assert!(txn.step().is_err());
        assert!(temp.record() == pending);
    }
}

#[test]
fn same_value_aba_is_indistinguishable_from_unchanged_observation() {
    for key in EnvironmentKey::ALL {
        let temp = Temp::new();
        let host = LayeredManager::new(key, EnvironmentValue::Absent, present("synthetic-prior"));
        let mut txn = transaction(&temp, host, key);
        apply(&mut txn);
        let active = txn.host.snapshot();
        let record = temp.record();
        txn.host.client[index(key)] = present("synthetic-foreign-intermediate");
        txn.host.client[index(key)] = present("synthetic-owned-proxy");
        assert!(txn.host.snapshot() == active && temp.record() == record);
        restore(&mut txn);
        assert!(txn.host.client[index(key)] == present("synthetic-prior"));
        // Acceptance by the pure comparator characterizes its information loss,
        // never exclusive authority over an external same-value edit.
    }
}
