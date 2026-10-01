// SPDX-License-Identifier: MIT

//! Test-only candidate for the *inner* plaintext payload, never a backup file.
//! No cryptography, authentication, semantic validation, or restore authority.
//! Future callers must authenticate a complete bounded outer envelope first.

use crate::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use std::fmt;

// Experimental marker, explicitly not a stable interoperable backup format.
const MAGIC: &[u8; 8] = b"OVTESTP1";
const HEADER_BYTES: usize = 16;
const MAX_PAYLOAD_BYTES: usize = HEADER_BYTES + MAX_PRIVATE_STORE_BYTES + MAX_TEMPLATE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InvalidPayload;

impl fmt::Display for InvalidPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("backup_unreadable")
    }
}

// No Debug, Display, Clone or serialization: contents remain private even if a
// future authenticated envelope yields these bytes. Borrowing does not duplicate
// plaintext; its owner remains responsible for memory lifetime/zeroization.
struct FramedPayload<'a> {
    store: &'a [u8],
    template: &'a [u8],
}

impl<'a> FramedPayload<'a> {
    // Test-only semantic composition. The caller still must authenticate the
    // outer envelope first; neither framing nor a valid store authenticates it.
    fn validate_store(
        &self,
    ) -> Result<crate::private_store::backup_candidate::ValidatedBackupStore<'a>, InvalidPayload>
    {
        crate::private_store::backup_candidate::validate(self.store).map_err(|_| InvalidPayload)
    }
}

fn lengths(store: usize, template: usize) -> Result<usize, InvalidPayload> {
    if store == 0
        || store > MAX_PRIVATE_STORE_BYTES
        || template == 0
        || template > MAX_TEMPLATE_BYTES
    {
        return Err(InvalidPayload);
    }
    HEADER_BYTES
        .checked_add(store)
        .and_then(|size| size.checked_add(template))
        .ok_or(InvalidPayload)
}

fn decode(input: &[u8]) -> Result<FramedPayload<'_>, InvalidPayload> {
    if !(HEADER_BYTES..=MAX_PAYLOAD_BYTES).contains(&input.len())
        || input.get(..8) != Some(MAGIC.as_slice())
    {
        return Err(InvalidPayload);
    }
    let word = |offset| -> Result<usize, InvalidPayload> {
        let bytes = input
            .get(offset..offset + 4)
            .ok_or(InvalidPayload)?
            .try_into()
            .map_err(|_| InvalidPayload)?;
        usize::try_from(u32::from_be_bytes(bytes)).map_err(|_| InvalidPayload)
    };
    let store_len = word(8)?;
    let template_len = word(12)?;
    if lengths(store_len, template_len)? != input.len() {
        return Err(InvalidPayload);
    }
    let (store, template) = input[HEADER_BYTES..].split_at(store_len);
    Ok(FramedPayload { store, template })
}

// Only the synthetic gate uses this encoder. Never write its plaintext output
// as a product backup, even with private permissions.
fn encode(store: &[u8], template: &[u8]) -> Result<Vec<u8>, InvalidPayload> {
    let total = lengths(store.len(), template.len())?;
    let store_len = u32::try_from(store.len()).map_err(|_| InvalidPayload)?;
    let template_len = u32::try_from(template.len()).map_err(|_| InvalidPayload)?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&store_len.to_be_bytes());
    output.extend_from_slice(&template_len.to_be_bytes());
    output.extend_from_slice(store);
    output.extend_from_slice(template);
    Ok(output)
}

#[test]
fn independent_wire_fixture_preserves_exact_bytes() {
    // Hand-authored lengths and body; not an encoder-generated expectation.
    let wire = b"OVTESTP1\x00\x00\x00\x02\x00\x00\x00\x03{}abc";
    let parsed = decode(wire).expect("synthetic framing accepted");
    assert!(parsed.store == b"{}");
    assert!(parsed.template == b"abc");
    assert!(encode(b"{}", b"abc").unwrap() == wire);
    assert!(std::ptr::eq(parsed.store.as_ptr(), wire[16..].as_ptr()));
}

#[test]
fn every_truncation_and_extra_member_is_rejected() {
    let wire = b"OVTESTP1\x00\x00\x00\x02\x00\x00\x00\x03{}abc";
    for end in 0..wire.len() {
        assert!(decode(&wire[..end]).is_err());
    }
    for suffix in [b"\0".as_slice(), b"desired.json", wire.as_slice()] {
        let mut extra = wire.to_vec();
        extra.extend_from_slice(suffix);
        assert!(decode(&extra).is_err());
    }
}

#[test]
fn unknown_version_and_each_magic_byte_are_rejected() {
    let wire = encode(b"{}", b"abc").unwrap();
    for index in 0..MAGIC.len() {
        let mut altered = wire.clone();
        altered[index] ^= 1;
        assert!(decode(&altered).is_err());
    }
}

#[test]
fn untrusted_lengths_refuse_zero_oversize_and_mismatch() {
    for offset in [8, 12] {
        for value in [0_u32, 1, 4, u32::MAX] {
            let mut wire = encode(b"{}", b"abc").unwrap();
            wire[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            assert!(decode(&wire).is_err());
        }
    }
    assert!(lengths(usize::MAX, 1).is_err());
    assert!(lengths(1, usize::MAX).is_err());
}

#[test]
fn exact_member_limits_and_total_limit_are_enforced() {
    let store = vec![0; MAX_PRIVATE_STORE_BYTES];
    let template = vec![0; MAX_TEMPLATE_BYTES];
    let mut wire = encode(&store, &template).unwrap();
    let parsed = decode(&wire).unwrap();
    assert!(parsed.store.len() == MAX_PRIVATE_STORE_BYTES);
    assert!(parsed.template.len() == MAX_TEMPLATE_BYTES);
    wire.push(0);
    assert!(decode(&wire).is_err());
    assert!(encode(&[], b"a").is_err());
    assert!(encode(b"a", &[]).is_err());
    assert!(encode(&vec![0; MAX_PRIVATE_STORE_BYTES + 1], b"a").is_err());
    assert!(encode(b"a", &vec![0; MAX_TEMPLATE_BYTES + 1]).is_err());
    // Correct total size cannot disguise an individually oversized member.
    wire.pop();
    wire[8..12].copy_from_slice(&((MAX_PRIVATE_STORE_BYTES + 1) as u32).to_be_bytes());
    wire[12..16].copy_from_slice(&((MAX_TEMPLATE_BYTES - 1) as u32).to_be_bytes());
    assert!(decode(&wire).is_err());
}

#[test]
fn payload_content_does_not_claim_authentication_or_semantic_validity() {
    // Invalid UTF-8, invalid store and arbitrary template remain framing-only.
    let wire = encode(&[0xff], b"not a routing template").unwrap();
    assert!(decode(&wire).is_ok());
    let mut altered = wire;
    altered[HEADER_BYTES] ^= 1;
    assert!(decode(&altered).is_ok());
    // This deliberate test prevents treating structural success as AEAD proof.
}

#[test]
fn refusal_is_fixed_and_does_not_format_input() {
    let error = decode(b"synthetic-private-marker").err().unwrap();
    assert_eq!(error.to_string(), "backup_unreadable");
    assert_eq!(format!("{error:?}"), "InvalidPayload");
}

#[test]
fn framed_store_validation_does_not_authorize_template_or_restore() {
    let store = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
    let wire = encode(store, b"not a portable routing template").unwrap();
    let framed = decode(&wire).unwrap();
    let validated = framed.validate_store().unwrap();
    assert!(validated.bytes == store);
    assert_eq!(validated.profiles, 0);
    assert_eq!(validated.subscriptions, 0);
    // Template validation is intentionally still a distinct gate.
    assert!(framed.template == b"not a portable routing template");
    let invalid = encode(b"{}", b"not a portable routing template").unwrap();
    assert!(decode(&invalid).unwrap().validate_store().is_err());
}
