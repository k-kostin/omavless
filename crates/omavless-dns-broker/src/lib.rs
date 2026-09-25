//! Opt-in root DNS broker candidate. Not installed by the production package.
mod access;
pub mod admission;
pub mod journal;
mod server;
mod transaction;
pub use server::{Error, serve};
