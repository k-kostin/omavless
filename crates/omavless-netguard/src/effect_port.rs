//! Inactive kernel-effect contract for the single-writer `LockedState` model.
//! This module performs no I/O and establishes no ownership by itself.
use crate::{policy::Policy, transaction::Table};

/// An independently established kernel identity supplied by the future adapter.
/// These fields are a model, not an attestation: matching a durable receipt,
/// table name, policy, handle or owner port ID cannot construct actual authority.
/// A production adapter must bind its observations and conditional effects to
/// the same pinned namespace and live kernel session for the entire request.
/// There is deliberately no conversion from a receipt or the legacy port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectIdentity {
    pub boot: [u8; 16],
    pub netns_inode: u64,
    pub table_handle: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectSnapshot {
    pub table: Table,
    pub identity: Option<EffectIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectError {
    UnavailableOrUncertain,
}

/// Kernel effects only; `LockedState` is the sole marker/receipt writer.
///
/// A future implementation must exclusively create, condition replacement and
/// deletion on independently proven live ownership, verify complete readback,
/// and return uncertainty for a lost acknowledgement or incomplete observation.
/// It must retain the same namespace/socket lifetime across observation, effect
/// and readback. Success describes the kernel effect, never filesystem commit
/// or a protected product state. The caller publishes the terminal receipt only
/// after kernel readback and durable marker persistence under its held lock.
///
/// Implementations must not open/write a root marker or receipt store, acquire
/// that store's lock, or infer orphan ownership from its records. No production
/// implementation exists. In particular there is no blanket adapter from the
/// legacy `coordinator::KernelPort`, whose contract includes receipt writes.
pub trait EffectPort {
    fn observe(&mut self) -> Result<EffectSnapshot, EffectError>;
    fn create_if_absent(&mut self, policy: Policy) -> Result<EffectIdentity, EffectError>;
    fn replace_owned(
        &mut self,
        identity: EffectIdentity,
        policy: Policy,
    ) -> Result<EffectIdentity, EffectError>;
    fn delete_owned(&mut self, identity: EffectIdentity) -> Result<(), EffectError>;
}
