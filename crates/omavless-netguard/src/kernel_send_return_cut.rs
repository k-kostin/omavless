//! Fixed cfg(test) checkpoint AFTER full send return, BEFORE first receive.
//! It proves neither kernel commit nor ACK delivery. No production entrypoint.
use super::atomic_batch::AtomicReplies;
use super::*;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SendKind {
    Create,
    Replace,
    Delete,
}
impl SendKind {
    pub(super) fn from_case(case: &str) -> Option<Self> {
        match case {
            "send-create" => Some(Self::Create),
            "send-replace" => Some(Self::Replace),
            "send-delete" => Some(Self::Delete),
            _ => None,
        }
    }
    pub(super) fn checkpoint(self) -> &'static [u8] {
        match self {
            Self::Create => b"K1_SEND_RETURN_CUT create sends=1 acks=0 readbacks=0\n",
            Self::Replace => b"K1_SEND_RETURN_CUT replace sends=1 acks=0 readbacks=0\n",
            Self::Delete => b"K1_SEND_RETURN_CUT delete sends=1 acks=0 readbacks=0\n",
        }
    }
    pub(super) fn phase(self) -> &'static str {
        match self {
            Self::Create => "pending_create",
            Self::Replace => "pending_replace",
            Self::Delete => "pending_delete",
        }
    }
    fn matches(self, replies: &AtomicReplies) -> bool {
        let requests = replies.requests();
        match self {
            Self::Create => requests.len() == 14 && u16n(&requests[1][4..6]) == Ok(NFT),
            Self::Replace => requests.len() == 15 && u16n(&requests[1][4..6]) == Ok(NFT + 2),
            Self::Delete => requests.len() == 3 && u16n(&requests[1][4..6]) == Ok(NFT + 2),
        }
    }
}

#[derive(Default)]
pub(super) struct OneShotSendCut {
    armed: Option<(i32, SendKind, PathBuf)>,
    consumed: bool,
}
impl OneShotSendCut {
    pub(super) fn arm(&mut self, socket: i32, kind: SendKind, parent: PathBuf) {
        assert!(socket >= 0 && self.armed.is_none() && !self.consumed);
        let meta = std::fs::symlink_metadata(&parent).unwrap();
        assert!(meta.is_dir() && meta.uid() == 1001 && meta.mode() & 0o7777 == 0o700);
        self.armed = Some((socket, kind, parent));
    }
    fn take(
        &mut self,
        socket: i32,
        replies: &AtomicReplies,
    ) -> Result<Option<(SendKind, PathBuf)>> {
        require(!self.consumed)?;
        let Some((retained, kind, parent)) = self.armed.take() else {
            return Ok(None);
        };
        self.consumed = true;
        require(retained == socket && kind.matches(replies))?;
        require(!replies.complete() && !replies.poisoned() && !replies.changed())?;
        require(replies.acks().iter().all(|ack| !ack))?;
        Ok(Some((kind, parent)))
    }
    pub(super) fn after_send(&mut self, socket: i32, replies: &AtomicReplies) -> Result<()> {
        let Some((kind, parent)) = self.take(socket, replies)? else {
            return Ok(());
        };
        // Called only after the adapter's successful full-length send check.
        // The holder owns this private fixture and validates the exact bytes,
        // Pending record and live lock before killing the still-unreaped writer.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(parent.join(".send-return-cut.next"))
            .map_err(|_| REFUSE)?;
        file.write_all(kind.checkpoint()).map_err(|_| REFUSE)?;
        file.sync_all().map_err(|_| REFUSE)?;
        require(
            matches!(std::fs::symlink_metadata(parent.join("send-return-cut")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound),
        )?;
        std::fs::rename(
            parent.join(".send-return-cut.next"),
            parent.join("send-return-cut"),
        )
        .map_err(|_| REFUSE)?;
        std::thread::sleep(Duration::from_secs(10));
        Err(REFUSE) // Not killed at the checkpoint: never continue to receive.
    }
}

#[cfg(test)]
mod tests {
    use super::super::atomic_batch::{delete_batch, full_batch};
    use super::*;
    fn replies(kind: SendKind) -> AtomicReplies {
        let batch = match kind {
            SendKind::Create => full_batch(7, 10, None),
            SendKind::Replace => full_batch(7, 10, Some(9)),
            SendKind::Delete => delete_batch(7, 9, 10),
        }
        .unwrap();
        AtomicReplies::new(batch, 42).unwrap()
    }
    fn armed(kind: SendKind) -> OneShotSendCut {
        // Pure state-machine input only; no file, socket, send or checkpoint.
        OneShotSendCut {
            armed: Some((7, kind, PathBuf::from("unused"))),
            consumed: false,
        }
    }
    #[test]
    fn send_return_cut_is_one_shot_and_fixed_to_socket_and_operation() {
        for kind in [SendKind::Create, SendKind::Replace, SendKind::Delete] {
            let mut hook = armed(kind);
            assert_eq!(
                hook.take(7, &replies(kind)).unwrap(),
                Some((kind, PathBuf::from("unused")))
            );
            assert!(hook.take(7, &replies(kind)).is_err());
            let mut wrong_socket = armed(kind);
            assert!(wrong_socket.take(8, &replies(kind)).is_err());
            assert!(wrong_socket.take(7, &replies(kind)).is_err());
            for other in [SendKind::Create, SendKind::Replace, SendKind::Delete] {
                if kind != other {
                    let mut wrong_kind = armed(kind);
                    assert!(wrong_kind.take(7, &replies(other)).is_err());
                    assert!(wrong_kind.take(7, &replies(kind)).is_err());
                }
            }
        }
    }
    #[test]
    fn send_return_cut_refuses_any_already_observed_ack_or_poison() {
        for kind in [SendKind::Create, SendKind::Replace, SendKind::Delete] {
            let mut collected = replies(kind);
            let request = &collected.requests()[0];
            let body = [0_i32.to_ne_bytes().to_vec(), request[..16].to_vec()].concat();
            let ack = message(2, 0, 10, 42, &body);
            collected
                .receive(&ack, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
                .unwrap();
            assert!(armed(kind).take(7, &collected).is_err());
            collected.poison();
            assert!(armed(kind).take(7, &collected).is_err());
        }
    }
    #[test]
    fn send_return_cut_default_is_inactive_without_touching_path() {
        let mut hook = OneShotSendCut::default();
        for kind in [SendKind::Create, SendKind::Replace, SendKind::Delete] {
            assert_eq!(hook.take(7, &replies(kind)).unwrap(), None);
        }
        assert!(!hook.consumed);
    }
    #[test]
    fn send_return_cut_vocabulary_cannot_select_an_arbitrary_checkpoint() {
        for case in [
            "",
            "create",
            "crash-create",
            "send-create extra",
            "send-all",
        ] {
            assert_eq!(SendKind::from_case(case), None);
        }
        for (case, kind, phase) in [
            ("send-create", SendKind::Create, "pending_create"),
            ("send-replace", SendKind::Replace, "pending_replace"),
            ("send-delete", SendKind::Delete, "pending_delete"),
        ] {
            assert_eq!(SendKind::from_case(case), Some(kind));
            assert_eq!(kind.phase(), phase);
            assert!(
                std::str::from_utf8(kind.checkpoint())
                    .unwrap()
                    .ends_with("sends=1 acks=0 readbacks=0\n")
            );
        }
    }
}
