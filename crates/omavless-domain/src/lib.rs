// SPDX-License-Identifier: MIT

//! Pure R3 domain semantics. This crate has no filesystem, process, network or
//! production-runtime ownership; Python remains the current owner and oracle.

pub mod routing;
pub mod store;
