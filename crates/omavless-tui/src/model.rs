// SPDX-License-Identifier: MIT
//! Private display metadata: deliberately no Debug/Serialize implementation.
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    Unavailable,
    Incompatible,
    Invalid,
    Changed,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Actual {
    Disconnected,
    Starting,
    Connected,
    Reconnecting,
    Stopping,
    Failed,
    ManualRecoveryRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Rule,
    Global,
    Direct,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Desired {
    pub connected: bool,
    #[serde(default)]
    pub profile_id: String,
    pub mode: Mode,
    pub generation: u64,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub subscription_id: Option<String>,
    pub missing: bool,
    pub favorite: bool,
}

#[derive(Clone, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub schema_version: u32,
    pub scope: String,
    pub instance_id: String,
    pub desired: Desired,
    pub last_known_actual: Actual,
    pub health_fresh: bool,
    pub live_health: String,
    pub profiles: Vec<Profile>,
    pub subscriptions: Vec<Subscription>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub owned_core_running: bool,
    pub visible_mihomo_count: u32,
    pub owned_auxiliary_mihomo_count: u32,
    pub visible_tun_count: u32,
    pub managed_tun_count: u32,
    pub owned_controller_config_verified: bool,
    pub desired_profile_matches_owned: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub schema_version: u32,
    pub scope: String,
    pub instance_id: String,
    pub availability: String,
    pub desired: Desired,
    pub last_known_actual: Actual,
    pub manual_recovery_required: bool,
    pub facts: Option<Facts>,
}

#[derive(Clone)]
pub struct Snapshot {
    pub traffic: Option<crate::inspection::Traffic>,
    pub diagnostics: Option<crate::inspection::Diagnostics>,
    pub inspection_available: (bool, bool),
    pub actions_available: bool,
    pub revision: u64,
    pub metadata: Metadata,
    pub observation: Observation,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Connected,
    Disconnected,
    Recovery,
    Unverified,
}

pub fn opaque(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|c| (33..=126).contains(&c))
}

fn name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 320 && value.chars().count() <= 80
}

/// Strip terminal escape/control and directional override characters before
/// any dynamic value reaches the renderer. Never interpret provider markup.
pub fn display(value: &str, limit: usize) -> String {
    value.chars().take(limit).map(|c| {
        if c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{061c}') {
            '\u{fffd}'
        } else { c }
    }).collect()
}

impl Snapshot {
    pub fn parse(meta: &Value, observed: &Value, instance: &str) -> Result<Self, ReadError> {
        let revision = meta["revision"]
            .as_u64()
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or(ReadError::Invalid)?;
        if observed["revision"].as_u64() != Some(revision) {
            return Err(ReadError::Changed);
        }
        let metadata: Metadata =
            serde_json::from_value(meta["result"].clone()).map_err(|_| ReadError::Invalid)?;
        let observation: Observation =
            serde_json::from_value(observed["result"].clone()).map_err(|_| ReadError::Invalid)?;
        if metadata.schema_version != 1
            || metadata.scope != "private_ui_metadata"
            || observation.schema_version != 1
            || observation.scope != "local_runtime_observation"
            || !opaque(&metadata.instance_id)
            || metadata.health_fresh
            || metadata.live_health != "unavailable"
            || metadata.profiles.len() > 256
            || metadata.subscriptions.len() > 64
            || meta["result"].get("transition") != Some(&Value::Null)
            || observed["result"].get("transition") != Some(&Value::Null)
            || metadata.desired.generation > i64::MAX as u64
            || (metadata.desired.connected
                && (!opaque(&metadata.desired.profile_id)
                    || metadata.desired.profile_id.len() > 64))
            || (!metadata.desired.connected && !metadata.desired.profile_id.is_empty())
        {
            return Err(ReadError::Invalid);
        }
        if metadata.instance_id != instance
            || observation.instance_id != instance
            || metadata.desired.connected != observation.desired.connected
            || metadata.desired.generation != observation.desired.generation
            || metadata.desired.mode != observation.desired.mode
            || metadata.last_known_actual != observation.last_known_actual
        {
            return Err(ReadError::Changed);
        }
        let mut ids = HashSet::new();
        let mut subscriptions = HashSet::new();
        for sub in &metadata.subscriptions {
            if !opaque(&sub.id)
                || sub.id.len() > 64
                || !name(&sub.name)
                || !subscriptions.insert(sub.id.as_str())
            {
                return Err(ReadError::Invalid);
            }
        }
        for profile in &metadata.profiles {
            if !opaque(&profile.id)
                || profile.id.len() > 64
                || !name(&profile.name)
                || !ids.insert(profile.id.as_str())
                || profile
                    .subscription_id
                    .as_deref()
                    .is_some_and(|s| !s.is_empty() && !subscriptions.contains(s))
            {
                return Err(ReadError::Invalid);
            }
        }
        if observation.manual_recovery_required
            != (observation.last_known_actual == Actual::ManualRecoveryRequired)
        {
            return Err(ReadError::Invalid);
        }
        match (observation.availability.as_str(), &observation.facts) {
            ("unavailable", None) => {}
            ("observed", Some(f))
                if f.visible_mihomo_count <= 64
                    && f.visible_tun_count <= 8
                    && (!f.owned_core_running || f.visible_mihomo_count >= 1)
                    && f.managed_tun_count <= f.visible_tun_count
                    && f.owned_auxiliary_mihomo_count <= f.visible_mihomo_count.min(1)
                    && (!f.desired_profile_matches_owned
                        || (f.owned_core_running && metadata.desired.connected))
                    && (!f.owned_controller_config_verified || f.desired_profile_matches_owned) => {
            }
            _ => return Err(ReadError::Invalid),
        }
        Ok(Self {
            traffic: None,
            diagnostics: None,
            inspection_available: (false, false),
            actions_available: false,
            revision,
            metadata,
            observation,
        })
    }

    pub fn status(&self) -> Status {
        if self.observation.manual_recovery_required {
            return Status::Recovery;
        }
        let Some(f) = &self.observation.facts else {
            return Status::Unverified;
        };
        match self.metadata.last_known_actual {
            Actual::Connected
                if self.metadata.desired.connected
                    && self
                        .metadata
                        .profiles
                        .iter()
                        .any(|p| p.id == self.metadata.desired.profile_id)
                    && f.owned_core_running
                    && f.owned_controller_config_verified
                    && f.desired_profile_matches_owned
                    && f.managed_tun_count == 1 =>
            {
                Status::Connected
            }
            Actual::Disconnected
                if !self.metadata.desired.connected
                    && !f.owned_core_running
                    && f.managed_tun_count == 0
                    && f.owned_auxiliary_mihomo_count == 0 =>
            {
                Status::Disconnected
            }
            _ => Status::Unverified,
        }
    }
}
