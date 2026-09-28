//! Fixed-purpose root DNS broker, with distinct experimental and release identities.
#[cfg(feature = "release-package")]
pub(crate) const ENROLLMENT_POLICY: &str = "meta-ipv4-release-v1";
#[cfg(not(feature = "release-package"))]
pub(crate) const ENROLLMENT_POLICY: &str = "meta-ipv4-v1";
mod access;
pub mod admin;
pub mod admission;
mod diagnostic;
pub mod journal;
mod server;
mod transaction;
pub use server::{Error, serve};
