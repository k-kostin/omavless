//! Inactive, read-only fixed-table metadata observation in the calling namespace.
//! No ownership, policy verification, canonical-host identity or effect authority.
#[path = "kernel_atomic_batch.rs"]
mod atomic_batch;
#[path = "kernel_chain_observer.rs"]
mod chain;
pub use chain::LocalChainInventory;
#[path = "kernel_rule_observer.rs"]
mod rule;
#[path = "kernel_rule_wire.rs"]
mod rule_wire;
use nix::sys::socket::{
    AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType, bind, getsockname,
    recvmsg, sendto, socket,
};
use nix::sys::statfs::{NSFS_MAGIC, PROC_SUPER_MAGIC, fstatfs};
pub use rule_wire::LocalRuleInventory;
#[path = "kernel_inventory.rs"]
mod inventory;
pub use inventory::LocalPolicyInventory;
#[cfg(test)]
#[path = "kernel_conditional_delete.rs"]
mod conditional_delete;
#[cfg(test)]
#[path = "kernel_creator_lifecycle.rs"]
mod creator_lifecycle;
#[cfg(test)]
#[path = "kernel_end_ack_loss.rs"]
mod end_ack_loss;
#[cfg(test)]
#[path = "kernel_prefix_ack_loss.rs"]
mod prefix_ack_loss;
#[cfg(test)]
#[path = "kernel_receive_truncation.rs"]
mod receive_truncation;
use std::{
    fs::File,
    io::IoSliceMut,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    time::{Duration, Instant},
};

const LIMIT: usize = 32768;
const TABLE: &[u8] = b"omavless_netguard\0";
const NFT: u16 = 10 << 8;
const GET_TABLE: u16 = NFT + 1;
const GET_GEN: u16 = NFT + 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationError {
    UnavailableOrUncertain,
}
type Result<T> = std::result::Result<T, ObservationError>;
const REFUSE: ObservationError = ObservationError::UnavailableOrUncertain;

/// Only presence of the fixed table in the calling thread's namespace.
/// Present tables remain untrusted, including apparently empty/owner tables.
/// This deliberately cannot be supplied to `LockedState` or `EffectPort`.
///
/// ```compile_fail
/// use omavless_netguard::{effect_port::EffectPort, kernel_observer::LocalTablePresence};
/// fn effect_adapter(_: impl EffectPort) {}
/// effect_adapter(LocalTablePresence::Absent);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalTablePresence {
    Absent,
    PresentUntrusted,
}

fn require(ok: bool) -> Result<()> {
    if ok { Ok(()) } else { Err(REFUSE) }
}
fn u16n(b: &[u8]) -> Result<u16> {
    Ok(u16::from_ne_bytes(b.try_into().map_err(|_| REFUSE)?))
}
fn u32n(b: &[u8]) -> Result<u32> {
    Ok(u32::from_ne_bytes(b.try_into().map_err(|_| REFUSE)?))
}
fn u32b(b: &[u8]) -> Result<u32> {
    Ok(u32::from_be_bytes(b.try_into().map_err(|_| REFUSE)?))
}
fn aligned(n: usize) -> usize {
    (n + 3) & !3
}
fn attribute(kind: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&((4 + data.len()) as u16).to_ne_bytes());
    out.extend_from_slice(&kind.to_ne_bytes());
    out.extend_from_slice(data);
    out.resize(aligned(out.len()), 0);
    out
}
fn message(kind: u16, flags: u16, seq: u32, port: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&((16 + body.len()) as u32).to_ne_bytes());
    out.extend_from_slice(&kind.to_ne_bytes());
    out.extend_from_slice(&flags.to_ne_bytes());
    out.extend_from_slice(&seq.to_ne_bytes());
    out.extend_from_slice(&port.to_ne_bytes());
    out.extend_from_slice(body);
    out.resize(aligned(out.len()), 0);
    out
}
fn request(kind: u16, seq: u32) -> Result<Vec<u8>> {
    require(seq > 0 && matches!(kind, GET_GEN | GET_TABLE))?;
    let mut body = vec![u8::from(kind == GET_TABLE), 0, 0, 0];
    if kind == GET_TABLE {
        body.extend(attribute(1, TABLE));
    }
    Ok(message(kind, 1 | 4, seq, 0, &body))
}

fn attributes(mut bytes: &[u8], max: usize) -> Result<Vec<Option<&[u8]>>> {
    require(bytes.len() <= LIMIT)?;
    let mut values = vec![None; max + 1];
    while !bytes.is_empty() {
        require(bytes.len() >= 4)?;
        let length = usize::from(u16n(&bytes[..2])?);
        let raw_kind = u16n(&bytes[2..4])?;
        let kind = usize::from(raw_kind & 0x3fff);
        // Neither nested nor network-byte-order tagged attributes belong to
        // this fixed response schema. Masking their type alone could silently
        // reinterpret a future kernel field as one we understand.
        require(raw_kind & 0xc000 == 0 && kind > 0 && kind <= max)?;
        require(length >= 4 && aligned(length) <= bytes.len() && values[kind].is_none())?;
        require(bytes[length..aligned(length)].iter().all(|v| *v == 0))?;
        values[kind] = Some(&bytes[4..length]);
        bytes = &bytes[aligned(length)..];
    }
    Ok(values)
}

/// One exact non-dump request. Never accept silence, partial ACKs, or NLMSG_DONE
/// as success. Raw bytes remain private and are never formatted as diagnostics.
struct Exchange {
    request: Vec<u8>,
    port: u32,
    ack: bool,
    absent: bool,
    body: Option<Vec<u8>>,
    total: usize,
    datagrams: usize,
}

#[derive(Debug, Eq, PartialEq)]
struct TableMetadata {
    flags: u32,
    uses: u32,
    handle: u64,
    owner: Option<u32>,
    userdata: Option<Vec<u8>>,
}
impl Exchange {
    fn new(kind: u16, seq: u32, port: u32) -> Result<Self> {
        require(port != 0)?;
        Ok(Self {
            request: request(kind, seq)?,
            port,
            ack: false,
            absent: false,
            body: None,
            total: 0,
            datagrams: 0,
        })
    }
    fn complete(&self) -> bool {
        self.absent || (self.ack && self.body.is_some())
    }
    fn receive(
        &mut self,
        mut bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        require(sender == Some(NetlinkAddr::new(0, 0)) && flags.is_empty())?;
        self.total = self.total.checked_add(bytes.len()).ok_or(REFUSE)?;
        self.datagrams += 1;
        require(
            !bytes.is_empty() && self.total <= LIMIT && self.datagrams <= 16 && !self.complete(),
        )?;
        let request_kind = u16n(&self.request[4..6])?;
        while !bytes.is_empty() {
            require(bytes.len() >= 16)?;
            let length = usize::try_from(u32n(&bytes[..4])?).map_err(|_| REFUSE)?;
            require(length >= 16 && aligned(length) <= bytes.len())?;
            require(bytes[length..aligned(length)].iter().all(|v| *v == 0))?;
            let kind = u16n(&bytes[4..6])?;
            let flags = u16n(&bytes[6..8])?;
            require(bytes[8..12] == self.request[8..12] && u32n(&bytes[12..16])? == self.port)?;
            let body = &bytes[16..length];
            if kind == 2 {
                // Successful ACKs omit original payload. Uncapped errors echo
                // it; capped errors contain only the original header.
                require(flags & !0x100 == 0 && body.len() >= 20 && !self.ack && !self.absent)?;
                require(body[4..20] == self.request[..16])?;
                let code = i32::from_ne_bytes(body[..4].try_into().map_err(|_| REFUSE)?);
                if code == 0 {
                    require(body.len() == 20)?;
                    self.ack = true;
                } else {
                    require(code == -2 && request_kind == GET_TABLE && self.body.is_none())?;
                    let expected = if flags == 0x100 {
                        &self.request[..16]
                    } else {
                        &self.request[..]
                    };
                    require(&body[4..] == expected)?;
                    self.absent = true;
                }
            } else {
                require(flags == 0 && !self.absent && self.body.is_none())?;
                require(
                    kind == if request_kind == GET_GEN {
                        NFT + 15
                    } else {
                        NFT
                    },
                )?;
                require(
                    body.len() >= 4
                        && body[0] == u8::from(request_kind == GET_TABLE)
                        && body[1] == 0,
                )?;
                self.body = Some(body.to_vec());
            }
            bytes = &bytes[aligned(length)..];
        }
        Ok(())
    }
    fn generation(&self) -> Result<u32> {
        require(self.complete() && !self.absent)?;
        let body = self.body.as_deref().ok_or(REFUSE)?;
        let values = attributes(&body[4..], 3)?;
        let value = u32b(values[1].ok_or(REFUSE)?)?;
        require(value != 0 && body[2..4] == (value as u16).to_be_bytes())?;
        if let Some(pid) = values[2] {
            u32b(pid)?;
        }
        if let Some(name) = values[3] {
            require(
                !name.is_empty()
                    && name.len() <= 16
                    && name.last() == Some(&0)
                    && !name[..name.len() - 1].contains(&0),
            )?;
        }
        Ok(value)
    }
    fn table(&self, generation: u32) -> Result<LocalTablePresence> {
        Ok(if self.table_metadata(generation)?.is_some() {
            LocalTablePresence::PresentUntrusted
        } else {
            LocalTablePresence::Absent
        })
    }
    fn table_metadata(&self, generation: u32) -> Result<Option<TableMetadata>> {
        require(self.complete())?;
        if self.absent {
            return Ok(None);
        }
        let body = self.body.as_deref().ok_or(REFUSE)?;
        require(body[2..4] == (generation as u16).to_be_bytes())?;
        let values = attributes(&body[4..], 7)?;
        require(values[1] == Some(TABLE))?;
        let flags = u32b(values[2].ok_or(REFUSE)?)?;
        require(flags & !7 == 0)?;
        let uses = u32b(values[3].ok_or(REFUSE)?)?;
        let handle = u64::from_be_bytes(values[4].ok_or(REFUSE)?.try_into().map_err(|_| REFUSE)?);
        require(handle != 0 && values[5].is_none_or(|pad| pad.is_empty()))?;
        if let Some(data) = values[6] {
            require(data.len() <= 256)?;
        }
        match values[7] {
            Some(owner) => require(flags & 2 != 0 && u32b(owner)? != 0)?,
            None => require(flags & 2 == 0)?,
        }
        Ok(Some(TableMetadata {
            flags,
            uses,
            handle,
            owner: values[7].map(u32b).transpose()?,
            userdata: values[6].map(<[u8]>::to_vec),
        }))
    }
}

fn namespace_identity(fd: &File) -> Result<(u64, u64)> {
    require(fstatfs(fd).map_err(|_| REFUSE)?.filesystem_type() == NSFS_MAGIC)?;
    let metadata = fd.metadata().map_err(|_| REFUSE)?;
    let label = std::fs::read_link(format!("/proc/thread-self/fd/{}", fd.as_raw_fd()))
        .map_err(|_| REFUSE)?;
    require(metadata.ino() != 0 && label.to_str() == Some(&format!("net:[{}]", metadata.ino())))?;
    Ok((metadata.dev(), metadata.ino()))
}
fn namespace_file() -> Result<File> {
    let proc_ns = File::open("/proc/thread-self/ns").map_err(|_| REFUSE)?;
    require(fstatfs(&proc_ns).map_err(|_| REFUSE)?.filesystem_type() == PROC_SUPER_MAGIC)?;
    File::open("/proc/thread-self/ns/net").map_err(|_| REFUSE)
}

fn take_sequences(next: &mut u32) -> Result<[u32; 3]> {
    let first = *next;
    require(first != 0)?;
    let second = first.checked_add(1).ok_or(REFUSE)?;
    let third = second.checked_add(1).ok_or(REFUSE)?;
    *next = third.checked_add(1).ok_or(REFUSE)?;
    Ok([first, second, third])
}

/// A retained, read-only descriptor pair. Repeated inspections cannot swap in
/// caller-provided namespace/socket descriptors or replay a previous sequence.
/// This is not a canonical-host proof, an ownership receipt or an EffectPort.
/// No installed helper uses it.
pub struct LocalReadSession {
    namespace: File,
    identity: (u64, u64),
    socket: OwnedFd,
    local: NetlinkAddr,
    next_sequence: u32,
    poisoned: bool,
}

impl LocalReadSession {
    pub fn open() -> Result<Self> {
        let namespace = namespace_file()?;
        let identity = namespace_identity(&namespace)?;
        let socket = socket(
            AddressFamily::Netlink,
            SockType::Raw,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            SockProtocol::NetlinkNetFilter,
        )
        .map_err(|_| REFUSE)?;
        bind(socket.as_raw_fd(), &NetlinkAddr::new(0, 0)).map_err(|_| REFUSE)?;
        let local: NetlinkAddr = getsockname(socket.as_raw_fd()).map_err(|_| REFUSE)?;
        require(local.pid() != 0 && local.groups() == 0)?;
        let session = Self {
            namespace,
            identity,
            socket,
            local,
            next_sequence: 1,
            poisoned: false,
        };
        session.check(Instant::now() + Duration::from_secs(1))?;
        Ok(session)
    }

    fn check(&self, deadline: Instant) -> Result<()> {
        require(
            !self.poisoned
                && Instant::now() < deadline
                && namespace_identity(&self.namespace)? == self.identity
                && namespace_identity(&namespace_file()?)? == self.identity,
        )?;
        let actual: NetlinkAddr = getsockname(self.socket.as_raw_fd()).map_err(|_| REFUSE)?;
        require(actual == self.local)
    }

    fn sequences(&mut self) -> Result<[u32; 3]> {
        take_sequences(&mut self.next_sequence)
    }

    fn exchange(&self, kind: u16, seq: u32, deadline: Instant) -> Result<Exchange> {
        self.check(deadline)?;
        let mut exchange = Exchange::new(kind, seq, self.local.pid())?;
        require(
            sendto(
                self.socket.as_raw_fd(),
                &exchange.request,
                &NetlinkAddr::new(0, 0),
                MsgFlags::MSG_DONTWAIT,
            )
            .map_err(|_| REFUSE)?
                == exchange.request.len(),
        )?;
        while !exchange.complete() {
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
                    exchange.receive(&bytes[..length], sender, flags)?;
                }
                Err(nix::errno::Errno::EAGAIN) => std::thread::sleep(Duration::from_millis(1)),
                Err(_) => return Err(REFUSE),
            }
        }
        self.check(deadline)?;
        Ok(exchange)
    }

    /// Inspect exactly `inet omavless_netguard` with one bounded three-request
    /// pass. Canonical-host provenance and socket-cookie binding remain
    /// unauthenticated; even an absent result cannot authorize creation.
    pub fn inspect(&mut self) -> Result<LocalTablePresence> {
        let result = self.inspect_once();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn inspect_once(&mut self) -> Result<LocalTablePresence> {
        let deadline = Instant::now() + Duration::from_secs(1);
        self.check(deadline)?;
        let [before_seq, table_seq, after_seq] = self.sequences()?;
        let before = self.exchange(GET_GEN, before_seq, deadline)?.generation()?;
        let table = self
            .exchange(GET_TABLE, table_seq, deadline)?
            .table(before)?;
        let after = self.exchange(GET_GEN, after_seq, deadline)?.generation()?;
        require(before == after)?;
        self.check(deadline)?;
        Ok(table)
    }
}

/// One-shot compatibility wrapper for the inactive developer observation.
pub fn inspect_current_namespace() -> Result<LocalTablePresence> {
    LocalReadSession::open()?.inspect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const PORT: u32 = 123;
    #[test]
    fn retained_sequence_space_never_wraps_or_reuses_a_reply() {
        let mut next = 1;
        assert_eq!(take_sequences(&mut next), Ok([1, 2, 3]));
        assert_eq!(take_sequences(&mut next), Ok([4, 5, 6]));
        next = u32::MAX - 2;
        assert!(take_sequences(&mut next).is_err());
        assert_eq!(next, u32::MAX - 2);
        next = 0;
        assert!(take_sequences(&mut next).is_err());
    }
    #[test]
    fn namespace_identity_rejects_procfs_and_regular_file_descriptors() {
        let proc_status = File::open("/proc/thread-self/status").unwrap();
        assert_eq!(namespace_identity(&proc_status), Err(REFUSE));
        let regular = File::open(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(namespace_identity(&regular), Err(REFUSE));
    }
    fn receive(exchange: &mut Exchange, bytes: &[u8]) -> Result<()> {
        exchange.receive(bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
    }
    fn ack(exchange: &Exchange, code: i32, capped: bool) -> Vec<u8> {
        let mut body = code.to_ne_bytes().to_vec();
        body.extend_from_slice(if code == 0 || capped {
            &exchange.request[..16]
        } else {
            &exchange.request
        });
        message(
            2,
            if capped { 0x100 } else { 0 },
            u32n(&exchange.request[8..12]).unwrap(),
            PORT,
            &body,
        )
    }
    fn generation(seq: u32, value: u32) -> Vec<u8> {
        let mut body = vec![0, 0];
        body.extend_from_slice(&(value as u16).to_be_bytes());
        body.extend(attribute(1, &value.to_be_bytes()));
        body.extend(attribute(2, &17_u32.to_be_bytes()));
        body.extend(attribute(3, b"synthetic\0"));
        message(NFT + 15, 0, seq, PORT, &body)
    }
    fn table_body(flags: u32, owner: Option<u32>) -> Vec<u8> {
        let mut body = vec![1, 0, 0, 7];
        body.extend(attribute(1, TABLE));
        body.extend(attribute(2, &flags.to_be_bytes()));
        body.extend(attribute(3, &3_u32.to_be_bytes()));
        body.extend(attribute(4, &99_u64.to_be_bytes()));
        body.extend(attribute(5, &[]));
        body.extend(attribute(6, b"synthetic"));
        if let Some(owner) = owner {
            body.extend(attribute(7, &owner.to_be_bytes()));
        }
        body
    }
    fn table_exchange(body: &[u8]) -> Exchange {
        let mut exchange = Exchange::new(GET_TABLE, 2, PORT).unwrap();
        receive(&mut exchange, &message(NFT, 0, 2, PORT, body)).unwrap();
        let ack = ack(&exchange, 0, true);
        receive(&mut exchange, &ack).unwrap();
        exchange
    }
    #[test]
    fn builders_can_only_request_fixed_metadata() {
        assert_eq!(
            request(GET_GEN, 1).unwrap(),
            message(GET_GEN, 5, 1, 0, &[0; 4])
        );
        let table = request(GET_TABLE, 2).unwrap();
        assert_eq!(&table[16..20], &[1, 0, 0, 0]);
        assert_eq!(attributes(&table[20..], 1).unwrap()[1], Some(TABLE));
        for kind in [NFT, NFT + 2, 16, 17, NFT + 22] {
            assert!(request(kind, 1).is_err());
        }
        assert!(request(GET_GEN, 0).is_err());
        assert!(Exchange::new(GET_GEN, 1, 0).is_err());
    }
    #[test]
    fn exact_ack_and_reply_are_both_required_in_either_order() {
        for ack_first in [false, true] {
            let mut exchange = Exchange::new(GET_GEN, 1, PORT).unwrap();
            let ack = ack(&exchange, 0, true);
            let reply = generation(1, 7);
            let order = if ack_first {
                [&ack, &reply]
            } else {
                [&reply, &ack]
            };
            receive(&mut exchange, order[0]).unwrap();
            assert!(!exchange.complete());
            assert!(exchange.generation().is_err());
            receive(&mut exchange, order[1]).unwrap();
            assert_eq!(exchange.generation(), Ok(7));
            assert!(receive(&mut exchange, &ack).is_err());
        }
    }
    #[test]
    fn only_exact_table_enoent_is_absence() {
        for capped in [false, true] {
            let mut exchange = Exchange::new(GET_TABLE, 2, PORT).unwrap();
            let error = ack(&exchange, -2, capped);
            receive(&mut exchange, &error).unwrap();
            assert_eq!(exchange.table(7), Ok(LocalTablePresence::Absent));
        }
        for (kind, code) in [
            (GET_GEN, -2),
            (GET_TABLE, -1),
            (GET_TABLE, -13),
            (GET_TABLE, 1),
        ] {
            let mut exchange = Exchange::new(kind, 1, PORT).unwrap();
            let error = ack(&exchange, code, false);
            assert!(receive(&mut exchange, &error).is_err());
        }
        let mut exchange = Exchange::new(GET_TABLE, 2, PORT).unwrap();
        let mut error = ack(&exchange, -2, false);
        *error.last_mut().unwrap() = 1;
        assert!(receive(&mut exchange, &error).is_err());
    }
    #[test]
    fn all_valid_table_flags_stay_untrusted() {
        for flags in 0..8 {
            let owner = if flags & 2 == 0 { None } else { Some(PORT) };
            let exchange = table_exchange(&table_body(flags, owner));
            assert_eq!(exchange.table(7), Ok(LocalTablePresence::PresentUntrusted));
            assert!(exchange.table(8).is_err());
        }
        for (flags, owner) in [(8, None), (2, None), (0, Some(PORT)), (2, Some(0))] {
            assert!(table_exchange(&table_body(flags, owner)).table(7).is_err());
        }
    }
    #[test]
    fn unknown_duplicate_nested_or_incomplete_attributes_refuse() {
        for suffix in [
            attribute(8, b""),
            attribute(1, TABLE),
            attribute(0x8001, b""),
            attribute(0x4001, b""),
            vec![1, 2, 3],
        ] {
            let mut body = table_body(0, None);
            body.extend(suffix);
            assert!(table_exchange(&body).table(7).is_err());
        }
        for end in 4..table_body(0, None).len() {
            // Some cuts end at valid optional-field boundaries, so exercise the
            // attributes framing separately without requiring optional presence.
            let body = table_body(0, None);
            if end % 4 != 0 {
                assert!(attributes(&body[4..end], 7).is_err());
            }
        }
        assert!(attributes(&attribute(5, &[1]), 4).is_err());
        let mut pad = attribute(1, b"a");
        *pad.last_mut().unwrap() = 1;
        assert!(attributes(&pad, 7).is_err());
    }
    #[test]
    fn sender_sequence_pid_flags_and_frame_bounds_are_strict() {
        let valid = generation(1, 7);
        for sender in [
            None,
            Some(NetlinkAddr::new(1, 0)),
            Some(NetlinkAddr::new(0, 1)),
        ] {
            let mut exchange = Exchange::new(GET_GEN, 1, PORT).unwrap();
            assert!(exchange.receive(&valid, sender, MsgFlags::empty()).is_err());
        }
        for flags in [MsgFlags::MSG_TRUNC, MsgFlags::MSG_CTRUNC] {
            let mut exchange = Exchange::new(GET_GEN, 1, PORT).unwrap();
            assert!(
                exchange
                    .receive(&valid, Some(NetlinkAddr::new(0, 0)), flags)
                    .is_err()
            );
        }
        for offset in [0, 4, 6, 8, 12, 16, 17] {
            let mut bad = valid.clone();
            bad[offset] ^= 0x10;
            let mut exchange = Exchange::new(GET_GEN, 1, PORT).unwrap();
            assert!(receive(&mut exchange, &bad).is_err());
        }
        for end in 0..valid.len() {
            assert!(receive(&mut Exchange::new(GET_GEN, 1, PORT).unwrap(), &valid[..end]).is_err());
        }
        let mut duplicate = valid.clone();
        duplicate.extend_from_slice(&valid);
        assert!(receive(&mut Exchange::new(GET_GEN, 1, PORT).unwrap(), &duplicate).is_err());
        assert!(
            receive(
                &mut Exchange::new(GET_GEN, 1, PORT).unwrap(),
                &vec![0; LIMIT + 1]
            )
            .is_err()
        );
    }
    #[test]
    fn generation_zero_mismatch_and_unknown_metadata_refuse() {
        for value in [0, 7] {
            let mut exchange = Exchange::new(GET_GEN, 1, PORT).unwrap();
            let mut reply = generation(1, value);
            if value != 0 {
                reply[19] ^= 1;
            }
            receive(&mut exchange, &reply).unwrap();
            let ack = ack(&exchange, 0, true);
            receive(&mut exchange, &ack).unwrap();
            assert!(exchange.generation().is_err());
        }
    }
}
