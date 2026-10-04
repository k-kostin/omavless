// SPDX-License-Identifier: MIT
//! Normal-compiled, inactive fixed atomic wire contract. No sockets or effects.
//! Metadata is supplied, not authenticated here. Every result is UNTRUSTED:
//! none proves namespace identity, durable ownership, or permission to mutate.
//! This is distinct from the public legacy FullVpnTranscript GETGEN contract.
#![allow(dead_code)]
use super::{LIMIT, NFT, REFUSE, Result, aligned, attribute, message, require, u16n, u32n};
use nix::sys::socket::{MsgFlags, NetlinkAddr};

#[cfg(test)]
#[path = "kernel_atomic_batch_tests.rs"]
mod tests;

/// Constructible only by the fixed create/replace/delete encoders below.
/// Read-only dereferencing permits transport without arbitrary-message admission.
#[derive(Clone)]
pub(super) struct AtomicBatch(Vec<Vec<u8>>);
impl std::ops::Deref for AtomicBatch {
    type Target = [Vec<u8>];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Structural parser state only; not an effect receipt or trusted refusal.
/// Uncertainty is permanent; starting a new collector cannot authorize a retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UntrustedStatus {
    Incomplete,
    AllAcknowledged,
    GenerationRefused,
    Uncertain,
}
/// Build one atomic fixed FullVpn create or delete+exclusive-create replacement.
/// All messages require ACKs, especially END (commit), with no separate GETGEN
/// used to turn queued operation replies into an effect receipt.
pub(super) fn full_batch(generation: u32, first: u32, old: Option<u64>) -> Result<AtomicBatch> {
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
    Ok(AtomicBatch(requests))
}

/// Exact begin/each-operation/end structural UNTRUSTED classification.
/// Unknown errors poison; only ERESTART on BEGIN before any success classifies
/// as generation refusal. No classification is a trusted effect receipt.
#[derive(Clone)]
pub(super) struct AtomicReplies {
    requests: AtomicBatch,
    acks: Vec<bool>,
    port: u32,
    changed: bool,
    poisoned: bool,
    total: usize,
    datagrams: usize,
}
impl AtomicReplies {
    pub(super) fn requests(&self) -> &AtomicBatch {
        &self.requests
    }
    pub(super) fn acks(&self) -> &[bool] {
        &self.acks
    }
    pub(super) fn changed(&self) -> bool {
        self.changed
    }
    pub(super) fn poisoned(&self) -> bool {
        self.poisoned
    }
    pub(super) fn poison(&mut self) {
        self.poisoned = true;
    }
    pub(super) fn new(requests: AtomicBatch, port: u32) -> Result<Self> {
        require(port != 0 && matches!(requests.len(), 3 | 14 | 15))?;
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
    pub(super) fn status(&self) -> UntrustedStatus {
        if self.poisoned {
            UntrustedStatus::Uncertain
        } else if self.changed {
            UntrustedStatus::GenerationRefused
        } else if self.acks.iter().all(|ack| *ack) {
            UntrustedStatus::AllAcknowledged
        } else {
            UntrustedStatus::Incomplete
        }
    }
    pub(super) fn complete(&self) -> bool {
        matches!(
            self.status(),
            UntrustedStatus::AllAcknowledged | UntrustedStatus::GenerationRefused
        )
    }
    pub(super) fn receive(
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
        require(
            !bytes.is_empty()
                && self.total <= LIMIT
                && self.datagrams <= if self.requests.len() == 3 { 8 } else { 16 },
        )?;
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

pub(super) fn delete_batch(generation: u32, handle: u64, first: u32) -> Result<AtomicBatch> {
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
    Ok(AtomicBatch(vec![begin, delete, end]))
}

/// Fixed negative drift fixture only; never compiled into the normal contract.
#[cfg(test)]
pub(super) fn drift_batch(generation: u32, first: u32) -> Result<AtomicBatch> {
    require(generation != 0 && first != 0 && first.checked_add(3).is_some())?;
    let begin = message(
        16,
        5,
        first,
        0,
        &[vec![0, 0, 0, 10], attribute(1, &generation.to_be_bytes())].concat(),
    );
    let append = message(
        NFT + 6,
        0xc05,
        first + 1,
        0,
        &[
            vec![1, 0, 0, 0],
            attribute(1, super::TABLE),
            attribute(2, b"output_guard\0"),
            crate::emergency_wire::nested(4, &crate::emergency_wire::verdict(1)),
        ]
        .concat(),
    );
    let end = message(17, 5, first + 2, 0, &[0, 0, 0, 10]);
    Ok(AtomicBatch(vec![begin, append, end]))
}

/// One literal empty foreign table in the isolated retained-lease fixture only.
/// It exists solely to advance the real kernel generation between read/send.
#[cfg(test)]
pub(super) fn lease_generation_cut_batch(generation: u32, first: u32) -> Result<AtomicBatch> {
    require(generation != 0 && first != 0 && first.checked_add(3).is_some())?;
    Ok(AtomicBatch(vec![
        message(
            16,
            5,
            first,
            0,
            &[vec![0, 0, 0, 10], attribute(1, &generation.to_be_bytes())].concat(),
        ),
        message(
            NFT,
            0x605,
            first + 1,
            0,
            &[vec![1, 0, 0, 0], attribute(1, b"k1_retained_lease_cut\0")].concat(),
        ),
        message(17, 5, first + 2, 0, &[0, 0, 0, 10]),
    ]))
}
