//! Inactive K1 candidates: no production caller, installation or firewall mutation.
//! The root-state adapter is not installed or called by production runtime.
//! No production runtime depends on this crate. A future host adapter must
//! authenticate peers, serialize transactions and verify every acknowledged effect.

pub mod coordinator;
pub mod effect_port;
mod enrollment;
#[cfg(target_os = "linux")]
pub mod kernel_observer;
pub mod locked_state;
pub mod nft;
pub mod policy;
pub mod protocol;
pub mod receipt;
pub mod receipt_store;
pub mod root_state;
pub mod transaction;
