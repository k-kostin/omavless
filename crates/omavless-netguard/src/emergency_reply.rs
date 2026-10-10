// SPDX-License-Identifier: MIT
//! Pure, bounded decoder for the fixed Emergency-create reply transcript.
//!
//! The caller supplies receive metadata; this module opens no socket and cannot
//! authenticate that metadata, the namespace, the kernel effect or table owner.
//! A complete transcript is not permission to mutate or a durable receipt.

use super::{EmergencyCreate, encode};

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
pub struct EmergencyTranscript {
    expected: [[u8; 16]; 5],
    operation_acks: [bool; 4],
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

fn expected_headers(wire: &EmergencyCreate) -> Result<[[u8; 16]; 5], InvalidTranscript> {
    let mut rest = wire.batch();
    let mut expected = [[0; 16]; 5];
    for index in 0..6 {
        require(rest.len() >= 16)?;
        let length = usize::try_from(u32n(&rest[..4])?).map_err(|_| InvalidTranscript)?;
        require(length >= 16 && length <= rest.len() && aligned(length)? <= rest.len())?;
        if (1..=4).contains(&index) {
            expected[index - 1].copy_from_slice(&rest[..16]);
        }
        rest = &rest[aligned(length)?..];
    }
    require(rest.is_empty() && wire.barrier().len() >= 16)?;
    expected[4].copy_from_slice(&wire.barrier()[..16]);
    Ok(expected)
}

impl EmergencyTranscript {
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
            operation_acks: [false; 4],
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
                require(sequence == self.first_sequence + 6 && self.generation.is_none())?;
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
            1..=4 => usize::try_from(offset - 1).map_err(|_| InvalidTranscript)?,
            6 => 4,
            _ => return Err(InvalidTranscript),
        };
        require(body[4..20] == self.expected[index])?;
        if index == 4 {
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
mod tests {
    use super::*;

    const PORT: u32 = 42001;
    const SEQUENCE: u32 = 100;

    fn collector() -> EmergencyTranscript {
        EmergencyTranscript::new(7, SEQUENCE, PORT).unwrap()
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
    fn ack(c: &EmergencyTranscript, index: usize) -> Vec<u8> {
        let mut body = vec![0; 4];
        body.extend(c.expected[index]);
        message(
            ERROR,
            0,
            SEQUENCE + if index == 4 { 6 } else { index as u32 + 1 },
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
        message(NEWGEN, 0, SEQUENCE + 6, &body)
    }
    fn push(c: &mut EmergencyTranscript, bytes: &[u8]) -> Result<(), InvalidTranscript> {
        c.push_datagram(bytes, 0, 0, 0)
    }
    fn full() -> (EmergencyTranscript, Vec<Vec<u8>>) {
        let c = collector();
        let parts = vec![
            ack(&c, 0),
            ack(&c, 1),
            ack(&c, 2),
            ack(&c, 3),
            ack(&c, 4),
            generation_reply(),
        ];
        (c, parts)
    }

    #[test]
    fn accepts_exact_complete_untrusted_transcript_in_any_reply_order() {
        let (mut c, parts) = full();
        push(&mut c, &[parts[5].as_slice(), parts[4].as_slice()].concat()).unwrap();
        push(&mut c, &parts[3]).unwrap();
        push(
            &mut c,
            &[
                parts[0].as_slice(),
                parts[1].as_slice(),
                parts[2].as_slice(),
            ]
            .concat(),
        )
        .unwrap();
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
        for missing in 0..6 {
            let (mut c, parts) = full();
            for (index, part) in parts.iter().enumerate() {
                if index != missing {
                    push(&mut c, part).unwrap();
                }
            }
            assert_eq!(c.finish(), Err(InvalidTranscript));
            assert_eq!(push(&mut c, &parts[missing]), Err(InvalidTranscript));
        }
        for duplicate in 0..6 {
            let (mut c, parts) = full();
            push(&mut c, &parts[duplicate]).unwrap();
            assert_eq!(push(&mut c, &parts[duplicate]), Err(InvalidTranscript));
            assert_eq!(push(&mut c, &parts[0]), Err(InvalidTranscript));
        }
        let (mut c, parts) = full();
        let mut extra = parts.concat();
        extra.extend(message(3, 0, SEQUENCE + 6, &[]));
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
        for sequence in [SEQUENCE, SEQUENCE + 5, SEQUENCE + 7] {
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
            let mut bad = message(NEWGEN, 0, SEQUENCE + 6, &body);
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
            (1, u32::MAX - 5, PORT),
            (1, 1, 0),
        ] {
            assert!(EmergencyTranscript::new(generation, sequence, port).is_err());
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
}
