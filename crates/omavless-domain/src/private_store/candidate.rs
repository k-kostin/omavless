// SPDX-License-Identifier: MIT

//! Inactive, complete v4 mixed-store validation. The installed v1-v3 loader,
//! owner, mutations, IPC and core paths deliberately do not call this module.

use super::*;
use crate::store::{NormalizedStoreState, normalize_candidate_store_state};
use omavless_profile::wireguard::private_record::parse_private_wireguard_record;
use omavless_profile::wireguard::{WireGuardProfile, parse_unique_private_json};

/// Private credential variants remain typed; a WG record is never represented
/// by an invented URI or silently hidden from the shared identity graph.
enum CandidateCredential {
    Uri(CanonicalProfile),
    WireGuard(WireGuardProfile),
}

/// Not serializable or printable. No conversion to runtime `PrivateStore`.
pub struct CandidatePrivateStore {
    document: Value,
    state: NormalizedStoreState,
    credentials: Vec<CandidateCredential>,
}

impl CandidatePrivateStore {
    /// Publicly safe counts only. Profile names, endpoints and reusable keys
    /// stay inside the private candidate.
    #[must_use]
    pub fn profile_counts(&self) -> (usize, usize, usize) {
        let (mut uri, mut wg) = (0, 0);
        for credential in &self.credentials {
            match credential {
                CandidateCredential::Uri(profile) => {
                    let _ = profile.protocol();
                    uri += 1;
                }
                CandidateCredential::WireGuard(profile) => {
                    let _ = profile.facts().flavor;
                    wg += 1;
                }
            }
        }
        (uri, wg, self.state.subscriptions.len())
    }

    /// Test-only-safe metadata facts about normalized pointers. The IDs and
    /// startup target themselves are deliberately not released.
    #[must_use]
    pub fn pointer_presence(&self) -> (bool, bool, bool) {
        (
            !self.state.active_id.is_empty(),
            !self.state.last_id.is_empty(),
            self.state.startup.enabled,
        )
    }

    /// Intentional private byte release for a future owner-bound writer only.
    /// There is currently no production caller and no filesystem publication.
    pub fn into_private_bytes(mut self) -> Result<Vec<u8>, PrivateStoreError> {
        // Admission normalizes stale pointers in memory. Do not serialize the
        // original pointers again when this candidate is explicitly exported.
        // Keep unrelated root/row extensions and credential bytes untouched.
        let root = self
            .document
            .as_object_mut()
            .ok_or(PrivateStoreError::InvalidShape)?;
        root.insert(
            "activeId".to_owned(),
            Value::from(self.state.active_id.clone()),
        );
        root.insert("lastId".to_owned(), Value::from(self.state.last_id.clone()));
        let startup = root
            .entry("startup".to_owned())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or(PrivateStoreError::InvalidShape)?;
        startup.insert(
            "enabled".to_owned(),
            Value::from(self.state.startup.enabled),
        );
        startup.insert("target".to_owned(), Value::from(self.state.startup.target));
        startup.insert(
            "profileId".to_owned(),
            Value::from(self.state.startup.profile_id),
        );
        startup.insert("mode".to_owned(), Value::from(self.state.startup.mode));
        let mut bytes =
            serde_json::to_vec(&self.document).map_err(|_| PrivateStoreError::InvalidJson)?;
        bytes.push(b'\n');
        let text = std::str::from_utf8(&bytes).map_err(|_| PrivateStoreError::InvalidJson)?;
        parse_candidate_private_store(text)?;
        Ok(bytes)
    }

    /// Prepare one standalone native profile in memory. This never writes the
    /// store or makes the profile connectable through the installed owner.
    pub fn with_wireguard(
        mut self,
        id: &str,
        name: &str,
        profile: WireGuardProfile,
    ) -> Result<Self, PrivateStoreError> {
        if !valid_record_id(id) || !canonical_name(name) {
            return Err(PrivateStoreError::InvalidShape);
        }
        let root = self
            .document
            .as_object()
            .ok_or(PrivateStoreError::InvalidShape)?;
        if root
            .get("subscriptions")
            .and_then(Value::as_array)
            .is_some_and(|subscriptions| subscriptions.iter().any(|entry| entry["id"] == id))
        {
            return Err(PrivateStoreError::InvalidShape);
        }
        let profiles = root
            .get("profiles")
            .and_then(Value::as_array)
            .ok_or(PrivateStoreError::InvalidShape)?;
        if profiles.iter().any(|entry| entry["id"] == id) {
            return Err(PrivateStoreError::InvalidShape);
        }
        if profiles.iter().any(|entry| entry["name"] == name) {
            return Err(PrivateStoreError::DuplicateProfileName);
        }
        let protocol = profile.facts().flavor.protocol_name();
        let record = profile
            .private_record()
            .map_err(|_| PrivateStoreError::InvalidShape)?;
        let value: Value = serde_json::from_slice(record.expose_private_bytes())
            .map_err(|_| PrivateStoreError::InvalidShape)?;
        self.document["profiles"]
            .as_array_mut()
            .ok_or(PrivateStoreError::InvalidShape)?
            .push(serde_json::json!({
                "id": id, "name": name, "protocol": protocol, "wireguard": value
            }));
        parse_candidate_private_store(&self.document.to_string())
    }

    /// Prepare a standalone metadata rename over the complete mixed graph.
    /// No credential is converted, exported or made runtime-capable. The
    /// owner must separately fence any eventual publication/lifecycle effect.
    pub fn rename_standalone(
        mut self,
        id: &str,
        new_name: &str,
    ) -> Result<(Self, bool), PrivateStoreError> {
        let name = clean_mutation_name(new_name)?;
        let profiles = self
            .document
            .get_mut("profiles")
            .and_then(Value::as_array_mut)
            .ok_or(PrivateStoreError::InvalidShape)?;
        let index = profiles
            .iter()
            .position(|entry| entry["id"] == id)
            .ok_or(PrivateStoreError::ProfileNotFound)?;
        if profiles[index]["subscriptionId"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
        {
            return Err(PrivateStoreError::SubscribedProfile);
        }
        if profiles
            .iter()
            .enumerate()
            .any(|(other, entry)| other != index && entry["name"] == name)
        {
            return Err(PrivateStoreError::DuplicateProfileName);
        }
        let changed = profiles[index]["name"] != name;
        profiles[index]["name"] = Value::from(name);
        Ok((
            parse_candidate_private_store(&self.document.to_string())?,
            changed,
        ))
    }

    /// Prepare the same favorite metadata operation for URI and WG/AWG rows.
    /// Subscribed URI rows remain eligible, as in the current v3 store.
    pub fn set_favorite(
        mut self,
        id: &str,
        enabled: bool,
    ) -> Result<(Self, bool), PrivateStoreError> {
        let profiles = self
            .document
            .get_mut("profiles")
            .and_then(Value::as_array_mut)
            .ok_or(PrivateStoreError::InvalidShape)?;
        let entry = profiles
            .iter_mut()
            .find(|entry| entry["id"] == id)
            .ok_or(PrivateStoreError::ProfileNotFound)?;
        let changed = entry["favorite"].as_bool().unwrap_or(false) != enabled;
        entry["favorite"] = Value::from(enabled);
        Ok((
            parse_candidate_private_store(&self.document.to_string())?,
            changed,
        ))
    }
}

/// Convert a fully validated legacy store into an in-memory v4 candidate.
/// Original bytes must also pass duplicate-key rejection before the legacy
/// parser can discard any conflicting value. No file or owner state changes.
pub fn migrate_legacy_store_candidate(
    input: &str,
) -> Result<CandidatePrivateStore, PrivateStoreError> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(PrivateStoreError::TooLarge);
    }
    parse_unique_private_json(input).map_err(|_| PrivateStoreError::InvalidJson)?;
    let mut legacy = parse_private_store(input)?;
    legacy.document["version"] = Value::from(4);
    parse_candidate_private_store(&legacy.document.to_string())
}

/// Validate a complete future v4 document from original bytes. Duplicate keys
/// are rejected before `Value` can discard them, including nested WG fields.
/// This function does not migrate, write, import or activate anything.
pub fn parse_candidate_private_store(
    input: &str,
) -> Result<CandidatePrivateStore, PrivateStoreError> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(PrivateStoreError::TooLarge);
    }
    let value = parse_unique_private_json(input).map_err(|_| PrivateStoreError::InvalidJson)?;
    let root = object(&value)?;
    if root.get("version").and_then(Value::as_u64) != Some(4) {
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
        let _ = optional_u64(record, "updatedAt", 0)?;
        subscriptions.push(SubscriptionState { id: id.to_owned() });
    }

    let profile_values = array(root, "profiles")?;
    if profile_values.len() > MAX_PROFILES {
        return Err(PrivateStoreError::Store(StoreError::TooManyProfiles));
    }
    let mut profiles = Vec::with_capacity(profile_values.len());
    let mut credentials = Vec::with_capacity(profile_values.len());
    for value in profile_values {
        let record = object(value)?;
        let id = required_string(record, "id")?;
        let name = required_string(record, "name")?;
        if !canonical_name(name) {
            return Err(PrivateStoreError::InvalidName);
        }
        let stored_protocol = required_string(record, "protocol")?;
        let credential = match (record.get("uri"), record.get("wireguard")) {
            (Some(uri), None) => {
                let uri = uri.as_str().ok_or(PrivateStoreError::InvalidShape)?;
                let parsed = parse_canonical(uri).map_err(PrivateStoreError::Profile)?;
                if protocol(stored_protocol) != Some(parsed.protocol()) {
                    return Err(PrivateStoreError::ProtocolMismatch);
                }
                CandidateCredential::Uri(parsed)
            }
            (None, Some(wireguard)) => {
                // No hidden provider-managed records, second credential source,
                // or opaque per-row extensions until their semantics are owned.
                const WG_FIELDS: &[&str] = &["id", "name", "protocol", "wireguard", "favorite"];
                if record.keys().any(|key| !WG_FIELDS.contains(&key.as_str())) {
                    return Err(PrivateStoreError::InvalidShape);
                }
                let private = wireguard.to_string();
                let parsed = parse_private_wireguard_record(private.as_bytes())
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                if stored_protocol != parsed.facts().flavor.protocol_name() {
                    return Err(PrivateStoreError::ProtocolMismatch);
                }
                CandidateCredential::WireGuard(parsed)
            }
            _ => return Err(PrivateStoreError::InvalidShape),
        };
        let subscription_id = optional_string(record, "subscriptionId", "")?;
        let subscription_key = optional_string(record, "subscriptionKey", "")?;
        let missing = optional_bool(record, "missing", false)?;
        let favorite = optional_bool(record, "favorite", false)?;
        profiles.push(ProfileState {
            id: id.to_owned(),
            subscription_id: subscription_id.to_owned(),
            subscription_key: subscription_key.to_owned(),
            missing,
            favorite,
        });
        credentials.push(credential);
    }

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
            let rule_value = required_string(record, "value")?;
            let rule =
                CustomRule::parse(kind, action, rule_value).map_err(PrivateStoreError::Routing)?;
            if rule.value != rule_value || !custom_rule_pairs.insert((kind, rule_value)) {
                return Err(PrivateStoreError::InvalidShape);
            }
        }
    }
    let _ = optional_u64(root, "rulesUpdatedAt", 0)?;

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
    let state = normalize_candidate_store_state(StoreStateInput {
        version: 4,
        profiles,
        subscriptions,
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
    })
    .map_err(PrivateStoreError::Store)?;
    Ok(CandidatePrivateStore {
        document: value,
        state,
        credentials,
    })
}
