//! K1 foundation only: no socket, filesystem, process, privilege or firewall I/O.
//! No production runtime depends on this crate. A future host adapter must
//! authenticate peers, serialize transactions and verify every acknowledged effect.

pub mod policy;
pub mod protocol;
pub mod transaction;
