// SPDX-License-Identifier: MIT

//! Pure R3 domain semantics. This crate has no filesystem, process, network or
//! production-runtime ownership; Python remains the current owner and oracle.

pub mod config;
pub mod import;
pub mod private_backup;
pub mod private_store;
pub mod route_check;
pub mod routing;
pub mod store;
pub mod subscription;
pub mod subscription_feed;

// Internal plaintext framing and strict pair gate. Only private_backup exposes
// an authenticated public entry point; never persist this inner representation.
mod backup_payload_candidate;
