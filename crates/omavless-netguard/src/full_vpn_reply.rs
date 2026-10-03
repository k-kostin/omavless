// SPDX-License-Identifier: MIT
//! Pure, bounded decoder for the fixed FullVpn-create reply transcript.
//!
//! The caller supplies receive metadata; this module opens no socket and cannot
//! authenticate that metadata, the namespace, the kernel effect or table owner.
//! A complete transcript is not permission to mutate or a durable receipt.
//!
//! Legacy inactive contract: operation ACKs plus a separate GETGEN barrier.
//! It does not establish atomic BEGIN/every-operation/commit-END completeness.
//! Kernel fixtures use the distinct crate-private atomic collector; this public
//! API remains unchanged and must not be substituted for that commit contract.

use super::{FullVpnCreate, encode};

const LIMIT: usize = 32 * 1024;
const MAX_DATAGRAMS: usize = 16;
const ERROR: u16 = 2;
const NEWGEN: u16 = (10 << 8) + 15;
const CAPPED: u16 = 0x100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidTranscript;

/// Structural completeness only. Not effect, namespace or ownership evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteUntrustedTranscript {
    pub observed_generation: u32,
}

/// One closed, fixed request transcript. Any invalid input poisons the instance.
pub struct FullVpnTranscript {
    expected: [[u8; 16]; 13],
    operation_acks: [bool; 12],
    barrier_ack: bool,
    generation: Option<u32>,
    first_sequence: u32,
    local_port: u32,
    bytes: usize,
    datagrams: usize,
    poisoned: bool,
    finished: bool,
}

fn require(ok: bool) -> Result<(), InvalidTranscript> {
    if ok { Ok(()) } else { Err(InvalidTranscript) }
}
fn u16n(bytes: &[u8]) -> Result<u16, InvalidTranscript> {
    Ok(u16::from_ne_bytes(
        bytes.try_into().map_err(|_| InvalidTranscript)?,
    ))
}
fn u32n(bytes: &[u8]) -> Result<u32, InvalidTranscript> {
    Ok(u32::from_ne_bytes(
        bytes.try_into().map_err(|_| InvalidTranscript)?,
    ))
}
fn aligned(n: usize) -> Result<usize, InvalidTranscript> {
    n.checked_add(3).map(|n| n & !3).ok_or(InvalidTranscript)
}

fn expected_headers(wire: &FullVpnCreate) -> Result<[[u8; 16]; 13], InvalidTranscript> {
    let mut rest = wire.batch();
    let mut expected = [[0; 16]; 13];
    for index in 0..14 {
        require(rest.len() >= 16)?;
        let length = usize::try_from(u32n(&rest[..4])?).map_err(|_| InvalidTranscript)?;
        require(length >= 16 && length <= rest.len() && aligned(length)? <= rest.len())?;
        if (1..=12).contains(&index) {
            expected[index - 1].copy_from_slice(&rest[..16]);
        }
        rest = &rest[aligned(length)?..];
    }
    require(rest.is_empty() && wire.barrier().len() >= 16)?;
    expected[12].copy_from_slice(&wire.barrier()[..16]);
    Ok(expected)
}

impl FullVpnTranscript {
    /// Uses only the fixed encoder. `local_port` must be the bound transport's
    /// port; a supplied number is not independently authenticated here.
    pub fn new(
        request_generation: u32,
        first_sequence: u32,
        local_port: u32,
    ) -> Result<Self, InvalidTranscript> {
        require(local_port != 0)?;
        let wire = encode(request_generation, first_sequence).map_err(|_| InvalidTranscript)?;
        Ok(Self {
            expected: expected_headers(&wire)?,
            operation_acks: [false; 12],
            barrier_ack: false,
            generation: None,
            first_sequence,
            local_port,
            bytes: 0,
            datagrams: 0,
            poisoned: false,
            finished: false,
        })
    }

    /// `sender_port/groups` and receive flags come from a future trusted
    /// transport. Passing fabricated metadata here does not authenticate it.
    pub fn push_datagram(
        &mut self,
        bytes: &[u8],
        sender_port: u32,
        sender_groups: u32,
        receive_flags: u32,
    ) -> Result<(), InvalidTranscript> {
        if self.poisoned || self.finished {
            return Err(InvalidTranscript);
        }
        if self
            .push_inner(bytes, sender_port, sender_groups, receive_flags)
            .is_err()
        {
            self.poisoned = true;
            return Err(InvalidTranscript);
        }
        Ok(())
    }

    fn push_inner(
        &mut self,
        mut bytes: &[u8],
        sender_port: u32,
        sender_groups: u32,
        receive_flags: u32,
    ) -> Result<(), InvalidTranscript> {
        require(sender_port == 0 && sender_groups == 0 && receive_flags == 0)?;
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or(InvalidTranscript)?;
        self.datagrams = self.datagrams.checked_add(1).ok_or(InvalidTranscript)?;
        require(!bytes.is_empty() && self.bytes <= LIMIT && self.datagrams <= MAX_DATAGRAMS)?;
        while !bytes.is_empty() {
            require(bytes.len() >= 16)?;
            let length = usize::try_from(u32n(&bytes[..4])?).map_err(|_| InvalidTranscript)?;
            require(length >= 16 && aligned(length)? <= bytes.len())?;
            require(bytes[length..aligned(length)?].iter().all(|b| *b == 0))?;
            let kind = u16n(&bytes[4..6])?;
            let flags = u16n(&bytes[6..8])?;
            let sequence = u32n(&bytes[8..12])?;
            let port = u32n(&bytes[12..16])?;
            require(port == self.local_port)?;
            let body = &bytes[16..length];
            if kind == ERROR {
                self.accept_ack(flags, sequence, body)?;
            } else {
                require(kind == NEWGEN && flags == 0)?;
                require(sequence == self.first_sequence + 14 && self.generation.is_none())?;
                self.generation = Some(parse_generation(body)?);
            }
            bytes = &bytes[aligned(length)?..];
        }
        Ok(())
    }

    fn accept_ack(
        &mut self,
        flags: u16,
        sequence: u32,
        body: &[u8],
    ) -> Result<(), InvalidTranscript> {
        require(flags == 0 || flags == CAPPED)?;
        require(body.len() == 20)?;
        require(i32::from_ne_bytes(body[..4].try_into().map_err(|_| InvalidTranscript)?) == 0)?;
        let offset = sequence
            .checked_sub(self.first_sequence)
            .ok_or(InvalidTranscript)?;
        let index = match offset {
            1..=12 => usize::try_from(offset - 1).map_err(|_| InvalidTranscript)?,
            14 => 12,
            _ => return Err(InvalidTranscript),
        };
        require(body[4..20] == self.expected[index])?;
        if index == 12 {
            require(!self.barrier_ack)?;
            self.barrier_ack = true;
        } else {
            require(!self.operation_acks[index])?;
            self.operation_acks[index] = true;
        }
        Ok(())
    }

    /// Closes the collector. Incompleteness is a permanent refusal, not a
    /// retryable request for another datagram or a successful kernel effect.
    pub fn finish(&mut self) -> Result<CompleteUntrustedTranscript, InvalidTranscript> {
        if self.poisoned || self.finished {
            return Err(InvalidTranscript);
        }
        self.finished = true;
        if !self.barrier_ack || !self.operation_acks.iter().all(|ack| *ack) {
            self.poisoned = true;
            return Err(InvalidTranscript);
        }
        let observed_generation = match self.generation {
            Some(value) => value,
            None => {
                self.poisoned = true;
                return Err(InvalidTranscript);
            }
        };
        Ok(CompleteUntrustedTranscript {
            observed_generation,
        })
    }
}

fn parse_generation(body: &[u8]) -> Result<u32, InvalidTranscript> {
    require(body.len() >= 4 && body[0] == 0 && body[1] == 0)?;
    let mut attrs: [Option<&[u8]>; 4] = [None; 4];
    let mut rest = &body[4..];
    while !rest.is_empty() {
        require(rest.len() >= 4)?;
        let length = usize::from(u16n(&rest[..2])?);
        let kind = usize::from(u16n(&rest[2..4])?);
        require(length >= 4 && aligned(length)? <= rest.len())?;
        require((1..=3).contains(&kind) && attrs[kind].is_none())?;
        require(rest[length..aligned(length)?].iter().all(|b| *b == 0))?;
        attrs[kind] = Some(&rest[4..length]);
        rest = &rest[aligned(length)?..];
    }
    let value = u32::from_be_bytes(
        attrs[1]
            .ok_or(InvalidTranscript)?
            .try_into()
            .map_err(|_| InvalidTranscript)?,
    );
    require(value != 0 && body[2..4] == (value as u16).to_be_bytes())?;
    if let Some(pid) = attrs[2] {
        let _: [u8; 4] = pid.try_into().map_err(|_| InvalidTranscript)?;
    }
    if let Some(name) = attrs[3] {
        require(!name.is_empty() && name.len() <= 16 && name.last() == Some(&0))?;
        require(!name[..name.len() - 1].contains(&0))?;
    }
    Ok(value)
}

#[cfg(test)]
#[path = "full_vpn_reply_tests.rs"]
mod tests;
