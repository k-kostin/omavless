// SPDX-License-Identifier: MIT
//! Feature-only disconnected capture. Never invokes the validator or a host.

use super::{ValidationError, capture_resources, manifest};
use crate::candidate_disconnected_preparation::{
    CandidateDisconnectedSnapshot, PreparationError, prepare_disconnected_candidate,
};
use crate::desired::{DesiredState, RoutingMode};
use omavless_domain::private_store::CandidatePrivateStore;
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateCaptureError {
    Preparation(PreparationError),
    Resources(ValidationError),
}

impl fmt::Display for CandidateCaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Preparation(_) => "Candidate snapshot preparation refused",
            Self::Resources(_) => "Candidate snapshot resources unavailable",
        })
    }
}
impl std::error::Error for CandidateCaptureError {}

/// Immutable private input closure, not a validation or execution capability.
/// No Clone/Serialize/Display, path, execution conversion or host method.
pub struct CandidateValidationSnapshot {
    prepared: CandidateDisconnectedSnapshot,
    template: String,
    resources: Vec<(String, Vec<u8>)>,
}

impl CandidateValidationSnapshot {
    /// The same raw inputs produce config and resource membership together.
    /// An arbitrary prerendered config or caller manifest is not an input.
    /// Core/off/flavor eligibility and rendering finish before resource reads.
    pub fn capture(
        data: &Path,
        uid: u32,
        desired: &DesiredState,
        candidate: &CandidatePrivateStore,
        profile_id: &str,
        template: &str,
        core_bytes: &[u8],
    ) -> Result<Self, CandidateCaptureError> {
        let names = manifest(template).map_err(CandidateCaptureError::Resources)?;
        let prepared = prepare_disconnected_candidate(
            candidate,
            desired,
            profile_id,
            template,
            "/work/mihomo.sock",
            core_bytes,
        )
        .map_err(CandidateCaptureError::Preparation)?;
        let resources =
            capture_resources(data, uid, names).map_err(CandidateCaptureError::Resources)?;
        Ok(Self {
            prepared,
            template: template.to_owned(),
            resources,
        })
    }

    /// Deliberate private input access only; never a public report payload.
    pub fn expose_private_config(&self) -> &[u8] {
        self.prepared.expose_private_config()
    }
    pub fn expose_captured_core_bytes(&self) -> &[u8] {
        self.prepared.expose_captured_core_bytes()
    }
    pub fn expose_captured_template(&self) -> &str {
        &self.template
    }
    pub fn expose_private_resources(&self) -> &[(String, Vec<u8>)] {
        &self.resources
    }
    #[must_use]
    pub fn intent_generation(&self) -> u64 {
        self.prepared.intent_generation()
    }
    #[must_use]
    pub fn mode(&self) -> RoutingMode {
        self.prepared.mode()
    }
}

impl fmt::Debug for CandidateValidationSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CandidateValidationSnapshot([REDACTED])")
    }
}

#[cfg(test)]
mod tests;
