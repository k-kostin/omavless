// SPDX-License-Identifier: MIT
//! Private cfg(test) observer loss of one real BEGIN/operation success ACK.
//! END and every non-target frame retain their received bytes and metadata.
use super::*;
use std::borrow::Cow;

#[derive(Default)]
pub(super) struct OneShotPrefixAckLoss {
    socket: Option<i32>,
    target: usize,
    armed: bool,
    consumed: usize,
    remaining_delivered: bool,
}
impl OneShotPrefixAckLoss {
    pub(super) fn arm(&mut self, socket: i32, target: usize) {
        assert!(socket >= 0 && self.socket.is_none() && !self.armed && self.consumed == 0);
        self.socket = Some(socket);
        self.target = target;
        self.armed = true;
    }
    pub(super) fn target(&self) -> Option<usize> {
        self.socket.map(|_| self.target)
    }
    pub(super) fn deliver<'a>(
        &mut self,
        socket: i32,
        bytes: &'a [u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
        request: &[u8],
        port: u32,
    ) -> Result<Cow<'a, [u8]>> {
        require(self.socket.is_none_or(|retained| retained == socket))?;
        if !self.armed && self.consumed == 0 {
            return Ok(Cow::Borrowed(bytes));
        }
        require(
            sender == Some(NetlinkAddr::new(0, 0))
                && flags.is_empty()
                && !bytes.is_empty()
                && bytes.len() <= LIMIT
                && request.len() >= 20
                && matches!(u16n(&request[4..6])?, 16 | 0xa00 | 0xa02 | 0xa03 | 0xa06)
                && port != 0,
        )?;
        let sequence = u32n(&request[8..12])?;
        let mut rest = bytes;
        let mut delivered = Vec::new();
        let mut lost = false;
        while !rest.is_empty() {
            require(rest.len() >= 16)?;
            let length = usize::try_from(u32n(&rest[..4])?).map_err(|_| REFUSE)?;
            require(length >= 16 && aligned(length) <= rest.len())?;
            let frame = &rest[..aligned(length)];
            if u32n(&frame[8..12])? == sequence {
                // Validate the actual received ACK against the actual outgoing
                // request. No effect-path ACK is constructed or altered.
                require(
                    self.armed
                        && !lost
                        && length == 36
                        && u16n(&frame[4..6])? == 2
                        && matches!(u16n(&frame[6..8])?, 0 | 0x100)
                        && u32n(&frame[12..16])? == port
                        && frame[16..20] == [0; 4]
                        && frame[20..36] == request[..16],
                )?;
                lost = true;
            } else {
                delivered.extend_from_slice(frame);
            }
            rest = &rest[aligned(length)..];
        }
        if lost {
            self.armed = false;
            self.consumed += 1;
            Ok(Cow::Owned(delivered))
        } else {
            Ok(Cow::Borrowed(bytes))
        }
    }
    /// The caller must supply the existing collector's accepted ACK bitmap.
    /// Loss alone cannot short-circuit before actual END/other ACK delivery.
    pub(super) fn ready(&self, accepted: &[bool]) -> Result<bool> {
        let Some(target) = self.target() else {
            return Ok(false);
        };
        require(target < accepted.len().checked_sub(1).ok_or(REFUSE)?)?;
        Ok(self.consumed == 1
            && !self.remaining_delivered
            && !accepted[target]
            && accepted
                .iter()
                .enumerate()
                .all(|(i, ack)| i == target || *ack))
    }
    pub(super) fn confirm_remaining(&mut self) {
        assert_eq!(self.consumed, 1);
        assert!(!self.armed && !self.remaining_delivered);
        self.remaining_delivered = true;
    }
    pub(super) fn observed(&self) -> (usize, bool) {
        (self.consumed, self.remaining_delivered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Constructed ACKs are pure decoder inputs, never actual-kernel evidence.
    fn ack(request: &[u8]) -> Vec<u8> {
        message(
            2,
            0x100,
            u32n(&request[8..12]).unwrap(),
            42,
            &[vec![0; 4], request[..16].to_vec()].concat(),
        )
    }
    fn fault() -> OneShotPrefixAckLoss {
        let mut fault = OneShotPrefixAckLoss::default();
        fault.arm(7, 0);
        fault
    }
    fn deliver<'a>(
        f: &mut OneShotPrefixAckLoss,
        bytes: &'a [u8],
        r: &[u8],
    ) -> Result<Cow<'a, [u8]>> {
        f.deliver(
            7,
            bytes,
            Some(NetlinkAddr::new(0, 0)),
            MsgFlags::empty(),
            r,
            42,
        )
    }
    #[test]
    fn exact_target_only_is_withheld_with_coalesced_adjacent_end_unchanged() {
        for kind in [16, NFT, NFT + 2, NFT + 3, NFT + 6] {
            let target = message(kind, 5, 10, 0, &[0, 0, 0, 10]);
            let end = ack(&message(17, 5, 11, 0, &[0, 0, 0, 10]));
            let target_ack = ack(&target);
            for bytes in [
                &[target_ack.clone(), end.clone()].concat(),
                &[end.clone(), target_ack.clone()].concat(),
            ] {
                let mut f = fault();
                assert_eq!(&*deliver(&mut f, bytes, &target).unwrap(), end);
                assert_eq!(f.observed(), (1, false));
                assert!(!f.ready(&[false, false]).unwrap());
                assert!(f.ready(&[false, true]).unwrap());
                f.confirm_remaining();
                assert_eq!(f.observed(), (1, true));
                // A duplicate target cannot redeem the lost observation,
                // even before all remaining ACKs have reached the collector.
                assert!(deliver(&mut f, &target_ack, &target).is_err());
                assert_eq!(f.observed(), (1, true));
            }
            let mut f = fault();
            assert_eq!(&*deliver(&mut f, &end, &target).unwrap(), end);
            assert_eq!(f.observed(), (0, false));
            assert!(!f.ready(&[false, true]).unwrap());
            assert!(deliver(&mut f, &target_ack, &target).unwrap().is_empty());
            assert!(f.ready(&[false, true]).unwrap());
        }
    }
    #[test]
    fn malformed_metadata_target_or_duplicate_never_counts_as_observed_loss() {
        let target = message(16, 5, 10, 0, &[0, 0, 0, 10]);
        let a = ack(&target);
        for length in 0..a.len() {
            let mut f = fault();
            assert!(deliver(&mut f, &a[..length], &target).is_err());
            assert_eq!(f.observed(), (0, false));
        }
        for offset in [0, 4, 6, 12, 16, 20, 24, 28, 32] {
            let mut bad = a.clone();
            bad[offset] ^= 1;
            let mut f = fault();
            assert!(deliver(&mut f, &bad, &target).is_err());
            assert_eq!(f.observed(), (0, false));
        }
        for (socket, sender, flags) in [
            (8, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty()),
            (7, Some(NetlinkAddr::new(1, 0)), MsgFlags::empty()),
            (7, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_TRUNC),
            (7, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_CTRUNC),
        ] {
            let mut f = fault();
            assert!(f.deliver(socket, &a, sender, flags, &target, 42).is_err());
            assert_eq!(f.observed(), (0, false));
        }
        let mut f = fault();
        assert!(deliver(&mut f, &[a.clone(), a].concat(), &target).is_err());
        assert_eq!(f.observed(), (0, false));
        assert!(
            deliver(
                &mut fault(),
                &ack(&message(17, 5, 10, 0, &[0; 4])),
                &message(17, 5, 10, 0, &[0; 4])
            )
            .is_err()
        );
        assert!(!fault().ready(&[true, true]).unwrap());
        assert!(fault().ready(&[]).is_err());
    }
}
