// SPDX-License-Identifier: MIT
//! Fixed completed operations, never descriptors or serialized authority.

use super::Unavailable;

pub(super) const FRAME_BYTES: usize = 64;
const MAGIC: &[u8; 8] = b"OVT4SV01";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Challenge,
    Ready,
    ObserveManager,
    Completed,
    Halt,
    Closed,
    Rejected,
}

impl Kind {
    fn byte(self) -> u8 {
        match self {
            Self::Challenge => 1,
            Self::Ready => 2,
            Self::ObserveManager => 3,
            Self::Completed => 4,
            Self::Halt => 5,
            Self::Closed => 6,
            Self::Rejected => 7,
        }
    }
    fn from_byte(value: u8) -> Result<Self, Unavailable> {
        match value {
            1 => Ok(Self::Challenge),
            2 => Ok(Self::Ready),
            3 => Ok(Self::ObserveManager),
            4 => Ok(Self::Completed),
            5 => Ok(Self::Halt),
            6 => Ok(Self::Closed),
            7 => Ok(Self::Rejected),
            _ => Err(Unavailable),
        }
    }
}

pub(super) struct Frame {
    pub kind: Kind,
    pub sequence: u32,
    pub nonce: [u8; 32],
}

impl Frame {
    pub fn encode(&self) -> Result<[u8; FRAME_BYTES], Unavailable> {
        if self.nonce == [0; 32] {
            return Err(Unavailable);
        }
        let mut raw = [0; FRAME_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8] = self.kind.byte();
        raw[9..13].copy_from_slice(&self.sequence.to_be_bytes());
        raw[13..45].copy_from_slice(&self.nonce);
        Ok(raw)
    }
    pub fn decode(raw: &[u8]) -> Result<Self, Unavailable> {
        if raw.len() != FRAME_BYTES || &raw[..8] != MAGIC || raw[45..].iter().any(|v| *v != 0) {
            return Err(Unavailable);
        }
        let nonce = raw[13..45].try_into().map_err(|_| Unavailable)?;
        if nonce == [0; 32] {
            return Err(Unavailable);
        }
        Ok(Self {
            kind: Kind::from_byte(raw[8])?,
            sequence: u32::from_be_bytes(raw[9..13].try_into().map_err(|_| Unavailable)?),
            nonce,
        })
    }
}

/// All states/capabilities are internal to one owner. No reset or adoption API.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Phase {
    AwaitReady,
    Live,
    Pending,
    Revoked,
    Closed,
}
pub(super) struct Context {
    pub nonce: [u8; 32],
    pub sequence: u32,
    pub phase: Phase,
}
impl Context {
    pub fn new(nonce: [u8; 32]) -> Result<Self, Unavailable> {
        if nonce == [0; 32] {
            return Err(Unavailable);
        }
        Ok(Self {
            nonce,
            sequence: 0,
            phase: Phase::AwaitReady,
        })
    }
    pub fn ready(&mut self, frame: Frame) -> Result<(), Unavailable> {
        self.check(frame, Kind::Ready, Phase::AwaitReady)?;
        self.phase = Phase::Live;
        Ok(())
    }
    pub fn begin(&mut self, kind: Kind) -> Result<Frame, Unavailable> {
        if self.phase != Phase::Live || !matches!(kind, Kind::ObserveManager | Kind::Halt) {
            self.phase = Phase::Revoked;
            return Err(Unavailable);
        }
        let Some(next) = self.sequence.checked_add(1) else {
            self.phase = Phase::Revoked;
            return Err(Unavailable);
        };
        self.sequence = next;
        self.phase = Phase::Pending;
        Ok(Frame {
            kind,
            sequence: next,
            nonce: self.nonce,
        })
    }
    pub fn completed(&mut self, frame: Frame, kind: Kind) -> Result<(), Unavailable> {
        if !matches!(kind, Kind::Completed | Kind::Closed) {
            self.revoke();
            return Err(Unavailable);
        }
        self.check(frame, kind, Phase::Pending)?;
        self.phase = if kind == Kind::Closed {
            Phase::Closed
        } else {
            Phase::Live
        };
        Ok(())
    }
    pub fn check(&mut self, frame: Frame, kind: Kind, phase: Phase) -> Result<(), Unavailable> {
        if self.phase != phase
            || frame.kind != kind
            || frame.sequence != self.sequence
            || frame.nonce != self.nonce
        {
            self.revoke();
            return Err(Unavailable);
        }
        Ok(())
    }
    pub fn revoke(&mut self) {
        self.phase = Phase::Revoked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(kind: Kind, seq: u32) -> Frame {
        Frame {
            kind,
            sequence: seq,
            nonce: [1; 32],
        }
    }
    #[test]
    fn exact_wire_has_no_extension_or_descriptor_surface() {
        for kind in [
            Kind::Challenge,
            Kind::Ready,
            Kind::ObserveManager,
            Kind::Completed,
            Kind::Halt,
            Kind::Closed,
            Kind::Rejected,
        ] {
            let raw = frame(kind, 2).encode().unwrap();
            let decoded = Frame::decode(&raw).unwrap();
            assert_eq!(decoded.kind, kind);
            assert_eq!(decoded.sequence, 2);
            assert_eq!(decoded.nonce, [1; 32]);
            for len in 0..FRAME_BYTES {
                assert!(Frame::decode(&raw[..len]).is_err());
            }
            let mut extra = raw.to_vec();
            extra.push(0);
            assert!(Frame::decode(&extra).is_err());
        }
    }
    #[test]
    fn unknown_kind_reserved_bytes_magic_and_zero_nonce_refuse() {
        let raw = frame(Kind::Ready, 0).encode().unwrap();
        for index in [0, 8, 45, 63] {
            let mut changed = raw;
            changed[index] = 255;
            assert!(Frame::decode(&changed).is_err());
        }
        let mut changed = raw;
        changed[13..45].fill(0);
        assert!(Frame::decode(&changed).is_err());
        assert!(Context::new([0; 32]).is_err());
    }
    #[test]
    fn only_original_ready_then_complete_can_enable_next_operation() {
        let mut context = Context::new([1; 32]).unwrap();
        context.ready(frame(Kind::Ready, 0)).unwrap();
        assert_eq!(context.begin(Kind::ObserveManager).unwrap().sequence, 1);
        context
            .completed(frame(Kind::Completed, 1), Kind::Completed)
            .unwrap();
        assert_eq!(context.begin(Kind::Halt).unwrap().sequence, 2);
        context
            .completed(frame(Kind::Closed, 2), Kind::Closed)
            .unwrap();
        assert!(context.begin(Kind::ObserveManager).is_err());
    }
    #[test]
    fn missing_ready_late_reply_wrong_context_and_sequence_permanently_revoke() {
        let mut fresh = Context::new([1; 32]).unwrap();
        assert!(fresh.begin(Kind::ObserveManager).is_err());
        assert!(fresh.ready(frame(Kind::Ready, 0)).is_err());
        for cut in 0..4 {
            let mut context = Context::new([1; 32]).unwrap();
            context.ready(frame(Kind::Ready, 0)).unwrap();
            context.begin(Kind::ObserveManager).unwrap();
            let mut reply = frame(Kind::Completed, 1);
            match cut {
                0 => context.revoke(),
                1 => reply.sequence = 0,
                2 => reply.nonce = [2; 32],
                _ => reply.kind = Kind::Rejected,
            }
            assert!(context.completed(reply, Kind::Completed).is_err());
            assert_eq!(context.phase, Phase::Revoked);
            assert!(context.begin(Kind::ObserveManager).is_err());
            assert!(
                context
                    .completed(frame(Kind::Completed, 1), Kind::Completed)
                    .is_err()
            );
        }
    }
    #[test]
    fn concurrent_or_exhausted_request_never_creates_second_capability() {
        let mut context = Context::new([1; 32]).unwrap();
        context.ready(frame(Kind::Ready, 0)).unwrap();
        context.begin(Kind::ObserveManager).unwrap();
        assert!(context.begin(Kind::ObserveManager).is_err());
        let mut exhausted = Context::new([1; 32]).unwrap();
        exhausted.ready(frame(Kind::Ready, 0)).unwrap();
        exhausted.sequence = u32::MAX;
        assert!(exhausted.begin(Kind::ObserveManager).is_err());
        assert_eq!(exhausted.phase, Phase::Revoked);
    }
}
