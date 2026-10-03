// SPDX-License-Identifier: MIT
//! Private cfg(test) receive fault. It changes only the iovec capacity of the
//! next successful real recvmsg after an effect send, never bytes or metadata.
use super::*;

#[derive(Default)]
pub(super) struct OneShotTruncation {
    socket: Option<i32>,
    armed: bool,
    observed: usize,
}
impl OneShotTruncation {
    pub(super) fn arm(&mut self, socket: i32) {
        assert!(socket >= 0 && self.socket.is_none() && !self.armed && self.observed == 0);
        self.socket = Some(socket);
        self.armed = true;
    }
    pub(super) fn capacity(&self, socket: i32) -> Result<usize> {
        require(self.socket.is_none_or(|retained| retained == socket))?;
        Ok(if self.armed { 1 } else { LIMIT })
    }
    pub(super) fn received(&mut self, length: usize, flags: MsgFlags) -> Result<()> {
        if self.armed {
            // EAGAIN never consumes the one-shot. Only actual successful
            // recvmsg supplies these values. No input MSG_TRUNC flag is used.
            require(length == 1 && flags.contains(MsgFlags::MSG_TRUNC))?;
            self.armed = false;
            self.observed += 1;
        }
        Ok(())
    }
    pub(super) fn observed(&self) -> usize {
        self.observed
    }
}
