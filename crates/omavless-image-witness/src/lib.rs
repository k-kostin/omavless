// SPDX-License-Identifier: MIT
//! Developer-only current executable observation, never an effect permit.
//! No default helper/service, DNS/netguard privilege or arbitrary PID/path API.
#![cfg(all(target_os = "linux", feature = "developer-helper"))]

mod channel;
mod class;
mod kernel;
mod protocol;
mod service;
mod status;
pub use service::{Client, serve_development, serve_development_runtime};

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Refused,
    Unavailable,
    Expired,
    ChannelLost,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Refused => "Original executable witness refused.",
            Self::Unavailable => "Original executable witness unavailable.",
            Self::Expired => "Original executable witness deadline expired.",
            Self::ChannelLost => "Original executable witness channel lost.",
        })
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
