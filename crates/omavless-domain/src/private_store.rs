// SPDX-License-Identifier: MIT

//! Read-only validation and projection of the current private Python store.
//!
//! Values in this module include reusable credentials. They deliberately do
//! not implement `Debug` or serialization. Callers may publish only
//! [`StoreProjection`], which contains counts and booleans.

use crate::config::{ConfigError, MAX_TEMPLATE_BYTES, assemble_runtime_config};
use crate::import::valid_subscription_url;
use crate::routing::{CustomRule, RoutingError, template_with_mode};
use crate::store::{
    CURRENT_STORE_VERSION, MAX_PROFILES, MAX_SUBSCRIPTIONS, ProfileState, StartupState, StoreError,
    StoreStateInput, SubscriptionState, normalize_store_state, valid_record_id,
};
use omavless_profile::Protocol;
use omavless_profile::canonical::{CanonicalError, CanonicalProfile, parse_canonical};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;

const MAX_NAME_CHARS: usize = 80;
pub const MAX_PRIVATE_STORE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateStoreError {
    InvalidJson,
    TooLarge,
    InvalidShape,
    InvalidName,
    InvalidSubscriptionUrl,
    DuplicateSubscriptionUrl,
    InvalidTimestamp,
    ProtocolMismatch,
    Store(StoreError),
    Profile(CanonicalError),
    Routing(RoutingError),
    Config(ConfigError),
    ProfileNotFound,
    SubscribedProfile,
    DuplicateProfileName,
}

impl PrivateStoreError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::TooLarge => "too_large",
            Self::InvalidShape => "invalid_shape",
            Self::InvalidName => "invalid_name",
            Self::InvalidSubscriptionUrl => "invalid_subscription_url",
            Self::DuplicateSubscriptionUrl => "duplicate_subscription_url",
            Self::InvalidTimestamp => "invalid_timestamp",
            Self::ProtocolMismatch => "protocol_mismatch",
            Self::Store(error) => error.code(),
            Self::Profile(error) => error.code(),
            Self::Routing(error) => error.code(),
            Self::Config(_) => "invalid_runtime_config",
            Self::ProfileNotFound => "profile_not_found",
            Self::SubscribedProfile => "subscribed_profile",
            Self::DuplicateProfileName => "duplicate_profile_name",
        }
    }
}

impl fmt::Display for PrivateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJson => "Private profile store is not valid JSON",
            Self::TooLarge => "Private profile store is too large",
            Self::InvalidShape => "Private profile store has an invalid format",
            Self::InvalidName => "Private profile store has an invalid record name",
            Self::InvalidSubscriptionUrl => "Private profile store has an invalid subscription URL",
            Self::DuplicateSubscriptionUrl => {
                "Private profile store has a duplicate subscription URL"
            }
            Self::InvalidTimestamp => "Private profile store has an invalid timestamp",
            Self::ProtocolMismatch => "Stored profile protocol does not match its link",
            Self::Store(error) => return error.fmt(formatter),
            Self::Profile(error) => return error.fmt(formatter),
            Self::Routing(error) => return error.fmt(formatter),
            Self::Config(error) => return error.fmt(formatter),
            Self::ProfileNotFound => "Requested profile record was not found",
            Self::SubscribedProfile => "Subscribed profile is managed by its provider",
            Self::DuplicateProfileName => "A profile with this name already exists",
        })
    }
}

impl std::error::Error for PrivateStoreError {}

struct PrivateProfile {
    id: String,
    name: String,
    canonical: CanonicalProfile,
    subscription_id: String,
    subscription_key: String,
    missing: bool,
    favorite: bool,
}

struct PrivateSubscription {
    name: String,
    url: String,
    updated_at: u64,
}

/// Credential-free facts which may safely cross the future control boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreProjection {
    pub version: u8,
    pub profile_count: usize,
    pub subscription_count: usize,
    pub vless_count: usize,
    pub trojan_count: usize,
    pub hysteria2_count: usize,
    pub tuic_count: usize,
    pub active_present: bool,
    pub last_present: bool,
    pub routing_preset: String,
    pub custom_rule_count: usize,
    pub startup_configured: bool,
    pub onboarding_complete: bool,
}

/// One exact profile-store mutation. It deliberately has no formatting or
/// serialization implementation because rename input and record IDs are
/// private user data.
pub enum ProfileMutation {
    Rename {
        profile_id: String,
        new_name: String,
    },
    Favorite {
        profile_id: String,
        enabled: bool,
    },
    Delete {
        profile_id: String,
    },
}

/// Successfully validated private replacement payload plus one public fact.
/// The payload is released only to the fixed atomic store writer.
pub struct PrivateStoreMutation {
    payload: Vec<u8>,
    pub changed: bool,
}

impl PrivateStoreMutation {
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Validated private store. Never derive `Debug`, `Clone`, or serialization.
pub struct PrivateStore {
    document: Value,
    version: u8,
    profiles: Vec<PrivateProfile>,
    subscriptions: Vec<PrivateSubscription>,
    active_id: String,
    last_id: String,
    routing_preset: String,
    custom_rules: Vec<CustomRule>,
    rules_updated_at: u64,
    startup: StartupState,
    startup_configured: bool,
    onboarding_complete: bool,
}

fn object(value: &Value) -> Result<&Map<String, Value>, PrivateStoreError> {
    value.as_object().ok_or(PrivateStoreError::InvalidShape)
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], PrivateStoreError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(PrivateStoreError::InvalidShape)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, PrivateStoreError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(PrivateStoreError::InvalidShape)
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    default: &'a str,
) -> Result<&'a str, PrivateStoreError> {
    object
        .get(key)
        .map(|value| value.as_str().ok_or(PrivateStoreError::InvalidShape))
        .unwrap_or(Ok(default))
}

fn optional_bool(
    object: &Map<String, Value>,
    key: &str,
    default: bool,
) -> Result<bool, PrivateStoreError> {
    object
        .get(key)
        .map(|value| value.as_bool().ok_or(PrivateStoreError::InvalidShape))
        .unwrap_or(Ok(default))
}

fn optional_u64(
    object: &Map<String, Value>,
    key: &str,
    default: u64,
) -> Result<u64, PrivateStoreError> {
    object
        .get(key)
        .map(|value| value.as_u64().ok_or(PrivateStoreError::InvalidTimestamp))
        .unwrap_or(Ok(default))
}

fn canonical_name(value: &str) -> bool {
    let cleaned = value
        .chars()
        .filter(|character| !matches!(*character as u32, 0..=31 | 127))
        .collect::<String>();
    let cleaned = cleaned.trim();
    !cleaned.is_empty() && cleaned.chars().count() <= MAX_NAME_CHARS && cleaned == value
}

fn clean_mutation_name(value: &str) -> Result<String, PrivateStoreError> {
    let cleaned = value
        .chars()
        .filter(|character| !matches!(*character as u32, 0..=31 | 127))
        .collect::<String>();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned.chars().count() > MAX_NAME_CHARS {
        return Err(PrivateStoreError::InvalidName);
    }
    Ok(cleaned.to_owned())
}

fn public_yaml_scalar<'a>(line: &'a str, key: &str, allowed: &[&str]) -> Option<&'a str> {
    let (candidate, value) = line.split_once(':')?;
    if candidate.trim() != key {
        return None;
    }
    let value = value.trim();
    let value = match value.as_bytes() {
        [b'\'', .., b'\''] | [b'"', .., b'"'] if value.len() >= 2 => &value[1..value.len() - 1],
        [b'\'', ..] | [b'"', ..] | [.., b'\''] | [.., b'"'] => return None,
        _ => value,
    };
    allowed.contains(&value).then_some(value)
}

fn equivalent_legacy_config(expected: &str, actual: &str) -> bool {
    if expected == actual {
        return true;
    }
    let expected = expected.lines().collect::<Vec<_>>();
    let actual = actual.lines().collect::<Vec<_>>();
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(left, right)| {
            left == &right
                || [
                    ("type", &["vless", "trojan", "hysteria2", "tuic"] as &[&str]),
                    (
                        "network",
                        &["tcp", "ws", "grpc", "h2", "http", "xhttp"] as &[&str],
                    ),
                ]
                .into_iter()
                .any(|(key, allowed)| {
                    public_yaml_scalar(left, key, allowed)
                        .zip(public_yaml_scalar(right, key, allowed))
                        .is_some_and(|(left, right)| left == right)
                })
        })
}

fn protocol(value: &str) -> Option<Protocol> {
    match value {
        "vless" => Some(Protocol::Vless),
        "trojan" => Some(Protocol::Trojan),
        "hysteria2" => Some(Protocol::Hysteria2),
        "tuic" => Some(Protocol::Tuic),
        _ => None,
    }
}

/// Parse and fully validate one existing v1-v3 store without mutating it.
pub fn parse_private_store(input: &str) -> Result<PrivateStore, PrivateStoreError> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(PrivateStoreError::TooLarge);
    }
    let value: Value = serde_json::from_str(input).map_err(|_| PrivateStoreError::InvalidJson)?;
    let root = object(&value)?;
    let version = root
        .get("version")
        .map(|value| value.as_u64().ok_or(PrivateStoreError::InvalidShape))
        .unwrap_or(Ok(1))?;
    let version = u8::try_from(version).map_err(|_| PrivateStoreError::InvalidShape)?;
    if !(1..=CURRENT_STORE_VERSION).contains(&version) {
        return Err(PrivateStoreError::Store(StoreError::UnsupportedVersion));
    }

    let subscription_values = root
        .get("subscriptions")
        .map(|value| {
            value
                .as_array()
                .map(Vec::as_slice)
                .ok_or(PrivateStoreError::InvalidShape)
        })
        .unwrap_or(Ok(&[]))?;
    if subscription_values.len() > MAX_SUBSCRIPTIONS {
        return Err(PrivateStoreError::Store(StoreError::TooManySubscriptions));
    }
    let mut subscription_states = Vec::with_capacity(subscription_values.len());
    let mut subscriptions = Vec::with_capacity(subscription_values.len());
    let mut subscription_urls = BTreeSet::new();
    for value in subscription_values {
        let record = object(value)?;
        let id = required_string(record, "id")?;
        let name = required_string(record, "name")?;
        let url = required_string(record, "url")?;
        if !canonical_name(name) {
            return Err(PrivateStoreError::InvalidName);
        }
        if !valid_subscription_url(url) {
            return Err(PrivateStoreError::InvalidSubscriptionUrl);
        }
        if !subscription_urls.insert(url) {
            return Err(PrivateStoreError::DuplicateSubscriptionUrl);
        }
        let updated_at = optional_u64(record, "updatedAt", 0)?;
        subscription_states.push(SubscriptionState { id: id.to_owned() });
        subscriptions.push(PrivateSubscription {
            name: name.to_owned(),
            url: url.to_owned(),
            updated_at,
        });
    }

    let profile_values = array(root, "profiles")?;
    if profile_values.len() > MAX_PROFILES {
        return Err(PrivateStoreError::Store(StoreError::TooManyProfiles));
    }
    let mut profile_states = Vec::with_capacity(profile_values.len());
    let mut profiles = Vec::with_capacity(profile_values.len());
    for value in profile_values {
        let record = object(value)?;
        let id = required_string(record, "id")?;
        let name = required_string(record, "name")?;
        let uri = required_string(record, "uri")?;
        if !canonical_name(name) {
            return Err(PrivateStoreError::InvalidName);
        }
        let canonical = parse_canonical(uri).map_err(PrivateStoreError::Profile)?;
        let stored_protocol = match record.get("protocol") {
            Some(value) => value.as_str().ok_or(PrivateStoreError::InvalidShape)?,
            None if version < 3 => canonical.protocol().as_str(),
            None => return Err(PrivateStoreError::InvalidShape),
        };
        if protocol(stored_protocol) != Some(canonical.protocol()) {
            return Err(PrivateStoreError::ProtocolMismatch);
        }
        let subscription_id = optional_string(record, "subscriptionId", "")?;
        let subscription_key = optional_string(record, "subscriptionKey", "")?;
        let missing = optional_bool(record, "missing", false)?;
        let favorite = optional_bool(record, "favorite", false)?;
        profile_states.push(ProfileState {
            id: id.to_owned(),
            subscription_id: subscription_id.to_owned(),
            subscription_key: subscription_key.to_owned(),
            missing,
            favorite,
        });
        profiles.push(PrivateProfile {
            id: id.to_owned(),
            name: name.to_owned(),
            canonical,
            subscription_id: subscription_id.to_owned(),
            subscription_key: subscription_key.to_owned(),
            missing,
            favorite,
        });
    }

    let mut custom_rules = Vec::new();
    let mut custom_rule_pairs = BTreeSet::new();
    if let Some(value) = root.get("customRules") {
        let values = value.as_array().ok_or(PrivateStoreError::InvalidShape)?;
        if values.len() > crate::routing::MAX_CUSTOM_RULES {
            return Err(PrivateStoreError::Routing(RoutingError::TooManyRules));
        }
        for value in values {
            let record = object(value)?;
            let id = required_string(record, "id")?;
            if !valid_record_id(id) {
                return Err(PrivateStoreError::InvalidShape);
            }
            let kind = required_string(record, "kind")?;
            let action = required_string(record, "action")?;
            let value = required_string(record, "value")?;
            let rule =
                CustomRule::parse(kind, action, value).map_err(PrivateStoreError::Routing)?;
            if rule.value != value {
                return Err(PrivateStoreError::InvalidShape);
            }
            if !custom_rule_pairs.insert((kind, value)) {
                return Err(PrivateStoreError::Routing(RoutingError::DuplicateRule));
            }
            custom_rules.push(rule);
        }
    }
    let rules_updated_at = optional_u64(root, "rulesUpdatedAt", 0)?;

    let startup = root
        .get("startup")
        .map(object)
        .transpose()?
        .map(|startup| {
            Ok(StartupState {
                enabled: startup
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .ok_or(PrivateStoreError::InvalidShape)?,
                target: required_string(startup, "target")?.to_owned(),
                profile_id: optional_string(startup, "profileId", "")?.to_owned(),
                mode: required_string(startup, "mode")?.to_owned(),
            })
        })
        .transpose()?;
    let startup_configured = root
        .get("startupConfigured")
        .map(|value| value.as_bool().ok_or(PrivateStoreError::InvalidShape))
        .transpose()?;
    let onboarding_complete = root
        .get("onboardingComplete")
        .map(|value| value.as_bool().ok_or(PrivateStoreError::InvalidShape))
        .transpose()?;
    let input = StoreStateInput {
        version,
        profiles: profile_states,
        subscriptions: subscription_states,
        active_id: optional_string(root, "activeId", "")?.to_owned(),
        last_id: optional_string(root, "lastId", "")?.to_owned(),
        routing_preset: root
            .get("routingPreset")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or(PrivateStoreError::InvalidShape)
            })
            .transpose()?,
        startup,
        startup_configured,
        onboarding_complete,
    };
    let normalized = normalize_store_state(input).map_err(PrivateStoreError::Store)?;
    let mut store = PrivateStore {
        document: value,
        version: normalized.version,
        profiles,
        subscriptions,
        active_id: normalized.active_id,
        last_id: normalized.last_id,
        routing_preset: normalized.routing_preset,
        custom_rules,
        rules_updated_at,
        startup: normalized.startup,
        startup_configured: normalized.startup_configured,
        onboarding_complete: normalized.onboarding_complete,
    };
    store.normalize_document()?;
    Ok(store)
}

impl PrivateStore {
    fn normalize_document(&mut self) -> Result<(), PrivateStoreError> {
        let root = self
            .document
            .as_object_mut()
            .ok_or(PrivateStoreError::InvalidShape)?;
        root.insert("version".to_owned(), Value::from(CURRENT_STORE_VERSION));
        root.insert("activeId".to_owned(), Value::from(self.active_id.clone()));
        root.insert("lastId".to_owned(), Value::from(self.last_id.clone()));
        root.insert(
            "routingPreset".to_owned(),
            Value::from(self.routing_preset.clone()),
        );
        root.insert(
            "rulesUpdatedAt".to_owned(),
            Value::from(self.rules_updated_at),
        );
        root.insert(
            "startupConfigured".to_owned(),
            Value::from(self.startup_configured),
        );
        root.insert(
            "onboardingComplete".to_owned(),
            Value::from(self.onboarding_complete),
        );
        root.entry("customRules".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));

        let subscription_values = root
            .entry("subscriptions".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or(PrivateStoreError::InvalidShape)?;
        if subscription_values.len() != self.subscriptions.len() {
            return Err(PrivateStoreError::InvalidShape);
        }
        for (value, subscription) in subscription_values.iter_mut().zip(&self.subscriptions) {
            let record = value
                .as_object_mut()
                .ok_or(PrivateStoreError::InvalidShape)?;
            record.insert("name".to_owned(), Value::from(subscription.name.clone()));
            record.insert("url".to_owned(), Value::from(subscription.url.clone()));
            record.insert("updatedAt".to_owned(), Value::from(subscription.updated_at));
        }

        let profile_values = root
            .get_mut("profiles")
            .and_then(Value::as_array_mut)
            .ok_or(PrivateStoreError::InvalidShape)?;
        if profile_values.len() != self.profiles.len() {
            return Err(PrivateStoreError::InvalidShape);
        }
        for (value, profile) in profile_values.iter_mut().zip(&self.profiles) {
            let record = value
                .as_object_mut()
                .ok_or(PrivateStoreError::InvalidShape)?;
            record.insert("name".to_owned(), Value::from(profile.name.clone()));
            record.insert(
                "protocol".to_owned(),
                Value::from(profile.canonical.protocol().as_str()),
            );
            record.insert("favorite".to_owned(), Value::from(profile.favorite));
            if profile.subscription_id.is_empty() {
                for key in ["subscriptionId", "subscriptionKey", "missing"] {
                    record.remove(key);
                }
            } else {
                record.insert(
                    "subscriptionId".to_owned(),
                    Value::from(profile.subscription_id.clone()),
                );
                record.insert(
                    "subscriptionKey".to_owned(),
                    Value::from(profile.subscription_key.clone()),
                );
                record.insert("missing".to_owned(), Value::from(profile.missing));
            }
        }

        root.insert(
            "startup".to_owned(),
            serde_json::json!({
                "enabled": self.startup.enabled,
                "target": self.startup.target,
                "profileId": self.startup.profile_id,
                "mode": self.startup.mode,
            }),
        );
        Ok(())
    }

    fn private_payload(&self) -> Result<Vec<u8>, PrivateStoreError> {
        let mut payload = serde_json::to_vec_pretty(&self.document)
            .map_err(|_| PrivateStoreError::InvalidJson)?;
        payload.push(b'\n');
        if payload.len() > MAX_PRIVATE_STORE_BYTES {
            return Err(PrivateStoreError::TooLarge);
        }
        Ok(payload)
    }

    #[must_use]
    pub fn projection(&self) -> StoreProjection {
        let mut counts = [0_usize; 4];
        for profile in &self.profiles {
            let index = match profile.canonical.protocol() {
                Protocol::Vless => 0,
                Protocol::Trojan => 1,
                Protocol::Hysteria2 => 2,
                Protocol::Tuic => 3,
            };
            counts[index] += 1;
        }
        StoreProjection {
            version: self.version,
            profile_count: self.profiles.len(),
            subscription_count: self.subscriptions.len(),
            vless_count: counts[0],
            trojan_count: counts[1],
            hysteria2_count: counts[2],
            tuic_count: counts[3],
            active_present: !self.active_id.is_empty(),
            last_present: !self.last_id.is_empty(),
            routing_preset: self.routing_preset.clone(),
            custom_rule_count: self.custom_rules.len(),
            startup_configured: self.startup_configured,
            onboarding_complete: self.onboarding_complete,
        }
    }

    /// Render one internal profile into a complete private Mihomo config.
    pub fn prepare_config(
        &self,
        profile_id: &str,
        template: &str,
        controller_socket: &str,
    ) -> Result<String, PrivateStoreError> {
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(PrivateStoreError::ProfileNotFound)?;
        let rendered = profile.canonical.render_mihomo_proxy(&profile.name, None);
        assemble_runtime_config(template, &rendered, controller_socket, &self.custom_rules)
            .map_err(PrivateStoreError::Config)
    }

    /// Render one internal profile for an explicit connection mode without
    /// mutating the stored routing preference or private profile store.
    pub fn prepare_config_mode(
        &self,
        profile_id: &str,
        template: &str,
        controller_socket: &str,
        mode: &str,
    ) -> Result<String, PrivateStoreError> {
        let template = template_with_mode(template, mode).map_err(PrivateStoreError::Routing)?;
        self.prepare_config(profile_id, &template, controller_socket)
    }

    /// Prepare the last-selected profile without exposing its opaque ID.
    pub fn prepare_last_config(
        &self,
        template: &str,
        controller_socket: &str,
    ) -> Result<String, PrivateStoreError> {
        if self.last_id.is_empty() {
            return Err(PrivateStoreError::ProfileNotFound);
        }
        self.prepare_config(&self.last_id, template, controller_socket)
    }

    /// Compare the active legacy config to the exact canonical rendering
    /// without exposing the private record ID, endpoint, name, or credentials.
    ///
    /// This is intentionally a boolean ownership fact. A missing active
    /// profile, unknown/duplicated mode, or byte mismatch is not relaxed into
    /// adoption by the R5 cutover preflight.
    pub fn active_config_matches(
        &self,
        template: &str,
        controller_socket: &str,
        active_config: &str,
    ) -> Result<bool, PrivateStoreError> {
        if self.active_id.is_empty() || active_config.len() > MAX_TEMPLATE_BYTES {
            return Ok(false);
        }
        let mut mode = None;
        for line in active_config.lines() {
            if line.starts_with([' ', '\t']) {
                continue;
            }
            let Some((key, raw_value)) = line.split_once(':') else {
                continue;
            };
            if key.trim() != "mode" {
                continue;
            }
            if mode.is_some() {
                return Ok(false);
            }
            let value = raw_value
                .split_once('#')
                .map_or(raw_value, |(value, _comment)| value)
                .trim()
                .trim_matches(['\'', '"'])
                .to_ascii_lowercase();
            if !matches!(value.as_str(), "rule" | "global" | "direct") {
                return Ok(false);
            }
            mode = Some(value);
        }
        let Some(mode) = mode else {
            return Ok(false);
        };
        let expected =
            self.prepare_config_mode(&self.active_id, template, controller_socket, &mode)?;
        Ok(equivalent_legacy_config(&expected, active_config))
    }
}

/// Validate, normalize and apply one profile mutation entirely in memory.
/// No partial payload is returned when any validation or size gate fails.
pub fn apply_profile_mutation(
    input: &str,
    mutation: ProfileMutation,
) -> Result<PrivateStoreMutation, PrivateStoreError> {
    let mut store = parse_private_store(input)?;
    let changed = match mutation {
        ProfileMutation::Rename {
            profile_id,
            new_name,
        } => {
            let new_name = clean_mutation_name(&new_name)?;
            let index = store
                .profiles
                .iter()
                .position(|profile| profile.id == profile_id)
                .ok_or(PrivateStoreError::ProfileNotFound)?;
            if !store.profiles[index].subscription_id.is_empty() {
                return Err(PrivateStoreError::SubscribedProfile);
            }
            if store
                .profiles
                .iter()
                .enumerate()
                .any(|(other, profile)| other != index && profile.name == new_name)
            {
                return Err(PrivateStoreError::DuplicateProfileName);
            }
            let changed = store.profiles[index].name != new_name;
            store.profiles[index].name = new_name;
            changed
        }
        ProfileMutation::Favorite {
            profile_id,
            enabled,
        } => {
            let profile = store
                .profiles
                .iter_mut()
                .find(|profile| profile.id == profile_id)
                .ok_or(PrivateStoreError::ProfileNotFound)?;
            let changed = profile.favorite != enabled;
            profile.favorite = enabled;
            changed
        }
        ProfileMutation::Delete { profile_id } => {
            let index = store
                .profiles
                .iter()
                .position(|profile| profile.id == profile_id)
                .ok_or(PrivateStoreError::ProfileNotFound)?;
            if !store.profiles[index].subscription_id.is_empty() {
                return Err(PrivateStoreError::SubscribedProfile);
            }
            store.profiles.remove(index);
            store
                .document
                .get_mut("profiles")
                .and_then(Value::as_array_mut)
                .ok_or(PrivateStoreError::InvalidShape)?
                .remove(index);
            if store.active_id == profile_id {
                store.active_id.clear();
            }
            if store.last_id == profile_id {
                store.last_id = store
                    .profiles
                    .first()
                    .map_or_else(String::new, |profile| profile.id.clone());
            }
            if store.startup.profile_id == profile_id {
                store.startup.enabled = false;
                store.startup.profile_id.clear();
            }
            if store.startup.target == "last" && store.profiles.is_empty() {
                store.startup.enabled = false;
            }
            true
        }
    };
    store.normalize_document()?;
    Ok(PrivateStoreMutation {
        payload: store.private_payload()?,
        changed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
    const SUBSCRIPTION_ID: &str = "10000000-0000-0000-0000-000000000001";
    const PRIVATE_URI: &str = "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private";

    fn store(uri: &str, protocol: &str) -> String {
        format!(
            r#"{{
              "version": 3,
              "activeId": "{PROFILE_ID}",
              "lastId": "{PROFILE_ID}",
              "profiles": [{{
                "id": "{PROFILE_ID}", "name": "Synthetic",
                "uri": {uri:?}, "protocol": "{protocol}",
                "subscriptionId": "{SUBSCRIPTION_ID}",
                "subscriptionKey": "{}", "missing": false, "favorite": true
              }}],
              "subscriptions": [{{
                "id": "{SUBSCRIPTION_ID}", "name": "Synthetic source",
                "url": "https://example.invalid/sub", "updatedAt": 1
              }}],
              "routingPreset": "custom",
              "customRules": [{{
                "id": "20000000-0000-0000-0000-000000000001",
                "kind": "domain", "value": "example.invalid", "action": "proxy"
              }}],
              "rulesUpdatedAt": 1,
              "startupConfigured": true,
              "startup": {{"enabled": true, "target": "profile", "profileId": "{PROFILE_ID}", "mode": "global"}},
              "onboardingComplete": true
            }}"#,
            "a".repeat(64)
        )
    }

    fn local_document() -> Value {
        let mut value: Value = serde_json::from_str(&store(PRIVATE_URI, "vless")).unwrap();
        value["subscriptions"] = serde_json::json!([]);
        let profile = value["profiles"][0].as_object_mut().unwrap();
        for key in ["subscriptionId", "subscriptionKey", "missing"] {
            profile.remove(key);
        }
        value
    }

    fn mutation_error(
        result: Result<PrivateStoreMutation, PrivateStoreError>,
    ) -> PrivateStoreError {
        match result {
            Ok(_) => panic!("private mutation unexpectedly succeeded"),
            Err(error) => error,
        }
    }

    #[test]
    fn profile_favorite_and_legacy_normalization_preserve_private_values() {
        let mut value = local_document();
        value["version"] = Value::from(1);
        value["profiles"][0]
            .as_object_mut()
            .unwrap()
            .remove("protocol");
        value["profiles"][0]
            .as_object_mut()
            .unwrap()
            .remove("favorite");
        for key in [
            "routingPreset",
            "customRules",
            "rulesUpdatedAt",
            "startup",
            "startupConfigured",
            "onboardingComplete",
        ] {
            value.as_object_mut().unwrap().remove(key);
        }
        let input = serde_json::to_string(&value).unwrap();
        let result = apply_profile_mutation(
            &input,
            ProfileMutation::Favorite {
                profile_id: PROFILE_ID.to_owned(),
                enabled: true,
            },
        )
        .unwrap();
        assert!(result.changed);
        let written: Value = serde_json::from_slice(result.payload()).unwrap();
        assert_eq!(written["version"], 3);
        assert_eq!(written["profiles"][0]["protocol"], "vless");
        assert_eq!(written["profiles"][0]["favorite"], true);
        assert_eq!(written["profiles"][0]["uri"], PRIVATE_URI);
        assert_eq!(written["routingPreset"], "roscomvpn-default");
        assert_eq!(written["startupConfigured"], false);
        assert_eq!(written["onboardingComplete"], true);
    }

    #[test]
    fn subscribed_profile_allows_favorite_but_rejects_rename_and_delete() {
        let input = store(PRIVATE_URI, "vless");
        let favorite = apply_profile_mutation(
            &input,
            ProfileMutation::Favorite {
                profile_id: PROFILE_ID.to_owned(),
                enabled: false,
            },
        )
        .unwrap();
        assert!(favorite.changed);
        for mutation in [
            ProfileMutation::Rename {
                profile_id: PROFILE_ID.to_owned(),
                new_name: "Renamed".to_owned(),
            },
            ProfileMutation::Delete {
                profile_id: PROFILE_ID.to_owned(),
            },
        ] {
            assert_eq!(
                mutation_error(apply_profile_mutation(&input, mutation)),
                PrivateStoreError::SubscribedProfile
            );
        }
    }

    #[test]
    fn rename_is_canonical_unique_and_errors_do_not_echo_private_input() {
        let mut value = local_document();
        let mut second = value["profiles"][0].clone();
        second["id"] = Value::from("00000000-0000-0000-0000-000000000002");
        second["name"] = Value::from("Second");
        value["profiles"].as_array_mut().unwrap().push(second);
        let input = serde_json::to_string(&value).unwrap();
        assert_eq!(
            mutation_error(apply_profile_mutation(
                &input,
                ProfileMutation::Rename {
                    profile_id: PROFILE_ID.to_owned(),
                    new_name: "Second".to_owned(),
                },
            )),
            PrivateStoreError::DuplicateProfileName
        );
        let private_name = "private.example/password".repeat(8);
        let error = mutation_error(apply_profile_mutation(
            &input,
            ProfileMutation::Rename {
                profile_id: PROFILE_ID.to_owned(),
                new_name: private_name.clone(),
            },
        ));
        let public = format!("{error:?} {error}");
        assert_eq!(error, PrivateStoreError::InvalidName);
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
    }

    #[test]
    fn delete_repairs_active_last_and_startup_without_touching_other_profile() {
        let mut value = local_document();
        let mut second = value["profiles"][0].clone();
        second["id"] = Value::from("00000000-0000-0000-0000-000000000002");
        second["name"] = Value::from("Second");
        second["uri"] = Value::from(
            "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Second",
        );
        value["profiles"].as_array_mut().unwrap().push(second);
        let input = serde_json::to_string(&value).unwrap();
        let result = apply_profile_mutation(
            &input,
            ProfileMutation::Delete {
                profile_id: PROFILE_ID.to_owned(),
            },
        )
        .unwrap();
        assert!(result.changed);
        let written: Value = serde_json::from_slice(result.payload()).unwrap();
        assert_eq!(written["profiles"].as_array().unwrap().len(), 1);
        assert_eq!(written["activeId"], "");
        assert_eq!(written["lastId"], "00000000-0000-0000-0000-000000000002");
        assert_eq!(written["startup"]["enabled"], false);
        assert_eq!(written["startup"]["profileId"], "");
        assert_eq!(written["profiles"][0]["name"], "Second");
        assert!(
            written["profiles"][0]["uri"]
                .as_str()
                .unwrap()
                .contains("22222222")
        );
    }

    #[test]
    fn current_store_projects_only_safe_counts_and_prepares_config() {
        let input = store(
            "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private",
            "vless",
        );
        let store = parse_private_store(&input).unwrap();
        assert_eq!(
            store.projection(),
            StoreProjection {
                version: 3,
                profile_count: 1,
                subscription_count: 1,
                vless_count: 1,
                trojan_count: 0,
                hysteria2_count: 0,
                tuic_count: 0,
                active_present: true,
                last_present: true,
                routing_preset: "custom".into(),
                custom_rule_count: 1,
                startup_configured: true,
                onboarding_complete: true,
            }
        );
        let config = store
            .prepare_config(
                PROFILE_ID,
                "proxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
                "/run/user/1000/omavless/mihomo.sock",
            )
            .unwrap();
        assert!(config.contains("external-controller-unix"));
        assert!(config.contains("DOMAIN,example.invalid,PROXY"));
        assert!(!config.contains("external-controller:"));
        let direct = store
            .prepare_config_mode(
                PROFILE_ID,
                "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
                "/run/user/1000/omavless/mihomo.sock",
                "direct",
            )
            .unwrap();
        assert!(direct.contains("\nmode: direct\n"));
        assert!(!direct.contains("\nmode: rule\n"));
    }

    #[test]
    fn active_config_match_is_exact_private_and_fail_closed() {
        let private_uri = "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private";
        let store = parse_private_store(&store(private_uri, "vless")).unwrap();
        let template = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
        let controller = "/run/user/1000/legacy-private.sock";
        let active = store
            .prepare_config_mode(PROFILE_ID, template, controller, "global")
            .unwrap();
        assert!(
            store
                .active_config_matches(template, controller, &active)
                .unwrap()
        );
        let legacy_format = active
            .replace("type: \"vless\"", "type: vless")
            .replace("network: \"tcp\"", "network: tcp");
        assert!(
            store
                .active_config_matches(template, controller, &legacy_format)
                .unwrap()
        );
        assert!(
            !store
                .active_config_matches(
                    template,
                    controller,
                    &legacy_format.replace("type: vless", "type: \'vless\"")
                )
                .unwrap()
        );
        assert!(
            !store
                .active_config_matches(
                    template,
                    controller,
                    &active.replace("203.0.113.1", "203.0.113.9"),
                )
                .unwrap()
        );
        assert!(
            !store
                .active_config_matches(template, controller, &format!("mode: global\n{active}"),)
                .unwrap()
        );
        let debug = format!(
            "{:?}",
            store.active_config_matches(template, controller, &active)
        );
        for private in ["11111111", "203.0.113.1", "Private"] {
            assert!(!debug.contains(private));
        }
    }

    #[test]
    fn legacy_defaults_and_all_existing_protocols_are_supported() {
        let cases = [
            (
                "trojan://password@203.0.113.2:443?security=tls&sni=cdn.example.invalid",
                "trojan",
            ),
            (
                "hy2://auth@203.0.113.3:443?sni=cdn.example.invalid",
                "hysteria2",
            ),
            (
                "tuic://22222222-2222-4222-8222-222222222222:password@203.0.113.4:443?sni=cdn.example.invalid",
                "tuic",
            ),
        ];
        for (uri, protocol) in cases {
            assert_eq!(
                parse_private_store(&store(uri, protocol))
                    .unwrap()
                    .projection()
                    .profile_count,
                1
            );
        }
        let legacy = format!(
            r#"{{"profiles":[{{"id":"{PROFILE_ID}","name":"Legacy","uri":"vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp"}}]}}"#
        );
        let projection = parse_private_store(&legacy).unwrap().projection();
        assert_eq!(projection.version, 3);
        assert_eq!(projection.routing_preset, "roscomvpn-default");
        assert!(!projection.startup_configured);
    }

    #[test]
    fn errors_and_debug_never_echo_private_values() {
        let private = "trojan://secret@private.example:443";
        for input in [
            private.to_owned(),
            store(private, "vless"),
            store("trojan://secret@bad", "trojan"),
        ] {
            let error = match parse_private_store(&input) {
                Ok(_) => panic!("private invalid store was accepted"),
                Err(error) => error,
            };
            let output = format!("{error:?} {error}");
            assert!(!output.contains("secret"));
            assert!(!output.contains("private.example"));
        }
    }

    #[test]
    fn malformed_names_urls_relationships_rules_and_timestamps_fail_closed() {
        let base = store(
            "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp",
            "vless",
        );
        let replacements = [
            ("\"Synthetic\"", "\" Synthetic \""),
            ("https://example.invalid/sub", "http://example.invalid/sub"),
            (&"a".repeat(64), "not-a-key"),
            ("\"example.invalid\"", "\"EXAMPLE.INVALID\""),
            ("\"updatedAt\": 1", "\"updatedAt\": -1"),
        ];
        for (from, to) in replacements {
            assert!(parse_private_store(&base.replacen(from, to, 1)).is_err());
        }
    }
}
