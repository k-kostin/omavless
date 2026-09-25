// SPDX-License-Identifier: MIT
//! Uninstalled resolved D-Bus conformance boundary, not a DNS host broker.
//!
//! No system/session-bus discovery, privileged binary or IPC-selected targets.
//! ManagedResolved composes an already admitted real TUN with a trusted caller's
//! pre-authenticated connection/pinned owner. Its caller must prove enrollment,
//! retention, exclusive DNS ownership and fixed bus/policy admission separately.
//! The fixed synthetic policy below is not an adopted production policy.

use std::{
    fmt,
    future::Future,
    time::{Duration, Instant},
};

use serde::{Serialize, de::DeserializeOwned};
use zbus::{Message, blocking::Connection, zvariant::OwnedValue};

mod managed;
pub use managed::ManagedResolved;

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
type ExtendedServers = Vec<(i32, Vec<u8>, u16, String)>;
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
    NonPristine,
    UnsupportedPolicy,
    LeaseLost,
    OwnershipChanged,
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
            Self::NonPristine => "dns_baseline_not_pristine",
            Self::UnsupportedPolicy => "dns_policy_unsupported",
            Self::LeaseLost => "dns_lease_identity_lost",
            Self::OwnershipChanged => "dns_reserved_settings_changed",
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
            Self::NonPristine => "The managed link does not have a compatible empty DNS baseline",
            Self::LeaseLost => "The managed DNS lease identity could not be verified",
            Self::OwnershipChanged => "Reserved DNS settings changed; recovery is required",
            Self::UnsupportedPolicy => {
                "The existing resolver policy needs separate compatibility review"
            }
        })
    }
}

impl std::error::Error for Error {}

/// Effective observed values only: NOT a snapshot authorizing restoration.
/// In particular, DefaultRoute cannot distinguish automatic from explicit false.
pub struct Observation {
    servers: Servers,
    extended_servers: ExtendedServers,
    domains: Domains,
    default_route: bool,
    unchanged: Unchanged,
}

#[derive(Clone, PartialEq, Eq)]
struct Unchanged {
    llmnr: String,
    mdns: String,
    dns_over_tls: String,
    dnssec: String,
    negative_trust_anchors: Vec<String>,
}

/// Reserved for a future trusted composition adapter. No public factory exists.
/// Effective property reads cannot mint proof of exclusive ownership/freshness.
pub struct ExclusiveOwnership {
    _sealed: (),
}

/// Opaque effective-value comparison bound to one pinned service/link.
/// This is not a reversible snapshot or a kernel/retention authorization token.
pub struct Baseline {
    owner: String,
    link_path: String,
    ifindex: i32,
    unchanged: Unchanged,
}

impl fmt::Debug for Baseline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Baseline { private values omitted }")
    }
}

impl fmt::Debug for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Observation { private values omitted }")
    }
}

impl Observation {
    pub fn matches_fixed_policy(&self) -> bool {
        self.servers == [(2, FIXED_DNS.to_vec())]
            && self.extended_servers == [(2, FIXED_DNS.to_vec(), 0, String::new())]
            && self.domains == [(".".to_owned(), true)]
            && self.default_route
            && self.compatible_policy()
    }

    fn compatible_policy(&self) -> bool {
        // No extra privileged setter silently disables TLS or DNSSEC. Other
        // modes need a separately reviewed fake-IP/plaintext compatibility gate.
        self.unchanged.dns_over_tls == "no" && self.unchanged.dnssec == "no"
    }

    fn empty_dns(&self) -> bool {
        self.servers.is_empty()
            && self.extended_servers.is_empty()
            && self.domains.is_empty()
            && !self.default_route
            && self.unchanged.negative_trust_anchors.is_empty()
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
    deadline: Option<Instant>,
    uncertain_write: bool,
}

impl Resolved {
    fn set_deadline(&mut self, deadline: Instant) -> Result<(), Error> {
        validate_deadline(deadline)?;
        self.deadline = Some(deadline);
        Ok(())
    }

    fn call_deadline(&self) -> Result<Instant, Error> {
        operation_deadline(self.timeout, self.deadline)
    }

    /// Reads are uncached and separately requested; this is not an atomic
    /// multi-property snapshot and never constitutes a reversible baseline.
    pub fn observe(&self) -> Result<Observation, Error> {
        let servers: Servers = self.property("DNS")?;
        let extended_servers: ExtendedServers = self.property("DNSEx")?;
        let domains: Domains = self.property("Domains")?;
        let default_route: bool = self.property("DefaultRoute")?;
        let unchanged = Unchanged {
            llmnr: self.property("LLMNR")?,
            mdns: self.property("MulticastDNS")?,
            dns_over_tls: self.property("DNSOverTLS")?,
            dnssec: self.property("DNSSEC")?,
            negative_trust_anchors: self.property("DNSSECNegativeTrustAnchors")?,
        };
        if servers.len() > MAX_ENTRIES
            || extended_servers.len() > MAX_ENTRIES
            || domains.len() > MAX_ENTRIES
            || unchanged.negative_trust_anchors.len() > MAX_ENTRIES
            || servers
                .iter()
                .any(|(family, bytes)| !matches!((*family, bytes.len()), (2, 4) | (10, 16)))
            || extended_servers.iter().any(|(family, bytes, _, name)| {
                !matches!((*family, bytes.len()), (2, 4) | (10, 16))
                    || (!name.is_empty() && !bounded_domain(name))
            })
            || extended_servers.len() != servers.len()
            || extended_servers
                .iter()
                .zip(&servers)
                .any(|((family, address, _, _), old)| *family != old.0 || *address != old.1)
            || domains.iter().any(|(name, _)| !bounded_domain(name))
            || unchanged
                .negative_trust_anchors
                .iter()
                .any(|name| !bounded_domain(name))
            || !matches!(unchanged.llmnr.as_str(), "no" | "yes" | "resolve")
            || !matches!(unchanged.mdns.as_str(), "no" | "yes" | "resolve")
            || !matches!(
                unchanged.dns_over_tls.as_str(),
                "no" | "yes" | "opportunistic"
            )
            || !matches!(unchanged.dnssec.as_str(), "no" | "yes" | "allow-downgrade")
        {
            return Err(Error::InvalidReply);
        }
        Ok(Observation {
            servers,
            extended_servers,
            domains,
            default_route,
            unchanged,
        })
    }

    /// Empty effective fields are a refusal gate, not proof the link was freshly
    /// created. Only the future sealed ownership adapter may authorize capture.
    pub fn capture_baseline(&self, _ownership: &ExclusiveOwnership) -> Result<Baseline, Error> {
        if self.uncertain_write {
            return Err(Error::RecoveryRequired);
        }
        let observation = self.observe()?;
        if !observation.compatible_policy() {
            return Err(Error::UnsupportedPolicy);
        }
        if !observation.empty_dns() {
            return Err(Error::NonPristine);
        }
        Ok(Baseline {
            owner: self.owner.clone(),
            link_path: self.link_path.clone(),
            ifindex: self.ifindex,
            unchanged: observation.unchanged,
        })
    }

    /// Compare only. Does not dispatch Revert or authorize compensation after an
    /// unknown write; the broker must prove an ordered completion boundary.
    pub fn verify_reset(&self, baseline: &Baseline) -> Result<(), Error> {
        self.verify_baseline_binding(baseline)?;
        let observation = self.observe()?;
        if observation.empty_dns() && observation.unchanged == baseline.unchanged {
            Ok(())
        } else {
            Err(Error::ReadbackMismatch)
        }
    }

    pub fn verify_policy_preserves_baseline(&self, baseline: &Baseline) -> Result<(), Error> {
        self.verify_baseline_binding(baseline)?;
        let observation = self.observe()?;
        if observation.unchanged != baseline.unchanged {
            return Err(Error::OwnershipChanged);
        }
        if observation.matches_fixed_policy() {
            Ok(())
        } else {
            Err(Error::ReadbackMismatch)
        }
    }

    /// This is a prerequisite for a known-settled whole-link reset, never a fence
    /// for an unknown write. A foreign change must not be erased by RevertLink.
    fn revert_preserving_untouched(&mut self, baseline: &Baseline) -> Result<(), Error> {
        self.verify_baseline_binding(baseline)?;
        if self.observe()?.unchanged != baseline.unchanged {
            return Err(Error::OwnershipChanged);
        }
        self.revert_pristine_link()
    }

    fn verify_baseline_binding(&self, baseline: &Baseline) -> Result<(), Error> {
        if self.uncertain_write {
            return Err(Error::RecoveryRequired);
        }
        if self.owner != baseline.owner
            || self.link_path != baseline.link_path
            || self.ifindex != baseline.ifindex
        {
            return Err(Error::OwnershipChanged);
        }
        Ok(())
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
        let reply = complete_until(
            self.call_deadline()?,
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
        let deadline = self.call_deadline().map_err(|_| {
            self.uncertain_write = true;
            Error::OutcomeUnknown
        })?;
        let reply = complete_until(
            deadline,
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

fn bounded_domain(value: &str) -> bool {
    if value == "." {
        return true;
    }
    let normalized = value.strip_suffix('.').unwrap_or(value);
    !normalized.is_empty()
        && value.len() <= MAX_DOMAIN_BYTES
        && normalized.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
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

#[cfg(test)]
fn complete<T>(
    timeout: Duration,
    call: impl Future<Output = zbus::Result<T>>,
) -> Result<T, Option<zbus::Error>> {
    complete_until(Instant::now() + timeout, call)
}

fn validate_deadline(deadline: Instant) -> Result<(), Error> {
    if deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(30) {
        Err(Error::Unavailable)
    } else {
        Ok(())
    }
}

fn operation_deadline(timeout: Duration, operation: Option<Instant>) -> Result<Instant, Error> {
    let now = Instant::now();
    let deadline = operation.unwrap_or(now + timeout).min(now + timeout);
    if deadline <= now {
        Err(Error::Unavailable)
    } else {
        Ok(deadline)
    }
}

fn complete_until<T>(
    deadline: Instant,
    call: impl Future<Output = zbus::Result<T>>,
) -> Result<T, Option<zbus::Error>> {
    if Instant::now() >= deadline {
        return Err(None);
    }
    futures_lite::future::block_on(futures_lite::future::race(
        async { call.await.map_err(Some) },
        async {
            async_io::Timer::at(deadline).await;
            Err(None)
        },
    ))
}

#[cfg(test)]
mod tests;
