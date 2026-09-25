// SPDX-License-Identifier: MIT
//! Uninstalled resolved D-Bus conformance boundary, not a DNS host broker.
//!
//! There is deliberately no public constructor, system/session-bus discovery,
//! privileged binary, caller-selected destination, or production dependent.
//! Only in-crate tests construct it on their own disposable bus. A future
//! trusted adapter must prove enrollment, kernel lease, exclusive DNS ownership,
//! pristine baseline, bus authentication and routing before enabling access.
//! The fixed synthetic policy below is not an adopted production policy.

use std::{fmt, future::Future, time::Duration};

use serde::{Serialize, de::DeserializeOwned};
use zbus::{Message, blocking::Connection, zvariant::OwnedValue};

const MANAGER_PATH: &str = "/org/freedesktop/resolve1";
const MANAGER_INTERFACE: &str = "org.freedesktop.resolve1.Manager";
const LINK_INTERFACE: &str = "org.freedesktop.resolve1.Link";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const MAX_REPLY: usize = 16 * 1024;
const MAX_ENTRIES: usize = 8;
const MAX_DOMAIN_BYTES: usize = 253;
// Synthetic namespace policy only. No host installation consumes this crate.
const FIXED_DNS: [u8; 4] = [198, 18, 0, 2];

type Servers = Vec<(i32, Vec<u8>)>;
type Domains = Vec<(String, bool)>;

/// Fixed public errors deliberately retain no raw bus error, target or value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unavailable,
    InvalidReply,
    ReadbackMismatch,
    AuthorizationRefused,
    OutcomeUnknown,
    RecoveryRequired,
}

impl Error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "dns_transport_unavailable",
            Self::InvalidReply => "dns_invalid_reply",
            Self::ReadbackMismatch => "dns_readback_mismatch",
            Self::AuthorizationRefused => "dns_authorization_refused",
            Self::OutcomeUnknown => "dns_outcome_unknown",
            Self::RecoveryRequired => "dns_manual_recovery_required",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "DNS transport is unavailable",
            Self::InvalidReply => "DNS response is invalid or exceeds its bound",
            Self::ReadbackMismatch => "DNS settings do not match the fixed policy",
            Self::AuthorizationRefused => "DNS authorization was refused",
            Self::OutcomeUnknown => "DNS operation outcome is unknown",
            Self::RecoveryRequired => "DNS recovery is required before further writes",
        })
    }
}

impl std::error::Error for Error {}

/// Effective observed values only: NOT a snapshot authorizing restoration.
/// In particular, DefaultRoute cannot distinguish automatic from explicit false.
pub struct Observation {
    servers: Servers,
    domains: Domains,
    default_route: bool,
}

impl fmt::Debug for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Observation { private values omitted }")
    }
}

impl Observation {
    pub fn matches_fixed_policy(&self) -> bool {
        self.servers == [(2, FIXED_DNS.to_vec())]
            && self.domains == [(".".to_owned(), true)]
            && self.default_route
    }
}

/// Fixed semantic methods over a pinned bus owner and one internal link.
/// Methods are synchronous but every full send/reply future has a deadline.
/// No constructor is exported until trusted host admission exists. This is NOT
/// an authorization type: private fields prevent accidental current exposure,
/// not malicious code linked into the same future privileged process.
pub struct Resolved {
    connection: Connection,
    owner: String,
    link_path: String,
    ifindex: i32,
    timeout: Duration,
    uncertain_write: bool,
}

impl Resolved {
    /// Reads are uncached and separately requested; this is not an atomic
    /// multi-property snapshot and never constitutes a reversible baseline.
    pub fn observe(&self) -> Result<Observation, Error> {
        let servers: Servers = self.property("DNS")?;
        let domains: Domains = self.property("Domains")?;
        let default_route: bool = self.property("DefaultRoute")?;
        if servers.len() > MAX_ENTRIES
            || domains.len() > MAX_ENTRIES
            || servers
                .iter()
                .any(|(family, bytes)| !matches!((*family, bytes.len()), (2, 4) | (10, 16)))
            || domains.iter().any(|(name, _)| {
                name.is_empty()
                    || name.len() > MAX_DOMAIN_BYTES
                    || !name.is_ascii()
                    || name.bytes().any(|b| b.is_ascii_control())
            })
        {
            return Err(Error::InvalidReply);
        }
        Ok(Observation {
            servers,
            domains,
            default_route,
        })
    }

    pub fn verify_fixed_policy(&self) -> Result<(), Error> {
        if self.observe()?.matches_fixed_policy() {
            Ok(())
        } else {
            Err(Error::ReadbackMismatch)
        }
    }

    pub fn set_fixed_servers(&mut self) -> Result<(), Error> {
        self.write(
            "SetLinkDNS",
            &(self.ifindex, vec![(2_i32, FIXED_DNS.to_vec())]),
        )
    }

    pub fn set_root_routing_domain(&mut self) -> Result<(), Error> {
        self.write("SetLinkDomains", &(self.ifindex, vec![(".", true)]))
    }

    pub fn set_default_route(&mut self) -> Result<(), Error> {
        self.write("SetLinkDefaultRoute", &(self.ifindex, true))
    }

    /// Revert is a wire primitive, NOT restoration of observe(). It resets
    /// additional per-link properties. Future callers require proven pristine
    /// exclusive link ownership and verified cleanup; never infer that here.
    pub fn revert_pristine_link(&mut self) -> Result<(), Error> {
        self.write("RevertLink", &(self.ifindex,))
    }

    fn property<T>(&self, name: &'static str) -> Result<T, Error>
    where
        T: TryFrom<OwnedValue>,
    {
        let reply = complete(
            self.timeout,
            self.connection.inner().call_method(
                Some(self.owner.as_str()),
                self.link_path.as_str(),
                Some(PROPERTIES_INTERFACE),
                "Get",
                &(LINK_INTERFACE, name),
            ),
        )
        .map_err(|_| Error::Unavailable)?;
        let value: OwnedValue = bounded_reply(&reply)?;
        T::try_from(value).map_err(|_| Error::InvalidReply)
    }

    fn write<B>(&mut self, method: &'static str, body: &B) -> Result<(), Error>
    where
        B: Serialize + zbus::zvariant::DynamicType,
    {
        if self.uncertain_write {
            return Err(Error::RecoveryRequired);
        }
        let reply = complete(
            self.timeout,
            self.connection.inner().call_method(
                Some(self.owner.as_str()),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                method,
                body,
            ),
        );
        match reply {
            Ok(message) if bounded_reply::<()>(&message).is_ok() => Ok(()),
            Err(Some(zbus::Error::MethodError(name, _, _)))
                if matches!(
                    name.as_str(),
                    "org.freedesktop.DBus.Error.AccessDenied"
                        | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
                ) =>
            {
                Err(Error::AuthorizationRefused)
            }
            // Includes timeout, disconnect, malformed success and arbitrary
            // remote errors. Do not assume a failed reply means zero effect.
            _ => {
                self.uncertain_write = true;
                Err(Error::OutcomeUnknown)
            }
        }
    }
}

fn bounded_reply<T>(message: &Message) -> Result<T, Error>
where
    T: DeserializeOwned + zbus::zvariant::Type,
{
    // zbus receives a wire message before this check; upstream's allocation
    // ceiling is 128 MiB, not MAX_REPLY. See README before production adoption.
    if message.data().len() > MAX_REPLY || !message.data().fds().is_empty() {
        return Err(Error::InvalidReply);
    }
    message
        .body()
        .deserialize()
        .map_err(|_| Error::InvalidReply)
}

fn complete<T>(
    timeout: Duration,
    call: impl Future<Output = zbus::Result<T>>,
) -> Result<T, Option<zbus::Error>> {
    futures_lite::future::block_on(futures_lite::future::race(
        async { call.await.map_err(Some) },
        async {
            async_io::Timer::after(timeout).await;
            Err(None)
        },
    ))
}

#[cfg(test)]
mod tests;
