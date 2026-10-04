// SPDX-License-Identifier: MIT
//! Pure fixed Emergency create encoding. No socket, executor or ownership proof.
//!
//! The entire table, base chain and two rules share one generation-fenced batch.
//! Sending these bytes is NOT authorized by constructing them. A future executor
//! needs the canonical namespace and retained creator prerequisites, complete ACK
//! validation, full-policy readback and durable transaction ordering.

const NFT: u16 = 10 << 8;
const REQUEST: u16 = 1;
const ACK: u16 = 4;
const CREATE: u16 = 0x400;
const EXCL: u16 = 0x200;
const APPEND: u16 = 0x800;
const TABLE: &[u8] = b"omavless_netguard\0";
const CHAIN: &[u8] = b"output_guard\0";

#[path = "emergency_reply.rs"]
pub mod reply;

/// Invalid/ambiguous input is refused before producing any bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFence;

/// Fixed wire material only; neither authority nor evidence of an effect.
#[derive(Debug, PartialEq, Eq)]
pub struct EmergencyCreate {
    batch: Vec<u8>,
    barrier: Vec<u8>,
}

impl EmergencyCreate {
    /// One complete atomic nf_tables batch, with four operation ACK requests.
    pub fn batch(&self) -> &[u8] {
        &self.batch
    }

    /// Separate GETGEN request used only as an ordered reply barrier.
    /// Its success cannot substitute for operation ACKs or policy readback.
    pub fn barrier(&self) -> &[u8] {
        &self.barrier
    }
}

/// Encode only exclusive `owner,persist` Emergency creation in the inet family.
/// Sequence offsets: begin 0; table/chain/rules 1..=4; end 5; GETGEN 6.
/// Zero and wraparound are rejected. Generation freshness is a kernel decision,
/// not a fact this pure function establishes. The caller owns sequence reuse.
pub fn encode(generation: u32, first_sequence: u32) -> Result<EmergencyCreate, InvalidFence> {
    if generation == 0 || first_sequence == 0 || first_sequence.checked_add(6).is_none() {
        return Err(InvalidFence);
    }
    let mut batch = message(
        16,
        REQUEST,
        first_sequence,
        &[nf(0, 10), attr(1, &generation.to_be_bytes())].concat(),
    );
    let operations = [
        (
            NFT,
            CREATE | EXCL,
            [attr(1, TABLE), attr(2, &6_u32.to_be_bytes())].concat(),
        ),
        (
            NFT + 3,
            CREATE | EXCL,
            [
                attr(1, TABLE),
                attr(3, CHAIN),
                nested(
                    4,
                    &[
                        attr(1, &3_u32.to_be_bytes()),
                        attr(2, &300_u32.to_be_bytes()),
                    ]
                    .concat(),
                ),
                attr(5, &0_u32.to_be_bytes()),
                attr(7, b"filter\0"),
            ]
            .concat(),
        ),
        (NFT + 6, CREATE | APPEND, rule(true)),
        (NFT + 6, CREATE | APPEND, rule(false)),
    ];
    for (offset, (kind, flags, attrs)) in (1..=4).zip(operations) {
        batch.extend(message(
            kind,
            flags | REQUEST | ACK,
            first_sequence + offset,
            &[nf(1, 0), attrs].concat(),
        ));
    }
    batch.extend(message(17, REQUEST, first_sequence + 5, &nf(0, 10)));
    Ok(EmergencyCreate {
        batch,
        barrier: message(NFT + 16, REQUEST | ACK, first_sequence + 6, &nf(0, 0)),
    })
}

// All helpers are private, fixed small inputs; no arbitrary expressions/attrs.
pub(super) fn attr(kind: u16, value: &[u8]) -> Vec<u8> {
    let len = u16::try_from(4 + value.len()).expect("fixed attribute size");
    let mut bytes = Vec::from(len.to_ne_bytes());
    bytes.extend(kind.to_ne_bytes());
    bytes.extend(value);
    bytes.resize(usize::from(len).next_multiple_of(4), 0);
    bytes
}

pub(super) fn nested(kind: u16, value: &[u8]) -> Vec<u8> {
    attr(kind | 0x8000, value)
}

pub(super) fn nf(family: u8, resource: u16) -> Vec<u8> {
    [vec![family, 0], resource.to_be_bytes().to_vec()].concat()
}

pub(super) fn message(kind: u16, flags: u16, sequence: u32, payload: &[u8]) -> Vec<u8> {
    [
        u32::try_from(16 + payload.len())
            .expect("fixed message size")
            .to_ne_bytes()
            .to_vec(),
        kind.to_ne_bytes().to_vec(),
        flags.to_ne_bytes().to_vec(),
        sequence.to_ne_bytes().to_vec(),
        0_u32.to_ne_bytes().to_vec(),
        payload.to_vec(),
    ]
    .concat()
}

pub(super) fn expression(name: &[u8], data: &[u8]) -> Vec<u8> {
    nested(1, &[attr(1, name), nested(2, data)].concat())
}

pub(super) fn verdict(code: u32) -> Vec<u8> {
    expression(
        b"immediate\0",
        &[
            attr(1, &0_u32.to_be_bytes()),
            nested(2, &nested(2, &attr(1, &code.to_be_bytes()))),
        ]
        .concat(),
    )
}

fn rule(loopback: bool) -> Vec<u8> {
    let mut expr = Vec::new();
    if loopback {
        expr.extend(expression(
            b"meta\0",
            &[attr(1, &1_u32.to_be_bytes()), attr(2, &7_u32.to_be_bytes())].concat(),
        ));
        expr.extend(expression(
            b"cmp\0",
            &[
                attr(1, &1_u32.to_be_bytes()),
                attr(2, &0_u32.to_be_bytes()),
                nested(3, &attr(1, b"lo\0")),
            ]
            .concat(),
        ));
    }
    expr.extend(verdict(u32::from(loopback)));
    [attr(1, TABLE), attr(2, CHAIN), nested(4, &expr)].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_zero_and_sequence_wrap() {
        for (generation, sequence) in [(0, 1), (1, 0), (1, u32::MAX - 5), (u32::MAX, u32::MAX)] {
            assert_eq!(encode(generation, sequence), Err(InvalidFence));
        }
        assert!(encode(u32::MAX, u32::MAX - 6).is_ok());
    }

    #[test]
    fn fixed_message_order_flags_and_fences() {
        let encoded = encode(0x01020304, 20).unwrap();
        let mut rest = encoded.batch();
        for (i, (kind, flags)) in [
            (16, 1),
            (NFT, 0x605),
            (NFT + 3, 0x605),
            (NFT + 6, 0xc05),
            (NFT + 6, 0xc05),
            (17, 1),
        ]
        .into_iter()
        .enumerate()
        {
            let size = u32::from_ne_bytes(rest[..4].try_into().unwrap()) as usize;
            assert_eq!(u16::from_ne_bytes(rest[4..6].try_into().unwrap()), kind);
            assert_eq!(u16::from_ne_bytes(rest[6..8].try_into().unwrap()), flags);
            assert_eq!(
                u32::from_ne_bytes(rest[8..12].try_into().unwrap()),
                20 + i as u32
            );
            assert_eq!(&rest[12..16], &[0; 4]);
            assert_eq!(size % 4, 0);
            rest = &rest[size..];
        }
        assert!(rest.is_empty());
        assert_eq!(&encoded.batch()[24..28], &[1, 2, 3, 4]);
        assert_eq!(encoded.barrier(), message(NFT + 16, 5, 26, &nf(0, 0)));
        assert!(encoded.batch().len() < 1024);
    }
}
