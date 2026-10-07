// SPDX-License-Identifier: MIT
// Independent structural decoder: no encoder helpers or production rule builder.
use super::encode;

fn be(b: &[u8]) -> u32 {
    u32::from_be_bytes(b.try_into().unwrap())
}
fn attrs(mut b: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    while !b.is_empty() {
        assert!(b.len() >= 4);
        let n = u16::from_ne_bytes(b[..2].try_into().unwrap()) as usize;
        let t = u16::from_ne_bytes(b[2..4].try_into().unwrap());
        assert!(n >= 4 && n.next_multiple_of(4) <= b.len());
        assert!(b[n..n.next_multiple_of(4)].iter().all(|v| *v == 0));
        out.push((t, &b[4..n]));
        b = &b[n.next_multiple_of(4)..];
    }
    out
}
fn fields<'a>(b: &'a [u8], keys: &[u16]) -> Vec<&'a [u8]> {
    let values = attrs(b);
    assert_eq!(values.iter().map(|v| v.0).collect::<Vec<_>>(), keys);
    values.into_iter().map(|v| v.1).collect()
}
#[derive(Debug, PartialEq, Eq)]
enum Expr {
    Meta(u32),
    Load(u32, u32, u32),
    Mask(Vec<u8>),
    Equal(Vec<u8>),
    Verdict(u32),
}
fn expressions(b: &[u8]) -> Vec<Expr> {
    attrs(b)
        .into_iter()
        .map(|(kind, data)| {
            assert_eq!(kind, 0x8001);
            let v = fields(data, &[1, 0x8002]);
            match v[0] {
                b"meta\0" => {
                    let a = fields(v[1], &[1, 2]);
                    assert_eq!(be(a[0]), 1);
                    Expr::Meta(be(a[1]))
                }
                b"payload\0" => {
                    let a = fields(v[1], &[1, 2, 3, 4]);
                    assert_eq!(be(a[0]), 1);
                    Expr::Load(be(a[1]), be(a[2]), be(a[3]))
                }
                b"cmp\0" => {
                    let a = fields(v[1], &[1, 2, 0x8003]);
                    assert_eq!((be(a[0]), be(a[1])), (1, 0));
                    Expr::Equal(fields(a[2], &[1])[0].to_vec())
                }
                b"bitwise\0" => {
                    let a = fields(v[1], &[1, 2, 3, 0x8004, 0x8005]);
                    assert_eq!((be(a[0]), be(a[1]), be(a[2])), (1, 1, 16));
                    assert_eq!(fields(a[4], &[1])[0], &[0; 16]);
                    let mask = fields(a[3], &[1])[0];
                    assert_eq!(mask.len(), 16);
                    Expr::Mask(mask.to_vec())
                }
                b"immediate\0" => {
                    let a = fields(v[1], &[1, 0x8002]);
                    assert_eq!(be(a[0]), 0);
                    Expr::Verdict(be(fields(fields(a[1], &[0x8002])[0], &[1])[0]))
                }
                _ => panic!("unexpected expression"),
            }
        })
        .collect()
}

fn decode(generation: u32, seq: u32) -> Vec<Vec<Expr>> {
    let wire = encode(generation, seq).unwrap();
    assert!(wire.batch().len() < 8192);
    let mut b = wire.batch();
    let mut rules = Vec::new();
    for i in 0..14 {
        let length = u32::from_ne_bytes(b[..4].try_into().unwrap()) as usize;
        let kind = u16::from_ne_bytes(b[4..6].try_into().unwrap());
        let flags = u16::from_ne_bytes(b[6..8].try_into().unwrap());
        assert_eq!(u32::from_ne_bytes(b[8..12].try_into().unwrap()), seq + i);
        assert_eq!(&b[12..16], &[0; 4]);
        assert_eq!(length % 4, 0);
        let payload = &b[16..length];
        assert_eq!(
            &payload[..4],
            if i == 0 || i == 13 {
                &[0, 0, 0, 10]
            } else {
                &[1, 0, 0, 0]
            }
        );
        match i {
            0 => {
                assert_eq!((kind, flags), (16, 1));
                assert_eq!(be(fields(&payload[4..], &[1])[0]), generation);
            }
            1 => {
                assert_eq!((kind, flags), (0xa00, 0x605));
                let a = fields(&payload[4..], &[1, 2]);
                assert_eq!(a[0], b"omavless_netguard\0");
                assert_eq!(be(a[1]), 6);
            }
            2 => {
                assert_eq!((kind, flags), (0xa03, 0x605));
                let a = fields(&payload[4..], &[1, 3, 0x8004, 5, 7]);
                assert_eq!(a[0], b"omavless_netguard\0");
                assert_eq!(a[1], b"output_guard\0");
                let hook = fields(a[2], &[1, 2]);
                assert_eq!((be(hook[0]), be(hook[1]), be(a[3])), (3, 300, 0));
                assert_eq!(a[4], b"filter\0");
            }
            3..=12 => {
                assert_eq!((kind, flags), (0xa06, 0xc05));
                let a = fields(&payload[4..], &[1, 2, 0x8004]);
                assert_eq!(a[0], b"omavless_netguard\0");
                assert_eq!(a[1], b"output_guard\0");
                rules.push(expressions(a[2]));
            }
            13 => {
                assert_eq!((kind, flags), (17, 1));
                assert_eq!(payload.len(), 4);
            }
            _ => unreachable!(),
        }
        b = &b[length..];
    }
    assert!(b.is_empty());
    let barrier = wire.barrier();
    assert_eq!(barrier.len(), 20);
    assert_eq!(
        &barrier[..8],
        &[
            20_u32.to_ne_bytes().as_slice(),
            0xa10_u16.to_ne_bytes().as_slice(),
            5_u16.to_ne_bytes().as_slice()
        ]
        .concat()
    );
    assert_eq!(
        u32::from_ne_bytes(barrier[8..12].try_into().unwrap()),
        seq + 14
    );
    assert_eq!(&barrier[12..], &[0; 8]);
    rules
}
fn eq(b: &[u8]) -> Expr {
    Expr::Equal(b.to_vec())
}
fn ip(text: &str) -> Vec<u8> {
    text.parse::<std::net::Ipv6Addr>()
        .unwrap()
        .octets()
        .to_vec()
}
fn link_mask() -> Vec<u8> {
    vec![255, 192, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

#[test]
fn cold_full_policy_has_no_conntrack_established_or_ct_mark_grant() {
    // The independent decoder rejects every unknown expression, including ct.
    // META_MARK is the current skb mark, not a conntrack-mark/state load. With
    // no core/TUN these fixed exceptions do not invent VPN connectivity; there
    // is no blanket established-flow accept or conntrack-mark restoration.
    let rules = decode(11, 71);
    assert_eq!(rules.len(), 10);
    assert_eq!(
        rules[2],
        vec![
            Expr::Meta(3),
            eq(&0x4f4d4101_u32.to_ne_bytes()),
            Expr::Verdict(1)
        ]
    );
    assert_eq!(rules[9], vec![Expr::Verdict(0)]);
    assert_eq!(
        rules
            .iter()
            .flatten()
            .filter(|e| matches!(e, Expr::Meta(3)))
            .count(),
        1
    );
}

#[test]
fn independently_decodes_every_fixed_expression_and_protocol_dependency() {
    use Expr::*;
    let expected = vec![
        vec![Meta(7), eq(b"lo\0"), Verdict(1)],
        vec![Meta(7), eq(b"omavless0\0"), Verdict(1)],
        vec![Meta(3), eq(&0x4f4d4101_u32.to_ne_bytes()), Verdict(1)],
        vec![
            Meta(15),
            eq(&[2]),
            Meta(16),
            eq(&[17]),
            Load(1, 16, 4),
            eq(&[255; 4]),
            Load(2, 0, 4),
            eq(&[0, 68, 0, 67]),
            Verdict(1),
        ],
        vec![
            Meta(15),
            eq(&[10]),
            Meta(16),
            eq(&[17]),
            Load(1, 8, 16),
            Mask(link_mask()),
            eq(&ip("fe80::")),
            Load(1, 24, 16),
            eq(&ip("ff02::1:2")),
            Load(2, 0, 4),
            eq(&[2, 34, 2, 35]),
            Verdict(1),
        ],
        vec![
            Meta(15),
            eq(&[10]),
            Meta(16),
            eq(&[58]),
            Load(1, 8, 16),
            eq(&[0; 16]),
            Load(1, 24, 16),
            eq(&ip("ff02::2")),
            Load(1, 7, 1),
            eq(&[255]),
            Load(2, 0, 2),
            eq(&[133, 0]),
            Verdict(1),
        ],
        vec![
            Meta(15),
            eq(&[10]),
            Meta(16),
            eq(&[58]),
            Load(1, 8, 16),
            Mask(link_mask()),
            eq(&ip("fe80::")),
            Load(1, 24, 16),
            eq(&ip("ff02::2")),
            Load(1, 7, 1),
            eq(&[255]),
            Load(2, 0, 2),
            eq(&[133, 0]),
            Verdict(1),
        ],
        vec![
            Meta(15),
            eq(&[10]),
            Meta(16),
            eq(&[58]),
            Load(1, 24, 16),
            Mask(vec![
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0,
            ]),
            eq(&ip("ff02::1:ff00:0")),
            Load(1, 7, 1),
            eq(&[255]),
            Load(2, 0, 2),
            eq(&[135, 0]),
            Verdict(1),
        ],
        vec![
            Meta(15),
            eq(&[10]),
            Meta(16),
            eq(&[58]),
            Load(1, 8, 16),
            Mask(link_mask()),
            eq(&ip("fe80::")),
            Load(1, 24, 16),
            Mask(link_mask()),
            eq(&ip("fe80::")),
            Load(1, 7, 1),
            eq(&[255]),
            Load(2, 0, 2),
            eq(&[136, 0]),
            Verdict(1),
        ],
        vec![Verdict(0)],
    ];
    for (generation, sequence) in [(1, 1), (0x01020304, 20), (u32::MAX, u32::MAX - 14)] {
        assert_eq!(decode(generation, sequence), expected);
    }
    // The independent JSON golden tests remain untouched. Pin this encoder's
    // rule count to that separate renderer too, without using it as byte oracle.
    let rendered: serde_json::Value =
        serde_json::from_slice(&crate::nft::render_create(crate::policy::Policy::FullVpn)).unwrap();
    let rule_count = rendered["nftables"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|command| command["add"].get("rule").is_some())
        .count();
    assert_eq!(expected.len(), rule_count);
}

#[derive(Clone)]
struct Packet {
    family: u8,
    protocol: u8,
    network: Vec<u8>,
    transport: Vec<u8>,
}
fn allows(rule: &[Expr], p: &Packet) -> bool {
    let mut reg = Vec::new();
    for e in rule {
        match e {
            Expr::Meta(15) => reg = vec![p.family],
            Expr::Meta(16) => reg = vec![p.protocol],
            Expr::Load(base, off, len) => {
                let input = if *base == 1 {
                    &p.network
                } else {
                    assert_eq!(*base, 2);
                    &p.transport
                };
                let Some(v) = input.get(*off as usize..(*off + *len) as usize) else {
                    return false;
                };
                reg = v.to_vec();
            }
            Expr::Mask(mask) => {
                assert_eq!(reg.len(), mask.len());
                for (v, m) in reg.iter_mut().zip(mask) {
                    *v &= m;
                }
            }
            Expr::Equal(value) => {
                if reg != *value {
                    return false;
                }
            }
            Expr::Verdict(v) => return *v == 1,
            _ => panic!("not a maintenance rule"),
        }
    }
    panic!("missing verdict")
}
#[test]
fn independent_maintenance_vectors_refuse_wrong_family_protocol_and_fields() {
    let rules = decode(1, 1);
    for (index, family, protocol, source, destination, transport) in [
        (3, 2, 17, "::", "::", vec![0, 68, 0, 67]),
        (4, 10, 17, "fe80::abcd", "ff02::1:2", vec![2, 34, 2, 35]),
        (5, 10, 58, "::", "ff02::2", vec![133, 0]),
        (6, 10, 58, "febf::abcd", "ff02::2", vec![133, 0]),
        (7, 10, 58, "2001:db8::1", "ff02::1:ffab:cdef", vec![135, 0]),
        (8, 10, 58, "fe80::1", "febf::2", vec![136, 0]),
    ] {
        let mut p = Packet {
            family,
            protocol,
            network: vec![0; 40],
            transport,
        };
        p.network[7] = 255;
        if family == 2 {
            p.network[16..20].fill(255);
        } else {
            p.network[8..24].copy_from_slice(&ip(source));
            p.network[24..40].copy_from_slice(&ip(destination));
        }
        assert!(allows(&rules[index], &p));
        for changed in [
            Packet {
                family: 99,
                ..p.clone()
            },
            Packet {
                protocol: 6,
                ..p.clone()
            },
            Packet {
                transport: vec![],
                ..p.clone()
            },
        ] {
            assert!(!allows(&rules[index], &changed));
        }
        for offset in 0..p.transport.len() {
            let mut changed = p.clone();
            changed.transport[offset] ^= 1;
            assert!(!allows(&rules[index], &changed));
        }
        let mut changed = p.clone();
        changed.network[if family == 2 { 16 } else { 24 }] ^= 1;
        assert!(!allows(&rules[index], &changed));
        if index >= 5 {
            let mut changed = p.clone();
            changed.network[7] = 254;
            assert!(!allows(&rules[index], &changed));
        }
        if [4, 6, 8].contains(&index) {
            let mut changed = p.clone();
            changed.network[8..24].copy_from_slice(&ip("fec0::1"));
            assert!(!allows(&rules[index], &changed));
        }
    }
}

#[test]
fn input_is_only_nonzero_bounded_fences() {
    for (g, s) in [(0, 1), (1, 0), (1, u32::MAX - 13), (u32::MAX, u32::MAX)] {
        assert_eq!(encode(g, s), Err(super::InvalidFence));
    }
}
