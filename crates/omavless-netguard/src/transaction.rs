//! Serial, acknowledged helper transaction model. It never decides runtime
//! desired/core state: the runtime must persist disconnected and prove owned
//! core cleanup before requesting disarm. Peers cannot supply observations.
use crate::policy::Policy;
use crate::protocol::{ErrorCode, Health, Mode, POLICY_VERSION, Protection, Request, Response};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Marker {
    Missing,
    Armed(u64),
    /// Includes corrupt, unsupported, unsafe or unreadable existing state.
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Table {
    Absent,
    Verified(Policy),
    /// Includes missing observation and an unexpected owned-table shape.
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    pub marker: Marker,
    pub table: Table,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    InstallAtomic(Policy),
    VerifyTable(Policy),
    PersistArmedDurably(u64),
    RemoveMarkerDurably,
    DeleteOwnedTableAtomic,
    VerifyTableAbsent,
}

#[derive(Debug)]
pub struct Transaction {
    steps: Vec<Effect>,
    next: usize,
    completion: Response,
    failed: bool,
}

impl Transaction {
    fn new(steps: Vec<Effect>, completion: Response) -> Self {
        Self {
            steps,
            next: 0,
            completion,
            failed: false,
        }
    }

    pub fn next_effect(&self) -> Option<Effect> {
        if self.failed {
            None
        } else {
            self.steps.get(self.next).copied()
        }
    }

    /// A future adapter may acknowledge only after executing AND verifying the
    /// exact effect under its exclusive lock. A wrong/failed acknowledgement
    /// poisons this transaction, including uncertain write outcomes.
    pub fn acknowledge(&mut self, effect: Effect, success: bool) -> Result<(), ErrorCode> {
        if !success || self.next_effect() != Some(effect) {
            self.failed = true;
            return Err(ErrorCode::ManualRecoveryRequired);
        }
        self.next += 1;
        Ok(())
    }

    pub fn response(&self) -> Result<Response, ErrorCode> {
        if self.failed || self.next != self.steps.len() {
            Err(ErrorCode::ManualRecoveryRequired)
        } else {
            Ok(self.completion)
        }
    }
}

fn status(protection: Protection, health: Health) -> Response {
    Response::Status {
        policy_version: POLICY_VERSION,
        protection,
        health,
    }
}

pub fn observe(observation: Observation) -> Response {
    match (observation.marker, observation.table) {
        (Marker::Missing, Table::Absent) => status(Protection::Disarmed {}, Health::Verified),
        (Marker::Armed(generation), Table::Verified(Policy::FullVpn)) => {
            status(Protection::Armed { generation }, Health::Verified)
        }
        (Marker::Invalid, Table::Verified(Policy::Emergency)) => {
            status(Protection::Emergency {}, Health::ManualRecoveryRequired)
        }
        _ => Response::Error {
            code: ErrorCode::ManualRecoveryRequired,
        },
    }
}

/// This pure function is not peer authentication. Production must derive UID
/// from SO_PEERCRED, validate enrollment, and serialize before calling it.
pub fn plan(request: Request, observation: Observation) -> Result<Transaction, ErrorCode> {
    if request == (Request::Status {}) {
        return Ok(Transaction::new(vec![], observe(observation)));
    }
    if observation.marker == Marker::Invalid {
        return Err(ErrorCode::ManualRecoveryRequired);
    }
    match request {
        Request::Arm {
            generation,
            mode: Mode::Full,
        } => {
            if matches!(observation.marker, Marker::Armed(current) if current != generation) {
                return Err(ErrorCode::GenerationConflict);
            }
            // Reinstall/verify even an idempotent retry: cached success is not
            // fresh evidence that a root-owned table still exists.
            Ok(Transaction::new(
                vec![
                    Effect::InstallAtomic(Policy::FullVpn),
                    Effect::VerifyTable(Policy::FullVpn),
                    Effect::PersistArmedDurably(generation),
                ],
                status(Protection::Armed { generation }, Health::Verified),
            ))
        }
        Request::Disarm { generation } => {
            if observation.marker != Marker::Armed(generation) {
                // Lost disarm acknowledgement is safe to retry only if both
                // marker and owned table are already absent.
                if observation
                    == (Observation {
                        marker: Marker::Missing,
                        table: Table::Absent,
                    })
                {
                    return Ok(Transaction::new(vec![], observe(observation)));
                }
                return Err(ErrorCode::GenerationConflict);
            }
            Ok(disarm())
        }
        Request::Status {} => unreachable!(),
    }
}

fn disarm() -> Transaction {
    Transaction::new(
        vec![
            Effect::RemoveMarkerDurably,
            Effect::DeleteOwnedTableAtomic,
            Effect::VerifyTableAbsent,
        ],
        status(Protection::Disarmed {}, Health::Verified),
    )
}

/// Root/internal startup only; deliberately absent from the wire request enum.
/// The helper lock must cover snapshot, reconciliation and final observation.
pub fn reconcile(marker: Marker) -> Transaction {
    match marker {
        Marker::Missing => disarm(),
        Marker::Armed(generation) => Transaction::new(
            vec![
                Effect::InstallAtomic(Policy::FullVpn),
                Effect::VerifyTable(Policy::FullVpn),
            ],
            status(Protection::Armed { generation }, Health::Verified),
        ),
        Marker::Invalid => Transaction::new(
            vec![
                Effect::InstallAtomic(Policy::Emergency),
                Effect::VerifyTable(Policy::Emergency),
            ],
            status(Protection::Emergency {}, Health::ManualRecoveryRequired),
        ),
    }
}
