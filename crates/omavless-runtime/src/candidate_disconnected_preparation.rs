// SPDX-License-Identifier: MIT

//! Opt-in pure preparation eligibility for already-v4, disconnected intent.
//! No filesystem, process, host observation, config validation or publication.
//! A returned snapshot is not a host-empty proof, startup/execute permission,
//! core source-build attestation, minimum-version policy or provider acceptance.

use crate::desired::{DesiredState, RoutingMode};
use omavless_domain::private_store::{CandidatePrivateStore, CandidateRuntimeConfig};
use omavless_profile::wireguard::{AwgGeneration, WireGuardFlavor};
use sha2::{Digest, Sha256};
use std::fmt;

const MAX_CORE_BYTES: usize = 128 * 1024 * 1024;
// Exact x86_64 Mihomo Meta 1.19.31 artifact used by #548/#552 transport rows.
// An observed version string or a newer binary never substitutes for its bytes.
const REVIEWED_CORE_SHA256: [u8; 32] = [
    0xba, 0x7a, 0x74, 0xed, 0x6b, 0xbc, 0x30, 0x98, 0x93, 0x0e, 0x6e, 0x17, 0x2f, 0xa9, 0xb7, 0xd4,
    0xef, 0x7e, 0x42, 0xa7, 0x7f, 0x7a, 0x51, 0x60, 0x46, 0xd6, 0x1f, 0xbe, 0xb3, 0xbf, 0xcf, 0xe6,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationError {
    InvalidOffIntent,
    ActiveCandidate,
    ProfileUnavailable,
    UnsupportedCore,
    UnsupportedFlavor,
    InvalidConfiguration,
}

impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidOffIntent => "Disconnected candidate intent is invalid",
            Self::ActiveCandidate => "Active candidate preparation is unavailable",
            Self::ProfileUnavailable => "Candidate profile is unavailable",
            Self::UnsupportedCore => "Candidate core artifact is unsupported",
            Self::UnsupportedFlavor => "Candidate flavor is not reviewed for this core",
            Self::InvalidConfiguration => "Candidate configuration is invalid",
        })
    }
}
impl std::error::Error for PreparationError {}

/// Owned private inputs for a future separately reviewed validator only.
/// No Clone/Serialize/Display, execution method, path or lifecycle capability.
pub struct CandidateDisconnectedSnapshot {
    config: CandidateRuntimeConfig,
    core: Vec<u8>,
    intent_generation: u64,
    mode: RoutingMode,
}

impl CandidateDisconnectedSnapshot {
    pub fn expose_private_config(&self) -> &[u8] {
        self.config.expose_private_bytes()
    }
    pub fn private_readiness_name(&self) -> &str {
        self.config.private_name()
    }
    pub fn expose_captured_core_bytes(&self) -> &[u8] {
        &self.core
    }
    #[must_use]
    pub fn intent_generation(&self) -> u64 {
        self.intent_generation
    }
    #[must_use]
    pub fn mode(&self) -> RoutingMode {
        self.mode
    }
}

impl fmt::Debug for CandidateDisconnectedSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CandidateDisconnectedSnapshot([REDACTED])")
    }
}

fn validate_intent(
    candidate: &CandidatePrivateStore,
    desired: &DesiredState,
) -> Result<(), PreparationError> {
    desired
        .validate()
        .map_err(|_| PreparationError::InvalidOffIntent)?;
    if desired.connected {
        return Err(PreparationError::InvalidOffIntent);
    }
    if candidate.pointer_presence().0 {
        return Err(PreparationError::ActiveCandidate);
    }
    Ok(())
}

fn compatible_flavor(
    digest: [u8; 32],
    flavor: Option<WireGuardFlavor>,
) -> Result<(), PreparationError> {
    if digest != REVIEWED_CORE_SHA256 {
        return Err(PreparationError::UnsupportedCore);
    }
    match flavor {
        // URI render parity is retained, not a V0/provider maturity grant.
        None
        | Some(WireGuardFlavor::Standard)
        | Some(WireGuardFlavor::Amnezia(AwgGeneration::V3 | AwgGeneration::V3_1)) => Ok(()),
        Some(WireGuardFlavor::Amnezia(AwgGeneration::V1 | AwgGeneration::V2)) => {
            Err(PreparationError::UnsupportedFlavor)
        }
    }
}

fn static_x86_elf(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || &bytes[..7] != b"\x7fELF\x02\x01\x01" {
        return false;
    }
    let kind = u16::from_le_bytes(bytes[16..18].try_into().unwrap());
    let machine = u16::from_le_bytes(bytes[18..20].try_into().unwrap());
    let version = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    let header = u16::from_le_bytes(bytes[52..54].try_into().unwrap());
    let size = u16::from_le_bytes(bytes[54..56].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as usize;
    let offset = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let Ok(offset) = usize::try_from(offset) else {
        return false;
    };
    if !matches!(kind, 2 | 3)
        || machine != 62
        || version != 1
        || header != 64
        || size != 56
        || count == 0
        || count > 128
        || offset < 64
    {
        return false;
    }
    let Some(end) = offset.checked_add(size * count) else {
        return false;
    };
    if end > bytes.len() {
        return false;
    }
    bytes[offset..end].chunks_exact(size).all(|entry| {
        let kind = u32::from_le_bytes(entry[..4].try_into().unwrap());
        let start = u64::from_le_bytes(entry[8..16].try_into().unwrap());
        let length = u64::from_le_bytes(entry[32..40].try_into().unwrap());
        // No interpreter/dynamic dependency graph is admitted by this row.
        kind != 2
            && kind != 3
            && start
                .checked_add(length)
                .is_some_and(|end| end <= bytes.len() as u64)
    })
}

fn render_snapshot(
    candidate: &CandidatePrivateStore,
    desired: &DesiredState,
    profile_id: &str,
    template: &str,
    controller: &str,
) -> Result<CandidateRuntimeConfig, PreparationError> {
    candidate
        .prepare_private_config_mode(profile_id, template, controller, desired.mode.as_str())
        .map_err(|_| PreparationError::InvalidConfiguration)
}

/// Capture exact caller-supplied core bytes, never trust a supplied core label,
/// digest, version or flavor claim. Inputs must already have come from the
/// caller's separately reviewed acquisition/owner snapshot; this pure operation
/// establishes no filesystem identity, live host emptiness or future authority.
pub fn prepare_disconnected_candidate(
    candidate: &CandidatePrivateStore,
    desired: &DesiredState,
    profile_id: &str,
    template: &str,
    controller: &str,
    core_bytes: &[u8],
) -> Result<CandidateDisconnectedSnapshot, PreparationError> {
    validate_intent(candidate, desired)?;
    let flavor = candidate
        .selected_wireguard_flavor(profile_id)
        .map_err(|_| PreparationError::ProfileUnavailable)?;
    if core_bytes.is_empty() || core_bytes.len() > MAX_CORE_BYTES || !static_x86_elf(core_bytes) {
        return Err(PreparationError::UnsupportedCore);
    }
    compatible_flavor(Sha256::digest(core_bytes).into(), flavor)?;
    // Eligibility/refusal precedes even pure rendering; no quiesce or host API
    // is reached. Resources and actual config validation remain separate gates.
    let config = render_snapshot(candidate, desired, profile_id, template, controller)?;
    Ok(CandidateDisconnectedSnapshot {
        config,
        core: core_bytes.to_vec(),
        intent_generation: desired.generation,
        mode: desired.mode,
    })
}

#[cfg(test)]
mod tests;
