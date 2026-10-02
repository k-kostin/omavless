//! Fixed raw GETRULE readback on the retained observer socket. This checks
//! ordered rule expressions only, not table ownership or other object kinds.
use super::*;
use crate::{emergency_wire, full_vpn_wire, policy::Policy};
use std::collections::{BTreeMap, BTreeSet};

const CHAIN: &[u8] = b"output_guard\0";
const GET_RULE: u16 = NFT + 7;
const NEW_RULE: u16 = NFT + 6;
const MAX_BYTES: usize = 128 * 1024;
const MAX_MESSAGES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalRuleInventory {
    TableAbsent,
    ExactRulesUntrusted(Policy),
    OtherUntrusted,
}

// Preserve order for expression lists; keyed expression data is compared by
// unique attribute type. Never interpret opaque comparison/payload bytes as
// attributes. Only expected nested containers may carry the nested flag.
fn attrs(mut bytes: &[u8]) -> Result<Vec<(u16, &[u8])>> {
    require(bytes.len() <= LIMIT)?;
    let mut out = Vec::new();
    while !bytes.is_empty() {
        require(bytes.len() >= 4 && out.len() < 64)?;
        let length = usize::from(u16n(&bytes[..2])?);
        let kind = u16n(&bytes[2..4])?;
        require(length >= 4 && aligned(length) <= bytes.len() && kind & 0x4000 == 0)?;
        require(bytes[length..aligned(length)].iter().all(|v| *v == 0))?;
        out.push((kind, &bytes[4..length]));
        bytes = &bytes[aligned(length)..];
    }
    Ok(out)
}

fn same_fields(actual: &[u8], expected: &[u8], depth: usize) -> Result<bool> {
    require(depth <= 5)?;
    let actual = attrs(actual)?;
    let expected = attrs(expected)?;
    let mut seen = BTreeSet::new();
    for (kind, _) in &actual {
        require(seen.insert(kind & 0x3fff))?;
    }
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for (kind, expected) in expected {
        let Some((actual_kind, actual)) = actual.iter().find(|(k, _)| k & 0x3fff == kind & 0x3fff)
        else {
            return Ok(false);
        };
        if kind & 0x8000 != 0 {
            if !same_fields(actual, expected, depth + 1)? {
                return Ok(false);
            }
        } else if *actual_kind != kind || *actual != expected {
            return Ok(false);
        }
    }
    Ok(true)
}

fn same_expressions(actual: &[u8], expected: &[u8]) -> Result<bool> {
    let actual = attrs(actual)?;
    let expected = attrs(expected)?;
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for ((kind, actual), (_, expected)) in actual.into_iter().zip(expected) {
        require(matches!(kind, 1 | 0x8001))?;
        // Kernel dump adds the explicit default MASK_XOR operation. The
        // creation ABI permits its omission. No other default is synthesized.
        let fields = attrs(expected)?;
        let expected = if fields.iter().any(|(k, v)| *k == 1 && *v == b"bitwise\0") {
            let data = fields.iter().find(|(k, _)| *k == 0x8002).ok_or(REFUSE)?.1;
            [
                emergency_wire::attr(1, b"bitwise\0"),
                emergency_wire::nested(
                    2,
                    &[data, &emergency_wire::attr(6, &0_u32.to_be_bytes())].concat(),
                ),
            ]
            .concat()
        } else {
            expected.to_vec()
        };
        if !same_fields(actual, &expected, 0)? {
            return Ok(false);
        }
    }
    Ok(true)
}

struct RuleDump {
    request: Vec<u8>,
    port: u32,
    generation: u32,
    total: usize,
    datagrams: usize,
    messages: usize,
    done: bool,
    handles: BTreeSet<u64>,
    previous: BTreeMap<Vec<u8>, u64>,
    count: usize,
    full: bool,
    emergency: bool,
}
impl RuleDump {
    fn new(sequence: u32, port: u32, generation: u32) -> Result<Self> {
        require(sequence != 0 && port != 0 && generation != 0)?;
        Ok(Self {
            request: message(
                GET_RULE,
                0x301,
                sequence,
                0,
                &[vec![1, 0, 0, 0], attribute(1, TABLE)].concat(),
            ),
            port,
            generation,
            total: 0,
            datagrams: 0,
            messages: 0,
            done: false,
            handles: BTreeSet::new(),
            previous: BTreeMap::new(),
            count: 0,
            full: true,
            emergency: true,
        })
    }

    fn rule(&mut self, body: &[u8]) -> Result<()> {
        require(
            body.len() >= 4
                && body[..2] == [1, 0]
                && body[2..4] == (self.generation as u16).to_be_bytes(),
        )?;
        let mut values = [None; 9];
        for (kind, data) in attrs(&body[4..])? {
            // Kernel may insert more than one empty alignment PAD around u64s.
            if kind == 8 {
                require(data.is_empty())?;
                continue;
            }
            require((1..=7).contains(&kind) && values[usize::from(kind)].is_none())?;
            values[usize::from(kind)] = Some(data);
        }
        require(values[1] == Some(TABLE))?;
        let chain = values[2].ok_or(REFUSE)?;
        require(
            chain.len() >= 2
                && chain.len() <= 256
                && chain.last() == Some(&0)
                && !chain[..chain.len() - 1].contains(&0),
        )?;
        let handle = u64::from_be_bytes(values[3].ok_or(REFUSE)?.try_into().map_err(|_| REFUSE)?);
        require(handle != 0 && self.handles.insert(handle))?;
        require(values[5].is_none())?;
        let position = values[6]
            .map(|v| v.try_into().map(u64::from_be_bytes).map_err(|_| REFUSE))
            .transpose()?;
        require(position == self.previous.insert(chain.to_vec(), handle))?;
        require(values[7].is_none_or(|v| v.len() <= 256))?;
        let expressions = values[4].ok_or(REFUSE)?;
        let full = full_vpn_wire::rules();
        let exact_metadata = chain == CHAIN && values[7].is_none();
        self.full &= exact_metadata
            && match full.get(self.count) {
                Some(expected) => same_expressions(expressions, expected)?,
                None => false,
            };
        // Emergency is exactly loopback accept followed by terminal drop.
        let emergency = match self.count {
            0 => full.first(),
            1 => full.last(),
            _ => None,
        };
        self.emergency &= exact_metadata
            && match emergency {
                Some(expected) => same_expressions(expressions, expected)?,
                None => false,
            };
        self.count += 1;
        Ok(())
    }

    fn receive(
        &mut self,
        mut bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        require(!self.done && sender == Some(NetlinkAddr::new(0, 0)) && flags.is_empty())?;
        self.total = self.total.checked_add(bytes.len()).ok_or(REFUSE)?;
        self.datagrams += 1;
        require(!bytes.is_empty() && self.total <= MAX_BYTES && self.datagrams <= 32)?;
        while !bytes.is_empty() {
            self.messages += 1;
            require(self.messages <= MAX_MESSAGES && bytes.len() >= 16)?;
            let length = usize::try_from(u32n(&bytes[..4])?).map_err(|_| REFUSE)?;
            require(
                length >= 16
                    && aligned(length) <= bytes.len()
                    && bytes[length..aligned(length)].iter().all(|v| *v == 0)
                    && bytes[8..12] == self.request[8..12]
                    && u32n(&bytes[12..16])? == self.port,
            )?;
            let kind = u16n(&bytes[4..6])?;
            let flags = u16n(&bytes[6..8])?;
            match kind {
                NEW_RULE => {
                    require(flags == 0x802)?;
                    self.rule(&bytes[16..length])?;
                }
                3 => {
                    require(
                        matches!(flags, 0 | 2)
                            && bytes[16..length] == [0; 4]
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

    fn classify(&self) -> Result<LocalRuleInventory> {
        require(self.done)?;
        Ok(if self.full && self.count == 10 {
            LocalRuleInventory::ExactRulesUntrusted(Policy::FullVpn)
        } else if self.emergency && self.count == 2 {
            LocalRuleInventory::ExactRulesUntrusted(Policy::Emergency)
        } else {
            LocalRuleInventory::OtherUntrusted
        })
    }
}

impl LocalReadSession {
    /// Fixed table-scoped ordered expressions, on this session's retained
    /// socket. Does not check chains, sets, objects, creator or canonical host.
    /// The separate full JSON policy check remains necessary for whole-table
    /// shape until all object inventories are available on this same socket.
    pub fn inspect_rules(&mut self) -> Result<LocalRuleInventory> {
        let result = self.inspect_rules_once();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn inspect_rules_once(&mut self) -> Result<LocalRuleInventory> {
        let deadline = Instant::now() + Duration::from_secs(1);
        self.check(deadline)?;
        let first = self.next_sequence;
        require(first != 0)?;
        self.next_sequence = first.checked_add(4).ok_or(REFUSE)?;
        let generation = self.exchange(GET_GEN, first, deadline)?.generation()?;
        let table = self
            .exchange(GET_TABLE, first + 1, deadline)?
            .table(generation)?;
        let inventory = if table == LocalTablePresence::Absent {
            LocalRuleInventory::TableAbsent
        } else {
            let mut dump = RuleDump::new(first + 2, self.local.pid(), generation)?;
            self.check(deadline)?;
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
            dump.classify()?
        };
        require(self.exchange(GET_GEN, first + 3, deadline)?.generation()? == generation)?;
        self.check(deadline)?;
        Ok(inventory)
    }
}

#[cfg(test)]
#[path = "kernel_rule_wire_tests.rs"]
mod tests;
