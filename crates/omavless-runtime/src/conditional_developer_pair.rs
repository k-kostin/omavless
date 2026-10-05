// SPDX-License-Identifier: MIT
//! Fixed x86_64 developer pair, not released package attestation.
//! Trusts administrator-provisioned exact objects; never installs or repairs.

use super::super::{ExecutableEvidence, FileIdentity, Session};
use super::{Objects, Refusal, Result, elf_architecture, member, read_receipt};
use serde::Deserialize;
use std::cell::Cell;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(in crate::conditional_close_candidate) const DIRECTORY: &str =
    "/var/lib/omavless-close-development-pair";
const BROKER: &str = "omavless-dns-broker";
const CORE_HASH: &str = "3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544";
const BROKER_HASH: &str = "ea958302d745b901294df6164c624a431a7493b67457a255306ec8216545eb9d";
const CORE_SOURCE: &str = "8c038e76c8407eebd7afdd6e0389fc2bbc28cab9";
const BROKER_SOURCE: &str = "aff0c38075338d51d979acc9f10dab1ae6dbba6f";
const COMPOSITION_SOURCE: &str = "12b0253564f25f918af18a0c0ddc6f2e2231b2db";
const CAPSULE: &str = "06507e5cc4777c4a6b88be1b16e37d623663eeaf2b8600a0ee72a38af6d3271d";
const WHOLE_CAPTURE: &str = "f174aedb11ddef42f2a12d96bf872ecb93d218835e43e3a611f6588e232cab23";
const BUDGET: Duration = Duration::from_secs(3);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    architecture: String,
    abi: u8,
    core_source: String,
    broker_source: String,
    composition_source: String,
    capsule_sha256: String,
    whole_capture_sha256: String,
    core_sha256: String,
    broker_sha256: String,
    production_adoption: bool,
}

fn decode(raw: &[u8], architecture: &str) -> Result<()> {
    if raw.is_empty() || raw.len() > super::MAX_RECEIPT as usize {
        return Err(Refusal::Receipt);
    }
    let r: Receipt = serde_json::from_slice(raw).map_err(|_| Refusal::Receipt)?;
    if architecture != "x86_64"
        || r.architecture != architecture
        || r.schema != "omavless-developer-conditional-pair-v1"
        || r.abi != 1
        || r.core_source != CORE_SOURCE
        || r.broker_source != BROKER_SOURCE
        || r.composition_source != COMPOSITION_SOURCE
        || r.capsule_sha256 != CAPSULE
        || r.whole_capture_sha256 != WHOLE_CAPTURE
        || r.core_sha256 != CORE_HASH
        || r.broker_sha256 != BROKER_HASH
        || r.production_adoption
    {
        return Err(Refusal::Receipt);
    }
    Ok(())
}

fn hash_text(hash: [u8; 32]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in hash {
        write!(&mut text, "{byte:02x}").expect("String formatting is infallible");
    }
    text
}

/// Bound to one original Session; no Clone, Debug, wire constructor or grant.
/// Any observed drift permanently refuses this evidence even after restoration.
pub(in crate::conditional_close_candidate) struct Evidence {
    objects: Objects,
    broker: File,
    broker_identity: FileIdentity,
    session: OriginalSession,
}

struct OriginalSession {
    identity: Arc<()>,
    refused: Cell<bool>,
}
impl OriginalSession {
    fn new(identity: &Arc<()>) -> Self {
        Self {
            identity: Arc::clone(identity),
            refused: Cell::new(false),
        }
    }
    fn matches(&self, identity: &Arc<()>) -> bool {
        !self.refused.get() && Arc::ptr_eq(identity, &self.identity)
    }
    fn refuse(&self) {
        self.refused.set(true);
    }
}

impl Evidence {
    /// No filesystem work: usable while holding the lifetime gate.
    pub(in crate::conditional_close_candidate) fn belongs_to(&self, session: &Arc<()>) -> bool {
        self.session.matches(session)
    }
    pub(in crate::conditional_close_candidate) fn capture(session: &mut Session) -> Result<Self> {
        session.check().map_err(|_| Refusal::Child)?;
        let until = Instant::now() + BUDGET;
        let mut objects = Objects::open_at(Path::new(DIRECTORY))?;
        let raw = read_receipt(&mut objects.receipt, objects.receipt_identity)?;
        decode(&raw, std::env::consts::ARCH)?;
        let parent = &objects.directories.last().ok_or(Refusal::Object)?.file;
        let (mut broker, broker_identity) = member(parent, BROKER, 0o755, 32 * 1024 * 1024)?;
        elf_architecture(&mut objects.core, "x86_64")?;
        elf_architecture(&mut broker, "x86_64")?;
        for (file, identity, expected) in [
            (&mut objects.core, objects.core_identity, CORE_HASH),
            (&mut broker, broker_identity, BROKER_HASH),
        ] {
            if ExecutableEvidence::hash(file, identity, until)
                .map(hash_text)
                .as_deref()
                != Some(expected)
            {
                return Err(Refusal::Object);
            }
        }
        let evidence = Self {
            objects,
            broker,
            broker_identity,
            session: OriginalSession::new(&session.identity),
        };
        evidence.check(
            &session.identity,
            session.executable.as_ref().ok_or(Refusal::Child)?,
            session.binding.pid,
        )?;
        session.check().map_err(|_| Refusal::Child)?;
        if Instant::now() >= until {
            return Err(Refusal::Expired);
        }
        Ok(evidence)
    }

    pub(in crate::conditional_close_candidate) fn check(
        &self,
        session: &Arc<()>,
        image: &ExecutableEvidence,
        pid: u32,
    ) -> Result<()> {
        let result = self.check_inner(session, image, pid);
        if result.is_err() {
            self.session.refuse();
        }
        result
    }

    fn check_inner(&self, session: &Arc<()>, image: &ExecutableEvidence, pid: u32) -> Result<()> {
        if !self.belongs_to(session) {
            return Err(Refusal::Child);
        }
        self.objects.recheck()?;
        let parent = &self.objects.directories.last().ok_or(Refusal::Object)?.file;
        let (_, named_broker) = member(parent, BROKER, 0o755, 32 * 1024 * 1024)?;
        if named_broker != self.broker_identity || !self.broker_identity.matches(&self.broker) {
            return Err(Refusal::Object);
        }
        if image.image_identity != self.objects.core_identity
            || image.source_identity != self.objects.core_identity
            || image.source_path != Path::new(DIRECTORY).join("mihomo")
            || !self.objects.core_identity.matches(&image.image)
            || !self.objects.core_identity.matches(&image.source)
            || !image.check(pid)
        {
            return Err(Refusal::Child);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> serde_json::Value {
        serde_json::json!({
            "schema":"omavless-developer-conditional-pair-v1", "architecture":"x86_64",
            "abi":1, "core_source":CORE_SOURCE, "broker_source":BROKER_SOURCE,
            "composition_source":COMPOSITION_SOURCE, "capsule_sha256":CAPSULE,
            "whole_capture_sha256":WHOLE_CAPTURE, "core_sha256":CORE_HASH,
            "broker_sha256":BROKER_HASH, "production_adoption":false
        })
    }
    #[test]
    fn original_session_cannot_be_rebound_or_revived_after_refusal() {
        let original = Arc::new(());
        let foreign = Arc::new(());
        let binding = OriginalSession::new(&original);
        assert!(binding.matches(&Arc::clone(&original)));
        assert!(!binding.matches(&foreign));
        binding.refuse();
        assert!(!binding.matches(&original));
        assert!(!binding.matches(&foreign));
        binding.refuse();
        assert!(!binding.matches(&original));
    }
    #[test]
    fn developer_receipt_is_exact_and_never_an_arm_or_product_grant() {
        let original = receipt();
        let bytes = serde_json::to_vec(&original).unwrap();
        assert!(decode(&bytes, "x86_64").is_ok());
        assert!(decode(&bytes, "aarch64").is_err());
        for key in [
            "schema",
            "architecture",
            "core_source",
            "broker_source",
            "composition_source",
            "capsule_sha256",
            "whole_capture_sha256",
            "core_sha256",
            "broker_sha256",
        ] {
            let mut changed = original.clone();
            changed[key] = "foreign".into();
            assert!(decode(&serde_json::to_vec(&changed).unwrap(), "x86_64").is_err());
        }
        for (key, value) in [
            ("abi", serde_json::json!(2)),
            ("production_adoption", serde_json::json!(true)),
            ("command", serde_json::json!("grant")),
        ] {
            let mut changed = original.clone();
            changed[key] = value;
            assert!(decode(&serde_json::to_vec(&changed).unwrap(), "x86_64").is_err());
        }
        let duplicate = String::from_utf8(bytes)
            .unwrap()
            .replacen('{', "{\"abi\":1,", 1);
        assert!(decode(duplicate.as_bytes(), "x86_64").is_err());
        assert!(
            decode(
                &vec![b' '; super::super::MAX_RECEIPT as usize + 1],
                "x86_64"
            )
            .is_err()
        );
    }
}
