// SPDX-License-Identifier: MIT
//! Private bytes inside the original actor channel, never FD/proof import.
//!
//! One pre-reserved input slot. Every received prefix and positive authenticated
//! plaintext remains owned here across Result failure; only normal Halt clears
//! it. Crypto's own internal allocations/zeroization are backend boundaries.

use super::{Unavailable, tick};
use omavless_domain::private_backup::{MAX_BACKUP_BYTES, OpenedBackup};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

const HEADER_BYTES: usize = 16;
const MAGIC: &[u8; 8] = b"OVT4TR01";
const MIN_PASSPHRASE: usize = 12;
const MAX_PASSPHRASE: usize = 1024;
const RESERVED_BYTES: usize = MAX_BACKUP_BYTES + MAX_PASSPHRASE;

fn read_into<T: Read>(stream: &mut T, bytes: &mut [u8], until: Instant) -> Result<(), Unavailable> {
    let mut done = 0;
    while done < bytes.len() {
        tick(until)?;
        match stream.read(&mut bytes[done..]) {
            Ok(0) => return Err(Unavailable),
            Ok(size) => done += size,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return Err(Unavailable),
        }
        tick(until)?;
    }
    Ok(())
}

fn write_bytes<T: Write>(stream: &mut T, bytes: &[u8], until: Instant) -> Result<(), Unavailable> {
    let mut done = 0;
    while done < bytes.len() {
        tick(until)?;
        match stream.write(&bytes[done..]) {
            Ok(0) => return Err(Unavailable),
            Ok(size) => done += size,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return Err(Unavailable),
        }
        tick(until)?;
    }
    Ok(())
}

fn header(archive: usize, passphrase: usize) -> Result<[u8; HEADER_BYTES], Unavailable> {
    if !(1..=MAX_BACKUP_BYTES).contains(&archive)
        || !(MIN_PASSPHRASE..=MAX_PASSPHRASE).contains(&passphrase)
    {
        return Err(Unavailable);
    }
    let mut raw = [0; HEADER_BYTES];
    raw[..8].copy_from_slice(MAGIC);
    raw[8..12].copy_from_slice(
        &u32::try_from(archive)
            .map_err(|_| Unavailable)?
            .to_be_bytes(),
    );
    raw[12..14].copy_from_slice(
        &u16::try_from(passphrase)
            .map_err(|_| Unavailable)?
            .to_be_bytes(),
    );
    Ok(raw)
}

fn lengths(raw: &[u8; HEADER_BYTES]) -> Result<(usize, usize), Unavailable> {
    if &raw[..8] != MAGIC || raw[14..] != [0, 0] {
        return Err(Unavailable);
    }
    let archive = usize::try_from(u32::from_be_bytes(
        raw[8..12].try_into().map_err(|_| Unavailable)?,
    ))
    .map_err(|_| Unavailable)?;
    let passphrase = usize::from(u16::from_be_bytes(
        raw[12..14].try_into().map_err(|_| Unavailable)?,
    ));
    // Reuse exactly the writer's finite bounds, no input-controlled KDF cost.
    header(archive, passphrase)?;
    Ok((archive, passphrase))
}

pub(super) fn send<T: Write>(
    stream: &mut T,
    archive: &[u8],
    passphrase: &[u8],
    until: Instant,
) -> Result<(), Unavailable> {
    let raw = header(archive.len(), passphrase.len())?;
    write_bytes(stream, &raw, until)?;
    write_bytes(stream, archive, until)?;
    write_bytes(stream, passphrase, until)
}

pub(super) struct Transfer {
    header: [u8; HEADER_BYTES],
    // Allocate and initialize the entire finite slot BEFORE READY/acquisition.
    // No continuation allocation or growth after private bytes are received.
    body: Zeroizing<Vec<u8>>,
    state: InputState,
    opened: Option<OpenedBackup>,
    restored_store: Option<Zeroizing<Vec<u8>>>,
    restore_consumed: bool,
    authenticated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputState {
    Fresh,
    Admitted,
    Consumed,
}

impl Transfer {
    pub fn new() -> Result<Self, Unavailable> {
        let mut body = Vec::new();
        body.try_reserve_exact(RESERVED_BYTES)
            .map_err(|_| Unavailable)?;
        body.resize(RESERVED_BYTES, 0);
        Ok(Self {
            header: [0; HEADER_BYTES],
            body: Zeroizing::new(body),
            state: InputState::Fresh,
            opened: None,
            restored_store: None,
            restore_consumed: false,
            authenticated: false,
        })
    }

    pub fn admit(&mut self, until: Instant) -> Result<(), Unavailable> {
        if self.state != InputState::Fresh {
            return Err(Unavailable);
        }
        // Consume admission BEFORE manager capture or private input. Even a
        // deadline failure here cannot restore this slot or permit new effects.
        self.state = InputState::Consumed;
        tick(until)?;
        self.state = InputState::Admitted;
        Ok(())
    }

    pub fn receive<T: Read>(&mut self, stream: &mut T, until: Instant) -> Result<(), Unavailable> {
        if self.state != InputState::Admitted {
            return Err(Unavailable);
        }
        // One admitted read only; preserve every prefix even on failure.
        self.state = InputState::Consumed;
        read_into(stream, &mut self.header, until)?;
        let (archive, passphrase) = lengths(&self.header)?;
        let total = archive.checked_add(passphrase).ok_or(Unavailable)?;
        read_into(stream, &mut self.body[..total], until)?;
        tick(until)?;
        let opened = omavless_domain::private_backup::open(
            &self.body[..archive],
            &self.body[archive..total],
        )
        .map_err(|_| Unavailable)?;
        // Positive authenticated return retained BEFORE its post-call deadline.
        // No plaintext/source byte or reusable authority leaves this module.
        self.keep_opened(opened, until)
    }

    fn keep_opened(&mut self, opened: OpenedBackup, until: Instant) -> Result<(), Unavailable> {
        self.opened = Some(opened);
        self.authenticated = false;
        tick(until)?;
        self.authenticated = true;
        Ok(())
    }

    pub fn with_restore_pair(
        &mut self,
        until: Instant,
        operation: impl FnOnce(&[u8], &[u8]) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        if self.state != InputState::Consumed || self.restore_consumed || !self.authenticated {
            return Err(Unavailable);
        }
        self.restore_consumed = true;
        self.authenticated = false;
        tick(until)?;
        let opened = self.opened.as_ref().ok_or(Unavailable)?;
        let restored = opened.restore_store_off().map_err(|_| Unavailable)?;
        self.restored_store = Some(restored); // positive owned bytes BEFORE post-tick
        tick(until)?;
        operation(
            self.restored_store.as_ref().ok_or(Unavailable)?,
            opened.template(),
        )
    }

    pub fn finish(&mut self) {
        // Only the actor's separately valid normal Halt may call this. No
        // uncertain owner, failed transfer or channel-loss cleanup calls it.
        self.header.zeroize();
        self.body.as_mut_slice().zeroize();
        self.opened = None;
        self.restored_store = None;
        self.restore_consumed = true;
        self.authenticated = false;
        self.state = InputState::Consumed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const PASSPHRASE: &[u8] = b"synthetic transfer passphrase";
    const STORE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
    const TEMPLATE: &[u8] = include_bytes!("../../../templates/default.yaml");

    #[test]
    fn fixed_header_whole_bound_and_no_extension_or_cost_parameters() {
        assert_eq!(RESERVED_BYTES, MAX_BACKUP_BYTES + 1024);
        assert_eq!(MAX_BACKUP_BYTES, 52 + 16 + 16 + 7 * 1024 * 1024);
        for (archive, passphrase) in [(1, 12), (MAX_BACKUP_BYTES, 1024)] {
            let raw = header(archive, passphrase).unwrap();
            assert_eq!(lengths(&raw).unwrap(), (archive, passphrase));
            for index in [0, 14, 15] {
                let mut changed = raw;
                changed[index] = 255;
                assert!(lengths(&changed).is_err());
            }
        }
        for (archive, passphrase) in [(0, 12), (MAX_BACKUP_BYTES + 1, 12), (1, 11), (1, 1025)] {
            assert!(header(archive, passphrase).is_err());
        }
    }

    #[test]
    fn invalid_header_refuses_before_body_and_cannot_replace_the_owned_prefix() {
        for index in [0, 8, 12, 14, 15] {
            let mut raw = header(3, PASSPHRASE.len()).unwrap().to_vec();
            raw[index] = 255;
            raw.extend_from_slice(b"not read or admitted");
            let mut owner = Transfer::new().unwrap();
            owner
                .admit(Instant::now() + Duration::from_secs(1))
                .unwrap();
            let mut stream = Cursor::new(raw.clone());
            assert!(
                owner
                    .receive(&mut stream, Instant::now() + Duration::from_secs(1))
                    .is_err()
            );
            assert_eq!(stream.position(), 16);
            assert_eq!(owner.header, raw[..16]);
            assert!(owner.body.iter().all(|byte| *byte == 0));
            assert!(
                owner
                    .admit(Instant::now() + Duration::from_secs(1))
                    .is_err()
            );
            assert!(
                owner
                    .receive(&mut stream, Instant::now() + Duration::from_secs(1))
                    .is_err()
            );
            assert_eq!(stream.position(), 16);
        }
    }

    #[test]
    fn every_truncated_prefix_is_owned_and_second_receive_has_no_io() {
        let mut raw = header(3, PASSPHRASE.len()).unwrap().to_vec();
        raw.extend_from_slice(b"bad");
        raw.extend_from_slice(PASSPHRASE);
        for cut in 0..=raw.len() {
            let mut owner = Transfer::new().unwrap();
            let capacity = owner.body.capacity();
            let mut stream = Cursor::new(raw[..cut].to_vec());
            owner
                .admit(Instant::now() + Duration::from_secs(1))
                .unwrap();
            assert!(
                owner
                    .receive(&mut stream, Instant::now() + Duration::from_secs(1))
                    .is_err()
            );
            assert_eq!(owner.state, InputState::Consumed);
            assert!(owner.opened.is_none());
            assert_eq!(owner.body.capacity(), capacity);
            assert_eq!(&owner.header[..cut.min(16)], &raw[..cut.min(16)]);
            let body = cut.saturating_sub(16);
            assert_eq!(&owner.body[..body], &raw[16.min(cut)..cut]);
            let position = stream.position();
            assert!(
                owner
                    .receive(&mut stream, Instant::now() + Duration::from_secs(1))
                    .is_err()
            );
            assert_eq!(stream.position(), position);
        }
    }

    #[test]
    fn genuine_v1_authentication_stays_private_and_normal_finish_zeroizes() {
        let archive = omavless_domain::private_backup::seal(STORE, TEMPLATE, PASSPHRASE).unwrap();
        let mut raw = Vec::new();
        send(
            &mut raw,
            &archive,
            PASSPHRASE,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        let mut owner = Transfer::new().unwrap();
        owner
            .admit(Instant::now() + Duration::from_secs(1))
            .unwrap();
        owner
            .receive(
                &mut Cursor::new(raw),
                Instant::now() + Duration::from_secs(20),
            )
            .unwrap();
        assert_eq!(owner.opened.as_ref().unwrap().store(), STORE);
        assert_eq!(owner.opened.as_ref().unwrap().template(), TEMPLATE);
        assert_eq!(owner.state, InputState::Consumed);
        assert!(
            owner
                .with_restore_pair(
                    Instant::now() + Duration::from_secs(1),
                    |store, template| {
                        assert_eq!(store, STORE);
                        assert_eq!(template, TEMPLATE);
                        Err(Unavailable) // downstream stage failure retains the private return
                    }
                )
                .is_err()
        );
        assert_eq!(owner.restored_store.as_ref().unwrap().as_slice(), STORE);
        assert!(
            owner
                .with_restore_pair(Instant::now() + Duration::from_secs(1), |_, _| panic!(
                    "normalize reentry"
                ))
                .is_err()
        );
        // A reported positive backend result survives a late post-call tick.
        let opened = owner.opened.take().unwrap();
        assert!(
            owner
                .keep_opened(opened, Instant::now() - Duration::from_secs(1))
                .is_err()
        );
        assert!(owner.opened.is_some());
        assert!(!owner.authenticated);
        owner.finish();
        assert!(owner.opened.is_none());
        assert!(owner.restored_store.is_none());
        assert!(owner.body.iter().all(|byte| *byte == 0));
        assert_eq!(owner.header, [0; 16]);
    }

    #[test]
    fn expiry_consumes_input_slot_without_first_read_or_reset() {
        let mut owner = Transfer::new().unwrap();
        let mut stream = Cursor::new(vec![0; 16]);
        assert!(
            owner
                .receive(&mut stream, Instant::now() + Duration::from_secs(1))
                .is_err()
        );
        assert_eq!(stream.position(), 0);
        assert!(
            owner
                .admit(Instant::now() - Duration::from_secs(1))
                .is_err()
        );
        assert!(
            owner
                .admit(Instant::now() + Duration::from_secs(1))
                .is_err()
        );
        assert!(
            owner
                .receive(&mut stream, Instant::now() - Duration::from_secs(1))
                .is_err()
        );
        assert_eq!(owner.state, InputState::Consumed);
        assert_eq!(stream.position(), 0);
        assert!(
            owner
                .receive(&mut stream, Instant::now() + Duration::from_secs(1))
                .is_err()
        );
        assert_eq!(stream.position(), 0);
    }
}
