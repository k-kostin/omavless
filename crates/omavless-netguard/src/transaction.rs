//! Serial, acknowledged helper transaction model. It never decides runtime
//! desired/core state: the runtime must persist disconnected and prove owned
//! core cleanup before requesting disarm. Peers cannot supply observations.
use crate::policy::Policy;
use crate::protocol::{ErrorCode, Health, Mode, POLICY_VERSION, Protection, Request, Response};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Marker {
    /// Fresh installation only. A completed disarm must retain Closed instead.
    Missing,
    Armed(u64),
    /// Durable high-water fence: this and every older generation are retired.
    /// Never remove/reset this record as part of normal disarm or restart.
    Closed(u64),
    /// Includes corrupt, unsupported, unsafe or unreadable existing state.
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Table {
    Absent,
    /// Both package ownership and the complete current policy were proven.
    OwnedVerified(Policy),
    /// Ownership was proven independently, but the policy needs reconciliation.
    OwnedUnrecognized,
    /// A table with the fixed name exists without valid ownership proof.
    Foreign,
    /// Includes failed, partial or unavailable table/ownership observations.
    Unreadable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    pub marker: Marker,
    pub table: Table,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Must fail if any table appeared after the observed absence.
    CreateTableAtomic(Policy),
    /// Must revalidate the same proven ownership before atomic replacement.
    ReplaceOwnedTableAtomic(Policy),
    VerifyTable(Policy),
    PersistArmedDurably(u64),
    PersistClosedDurably(u64),
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
        (Marker::Missing | Marker::Closed(_), Table::Absent) => {
            status(Protection::Disarmed {}, Health::Verified)
        }
        (Marker::Armed(generation), Table::OwnedVerified(Policy::FullVpn)) => {
            status(Protection::Armed { generation }, Health::Verified)
        }
        (Marker::Invalid, Table::OwnedVerified(Policy::Emergency)) => {
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
    require_observed_ownership(observation.table)?;
    match request {
        Request::Arm {
            generation,
            mode: Mode::Full,
        } => {
            if matches!(observation.marker, Marker::Armed(current) if current != generation) {
                return Err(ErrorCode::GenerationConflict);
            }
            if matches!(observation.marker, Marker::Closed(closed) if generation <= closed) {
                return Err(ErrorCode::GenerationConflict);
            }
            // Reinstall/verify even an idempotent retry: cached success is not
            // fresh evidence that a root-owned table still exists.
            Ok(Transaction::new(
                vec![
                    install_effect(observation.table, Policy::FullVpn)?,
                    Effect::VerifyTable(Policy::FullVpn),
                    Effect::PersistArmedDurably(generation),
                ],
                status(Protection::Armed { generation }, Health::Verified),
            ))
        }
        Request::Disarm { generation } => {
            if observation.marker != Marker::Armed(generation) {
                // A retry confirms only the exact durably closed generation.
                // Missing state cannot prove that a disarm ever happened.
                if observation
                    == (Observation {
                        marker: Marker::Closed(generation),
                        table: Table::Absent,
                    })
                {
                    return Ok(Transaction::new(vec![], observe(observation)));
                }
                return Err(ErrorCode::GenerationConflict);
            }
            Ok(disarm(generation, observation.table))
        }
        Request::Status {} => unreachable!(),
    }
}

fn disarm(generation: u64, table: Table) -> Transaction {
    let mut steps = vec![Effect::PersistClosedDurably(generation)];
    steps.extend(remove_owned_table(table));
    Transaction::new(steps, status(Protection::Disarmed {}, Health::Verified))
}

/// Root/internal startup only; deliberately absent from the wire request enum.
/// The helper lock must cover snapshot, reconciliation and final observation.
pub fn reconcile(observation: Observation) -> Result<Transaction, ErrorCode> {
    require_observed_ownership(observation.table)?;
    Ok(match observation.marker {
        Marker::Missing | Marker::Closed(_) => Transaction::new(
            remove_owned_table(observation.table),
            status(Protection::Disarmed {}, Health::Verified),
        ),
        Marker::Armed(generation) => Transaction::new(
            vec![
                install_effect(observation.table, Policy::FullVpn)?,
                Effect::VerifyTable(Policy::FullVpn),
            ],
            status(Protection::Armed { generation }, Health::Verified),
        ),
        Marker::Invalid => Transaction::new(
            vec![
                install_effect(observation.table, Policy::Emergency)?,
                Effect::VerifyTable(Policy::Emergency),
            ],
            status(Protection::Emergency {}, Health::ManualRecoveryRequired),
        ),
    })
}

fn require_observed_ownership(table: Table) -> Result<(), ErrorCode> {
    match table {
        Table::Absent | Table::OwnedVerified(_) | Table::OwnedUnrecognized => Ok(()),
        Table::Foreign | Table::Unreadable => Err(ErrorCode::ManualRecoveryRequired),
    }
}

fn install_effect(table: Table, policy: Policy) -> Result<Effect, ErrorCode> {
    match table {
        Table::Absent => Ok(Effect::CreateTableAtomic(policy)),
        Table::OwnedVerified(_) | Table::OwnedUnrecognized => {
            Ok(Effect::ReplaceOwnedTableAtomic(policy))
        }
        Table::Foreign | Table::Unreadable => Err(ErrorCode::ManualRecoveryRequired),
    }
}

/// Called only after ownership admission. The future executor must revalidate
/// ownership immediately before deletion; the fixed table name is not proof.
fn remove_owned_table(table: Table) -> Vec<Effect> {
    match table {
        Table::Absent => vec![Effect::VerifyTableAbsent],
        Table::OwnedVerified(_) | Table::OwnedUnrecognized => {
            vec![Effect::DeleteOwnedTableAtomic, Effect::VerifyTableAbsent]
        }
        Table::Foreign | Table::Unreadable => unreachable!("ownership admission required"),
    }
}
