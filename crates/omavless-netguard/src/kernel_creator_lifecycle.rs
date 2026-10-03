// SPDX-License-Identifier: MIT
//! Disposable-VM test composition only. The epoch below is fixture vocabulary,
//! never canonical-host authority. No production EffectPort is implemented.
use super::*;
use crate::effect_port::{EffectError, EffectIdentity, EffectPort, EffectSnapshot, sealed};
use crate::policy::Policy;
use crate::receipt::HostEpoch;
use crate::transaction::Table;

/// Build one atomic fixed FullVpn create or delete+exclusive-create replacement.
/// All messages require ACKs, especially END (commit), with no separate GETGEN
/// used to turn queued operation replies into an effect receipt.
fn full_batch(generation: u32, first: u32, old: Option<u64>) -> Result<Vec<Vec<u8>>> {
    require(generation != 0 && first != 0 && first.checked_add(15).is_some())?;
    require(old != Some(0))?;
    let wire = crate::full_vpn_wire::encode(generation, first).map_err(|_| REFUSE)?;
    let mut requests = Vec::new();
    let mut rest = wire.batch();
    while !rest.is_empty() {
        let length = usize::try_from(u32n(&rest[..4])?).map_err(|_| REFUSE)?;
        let mut request = rest[..aligned(length)].to_vec();
        let flags = u16n(&request[6..8])? | 4;
        request[6..8].copy_from_slice(&flags.to_ne_bytes());
        if requests.len() == 1
            && let Some(handle) = old
        {
            requests.push(message(
                NFT + 2,
                5,
                1,
                0,
                &[vec![1, 0, 0, 0], attribute(4, &handle.to_be_bytes())].concat(),
            ));
        }
        requests.push(request);
        rest = &rest[aligned(length)..];
    }
    for (index, request) in requests.iter_mut().enumerate() {
        request[8..12].copy_from_slice(&(first + index as u32).to_ne_bytes());
    }
    Ok(requests)
}

/// Exact begin/each-operation/end receipt. All unknown errors poison; the only
/// definitive refusal is generation ERESTART on BEGIN before any success.
struct BatchReplies {
    requests: Vec<Vec<u8>>,
    acks: Vec<bool>,
    port: u32,
    changed: bool,
    poisoned: bool,
    total: usize,
    datagrams: usize,
}
impl BatchReplies {
    fn new(requests: Vec<Vec<u8>>, port: u32) -> Result<Self> {
        require(port != 0 && matches!(requests.len(), 14 | 15))?;
        Ok(Self {
            acks: vec![false; requests.len()],
            requests,
            port,
            changed: false,
            poisoned: false,
            total: 0,
            datagrams: 0,
        })
    }
    fn complete(&self) -> bool {
        !self.poisoned && (self.changed || self.acks.iter().all(|ack| *ack))
    }
    fn receive(
        &mut self,
        bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        let result = self.receive_inner(bytes, sender, flags);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn receive_inner(
        &mut self,
        mut bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        require(
            !self.poisoned
                && !self.complete()
                && sender == Some(NetlinkAddr::new(0, 0))
                && flags.is_empty(),
        )?;
        self.total = self.total.checked_add(bytes.len()).ok_or(REFUSE)?;
        self.datagrams += 1;
        require(!bytes.is_empty() && self.total <= LIMIT && self.datagrams <= 16)?;
        let first = u32n(&self.requests[0][8..12])?;
        while !bytes.is_empty() {
            require(!self.changed && bytes.len() >= 16)?;
            let length = usize::try_from(u32n(&bytes[..4])?).map_err(|_| REFUSE)?;
            require(
                length >= 36
                    && aligned(length) <= bytes.len()
                    && bytes[length..aligned(length)].iter().all(|v| *v == 0)
                    && u16n(&bytes[4..6])? == 2
                    && u32n(&bytes[12..16])? == self.port,
            )?;
            let flags = u16n(&bytes[6..8])?;
            require(matches!(flags, 0 | 0x100))?;
            let index = usize::try_from(u32n(&bytes[8..12])?.checked_sub(first).ok_or(REFUSE)?)
                .map_err(|_| REFUSE)?;
            let original = self.requests.get(index).ok_or(REFUSE)?;
            require(!self.acks[index])?;
            let body = &bytes[16..length];
            let code = i32::from_ne_bytes(body[..4].try_into().map_err(|_| REFUSE)?);
            let echoed = if code == 0 || flags == 0x100 {
                &original[..16]
            } else {
                original
            };
            require(&body[4..] == echoed)?;
            if code == -85 {
                require(
                    index == 0 && self.acks.iter().all(|a| !*a) && aligned(length) == bytes.len(),
                )?;
                self.changed = true;
            } else {
                require(code == 0)?;
                self.acks[index] = true;
            }
            bytes = &bytes[aligned(length)..];
        }
        Ok(())
    }
}

/// Exclusive causality remains in this live object, never reconstructed from a
/// receipt, owner-port, handle or matching shape. Epoch is explicitly synthetic.
struct FixtureCreator {
    session: LocalReadSession,
    epoch: HostEpoch,
    created: Option<u64>,
    effects: usize,
    lose_reply: bool,
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
        let root = self.state_parent.join("omavless-netguard");
        let bytes = std::fs::read(root.join("table-receipt-v1.json")).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["phase"], phase);
        let meta = std::fs::metadata(&self.state_parent).unwrap();
        assert!(matches!(
            crate::root_state::RootStateStore::open_test_parent(
                File::open(&self.state_parent).unwrap(),
                (meta.uid(), meta.gid()),
                1001
            ),
            Err(crate::root_state::StateError::Busy)
        ));
    }
    fn send_full(&mut self, generation: u32, old: Option<u64>, deadline: Instant) -> Result<()> {
        let requests = full_batch(generation, self.session.next_sequence, old)?;
        self.session.next_sequence = self
            .session
            .next_sequence
            .checked_add(requests.len() as u32)
            .ok_or(REFUSE)?;
        let batch = requests.concat();
        let mut replies = BatchReplies::new(requests, self.session.local.pid())?;
        self.session.check(deadline)?;
        // From this syscall onward uncertainty is terminal. There is no retry.
        self.effects += 1;
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
        while !replies.complete() {
            self.session.check(deadline)?;
            let mut bytes = [0; LIMIT];
            let mut iov = [IoSliceMut::new(&mut bytes)];
            match recvmsg::<NetlinkAddr>(
                self.session.socket.as_raw_fd(),
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
        self.session.check(deadline)?;
        require(!replies.changed)
    }
    fn full(
        &mut self,
        old: Option<EffectIdentity>,
    ) -> std::result::Result<EffectIdentity, EffectError> {
        let result = (|| -> Result<EffectIdentity> {
            let deadline = Instant::now() + Duration::from_secs(1);
            let (inventory, generation, table) = self.inspect()?;
            match old {
                None => require(
                    inventory == LocalPolicyInventory::TableAbsent && self.created.is_none(),
                )?,
                Some(id) => require(
                    inventory == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn)
                        && self.created == Some(id.table_handle)
                        && id == self.id(id.table_handle)
                        && table.as_ref().is_some_and(|t| t.handle == id.table_handle),
                )?,
            }
            self.pending(if old.is_some() {
                "pending_replace"
            } else {
                "pending_create"
            });
            self.send_full(generation, old.map(|id| id.table_handle), deadline)?;
            let (inventory, _, table) = self.inspect()?;
            require(inventory == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn))?;
            let handle = table.ok_or(REFUSE)?.handle;
            require(old.is_none_or(|id| id.table_handle != handle))?;
            self.created = Some(handle);
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
                _ => EffectSnapshot {
                    table: Table::Foreign,
                    identity: None,
                },
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
            require(witness.consume() == conditional_delete::DeleteOutcome::AcknowledgedAndAbsent)?;
            self.created = None;
            require(!self.lose_reply)
        })();
        result.map_err(|_| self.fail())
    }
}

#[cfg(test)]
#[path = "kernel_creator_lifecycle_tests.rs"]
mod tests;
