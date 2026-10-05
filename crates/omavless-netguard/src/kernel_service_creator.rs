// SPDX-License-Identifier: MIT
//! Developer service's private live causal owner. No receipt/handle adoption.
//! This module is reached only through the fixed trusted-launch acquisition.
use super::*;
use crate::authority_composition::{Boundary, CanonicalCreator};
use crate::effect_port::{EffectError, EffectIdentity, EffectPort, EffectSnapshot};
use crate::policy::Policy;
use crate::receipt::HostEpoch;
use crate::transaction::Table;
use atomic_batch::{AtomicBatch, AtomicReplies, UntrustedStatus, delete_batch, full_batch};
use std::os::fd::AsFd;

/// No public constructor or descriptor extraction. The epoch is a projection
/// of the launch's held originals, never authority supplied by a client/store.
pub(crate) struct LiveCreator {
    session: LocalReadSession,
    epoch: HostEpoch,
    created: Option<u64>,
}
impl LiveCreator {
    pub(crate) fn open(epoch: HostEpoch) -> Result<Self> {
        let session = LocalReadSession::open()?;
        require(session.identity == (epoch.namespace_device, epoch.namespace_inode))?;
        Ok(Self {
            session,
            epoch,
            created: None,
        })
    }

    /// Safe duplicates share the SAME original open file descriptions. Every
    /// alias is retained by acquisition and CLOEXEC; none is exported to IPC.
    pub(crate) fn original_aliases(&self) -> Result<(File, OwnedFd)> {
        Ok((
            self.session.namespace.try_clone().map_err(|_| REFUSE)?,
            self.session
                .socket
                .as_fd()
                .try_clone_to_owned()
                .map_err(|_| REFUSE)?,
        ))
    }

    fn id(&self, handle: u64) -> EffectIdentity {
        EffectIdentity {
            boot: self.epoch.boot,
            netns_inode: self.epoch.namespace_inode,
            table_handle: handle,
        }
    }

    fn terminal<T>(&mut self, result: Result<T>) -> std::result::Result<T, EffectError> {
        result.map_err(|_| {
            // Causal history is not reconstructed after uncertainty. The
            // enclosing ManuallyDrop graph retains original resources.
            self.session.poisoned = true;
            EffectError::UnavailableOrUncertain
        })
    }

    fn full(
        &mut self,
        old: Option<EffectIdentity>,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        let result = (|| {
            let expected_old = old.map(|id| self.id(id.table_handle));
            let mut lease = self.session.borrow_policy_inventory()?;
            let deadline = lease.deadline;
            match old {
                None => require(
                    lease.observed() == LocalPolicyInventory::TableAbsent && self.created.is_none(),
                )?,
                Some(id) => require(
                    Some(id) == expected_old
                        && self.created == Some(id.table_handle)
                        && lease.observed()
                            == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn)
                        && lease
                            .table
                            .as_ref()
                            .is_some_and(|t| t.handle == id.table_handle),
                )?,
            }
            // LockedState has already durably written Pending under its sole
            // lock before calling this private EffectPort operation.
            let handle = if old.is_none() {
                inventory::create_witness::PreparedCreate::new(lease, deadline)?
                    .execute(send_original_once)?
                    .finish()?
            } else {
                lease.recheck()?;
                let requests = full_batch(
                    lease.generation,
                    lease.session.next_sequence,
                    old.map(|id| id.table_handle),
                )?;
                send_original_once(lease.session, requests, deadline)?;
                let (shape, _, table) = lease.session.inspect_policy_inventory_before(deadline)?;
                require(shape == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn))?;
                let handle = table.ok_or(REFUSE)?.handle;
                require(handle != 0 && old.is_none_or(|id| id.table_handle != handle))?;
                lease.session.check(deadline)?;
                handle
            };
            self.created = Some(handle);
            Ok(self.id(handle))
        })();
        self.terminal(result)
    }
}

/// One exact original send; never resend after syscall entry. Every ACK,
/// including END, and the original deadline are required. InFlight poisons on
/// all errors/unwind, without queries, descriptor closure or cleanup.
fn send_original_once(
    session: &mut LocalReadSession,
    requests: AtomicBatch,
    deadline: Instant,
) -> Result<AtomicReplies> {
    struct InFlight<'a> {
        session: &'a mut LocalReadSession,
        complete: bool,
    }
    impl Drop for InFlight<'_> {
        fn drop(&mut self) {
            if !self.complete {
                self.session.poisoned = true;
            }
        }
    }
    let mut held = InFlight {
        session,
        complete: false,
    };
    let session = &mut *held.session;
    session.next_sequence = session
        .next_sequence
        .checked_add(u32::try_from(requests.len()).map_err(|_| REFUSE)?)
        .ok_or(REFUSE)?;
    let bytes = requests.concat();
    let mut replies = AtomicReplies::new(requests, session.local.pid())?;
    session.check(deadline)?;
    require(
        sendto(
            session.socket.as_raw_fd(),
            &bytes,
            &NetlinkAddr::new(0, 0),
            MsgFlags::MSG_DONTWAIT,
        )
        .map_err(|_| REFUSE)?
            == bytes.len(),
    )?;
    while !replies.complete() {
        session.check(deadline)?;
        let mut bytes = [0; LIMIT];
        let mut iov = [IoSliceMut::new(&mut bytes)];
        match recvmsg::<NetlinkAddr>(
            session.socket.as_raw_fd(),
            &mut iov,
            None,
            MsgFlags::MSG_DONTWAIT,
        ) {
            Ok(reply) => {
                let (length, sender, flags) = (reply.bytes, reply.address, reply.flags);
                require(length <= LIMIT)?;
                replies.receive(&bytes[..length], sender, flags)?;
            }
            Err(nix::errno::Errno::EAGAIN) => std::thread::sleep(Duration::from_millis(1)),
            Err(_) => return Err(REFUSE),
        }
    }
    session.check(deadline)?;
    require(replies.status() == UntrustedStatus::AllAcknowledged)?;
    held.complete = true;
    Ok(replies)
}

impl crate::effect_port::sealed::Sealed for LiveCreator {}
impl crate::authority_composition::sealed::Sealed for LiveCreator {}
impl CanonicalCreator for LiveCreator {
    fn retained_epoch(&mut self, _: Boundary) -> std::result::Result<HostEpoch, EffectError> {
        let result = self
            .session
            .check(Instant::now() + Duration::from_secs(1))
            .map(|()| self.epoch);
        self.terminal(result)
    }
}
impl EffectPort for LiveCreator {
    fn observe(&mut self) -> std::result::Result<EffectSnapshot, EffectError> {
        let result = (|| {
            if self.created.is_none()
                && self.session.inspect()? == LocalTablePresence::PresentUntrusted
            {
                // No causal history: do not inspect shape to manufacture
                // adoption. Even matching Live userdata remains an orphan.
                return Ok(EffectSnapshot {
                    table: Table::Foreign,
                    identity: None,
                });
            }
            let (shape, _, table) = self.session.inspect_policy_inventory_once()?;
            Ok(match (shape, self.created, table) {
                (LocalPolicyInventory::TableAbsent, None, None) => EffectSnapshot {
                    table: Table::Absent,
                    identity: None,
                },
                (
                    LocalPolicyInventory::ExactUntrusted(Policy::FullVpn),
                    Some(created),
                    Some(table),
                ) if table.handle == created => EffectSnapshot {
                    table: Table::OwnedVerified(Policy::FullVpn),
                    identity: Some(self.id(created)),
                },
                // Cold orphan remains explicitly unowned. It can be reported
                // conservatively but can NEVER enter replacement/deletion.
                (_, None, Some(_)) => EffectSnapshot {
                    table: Table::Foreign,
                    identity: None,
                },
                _ => return Err(REFUSE),
            })
        })();
        self.terminal(result)
    }
    fn create_if_absent(
        &mut self,
        policy: Policy,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        if policy != Policy::FullVpn {
            return self.terminal(Err(REFUSE));
        }
        self.full(None)
    }
    fn replace_owned(
        &mut self,
        id: EffectIdentity,
        policy: Policy,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        if policy != Policy::FullVpn {
            return self.terminal(Err(REFUSE));
        }
        self.full(Some(id))
    }
    fn delete_owned(&mut self, id: EffectIdentity) -> std::result::Result<(), EffectError> {
        let result = (|| {
            require(self.created == Some(id.table_handle) && id == self.id(id.table_handle))?;
            let mut lease = self.session.borrow_policy_inventory()?;
            require(
                lease.observed() == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn)
                    && lease
                        .table
                        .as_ref()
                        .is_some_and(|t| t.handle == id.table_handle),
            )?;
            lease.recheck()?;
            let deadline = lease.deadline;
            let requests = delete_batch(
                lease.generation,
                id.table_handle,
                lease.session.next_sequence,
            )?;
            send_original_once(lease.session, requests, deadline)?;
            let (shape, _, table) = lease.session.inspect_policy_inventory_before(deadline)?;
            require(shape == LocalPolicyInventory::TableAbsent && table.is_none())?;
            lease.session.check(deadline)?;
            self.created = None;
            Ok(())
        })();
        self.terminal(result)
    }
}
