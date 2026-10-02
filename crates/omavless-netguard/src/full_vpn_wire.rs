// SPDX-License-Identifier: MIT
//! Pure fixed FullVpn atomic-create candidate. No I/O, ownership or executor.
//! General kernel compatibility and packet enforcement are NOT proven; the
//! separate opt-in fixture supplies only its exact documented mechanism gate.
//! This is not a selectable-policy or general expression-building API.

use crate::emergency_wire::{attr, expression, message, nested, nf, verdict};

#[path = "full_vpn_reply.rs"]
pub mod reply;

const TABLE: &[u8] = b"omavless_netguard\0";
const CHAIN: &[u8] = b"output_guard\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFence;

/// Wire material, never evidence that anything was sent or installed.
#[derive(Debug, PartialEq, Eq)]
pub struct FullVpnCreate {
    batch: Vec<u8>,
    barrier: Vec<u8>,
}
impl FullVpnCreate {
    pub fn batch(&self) -> &[u8] {
        &self.batch
    }
    pub fn barrier(&self) -> &[u8] {
        &self.barrier
    }
}

/// Begin s, table/chain s+1..2, ten rules s+3..12, end s+13, GETGEN s+14.
/// Generation freshness and sequence non-reuse belong to a future session.
pub fn encode(generation: u32, first_sequence: u32) -> Result<FullVpnCreate, InvalidFence> {
    if generation == 0 || first_sequence == 0 || first_sequence.checked_add(14).is_none() {
        return Err(InvalidFence);
    }
    let mut batch = message(
        16,
        1,
        first_sequence,
        &[nf(0, 10), attr(1, &generation.to_be_bytes())].concat(),
    );
    let table = [attr(1, TABLE), attr(2, &6_u32.to_be_bytes())].concat();
    let chain = [
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
    .concat();
    for (offset, kind, data) in [(1, 0xa00, table), (2, 0xa03, chain)] {
        batch.extend(message(
            kind,
            0x605,
            first_sequence + offset,
            &[nf(1, 0), data].concat(),
        ));
    }
    for (offset, expressions) in (3..=12).zip(rules()) {
        let rule = [attr(1, TABLE), attr(2, CHAIN), nested(4, &expressions)].concat();
        batch.extend(message(
            0xa06,
            0xc05,
            first_sequence + offset,
            &[nf(1, 0), rule].concat(),
        ));
    }
    batch.extend(message(17, 1, first_sequence + 13, &nf(0, 10)));
    Ok(FullVpnCreate {
        batch,
        barrier: message(0xa10, 5, first_sequence + 14, &nf(0, 0)),
    })
}

// Register 1 is a 16-byte NFT_REG_1. Every load is immediately checked; no
// expressions consume uninitialized bytes. Attribute scalars are network order;
// cmp data retains its source representation (notably native-endian skb mark).
fn compare(value: &[u8]) -> Vec<u8> {
    expression(
        b"cmp\0",
        &[
            attr(1, &1_u32.to_be_bytes()),
            attr(2, &0_u32.to_be_bytes()),
            nested(3, &attr(1, value)),
        ]
        .concat(),
    )
}
fn meta(key: u32, value: &[u8]) -> Vec<u8> {
    [
        expression(
            b"meta\0",
            &[attr(1, &1_u32.to_be_bytes()), attr(2, &key.to_be_bytes())].concat(),
        ),
        compare(value),
    ]
    .concat()
}
fn load(base: u32, offset: u32, length: u32) -> Vec<u8> {
    expression(
        b"payload\0",
        &[
            attr(1, &1_u32.to_be_bytes()),
            attr(2, &base.to_be_bytes()),
            attr(3, &offset.to_be_bytes()),
            attr(4, &length.to_be_bytes()),
        ]
        .concat(),
    )
}
fn payload(base: u32, offset: u32, value: &[u8]) -> Vec<u8> {
    [
        load(
            base,
            offset,
            u32::try_from(value.len()).expect("fixed payload"),
        ),
        compare(value),
    ]
    .concat()
}
fn prefix(offset: u32, address: &[u8; 16], bits: usize) -> Vec<u8> {
    let mask: Vec<u8> = (0..16)
        .map(|i| match bits.saturating_sub(i * 8) {
            0 => 0,
            1..=7 => 0xff << (8 - (bits - i * 8)),
            _ => 0xff,
        })
        .collect();
    let masked: Vec<u8> = address.iter().zip(&mask).map(|(a, m)| a & m).collect();
    [
        load(1, offset, 16),
        expression(
            b"bitwise\0",
            &[
                attr(1, &1_u32.to_be_bytes()),
                attr(2, &1_u32.to_be_bytes()),
                attr(3, &16_u32.to_be_bytes()),
                nested(4, &attr(1, &mask)),
                nested(5, &attr(1, &[0; 16])),
            ]
            .concat(),
        ),
        compare(&masked),
    ]
    .concat()
}
fn ipv6(text: &str) -> [u8; 16] {
    text.parse::<std::net::Ipv6Addr>()
        .expect("fixed IPv6 literal")
        .octets()
}
pub(super) fn rules() -> [Vec<u8>; 10] {
    // nfproto and l4proto precede all payload accesses. Transport-header base
    // follows kernel protocol parsing, not fixed IPv4/IPv6 header-size guesses.
    // Failed transport access (including noninitial fragments) must break rule.
    let udp4 = [meta(15, &[2]), meta(16, &[17])].concat();
    let udp6 = [meta(15, &[10]), meta(16, &[17])].concat();
    let nd = [meta(15, &[10]), meta(16, &[58])].concat();
    let link = ipv6("fe80::");
    let rs = |source: Vec<u8>| {
        [
            nd.clone(),
            source,
            payload(1, 24, &ipv6("ff02::2")),
            payload(1, 7, &[255]),
            payload(2, 0, &[133, 0]),
            verdict(1),
        ]
        .concat()
    };
    [
        [meta(7, b"lo\0"), verdict(1)].concat(),
        [meta(7, b"omavless0\0"), verdict(1)].concat(),
        [meta(3, &0x4f4d4101_u32.to_ne_bytes()), verdict(1)].concat(),
        [
            udp4,
            payload(1, 16, &[255; 4]),
            payload(2, 0, &[0, 68, 0, 67]),
            verdict(1),
        ]
        .concat(),
        [
            udp6,
            prefix(8, &link, 10),
            payload(1, 24, &ipv6("ff02::1:2")),
            payload(2, 0, &[2, 34, 2, 35]),
            verdict(1),
        ]
        .concat(),
        rs(payload(1, 8, &[0; 16])),
        rs(prefix(8, &link, 10)),
        [
            nd.clone(),
            prefix(24, &ipv6("ff02::1:ff00:0"), 104),
            payload(1, 7, &[255]),
            payload(2, 0, &[135, 0]),
            verdict(1),
        ]
        .concat(),
        [
            nd,
            prefix(8, &link, 10),
            prefix(24, &link, 10),
            payload(1, 7, &[255]),
            payload(2, 0, &[136, 0]),
            verdict(1),
        ]
        .concat(),
        verdict(0),
    ]
}

#[cfg(test)]
#[path = "full_vpn_wire_tests.rs"]
mod tests;
