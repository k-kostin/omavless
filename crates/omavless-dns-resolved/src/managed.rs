// SPDX-License-Identifier: MIT
use super::*;
use omavless_dns_tun::HeldTun;
use zbus::zvariant::OwnedObjectPath;

const SERVICE: &str = "org.freedesktop.resolve1";
const CALL_TIMEOUT: Duration = Duration::from_secs(2);

/// Trusted-library bridge, not a root authorization mechanism or IPC API.
///
/// Caller supplies a fixed-system-bus connection and authenticated unique owner
/// verified by its root-owned enrollment context. It must reserve exclusive
/// meta-ipv4-v1 DNS ownership, independently retain the proof, and journal Pending
/// before writes. Targets come from HeldTun, never caller index/name/DNS data.
/// Pre/post checks are NOT atomic with D-Bus and cannot fence a timed-out write.
/// Root/CAP_NET_ADMIN administration is outside this threat scope. Drop is not
/// verified DNS cleanup; this type retains its actual descriptor until dropped.
pub struct ManagedResolved {
    inner: Managed<HeldTun>,
}

impl ManagedResolved {
    pub fn from_admitted_parts(
        connection: Connection,
        owner: String,
        held: HeldTun,
    ) -> Result<Self, Error> {
        Ok(Self {
            inner: Managed::from_admitted_parts(connection, owner, held)?,
        })
    }

    /// Covers initial owner/link lookup as well as the later operation calls.
    pub fn from_admitted_parts_with_deadline(
        connection: Connection,
        owner: String,
        held: HeldTun,
        deadline: Instant,
    ) -> Result<Self, Error> {
        Ok(Self {
            inner: Managed::construct(connection, owner, held, Some(deadline))?,
        })
    }

    /// Trusted operation budget. Never clears uncertainty or permits compensation
    /// after a timed-out write; this is not an interactive authorization deadline.
    pub fn set_deadline(&mut self, deadline: Instant) -> Result<(), Error> {
        self.inner.resolved.set_deadline(deadline)
    }

    /// The trusted caller has explicitly reserved exclusive DNS ownership;
    /// neither a name nor empty observed fields establishes that fact here.
    pub fn capture_reserved_baseline(&mut self) -> Result<Baseline, Error> {
        self.inner
            .checked(|r| r.capture_baseline(&ExclusiveOwnership { _sealed: () }))
    }

    pub fn set_fixed_servers(&mut self) -> Result<(), Error> {
        self.inner.checked(Resolved::set_fixed_servers)
    }

    pub fn set_root_routing_domain(&mut self) -> Result<(), Error> {
        self.inner.checked(Resolved::set_root_routing_domain)
    }

    pub fn set_default_route(&mut self) -> Result<(), Error> {
        self.inner.checked(Resolved::set_default_route)
    }

    pub fn verify_policy_preserves_baseline(&mut self, baseline: &Baseline) -> Result<(), Error> {
        self.inner
            .checked(|r| r.verify_policy_preserves_baseline(baseline))
    }

    /// Requires owned-pristine-link authority and a settled-operation boundary.
    /// Never restores arbitrary captured values or clears quarantine.
    pub fn revert_owned_link(&mut self, baseline: &Baseline) -> Result<(), Error> {
        self.inner
            .checked(|r| r.revert_preserving_untouched(baseline))
    }

    pub fn verify_reset(&mut self, baseline: &Baseline) -> Result<(), Error> {
        self.inner.checked(|r| r.verify_reset(baseline))
    }

    pub fn recheck(&mut self) -> Result<(), Error> {
        self.inner.checked(|_| Ok(()))
    }
}

trait Lease {
    fn index(&self) -> u32;
    fn recheck(&self) -> Result<(), Error>;
}

impl Lease for HeldTun {
    fn index(&self) -> u32 {
        self.interface_index()
    }
    fn recheck(&self) -> Result<(), Error> {
        HeldTun::recheck(self).map_err(|_| Error::LeaseLost)
    }
}

struct Managed<L> {
    resolved: Resolved,
    held: L,
    poisoned: bool,
}

impl<L: Lease> Managed<L> {
    fn from_admitted_parts(connection: Connection, owner: String, held: L) -> Result<Self, Error> {
        Self::construct(connection, owner, held, None)
    }

    fn construct(
        connection: Connection,
        owner: String,
        held: L,
        deadline: Option<Instant>,
    ) -> Result<Self, Error> {
        if let Some(deadline) = deadline {
            validate_deadline(deadline)?;
        }
        held.recheck()?;
        if owner.len() > 255 || zbus::names::UniqueName::try_from(owner.as_str()).is_err() {
            return Err(Error::Unavailable);
        }
        let ifindex = i32::try_from(held.index()).map_err(|_| Error::LeaseLost)?;
        if ifindex <= 0 {
            return Err(Error::LeaseLost);
        }
        verify_owner(
            &connection,
            &owner,
            operation_deadline(CALL_TIMEOUT, deadline)?,
        )?;
        let reply = complete_until(
            operation_deadline(CALL_TIMEOUT, deadline)?,
            connection.inner().call_method(
                Some(owner.as_str()),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                "GetLink",
                &(ifindex,),
            ),
        )
        .map_err(|_| Error::Unavailable)?;
        let path: OwnedObjectPath = bounded_reply(&reply)?;
        if path.as_str() != link_path(ifindex)? {
            return Err(Error::InvalidReply);
        }
        held.recheck()?;
        verify_owner(
            &connection,
            &owner,
            operation_deadline(CALL_TIMEOUT, deadline)?,
        )?;
        Ok(Self {
            resolved: Resolved {
                connection,
                owner,
                link_path: path.to_string(),
                ifindex,
                timeout: CALL_TIMEOUT,
                deadline,
                uncertain_write: false,
            },
            held,
            poisoned: false,
        })
    }

    fn guard(&self) -> Result<(), Error> {
        self.held.recheck()?;
        if self.held.index() != self.resolved.ifindex as u32 {
            return Err(Error::LeaseLost);
        }
        verify_owner(
            &self.resolved.connection,
            &self.resolved.owner,
            self.resolved.call_deadline()?,
        )
    }

    fn checked<T>(
        &mut self,
        action: impl FnOnce(&mut Resolved) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if self.poisoned || self.resolved.uncertain_write {
            return Err(Error::RecoveryRequired);
        }
        if let Err(error) = self.guard() {
            self.poisoned = true;
            return Err(error);
        }
        let result = action(&mut self.resolved);
        if matches!(result, Err(Error::OwnershipChanged)) {
            self.poisoned = true;
        }
        // A failed/denied call cannot conceal identity loss while it ran.
        if let Err(error) = self.guard() {
            self.poisoned = true;
            return Err(error);
        }
        result
    }
}

fn link_path(index: i32) -> Result<String, Error> {
    if index <= 0 {
        return Err(Error::LeaseLost);
    }
    // resolved uses sd_bus_path_encode on the decimal index. Its FIRST digit
    // is escaped as _3N (ASCII hex); later decimal digits remain unchanged.
    // Keep an exact target check, not a permissive object-path prefix match.
    Ok(format!("/org/freedesktop/resolve1/link/_3{index}"))
}

fn verify_owner(connection: &Connection, expected: &str, deadline: Instant) -> Result<(), Error> {
    let reply = complete_until(
        deadline,
        connection.inner().call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &(SERVICE,),
        ),
    )
    .map_err(|_| Error::Unavailable)?;
    let actual: String = bounded_reply(&reply)?;
    if actual != expected {
        return Err(Error::LeaseLost);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
