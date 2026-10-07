// SPDX-License-Identifier: MIT
//! Explicit developer-only closed failure projection, never authority.
//! Marks perform no I/O. One terminal frame is written before existing park;
//! failed/short output parks without retry or dropping the held service graph.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    NotEntered,
    RecoveryAccept,
    RecoveryOperation,
    RecoveryValidate,
    BeforeAccept,
    ListenerValidate,
    EnrollmentValidate,
    Accept,
    AfterAccept,
    AcceptedListener,
    StreamBlocking,
    Exchange,
    VerifierBefore,
    Creator,
    VerifierAfter,
}
impl Stage {
    const fn token(self) -> &'static [u8] {
        match self {
            Self::NotEntered => b"NOT_ENTERED",
            Self::RecoveryAccept => b"RECOVERY_ACCEPT",
            Self::RecoveryOperation => b"RECOVERY_OPERATION",
            Self::RecoveryValidate => b"RECOVERY_VALIDATE",
            Self::BeforeAccept => b"BEFORE_ACCEPT",
            Self::ListenerValidate => b"LISTENER_VALIDATE",
            Self::EnrollmentValidate => b"ENROLLMENT_VALIDATE",
            Self::Accept => b"ACCEPT",
            Self::AfterAccept => b"AFTER_ACCEPT",
            Self::AcceptedListener => b"ACCEPTED_LISTENER",
            Self::StreamBlocking => b"STREAM_BLOCKING",
            Self::Exchange => b"EXCHANGE",
            Self::VerifierBefore => b"VERIFIER_BEFORE",
            Self::Creator => b"CREATOR",
            Self::VerifierAfter => b"VERIFIER_AFTER",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    RecoveryAccept,
    RecoveryValidate,
    NoEnrollment,
    AuthorityUnavailable,
    Receive,
    ReplyUnknown,
    AcceptUnavailable,
    AuthorityLost,
    ListenerLost,
    EntryOrUnwind,
}
impl Reason {
    const fn token(self) -> &'static [u8] {
        match self {
            Self::RecoveryAccept => b"RECOVERY_ACCEPT",
            Self::RecoveryValidate => b"RECOVERY_VALIDATE",
            Self::NoEnrollment => b"NO_ENROLLMENT",
            Self::AuthorityUnavailable => b"AUTHORITY_UNAVAILABLE",
            Self::Receive => b"RECEIVE",
            Self::ReplyUnknown => b"REPLY_UNKNOWN",
            Self::AcceptUnavailable => b"ACCEPT_UNAVAILABLE",
            Self::AuthorityLost => b"AUTHORITY_LOST",
            Self::ListenerLost => b"LISTENER_LOST",
            Self::EntryOrUnwind => b"ENTRY_OR_UNWIND",
        }
    }
}
type Origin = Option<([u8; 96], usize)>;
thread_local! {
    static LAST: Cell<Stage> = const { Cell::new(Stage::NotEntered) };
    static FIRST_ORIGIN_FAILURE: Cell<Origin> = const { Cell::new(None) };
    static SENT: Cell<bool> = const { Cell::new(false) };
}
pub(crate) fn mark(stage: Stage) {
    LAST.set(stage);
}
pub(crate) fn remember_origin(bytes: [u8; 96], length: usize) {
    if FIRST_ORIGIN_FAILURE.get().is_none() {
        FIRST_ORIGIN_FAILURE.set(Some((bytes, length)));
    }
}
fn frame(stage: Stage, reason: Reason) -> ([u8; 96], usize) {
    let mut bytes = [0; 96];
    let mut length = 0;
    for part in [
        b"K1_DEV_RUNTIME_V1 ".as_slice(),
        stage.token(),
        b" ",
        reason.token(),
        b"\n",
    ] {
        bytes[length..length + part.len()].copy_from_slice(part);
        length += part.len();
    }
    (bytes, length)
}
fn admitted_write(bytes: &[u8], write: impl FnOnce(&[u8]) -> Option<usize>) -> bool {
    write(bytes) == Some(bytes.len())
}
fn write_once(bytes: &[u8]) {
    if !admitted_write(bytes, |frame| {
        nix::unistd::write(std::io::stderr(), frame).ok()
    }) {
        loop {
            std::thread::park();
        }
    }
}
pub(crate) fn emit_refusal(reason: Reason) {
    if SENT.replace(true) {
        return;
    }
    let (bytes, length) = frame(LAST.get(), reason);
    write_once(&bytes[..length]);
    if let Some((bytes, length)) = FIRST_ORIGIN_FAILURE.get() {
        write_once(&bytes[..length]);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_output_is_one_attempt_and_refuses_short_failed_or_extra_count() {
        for result in [None, Some(0), Some(2), Some(4)] {
            let calls = Cell::new(0);
            assert!(!admitted_write(b"abc", |_| {
                calls.set(calls.get() + 1);
                result
            }));
            assert_eq!(calls.get(), 1);
        }
        assert!(admitted_write(b"abc", |_| Some(3)));
    }
    #[test]
    fn finite_runtime_frames_and_original_failure_are_thread_local() {
        let stages = [
            Stage::NotEntered,
            Stage::RecoveryAccept,
            Stage::RecoveryOperation,
            Stage::RecoveryValidate,
            Stage::BeforeAccept,
            Stage::ListenerValidate,
            Stage::EnrollmentValidate,
            Stage::Accept,
            Stage::AfterAccept,
            Stage::AcceptedListener,
            Stage::StreamBlocking,
            Stage::Exchange,
            Stage::VerifierBefore,
            Stage::Creator,
            Stage::VerifierAfter,
        ];
        let reasons = [
            Reason::RecoveryAccept,
            Reason::RecoveryValidate,
            Reason::NoEnrollment,
            Reason::AuthorityUnavailable,
            Reason::Receive,
            Reason::ReplyUnknown,
            Reason::AcceptUnavailable,
            Reason::AuthorityLost,
            Reason::ListenerLost,
            Reason::EntryOrUnwind,
        ];
        for stage in stages {
            for reason in reasons {
                let (bytes, length) = frame(stage, reason);
                assert!(length <= 96);
                let text = std::str::from_utf8(&bytes[..length]).unwrap();
                assert!(text.starts_with("K1_DEV_RUNTIME_V1 "));
                assert_eq!(text.matches('\n').count(), 1);
            }
        }
        mark(Stage::VerifierBefore);
        remember_origin([b'X'; 96], 12);
        remember_origin([b'Y'; 96], 13);
        assert_eq!(FIRST_ORIGIN_FAILURE.get(), Some(([b'X'; 96], 12)));
        std::thread::spawn(|| {
            assert_eq!(LAST.get(), Stage::NotEntered);
            assert!(FIRST_ORIGIN_FAILURE.get().is_none());
        })
        .join()
        .unwrap();
        assert_eq!(LAST.get(), Stage::VerifierBefore);
        FIRST_ORIGIN_FAILURE.set(None);
    }
}
