//! Opt-in root DNS broker candidate. Not installed by the production package.
mod access;
pub mod admission;
mod diagnostic;
pub mod journal;
mod server;
mod transaction;
pub use server::{Error, serve};
