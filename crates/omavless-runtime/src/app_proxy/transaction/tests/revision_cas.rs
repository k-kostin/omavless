// SPDX-License-Identifier: MIT
//! Hypothetical atomic protocol, exclusively compiled as tests. Not systemd IPC.
use crate::app_proxy::codec::{EnvironmentKey, EnvironmentValue};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    owner: u64,
    incarnation: u64,
    revision: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct Snapshot {
    stamp: Stamp,
    transient: [EnvironmentValue; 10],
    client: [EnvironmentValue; 10],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Snapshot,
    Compare,
    Commit,
    Conflict,
    Unknown,
}

#[derive(Clone, Copy)]
enum Delivery {
    Known,
    LostBefore,
    LostAfter,
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Conflict,
    Unknown,
    Exhausted,
    Sealed,
    WrongPhase,
}

struct FakeManager {
    state: Snapshot,
    transcript: Vec<Event>,
}

fn present(value: &str) -> EnvironmentValue {
    EnvironmentValue::Present(value.into())
}

fn index(key: EnvironmentKey) -> usize {
    EnvironmentKey::ALL.iter().position(|k| *k == key).unwrap()
}

impl FakeManager {
    fn new(key: EnvironmentKey, client: EnvironmentValue) -> Self {
        let mut state = Snapshot {
            stamp: Stamp {
                owner: 1,
                incarnation: 1,
                revision: 0,
            },
            transient: std::array::from_fn(|_| present("synthetic-transient")),
            client: std::array::from_fn(|_| EnvironmentValue::Absent),
        };
        state.client[index(key)] = client;
        Self {
            state,
            transcript: Vec::new(),
        }
    }

    fn snapshot(&mut self) -> Snapshot {
        self.transcript.push(Event::Snapshot);
        self.state.clone()
    }

    fn advance(&mut self) -> Result<(), Refusal> {
        // Exhaustion refuses before any mutation, never wraps into an ABA token.
        self.state.stamp.revision = self
            .state
            .stamp
            .revision
            .checked_add(1)
            .ok_or(Refusal::Exhausted)?;
        Ok(())
    }

    fn foreign(&mut self, key: EnvironmentKey, transient: bool, value: EnvironmentValue) {
        self.advance().unwrap();
        if transient {
            self.state.transient[index(key)] = value;
        } else {
            self.state.client[index(key)] = value;
        }
    }

    fn effective(&self, key: EnvironmentKey) -> &EnvironmentValue {
        match &self.state.client[index(key)] {
            EnvironmentValue::Absent => &self.state.transient[index(key)],
            value => value,
        }
    }

    fn compare_exchange(
        &mut self,
        expected: &Snapshot,
        key: EnvironmentKey,
        replacement: EnvironmentValue,
        delivery: Delivery,
    ) -> Result<Snapshot, Refusal> {
        self.transcript.push(Event::Compare);
        if matches!(delivery, Delivery::LostBefore) {
            self.transcript.push(Event::Unknown);
            return Err(Refusal::Unknown);
        }
        // One indivisible fake-manager step: no read/write gap. This stronger
        // full-layer comparison does not substitute for the revision counter.
        if &self.state != expected {
            self.transcript.push(Event::Conflict);
            return Err(Refusal::Conflict);
        }
        self.advance()?;
        self.state.client[index(key)] = replacement;
        self.transcript.push(Event::Commit);
        if matches!(delivery, Delivery::LostAfter) {
            self.transcript.push(Event::Unknown);
            return Err(Refusal::Unknown);
        }
        Ok(self.state.clone())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Captured,
    Active,
    Released,
    Sealed,
}

struct Protocol {
    original: Snapshot,
    expected: Snapshot,
    key: EnvironmentKey,
    phase: Phase,
}

impl Protocol {
    fn capture(manager: &mut FakeManager, key: EnvironmentKey) -> Self {
        let original = manager.snapshot();
        Self {
            expected: original.clone(),
            original,
            key,
            phase: Phase::Captured,
        }
    }

    fn step(&mut self, manager: &mut FakeManager, delivery: Delivery) -> Result<(), Refusal> {
        let (replacement, next) = match self.phase {
            Phase::Captured => (present("synthetic-owned"), Phase::Active),
            Phase::Active => (
                self.original.client[index(self.key)].clone(),
                Phase::Released,
            ),
            Phase::Sealed => return Err(Refusal::Sealed),
            Phase::Released => return Err(Refusal::WrongPhase),
        };
        // Seal BEFORE the model operation: any missing/failed completion cannot
        // trigger another read, retry, restore, drain or optimistic release.
        self.phase = Phase::Sealed;
        let receipt = manager.compare_exchange(&self.expected, self.key, replacement, delivery)?;
        self.expected = receipt;
        self.phase = next;
        Ok(())
    }
}

fn assert_terminal(protocol: &mut Protocol, manager: &mut FakeManager) {
    let transcript = manager.transcript.clone();
    let state = manager.state.clone();
    assert_eq!(protocol.phase, Phase::Sealed);
    for _ in 0..3 {
        assert_eq!(
            protocol.step(manager, Delivery::Known),
            Err(Refusal::Sealed)
        );
    }
    assert_eq!(manager.transcript, transcript);
    assert!(manager.state == state);
}

#[test]
fn exact_client_absent_empty_and_value_restored_without_transient_override() {
    for key in EnvironmentKey::ALL {
        for client in [
            EnvironmentValue::Absent,
            present(""),
            present("synthetic-prior"),
        ] {
            let mut manager = FakeManager::new(key, client);
            let original = manager.state.clone();
            let mut protocol = Protocol::capture(&mut manager, key);
            protocol.step(&mut manager, Delivery::Known).unwrap();
            protocol.step(&mut manager, Delivery::Known).unwrap();
            assert_eq!(protocol.phase, Phase::Released);
            assert!(manager.state.client == original.client);
            assert!(manager.state.transient == original.transient);
            assert_eq!(manager.state.stamp.revision, 2);
            assert_eq!(
                manager.transcript,
                [
                    Event::Snapshot,
                    Event::Compare,
                    Event::Commit,
                    Event::Compare,
                    Event::Commit
                ]
            );
            let transcript = manager.transcript.clone();
            assert_eq!(
                protocol.step(&mut manager, Delivery::Known),
                Err(Refusal::WrongPhase)
            );
            assert_eq!(manager.transcript, transcript);
        }
    }
}

#[test]
fn owner_incarnation_and_revision_drift_refuse_before_mutation_at_both_phases() {
    for key in EnvironmentKey::ALL {
        for active in [false, true] {
            for dimension in 0..3 {
                let mut manager = FakeManager::new(key, EnvironmentValue::Absent);
                let mut protocol = Protocol::capture(&mut manager, key);
                if active {
                    protocol.step(&mut manager, Delivery::Known).unwrap();
                }
                match dimension {
                    0 => manager.state.stamp.owner += 1,
                    1 => manager.state.stamp.incarnation += 1,
                    _ => manager.advance().unwrap(),
                }
                let before = manager.state.clone();
                assert_eq!(
                    protocol.step(&mut manager, Delivery::Known),
                    Err(Refusal::Conflict)
                );
                assert!(manager.state == before);
                assert_terminal(&mut protocol, &mut manager);
            }
        }
    }
}

#[test]
fn masked_transient_edit_is_not_hidden_from_revision_cas() {
    for key in EnvironmentKey::ALL {
        let mut manager = FakeManager::new(key, EnvironmentValue::Absent);
        let mut protocol = Protocol::capture(&mut manager, key);
        protocol.step(&mut manager, Delivery::Known).unwrap();
        let effective = manager.effective(key).clone();
        manager.foreign(key, true, present("synthetic-foreign-transient"));
        assert!(*manager.effective(key) == effective);
        assert_eq!(
            protocol.step(&mut manager, Delivery::Known),
            Err(Refusal::Conflict)
        );
        assert_terminal(&mut protocol, &mut manager);
    }
}

#[test]
fn same_value_and_aba_edits_of_either_layer_are_conflicts() {
    for key in EnvironmentKey::ALL {
        for transient in [false, true] {
            for aba in [false, true] {
                let mut manager = FakeManager::new(key, present("synthetic-prior"));
                let mut protocol = Protocol::capture(&mut manager, key);
                protocol.step(&mut manager, Delivery::Known).unwrap();
                let value = if transient {
                    manager.state.transient[index(key)].clone()
                } else {
                    manager.state.client[index(key)].clone()
                };
                if aba {
                    manager.foreign(key, transient, present("synthetic-intermediate"));
                }
                manager.foreign(key, transient, value);
                assert!(manager.state.client == protocol.expected.client);
                assert!(manager.state.transient == protocol.expected.transient);
                assert_eq!(
                    protocol.step(&mut manager, Delivery::Known),
                    Err(Refusal::Conflict)
                );
                assert_terminal(&mut protocol, &mut manager);
            }
        }
    }
}

#[test]
fn unknown_before_or_after_commit_never_retries_or_restores() {
    for key in EnvironmentKey::ALL {
        for active in [false, true] {
            for delivery in [Delivery::LostBefore, Delivery::LostAfter] {
                let mut manager = FakeManager::new(key, EnvironmentValue::Absent);
                let mut protocol = Protocol::capture(&mut manager, key);
                if active {
                    protocol.step(&mut manager, Delivery::Known).unwrap();
                }
                let revision = manager.state.stamp.revision;
                assert_eq!(protocol.step(&mut manager, delivery), Err(Refusal::Unknown));
                assert_eq!(
                    manager.state.stamp.revision,
                    revision + u64::from(matches!(delivery, Delivery::LostAfter))
                );
                assert_terminal(&mut protocol, &mut manager);
            }
        }
    }
}

#[test]
fn revision_exhaustion_is_terminal_without_wrapping_or_mutation() {
    let mut manager = FakeManager::new(EnvironmentKey::ALL[0], EnvironmentValue::Absent);
    manager.state.stamp.revision = u64::MAX;
    let mut protocol = Protocol::capture(&mut manager, EnvironmentKey::ALL[0]);
    let before = manager.state.clone();
    assert_eq!(
        protocol.step(&mut manager, Delivery::Known),
        Err(Refusal::Exhausted)
    );
    assert!(manager.state == before);
    assert_terminal(&mut protocol, &mut manager);
}

#[test]
fn apply_at_last_revision_then_restore_exhaustion_retains_owned_value_and_seals() {
    for key in EnvironmentKey::ALL {
        for client in [
            EnvironmentValue::Absent,
            present(""),
            present("synthetic-prior"),
        ] {
            let mut manager = FakeManager::new(key, client);
            manager.state.stamp.revision = u64::MAX - 1;
            let mut protocol = Protocol::capture(&mut manager, key);
            let original = protocol.original.clone();
            protocol.step(&mut manager, Delivery::Known).unwrap();
            assert_eq!(protocol.phase, Phase::Active);
            assert_eq!(manager.state.stamp.revision, u64::MAX);
            assert!(manager.state.client[index(key)] == present("synthetic-owned"));
            let owned = manager.state.clone();
            let acknowledged = protocol.expected.clone();
            assert_eq!(
                protocol.step(&mut manager, Delivery::Known),
                Err(Refusal::Exhausted)
            );
            assert!(manager.state == owned);
            assert!(protocol.expected == acknowledged && protocol.original == original);
            assert_eq!(
                manager.transcript,
                [
                    Event::Snapshot,
                    Event::Compare,
                    Event::Commit,
                    Event::Compare
                ]
            );
            assert_terminal(&mut protocol, &mut manager);
        }
    }
}

#[test]
fn other_key_drift_or_aba_in_either_layer_blocks_apply_and_restore_without_mutation() {
    for key in EnvironmentKey::ALL {
        for other in EnvironmentKey::ALL {
            if key == other {
                continue;
            }
            for active in [false, true] {
                for transient in [false, true] {
                    for aba in [false, true] {
                        let mut manager = FakeManager::new(key, present("synthetic-prior"));
                        let mut protocol = Protocol::capture(&mut manager, key);
                        if active {
                            protocol.step(&mut manager, Delivery::Known).unwrap();
                        }
                        let acknowledged = protocol.expected.clone();
                        let original = protocol.original.clone();
                        let other_before = if transient {
                            manager.state.transient[index(other)].clone()
                        } else {
                            manager.state.client[index(other)].clone()
                        };
                        manager.foreign(other, transient, present("synthetic-other-key-edit"));
                        if aba {
                            manager.foreign(other, transient, other_before);
                            assert!(manager.state.client == acknowledged.client);
                            assert!(manager.state.transient == acknowledged.transient);
                        }
                        assert!(
                            manager.state.client[index(key)] == acknowledged.client[index(key)]
                        );
                        assert!(
                            manager.state.transient[index(key)]
                                == acknowledged.transient[index(key)]
                        );
                        let foreign = manager.state.clone();
                        let mut expected_events = manager.transcript.clone();
                        expected_events.extend([Event::Compare, Event::Conflict]);
                        assert_eq!(
                            protocol.step(&mut manager, Delivery::Known),
                            Err(Refusal::Conflict)
                        );
                        assert!(manager.state == foreign);
                        assert!(protocol.expected == acknowledged && protocol.original == original);
                        assert_eq!(manager.transcript, expected_events);
                        assert_terminal(&mut protocol, &mut manager);
                    }
                }
            }
        }
    }
}
