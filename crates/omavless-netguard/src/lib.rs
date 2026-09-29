//! Inactive K1 foundation: no socket, process, privilege or firewall execution.
//! The root-state adapter is not installed or called by production runtime.
//! No production runtime depends on this crate. A future host adapter must
//! authenticate peers, serialize transactions and verify every acknowledged effect.

pub mod coordinator;
pub mod locked_state;
pub mod nft;
pub mod policy;
pub mod protocol;
pub mod receipt;
pub mod receipt_store;
pub mod root_state;
pub mod transaction;
