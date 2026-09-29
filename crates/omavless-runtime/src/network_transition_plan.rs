// SPDX-License-Identifier: MIT

//! Inactive T4 decision model for a suspend/network-change *hint*.
//! A Candidate is not permission to reconnect: the future owner must durably
//! fence the attempt and re-observe under its mutation lease before any effect.

/// A monotonic quiet interval for a burst of link/resume hints. The eventual
/// event adapter must supply monotonic ticks, not wall-clock seconds.
pub const QUIET_SECS: u64 = 3;
pub const MAX_HINT_AGE_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hint {
    pub owner_generation: u64,
    pub desired_revision: u64,
    pub network_epoch: u64,
    pub last_hint_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedState {
    /// The exact owned core/controller/TUN is locally verified. This is not
    /// evidence of functional DNS, routes, or external Internet.
    VerifiedLocal,
    /// All owned resources are proven absent; foreign resources are separate.
    ProvenEmpty,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    /// A future durable, exact-generation receipt has proven no prior attempt.
    NoneProven,
    StartedOrFailed,
    OutcomeUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Current {
    pub owner_generation: u64,
    pub desired_revision: u64,
    pub network_epoch: u64,
    pub now_tick: u64,
    pub desired_connected: bool,
    pub mutation_idle: bool,
    pub owned: OwnedState,
    pub attempt: Attempt,
    /// False if route, DNS, foreign-VPN, protection or owner evidence is
    /// incomplete for a recovery. It is not computed by this pure model.
    pub recovery_safety_proven: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Off,
    Stale,
    WaitUntil(u64),
    ObserveOnly,
    ManualRecovery,
    /// Advisory only. Never feed directly to a Connect or host adapter.
    CandidateOnce,
}

/// A conservative, side-effect-free admission *proposal* for one stable hint.
/// Missing or inconsistent facts fail closed; a later owner must implement a
/// durable one-attempt receipt and repeat all fences under its mutation lease.
#[must_use]
pub fn plan(hint: Hint, current: Current) -> Decision {
    if hint.owner_generation == 0
        || hint.network_epoch == 0
        || hint.owner_generation != current.owner_generation
        || hint.desired_revision != current.desired_revision
        || hint.network_epoch != current.network_epoch
    {
        return Decision::Stale;
    }
    let Some(age) = current.now_tick.checked_sub(hint.last_hint_tick) else {
        return Decision::Stale;
    };
    if age > MAX_HINT_AGE_SECS {
        return Decision::Stale;
    }
    if !current.desired_connected {
        return Decision::Off;
    }
    // No queued event may preempt a foreground mutation or become a later
    // implicit Connect after that mutation completes.
    if !current.mutation_idle {
        return Decision::Stale;
    }
    let Some(quiet_until) = hint.last_hint_tick.checked_add(QUIET_SECS) else {
        return Decision::Stale;
    };
    if current.now_tick < quiet_until {
        return Decision::WaitUntil(quiet_until);
    }
    match current.owned {
        OwnedState::VerifiedLocal => Decision::ObserveOnly,
        OwnedState::Unknown => Decision::ManualRecovery,
        OwnedState::ProvenEmpty
            if current.recovery_safety_proven && current.attempt == Attempt::NoneProven =>
        {
            Decision::CandidateOnce
        }
        OwnedState::ProvenEmpty => Decision::ManualRecovery,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        attempt: Attempt::NoneProven,
        recovery_safety_proven: true,
    };

    #[test]
    fn off_and_stale_hints_never_offer_a_recovery() {
        assert_eq!(
            plan(
                HINT,
                Current {
                    desired_connected: false,
                    ..CURRENT
                }
            ),
            Decision::Off
        );
        for current in [
            Current {
                owner_generation: 8,
                ..CURRENT
            },
            Current {
                desired_revision: 13,
                ..CURRENT
            },
            Current {
                network_epoch: 6,
                ..CURRENT
            },
            Current {
                now_tick: 99,
                ..CURRENT
            },
            Current {
                now_tick: 161,
                ..CURRENT
            },
            Current {
                mutation_idle: false,
                ..CURRENT
            },
        ] {
            assert_eq!(plan(HINT, current), Decision::Stale);
        }
        for hint in [
            Hint {
                owner_generation: 0,
                ..HINT
            },
            Hint {
                network_epoch: 0,
                ..HINT
            },
            Hint {
                last_hint_tick: u64::MAX,
                ..HINT
            },
        ] {
            assert_eq!(plan(hint, CURRENT), Decision::Stale);
        }
        assert_eq!(
            plan(
                Hint {
                    last_hint_tick: u64::MAX - 1,
                    ..HINT
                },
                Current {
                    now_tick: u64::MAX,
                    ..CURRENT
                }
            ),
            Decision::Stale
        );
    }

    #[test]
    fn burst_waits_on_monotonic_ticks_without_host_effects() {
        assert_eq!(
            plan(
                HINT,
                Current {
                    now_tick: 102,
                    ..CURRENT
                }
            ),
            Decision::WaitUntil(103)
        );
        let newer = Hint {
            last_hint_tick: 102,
            ..HINT
        };
        assert_eq!(plan(newer, CURRENT), Decision::WaitUntil(105));
        assert_eq!(
            plan(
                newer,
                Current {
                    now_tick: 105,
                    ..CURRENT
                }
            ),
            Decision::CandidateOnce
        );
    }

    #[test]
    fn healthy_is_observation_only_and_uncertainty_requires_a_person() {
        assert_eq!(
            plan(
                HINT,
                Current {
                    owned: OwnedState::VerifiedLocal,
                    ..CURRENT
                }
            ),
            Decision::ObserveOnly
        );
        for current in [
            Current {
                owned: OwnedState::Unknown,
                ..CURRENT
            },
            Current {
                recovery_safety_proven: false,
                ..CURRENT
            },
            Current {
                attempt: Attempt::StartedOrFailed,
                ..CURRENT
            },
            Current {
                attempt: Attempt::OutcomeUnknown,
                ..CURRENT
            },
        ] {
            assert_eq!(plan(HINT, current), Decision::ManualRecovery);
        }
        assert_eq!(plan(HINT, CURRENT), Decision::CandidateOnce);
    }
}
