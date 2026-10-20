// SPDX-License-Identifier: MIT
//! Pure local-snapshot preparation. No filesystem, transport or lifecycle work.
use super::*;
use crate::subscription_feed::decode_local_profile_lines;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalBundleCounts {
    pub accepted: usize,
    pub duplicates: usize,
    pub skipped: usize,
}

/// Contains private links and the exact validated store snapshot. Intentionally
/// not Debug/Clone/Serialize. Runtime integration must fence preview/confirmation
/// and use its existing compare-before-replace atomic store transaction.
pub struct LocalBundlePlan {
    store: PrivateStore,
    entries: Vec<(String, CanonicalProfile)>,
    counts: LocalBundleCounts,
}

impl PrivateStore {
    pub fn plan_local_bundle(self, input: &str) -> Result<LocalBundlePlan, PrivateStoreError> {
        let feed = decode_local_profile_lines(input).map_err(|error| match error {
            crate::subscription_feed::SubscriptionFeedError::TooLarge
            | crate::subscription_feed::SubscriptionFeedError::TooManyCandidates => {
                PrivateStoreError::TooLarge
            }
            _ => PrivateStoreError::InvalidShape,
        })?;
        let duplicates = feed.duplicate_count();
        let mut counts = LocalBundleCounts {
            accepted: 0,
            duplicates,
            skipped: feed.counts().skipped - duplicates,
        };
        let existing: BTreeSet<_> = self
            .profiles
            .iter()
            .map(|profile| profile.credential.subscription_identity())
            .collect();
        let mut entries = Vec::new();
        for uri in feed.into_private_uris() {
            let canonical = parse_canonical(&uri).map_err(PrivateStoreError::Profile)?;
            if existing.contains(&canonical.subscription_identity()) {
                counts.duplicates += 1;
            } else {
                entries.push((uri, canonical));
            }
        }
        counts.accepted = entries.len();
        if self.profiles.len() + counts.accepted > MAX_PROFILES {
            return Err(PrivateStoreError::TooLarge);
        }
        Ok(LocalBundlePlan {
            store: self,
            entries,
            counts,
        })
    }
}

impl LocalBundlePlan {
    #[must_use]
    pub const fn counts(&self) -> LocalBundleCounts {
        self.counts
    }

    /// Build one complete candidate or return an error. IDs come from the
    /// trusted owner only; invalid/colliding IDs never yield a partial payload.
    /// Existing records, selection, connection preferences and subscriptions
    /// are preserved; bundle links become standalone profiles, not a feed URL.
    pub fn into_mutation(
        self,
        mut next_record_id: impl FnMut() -> String,
    ) -> Result<PrivateStoreMutation, PrivateStoreError> {
        let mut ids: BTreeSet<_> = self
            .store
            .profiles
            .iter()
            .map(|p| p.id.clone())
            .chain(self.store.subscriptions.iter().map(|s| s.id.clone()))
            .collect();
        let mut names: BTreeSet<_> = self.store.profiles.iter().map(|p| p.name.clone()).collect();
        let changed = !self.entries.is_empty();
        let mut document = self.store.document;
        let profiles = document["profiles"]
            .as_array_mut()
            .ok_or(PrivateStoreError::InvalidShape)?;
        for (uri, canonical) in self.entries {
            let id = next_record_id();
            if !valid_record_id(&id) || !ids.insert(id.clone()) {
                return Err(PrivateStoreError::InvalidShape);
            }
            let suggested = canonical.subscription_name_candidate();
            let base =
                clean_mutation_name(&suggested).unwrap_or_else(|_| "Imported profile".into());
            let mut name = base.clone();
            // At most MAX_PROFILES existing/new names. A finite suffix search
            // preserves display labels without overwriting an unrelated record.
            for suffix in 2..=MAX_PROFILES + 1 {
                if !names.contains(&name) {
                    break;
                }
                let tail = format!(" ({suffix})");
                name = format!(
                    "{}{}",
                    base.chars()
                        .take(MAX_NAME_CHARS - tail.len())
                        .collect::<String>(),
                    tail
                );
            }
            if !names.insert(name.clone()) {
                return Err(PrivateStoreError::DuplicateProfileName);
            }
            profiles.push(serde_json::json!({"id":id, "name":name, "uri":uri,
                "protocol":canonical.protocol().as_str()}));
        }
        let validated = parse_private_store(&document.to_string())?;
        Ok(PrivateStoreMutation {
            payload: validated.private_payload()?,
            changed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{
        Engine,
        engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    };

    fn uri(node: usize, label: &str) -> String {
        format!(
            "vless://11111111-1111-4111-8111-111111111111@node-{node}.invalid:443?security=none&type=tcp#{label}"
        )
    }
    fn empty() -> String {
        serde_json::json!({"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":""})
            .to_string()
    }
    fn plan(input: &str) -> LocalBundlePlan {
        parse_private_store(&empty())
            .unwrap()
            .plan_local_bundle(input)
            .ok()
            .unwrap()
    }
    fn ids() -> impl FnMut() -> String {
        let mut number = 0;
        move || {
            number += 1;
            format!("00000000-0000-4000-8000-{number:012}")
        }
    }
    fn document(mutation: &PrivateStoreMutation) -> Value {
        serde_json::from_slice(mutation.payload()).unwrap()
    }

    #[test]
    fn raw_standard_urlsafe_wrapped_and_bom_share_one_result() {
        let raw = format!("{}\n{}\n", uri(1, "Первый сервер"), uri(2, "東京"));
        let encoded = STANDARD.encode(raw.as_bytes());
        let wrapped = encoded
            .as_bytes()
            .chunks(40)
            .map(|part| std::str::from_utf8(part).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n");
        let expected = document(&plan(&raw).into_mutation(ids()).unwrap());
        for input in [
            encoded,
            wrapped,
            URL_SAFE_NO_PAD.encode(raw.as_bytes()),
            format!("\u{feff}{raw}"),
            STANDARD.encode(format!("\u{feff}{raw}")),
        ] {
            assert_eq!(
                document(&plan(&input).into_mutation(ids()).unwrap()),
                expected
            );
        }
        assert_eq!(expected["profiles"][0]["name"], "Первый сервер");
        assert_eq!(expected["profiles"][1]["name"], "東京");
    }

    #[test]
    fn url_shaped_labels_are_not_mistaken_for_second_links() {
        for label in [
            "https://synthetic.invalid",
            "https%3A%2F%2Fsynthetic.invalid",
        ] {
            let raw = uri(1, label);
            for input in [
                raw.clone(),
                STANDARD.encode(&raw),
                URL_SAFE_NO_PAD.encode(&raw),
            ] {
                let mutation = plan(&input).into_mutation(ids()).unwrap();
                assert_eq!(
                    document(&mutation)["profiles"][0]["name"],
                    "https://synthetic.invalid"
                );
            }
        }
    }

    #[test]
    fn same_uuid_different_servers_survive_but_label_only_duplicates_do_not() {
        let input = format!(
            "{}\n{}\n{}",
            uri(1, "One"),
            uri(2, "Two"),
            uri(1, "Renamed")
        );
        let plan = plan(&input);
        assert_eq!(
            plan.counts(),
            LocalBundleCounts {
                accepted: 2,
                duplicates: 1,
                skipped: 0
            }
        );
        assert_eq!(
            document(&plan.into_mutation(ids()).unwrap())["profiles"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn malformed_and_unsupported_links_are_counted_without_becoming_profiles() {
        let input = format!("{}\nvless://bad\nss://unsupported", uri(1, "One"));
        assert_eq!(
            plan(&input).counts(),
            LocalBundleCounts {
                accepted: 1,
                duplicates: 0,
                skipped: 2
            }
        );
    }

    #[test]
    fn ambiguous_documents_and_remote_urls_are_not_scanned_for_embedded_links() {
        let link = uri(1, "One");
        for input in [
            format!("proxies:\n{link}"),
            format!("{{\"uri\":\"{link}\"}}"),
            format!("- {link}"),
            format!("{link}\nhttps://synthetic.invalid/feed"),
            format!("{link}\nplain text"),
            format!("{} {}", uri(1, ""), uri(2, "")),
            "https://synthetic.invalid/feed".into(),
            "vless://bad".into(),
            "%%%".into(),
            String::new(),
        ] {
            assert!(
                parse_private_store(&empty())
                    .unwrap()
                    .plan_local_bundle(&input)
                    .is_err()
            );
            let encoded = STANDARD.encode(input.as_bytes());
            assert!(
                parse_private_store(&empty())
                    .unwrap()
                    .plan_local_bundle(&encoded)
                    .is_err()
            );
        }
    }

    #[test]
    fn byte_and_row_bounds_are_enforced_before_candidate_creation() {
        for input in [
            "a".repeat(32769),
            "я".repeat(16385),
            ("ss://x\n".repeat(1024) + &uri(1, "One")),
        ] {
            assert!(matches!(
                parse_private_store(&empty())
                    .unwrap()
                    .plan_local_bundle(&input),
                Err(PrivateStoreError::TooLarge)
            ));
        }
    }

    #[test]
    fn realistic_two_hundred_profile_snapshot_has_bounded_counts() {
        let raw = (0..200)
            .map(|i| uri(i, "Node"))
            .collect::<Vec<_>>()
            .join("\n");
        for input in [raw.clone(), STANDARD.encode(raw)] {
            assert!(input.len() <= 32768);
            let plan = plan(&input);
            assert_eq!(plan.counts().accepted, 200);
            let mutation = plan.into_mutation(ids()).unwrap();
            assert_eq!(
                document(&mutation)["profiles"].as_array().unwrap().len(),
                200
            );
        }
    }

    #[test]
    fn store_capacity_is_all_or_nothing() {
        let raw = (0..257).map(|i| uri(i, "N")).collect::<Vec<_>>().join("\n");
        assert!(matches!(
            parse_private_store(&empty())
                .unwrap()
                .plan_local_bundle(&raw),
            Err(PrivateStoreError::TooLarge)
        ));
    }

    #[test]
    fn colliding_or_invalid_owner_ids_refuse_entire_candidate() {
        let input = format!("{}\n{}", uri(1, "One"), uri(2, "Two"));
        for id in ["not-an-id", "00000000-0000-4000-8000-000000000001"] {
            assert!(plan(&input).into_mutation(|| id.into()).is_err());
        }
    }

    #[test]
    fn same_display_name_gets_deterministic_suffix_without_replacement() {
        let label = "я".repeat(80);
        let input = format!("{}\n{}\n{}", uri(1, &label), uri(2, &label), uri(3, &label));
        let mutation = plan(&input).into_mutation(ids()).unwrap();
        let value = document(&mutation);
        assert_eq!(value["profiles"][0]["name"], label);
        for (index, suffix) in [(1, " (2)"), (2, " (3)")] {
            let name = value["profiles"][index]["name"].as_str().unwrap();
            assert!(name.ends_with(suffix));
            assert_eq!(name.chars().count(), 80);
        }
    }

    #[test]
    fn existing_identity_is_skipped_and_unrelated_metadata_stays_unchanged() {
        let initial = plan(&uri(1, "Existing")).into_mutation(ids()).unwrap();
        let mut original = document(&initial);
        original["activeId"] = original["profiles"][0]["id"].clone();
        original["lastId"] = original["activeId"].clone();
        original["extension"] = serde_json::json!({"keep":true});
        original["profiles"][0]["favorite"] = Value::Bool(true);
        let store = parse_private_store(&original.to_string()).unwrap();
        let before: Value = serde_json::from_slice(&store.private_payload().unwrap()).unwrap();
        let plan = store
            .plan_local_bundle(&format!(
                "{}\n{}",
                uri(1, "Other label"),
                uri(2, "Existing")
            ))
            .ok()
            .unwrap();
        assert_eq!(
            plan.counts(),
            LocalBundleCounts {
                accepted: 1,
                duplicates: 1,
                skipped: 0
            }
        );
        let result = document(
            &plan
                .into_mutation(|| "00000000-0000-4000-8000-000000000002".into())
                .unwrap(),
        );
        for key in [
            "activeId",
            "lastId",
            "subscriptions",
            "startup",
            "routingPreset",
            "customRules",
            "extension",
        ] {
            assert_eq!(result[key], before[key], "{key}");
        }
        assert_eq!(result["profiles"][0], before["profiles"][0]);
        assert_eq!(result["profiles"][1]["name"], "Existing (2)");
        assert!(result["profiles"][1].get("subscriptionId").is_none());
    }

    #[test]
    fn all_existing_duplicates_are_noop_and_do_not_allocate_ids() {
        let initial = plan(&uri(1, "One")).into_mutation(ids()).unwrap();
        let store = parse_private_store(std::str::from_utf8(initial.payload()).unwrap()).unwrap();
        let plan = store.plan_local_bundle(&uri(1, "Renamed")).ok().unwrap();
        assert_eq!(
            plan.counts(),
            LocalBundleCounts {
                accepted: 0,
                duplicates: 1,
                skipped: 0
            }
        );
        let result = plan
            .into_mutation(|| panic!("no new record may be allocated"))
            .unwrap();
        assert!(!result.changed);
        assert_eq!(result.payload(), initial.payload());
    }

    #[test]
    fn errors_never_echo_private_fragments() {
        let error = parse_private_store(&empty())
            .unwrap()
            .plan_local_bundle("https://private.invalid/secret-token")
            .err()
            .unwrap();
        let public = format!("{error:?} {error}");
        for secret in ["private.invalid", "secret-token", "https://"] {
            assert!(!public.contains(secret));
        }
    }
}
