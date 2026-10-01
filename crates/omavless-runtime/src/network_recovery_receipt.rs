// SPDX-License-Identifier: MIT

//! Test-only T4 receipt protocol. Storage and host observations below are models,
//! not production durability/provenance adapters. See the owning contract.
//! No automatic operation can manufacture a Ready record from absence or
//! re-arm a record after restart, a changed epoch, or an attempted recovery.

use crate::network_transition_plan::{self as hint_plan, Attempt, Current, Decision, Hint};
use serde::{Deserialize, Serialize};

#[path = "network_recovery_receipt_files.rs"]
mod file_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fence {
    boot: [u8; 16],
    owner_instance: [u8; 16],
    owner_generation: u64,
    desired_revision: u64,
    network_epoch: u64,
}

impl Fence {
    fn valid(self) -> bool {
        self.boot != [0; 16]
            && self.owner_instance != [0; 16]
            && self.owner_generation != 0
            && self.network_epoch != 0
    }

    fn matches(self, current: Current) -> bool {
        self.valid()
            && self.owner_generation == current.owner_generation
            && self.desired_revision == current.desired_revision
            && self.network_epoch == current.network_epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Ready,
    Reserved,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u8,
    fence: Fence,
    phase: Phase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Refused;

fn decode(raw: &[u8]) -> Result<Receipt, Refused> {
    if raw.len() > 1024 {
        return Err(Refused);
    }
    // Deserialize the struct directly: Value would silently erase duplicate keys.
    let receipt: Receipt = serde_json::from_slice(raw).map_err(|_| Refused)?;
    if receipt.schema != 1 || !receipt.fence.valid() {
        return Err(Refused);
    }
    Ok(receipt)
}

/// An implementation must hold one non-replaced mutation/ownership lease for
/// this whole exchange and provide durable compare-and-replace semantics.
/// Missing/unreadable/ambiguous state is an error. Successful load must establish
/// current durable state, not merely see an unsynced file from a previous writer.
/// A successful replacement must survive every modeled subsequent crash. An
/// error can mean either old or new state persisted, never permission to act.
/// The model does not prove that any existing filesystem adapter meets this.
trait Journal {
    fn load(&mut self) -> Result<Receipt, Refused>;
    fn replace_synced(&mut self, expected: Receipt, next: Receipt) -> Result<(), Refused>;
}

/// Only fresh, attributed observations qualify. The lease also serializes Off,
/// profile/mode changes, owner revocation and later network epochs. Production
/// kernel/event source and lease composition are intentionally absent.
trait Observation {
    fn current(&mut self) -> Result<(Fence, Current), Refused>;
    fn synthetic_effect(&mut self) -> Result<(), Refused>;
}

struct Admission {
    fence: Fence,
    poisoned: bool,
}

impl Admission {
    // Construct exactly once with the process's fresh owner instance. Recreating
    // this handle for each event would erase an in-memory uncertain-write latch;
    // the future owner adapter must make that impossible under its singleton.
    fn new(fence: Fence) -> Result<Self, Refused> {
        if !fence.valid() {
            return Err(Refused);
        }
        Ok(Self {
            fence,
            poisoned: false,
        })
    }

    fn attempt(
        &mut self,
        hint: Hint,
        journal: &mut impl Journal,
        host: &mut impl Observation,
    ) -> Result<(), Refused> {
        if self.poisoned {
            return Err(Refused);
        }
        // Any error or panic after this point invalidates this in-memory handle.
        // Even an uncertain reservation that left Ready must not be retried here.
        self.poisoned = true;
        let ready = journal.load()?;
        if ready.schema != 1 || ready.fence != self.fence || ready.phase != Phase::Ready {
            return Err(Refused);
        }
        self.check(hint, host)?;
        let reserved = Receipt {
            phase: Phase::Reserved,
            ..ready
        };
        journal.replace_synced(ready, reserved)?;
        if journal.load()? != reserved {
            return Err(Refused);
        }
        // Off or stale facts discovered after sync burn the receipt too. The
        // only safe effect site is after durable reservation AND a fresh fence.
        self.check(hint, host)?;
        host.synthetic_effect()?;
        // Completion never grants another attempt. A completion-write error or
        // crash keeps Reserved/Finished, both terminal for automatic recovery.
        journal.replace_synced(
            reserved,
            Receipt {
                phase: Phase::Finished,
                ..reserved
            },
        )?;
        Ok(())
    }

    fn check(&self, hint: Hint, host: &mut impl Observation) -> Result<(), Refused> {
        let (fence, mut current) = host.current()?;
        if fence != self.fence || !self.fence.matches(current) {
            return Err(Refused);
        }
        // Never accept caller-supplied NoneProven. It is derived only from the
        // Ready receipt at admission; after reservation this same invocation is
        // consuming that proof, not re-admitting another attempt.
        current.attempt = Attempt::NoneProven;
        if hint_plan::plan(hint, current) != Decision::CandidateOnce {
            return Err(Refused);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_transition_plan::OwnedState;

    const FENCE: Fence = Fence {
        boot: [1; 16],
        owner_instance: [2; 16],
        owner_generation: 7,
        desired_revision: 12,
        network_epoch: 5,
    };
    const HINT: Hint = Hint {
        owner_generation: 7,
        desired_revision: 12,
        network_epoch: 5,
        last_hint_tick: 100,
    };
    const CURRENT: Current = Current {
        owner_generation: 7,
        desired_revision: 12,
        network_epoch: 5,
        now_tick: 103,
        desired_connected: true,
        mutation_idle: true,
        owned: OwnedState::ProvenEmpty,
        attempt: Attempt::OutcomeUnknown,
        recovery_safety_proven: true,
    };
    const READY: Receipt = Receipt {
        schema: 1,
        fence: FENCE,
        phase: Phase::Ready,
    };

    #[derive(Clone, Copy)]
    enum Fault {
        None,
        Before(usize),
        After(usize),
    }

    struct Disk {
        // This is explicitly the crash-surviving model state, not an fs cache.
        durable: Option<Receipt>,
        fault: Fault,
        calls: usize,
    }
    impl Disk {
        fn seeded() -> Self {
            // Test fixture only. No product initializer for Ready is provided.
            Self {
                durable: Some(READY),
                fault: Fault::None,
                calls: 0,
            }
        }
        fn before(&mut self) -> Result<(), Refused> {
            self.calls += 1;
            if matches!(self.fault, Fault::Before(n) if n == self.calls) {
                return Err(Refused);
            }
            Ok(())
        }
        fn after(&self) -> Result<(), Refused> {
            if matches!(self.fault, Fault::After(n) if n == self.calls) {
                return Err(Refused);
            }
            Ok(())
        }
    }
    impl Journal for Disk {
        fn load(&mut self) -> Result<Receipt, Refused> {
            self.before()?;
            self.after()?;
            self.durable.ok_or(Refused)
        }
        fn replace_synced(&mut self, expected: Receipt, next: Receipt) -> Result<(), Refused> {
            self.before()?;
            if self.durable != Some(expected) {
                return Err(Refused);
            }
            self.durable = Some(next);
            self.after()
        }
    }
    struct Host {
        facts: (Fence, Current),
        after_first: Option<(Fence, Current)>,
        reads: usize,
        effects: usize,
        effect_fails: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                facts: (FENCE, CURRENT),
                after_first: None,
                reads: 0,
                effects: 0,
                effect_fails: false,
            }
        }
    }
    impl Observation for Host {
        fn current(&mut self) -> Result<(Fence, Current), Refused> {
            self.reads += 1;
            if self.reads > 1 {
                Ok(self.after_first.unwrap_or(self.facts))
            } else {
                Ok(self.facts)
            }
        }
        fn synthetic_effect(&mut self) -> Result<(), Refused> {
            self.effects += 1;
            if self.effect_fails {
                Err(Refused)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn successful_receipt_and_duplicate_hint_never_retry() {
        let mut disk = Disk::seeded();
        let mut host = Host::new();
        let mut admission = Admission::new(FENCE).unwrap();
        assert_eq!(admission.attempt(HINT, &mut disk, &mut host), Ok(()));
        assert_eq!(disk.durable.unwrap().phase, Phase::Finished);
        assert_eq!(host.effects, 1);
        assert_eq!(admission.attempt(HINT, &mut disk, &mut host), Err(Refused));
        assert_eq!(
            Admission::new(FENCE)
                .unwrap()
                .attempt(HINT, &mut disk, &mut host),
            Err(Refused)
        );
        assert_eq!(host.effects, 1);
    }

    #[test]
    fn absent_lost_reserved_or_finished_is_never_no_attempt() {
        for receipt in [
            None,
            Some(Receipt {
                phase: Phase::Reserved,
                ..READY
            }),
            Some(Receipt {
                phase: Phase::Finished,
                ..READY
            }),
        ] {
            let mut disk = Disk {
                durable: receipt,
                ..Disk::seeded()
            };
            let mut host = Host::new();
            assert_eq!(
                Admission::new(FENCE)
                    .unwrap()
                    .attempt(HINT, &mut disk, &mut host),
                Err(Refused)
            );
            assert_eq!(host.effects, 0);
            assert_eq!(disk.durable, receipt);
        }
        // Loss after an effect is indistinguishable from never initialized.
        let mut disk = Disk::seeded();
        let mut host = Host::new();
        Admission::new(FENCE)
            .unwrap()
            .attempt(HINT, &mut disk, &mut host)
            .unwrap();
        disk.durable = None;
        assert_eq!(
            Admission::new(FENCE)
                .unwrap()
                .attempt(HINT, &mut disk, &mut host),
            Err(Refused)
        );
        assert_eq!(host.effects, 1);
    }

    #[test]
    fn every_io_uncertainty_stops_before_effect_or_burns_reservation() {
        // Four journal boundaries: read Ready, reserve+sync, readback, finish+sync.
        for n in 1..=4 {
            for fault in [Fault::Before(n), Fault::After(n)] {
                let mut disk = Disk {
                    fault,
                    ..Disk::seeded()
                };
                let mut host = Host::new();
                let mut admission = Admission::new(FENCE).unwrap();
                assert_eq!(admission.attempt(HINT, &mut disk, &mut host), Err(Refused));
                assert_eq!(host.effects, usize::from(n == 4));
                if host.effects != 0 {
                    assert_ne!(disk.durable.unwrap().phase, Phase::Ready);
                }
                disk.fault = Fault::None;
                assert_eq!(admission.attempt(HINT, &mut disk, &mut host), Err(Refused));
                // A real owner restart must create a fresh instance, even in the
                // same boot. Old Ready from a failed reservation cannot rearm it.
                let restarted = Fence {
                    owner_instance: [3; 16],
                    ..FENCE
                };
                host.facts.0 = restarted;
                assert_eq!(
                    Admission::new(restarted)
                        .unwrap()
                        .attempt(HINT, &mut disk, &mut host),
                    Err(Refused)
                );
                assert_eq!(host.effects, usize::from(n == 4));
            }
        }
    }

    #[test]
    fn failed_effect_remains_reserved_even_for_later_epoch() {
        let mut disk = Disk::seeded();
        let mut host = Host {
            effect_fails: true,
            ..Host::new()
        };
        assert_eq!(
            Admission::new(FENCE)
                .unwrap()
                .attempt(HINT, &mut disk, &mut host),
            Err(Refused)
        );
        assert_eq!(disk.durable.unwrap().phase, Phase::Reserved);
        let next = Fence {
            network_epoch: 6,
            ..FENCE
        };
        host.facts = (
            next,
            Current {
                network_epoch: 6,
                ..CURRENT
            },
        );
        let hint = Hint {
            network_epoch: 6,
            ..HINT
        };
        assert_eq!(
            Admission::new(next)
                .unwrap()
                .attempt(hint, &mut disk, &mut host),
            Err(Refused)
        );
        assert_eq!(host.effects, 1);
    }

    #[test]
    fn every_identity_change_before_and_after_sync_refuses() {
        for fence in [
            Fence {
                boot: [4; 16],
                ..FENCE
            },
            Fence {
                owner_instance: [4; 16],
                ..FENCE
            },
            Fence {
                owner_generation: 8,
                ..FENCE
            },
            Fence {
                desired_revision: 13,
                ..FENCE
            },
            Fence {
                network_epoch: 6,
                ..FENCE
            },
        ] {
            for after_sync in [false, true] {
                let mut disk = Disk::seeded();
                let mut host = Host::new();
                if after_sync {
                    host.after_first = Some((fence, CURRENT));
                } else {
                    host.facts.0 = fence;
                }
                assert_eq!(
                    Admission::new(FENCE)
                        .unwrap()
                        .attempt(HINT, &mut disk, &mut host),
                    Err(Refused)
                );
                assert_eq!(host.effects, 0);
                assert_eq!(
                    disk.durable.unwrap().phase,
                    if after_sync {
                        Phase::Reserved
                    } else {
                        Phase::Ready
                    }
                );
            }
        }
    }

    #[test]
    fn off_stale_and_uncertain_observations_cannot_spend_reservation() {
        for current in [
            Current {
                desired_connected: false,
                ..CURRENT
            },
            Current {
                mutation_idle: false,
                ..CURRENT
            },
            Current {
                owned: OwnedState::Unknown,
                ..CURRENT
            },
            Current {
                owned: OwnedState::VerifiedLocal,
                ..CURRENT
            },
            Current {
                recovery_safety_proven: false,
                ..CURRENT
            },
            Current {
                now_tick: 102,
                ..CURRENT
            },
            Current {
                now_tick: 161,
                ..CURRENT
            },
        ] {
            let mut disk = Disk::seeded();
            let mut host = Host {
                after_first: Some((FENCE, current)),
                ..Host::new()
            };
            assert_eq!(
                Admission::new(FENCE)
                    .unwrap()
                    .attempt(HINT, &mut disk, &mut host),
                Err(Refused)
            );
            assert_eq!(host.effects, 0);
            assert_eq!(disk.durable.unwrap().phase, Phase::Reserved);
        }
    }

    #[test]
    fn receipt_decoder_rejects_ambiguous_or_unversioned_state() {
        let raw = serde_json::to_vec(&READY).unwrap();
        assert_eq!(decode(&raw), Ok(READY));
        for raw in [
            Vec::new(),
            b"null".to_vec(),
            b"{}".to_vec(),
            vec![b' '; 1025],
            serde_json::to_vec(&Receipt { schema: 2, ..READY }).unwrap(),
            serde_json::to_vec(&Receipt {
                fence: Fence {
                    boot: [0; 16],
                    ..FENCE
                },
                ..READY
            })
            .unwrap(),
            String::from_utf8(raw.clone())
                .unwrap()
                .replacen("\"schema\":1", "\"schema\":1,\"schema\":1", 1)
                .into_bytes(),
            String::from_utf8(raw.clone())
                .unwrap()
                .replacen(
                    "\"network_epoch\":5",
                    "\"network_epoch\":5,\"network_epoch\":5",
                    1,
                )
                .into_bytes(),
            String::from_utf8(raw)
                .unwrap()
                .replacen("\"schema\":1", "\"schema\":1,\"extra\":0", 1)
                .into_bytes(),
        ] {
            assert_eq!(decode(&raw), Err(Refused));
        }
    }
}
