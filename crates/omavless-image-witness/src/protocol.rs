// SPDX-License-Identifier: MIT
use crate::{Error, Result};

pub(crate) const BYTES: usize = 16;
pub(crate) const MAX_OBSERVATIONS: u32 = 2048;
const MAGIC: [u8; 4] = *b"OVIM";
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Kind {
    Bind = 1,
    Observe = 2,
    Finish = 3,
    Image = 4,
    Ack = 5,
}
pub(crate) fn frame(kind: Kind, sequence: u32) -> [u8; BYTES] {
    let mut bytes = [0; BYTES];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4] = 1;
    bytes[5] = kind as u8;
    bytes[8..12].copy_from_slice(&sequence.to_le_bytes());
    bytes
}
pub(crate) fn decode(bytes: [u8; BYTES]) -> Result<(Kind, u32)> {
    if bytes[..4] != MAGIC || bytes[4] != 1 || bytes[6..8] != [0; 2] || bytes[12..] != [0; 4] {
        return Err(Error::Refused);
    }
    let kind = match bytes[5] {
        1 => Kind::Bind,
        2 => Kind::Observe,
        3 => Kind::Finish,
        4 => Kind::Image,
        5 => Kind::Ack,
        _ => return Err(Error::Refused),
    };
    Ok((
        kind,
        u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| Error::Refused)?),
    ))
}

/// Consumed before any acquisition/reply; a failure never permits a retry.
pub(crate) struct Sequence {
    bound: bool,
    next: u32,
    terminal: bool,
}
impl Sequence {
    pub(crate) fn new() -> Self {
        Self {
            bound: false,
            next: 0,
            terminal: false,
        }
    }
    pub(crate) fn consume(&mut self, kind: Kind, sequence: u32, has_fd: bool) -> Result<()> {
        let valid = !self.terminal
            && sequence == self.next
            && match kind {
                Kind::Bind => !self.bound && sequence == 0 && has_fd,
                Kind::Observe => self.bound && !has_fd && sequence <= MAX_OBSERVATIONS,
                Kind::Finish => self.bound && !has_fd,
                Kind::Image | Kind::Ack => false,
            };
        self.terminal = true;
        if !valid {
            return Err(Error::Refused);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Refused)?;
        self.bound = true;
        self.terminal = kind == Kind::Finish;
        Ok(())
    }
    pub(crate) fn poison(&mut self) {
        self.terminal = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_frames_reject_extensions_and_foreign_commands() {
        for kind in [
            Kind::Bind,
            Kind::Observe,
            Kind::Finish,
            Kind::Image,
            Kind::Ack,
        ] {
            let bytes = frame(kind, 12);
            let (decoded, seq) = decode(bytes).unwrap();
            assert!(decoded == kind);
            assert_eq!(seq, 12);
            for index in [0, 4, 6, 7, 12, 15] {
                let mut bad = bytes;
                bad[index] ^= 1;
                assert!(decode(bad).is_err());
            }
        }
        let mut bad = frame(Kind::Bind, 0);
        bad[5] = 0;
        assert!(decode(bad).is_err());
    }
    #[test]
    fn one_bind_ordered_observation_and_terminal_errors() {
        let mut s = Sequence::new();
        s.consume(Kind::Bind, 0, true).unwrap();
        s.consume(Kind::Observe, 1, false).unwrap();
        assert!(s.consume(Kind::Observe, 1, false).is_err());
        assert!(s.consume(Kind::Observe, 2, false).is_err());
        let mut s = Sequence::new();
        assert!(s.consume(Kind::Observe, 0, false).is_err());
        assert!(s.consume(Kind::Bind, 0, true).is_err());
        let mut s = Sequence::new();
        s.consume(Kind::Bind, 0, true).unwrap();
        s.consume(Kind::Finish, 1, false).unwrap();
        assert!(s.consume(Kind::Observe, 2, false).is_err());
    }
    #[test]
    fn admission_limit_and_explicit_poison_are_sticky() {
        let mut s = Sequence::new();
        s.consume(Kind::Bind, 0, true).unwrap();
        for n in 1..=MAX_OBSERVATIONS {
            s.consume(Kind::Observe, n, false).unwrap();
        }
        assert!(
            s.consume(Kind::Observe, MAX_OBSERVATIONS + 1, false)
                .is_err()
        );
        assert!(
            s.consume(Kind::Finish, MAX_OBSERVATIONS + 1, false)
                .is_err()
        );
        let mut s = Sequence::new();
        s.poison();
        assert!(s.consume(Kind::Bind, 0, true).is_err());
    }
}
