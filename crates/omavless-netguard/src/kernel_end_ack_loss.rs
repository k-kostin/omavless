// SPDX-License-Identifier: MIT
//! Private cfg(test) observer loss, not kernel ACK dropping. A successful real
//! recvmsg consumes exact END, but its bytes are withheld from the collector.
use super::*;
use std::borrow::Cow;

#[derive(Default)]
pub(super) struct OneShotEndAckLoss {
    socket: Option<i32>,
    armed: bool,
    consumed: usize,
    prefix_delivered: bool,
}
impl OneShotEndAckLoss {
    pub(super) fn arm(&mut self, socket: i32) {
        assert!(socket >= 0 && self.socket.is_none() && !self.armed && self.consumed == 0);
        self.socket = Some(socket);
        self.armed = true;
    }
    pub(super) fn deliver<'a>(
        &mut self,
        socket: i32,
        bytes: &'a [u8],
        sender: Option<NetlinkAddr>,
        flags: MsgFlags,
        end_request: &[u8],
        port: u32,
    ) -> Result<(Cow<'a, [u8]>, bool)> {
        require(self.socket.is_none_or(|retained| retained == socket))?;
        if !self.armed {
            return Ok((Cow::Borrowed(bytes), false));
        }
        require(
            sender == Some(NetlinkAddr::new(0, 0))
                && flags.is_empty()
                && !bytes.is_empty()
                && bytes.len() <= LIMIT
                && end_request.len() == 20
                && u16n(&end_request[4..6])? == 17
                && port != 0,
        )?;
        let end_sequence = u32n(&end_request[8..12])?;
        let mut rest = bytes;
        let mut delivered = Vec::new();
        let mut lost = false;
        while !rest.is_empty() {
            require(rest.len() >= 16)?;
            let length = usize::try_from(u32n(&rest[..4])?).map_err(|_| REFUSE)?;
            require(length >= 16 && aligned(length) <= rest.len())?;
            let frame = &rest[..aligned(length)];
            if u32n(&frame[8..12])? == end_sequence {
                // Validate actual received END success against the outgoing
                // request header. No ACK is constructed on this effect path.
                require(
                    !lost
                        && length == 36
                        && u16n(&frame[4..6])? == 2
                        && matches!(u16n(&frame[6..8])?, 0 | 0x100)
                        && u32n(&frame[12..16])? == port
                        && frame[16..20] == [0; 4]
                        && frame[20..36] == end_request[..16],
                )?;
                lost = true;
            } else {
                // Other messages, including every BEGIN/operation ACK, retain
                // byte-for-byte framing and their actual sender/receive flags.
                delivered.extend_from_slice(frame);
            }
            rest = &rest[aligned(length)..];
        }
        if lost {
            self.armed = false;
            self.consumed += 1;
            Ok((Cow::Owned(delivered), true))
        } else {
            Ok((Cow::Borrowed(bytes), false))
        }
    }
    pub(super) fn confirm_prefix(&mut self) {
        assert_eq!(self.consumed, 1);
        assert!(!self.prefix_delivered);
        self.prefix_delivered = true;
    }
    pub(super) fn observed(&self) -> (usize, bool) {
        (self.consumed, self.prefix_delivered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Constructed ACKs below are PURE unit inputs, never the actual VM path.
    fn requests() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let end = message(17, 5, 12, 0, &[0, 0, 0, 10]);
        let operation = message(2, 0, 11, 42, &[vec![0; 4], end[..16].to_vec()].concat());
        let ack = message(2, 0x100, 12, 42, &[vec![0; 4], end[..16].to_vec()].concat());
        (end, operation, ack)
    }
    fn fault() -> OneShotEndAckLoss {
        let mut fault = OneShotEndAckLoss::default();
        fault.arm(7);
        fault
    }
    #[test]
    fn only_exact_end_is_withheld_other_actual_bytes_are_unchanged() {
        let (end, operation, ack) = requests();
        for bytes in [
            &[operation.clone(), ack.clone()].concat(),
            &[ack.clone(), operation.clone()].concat(),
        ] {
            let mut fault = fault();
            let (delivered, lost) = fault
                .deliver(
                    7,
                    bytes,
                    Some(NetlinkAddr::new(0, 0)),
                    MsgFlags::empty(),
                    &end,
                    42,
                )
                .unwrap();
            assert!(lost);
            assert_eq!(&*delivered, operation);
            assert_eq!(fault.observed(), (1, false));
            fault.confirm_prefix();
            assert_eq!(fault.observed(), (1, true));
        }
        let mut fault = fault();
        let (delivered, lost) = fault
            .deliver(
                7,
                &operation,
                Some(NetlinkAddr::new(0, 0)),
                MsgFlags::empty(),
                &end,
                42,
            )
            .unwrap();
        assert!(!lost);
        assert_eq!(&*delivered, operation);
        assert_eq!(fault.observed(), (0, false));
        let (delivered, lost) = fault
            .deliver(
                7,
                &ack,
                Some(NetlinkAddr::new(0, 0)),
                MsgFlags::empty(),
                &end,
                42,
            )
            .unwrap();
        assert!(lost && delivered.is_empty());
    }
    #[test]
    fn malformed_wrong_socket_sender_flags_or_end_never_count_as_observed_loss() {
        let (end, _, ack) = requests();
        for length in 0..ack.len() {
            let mut fault = fault();
            assert!(
                fault
                    .deliver(
                        7,
                        &ack[..length],
                        Some(NetlinkAddr::new(0, 0)),
                        MsgFlags::empty(),
                        &end,
                        42
                    )
                    .is_err()
            );
            assert_eq!(fault.observed(), (0, false));
        }
        for offset in [0, 4, 6, 12, 16, 20, 24, 28, 32] {
            let mut bad = ack.clone();
            bad[offset] ^= 1;
            let mut fault = fault();
            assert!(
                fault
                    .deliver(
                        7,
                        &bad,
                        Some(NetlinkAddr::new(0, 0)),
                        MsgFlags::empty(),
                        &end,
                        42
                    )
                    .is_err()
            );
            assert_eq!(fault.observed(), (0, false));
        }
        for (socket, sender, flags) in [
            (8, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty()),
            (7, Some(NetlinkAddr::new(1, 0)), MsgFlags::empty()),
            (7, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_TRUNC),
            (7, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_CTRUNC),
        ] {
            let mut fault = fault();
            assert!(
                fault
                    .deliver(socket, &ack, sender, flags, &end, 42)
                    .is_err()
            );
            assert_eq!(fault.observed(), (0, false));
        }
        assert!(
            fault()
                .deliver(
                    7,
                    &[ack.clone(), ack].concat(),
                    Some(NetlinkAddr::new(0, 0)),
                    MsgFlags::empty(),
                    &end,
                    42
                )
                .is_err()
        );
    }
}
