// SPDX-License-Identifier: MIT
//! Private client input and confirmation, not a second import parser or owner.
use crate::model::{ReadError, opaque};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

pub const MAX_INPUT: usize = 32 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Editing,
    Classifying,
    Preview,
    Submitting,
    Completed,
    Rejected,
    Changed,
    Unknown,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Profile,
    Subscription,
    Bundle,
}
pub struct Preview {
    pub kind: Kind,
    pub name: String,
    pub accepted: u16,
    pub duplicates: u16,
    pub skipped: u16,
    pub warning: bool,
    fingerprint: String,
}
fn name(value: &Value, fallback: &str) -> Option<String> {
    let text = value.as_str()?;
    if text.len() > 320 || text.chars().count() > 80 || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(if text.trim().is_empty() {
        fallback.into()
    } else {
        text.trim().into()
    })
}
fn preview(value: &Value, revision: u64) -> Option<Preview> {
    if value["ok"] != true || value["revision"].as_u64()? != revision {
        return None;
    }
    let r = &value["result"];
    if r["version"] != 1 {
        return None;
    }
    let (kind, label, accepted, duplicates, skipped, fingerprint) = match r["kind"].as_str()? {
        "profile" if r.as_object()?.len() == 3 => {
            let p = &r["profile"];
            if p["version"] != 1
                || !matches!(
                    p["protocol"].as_str()?,
                    "vless" | "trojan" | "hysteria2" | "tuic"
                )
            {
                return None;
            }
            (
                Kind::Profile,
                name(&p["suggestedName"], "Profile")?,
                1,
                0,
                0,
                String::new(),
            )
        }
        "subscription" if r.as_object()?.len() == 4 => {
            if r["duplicate"].as_bool()? {
                return None;
            }
            (
                Kind::Subscription,
                name(&r["suggestedName"], "Subscription")?,
                1,
                0,
                0,
                String::new(),
            )
        }
        "bundle" if r.as_object()?.len() == 4 => {
            let c = &r["counts"];
            if c.as_object()?.len() != 3 {
                return None;
            }
            let a = c["accepted"].as_u64().filter(|n| *n <= 256)? as u16;
            let d = c["duplicates"].as_u64().filter(|n| *n <= 1024)? as u16;
            let s = c["skipped"].as_u64().filter(|n| *n <= 1024)? as u16;
            if !(1..=1024).contains(&(u32::from(a) + u32::from(d) + u32::from(s))) {
                return None;
            }
            let fingerprint = r["previewFingerprint"].as_str()?.to_owned();
            if fingerprint.len() != 64
                || !fingerprint
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            (Kind::Bundle, String::new(), a, d, s, fingerprint)
        }
        _ => return None,
    };
    let warning = kind == Kind::Profile
        && (r["profile"]["insecure"].as_bool()? || r["profile"]["experimental"].as_bool()?);
    Some(Preview {
        kind,
        name: label,
        accepted,
        duplicates,
        skipped,
        warning,
        fingerprint,
    })
}

/// No Debug/Serialize/Clone: private inputs may cross only the fixed adapter.
pub struct Request {
    instance: String,
    revision: u64,
    serial: u64,
    input: Zeroizing<String>,
    operation: Option<String>,
    kind: Option<Kind>,
    name: String,
    fingerprint: String,
}
impl Request {
    pub fn instance(&self) -> &str {
        &self.instance
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn mutation(&self) -> bool {
        self.operation.is_some()
    }
    pub fn method(&self) -> &'static str {
        if self.mutation() {
            "plugin.action"
        } else {
            "imports.classify"
        }
    }
    /// Original client queue/preflight refusal, before any mutation was sent.
    pub fn not_sent(self) -> Completion {
        let result = if self.mutation() {
            ResultValue::Saved(Saved::Rejected)
        } else {
            ResultValue::Preview(None)
        };
        Completion {
            serial: self.serial,
            result,
        }
    }
    pub fn params(&self) -> Value {
        if !self.mutation() {
            return json!({"input": self.input.as_str()});
        }
        let mut value = json!({"instanceId": self.instance, "expectedRevision": self.revision,
        "operationId": self.operation, "action": match self.kind {
            Some(Kind::Profile) => "profile-import",
            Some(Kind::Subscription) => "subscription-add",
            Some(Kind::Bundle) => "profile-bundle-import",
            None => "",
        }});
        match self.kind {
            Some(Kind::Subscription) => {
                value["url"] = json!(self.input.trim());
                value["name"] = json!(self.name);
            }
            Some(Kind::Profile) => {
                value["input"] = json!(self.input.as_str());
                value["name"] = json!(self.name);
            }
            Some(Kind::Bundle) => {
                value["input"] = json!(self.input.as_str());
                value["previewFingerprint"] = json!(self.fingerprint);
            }
            None => (),
        }
        value
    }
    /// Discard raw errors and private response fields before returning to UI.
    pub fn settle(self, response: Result<Value, ReadError>) -> Completion {
        let result = if let Some(operation) = &self.operation {
            let mut outcome = Saved::Unknown;
            if let Ok(value) = response {
                let r = &value["result"];
                if value["ok"] == true
                    && r.as_object().is_some_and(|o| o.len() == 5)
                    && r["schemaVersion"] == 1
                    && r["applied"] == true
                    && r["instanceId"] == self.instance
                    && r["operationId"] == *operation
                    && r["action"].as_str()
                        == Some(match self.kind {
                            Some(Kind::Profile) => "profile-import",
                            Some(Kind::Subscription) => "subscription-add",
                            Some(Kind::Bundle) => "profile-bundle-import",
                            None => "",
                        })
                    && value["revision"]
                        .as_u64()
                        .is_some_and(|n| n >= self.revision && n <= i64::MAX as u64)
                {
                    outcome = Saved::Applied(value["revision"].as_u64().unwrap_or(self.revision));
                } else if value["ok"] == false
                    && value["revision"]
                        .as_u64()
                        .is_some_and(|n| n <= i64::MAX as u64)
                    && (matches!(
                        value["error"]["code"].as_str(),
                        Some(
                            "conflict"
                                | "busy"
                                | "invalid_argument"
                                | "not_found"
                                | "capability_unavailable"
                                | "permission_denied"
                                | "manual_recovery_required"
                                | "daemon_restarting"
                        )
                    ) || (self.kind == Some(Kind::Subscription)
                        && value["error"]["code"] == "subscription_unavailable"))
                {
                    outcome = Saved::Rejected;
                }
            }
            ResultValue::Saved(outcome)
        } else {
            ResultValue::Preview(response.ok().and_then(|v| preview(&v, self.revision)))
        };
        Completion {
            serial: self.serial,
            result,
        }
    }
}
enum Saved {
    Applied(u64),
    Rejected,
    Unknown,
}
enum ResultValue {
    Preview(Option<Preview>),
    Saved(Saved),
}
pub struct Completion {
    serial: u64,
    result: ResultValue,
}

/// Bounded private draft; UI cancellation never cancels a submitted mutation.
pub struct Workspace {
    input: Zeroizing<String>,
    instance: String,
    revision: u64,
    serial: u64,
    state: State,
    preview: Option<Preview>,
    deadline: Option<Instant>,
    applied_revision: Option<u64>,
}
impl Workspace {
    pub fn new(instance: &str, revision: u64, serial: u64) -> Option<Self> {
        if !opaque(instance) || revision > i64::MAX as u64 {
            return None;
        }
        Some(Self {
            input: Zeroizing::new(String::with_capacity(MAX_INPUT)),
            instance: instance.into(),
            revision,
            serial,
            state: State::Editing,
            preview: None,
            deadline: None,
            applied_revision: None,
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn input_bytes(&self) -> usize {
        self.input.len()
    }
    pub fn preview(&self) -> Option<&Preview> {
        self.preview.as_ref()
    }
    pub fn applied_context(&self) -> Option<(&str, u64)> {
        if self.state != State::Completed {
            return None;
        }
        Some((&self.instance, self.applied_revision?))
    }
    pub fn blocks_actions(&self) -> bool {
        matches!(self.state, State::Submitting | State::Unknown)
    }
    pub fn worker_lost(&mut self) {
        if self.state == State::Submitting {
            self.input.zeroize();
            self.state = State::Unknown;
        } else if self.state == State::Classifying {
            self.cancel();
            self.state = State::Rejected;
        }
    }
    pub fn append(&mut self, text: &str) -> bool {
        if self.state != State::Editing
            || self.input.len().saturating_add(text.len()) > MAX_INPUT
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return false;
        }
        self.input.push_str(text);
        true
    }
    pub fn backspace(&mut self) {
        if self.state == State::Editing
            && let Some((end, _)) = self.input.char_indices().next_back()
        {
            let mut next = Zeroizing::new(String::with_capacity(MAX_INPUT));
            next.push_str(&self.input[..end]);
            self.input = next;
        }
    }
    pub fn clear(&mut self) {
        if self.state == State::Editing {
            self.input.zeroize();
        }
    }
    pub fn cancel(&mut self) {
        self.input.zeroize();
        if !self.blocks_actions() {
            self.state = State::Changed;
            self.preview = None;
            self.deadline = None;
        }
    }
    pub fn observe(&mut self, instance: &str, revision: u64, available: bool, now: Instant) {
        let mismatch = self.instance != instance
            || (self.state != State::Submitting && self.revision != revision);
        if matches!(
            self.state,
            State::Editing | State::Classifying | State::Preview
        ) && (!available || mismatch)
        {
            self.cancel();
        } else if self.state == State::Submitting && self.instance != instance {
            self.input.zeroize();
            self.state = State::Unknown;
        }
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            match self.state {
                State::Classifying => {
                    self.cancel();
                    self.state = State::Rejected;
                }
                State::Submitting => {
                    self.input.zeroize();
                    self.state = State::Unknown;
                }
                _ => (),
            }
        }
    }
    pub fn classify(&mut self, now: Instant) -> Option<Request> {
        if self.state != State::Editing || self.input.trim().is_empty() {
            return None;
        }
        self.state = State::Classifying;
        self.deadline = now.checked_add(Duration::from_secs(15));
        Some(self.request(None))
    }
    fn request(&self, operation: Option<String>) -> Request {
        Request {
            instance: self.instance.clone(),
            revision: self.revision,
            serial: self.serial,
            input: self.input.clone(),
            operation,
            kind: self.preview.as_ref().map(|p| p.kind),
            name: self
                .preview
                .as_ref()
                .map_or_else(String::new, |p| p.name.clone()),
            fingerprint: self
                .preview
                .as_ref()
                .map_or_else(String::new, |p| p.fingerprint.clone()),
        }
    }
    pub fn confirm(&mut self, operation: String, now: Instant) -> Option<Request> {
        if self.state != State::Preview
            || self.preview.as_ref()?.accepted == 0
            || !opaque(&operation)
        {
            return None;
        }
        let request = self.request(Some(operation));
        self.state = State::Submitting;
        self.input.zeroize();
        self.deadline = now.checked_add(Duration::from_secs(130));
        Some(request)
    }
    pub fn accept(&mut self, completion: Completion) {
        if completion.serial != self.serial {
            return;
        }
        match (self.state, completion.result) {
            (State::Classifying, ResultValue::Preview(Some(value))) => {
                self.preview = Some(value);
                self.state = State::Preview;
            }
            (State::Classifying, ResultValue::Preview(None)) => {
                self.cancel();
                self.state = State::Rejected;
            }
            (State::Submitting, ResultValue::Saved(outcome)) => {
                self.state = match outcome {
                    Saved::Applied(revision) => {
                        self.applied_revision = Some(revision);
                        State::Completed
                    }
                    Saved::Rejected => State::Rejected,
                    Saved::Unknown => State::Unknown,
                };
                self.preview = None;
            }
            _ => return, // Cancelled/unknown contexts cannot be revived by late responses.
        }
        self.deadline = None;
    }
}
