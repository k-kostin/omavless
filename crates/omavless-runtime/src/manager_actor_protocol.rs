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
    AuthenticateBackup,
    BackupAuthenticated,
    StageAuthenticatedBackup,
    StageRecorded,
    ObserveStopped,
    StoppedObserved,
    CommitAuthenticatedBackup,
    PairCommitted,
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
            Self::AuthenticateBackup => 8,
            Self::BackupAuthenticated => 9,
            Self::StageAuthenticatedBackup => 10,
            Self::StageRecorded => 11,
            Self::ObserveStopped => 12,
            Self::StoppedObserved => 13,
            Self::CommitAuthenticatedBackup => 14,
            Self::PairCommitted => 15,
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
            8 => Ok(Self::AuthenticateBackup),
            9 => Ok(Self::BackupAuthenticated),
            10 => Ok(Self::StageAuthenticatedBackup),
            11 => Ok(Self::StageRecorded),
            12 => Ok(Self::ObserveStopped),
            13 => Ok(Self::StoppedObserved),
            14 => Ok(Self::CommitAuthenticatedBackup),
            15 => Ok(Self::PairCommitted),
            _ => Err(Unavailable),
        }
    }
    pub fn completion(self) -> Result<Self, Unavailable> {
        match self {
            Self::ObserveManager => Ok(Self::Completed),
            Self::Halt => Ok(Self::Closed),
            Self::AuthenticateBackup => Ok(Self::BackupAuthenticated),
            Self::StageAuthenticatedBackup => Ok(Self::StageRecorded),
            Self::ObserveStopped => Ok(Self::StoppedObserved),
            Self::CommitAuthenticatedBackup => Ok(Self::PairCommitted),
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
    pending: Option<Kind>,
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
            pending: None,
        })
    }
    pub fn ready(&mut self, frame: Frame) -> Result<(), Unavailable> {
        self.check(frame, Kind::Ready, Phase::AwaitReady)?;
        self.phase = Phase::Live;
        Ok(())
    }
    pub fn begin(&mut self, kind: Kind) -> Result<Frame, Unavailable> {
        if self.phase != Phase::Live || self.pending.is_some() || kind.completion().is_err() {
            self.revoke();
            return Err(Unavailable);
        }
        let Some(next) = self.sequence.checked_add(1) else {
            self.revoke();
            return Err(Unavailable);
        };
        self.sequence = next;
        self.phase = Phase::Pending;
        self.pending = Some(kind);
        Ok(Frame {
            kind,
            sequence: next,
            nonce: self.nonce,
        })
    }
    pub fn completed(&mut self, frame: Frame, kind: Kind) -> Result<(), Unavailable> {
        if !matches!(
            (self.pending, kind),
            (Some(Kind::ObserveManager), Kind::Completed)
                | (Some(Kind::Halt), Kind::Closed)
                | (Some(Kind::AuthenticateBackup), Kind::BackupAuthenticated)
                | (Some(Kind::StageAuthenticatedBackup), Kind::StageRecorded)
                | (Some(Kind::ObserveStopped), Kind::StoppedObserved)
                | (Some(Kind::CommitAuthenticatedBackup), Kind::PairCommitted)
        ) {
            self.revoke();
            return Err(Unavailable);
        }
        self.check(frame, kind, Phase::Pending)?;
        self.pending = None;
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
        self.pending = None;
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
            Kind::AuthenticateBackup,
            Kind::BackupAuthenticated,
            Kind::StageAuthenticatedBackup,
            Kind::StageRecorded,
            Kind::ObserveStopped,
            Kind::StoppedObserved,
            Kind::CommitAuthenticatedBackup,
            Kind::PairCommitted,
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
    fn canonical_completion_is_bound_to_original_pending_kind() {
        for reply in [Kind::Completed, Kind::StageRecorded, Kind::StoppedObserved] {
            let mut context = Context::new([1; 32]).unwrap();
            context.ready(frame(Kind::Ready, 0)).unwrap();
            context.begin(Kind::ObserveStopped).unwrap();
            let result = context.completed(frame(reply, 1), reply);
            assert_eq!(result.is_ok(), reply == Kind::StoppedObserved);
            if result.is_err() {
                assert!(context.begin(Kind::Halt).is_err());
            }
        }
    }

    #[test]
    fn commit_completion_never_aliases_stage_or_generic_completion() {
        for reply in [
            Kind::Completed,
            Kind::StageRecorded,
            Kind::PairCommitted,
            Kind::Closed,
        ] {
            let mut context = Context::new([1; 32]).unwrap();
            context.ready(frame(Kind::Ready, 0)).unwrap();
            context.begin(Kind::CommitAuthenticatedBackup).unwrap();
            let result = context.completed(frame(reply, 1), reply);
            assert_eq!(result.is_ok(), reply == Kind::PairCommitted);
            if result.is_err() {
                assert!(context.begin(Kind::Halt).is_err());
            }
        }
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

    #[test]
    fn completed_kind_is_bound_to_original_pending_operation() {
        for (request, wrong) in [
            (Kind::ObserveManager, Kind::Closed),
            (Kind::Halt, Kind::Completed),
            (Kind::AuthenticateBackup, Kind::Completed),
            (Kind::AuthenticateBackup, Kind::Closed),
            (Kind::ObserveManager, Kind::BackupAuthenticated),
            (Kind::Halt, Kind::BackupAuthenticated),
        ] {
            let mut context = Context::new([1; 32]).unwrap();
            context.ready(frame(Kind::Ready, 0)).unwrap();
            context.begin(request).unwrap();
            assert!(context.completed(frame(wrong, 1), wrong).is_err());
            assert_eq!(context.phase, Phase::Revoked);
            assert!(context.begin(request).is_err());
        }
    }

    #[test]
    fn authenticated_backup_reply_is_completed_observation_not_restore_authority() {
        let mut context = Context::new([1; 32]).unwrap();
        context.ready(frame(Kind::Ready, 0)).unwrap();
        context.begin(Kind::AuthenticateBackup).unwrap();
        context
            .completed(
                frame(Kind::BackupAuthenticated, 1),
                Kind::BackupAuthenticated,
            )
            .unwrap();
        assert_eq!(context.phase, Phase::Live);
        // Duplicated/lost old reply cannot issue another pending capability.
        assert!(
            context
                .completed(
                    frame(Kind::BackupAuthenticated, 1),
                    Kind::BackupAuthenticated
                )
                .is_err()
        );
        assert_eq!(context.phase, Phase::Revoked);
        assert!(context.begin(Kind::Halt).is_err());
    }
}
