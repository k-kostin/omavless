// SPDX-License-Identifier: MIT
//! Daily opt-in admission receipts, not a recoverable job queue.
use super::*;
use serde_json::json;

pub const DAILY_REFRESH_MILLIS: u64 = 86_400_000;
const MAX_TIME: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleDisposition {
    Admitted,
    Committed,
    FailedBeforeCommit,
    CancelledBeforeCommit,
    Unknown,
}
impl ScheduleDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Committed => "committed",
            Self::FailedBeforeCommit => "failed_before_commit",
            Self::CancelledBeforeCommit => "cancelled_before_commit",
            Self::Unknown => "unknown",
        }
    }
    fn parse(text: &str) -> Result<Self, PrivateStoreError> {
        match text {
            "admitted" => Ok(Self::Admitted),
            "committed" => Ok(Self::Committed),
            "failed_before_commit" => Ok(Self::FailedBeforeCommit),
            "cancelled_before_commit" => Ok(Self::CancelledBeforeCommit),
            "unknown" => Ok(Self::Unknown),
            _ => Err(PrivateStoreError::InvalidShape),
        }
    }
}

// Opaque record/operation identities deliberately have no Debug/serialization.
pub struct DueSubscription {
    pub subscription_id: String,
    pub due_at: u64,
}
pub struct ScheduledAttempt {
    pub subscription_id: String,
    pub instance_id: String,
    pub operation_id: String,
}

fn timestamp(value: &Value) -> Result<u64, PrivateStoreError> {
    value
        .as_u64()
        .filter(|v| *v <= MAX_TIME)
        .ok_or(PrivateStoreError::InvalidTimestamp)
}
fn identity(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_graphic()))
}
pub(super) fn validate(value: Option<&Value>) -> Result<(), PrivateStoreError> {
    let Some(value) = value else {
        return Ok(());
    };
    let fields = object(value)?;
    if fields.len() != 3
        || !fields.contains_key("enabled")
        || !fields.contains_key("nextDueAt")
        || !fields.contains_key("lastAttempt")
        || !fields["enabled"].is_boolean()
    {
        return Err(PrivateStoreError::InvalidShape);
    }
    timestamp(&fields["nextDueAt"])?;
    if !fields["lastAttempt"].is_null() {
        let attempt = object(&fields["lastAttempt"])?;
        if attempt.len() != 5
            || ![
                "instanceId",
                "operationId",
                "dueAt",
                "admittedAt",
                "disposition",
            ]
            .iter()
            .all(|k| attempt.contains_key(*k))
            || !identity(&attempt["instanceId"])
            || !identity(&attempt["operationId"])
        {
            return Err(PrivateStoreError::InvalidShape);
        }
        let due = timestamp(&attempt["dueAt"])?;
        let admitted = timestamp(&attempt["admittedAt"])?;
        if due > admitted || timestamp(&fields["nextDueAt"])? <= admitted {
            return Err(PrivateStoreError::InvalidShape);
        }
        ScheduleDisposition::parse(
            attempt["disposition"]
                .as_str()
                .ok_or(PrivateStoreError::InvalidShape)?,
        )?;
    }
    Ok(())
}

fn row(store: &PrivateStore, id: &str) -> Result<usize, PrivateStoreError> {
    store
        .subscriptions
        .iter()
        .position(|s| s.id == id)
        .ok_or(PrivateStoreError::SubscriptionNotFound)
}
fn payload(store: &PrivateStore) -> Result<Vec<u8>, PrivateStoreError> {
    // Revalidate the complete document and the schedule before publication.
    parse_private_store(&store.document.to_string())?.private_payload()
}
impl PrivateStore {
    pub fn due_subscription(&self, now: u64) -> Option<DueSubscription> {
        self.document["subscriptions"]
            .as_array()?
            .iter()
            .filter_map(|s| {
                let schedule = &s["refreshSchedule"];
                let due = schedule["nextDueAt"].as_u64()?;
                (schedule["enabled"] == true && due <= now).then(|| DueSubscription {
                    subscription_id: s["id"].as_str().unwrap_or_default().to_owned(),
                    due_at: due,
                })
            })
            .min_by_key(|s| s.due_at)
    }

    /// Explicit same-user settings read; never part of support diagnostics.
    pub fn subscription_schedule(
        &self,
        id: &str,
        instance: &str,
    ) -> Result<Value, PrivateStoreError> {
        let index = row(self, id)?;
        let value = &self.document["subscriptions"][index]["refreshSchedule"];
        let mut attempt = value["lastAttempt"].clone();
        if !attempt.is_null()
            && attempt["disposition"] == "admitted"
            && attempt["instanceId"] != instance
        {
            attempt["disposition"] = json!("unknown");
        }
        Ok(
            json!({"subscriptionId": id, "enabled": value["enabled"].as_bool().unwrap_or(false),
            "intervalSeconds": 86400, "nextDueAt": value["nextDueAt"], "lastAttempt": attempt}),
        )
    }
}

pub fn apply_subscription_schedule(
    input: &str,
    id: &str,
    enabled: bool,
    now: u64,
) -> Result<(Vec<u8>, bool), PrivateStoreError> {
    let mut store = parse_private_store(input)?;
    let index = row(&store, id)?;
    let previous = &store.document["subscriptions"][index]["refreshSchedule"];
    if previous["enabled"].as_bool().unwrap_or(false) == enabled {
        return Ok((input.as_bytes().to_vec(), false));
    }
    let mut schedule = if previous.is_null() {
        json!({"enabled": false, "nextDueAt": 0, "lastAttempt": null})
    } else {
        previous.clone()
    };
    schedule["enabled"] = json!(enabled);
    if enabled {
        // A new explicit opt-in may create a new slot, never reuse the last one.
        let previous_admission = schedule["lastAttempt"]["admittedAt"].as_u64().unwrap_or(0);
        schedule["nextDueAt"] = json!(now.max(previous_admission.saturating_add(1)));
    }
    store.document["subscriptions"][index]["refreshSchedule"] = schedule;
    Ok((payload(&store)?, true))
}

pub fn admit_subscription_schedule(
    input: &str,
    selected: &DueSubscription,
    attempt: &ScheduledAttempt,
    now: u64,
) -> Result<(Vec<u8>, bool), PrivateStoreError> {
    let mut store = parse_private_store(input)?;
    let index = row(&store, &selected.subscription_id)?;
    let schedule = &mut store.document["subscriptions"][index]["refreshSchedule"];
    if attempt.subscription_id != selected.subscription_id
        || schedule["enabled"] != true
        || schedule["nextDueAt"] != selected.due_at
        || selected.due_at > now
    {
        return Err(PrivateStoreError::SubscriptionChanged);
    }
    let next = now
        .checked_add(DAILY_REFRESH_MILLIS)
        .filter(|v| *v <= MAX_TIME)
        .ok_or(PrivateStoreError::InvalidTimestamp)?;
    schedule["nextDueAt"] = json!(next);
    schedule["lastAttempt"] = json!({"instanceId": attempt.instance_id, "operationId": attempt.operation_id,
        "dueAt": selected.due_at, "admittedAt": now, "disposition": "admitted"});
    Ok((payload(&store)?, true))
}

pub fn finish_subscription_schedule(
    input: &str,
    attempt: &ScheduledAttempt,
    disposition: ScheduleDisposition,
    now: u64,
) -> Result<(Vec<u8>, bool), PrivateStoreError> {
    let mut store = parse_private_store(input)?;
    let index = row(&store, &attempt.subscription_id)?;
    let last = &mut store.document["subscriptions"][index]["refreshSchedule"]["lastAttempt"];
    if last["instanceId"] != attempt.instance_id
        || last["operationId"] != attempt.operation_id
        || last["disposition"] != "admitted"
        || disposition == ScheduleDisposition::Admitted
    {
        return Err(PrivateStoreError::SubscriptionChanged);
    }
    last["disposition"] = json!(disposition.as_str());
    let next = now
        .checked_add(DAILY_REFRESH_MILLIS)
        .filter(|v| *v <= MAX_TIME)
        .ok_or(PrivateStoreError::InvalidTimestamp)?;
    let schedule = &mut store.document["subscriptions"][index]["refreshSchedule"];
    schedule["nextDueAt"] = json!(
        schedule["nextDueAt"]
            .as_u64()
            .ok_or(PrivateStoreError::InvalidShape)?
            .max(next)
    );
    Ok((payload(&store)?, true))
}

impl PrivateSubscriptionMutation {
    /// The successful feed publication and its admission receipt use one rename.
    pub fn finish_scheduled(
        mut self,
        attempt: &ScheduledAttempt,
        now: u64,
    ) -> Result<Self, PrivateStoreError> {
        let input =
            std::str::from_utf8(&self.payload).map_err(|_| PrivateStoreError::InvalidJson)?;
        self.payload =
            finish_subscription_schedule(input, attempt, ScheduleDisposition::Committed, now)?.0;
        self.changed = true;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "10000000-0000-4000-8000-000000000001";
    fn fixture() -> String {
        json!({"version":3,"profiles":[],"subscriptions":[{"id":ID,"name":"Daily","url":"https://fixture.invalid/feed","updatedAt":12,"extension":{"keep":true}}],"activeId":"","lastId":"","startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"}}).to_string()
    }
    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).unwrap()
    }
    fn binding(operation: &str) -> ScheduledAttempt {
        ScheduledAttempt {
            subscription_id: ID.to_owned(),
            instance_id: "first-instance".into(),
            operation_id: operation.into(),
        }
    }
    fn admitted() -> String {
        let enabled = text(
            apply_subscription_schedule(&fixture(), ID, true, 1000)
                .unwrap()
                .0,
        );
        let due = parse_private_store(&enabled)
            .unwrap()
            .due_subscription(1000)
            .unwrap();
        text(
            admit_subscription_schedule(&enabled, &due, &binding("one"), 1000)
                .unwrap()
                .0,
        )
    }
    #[test]
    fn daily_schedule_default_off_and_explicit_settings_preserve_other_intent() {
        let original = fixture();
        let store = parse_private_store(&original).unwrap();
        assert!(store.due_subscription(u64::MAX).is_none());
        assert_eq!(
            store.subscription_schedule(ID, "first-instance").unwrap()["enabled"],
            false
        );
        assert!(
            !apply_subscription_schedule(&original, ID, false, 1000)
                .unwrap()
                .1
        );
        let enabled = text(
            apply_subscription_schedule(&original, ID, true, 1000)
                .unwrap()
                .0,
        );
        let value: Value = serde_json::from_str(&enabled).unwrap();
        let canonical: Value = serde_json::from_slice(
            &parse_private_store(&original)
                .unwrap()
                .private_payload()
                .unwrap(),
        )
        .unwrap();
        for key in ["profiles", "startup", "activeId", "lastId"] {
            assert_eq!(value[key], canonical[key]);
        }
        assert_eq!(value["subscriptions"][0]["extension"], json!({"keep":true}));
        assert!(
            parse_private_store(&enabled)
                .unwrap()
                .due_subscription(999)
                .is_none()
        );
        assert!(
            parse_private_store(&enabled)
                .unwrap()
                .due_subscription(1000)
                .is_some()
        );
        let disabled = text(
            apply_subscription_schedule(&enabled, ID, false, 1001)
                .unwrap()
                .0,
        );
        assert!(
            parse_private_store(&disabled)
                .unwrap()
                .due_subscription(u64::MAX)
                .is_none()
        );
    }
    #[test]
    fn daily_schedule_consumption_survives_reconstruction_and_never_replays_slot() {
        let admitted = admitted();
        let store = parse_private_store(&admitted).unwrap();
        for now in [0, 1000, 1001, DAILY_REFRESH_MILLIS + 999] {
            assert!(store.due_subscription(now).is_none());
        }
        let unknown = store.subscription_schedule(ID, "second-instance").unwrap();
        assert_eq!(unknown["lastAttempt"]["disposition"], "unknown");
        assert_eq!(unknown["lastAttempt"]["operationId"], "one");
        let due = store.due_subscription(10 * DAILY_REFRESH_MILLIS).unwrap();
        assert_eq!(due.due_at, 1000 + DAILY_REFRESH_MILLIS);
        let next = text(
            admit_subscription_schedule(
                &admitted,
                &due,
                &binding("two"),
                10 * DAILY_REFRESH_MILLIS,
            )
            .unwrap()
            .0,
        );
        assert!(
            parse_private_store(&next)
                .unwrap()
                .due_subscription(10 * DAILY_REFRESH_MILLIS)
                .is_none()
        );
        assert_eq!(
            parse_private_store(&next)
                .unwrap()
                .subscription_schedule(ID, "first-instance")
                .unwrap()["nextDueAt"],
            11 * DAILY_REFRESH_MILLIS
        );
        // A missing pre-dispatch consumption really would leave the old slot eligible.
        let unconsumed = text(
            apply_subscription_schedule(&fixture(), ID, true, 1000)
                .unwrap()
                .0,
        );
        assert_eq!(
            parse_private_store(&unconsumed)
                .unwrap()
                .due_subscription(1001)
                .unwrap()
                .due_at,
            1000
        );
    }
    #[test]
    fn daily_schedule_terminal_results_are_bound_and_do_not_rearm() {
        for disposition in [
            ScheduleDisposition::Committed,
            ScheduleDisposition::FailedBeforeCommit,
            ScheduleDisposition::CancelledBeforeCommit,
            ScheduleDisposition::Unknown,
        ] {
            let original = admitted();
            assert!(
                finish_subscription_schedule(&original, &binding("other"), disposition, 1001)
                    .is_err()
            );
            let done = text(
                finish_subscription_schedule(&original, &binding("one"), disposition, 1001)
                    .unwrap()
                    .0,
            );
            assert_eq!(
                parse_private_store(&done)
                    .unwrap()
                    .subscription_schedule(ID, "first-instance")
                    .unwrap()["lastAttempt"]["disposition"],
                disposition.as_str()
            );
            assert!(
                parse_private_store(&done)
                    .unwrap()
                    .due_subscription(1001)
                    .is_none()
            );
            assert!(
                finish_subscription_schedule(&done, &binding("one"), disposition, 1001).is_err()
            );
        }
    }
    #[test]
    fn daily_schedule_completion_after_forward_jump_has_no_catch_up() {
        let input = admitted();
        let now = 10 * DAILY_REFRESH_MILLIS;
        let done = text(
            finish_subscription_schedule(
                &input,
                &binding("one"),
                ScheduleDisposition::Committed,
                now,
            )
            .unwrap()
            .0,
        );
        let store = parse_private_store(&done).unwrap();
        assert!(store.due_subscription(now).is_none());
        assert_eq!(
            store.subscription_schedule(ID, "first-instance").unwrap()["nextDueAt"],
            now + DAILY_REFRESH_MILLIS
        );
    }
    #[test]
    fn daily_schedule_refuses_stale_disabled_malformed_and_overflow() {
        let enabled = text(
            apply_subscription_schedule(&fixture(), ID, true, 1000)
                .unwrap()
                .0,
        );
        let due = parse_private_store(&enabled)
            .unwrap()
            .due_subscription(1000)
            .unwrap();
        assert!(admit_subscription_schedule(&enabled, &due, &binding("one"), 999).is_err());
        assert!(admit_subscription_schedule(&enabled, &due, &binding("one"), u64::MAX).is_err());
        let disabled = text(
            apply_subscription_schedule(&enabled, ID, false, 1000)
                .unwrap()
                .0,
        );
        assert!(admit_subscription_schedule(&disabled, &due, &binding("one"), 1000).is_err());
        for bad in [
            json!({}),
            json!({"enabled":true,"nextDueAt":-1,"lastAttempt":null}),
            json!({"enabled":"true","nextDueAt":1000,"lastAttempt":null}),
            json!({"enabled":true,"nextDueAt":1000,"lastAttempt":null,"url":"private"}),
        ] {
            let mut document: Value = serde_json::from_str(&enabled).unwrap();
            document["subscriptions"][0]["refreshSchedule"] = bad;
            assert!(parse_private_store(&document.to_string()).is_err());
        }
    }
}
