// SPDX-License-Identifier: MIT
//! Disposable-VM test composition only. The epoch below is fixture vocabulary,
//! never canonical-host authority. No production EffectPort is implemented.
use super::*;
use crate::effect_port::{EffectError, EffectIdentity, EffectPort, EffectSnapshot, sealed};
use crate::policy::Policy;
use crate::receipt::HostEpoch;
use crate::transaction::Table;

use super::atomic_batch::{AtomicBatch, AtomicReplies as BatchReplies, full_batch};

/// Exclusive causality remains in this live object, never reconstructed from a
/// receipt, owner-port, handle or matching shape. Epoch is explicitly synthetic.
struct FixtureCreator {
    session: LocalReadSession,
    epoch: HostEpoch,
    created: Option<u64>,
    effects: usize,
    lose_reply: bool,
    receive_fault: receive_truncation::OneShotTruncation,
    end_ack_loss: end_ack_loss::OneShotEndAckLoss,
    prefix_ack_loss: prefix_ack_loss::OneShotPrefixAckLoss,
    send_cut: send_return_cut::OneShotSendCut,
    cut_after_effect: bool,
    change_generation_before_send: bool,
    retained_generation_cut: Option<Box<FixtureCreator>>,
    generation_refused: bool,
    state_parent: std::path::PathBuf,
}
impl FixtureCreator {
    fn open(state_parent: std::path::PathBuf) -> Result<Self> {
        let session = LocalReadSession::open()?;
        let (device, inode) = session.identity;
        Ok(Self {
            session,
            epoch: HostEpoch {
                boot: [0x31; 16],
                namespace_epoch: [0x32; 16],
                namespace_device: device,
                namespace_inode: inode,
            },
            created: None,
            effects: 0,
            lose_reply: false,
            receive_fault: receive_truncation::OneShotTruncation::default(),
            end_ack_loss: end_ack_loss::OneShotEndAckLoss::default(),
            prefix_ack_loss: prefix_ack_loss::OneShotPrefixAckLoss::default(),
            send_cut: send_return_cut::OneShotSendCut::default(),
            cut_after_effect: false,
            change_generation_before_send: false,
            retained_generation_cut: None,
            generation_refused: false,
            state_parent,
        })
    }
    fn id(&self, handle: u64) -> EffectIdentity {
        EffectIdentity {
            boot: self.epoch.boot,
            netns_inode: self.epoch.namespace_inode,
            table_handle: handle,
        }
    }
    fn fail(&mut self) -> EffectError {
        self.session.poisoned = true;
        self.created = None;
        EffectError::UnavailableOrUncertain
    }
    fn inspect(&mut self) -> Result<(LocalPolicyInventory, u32, Option<TableMetadata>)> {
        let result = self.session.inspect_policy_inventory_once();
        if result.is_err() {
            self.session.poisoned = true;
            self.created = None;
        }
        result
    }
    fn pending(&self, phase: &str) {
        Self::pending_at(&self.state_parent, phase)
    }
    fn pending_at(state_parent: &std::path::Path, phase: &str) {
        let root = state_parent.join("omavless-netguard");
        let bytes = std::fs::read(root.join("table-receipt-v1.json")).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["phase"], phase);
        if phase == "pending_delete" {
            let marker: serde_json::Value =
                serde_json::from_slice(&std::fs::read(root.join("armed-v1.json")).unwrap())
                    .unwrap();
            assert_eq!(marker["armed"], false);
            assert_eq!(marker["generation"], 7);
        }
        let meta = std::fs::metadata(state_parent).unwrap();
        assert!(matches!(
            crate::root_state::RootStateStore::open_test_parent(
                File::open(state_parent).unwrap(),
                (meta.uid(), meta.gid()),
                1001
            ),
            Err(crate::root_state::StateError::Busy)
        ));
    }
    fn cut(&self) {
        if self.cut_after_effect {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.state_parent.join(".effect-cut.next"))
                .unwrap();
            file.write_all(b"K1_EFFECT_CUT\n").unwrap();
            file.sync_all().unwrap();
            std::fs::rename(
                self.state_parent.join(".effect-cut.next"),
                self.state_parent.join("effect-cut"),
            )
            .unwrap();
            // The holder kills/reaps this exact process; timeout is a failing
            // fallback, never a successful crash receipt or normal retry.
            std::thread::sleep(Duration::from_secs(10));
            panic!("isolated crash checkpoint was not killed");
        }
    }
    fn send_batch(&mut self, requests: AtomicBatch, deadline: Instant) -> Result<()> {
        BatchSender {
            session: &mut self.session,
            effects: &mut self.effects,
            receive_fault: &mut self.receive_fault,
            end_ack_loss: &mut self.end_ack_loss,
            prefix_ack_loss: &mut self.prefix_ack_loss,
            send_cut: &mut self.send_cut,
            generation_refused: &mut self.generation_refused,
        }
        .send_batch(requests, deadline)
    }
    fn full(
        &mut self,
        old: Option<EffectIdentity>,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        let result = (|| -> Result<EffectIdentity> {
            let deadline = Instant::now() + Duration::from_secs(1);
            let expected = old.map(|id| self.id(id.table_handle));
            let mut lease = self.session.borrow_policy_inventory()?;
            let inventory = lease.observed();
            let table = &lease.table;
            match old {
                None => require(
                    inventory == LocalPolicyInventory::TableAbsent && self.created.is_none(),
                )?,
                Some(id) => require(
                    inventory == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn)
                        && self.created == Some(id.table_handle)
                        && Some(id) == expected
                        && table.as_ref().is_some_and(|t| t.handle == id.table_handle),
                )?,
            }
            Self::pending_at(
                &self.state_parent,
                if old.is_some() {
                    "pending_replace"
                } else {
                    "pending_create"
                },
            );
            if self.change_generation_before_send {
                tests::fixed_foreign_change();
            }
            if let Some(cut) = &mut self.retained_generation_cut {
                // Fixed cfg(test)-only second socket, retained even on failure.
                // No subprocess, arbitrary rule, cleanup or receipt adoption.
                require(cut.effects == 0 && cut.retained_generation_cut.is_none())?;
                cut.send_batch(
                    atomic_batch::lease_generation_cut_batch(
                        lease.generation,
                        cut.session.next_sequence,
                    )?,
                    deadline.min(lease.deadline),
                )?;
            }
            if old.is_none() {
                // Only this existing cfg(test) path, after durable Pending,
                // can exercise the extracted original-session create witness.
                let prepared = inventory::create_witness::PreparedCreate::new(lease, deadline)?;
                let witness = prepared.execute(|session, requests, allowance| {
                    BatchSender {
                        session,
                        effects: &mut self.effects,
                        receive_fault: &mut self.receive_fault,
                        end_ack_loss: &mut self.end_ack_loss,
                        prefix_ack_loss: &mut self.prefix_ack_loss,
                        send_cut: &mut self.send_cut,
                        generation_refused: &mut self.generation_refused,
                    }
                    .send_batch_replies(requests, allowance)
                })?;
                let handle = witness.finish()?;
                self.created = Some(handle);
                self.cut();
                require(!self.lose_reply)?;
                return Ok(self.id(handle));
            }
            lease.recheck()?;
            let requests = full_batch(
                lease.generation,
                lease.session.next_sequence,
                old.map(|id| id.table_handle),
            )?;
            BatchSender {
                session: lease.session,
                effects: &mut self.effects,
                receive_fault: &mut self.receive_fault,
                end_ack_loss: &mut self.end_ack_loss,
                prefix_ack_loss: &mut self.prefix_ack_loss,
                send_cut: &mut self.send_cut,
                generation_refused: &mut self.generation_refused,
            }
            .send_batch(requests, deadline.min(lease.deadline))?;
            let (inventory, _, table) = self.inspect()?;
            require(inventory == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn))?;
            let handle = table.ok_or(REFUSE)?.handle;
            require(old.is_none_or(|id| id.table_handle != handle))?;
            self.created = Some(handle);
            self.cut();
            // Deliberately withhold the adapter result AFTER real ACK/readback;
            // this models upper-layer loss, not dropped raw kernel ACK packets.
            require(!self.lose_reply)?;
            Ok(self.id(handle))
        })();
        result.map_err(|_| self.fail())
    }
}
impl sealed::Sealed for FixtureCreator {}
impl EffectPort for FixtureCreator {
    fn observe(&mut self) -> std::result::Result<EffectSnapshot, EffectError> {
        let result = (|| -> Result<EffectSnapshot> {
            let (inventory, _, table) = self.inspect()?;
            Ok(match inventory {
                LocalPolicyInventory::TableAbsent if self.created.is_none() => EffectSnapshot {
                    table: Table::Absent,
                    identity: None,
                },
                LocalPolicyInventory::ExactUntrusted(Policy::FullVpn)
                    if table
                        .as_ref()
                        .is_some_and(|t| Some(t.handle) == self.created) =>
                {
                    EffectSnapshot {
                        table: Table::OwnedVerified(Policy::FullVpn),
                        identity: Some(self.id(table.ok_or(REFUSE)?.handle)),
                    }
                }
                _ => return Err(REFUSE),
            })
        })();
        result.map_err(|_| self.fail())
    }
    fn create_if_absent(
        &mut self,
        policy: Policy,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        if policy != Policy::FullVpn {
            return Err(self.fail());
        }
        self.full(None)
    }
    fn replace_owned(
        &mut self,
        id: EffectIdentity,
        policy: Policy,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        if policy != Policy::FullVpn {
            return Err(self.fail());
        }
        self.full(Some(id))
    }
    fn delete_owned(&mut self, id: EffectIdentity) -> std::result::Result<(), EffectError> {
        let result = (|| -> Result<()> {
            require(self.created == Some(id.table_handle) && id == self.id(id.table_handle))?;
            self.pending("pending_delete");
            let witness = self.session.prepare_inventory_delete()?;
            require(witness.handle_for_fixture() == id.table_handle)?;
            self.effects += 1;
            require(
                witness.consume_with_send_cut(
                    &mut self.receive_fault,
                    &mut self.end_ack_loss,
                    &mut self.prefix_ack_loss,
                    &mut self.send_cut,
                ) == conditional_delete::DeleteOutcome::AcknowledgedAndAbsent,
            )?;
            self.created = None;
            self.cut();
            require(!self.lose_reply)
        })();
        result.map_err(|_| self.fail())
    }
}

#[cfg(test)]
#[path = "kernel_creator_lifecycle_tests.rs"]
mod tests;

#[path = "kernel_manager_private_fixture.rs"]
mod manager_private;

// Literal-only new developer tuple. Same existing creator, never a second API.
#[path = "kernel_private_admission_fixture.rs"]
mod private_admission;

#[path = "kernel_response_diagnostic_fixture.rs"]
mod response_diagnostic;

#[path = "kernel_retained_lease_fixture.rs"]
mod retained_lease;

/// Split borrows keep the real readback's exclusive session borrow across the
/// conditional send; fault injection and counters remain fixture-only.
struct BatchSender<'a> {
    session: &'a mut LocalReadSession,
    effects: &'a mut usize,
    receive_fault: &'a mut receive_truncation::OneShotTruncation,
    end_ack_loss: &'a mut end_ack_loss::OneShotEndAckLoss,
    prefix_ack_loss: &'a mut prefix_ack_loss::OneShotPrefixAckLoss,
    send_cut: &'a mut send_return_cut::OneShotSendCut,
    generation_refused: &'a mut bool,
}
impl BatchSender<'_> {
    fn send_batch(&mut self, requests: AtomicBatch, deadline: Instant) -> Result<()> {
        self.send_batch_replies(requests, deadline).map(|_| ())
    }
    fn send_batch_replies(
        &mut self,
        requests: AtomicBatch,
        deadline: Instant,
    ) -> Result<BatchReplies> {
        self.session.next_sequence = self
            .session
            .next_sequence
            .checked_add(requests.len() as u32)
            .ok_or(REFUSE)?;
        let batch = requests.concat();
        let mut replies = BatchReplies::new(requests, self.session.local.pid())?;
        self.session.check(deadline)?;
        require(Instant::now() < deadline)?;
        // From this syscall onward uncertainty is terminal. There is no retry.
        *self.effects += 1;
        require(
            sendto(
                self.session.socket.as_raw_fd(),
                &batch,
                &NetlinkAddr::new(0, 0),
                MsgFlags::MSG_DONTWAIT,
            )
            .map_err(|_| REFUSE)?
                == batch.len(),
        )?;
        self.send_cut
            .after_send(self.session.socket.as_raw_fd(), &replies)?;
        while !replies.complete() {
            self.session.check(deadline)?;
            let mut bytes = [0; LIMIT];
            let capacity = self
                .receive_fault
                .capacity(self.session.socket.as_raw_fd())?;
            let mut iov = [IoSliceMut::new(&mut bytes[..capacity])];
            match recvmsg::<NetlinkAddr>(
                self.session.socket.as_raw_fd(),
                &mut iov,
                None,
                MsgFlags::MSG_DONTWAIT,
            ) {
                Ok(reply) => {
                    let (length, sender, flags) = (reply.bytes, reply.address, reply.flags);
                    require(length <= capacity)?;
                    self.receive_fault.received(length, flags)?;
                    // These are the real received bytes/sender/flags. In the
                    // one-byte fault case the existing collector must refuse.
                    let (delivered, lost_end) = self.end_ack_loss.deliver(
                        self.session.socket.as_raw_fd(),
                        &bytes[..length],
                        sender,
                        flags,
                        replies.requests().last().ok_or(REFUSE)?,
                        self.session.local.pid(),
                    )?;
                    let prefix_request = replies
                        .requests()
                        .get(self.prefix_ack_loss.target().unwrap_or(0))
                        .ok_or(REFUSE)?;
                    let consumed_before = self.prefix_ack_loss.observed().0;
                    let delivered = self.prefix_ack_loss.deliver(
                        self.session.socket.as_raw_fd(),
                        &delivered,
                        sender,
                        flags,
                        prefix_request,
                        self.session.local.pid(),
                    )?;
                    let lost_prefix = self.prefix_ack_loss.observed().0 != consumed_before;
                    if !delivered.is_empty() || (!lost_end && !lost_prefix) {
                        replies.receive(&delivered, sender, flags)?;
                    }
                    if replies.finish_prefix_loss(self.prefix_ack_loss)? {
                        return Err(REFUSE);
                    }
                    if lost_end {
                        require(
                            !replies.poisoned()
                                && !replies.changed()
                                && !replies.complete()
                                && !replies.acks().last().copied().ok_or(REFUSE)?
                                && replies.acks()[..replies.acks().len() - 1]
                                    .iter()
                                    .all(|ack| *ack),
                        )?;
                        self.end_ack_loss.confirm_prefix();
                        replies.poison();
                        require(!replies.complete())?;
                        // Actual END was consumed by this observer, not
                        // delivered to the collector. No timeout or retry.
                        return Err(REFUSE);
                    }
                }
                Err(nix::errno::Errno::EAGAIN) => std::thread::sleep(Duration::from_millis(1)),
                Err(_) => return Err(REFUSE),
            }
        }
        self.session.check(deadline)?;
        require(Instant::now() < deadline)?;
        *self.generation_refused = replies.changed();
        require(!replies.changed())?;
        Ok(replies)
    }
}
