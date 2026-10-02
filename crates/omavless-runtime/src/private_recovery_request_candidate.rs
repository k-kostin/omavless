// SPDX-License-Identifier: MIT
//! Inactive private request reader. No listener, registration, KDF, archive
//! authentication, lease acquisition, filesystem operation or executor call.
//! Framing/peer admission NEVER supplies recovery or normal-owner authority.

use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use nix::unistd::{geteuid, getuid};
use omavless_domain::private_backup::MAX_BACKUP_BYTES;
use std::fmt;
use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"OVRREQ01";
const HEADER_BYTES: usize = 20;
const RESYNC_COMPLETED: u8 = 1;
// Fixed candidate transport-v1 bounds, not a claim of archive authentication.
const MIN_PASSPHRASE_BYTES: usize = 12;
const MAX_PASSPHRASE_BYTES: usize = 1024;
const MAX_BODY_BYTES: usize = MAX_BACKUP_BYTES + MAX_PASSPHRASE_BYTES;
const READ_BUDGET: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestError {
    Refused,
}
impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Private recovery request refused")
    }
}
impl std::error::Error for RequestError {}
type Result<T> = std::result::Result<T, RequestError>;
const REFUSE: RequestError = RequestError::Refused;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    ResyncCompletedStillFenced,
}

/// One zeroizing allocation owns ciphertext and passphrase; accessors borrow
/// slices without String/JSON copies. Intentionally no Debug, Clone or encoder.
/// Owning zeroization covers normal return/unwind, not SIGKILL/process death.
pub(crate) struct Request {
    body: Zeroizing<Vec<u8>>,
    ciphertext_bytes: usize,
}
impl Request {
    pub(crate) fn operation(&self) -> Operation {
        Operation::ResyncCompletedStillFenced
    }
    pub(crate) fn ciphertext(&self) -> &[u8] {
        &self.body[..self.ciphertext_bytes]
    }
    pub(crate) fn passphrase(&self) -> &[u8] {
        &self.body[self.ciphertext_bytes..]
    }
}

fn peer_allowed(real_uid: u32, effective_uid: u32, peer_uid: u32, peer_pid: i32) -> bool {
    real_uid == effective_uid && peer_uid == effective_uid && peer_pid > 0
}

fn checked_lengths(header: &[u8; HEADER_BYTES]) -> Result<(usize, usize)> {
    if &header[..8] != MAGIC
        || header[8] != RESYNC_COMPLETED
        || header[9..12].iter().chain(&header[18..20]).any(|b| *b != 0)
    {
        return Err(REFUSE);
    }
    let ciphertext = usize::try_from(u32::from_le_bytes(
        header[12..16].try_into().map_err(|_| REFUSE)?,
    ))
    .map_err(|_| REFUSE)?;
    let passphrase = usize::from(u16::from_le_bytes(
        header[16..18].try_into().map_err(|_| REFUSE)?,
    ));
    if ciphertext == 0
        || ciphertext > MAX_BACKUP_BYTES
        || !(MIN_PASSPHRASE_BYTES..=MAX_PASSPHRASE_BYTES).contains(&passphrase)
    {
        return Err(REFUSE);
    }
    let total = ciphertext
        .checked_add(passphrase)
        .filter(|n| *n <= MAX_BODY_BYTES)
        .ok_or(REFUSE)?;
    Ok((ciphertext, total))
}

fn read_before(stream: &mut UnixStream, bytes: &mut [u8], deadline: Instant) -> Result<usize> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(REFUSE)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| REFUSE)?;
        match stream.read(bytes) {
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Ok(count) if Instant::now() <= deadline => return Ok(count),
            _ => return Err(REFUSE),
        }
    }
}

fn read_exact_before(stream: &mut UnixStream, bytes: &mut [u8], deadline: Instant) -> Result<()> {
    let mut filled = 0;
    while filled < bytes.len() {
        let count = read_before(stream, &mut bytes[filled..], deadline)?;
        if count == 0 {
            return Err(REFUSE);
        }
        filled += count;
    }
    Ok(())
}

/// Consumes one already-connected stream. Kernel peer credentials are checked
/// before even the fixed header is read, and BEFORE attacker-proportional
/// allocation. Caller supplies no path, UID, timeout or operation override.
/// The sender must half-close after its one exact frame; trailing data refuses.
pub(crate) fn receive(stream: UnixStream) -> Result<Request> {
    receive_with_budget(stream, READ_BUDGET)
}

fn receive_with_budget(mut stream: UnixStream, budget: Duration) -> Result<Request> {
    if budget.is_zero() || budget > READ_BUDGET {
        return Err(REFUSE);
    }
    let credentials = getsockopt(&stream, PeerCredentials).map_err(|_| REFUSE)?;
    if !peer_allowed(
        getuid().as_raw(),
        geteuid().as_raw(),
        credentials.uid(),
        credentials.pid(),
    ) {
        return Err(REFUSE);
    }
    let deadline = Instant::now().checked_add(budget).ok_or(REFUSE)?;
    // Even malformed header/extra-byte input is private and zeroized on drop.
    let mut header = Zeroizing::new([0_u8; HEADER_BYTES]);
    read_exact_before(&mut stream, &mut header[..], deadline)?;
    let (ciphertext_bytes, total) = checked_lengths(&header)?;
    let mut body = Zeroizing::new(Vec::new());
    body.try_reserve_exact(total).map_err(|_| REFUSE)?;
    body.resize(total, 0);
    read_exact_before(&mut stream, &mut body, deadline)?;
    let mut extra = Zeroizing::new([0_u8; 1]);
    if read_before(&mut stream, &mut extra[..], deadline)? != 0 {
        return Err(REFUSE);
    }
    Ok(Request {
        body,
        ciphertext_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::Shutdown;
    use std::thread;

    const CIPHER: &[u8] = b"synthetic opaque ciphertext";
    const SECRET: &[u8] = b"synthetic secret only";
    fn header(ciphertext: u32, passphrase: u16) -> [u8; HEADER_BYTES] {
        let mut h = [0; HEADER_BYTES];
        h[..8].copy_from_slice(MAGIC);
        h[8] = RESYNC_COMPLETED;
        h[12..16].copy_from_slice(&ciphertext.to_le_bytes());
        h[16..18].copy_from_slice(&passphrase.to_le_bytes());
        h
    }
    fn frame(cipher: &[u8], secret: &[u8]) -> Zeroizing<Vec<u8>> {
        let mut raw = Zeroizing::new(header(cipher.len() as u32, secret.len() as u16).to_vec());
        raw.extend_from_slice(cipher);
        raw.extend_from_slice(secret);
        raw
    }
    fn received(raw: Zeroizing<Vec<u8>>) -> Result<Request> {
        let (rx, mut tx) = UnixStream::pair().unwrap();
        let sender = thread::spawn(move || {
            let _ = tx.write_all(&raw);
            let _ = tx.shutdown(Shutdown::Write);
        });
        let result = receive(rx);
        sender.join().unwrap();
        result
    }
    #[test]
    fn private_recovery_request_same_peer_opaque_borrowed_parts_and_only_fixed_operation() {
        for secret in [SECRET, &[0x80; MIN_PASSPHRASE_BYTES]] {
            let request = received(frame(CIPHER, secret)).unwrap();
            assert_eq!(request.operation(), Operation::ResyncCompletedStillFenced);
            assert_eq!(request.ciphertext(), CIPHER);
            assert_eq!(request.passphrase(), secret);
            assert_eq!(request.ciphertext().as_ptr(), request.body.as_ptr());
            assert_eq!(
                request.passphrase().as_ptr(),
                request.body[CIPHER.len()..].as_ptr()
            );
        }
        fn clears<T: zeroize::ZeroizeOnDrop>() {}
        clears::<Zeroizing<Vec<u8>>>();
        clears::<Zeroizing<[u8; HEADER_BYTES]>>();
    }
    #[test]
    fn private_recovery_request_every_truncation_trailing_byte_and_unknown_field_refuses() {
        let good = frame(CIPHER, SECRET);
        for length in 0..good.len() {
            assert!(
                received(Zeroizing::new(good[..length].to_vec())).is_err(),
                "length={length}"
            );
        }
        let mut extra = frame(CIPHER, SECRET);
        extra.push(1);
        assert!(received(extra).is_err());
        for index in (0..12).chain(18..20) {
            let mut raw = frame(CIPHER, SECRET);
            raw[index] ^= 0x80;
            assert!(received(raw).is_err(), "index={index}");
        }
    }
    #[test]
    fn private_recovery_request_header_limits_refuse_before_waiting_for_body() {
        for (cipher, secret) in [
            (0, 12),
            (u32::MAX, 12),
            ((MAX_BACKUP_BYTES + 1) as u32, 12),
            (1, 0),
            (1, 11),
            (1, 1025),
            (1, u16::MAX),
        ] {
            let (rx, mut tx) = UnixStream::pair().unwrap();
            tx.write_all(&header(cipher, secret)).unwrap();
            // Keep peer open: a mistaken body read would wait for the full 3s.
            let started = Instant::now();
            assert!(receive(rx).is_err());
            assert!(started.elapsed() < Duration::from_secs(2));
        }
        assert_eq!(
            checked_lengths(&header(MAX_BACKUP_BYTES as u32, 1024)),
            Ok((MAX_BACKUP_BYTES, MAX_BODY_BYTES))
        );
    }
    #[test]
    fn private_recovery_request_peer_gate_rejects_wrong_uid_setuid_and_unconnected_pid() {
        assert!(peer_allowed(1000, 1000, 1000, 1));
        for (real, effective, peer, pid) in [
            (1000, 1000, 1001, 1),
            (1000, 0, 0, 1),
            (1000, 1000, 1000, 0),
            (1000, 1000, 1000, -1),
        ] {
            assert!(!peer_allowed(real, effective, peer, pid));
        }
        // Actual SO_PEERCRED is exercised by every UnixStream::pair test.
        // Cross-UID kernel integration is a separate installed acceptance gate.
    }
    #[test]
    fn private_recovery_request_total_deadline_handles_silent_and_trickling_peer() {
        let (rx, _tx) = UnixStream::pair().unwrap();
        assert!(receive_with_budget(rx, Duration::from_millis(30)).is_err());
        let (rx, mut tx) = UnixStream::pair().unwrap();
        let sender = thread::spawn(move || {
            for byte in frame(CIPHER, SECRET).iter() {
                if tx.write_all(&[*byte]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let started = Instant::now();
        assert!(receive_with_budget(rx, Duration::from_millis(60)).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        sender.join().unwrap();
    }
    #[test]
    fn private_recovery_request_complete_body_without_half_close_is_not_success() {
        let (rx, mut tx) = UnixStream::pair().unwrap();
        tx.write_all(&frame(CIPHER, SECRET)).unwrap();
        assert!(receive_with_budget(rx, Duration::from_millis(30)).is_err());
        let request = received(frame(CIPHER, &[0x81; MAX_PASSPHRASE_BYTES])).unwrap();
        assert_eq!(request.passphrase().len(), MAX_PASSPHRASE_BYTES);
    }
    #[test]
    fn private_recovery_request_errors_are_fixed_and_success_is_not_archive_authentication() {
        let error = received(frame(b"not an envelope", b"short")).err().unwrap();
        assert_eq!(error.to_string(), "Private recovery request refused");
        assert_eq!(format!("{error:?}"), "Refused");
        // Valid framing of non-envelope bytes is intentionally NOT decryption.
        assert!(received(frame(b"not an envelope", SECRET)).is_ok());
    }
}
