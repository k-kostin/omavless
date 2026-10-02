//! Inactive, bounded chain inventory for the fixed table in the calling
//! namespace. A complete chain dump is still not rule readback or ownership.

use super::*;
use nix::sys::socket::{NetlinkAddr, recvmsg, sendto};
use std::collections::BTreeSet;

const GET_CHAIN: u16 = NFT + 4;
const NEW_CHAIN: u16 = NFT + 3;
const DONE: u16 = 3;
const REQUEST_DUMP: u16 = 0x301;
const MULTI: u16 = 0x2;
const DUMP_BYTES: usize = 64 * 1024;
const DUMP_DATAGRAMS: usize = 32;
const DUMP_MESSAGES: usize = 64;
const EXPECTED_CHAIN: &[u8] = b"output_guard\0";
const FILTER: &[u8] = b"filter\0";

/// A *shape* summary only. Even ExpectedOutputChain is untrusted: no rules,
/// ownership receipt or canonical host namespace have been verified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalChainInventory {
    TableAbsent,
    Empty,
    ExpectedOutputChainUntrusted,
    OtherUntrusted,
}

fn fixed_request(seq: u32) -> Result<Vec<u8>> {
    require(seq != 0)?;
    let mut body = vec![1, 0, 0, 0];
    body.extend(attribute(1, TABLE));
    Ok(message(GET_CHAIN, REQUEST_DUMP, seq, 0, &body))
}

fn nul_string(bytes: &[u8], max: usize) -> Result<&[u8]> {
    require(
        (2..=max).contains(&bytes.len())
            && bytes.last() == Some(&0)
            && !bytes[..bytes.len() - 1].contains(&0),
    )?;
    Ok(bytes)
}

fn parse_chain(body: &[u8], generation: u32) -> Result<(Vec<u8>, Vec<u8>, bool)> {
    require(
        body.len() >= 4
            && body[0] == 1
            && body[1] == 0
            && body[2..4] == (generation as u16).to_be_bytes(),
    )?;
    let values = attributes(&body[4..], 12)?;
    let table = nul_string(values[1].ok_or(REFUSE)?, 256)?;
    // A dump is namespace-wide. Validate the complete supported schema before
    // a foreign record can be excluded from our fixed-table summary.
    let table = table.to_vec();
    let name = nul_string(values[3].ok_or(REFUSE)?, 256)?.to_vec();
    let handle = u64::from_be_bytes(values[2].ok_or(REFUSE)?.try_into().map_err(|_| REFUSE)?);
    require(handle != 0)?;
    if let Some(use_count) = values[6] {
        u32b(use_count)?;
    }
    if let Some(flags) = values[10] {
        u32b(flags)?;
    }
    if let Some(id) = values[11] {
        u32b(id)?;
    }
    require(values[9].is_none_or(|padding| padding.is_empty()))?;
    require(values[12].is_none_or(|data| data.len() <= 256))?;
    if let Some(counters) = values[8] {
        let nested = attributes(counters, 2)?;
        for count in nested.into_iter().flatten() {
            require(count.len() == 8)?;
        }
    }

    let hook = values[4].map(|raw| attributes(raw, 4)).transpose()?;
    let hook_exact = if let Some(hook) = hook {
        // Device-bound hooks are outside this fixed observer schema. Even a
        // foreign chain must not be skipped with unparsed hook data.
        require(hook[3].is_none() && hook[4].is_none())?;
        let number = u32b(hook[1].ok_or(REFUSE)?)?;
        let priority = i32::from_be_bytes(hook[2].ok_or(REFUSE)?.try_into().map_err(|_| REFUSE)?);
        number == 3 && priority == 300
    } else {
        false
    };
    let policy = values[5].map(u32b).transpose()?;
    let kind = values[7].map(|value| nul_string(value, 64)).transpose()?;
    let flags = values[10].map(u32b).transpose()?;
    Ok((
        table.clone(),
        name.clone(),
        table == TABLE
            && name == EXPECTED_CHAIN
            && hook_exact
            && policy == Some(0)
            && kind == Some(FILTER)
            && flags == Some(1),
    ))
}

struct ChainDump {
    request: Vec<u8>,
    port: u32,
    generation: u32,
    total: usize,
    datagrams: usize,
    messages: usize,
    done: bool,
    names: BTreeSet<(Vec<u8>, Vec<u8>)>,
    target_count: usize,
    expected: bool,
}

impl ChainDump {
    fn new(seq: u32, port: u32, generation: u32) -> Result<Self> {
        require(port != 0 && generation != 0)?;
        Ok(Self {
            request: fixed_request(seq)?,
            port,
            generation,
            total: 0,
            datagrams: 0,
            messages: 0,
            done: false,
            names: BTreeSet::new(),
            target_count: 0,
            expected: false,
        })
    }

    fn receive(
        &mut self,
        mut bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        require(sender == Some(NetlinkAddr::new(0, 0)) && flags.is_empty() && !self.done)?;
        self.total = self.total.checked_add(bytes.len()).ok_or(REFUSE)?;
        self.datagrams += 1;
        require(!bytes.is_empty() && self.total <= DUMP_BYTES && self.datagrams <= DUMP_DATAGRAMS)?;
        while !bytes.is_empty() {
            self.messages += 1;
            require(self.messages <= DUMP_MESSAGES && bytes.len() >= 16)?;
            let length = usize::try_from(u32n(&bytes[..4])?).map_err(|_| REFUSE)?;
            require(length >= 16 && aligned(length) <= bytes.len())?;
            require(bytes[length..aligned(length)].iter().all(|byte| *byte == 0))?;
            let kind = u16n(&bytes[4..6])?;
            let flags = u16n(&bytes[6..8])?;
            require(bytes[8..12] == self.request[8..12] && u32n(&bytes[12..16])? == self.port)?;
            let body = &bytes[16..length];
            match kind {
                NEW_CHAIN => {
                    require(flags == MULTI && !self.done)?;
                    let (table, name, expected) = parse_chain(body, self.generation)?;
                    let is_target = table == TABLE;
                    require(self.names.insert((table, name)))?;
                    if is_target {
                        self.target_count += 1;
                        self.expected |= expected;
                    }
                }
                DONE => {
                    require(
                        matches!(flags, 0 | MULTI)
                            && body == [0, 0, 0, 0]
                            && aligned(length) == bytes.len(),
                    )?;
                    self.done = true;
                }
                _ => return Err(REFUSE),
            }
            bytes = &bytes[aligned(length)..];
        }
        Ok(())
    }

    fn classify(&self) -> Result<LocalChainInventory> {
        require(self.done)?;
        Ok(if self.target_count == 0 {
            LocalChainInventory::Empty
        } else if self.target_count == 1 && self.expected {
            LocalChainInventory::ExpectedOutputChainUntrusted
        } else {
            LocalChainInventory::OtherUntrusted
        })
    }
}

impl LocalReadSession {
    fn chain_sequences(&mut self) -> Result<[u32; 4]> {
        let first = self.next_sequence;
        require(first != 0)?;
        let second = first.checked_add(1).ok_or(REFUSE)?;
        let third = second.checked_add(1).ok_or(REFUSE)?;
        let fourth = third.checked_add(1).ok_or(REFUSE)?;
        self.next_sequence = fourth.checked_add(1).ok_or(REFUSE)?;
        Ok([first, second, third, fourth])
    }

    fn dump_chains(&self, seq: u32, generation: u32, deadline: Instant) -> Result<ChainDump> {
        self.check(deadline)?;
        let mut dump = ChainDump::new(seq, self.local.pid(), generation)?;
        require(
            sendto(
                self.socket.as_raw_fd(),
                &dump.request,
                &NetlinkAddr::new(0, 0),
                MsgFlags::MSG_DONTWAIT,
            )
            .map_err(|_| REFUSE)?
                == dump.request.len(),
        )?;
        while !dump.done {
            self.check(deadline)?;
            let mut bytes = [0; LIMIT];
            let mut iov = [IoSliceMut::new(&mut bytes)];
            match recvmsg::<NetlinkAddr>(
                self.socket.as_raw_fd(),
                &mut iov,
                None,
                MsgFlags::MSG_DONTWAIT,
            ) {
                Ok(reply) => {
                    let (length, sender, flags) = (reply.bytes, reply.address, reply.flags);
                    require(length <= LIMIT)?;
                    dump.receive(&bytes[..length], sender, flags)?;
                }
                Err(nix::errno::Errno::EAGAIN) => std::thread::sleep(Duration::from_millis(1)),
                Err(_) => return Err(REFUSE),
            }
        }
        self.check(deadline)?;
        Ok(dump)
    }

    /// Read-only, complete fixed-table chain inventory. It is not rule
    /// inventory or authority to arm/disarm/adopt any policy. Fully validated
    /// foreign chain records are excluded from the fixed-table summary.
    pub fn inspect_chains(&mut self) -> Result<LocalChainInventory> {
        let result = self.inspect_chains_once();
        if result.is_err() {
            // An interrupted dump may have unread frames on this socket.
            // Reopening a fresh session is safer than draining/reusing it.
            self.poisoned = true;
        }
        result
    }

    fn inspect_chains_once(&mut self) -> Result<LocalChainInventory> {
        let deadline = Instant::now() + Duration::from_secs(1);
        self.check(deadline)?;
        let [before_seq, table_seq, chain_seq, after_seq] = self.chain_sequences()?;
        let before = self.exchange(GET_GEN, before_seq, deadline)?.generation()?;
        let table = self
            .exchange(GET_TABLE, table_seq, deadline)?
            .table(before)?;
        let inventory = if table == LocalTablePresence::Absent {
            LocalChainInventory::TableAbsent
        } else {
            self.dump_chains(chain_seq, before, deadline)?.classify()?
        };
        let after = self.exchange(GET_GEN, after_seq, deadline)?.generation()?;
        require(before == after)?;
        self.check(deadline)?;
        Ok(inventory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT: u32 = 123;
    const GENERATION: u32 = 9;

    fn frame(kind: u16, flags: u16, seq: u32, body: &[u8]) -> Vec<u8> {
        message(kind, flags, seq, PORT, body)
    }

    fn chain_body(name: &[u8], hook: u32, priority: i32, policy: u32) -> Vec<u8> {
        chain_body_for_table(TABLE, name, hook, priority, policy)
    }

    fn chain_body_for_table(
        table: &[u8],
        name: &[u8],
        hook: u32,
        priority: i32,
        policy: u32,
    ) -> Vec<u8> {
        let mut body = vec![1, 0];
        body.extend_from_slice(&(GENERATION as u16).to_be_bytes());
        body.extend(attribute(1, table));
        body.extend(attribute(2, &17_u64.to_be_bytes()));
        body.extend(attribute(3, name));
        let mut hooks = attribute(1, &hook.to_be_bytes());
        hooks.extend(attribute(2, &priority.to_be_bytes()));
        body.extend(attribute(4, &hooks));
        body.extend(attribute(5, &policy.to_be_bytes()));
        body.extend(attribute(6, &0_u32.to_be_bytes()));
        body.extend(attribute(7, FILTER));
        body.extend(attribute(10, &1_u32.to_be_bytes()));
        body
    }

    fn receive(dump: &mut ChainDump, bytes: &[u8]) -> Result<()> {
        dump.receive(bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
    }

    fn finish(dump: &mut ChainDump) -> Result<LocalChainInventory> {
        receive(dump, &frame(DONE, MULTI, 2, &[0; 4]))?;
        dump.classify()
    }

    #[test]
    fn fixed_dump_request_and_complete_inventory_are_untrusted() {
        assert_eq!(fixed_request(2).unwrap()[6..8], REQUEST_DUMP.to_ne_bytes());
        let mut empty = ChainDump::new(2, PORT, GENERATION).unwrap();
        assert_eq!(finish(&mut empty), Ok(LocalChainInventory::Empty));
        let mut expected = ChainDump::new(2, PORT, GENERATION).unwrap();
        receive(
            &mut expected,
            &frame(NEW_CHAIN, MULTI, 2, &chain_body(EXPECTED_CHAIN, 3, 300, 0)),
        )
        .unwrap();
        assert_eq!(
            finish(&mut expected),
            Ok(LocalChainInventory::ExpectedOutputChainUntrusted)
        );
        for (name, hook, priority, policy) in [
            (b"other\0".as_slice(), 3, 300, 0),
            (EXPECTED_CHAIN, 1, 300, 0),
            (EXPECTED_CHAIN, 3, 299, 0),
            (EXPECTED_CHAIN, 3, 300, 1),
        ] {
            let mut other = ChainDump::new(2, PORT, GENERATION).unwrap();
            receive(
                &mut other,
                &frame(
                    NEW_CHAIN,
                    MULTI,
                    2,
                    &chain_body(name, hook, priority, policy),
                ),
            )
            .unwrap();
            assert_eq!(finish(&mut other), Ok(LocalChainInventory::OtherUntrusted));
        }
    }

    #[test]
    fn dump_refuses_incomplete_duplicate_interrupted_and_foreign_replies() {
        let good = frame(NEW_CHAIN, MULTI, 2, &chain_body(EXPECTED_CHAIN, 3, 300, 0));
        let mut incomplete = ChainDump::new(2, PORT, GENERATION).unwrap();
        receive(&mut incomplete, &good).unwrap();
        assert!(incomplete.classify().is_err());
        assert!(receive(&mut incomplete, &good).is_err());
        for bad in [
            frame(DONE, MULTI | 0x10, 2, &[0; 4]),
            frame(DONE, MULTI, 3, &[0; 4]),
            frame(DONE, MULTI, 2, &[1; 4]),
            frame(NEW_CHAIN, 0, 2, &chain_body(EXPECTED_CHAIN, 3, 300, 0)),
            frame(2, 0, 2, &[]),
        ] {
            assert!(receive(&mut ChainDump::new(2, PORT, GENERATION).unwrap(), &bad).is_err());
        }
        let mut foreign = chain_body(EXPECTED_CHAIN, 3, 300, 0);
        foreign[8 + TABLE.len() - 1] = b'x';
        assert!(
            receive(
                &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                &frame(NEW_CHAIN, MULTI, 2, &foreign)
            )
            .is_err()
        );
        assert!(
            receive(
                &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                &good[..good.len() - 1]
            )
            .is_err()
        );
    }

    #[test]
    fn coalesced_chain_and_done_are_still_a_complete_untrusted_shape() {
        let mut datagram = frame(NEW_CHAIN, MULTI, 2, &chain_body(EXPECTED_CHAIN, 3, 300, 0));
        datagram.extend(frame(DONE, MULTI, 2, &[0; 4]));
        let mut dump = ChainDump::new(2, PORT, GENERATION).unwrap();
        receive(&mut dump, &datagram).unwrap();
        assert_eq!(
            dump.classify(),
            Ok(LocalChainInventory::ExpectedOutputChainUntrusted)
        );
        assert!(receive(&mut dump, &frame(DONE, MULTI, 2, &[0; 4])).is_err());
    }

    #[test]
    fn dump_refuses_transport_identity_generation_and_resource_bounds() {
        let done = frame(DONE, MULTI, 2, &[0; 4]);
        let mut wrong_port = done.clone();
        wrong_port[12..16].copy_from_slice(&(PORT + 1).to_ne_bytes());
        for bad in [wrong_port, frame(DONE, MULTI, 3, &[0; 4])] {
            assert!(receive(&mut ChainDump::new(2, PORT, GENERATION).unwrap(), &bad).is_err());
        }
        let mut dump = ChainDump::new(2, PORT, GENERATION).unwrap();
        assert!(
            dump.receive(&done, Some(NetlinkAddr::new(1, 0)), MsgFlags::empty())
                .is_err()
        );
        let mut dump = ChainDump::new(2, PORT, GENERATION).unwrap();
        assert!(
            dump.receive(&done, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_TRUNC)
                .is_err()
        );
        let mut wrong_generation = chain_body(EXPECTED_CHAIN, 3, 300, 0);
        wrong_generation[2..4].copy_from_slice(&((GENERATION + 1) as u16).to_be_bytes());
        assert!(
            receive(
                &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                &frame(NEW_CHAIN, MULTI, 2, &wrong_generation)
            )
            .is_err()
        );
        for bounded in [
            ChainDump {
                total: DUMP_BYTES,
                ..ChainDump::new(2, PORT, GENERATION).unwrap()
            },
            ChainDump {
                datagrams: DUMP_DATAGRAMS,
                ..ChainDump::new(2, PORT, GENERATION).unwrap()
            },
            ChainDump {
                messages: DUMP_MESSAGES,
                ..ChainDump::new(2, PORT, GENERATION).unwrap()
            },
        ] {
            let mut bounded = bounded;
            assert!(receive(&mut bounded, &done).is_err());
        }
    }

    #[test]
    fn dump_refuses_duplicate_unknown_and_malformed_chain_attributes() {
        let original = chain_body(EXPECTED_CHAIN, 3, 300, 0);
        for extra in [attribute(3, EXPECTED_CHAIN), attribute(13, &[0; 4])] {
            let mut bad = original.clone();
            bad.extend(extra);
            assert!(
                receive(
                    &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                    &frame(NEW_CHAIN, MULTI, 2, &bad)
                )
                .is_err()
            );
        }
        let mut bad_hook = original;
        let hook_start = 4
            + attribute(1, TABLE).len()
            + attribute(2, &17_u64.to_be_bytes()).len()
            + attribute(3, EXPECTED_CHAIN).len()
            + 4;
        bad_hook[hook_start + 2..hook_start + 4].copy_from_slice(&0_u16.to_ne_bytes());
        assert!(
            receive(
                &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                &frame(NEW_CHAIN, MULTI, 2, &bad_hook)
            )
            .is_err()
        );
    }

    #[test]
    fn validated_foreign_chains_do_not_change_fixed_table_shape() {
        const FOREIGN: &[u8] = b"unrelated_synthetic\0";
        let foreign = frame(
            NEW_CHAIN,
            MULTI,
            2,
            &chain_body_for_table(FOREIGN, EXPECTED_CHAIN, 3, 300, 0),
        );
        let target = frame(NEW_CHAIN, MULTI, 2, &chain_body(EXPECTED_CHAIN, 3, 300, 0));
        let mut only_foreign = ChainDump::new(2, PORT, GENERATION).unwrap();
        receive(&mut only_foreign, &foreign).unwrap();
        assert!(only_foreign.classify().is_err());
        assert_eq!(finish(&mut only_foreign), Ok(LocalChainInventory::Empty));
        for first_target in [false, true] {
            let mut dump = ChainDump::new(2, PORT, GENERATION).unwrap();
            let mut datagram = if first_target {
                target.clone()
            } else {
                foreign.clone()
            };
            datagram.extend(if first_target { &foreign } else { &target });
            receive(&mut dump, &datagram).unwrap();
            assert!(dump.classify().is_err());
            assert_eq!(
                finish(&mut dump),
                Ok(LocalChainInventory::ExpectedOutputChainUntrusted)
            );
        }
        let mut separated = ChainDump::new(2, PORT, GENERATION).unwrap();
        receive(&mut separated, &target).unwrap();
        receive(&mut separated, &foreign).unwrap();
        assert_eq!(
            finish(&mut separated),
            Ok(LocalChainInventory::ExpectedOutputChainUntrusted)
        );
        let mut duplicate_foreign = ChainDump::new(2, PORT, GENERATION).unwrap();
        receive(&mut duplicate_foreign, &foreign).unwrap();
        assert!(receive(&mut duplicate_foreign, &foreign).is_err());

        let mut malformed_foreign = chain_body_for_table(FOREIGN, EXPECTED_CHAIN, 3, 300, 0);
        malformed_foreign.extend(attribute(13, &[0; 4]));
        assert!(
            receive(
                &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                &frame(NEW_CHAIN, MULTI, 2, &malformed_foreign),
            )
            .is_err()
        );
        let mut wrong_generation = chain_body_for_table(FOREIGN, EXPECTED_CHAIN, 3, 300, 0);
        wrong_generation[2..4].copy_from_slice(&((GENERATION + 1) as u16).to_be_bytes());
        assert!(
            receive(
                &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                &frame(NEW_CHAIN, MULTI, 2, &wrong_generation),
            )
            .is_err()
        );
        for hook_kind in [3, 4] {
            let mut body = vec![1, 0];
            body.extend_from_slice(&(GENERATION as u16).to_be_bytes());
            body.extend(attribute(1, FOREIGN));
            body.extend(attribute(2, &17_u64.to_be_bytes()));
            body.extend(attribute(3, EXPECTED_CHAIN));
            let mut hook = attribute(1, &3_u32.to_be_bytes());
            hook.extend(attribute(2, &300_i32.to_be_bytes()));
            hook.extend(attribute(hook_kind, &[1, 2, 3]));
            body.extend(attribute(4, &hook));
            assert!(
                receive(
                    &mut ChainDump::new(2, PORT, GENERATION).unwrap(),
                    &frame(NEW_CHAIN, MULTI, 2, &body),
                )
                .is_err()
            );
        }
    }
}
