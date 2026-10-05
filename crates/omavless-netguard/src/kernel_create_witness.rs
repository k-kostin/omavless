// SPDX-License-Identifier: MIT
//! Private fixture-only causality extraction; no canonical owner or effect API.
//! The caller is the trusted existing fixture transport, not supplied metadata.
use super::*;
use crate::kernel_observer::atomic_batch::{
    AtomicBatch, AtomicReplies, UntrustedStatus, full_batch,
};

/// Cannot be cloned, copied, serialized, or constructed from a handle/port.
/// Retains the original complete absent inventory and exclusive session borrow.
pub(in crate::kernel_observer) struct PreparedCreate<'a> {
    lease: LocalInventoryLease<'a>,
    first: u32,
    requests: AtomicBatch,
}

// Retain the exclusive original borrow during every callback and full read.
// Error/unwind poisons only memory; Drop never sends, closes or queries.
struct InFlight<'a>(Option<&'a mut LocalReadSession>);
impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        if let Some(session) = &mut self.0 {
            session.poisoned = true;
        }
    }
}

fn acknowledged(replies: &AtomicReplies, generation: u32, first: u32) -> Result<()> {
    let expected = full_batch(generation, first, None)?;
    require(
        replies.status() == UntrustedStatus::AllAcknowledged
            && replies.requests()[..] == expected[..],
    )
}

fn readback(inventory: LocalPolicyInventory, table: &Option<TableMetadata>) -> Result<u64> {
    require(inventory == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn))?;
    let handle = table.as_ref().ok_or(REFUSE)?.handle;
    require(handle != 0)?;
    Ok(handle)
}

impl<'a> PreparedCreate<'a> {
    pub(in crate::kernel_observer) fn new(
        mut lease: LocalInventoryLease<'a>,
        deadline: Instant,
    ) -> Result<Self> {
        lease.deadline = lease.deadline.min(deadline);
        lease.recheck()?;
        require(lease.observed() == LocalPolicyInventory::TableAbsent && lease.table.is_none())?;
        let first = lease.session.next_sequence;
        let requests = full_batch(lease.generation, first, None)?;
        Ok(Self {
            lease,
            first,
            requests,
        })
    }

    /// The sole caller's durable Pending check must precede this operation.
    /// A callback here is only the already reviewed cfg(test) original sender;
    /// this internal injection seam does not authenticate arbitrary executors.
    pub(in crate::kernel_observer) fn execute(
        mut self,
        send: impl FnOnce(&mut LocalReadSession, AtomicBatch, Instant) -> Result<AtomicReplies>,
    ) -> Result<CreatedLease<'a>> {
        self.lease.recheck()?;
        let mut custody = InFlight(Some(self.lease.session));
        let deadline = self.lease.deadline;
        let result = (|| {
            let session = custody.0.as_deref_mut().ok_or(REFUSE)?;
            let replies = send(session, self.requests, deadline)?;
            acknowledged(&replies, self.lease.generation, self.first)?;
            session.check(deadline)?;
            // Reuse the actual complete parser, never a classification supplied
            // by the callback. No fresh one-second inventory allowance.
            let (inventory, generation, table) =
                session.inspect_policy_inventory_before(deadline)?;
            let handle = readback(inventory, &table)?;
            Ok((inventory, generation, table, handle))
        })();
        let (inventory, generation, table, handle) = result?;
        let session = custody.0.take().ok_or(REFUSE)?;
        let mut lease = LocalInventoryLease {
            session,
            inventory,
            generation,
            table,
            deadline,
        };
        lease.recheck()?;
        Ok(CreatedLease {
            lease,
            handle,
            completed: false,
        })
    }
}

/// Local fixture causality only, not Table::OwnedVerified or canonical authority.
/// The same owner remains exclusively borrowed through final revalidation.
pub(in crate::kernel_observer) struct CreatedLease<'a> {
    lease: LocalInventoryLease<'a>,
    handle: u64,
    completed: bool,
}
impl CreatedLease<'_> {
    pub(in crate::kernel_observer) fn finish(mut self) -> Result<u64> {
        self.lease.recheck()?;
        require(readback(self.lease.observed(), &self.lease.table)? == self.handle)?;
        self.completed = true;
        Ok(self.handle)
    }
}
impl Drop for CreatedLease<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.lease.session.poisoned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel_observer::{message, u32n};
    fn collector(generation: u32, old: Option<u64>, skip_end: bool) -> AtomicReplies {
        let batch = full_batch(generation, 100, old).unwrap();
        let mut replies = AtomicReplies::new(batch.clone(), 42).unwrap();
        for request in batch.iter().take(batch.len() - usize::from(skip_end)) {
            let bytes = message(
                2,
                0,
                u32n(&request[8..12]).unwrap(),
                42,
                &[0i32.to_ne_bytes().as_slice(), &request[..16]].concat(),
            );
            replies
                .receive(&bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
                .unwrap();
        }
        replies
    }
    #[test]
    fn complete_fixed_exclusive_ack_is_required_not_prefix_or_other_batch() {
        let empty = AtomicReplies::new(full_batch(7, 100, None).unwrap(), 42).unwrap();
        assert!(acknowledged(&empty, 7, 100).is_err());
        assert!(acknowledged(&collector(7, None, false), 7, 100).is_ok());
        assert!(acknowledged(&collector(7, None, true), 7, 100).is_err());
        assert!(acknowledged(&collector(7, Some(9), false), 7, 100).is_err());
        assert!(acknowledged(&collector(8, None, false), 7, 100).is_err());
        assert!(acknowledged(&collector(7, None, false), 7, 101).is_err());
        assert!(acknowledged(&collector(7, None, false), 0, 100).is_err());
    }
    #[test]
    fn generation_refusal_and_poison_never_are_positive_commits() {
        let batch = full_batch(7, 100, None).unwrap();
        let bytes = message(
            2,
            0x100,
            100,
            42,
            &[(-85i32).to_ne_bytes().as_slice(), &batch[0][..16]].concat(),
        );
        let mut replies = AtomicReplies::new(batch, 42).unwrap();
        replies
            .receive(&bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
            .unwrap();
        assert!(acknowledged(&replies, 7, 100).is_err());
        let mut positive = collector(7, None, false);
        positive.poison();
        assert!(acknowledged(&positive, 7, 100).is_err());
    }
    #[test]
    fn classification_without_complete_table_metadata_cannot_finish() {
        let mut table = Some(TableMetadata {
            flags: 6,
            uses: 1,
            handle: 9,
            owner: Some(42),
            userdata: None,
        });
        for inventory in [
            LocalPolicyInventory::TableAbsent,
            LocalPolicyInventory::OtherUntrusted,
            LocalPolicyInventory::ExactUntrusted(Policy::Emergency),
            LocalPolicyInventory::ExactUntrusted(Policy::FullVpn),
        ] {
            assert!(readback(inventory, &None).is_err());
            if inventory != LocalPolicyInventory::ExactUntrusted(Policy::FullVpn) {
                assert!(readback(inventory, &table).is_err());
            }
        }
        // Only the private projection helper is exercised, never CreatedLease.
        assert_eq!(
            readback(
                LocalPolicyInventory::ExactUntrusted(Policy::FullVpn),
                &table
            ),
            Ok(9)
        );
        if let Some(metadata) = &mut table {
            metadata.handle = 0;
        }
        assert!(
            readback(
                LocalPolicyInventory::ExactUntrusted(Policy::FullVpn),
                &table
            )
            .is_err()
        );
    }
}
