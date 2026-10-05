//! Complete fixed-table object-kind inventory on one retained socket.
//! Matching shape/owner-port does not authenticate canonical host or creation.
use super::*;
use crate::policy::Policy;
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "kernel_create_witness.rs"]
pub(super) mod create_witness;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalPolicyInventory {
    TableAbsent,
    /// Table owner-port agrees with this socket and the complete fixed shape
    /// matches. No exclusive-create history, namespace or effect authority.
    ExactUntrusted(Policy),
    OtherUntrusted,
}

/// A complete local readback borrowing its original namespace and socket owner.
/// This is neither canonical namespace authority nor evidence of creation.
/// Keeping this value alive prevents another operation through the session;
/// dropping it performs no exchange, cleanup or mutation.
///
/// ```compile_fail
/// use omavless_netguard::kernel_observer::LocalReadSession;
/// fn overlapping(session: &mut LocalReadSession) {
///     let lease = session.borrow_policy_inventory().unwrap();
///     session.inspect_policy_inventory().unwrap();
///     let _ = lease.observed();
/// }
/// ```
///
/// ```compile_fail
/// use omavless_netguard::kernel_observer::LocalInventoryLease;
/// fn duplicate(lease: LocalInventoryLease<'_>) {
///     let moved = lease;
///     let _ = lease.observed();
///     let _ = moved.observed();
/// }
/// ```
pub struct LocalInventoryLease<'a> {
    pub(super) session: &'a mut LocalReadSession,
    pub(super) inventory: LocalPolicyInventory,
    pub(super) generation: u32,
    pub(super) table: Option<TableMetadata>,
    pub(super) deadline: Instant,
}

impl LocalInventoryLease<'_> {
    /// A copied classification is explicitly untrusted, not a transferable
    /// lease or an ownership receipt.
    pub fn observed(&self) -> LocalPolicyInventory {
        self.inventory
    }

    /// Recheck the same original resources and the original readback budget.
    /// A refusal permanently poisons the session; no fresh budget is acquired.
    pub fn recheck(&mut self) -> Result<()> {
        let result = (|| {
            require(
                self.generation != 0
                    && self.session.last_generation == Some(self.generation)
                    && self.table.is_none()
                        == (self.inventory == LocalPolicyInventory::TableAbsent),
            )?;
            self.session.check(self.deadline)?;
            require(Instant::now() < self.deadline)
        })();
        if result.is_err() {
            self.session.poisoned = true;
        }
        result
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Set,
    Object,
    Flowtable,
}
impl Kind {
    fn operation(self) -> u16 {
        NFT + match self {
            Self::Set => 10,
            Self::Object => 19,
            Self::Flowtable => 23,
        }
    }
    fn handle(self) -> usize {
        match self {
            Self::Set => 16,
            Self::Object => 6,
            Self::Flowtable => 5,
        }
    }
    fn padding(self) -> usize {
        match self {
            Self::Set => 14,
            Self::Object => 7,
            Self::Flowtable => 6,
        }
    }
    fn max_attr(self) -> usize {
        match self {
            Self::Set => 20,
            Self::Object => 8,
            Self::Flowtable => 7,
        }
    }
    fn nested(self, kind: usize) -> bool {
        match self {
            Self::Set => matches!(kind, 9 | 17 | 18),
            Self::Object => kind == 4,
            Self::Flowtable => kind == 3,
        }
    }
    fn flags(self) -> u16 {
        match self {
            Self::Set => 2,
            Self::Object | Self::Flowtable => 0x802,
        }
    }
}

// We need only complete absence. Any object, including an unknown nested
// expression kind, prevents Exact. No nested object payload is interpreted as
// safe policy or skipped to manufacture emptiness.
struct ObjectDump {
    kind: Kind,
    request: Vec<u8>,
    port: u32,
    generation: u32,
    done: bool,
    bytes: usize,
    datagrams: usize,
    messages: usize,
    names: BTreeSet<Vec<u8>>,
    handles: BTreeSet<u64>,
}
impl ObjectDump {
    fn new(kind: Kind, sequence: u32, port: u32, generation: u32) -> Result<Self> {
        require(sequence != 0 && port != 0 && generation != 0)?;
        Ok(Self {
            kind,
            request: message(
                kind.operation(),
                0x301,
                sequence,
                0,
                &[vec![1, 0, 0, 0], attribute(1, TABLE)].concat(),
            ),
            port,
            generation,
            done: false,
            bytes: 0,
            datagrams: 0,
            messages: 0,
            names: BTreeSet::new(),
            handles: BTreeSet::new(),
        })
    }

    fn object(&mut self, body: &[u8]) -> Result<()> {
        require(
            body.len() >= 4
                && body[..2] == [1, 0]
                && body[2..4] == (self.generation as u16).to_be_bytes(),
        )?;
        let mut rest = &body[4..];
        let mut fields = vec![None; self.kind.max_attr() + 1];
        let mut count = 0;
        while !rest.is_empty() {
            count += 1;
            require(rest.len() >= 4 && count <= 32)?;
            let length = usize::from(u16n(&rest[..2])?);
            let raw = u16n(&rest[2..4])?;
            let kind = usize::from(raw & 0x3fff);
            require(
                length >= 4
                    && aligned(length) <= rest.len()
                    && kind != 0
                    && kind <= self.kind.max_attr()
                    && raw & 0x4000 == 0
                    && (raw & 0x8000 == 0 || self.kind.nested(kind))
                    && rest[length..aligned(length)].iter().all(|v| *v == 0),
            )?;
            let data = &rest[4..length];
            if kind == self.kind.padding() {
                require(data.is_empty())?;
            } else {
                require(fields[kind].replace(data).is_none())?;
            }
            rest = &rest[aligned(length)..];
        }
        // Kernel requests are table-filtered. A foreign response is uncertainty,
        // never a reason to skip records and claim our own table is empty.
        require(fields[1] == Some(TABLE))?;
        let name = fields[2].ok_or(REFUSE)?;
        require(
            (2..=256).contains(&name.len())
                && name.last() == Some(&0)
                && !name[..name.len() - 1].contains(&0)
                && self.names.insert(name.to_vec()),
        )?;
        let handle = u64::from_be_bytes(
            fields[self.kind.handle()]
                .ok_or(REFUSE)?
                .try_into()
                .map_err(|_| REFUSE)?,
        );
        require(handle != 0 && self.handles.insert(handle))?;
        Ok(())
    }

    fn receive(
        &mut self,
        mut bytes: &[u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
    ) -> Result<()> {
        require(!self.done && sender == Some(NetlinkAddr::new(0, 0)) && flags.is_empty())?;
        self.bytes = self.bytes.checked_add(bytes.len()).ok_or(REFUSE)?;
        self.datagrams += 1;
        require(!bytes.is_empty() && self.bytes <= 64 * 1024 && self.datagrams <= 32)?;
        while !bytes.is_empty() {
            self.messages += 1;
            require(bytes.len() >= 16 && self.messages <= 64)?;
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
            if kind == 3 {
                require(
                    matches!(flags, 0 | 2)
                        && bytes[16..length] == [0; 4]
                        && aligned(length) == bytes.len(),
                )?;
                self.done = true;
            } else {
                require(kind == self.kind.operation() - 1 && flags == self.kind.flags())?;
                self.object(&bytes[16..length])?;
            }
            bytes = &bytes[aligned(length)..];
        }
        Ok(())
    }
    fn empty(&self) -> Result<bool> {
        require(self.done)?;
        Ok(self.names.is_empty())
    }
}

fn classify(
    table: &TableMetadata,
    port: u32,
    chains: LocalChainInventory,
    rules: LocalRuleInventory,
    extras_empty: bool,
) -> LocalPolicyInventory {
    if table.flags == 6
        && table.owner == Some(port)
        && table.uses == 1
        && table.userdata.is_none()
        && extras_empty
        && chains == LocalChainInventory::ExpectedOutputChainUntrusted
        && let LocalRuleInventory::ExactRulesUntrusted(policy) = rules
    {
        return LocalPolicyInventory::ExactUntrusted(policy);
    }
    LocalPolicyInventory::OtherUntrusted
}

impl LocalReadSession {
    fn dump_objects(
        &self,
        kind: Kind,
        sequence: u32,
        generation: u32,
        deadline: Instant,
    ) -> Result<bool> {
        self.check(deadline)?;
        let mut dump = ObjectDump::new(kind, sequence, self.local.pid(), generation)?;
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
        dump.empty()
    }

    /// Inspect table identity, chains, rules, sets/maps, objects and flowtables
    /// inside one bounded generation window on this retained descriptor. Even
    /// ExactUntrusted cannot become a receipt, Table::OwnedVerified or EffectPort.
    pub fn inspect_policy_inventory(&mut self) -> Result<LocalPolicyInventory> {
        self.borrow_policy_inventory().map(|lease| lease.observed())
    }

    /// Borrow a complete generation-bracketed inventory on the actual retained
    /// session. No caller-supplied descriptor, epoch or inventory can construct it.
    pub fn borrow_policy_inventory(&mut self) -> Result<LocalInventoryLease<'_>> {
        let deadline = Instant::now() + Duration::from_secs(1);
        let result = self.inspect_policy_inventory_before(deadline);
        if result.is_err() {
            self.poisoned = true;
        }
        let (inventory, generation, table) = result?;
        Ok(LocalInventoryLease {
            session: self,
            inventory,
            generation,
            table,
            deadline,
        })
    }
    #[cfg(test)]
    pub(super) fn inspect_policy_inventory_once(
        &mut self,
    ) -> Result<(LocalPolicyInventory, u32, Option<TableMetadata>)> {
        let lease = self.borrow_policy_inventory()?;
        Ok((lease.inventory, lease.generation, lease.table))
    }

    fn inspect_policy_inventory_before(
        &mut self,
        deadline: Instant,
    ) -> Result<(LocalPolicyInventory, u32, Option<TableMetadata>)> {
        self.check(deadline)?;
        let first = self.next_sequence;
        require(first != 0)?;
        self.next_sequence = first.checked_add(9).ok_or(REFUSE)?;
        let generation = self.exchange(GET_GEN, first, deadline)?.generation()?;
        let before = self
            .exchange(GET_TABLE, first + 1, deadline)?
            .table_metadata(generation)?;
        let result = if let Some(table) = &before {
            let chains = self
                .dump_chains(first + 2, generation, deadline)?
                .classify()?;
            let rules = self
                .dump_rules(first + 3, generation, deadline)?
                .classify()?;
            let mut extras_empty = true;
            for (offset, kind) in [(4, Kind::Set), (5, Kind::Object), (6, Kind::Flowtable)] {
                extras_empty &= self.dump_objects(kind, first + offset, generation, deadline)?;
            }
            classify(table, self.local.pid(), chains, rules, extras_empty)
        } else {
            LocalPolicyInventory::TableAbsent
        };
        let after = self
            .exchange(GET_TABLE, first + 7, deadline)?
            .table_metadata(generation)?;
        require(before == after)?;
        require(self.exchange(GET_GEN, first + 8, deadline)?.generation()? == generation)?;
        self.check(deadline)?;
        Ok((result, generation, before))
    }
}

#[cfg(test)]
#[path = "kernel_inventory_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "kernel_inventory_lease_tests.rs"]
mod lease_tests;
