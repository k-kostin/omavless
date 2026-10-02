// SPDX-License-Identifier: MIT

//! Bounded, authenticated portable backup bytes. This module has no filesystem,
//! IPC, runtime ownership, preview, or restore authority. A caller must obtain
//! one committed store/template snapshot and publish the sealed result safely.

use crate::backup_payload_candidate::{
    HEADER_BYTES as INNER_HEADER_BYTES, MAX_PAYLOAD_BYTES, decode, encode,
};
use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

const MAGIC: &[u8; 8] = b"OVBKUP01";
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const HEADER_BYTES: usize = 8 + SALT_BYTES + NONCE_BYTES + 4;
const TAG_BYTES: usize = 16;
const MIN_PASSPHRASE_BYTES: usize = 12;
const MAX_PASSPHRASE_BYTES: usize = 1024;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_LANES: u32 = 1;

/// Maximum accepted encrypted file size. No decompression or variable KDF
/// resource parameters are read from a file.
pub const MAX_BACKUP_BYTES: usize = HEADER_BYTES + MAX_PAYLOAD_BYTES + TAG_BYTES;

/// Fixed public errors never include a passphrase, member, parser diagnostic or
/// authenticated content. An unreadable file does not distinguish wrong key,
/// tampering, future format and invalid plaintext semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    InvalidInput,
    Unavailable,
    Unreadable,
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "backup_invalid_input",
            Self::Unavailable => "backup_unavailable",
            Self::Unreadable => "backup_unreadable",
        })
    }
}

/// Owns the authenticated plaintext and clears its initialized bytes on drop.
/// Accessors borrow exact original bytes; no private data is formatted or
/// serialized by this type. The caller must still enforce disconnected restore
/// admission and a durable whole-pair transaction.
pub struct OpenedBackup {
    plaintext: Zeroizing<Vec<u8>>,
    store_len: usize,
    template_len: usize,
    profiles: usize,
    subscriptions: usize,
}

impl OpenedBackup {
    pub fn store(&self) -> &[u8] {
        &self.plaintext[INNER_HEADER_BYTES..INNER_HEADER_BYTES + self.store_len]
    }

    pub fn template(&self) -> &[u8] {
        &self.plaintext[INNER_HEADER_BYTES + self.store_len
            ..INNER_HEADER_BYTES + self.store_len + self.template_len]
    }

    pub fn profile_count(&self) -> usize {
        self.profiles
    }

    pub fn subscription_count(&self) -> usize {
        self.subscriptions
    }

    /// Construct the restore-specific store before staging or binding any
    /// transaction digest. Portable startup preferences are readable backup
    /// data, never authority to reconnect on the destination machine.
    pub fn restore_store_off(&self) -> Result<Zeroizing<Vec<u8>>, BackupError> {
        let mut document: Value =
            serde_json::from_slice(self.store()).map_err(|_| BackupError::Unreadable)?;
        let enabled = document
            .get_mut("startup")
            .and_then(Value::as_object_mut)
            .and_then(|startup| startup.get_mut("enabled"))
            .ok_or(BackupError::Unreadable)?;
        if enabled == &Value::Bool(false) {
            return Ok(Zeroizing::new(self.store().to_vec()));
        }
        if enabled != &Value::Bool(true) {
            return Err(BackupError::Unreadable);
        }
        *enabled = Value::Bool(false);
        let output =
            Zeroizing::new(serde_json::to_vec(&document).map_err(|_| BackupError::Unreadable)?);
        crate::private_store::backup_candidate::validate(&output)
            .map_err(|_| BackupError::Unreadable)?;
        Ok(output)
    }
}

fn passphrase_allowed(passphrase: &[u8]) -> bool {
    (MIN_PASSPHRASE_BYTES..=MAX_PASSPHRASE_BYTES).contains(&passphrase.len())
}

fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, BackupError> {
    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_ITERATIONS, ARGON2_LANES, Some(32))
        .map_err(|_| BackupError::Unavailable)?;
    // Argon2's allocating convenience method drops an ordinary Vec<Block>.
    // Its zeroize feature alone does not clear that memory-hard workspace.
    let mut workspace = Zeroizing::new(vec![Block::default(); params.block_count()]);
    derive_with_workspace(
        &Argon2::new(Algorithm::Argon2id, Version::V0x13, params),
        passphrase,
        salt,
        &mut workspace,
    )
}

fn derive_with_workspace(
    argon: &Argon2<'_>,
    passphrase: &[u8],
    salt: &[u8],
    workspace: &mut Zeroizing<Vec<Block>>,
) -> Result<Zeroizing<[u8; 32]>, BackupError> {
    let mut key = Zeroizing::new([0_u8; 32]);
    let result = argon.hash_password_into_with_memory(
        passphrase,
        salt,
        key.as_mut(),
        workspace.as_mut_slice(),
    );
    // Clear before returning on either normal result; owning Zeroizing also
    // covers unwinding. It cannot run after SIGKILL, abort or process death.
    for block in workspace.iter_mut() {
        block.zeroize();
    }
    result.map_err(|_| BackupError::Unavailable)?;
    Ok(key)
}

/// Seal only a strict current-schema store and its exact matching bundled
/// template. The passphrase is supplied as private in-memory bytes, never argv
/// or environment data. The caller is responsible for clearing its own copy.
/// Random salt and nonce come from the OS CSPRNG; RNG failure refuses output.
pub fn seal(store: &[u8], template: &[u8], passphrase: &[u8]) -> Result<Vec<u8>, BackupError> {
    if !passphrase_allowed(passphrase) {
        return Err(BackupError::InvalidInput);
    }
    let plaintext = Zeroizing::new(encode(store, template).map_err(|_| BackupError::InvalidInput)?);
    decode(&plaintext)
        .and_then(|framed| framed.validate_bundled_pair())
        .map_err(|_| BackupError::InvalidInput)?;

    let mut salt = [0_u8; SALT_BYTES];
    let mut nonce = [0_u8; NONCE_BYTES];
    getrandom::getrandom(&mut salt).map_err(|_| BackupError::Unavailable)?;
    getrandom::getrandom(&mut nonce).map_err(|_| BackupError::Unavailable)?;
    encrypt_payload(&plaintext, passphrase, &salt, &nonce)
}

fn encrypt_payload(
    plaintext: &[u8],
    passphrase: &[u8],
    salt: &[u8; SALT_BYTES],
    nonce_bytes: &[u8; NONCE_BYTES],
) -> Result<Vec<u8>, BackupError> {
    let mut header = [0_u8; HEADER_BYTES];
    header[..8].copy_from_slice(MAGIC);
    header[8..8 + SALT_BYTES].copy_from_slice(salt);
    header[8 + SALT_BYTES..HEADER_BYTES - 4].copy_from_slice(nonce_bytes);
    let encrypted_len = plaintext.len() + TAG_BYTES;
    header[HEADER_BYTES - 4..].copy_from_slice(
        &u32::try_from(encrypted_len)
            .map_err(|_| BackupError::InvalidInput)?
            .to_be_bytes(),
    );

    let key = derive_key(passphrase, &header[8..8 + SALT_BYTES])?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| BackupError::Unavailable)?;
    let nonce = XNonce::from_slice(&header[8 + SALT_BYTES..HEADER_BYTES - 4]);
    let encrypted = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| BackupError::Unavailable)?;
    let mut output = Vec::with_capacity(HEADER_BYTES + encrypted.len());
    output.extend_from_slice(&header);
    output.extend_from_slice(&encrypted);
    Ok(output)
}

/// Authenticate the entire bounded file before parsing the inner members. A
/// file cannot choose weaker Argon2 parameters; version 01 fixes Argon2id v19,
/// 64 MiB, 3 iterations, one lane and XChaCha20-Poly1305 with header AAD.
pub fn open(input: &[u8], passphrase: &[u8]) -> Result<OpenedBackup, BackupError> {
    if !passphrase_allowed(passphrase)
        || input.len() < HEADER_BYTES + TAG_BYTES + 1
        || input.len() > MAX_BACKUP_BYTES
        || input.get(..8) != Some(MAGIC.as_slice())
    {
        return Err(BackupError::Unreadable);
    }
    let header = &input[..HEADER_BYTES];
    let declared_len = u32::from_be_bytes(
        header[HEADER_BYTES - 4..]
            .try_into()
            .map_err(|_| BackupError::Unreadable)?,
    ) as usize;
    if declared_len != input.len() - HEADER_BYTES || declared_len > MAX_PAYLOAD_BYTES + TAG_BYTES {
        return Err(BackupError::Unreadable);
    }
    let key =
        derive_key(passphrase, &header[8..8 + SALT_BYTES]).map_err(|_| BackupError::Unreadable)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| BackupError::Unreadable)?;
    let nonce = XNonce::from_slice(&header[8 + SALT_BYTES..HEADER_BYTES - 4]);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &input[HEADER_BYTES..],
                    aad: header,
                },
            )
            .map_err(|_| BackupError::Unreadable)?,
    );
    let pair = decode(&plaintext)
        .and_then(|framed| framed.validate_bundled_pair())
        .map_err(|_| BackupError::Unreadable)?;
    let result = OpenedBackup {
        store_len: pair.store.bytes.len(),
        template_len: pair.template.len(),
        profiles: pair.store.profiles,
        subscriptions: pair.store.subscriptions,
        plaintext,
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const PASSPHRASE: &[u8] = b"synthetic passphrase only";
    const STORE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
    const TEMPLATE: &[u8] = include_bytes!("backup_payload_candidate/catalog/v1/default.yaml");

    #[test]
    fn caller_owned_kdf_workspace_is_cleared_on_success_and_failure() {
        let params =
            Params::new(ARGON2_MEMORY_KIB, ARGON2_ITERATIONS, ARGON2_LANES, Some(32)).unwrap();
        let count = params.block_count();
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut workspace = Zeroizing::new(vec![Block::default(); count]);
        for valid in [true, false] {
            // Seed every word: even early refusal must clear prior material.
            for block in workspace.iter_mut() {
                block.as_mut().fill(0xa5a5_a5a5_a5a5_a5a5);
            }
            let salt: &[u8] = if valid { &[7; SALT_BYTES] } else { b"short" };
            let result = derive_with_workspace(&argon, PASSPHRASE, salt, &mut workspace);
            if valid {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.err(), Some(BackupError::Unavailable));
            }
            assert_eq!(workspace.len(), count);
            assert!(
                workspace
                    .iter()
                    .all(|block| block.as_ref().iter().all(|word| *word == 0))
            );
        }
        // Compile-time guarantee for the owning unwind/drop path; do not read
        // freed allocator memory to pretend to test process-death cleanup.
        fn clears_on_drop<T: zeroize::ZeroizeOnDrop>() {}
        clears_on_drop::<Zeroizing<Vec<Block>>>();
    }

    #[test]
    fn independent_libargon2_libsodium_envelope_vector() {
        // Cross-implementation vector generated from libargon2 and libsodium
        // with this exact synthetic pair, fixed salt 00..0f and nonce 00..17.
        // Production seal obtains both from the OS CSPRNG instead.
        let plaintext = Zeroizing::new(encode(STORE, TEMPLATE).unwrap());
        let salt = std::array::from_fn(|index| index as u8);
        let nonce = std::array::from_fn(|index| index as u8);
        let envelope = encrypt_payload(&plaintext, PASSPHRASE, &salt, &nonce).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&envelope)),
            "b4f88a637758e714f35b9b3f77b675df0acbd393ebad765e9c61a55289eb651a"
        );
        let opened = open(&envelope, PASSPHRASE).unwrap();
        assert_eq!(opened.store(), STORE);
        assert_eq!(opened.template(), TEMPLATE);
    }

    #[test]
    fn authenticated_roundtrip_preserves_exact_pair_and_uses_fresh_entropy() {
        let first = seal(STORE, TEMPLATE, PASSPHRASE).unwrap();
        let second = seal(STORE, TEMPLATE, PASSPHRASE).unwrap();
        assert_ne!(first, second);
        assert!(first.len() <= MAX_BACKUP_BYTES);
        assert!(!first.windows(STORE.len()).any(|window| window == STORE));
        let opened = open(&first, PASSPHRASE).unwrap();
        assert_eq!(opened.store(), STORE);
        assert_eq!(opened.template(), TEMPLATE);
        assert_eq!(opened.profile_count(), 0);
        assert_eq!(opened.subscription_count(), 0);
    }

    #[test]
    fn restore_copy_disables_imported_last_and_pinned_startup_before_staging() {
        const PROFILE: &str = "10000000-0000-4000-8000-000000000001";
        for target in ["last", "profile"] {
            let mut store: serde_json::Value = serde_json::from_slice(STORE).unwrap();
            store["profiles"] = serde_json::json!([{
                "id": PROFILE, "name": "Synthetic",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic",
                "protocol": "vless", "favorite": false
            }]);
            store["activeId"] = PROFILE.into();
            store["lastId"] = PROFILE.into();
            store["startup"] = serde_json::json!({
                "enabled": true, "target": target,
                "profileId": if target == "profile" { PROFILE } else { "" },
                "mode": "rule"
            });
            let input = store.to_string();
            crate::private_store::parse_private_store(&input)
                .expect("synthetic fixture must satisfy ordinary store semantics");
            assert!(
                crate::private_store::backup_candidate::validate(input.as_bytes()).is_ok(),
                "synthetic fixture must satisfy strict backup admission"
            );
            let encrypted = seal(input.as_bytes(), TEMPLATE, PASSPHRASE).unwrap();
            let opened = open(&encrypted, PASSPHRASE).unwrap();
            assert_eq!(opened.store(), input.as_bytes());
            let restored = opened.restore_store_off().unwrap();
            let output: serde_json::Value = serde_json::from_slice(&restored).unwrap();
            assert_eq!(output["startup"]["enabled"], false);
            assert_eq!(output["startup"]["target"], target);
            assert_eq!(
                output["startup"]["profileId"],
                store["startup"]["profileId"]
            );
            assert_eq!(output["profiles"], store["profiles"]);
            assert_eq!(output["subscriptions"], store["subscriptions"]);
            assert_eq!(output["lastId"], store["lastId"]);
            assert_eq!(output["activeId"], store["activeId"]);
        }
        let encrypted = seal(STORE, TEMPLATE, PASSPHRASE).unwrap();
        let opened = open(&encrypted, PASSPHRASE).unwrap();
        assert_eq!(opened.restore_store_off().unwrap().as_slice(), STORE);
    }

    #[test]
    fn wrong_passphrase_and_tampering_have_one_public_error() {
        let original = seal(STORE, TEMPLATE, PASSPHRASE).unwrap();
        assert_eq!(
            open(&original, b"incorrect passphrase").err(),
            Some(BackupError::Unreadable)
        );
        for offset in [8, 24, HEADER_BYTES, original.len() - 1] {
            let mut changed = original.clone();
            changed[offset] ^= 1;
            assert_eq!(
                open(&changed, PASSPHRASE).err(),
                Some(BackupError::Unreadable)
            );
        }
        assert_eq!(BackupError::Unreadable.to_string(), "backup_unreadable");
    }

    #[test]
    fn malformed_lengths_versions_and_truncation_refuse_before_plaintext() {
        let original = seal(STORE, TEMPLATE, PASSPHRASE).unwrap();
        for end in [0, 8, HEADER_BYTES - 1, HEADER_BYTES, original.len() - 1] {
            assert_eq!(
                open(&original[..end], PASSPHRASE).err(),
                Some(BackupError::Unreadable)
            );
        }
        let mut changed = original.clone();
        changed[7] ^= 1;
        assert_eq!(
            open(&changed, PASSPHRASE).err(),
            Some(BackupError::Unreadable)
        );
        let mut changed = original.clone();
        changed[HEADER_BYTES - 4..HEADER_BYTES].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            open(&changed, PASSPHRASE).err(),
            Some(BackupError::Unreadable)
        );
        let mut changed = original;
        changed.push(0);
        assert_eq!(
            open(&changed, PASSPHRASE).err(),
            Some(BackupError::Unreadable)
        );
    }

    #[test]
    fn seal_requires_strict_pair_and_bounded_passphrase() {
        assert_eq!(
            seal(b"{}", TEMPLATE, PASSPHRASE),
            Err(BackupError::InvalidInput)
        );
        assert_eq!(
            seal(STORE, b"not template", PASSPHRASE),
            Err(BackupError::InvalidInput)
        );
        assert_eq!(
            seal(STORE, TEMPLATE, b"short"),
            Err(BackupError::InvalidInput)
        );
        assert_eq!(
            seal(STORE, TEMPLATE, &vec![b'x'; MAX_PASSPHRASE_BYTES + 1]),
            Err(BackupError::InvalidInput)
        );
    }
}
