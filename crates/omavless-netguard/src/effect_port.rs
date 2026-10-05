//! Inactive kernel-effect contract for the single-writer `LockedState` model.
//! This module performs no I/O and establishes no ownership by itself.
use crate::{policy::Policy, transaction::Table};

// A production implementation must be reviewed inside this crate. External
// callers cannot inject fabricated ownership snapshots into LockedState.
pub(crate) mod sealed {
    pub trait Sealed {}
}

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

/// Inactive composition fences, not evidence of namespace or creator authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExchangeBoundary {
    BeforeAccept,
    AfterAccept,
    BeforeReceive,
    AfterReceive,
    BeforeReply,
    AfterReply,
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
/// External crates cannot implement this port, even with a matching receipt or
/// policy-shaped table. This is a compile-time boundary, not kernel provenance.
///
/// ```compile_fail
/// use omavless_netguard::effect_port::{EffectPort, EffectSnapshot, EffectError, EffectIdentity};
/// use omavless_netguard::policy::Policy;
/// struct Invented;
/// impl EffectPort for Invented {
///     fn observe(&mut self) -> Result<EffectSnapshot, EffectError> { unimplemented!() }
///     fn create_if_absent(&mut self, _: Policy) -> Result<EffectIdentity, EffectError> { unimplemented!() }
///     fn replace_owned(&mut self, _: EffectIdentity, _: Policy) -> Result<EffectIdentity, EffectError> { unimplemented!() }
///     fn delete_owned(&mut self, _: EffectIdentity) -> Result<(), EffectError> { unimplemented!() }
/// }
/// ```
pub trait EffectPort: sealed::Sealed {
    /// Legacy/model ports have no authority provider. The default preserves
    /// their old behavior and must never be interpreted as authentication.
    /// The separate authority composition overrides this with its retained
    /// provider. No production implementation exists.
    fn exchange_boundary(&mut self, _: ExchangeBoundary) -> Result<(), EffectError> {
        Ok(())
    }
    fn observe(&mut self) -> Result<EffectSnapshot, EffectError>;
    fn create_if_absent(&mut self, policy: Policy) -> Result<EffectIdentity, EffectError>;
    fn replace_owned(
        &mut self,
        identity: EffectIdentity,
        policy: Policy,
    ) -> Result<EffectIdentity, EffectError>;
    fn delete_owned(&mut self, identity: EffectIdentity) -> Result<(), EffectError>;
}
