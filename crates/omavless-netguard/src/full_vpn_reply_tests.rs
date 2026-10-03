// SPDX-License-Identifier: MIT
use super::*;

const PORT: u32 = 42001;
const SEQUENCE: u32 = 100;

fn collector() -> FullVpnTranscript {
    FullVpnTranscript::new(7, SEQUENCE, PORT).unwrap()
}
fn message(kind: u16, flags: u16, sequence: u32, body: &[u8]) -> Vec<u8> {
    let length = 16 + body.len();
    let mut bytes = Vec::from((length as u32).to_ne_bytes());
    bytes.extend(kind.to_ne_bytes());
    bytes.extend(flags.to_ne_bytes());
    bytes.extend(sequence.to_ne_bytes());
    bytes.extend(PORT.to_ne_bytes());
    bytes.extend(body);
    bytes.resize(length.next_multiple_of(4), 0);
    bytes
}
fn ack(c: &FullVpnTranscript, index: usize) -> Vec<u8> {
    let mut body = vec![0; 4];
    body.extend(c.expected[index]);
    message(
        ERROR,
        0,
        SEQUENCE + if index == 12 { 14 } else { index as u32 + 1 },
        &body,
    )
}
fn attribute(kind: u16, value: &[u8]) -> Vec<u8> {
    let length = 4 + value.len();
    let mut bytes = Vec::from((length as u16).to_ne_bytes());
    bytes.extend(kind.to_ne_bytes());
    bytes.extend(value);
    bytes.resize(length.next_multiple_of(4), 0);
    bytes
}
fn generation_reply() -> Vec<u8> {
    let body = [vec![0, 0, 0, 9], attribute(1, &9_u32.to_be_bytes())].concat();
    message(NEWGEN, 0, SEQUENCE + 14, &body)
}
fn push(c: &mut FullVpnTranscript, bytes: &[u8]) -> Result<(), InvalidTranscript> {
    c.push_datagram(bytes, 0, 0, 0)
}
fn full() -> (FullVpnTranscript, Vec<Vec<u8>>) {
    let c = collector();
    let mut parts: Vec<_> = (0..13).map(|index| ack(&c, index)).collect();
    parts.push(generation_reply());
    (c, parts)
}

#[test]
fn accepts_exact_complete_untrusted_transcript_in_any_reply_order() {
    let (mut c, parts) = full();
    for part in parts.iter().rev() {
        push(&mut c, part).unwrap();
    }
    assert_eq!(
        c.finish(),
        Ok(CompleteUntrustedTranscript {
            observed_generation: 9
        })
    );
    assert_eq!(push(&mut c, &parts[0]), Err(InvalidTranscript));
    assert_eq!(c.finish(), Err(InvalidTranscript));
}

#[test]
fn missing_duplicate_and_extra_replies_poison() {
    for missing in 0..14 {
        let (mut c, parts) = full();
        for (index, part) in parts.iter().enumerate() {
            if index != missing {
                push(&mut c, part).unwrap();
            }
        }
        assert_eq!(c.finish(), Err(InvalidTranscript));
        assert_eq!(push(&mut c, &parts[missing]), Err(InvalidTranscript));
    }
    for duplicate in 0..14 {
        let (mut c, parts) = full();
        push(&mut c, &parts[duplicate]).unwrap();
        assert_eq!(push(&mut c, &parts[duplicate]), Err(InvalidTranscript));
        assert_eq!(push(&mut c, &parts[0]), Err(InvalidTranscript));
    }
    let (mut c, parts) = full();
    let mut extra = parts.concat();
    extra.extend(message(3, 0, SEQUENCE + 14, &[]));
    assert_eq!(push(&mut c, &extra), Err(InvalidTranscript));
    assert_eq!(c.finish(), Err(InvalidTranscript));
}

#[test]
fn metadata_wrong_sequence_port_and_echo_refuse() {
    let (mut c, parts) = full();
    assert_eq!(c.push_datagram(&parts[0], 1, 0, 0), Err(InvalidTranscript));
    assert_eq!(push(&mut c, &parts[0]), Err(InvalidTranscript));
    for metadata in [(0, 1, 0), (0, 0, 0x20)] {
        let (mut c, parts) = full();
        assert_eq!(
            c.push_datagram(&parts[0], metadata.0, metadata.1, metadata.2),
            Err(InvalidTranscript)
        );
    }
    for offset in [8, 12, 20] {
        let (mut c, parts) = full();
        let mut bad = parts[0].clone();
        bad[offset] ^= 1;
        assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
    }
    for sequence in [SEQUENCE, SEQUENCE + 13, SEQUENCE + 15] {
        let (mut c, _) = full();
        let bad = message(ERROR, 0, sequence, &[0; 20]);
        assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
    }
}

#[test]
fn error_flags_and_malformed_message_boundaries_refuse() {
    let (c, parts) = full();
    let good = parts[0].clone();
    let mut cases = vec![Vec::new(), good[..15].to_vec()];
    for offset in [0, 4, 6, 16, 17, 19] {
        let mut bad = good.clone();
        bad[offset] ^= 1;
        cases.push(bad);
    }
    let mut bad = good.clone();
    bad[16..20].copy_from_slice(&(-17_i32).to_ne_bytes());
    cases.push(bad);
    let mut bad = good.clone();
    bad[6..8].copy_from_slice(&1_u16.to_ne_bytes());
    cases.push(bad);
    let mut bad = good.clone();
    bad[0..4].copy_from_slice(&u32::MAX.to_ne_bytes());
    cases.push(bad);
    for bad in cases {
        let mut copy = collector();
        assert_eq!(push(&mut copy, &bad), Err(InvalidTranscript));
        assert_eq!(copy.finish(), Err(InvalidTranscript));
    }
    assert_eq!(c.expected[0][4..6], (10_u16 << 8).to_ne_bytes());
}

#[test]
fn capped_success_ack_is_bounded_but_extended_or_nonzero_error_refuses() {
    let (mut c, parts) = full();
    let mut capped = parts[0].clone();
    capped[6..8].copy_from_slice(&CAPPED.to_ne_bytes());
    push(&mut c, &capped).unwrap();
    for part in &parts[1..] {
        push(&mut c, part).unwrap();
    }
    assert!(c.finish().is_ok());

    for bad in [
        {
            let mut body = vec![0; 4];
            body.extend(collector().expected[0]);
            body.push(1);
            message(ERROR, 0, SEQUENCE + 1, &body)
        },
        {
            let mut body = (-17_i32).to_ne_bytes().to_vec();
            body.extend(collector().expected[0]);
            message(ERROR, 0, SEQUENCE + 1, &body)
        },
    ] {
        let mut c = collector();
        assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
        assert_eq!(c.finish(), Err(InvalidTranscript));
    }
}

#[test]
fn reply_header_and_attribute_padding_refuse() {
    let base = generation_reply();
    for offset in [4, 6, 8, 12, 16, 17, 18, 19] {
        let mut c = collector();
        let mut bad = base.clone();
        bad[offset] ^= 1;
        assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
    }
    for body in [
        [vec![0, 0, 0, 0], attribute(1, &0_u32.to_be_bytes())].concat(),
        [
            vec![0, 0, 0, 9],
            attribute(1, &9_u32.to_be_bytes()),
            attribute(3, &[b'a'; 17]),
        ]
        .concat(),
        [
            vec![0, 0, 0, 9],
            attribute(1, &9_u32.to_be_bytes()),
            attribute(3, b"x\0"),
        ]
        .concat(),
    ] {
        let mut c = collector();
        let mut bad = message(NEWGEN, 0, SEQUENCE + 14, &body);
        if body.last() == Some(&0) && body.len() > 12 {
            let last = bad.len() - 1;
            bad[last] = 1;
        }
        assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
    }
}

#[test]
fn generation_schema_is_closed_and_bounded() {
    let valid = [vec![0, 0, 0, 9], attribute(1, &9_u32.to_be_bytes())].concat();
    assert_eq!(parse_generation(&valid), Ok(9));
    let mut cases = vec![
        Vec::new(),
        vec![0, 0, 0, 9],
        [
            valid.as_slice(),
            attribute(1, &9_u32.to_be_bytes()).as_slice(),
        ]
        .concat(),
        [valid.as_slice(), attribute(4, &[]).as_slice()].concat(),
        [
            valid.as_slice(),
            attribute(0x8002, &1_u32.to_be_bytes()).as_slice(),
        ]
        .concat(),
        [valid.as_slice(), attribute(2, &[0]).as_slice()].concat(),
        [valid.as_slice(), attribute(3, b"bad\0name\0").as_slice()].concat(),
        [valid.as_slice(), attribute(3, b"no terminator").as_slice()].concat(),
    ];
    for index in [0, 1, 2, 3] {
        let mut bad = valid.clone();
        bad[index] ^= 1;
        cases.push(bad);
    }
    let mut bad = valid.clone();
    bad[4..6].copy_from_slice(&u16::MAX.to_ne_bytes());
    cases.push(bad);
    for bad in cases {
        assert_eq!(parse_generation(&bad), Err(InvalidTranscript));
    }
}

#[test]
fn total_and_datagram_limits_and_invalid_constructor_refuse() {
    for (generation, sequence, port) in [
        (0, 1, PORT),
        (1, 0, PORT),
        (1, u32::MAX - 13, PORT),
        (1, 1, 0),
    ] {
        assert!(FullVpnTranscript::new(generation, sequence, port).is_err());
    }
    let (mut c, parts) = full();
    let too_large = [parts[0].as_slice(), vec![0_u8; LIMIT].as_slice()].concat();
    assert_eq!(push(&mut c, &too_large), Err(InvalidTranscript));
    let (mut c, parts) = full();
    c.bytes = LIMIT;
    assert_eq!(push(&mut c, &parts[0]), Err(InvalidTranscript));
    let (mut c, parts) = full();
    c.datagrams = MAX_DATAGRAMS;
    assert_eq!(push(&mut c, &parts[0]), Err(InvalidTranscript));
    let (mut c, _) = full();
    assert_eq!(push(&mut c, &[0; 16]), Err(InvalidTranscript));
}

#[test]
fn each_full_request_header_and_success_echo_is_exact() {
    let c = collector();
    for (index, header) in c.expected.iter().enumerate() {
        let kind = match index {
            0 => 0xa00_u16,
            1 => 0xa03,
            12 => 0xa10,
            _ => 0xa06,
        };
        let flags = match index {
            0 | 1 => 0x605_u16,
            12 => 5,
            _ => 0xc05,
        };
        assert_eq!(&header[4..6], &kind.to_ne_bytes());
        assert_eq!(&header[6..8], &flags.to_ne_bytes());
        assert_eq!(
            &header[8..12],
            &(SEQUENCE + if index == 12 { 14 } else { index as u32 + 1 }).to_ne_bytes()
        );
        assert_eq!(&header[12..16], &[0; 4]);
        for offset in 20..36 {
            let mut c = collector();
            let mut bad = ack(&c, index);
            bad[offset] ^= 1;
            assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
            assert_eq!(c.finish(), Err(InvalidTranscript));
        }
    }
}

#[test]
fn complete_coalesced_transcript_and_maximum_sequence() {
    let (mut c, parts) = full();
    push(&mut c, &parts.concat()).unwrap();
    assert_eq!(c.finish().unwrap().observed_generation, 9);
    assert!(FullVpnTranscript::new(1, u32::MAX - 14, PORT).is_ok());
}

#[test]
fn every_truncated_message_and_foreign_operation_metadata_refuses() {
    let (_, parts) = full();
    for part in &parts {
        for end in 0..part.len() {
            let mut c = collector();
            assert_eq!(push(&mut c, &part[..end]), Err(InvalidTranscript));
            assert_eq!(c.finish(), Err(InvalidTranscript));
        }
        for offset in [8, 12] {
            let mut c = collector();
            let mut bad = part.clone();
            bad[offset..offset + 4].copy_from_slice(&0_u32.to_ne_bytes());
            assert_eq!(push(&mut c, &bad), Err(InvalidTranscript));
        }
        for metadata in [(1, 0, 0), (0, 1, 0), (0, 0, 8), (0, 0, 32)] {
            let mut c = collector();
            assert_eq!(
                c.push_datagram(part, metadata.0, metadata.1, metadata.2),
                Err(InvalidTranscript)
            );
            assert_eq!(c.finish(), Err(InvalidTranscript));
        }
    }
}

#[test]
fn optional_generation_metadata_is_structural_not_a_writer_identity() {
    let body = [
        vec![0, 0, 0, 9],
        attribute(1, &9_u32.to_be_bytes()),
        attribute(2, &u32::MAX.to_be_bytes()),
        attribute(3, b"untrusted\0"),
    ]
    .concat();
    assert_eq!(parse_generation(&body), Ok(9));
    for kind in [2, 3] {
        assert_eq!(
            parse_generation(&[body.clone(), attribute(kind, &[0; 4])].concat()),
            Err(InvalidTranscript)
        );
    }
}
