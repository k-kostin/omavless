// SPDX-License-Identifier: MIT
//! Test-only mechanism: fixed table-handle deletion from a retained complete
//! inventory, atomically fenced by the kernel's ruleset generation. This is NOT
//! canonical-host, durable creator, effect-port or production disarm authority.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DeleteOutcome {
    AcknowledgedAndAbsent,
    GenerationChanged,
    Unknown,
    RefusedBeforeSend,
}

/// Exclusive borrowed session, not reconstructible from serialized labels.
/// Drop/cancel does nothing. Consume never retries an uncertain datagram.
pub(super) struct InventoryDelete<'a> {
    session: &'a mut LocalReadSession,
    generation: u32,
    handle: u64,
    deadline: Instant,
}

impl LocalReadSession {
    pub(super) fn prepare_inventory_delete(&mut self) -> Result<InventoryDelete<'_>> {
        let deadline = Instant::now() + Duration::from_secs(1);
        let prepared = self.inspect_policy_inventory_once();
        let (inventory, generation, table) = match prepared {
            Ok(v) => v,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        require(matches!(inventory, LocalPolicyInventory::ExactUntrusted(_)))?;
        let table = table.ok_or(REFUSE)?;
        self.check(deadline)?;
        Ok(InventoryDelete {
            session: self,
            generation,
            handle: table.handle,
            deadline,
        })
    }
}

struct Wire {
    begin: Vec<u8>,
    delete: Vec<u8>,
    end: Vec<u8>,
    batch: Vec<u8>,
}
fn encode(generation: u32, handle: u64, first: u32) -> Result<Wire> {
    // Zero disables the kernel condition. Never encode it, or family UNSPEC
    // (which would turn a missing target into a ruleset-wide flush).
    require(generation != 0 && handle != 0 && first != 0 && first.checked_add(3).is_some())?;
    let begin = message(
        16,
        5,
        first,
        0,
        &[vec![0, 0, 0, 10], attribute(1, &generation.to_be_bytes())].concat(),
    );
    let delete = message(
        NFT + 2,
        5,
        first + 1,
        0,
        &[vec![1, 0, 0, 0], attribute(4, &handle.to_be_bytes())].concat(),
    );
    // BEGIN/operation successes can be queued even when final commit fails.
    // Only END success is emitted after ss->commit succeeds.
    let end = message(17, 5, first + 2, 0, &[0, 0, 0, 10]);
    let batch = [begin.clone(), delete.clone(), end.clone()].concat();
    Ok(Wire {
        begin,
        delete,
        end,
        batch,
    })
}

struct Replies {
    wire: Wire,
    port: u32,
    begin_ack: bool,
    delete_ack: bool,
    end_ack: bool,
    changed: bool,
    poisoned: bool,
    bytes: usize,
    datagrams: usize,
}
impl Replies {
    fn new(wire: Wire, port: u32) -> Result<Self> {
        require(port != 0)?;
        Ok(Self {
            wire,
            port,
            begin_ack: false,
            delete_ack: false,
            end_ack: false,
            changed: false,
            poisoned: false,
            bytes: 0,
            datagrams: 0,
        })
    }
    fn complete(&self) -> bool {
        !self.poisoned && (self.changed || (self.begin_ack && self.delete_ack && self.end_ack))
    }
    fn receive(
        &mut self,
        bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        if self.poisoned || self.complete() {
            self.poisoned = true;
            return Err(REFUSE);
        }
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
        require(sender == Some(NetlinkAddr::new(0, 0)) && flags.is_empty())?;
        self.bytes = self.bytes.checked_add(bytes.len()).ok_or(REFUSE)?;
        self.datagrams += 1;
        require(!bytes.is_empty() && self.bytes <= LIMIT && self.datagrams <= 8)?;
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
            let sequence = u32n(&bytes[8..12])?;
            let first = u32n(&self.wire.begin[8..12])?;
            let body = &bytes[16..length];
            let code = i32::from_ne_bytes(body[..4].try_into().map_err(|_| REFUSE)?);
            let original = if sequence == first {
                &self.wire.begin
            } else if sequence == first + 1 {
                &self.wire.delete
            } else {
                require(sequence == first + 2)?;
                &self.wire.end
            };
            // Capped errors and success echo only the original header; uncapped
            // errors must echo the entire exact request. No extack/text parsing.
            let echoed = if code == 0 || flags == 0x100 {
                &original[..16]
            } else {
                &original[..]
            };
            require(&body[4..] == echoed)?;
            if code == -85 {
                // ERESTART: kernel rejected generation before batch operations.
                require(
                    sequence == first
                        && !self.begin_ack
                        && !self.delete_ack
                        && !self.end_ack
                        && aligned(length) == bytes.len(),
                )?;
                self.changed = true;
            } else {
                require(code == 0)?;
                if sequence == first {
                    require(!self.begin_ack)?;
                    self.begin_ack = true;
                } else if sequence == first + 1 {
                    require(!self.delete_ack)?;
                    self.delete_ack = true;
                } else {
                    require(!self.end_ack)?;
                    self.end_ack = true;
                }
            }
            bytes = &bytes[aligned(length)..];
        }
        Ok(())
    }
}

impl InventoryDelete<'_> {
    pub(super) fn handle_for_fixture(&self) -> u64 {
        self.handle
    }
    pub(super) fn cancel(self) {}

    pub(super) fn consume(self) -> DeleteOutcome {
        self.consume_with_receive_fault(&mut receive_truncation::OneShotTruncation::default())
    }

    // Private cfg(test) path only. Ordinary consume keeps its external shape
    // and full-capacity receives. No alternate send, retry or readback receipt.
    pub(super) fn consume_with_receive_fault(
        self,
        receive_fault: &mut receive_truncation::OneShotTruncation,
    ) -> DeleteOutcome {
        self.consume_with_ack_faults(
            receive_fault,
            &mut end_ack_loss::OneShotEndAckLoss::default(),
        )
    }
    pub(super) fn consume_with_ack_faults(
        self,
        receive_fault: &mut receive_truncation::OneShotTruncation,
        end_ack_loss: &mut end_ack_loss::OneShotEndAckLoss,
    ) -> DeleteOutcome {
        let deadline = self.deadline;
        if self.session.check(deadline).is_err() {
            self.session.poisoned = true;
            return DeleteOutcome::RefusedBeforeSend;
        }
        let first = self.session.next_sequence;
        let Ok(wire) = encode(self.generation, self.handle, first) else {
            self.session.poisoned = true;
            return DeleteOutcome::RefusedBeforeSend;
        };
        self.session.next_sequence = first + 3;
        let Ok(mut replies) = Replies::new(wire, self.session.local.pid()) else {
            self.session.poisoned = true;
            return DeleteOutcome::RefusedBeforeSend;
        };
        // Encoding/collector work may consume the budget. Recheck identity and
        // the clock after that work, immediately before the first syscall.
        if self.session.check(deadline).is_err() || Instant::now() >= deadline {
            self.session.poisoned = true;
            return DeleteOutcome::RefusedBeforeSend;
        }
        // After the first syscall, even an error is unknown. No resend, fresh
        // generation acquisition or other cleanup mutation follows an unknown.
        let result = (|| -> Result<()> {
            require(
                sendto(
                    self.session.socket.as_raw_fd(),
                    &replies.wire.batch,
                    &NetlinkAddr::new(0, 0),
                    MsgFlags::MSG_DONTWAIT,
                )
                .map_err(|_| REFUSE)?
                    == replies.wire.batch.len(),
            )?;
            while !replies.complete() {
                self.session.check(deadline)?;
                let mut bytes = [0; LIMIT];
                let capacity = receive_fault.capacity(self.session.socket.as_raw_fd())?;
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
                        receive_fault.received(length, flags)?;
                        let (delivered, lost_end) = end_ack_loss.deliver(
                            self.session.socket.as_raw_fd(),
                            &bytes[..length],
                            sender,
                            flags,
                            &replies.wire.end,
                            self.session.local.pid(),
                        )?;
                        if !delivered.is_empty() || !lost_end {
                            replies.receive(&delivered, sender, flags)?;
                        }
                        if lost_end {
                            require(
                                !replies.poisoned
                                    && !replies.changed
                                    && !replies.complete()
                                    && replies.begin_ack
                                    && replies.delete_ack
                                    && !replies.end_ack,
                            )?;
                            end_ack_loss.confirm_prefix();
                            replies.poisoned = true;
                            require(!replies.complete())?;
                            return Err(REFUSE);
                        }
                    }
                    Err(nix::errno::Errno::EAGAIN) => std::thread::sleep(Duration::from_millis(1)),
                    Err(_) => return Err(REFUSE),
                }
            }
            self.session.check(deadline)?;
            Ok(())
        })();
        if result.is_err() {
            self.session.poisoned = true;
            return DeleteOutcome::Unknown;
        }
        if replies.changed {
            // Old witness is consumed. Explicit refusal is not fresh authority.
            self.session.poisoned = true;
            return DeleteOutcome::GenerationChanged;
        }
        // A fresh bounded kernel exchange is a readback/barrier, never a retry.
        match self.session.inspect_policy_inventory() {
            Ok(LocalPolicyInventory::TableAbsent) => DeleteOutcome::AcknowledgedAndAbsent,
            _ => {
                self.session.poisoned = true;
                DeleteOutcome::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PORT: u32 = 42;
    fn collector() -> Replies {
        Replies::new(encode(7, 9, 100).unwrap(), PORT).unwrap()
    }
    fn ack(request: &[u8], code: i32, capped: bool) -> Vec<u8> {
        let original = if capped || code == 0 {
            &request[..16]
        } else {
            request
        };
        message(
            2,
            if capped { 0x100 } else { 0 },
            u32n(&request[8..12]).unwrap(),
            PORT,
            &[code.to_ne_bytes().to_vec(), original.to_vec()].concat(),
        )
    }
    fn push(c: &mut Replies, bytes: &[u8]) -> Result<()> {
        c.receive(bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
    }
    #[test]
    fn nonzero_bounded_single_handle_wire_never_uses_name_or_unspec_flush() {
        let wire = encode(0x01020304, 0x0102030405060708, 5).unwrap();
        assert_eq!(&wire.begin[24..28], &[1, 2, 3, 4]);
        assert_eq!(
            &wire.delete[16..],
            &[1, 0, 0, 0, 12, 0, 4, 0, 1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(wire.batch.len(), 28 + 32 + 20);
        for (generation, handle, seq) in [(0, 1, 1), (1, 0, 1), (1, 1, 0), (1, 1, u32::MAX - 2)] {
            assert!(encode(generation, handle, seq).is_err());
        }
    }
    #[test]
    fn exact_begin_delete_and_commit_end_ack_required_in_any_order() {
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut c = collector();
            let a = [
                ack(&c.wire.begin, 0, false),
                ack(&c.wire.delete, 0, true),
                ack(&c.wire.end, 0, false),
            ];
            push(&mut c, &a[order[0]]).unwrap();
            assert!(!c.complete());
            push(&mut c, &a[order[1]]).unwrap();
            assert!(!c.complete());
            push(&mut c, &a[order[2]]).unwrap();
            assert!(c.complete());
            assert!(push(&mut c, &a[0]).is_err());
            assert!(!c.complete());
        }
    }
    #[test]
    fn queued_operation_success_without_final_commit_never_completes() {
        let mut c = collector();
        let begin = ack(&c.wire.begin, 0, false);
        let delete = ack(&c.wire.delete, 0, false);
        push(&mut c, &[begin, delete].concat()).unwrap();
        assert!(!c.complete()); // a lost commit-error cannot become success
        let commit_error = ack(&c.wire.begin, -22, false);
        assert!(push(&mut c, &commit_error).is_err());
        assert!(!c.complete());
        let mut c = collector();
        let failed_end = ack(&c.wire.end, -22, false);
        assert!(push(&mut c, &failed_end).is_err());
    }
    #[test]
    fn final_commit_ack_truncation_forgery_or_duplicate_poison() {
        let c = collector();
        let end = ack(&c.wire.end, 0, false);
        for length in 0..end.len() {
            assert!(push(&mut collector(), &end[..length]).is_err());
        }
        for offset in [0, 4, 6, 8, 12, 16, 20, 24, 28, 32] {
            let mut bad = end.clone();
            bad[offset] ^= 1;
            assert!(push(&mut collector(), &bad).is_err());
        }
        let mut c = collector();
        push(&mut c, &end).unwrap();
        assert!(!c.complete());
        assert!(push(&mut c, &end).is_err());
        assert!(!c.complete());
    }
    #[test]
    fn exact_restart_of_begin_only_is_changed_all_other_errors_unknown() {
        for capped in [false, true] {
            let mut c = collector();
            let bytes = ack(&c.wire.begin, -85, capped);
            push(&mut c, &bytes).unwrap();
            assert!(c.complete() && c.changed);
            for code in [-2, -1, -22, -95] {
                let mut c = collector();
                let bytes = ack(&c.wire.begin, code, capped);
                assert!(push(&mut c, &bytes).is_err());
            }
            let mut c = collector();
            let bytes = ack(&c.wire.delete, -85, capped);
            assert!(push(&mut c, &bytes).is_err());
        }
    }
    #[test]
    fn forged_duplicate_truncated_wrong_peer_and_trailing_changed_refuse() {
        let c = collector();
        let original = ack(&c.wire.begin, -85, false);
        for end in 0..original.len() {
            assert!(push(&mut collector(), &original[..end]).is_err());
        }
        for offset in [0, 4, 6, 8, 12, 16, 20, 24] {
            let mut bad = original.clone();
            bad[offset] ^= 1;
            assert!(push(&mut collector(), &bad).is_err());
        }
        let mut c = collector();
        assert!(
            c.receive(&original, Some(NetlinkAddr::new(1, 0)), MsgFlags::empty())
                .is_err()
        );
        let mut c = collector();
        assert!(
            c.receive(&original, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_TRUNC)
                .is_err()
        );
        assert!(push(&mut collector(), &[original.clone(), original].concat()).is_err());
        let mut c = collector();
        let a = ack(&c.wire.begin, 0, false);
        push(&mut c, &a).unwrap();
        assert!(push(&mut c, &a).is_err());
        let mut c = collector();
        c.bytes = LIMIT;
        let a = ack(&c.wire.begin, 0, false);
        assert!(push(&mut c, &a).is_err());
    }

    #[test]
    fn receive_truncation_flags_poison_delete_collector_without_later_repair() {
        for flags in [
            MsgFlags::MSG_TRUNC,
            MsgFlags::MSG_CTRUNC,
            MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC,
        ] {
            let mut c = collector();
            let a = ack(&c.wire.begin, 0, false);
            assert!(c.receive(&a, Some(NetlinkAddr::new(0, 0)), flags).is_err());
            assert!(c.poisoned && !c.complete());
            for request in [&c.wire.begin, &c.wire.delete, &c.wire.end].map(|r| ack(r, 0, false)) {
                assert!(push(&mut c, &request).is_err());
            }
            assert!(!c.complete());
        }
    }
}
