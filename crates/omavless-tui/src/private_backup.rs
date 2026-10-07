// SPDX-License-Identifier: MIT
//! Opt-in private Backup client state. DATA only; the runtime owns admission.
use crate::model::ReadError;
use serde_json::{Value, json};
use std::path::{Component, Path};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

// This first UI shows the complete target at 70x24; the runtime's separate
// 4096-byte admission stays unchanged. Longer paths use the explicit CLI.
const MAX_PATH: usize = 160;
const MAX_SECRET: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Destination,
    Passphrase,
    Repeat,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Editing,
    Confirming,
    Submitted,
    Completed,
    Denied,
    Unknown,
    Changed,
    Cancelled,
}

// No Debug/Clone/Serialize. Secret buffers reserve their complete byte ceiling
// before entry, avoiding reallocation of an old plaintext backing allocation.
pub struct Workspace {
    instance: String,
    revision: u64,
    destination: Zeroizing<String>,
    passphrase: Zeroizing<String>,
    repeat: Zeroizing<String>,
    field: Field,
    state: State,
    operation: Option<String>,
    until: Option<Instant>,
}
impl Workspace {
    /// These header facts are UI correlation, never backend permission.
    pub fn new(instance: &str, revision: u64) -> Option<Self> {
        if !opaque(instance, 128) || revision > i64::MAX as u64 {
            return None;
        }
        Some(Self {
            instance: instance.into(),
            revision,
            destination: Zeroizing::new(String::with_capacity(MAX_PATH)),
            passphrase: Zeroizing::new(String::with_capacity(MAX_SECRET)),
            repeat: Zeroizing::new(String::with_capacity(MAX_SECRET)),
            field: Field::Destination,
            state: State::Editing,
            operation: None,
            until: None,
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn field(&self) -> Field {
        self.field
    }
    pub fn destination(&self) -> &str {
        &self.destination
    }
    pub fn masked(&self, field: Field) -> &'static str {
        let present = match field {
            Field::Destination => !self.destination.is_empty(),
            Field::Passphrase => !self.passphrase.is_empty(),
            Field::Repeat => !self.repeat.is_empty(),
        };
        if present { "********" } else { "" }
    }
    pub fn next_field(&mut self) {
        if self.state == State::Editing {
            self.field = match self.field {
                Field::Destination => Field::Passphrase,
                Field::Passphrase => Field::Repeat,
                Field::Repeat => Field::Destination,
            };
        }
    }
    pub fn previous_field(&mut self) {
        if self.state == State::Editing {
            self.field = match self.field {
                Field::Destination => Field::Repeat,
                Field::Passphrase => Field::Destination,
                Field::Repeat => Field::Passphrase,
            };
        }
    }
    fn active(&mut self) -> (&mut Zeroizing<String>, usize) {
        match self.field {
            Field::Destination => (&mut self.destination, MAX_PATH),
            Field::Passphrase => (&mut self.passphrase, MAX_SECRET),
            Field::Repeat => (&mut self.repeat, MAX_SECRET),
        }
    }
    pub fn push(&mut self, value: char) -> bool {
        if self.state != State::Editing
            || value.is_control()
            || matches!(value, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{061c}')
        {
            return false;
        }
        let (text, bound) = self.active();
        if text.len() + value.len_utf8() > bound {
            return false;
        }
        text.push(value);
        true
    }
    pub fn backspace(&mut self) {
        if self.state != State::Editing {
            return;
        }
        let (text, bound) = self.active();
        if let Some((end, _)) = text.char_indices().next_back() {
            // Replacing the whole buffer zeroizes the removed suffix as well;
            // String::pop alone would leave that plaintext in unused capacity.
            let mut next = Zeroizing::new(String::with_capacity(bound));
            next.push_str(&text[..end]);
            *text = next;
        }
    }
    pub fn confirm(&mut self) -> bool {
        if self.state != State::Editing
            || !destination_valid(&self.destination)
            || !(12..=MAX_SECRET).contains(&self.passphrase.len())
            || self.passphrase != self.repeat
        {
            return false;
        }
        self.state = State::Confirming;
        true
    }
    fn wipe(&mut self) {
        self.destination.zeroize();
        self.passphrase.zeroize();
        self.repeat.zeroize();
    }
    pub fn cancel(&mut self) {
        if matches!(self.state, State::Editing | State::Confirming) {
            self.wipe();
            self.state = State::Cancelled;
        }
    }
    /// Never silently rebase an editor or revive a submitted/unknown request.
    pub fn check_header(&mut self, instance: &str, revision: u64, available: bool) {
        if matches!(self.state, State::Editing | State::Confirming)
            && (!available || instance != self.instance || revision != self.revision)
        {
            self.wipe();
            self.state = State::Changed;
        }
    }
    pub fn submit(&mut self, operation: String) -> Option<Request> {
        self.submit_at(operation, Instant::now())
    }
    pub(crate) fn submit_at(&mut self, operation: String, now: Instant) -> Option<Request> {
        if self.state != State::Confirming || !opaque(&operation, 64) {
            return None;
        }
        let until = now.checked_add(Duration::from_secs(120))?;
        // Consume before the request can reach even one worker/send boundary.
        self.state = State::Submitted;
        self.operation = Some(operation.clone());
        self.until = Some(until);
        self.repeat.zeroize();
        Some(Request {
            instance: self.instance.clone(),
            revision: self.revision,
            operation,
            destination: Zeroizing::new(self.destination.to_string()),
            passphrase: std::mem::take(&mut self.passphrase),
            until,
        })
    }
    pub fn accept(&mut self, completion: Completion) {
        self.accept_at(completion, Instant::now());
    }
    fn accept_at(&mut self, completion: Completion, now: Instant) {
        // Delivery order cannot bypass the same original deadline. In
        // particular, the event loop may drain a result before its clock tick.
        self.observe_deadline(now);
        if self.state != State::Submitted {
            return;
        }
        self.state = if completion.instance == self.instance
            && completion.revision == self.revision
            && self.operation.as_deref() == Some(completion.operation.as_str())
        {
            completion.state
        } else {
            State::Unknown
        };
        self.passphrase.zeroize();
        self.repeat.zeroize();
    }
    /// Missing/panicked/late worker completion never admits a fresh request.
    /// This is local uncertainty, not cancellation or backend compensation.
    pub(crate) fn observe_deadline(&mut self, now: Instant) {
        if self.state == State::Submitted && self.until.is_some_and(|until| now >= until) {
            self.state = State::Unknown;
        }
    }
}

fn opaque(value: &str, bound: usize) -> bool {
    !value.is_empty() && value.len() <= bound && value.bytes().all(|b| (33..=126).contains(&b))
}
fn destination_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PATH
        && Path::new(value).is_absolute()
        && !value.ends_with('/')
        && !value.contains("//")
        && !value.contains("/./")
        && !value.ends_with("/.")
        && !value.chars().any(char::is_control)
        && !Path::new(value)
            .components()
            .any(|p| matches!(p, Component::CurDir | Component::ParentDir))
}

/// One moved original request, not the ordinary cloneable action/job container.
/// params() creates JSON temporaries; their complete wiping is not guaranteed.
pub struct Request {
    instance: String,
    revision: u64,
    operation: String,
    destination: Zeroizing<String>,
    passphrase: Zeroizing<String>,
    until: Instant,
}
pub struct Completion {
    instance: String,
    revision: u64,
    operation: String,
    state: State,
}
impl Request {
    /// Immutable clock DATA only, not a caller-selected timeout or permission.
    pub fn deadline(&self) -> Instant {
        self.until
    }
    pub fn remaining(&self) -> Option<Duration> {
        self.until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    }
    pub fn params(&self) -> Value {
        json!({"schema":1,"archive":self.destination.as_str(),
            "passphrase":self.passphrase.as_str(),"confirmation":"export-current-private-pair",
            "instanceId":self.instance,"operationId":self.operation,"expectedRevision":self.revision})
    }
    /// Transport validates framing/ID/peer. Correlation stays in this original
    /// request because the existing success result has no instance/operation.
    pub fn settle(self, response: Result<Value, ReadError>) -> Completion {
        self.settle_at(response, Instant::now())
    }
    fn settle_at(self, response: Result<Value, ReadError>, now: Instant) -> Completion {
        let state = response
            .ok()
            .filter(|_| now < self.until)
            .map_or(State::Unknown, |v| {
                if v["ok"] == true
                    && v["revision"].as_u64() == Some(self.revision)
                    && v["result"].as_object().is_some_and(|o| o.len() == 3)
                    && v["result"]["completed"] == true
                    && v["result"]["replayed"] == false
                    && v["result"]["scope"] == "privatePair"
                {
                    State::Completed
                } else if v["ok"] == false
                    && matches!(
                        v["error"]["code"].as_str(),
                        Some(
                            "invalid_request"
                                | "invalid_argument"
                                | "busy"
                                | "conflict"
                                | "daemon_restarting"
                                | "capability_unavailable"
                                | "unknown_method"
                        )
                    )
                {
                    State::Denied
                } else {
                    State::Unknown
                }
            });
        Completion {
            instance: self.instance,
            revision: self.revision,
            operation: self.operation,
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edit(path: &str, secret: &str, repeated: &str) -> Workspace {
        let mut w = Workspace::new("synthetic-owner", 7).unwrap();
        for ch in path.chars() {
            assert!(w.push(ch));
        }
        w.next_field();
        for ch in secret.chars() {
            assert!(w.push(ch));
        }
        w.next_field();
        for ch in repeated.chars() {
            assert!(w.push(ch));
        }
        w
    }
    fn ready() -> Workspace {
        edit(
            "/private/synthetic.ovb",
            "synthetic-secret",
            "synthetic-secret",
        )
    }
    fn sent() -> (Workspace, Request) {
        let mut w = ready();
        assert!(w.confirm());
        let request = w.submit("synthetic-once".into()).unwrap();
        (w, request)
    }
    #[test]
    fn masked_bounded_editor_and_cancel_wipe_without_submission() {
        let mut w = ready();
        assert_eq!(w.masked(Field::Passphrase), "********");
        assert!(!w.push('\n'));
        w.cancel();
        assert_eq!(w.state(), State::Cancelled);
        assert!(w.destination.is_empty() && w.passphrase.is_empty() && w.repeat.is_empty());
        assert!(w.submit("never".into()).is_none());
    }
    #[test]
    fn unsafe_paths_mismatch_and_short_secrets_never_confirm() {
        for path in [
            "relative.ovb",
            "/",
            "/private/../a",
            "/private//a",
            "/private/./a",
            "/private/.",
        ] {
            assert!(!edit(path, "synthetic-secret", "synthetic-secret").confirm());
        }
        assert!(!edit("/private/a", "short", "short").confirm());
        assert!(!edit("/private/a", "synthetic-secret", "different-secret").confirm());
    }
    #[test]
    fn character_byte_limits_and_backspace_keep_masked_suffix_private() {
        let mut w = Workspace::new("original", 0).unwrap();
        w.next_field();
        for _ in 0..512 {
            assert!(w.push('я'));
        }
        assert!(!w.push('x'));
        w.backspace();
        assert_eq!(w.passphrase.len(), 1022);
        assert_eq!(w.masked(Field::Passphrase), "********");
    }
    #[test]
    fn changed_or_unavailable_context_cannot_rearm_confirmation() {
        for (id, rev, available) in [
            ("other", 7, true),
            ("synthetic-owner", 8, true),
            ("synthetic-owner", 7, false),
        ] {
            let mut w = ready();
            assert!(w.confirm());
            w.check_header(id, rev, available);
            assert_eq!(w.state(), State::Changed);
            assert!(w.submit("once".into()).is_none());
            w.check_header("synthetic-owner", 7, true);
            assert!(!w.confirm());
        }
    }
    #[test]
    fn exact_seven_params_one_submit_no_second_enter_or_secret_retention() {
        let (mut w, request) = sent();
        let v = request.params();
        assert_eq!(v.as_object().unwrap().len(), 7);
        assert_eq!(v["confirmation"], "export-current-private-pair");
        assert_eq!(v["expectedRevision"], 7);
        assert_eq!(v["operationId"], "synthetic-once");
        assert_eq!(w.destination(), "/private/synthetic.ovb");
        assert!(w.passphrase.is_empty() && w.repeat.is_empty());
        assert!(w.submit("second".into()).is_none());
        w.cancel();
        assert_eq!(w.state(), State::Submitted);
    }
    #[test]
    fn exact_completed_reply_and_known_exclusive_denial_are_distinct() {
        for (v, expected) in [
            (
                json!({"ok":true,"revision":7,"result":{"completed":true,"replayed":false,"scope":"privatePair"}}),
                State::Completed,
            ),
            (
                json!({"ok":false,"revision":7,"error":{"code":"conflict","message":"must not render"}}),
                State::Denied,
            ),
        ] {
            let (mut w, r) = sent();
            w.accept(r.settle(Ok(v)));
            assert_eq!(w.state(), expected);
            assert!(w.submit("repeat".into()).is_none());
        }
    }
    #[test]
    fn transport_backend_unrecognized_late_or_replayed_reply_remain_unknown() {
        for v in [
            json!({"ok":false,"error":{"code":"manual_recovery_required"}}),
            json!({"ok":false,"error":{"code":"future"}}),
            json!({"ok":true,"revision":8,"result":{"completed":true,"replayed":false,"scope":"privatePair"}}),
            json!({"ok":true,"revision":7,"result":{"completed":true,"replayed":true,"scope":"privatePair"}}),
            json!({"ok":true,"revision":7,"result":{"completed":true,"scope":"privatePair"}}),
        ] {
            let (mut w, r) = sent();
            w.accept(r.settle(Ok(v)));
            assert_eq!(w.state(), State::Unknown);
            assert!(!w.confirm());
            assert!(w.submit("fresh-id".into()).is_none());
        }
        let (mut w, r) = sent();
        w.accept(r.settle(Err(ReadError::Unavailable)));
        assert_eq!(w.state(), State::Unknown);
    }
    #[test]
    fn foreign_worker_completion_and_cancelled_editor_cannot_publish_success() {
        let (mut w, _) = sent();
        w.accept(Completion {
            instance: "other".into(),
            revision: 7,
            operation: "synthetic-once".into(),
            state: State::Completed,
        });
        assert_eq!(w.state(), State::Unknown);
        let mut w = ready();
        w.cancel();
        w.accept(Completion {
            instance: "synthetic-owner".into(),
            revision: 7,
            operation: "synthetic-once".into(),
            state: State::Completed,
        });
        assert_eq!(w.state(), State::Cancelled);
    }
    #[test]
    fn original_deadline_cannot_be_rebased_by_late_worker_result() {
        let mut w = ready();
        assert!(w.confirm());
        let began = Instant::now();
        let r = w.submit_at("original".into(), began).unwrap();
        let completion = r.settle_at(Ok(json!({"ok":true,"revision":7,"result":{"completed":true,"replayed":false,"scope":"privatePair"}})), began + Duration::from_secs(120));
        w.accept(completion);
        assert_eq!(w.state(), State::Unknown);
    }
    #[test]
    fn missing_worker_or_delayed_delivery_stays_unknown_after_original_deadline() {
        let mut w = ready();
        assert!(w.confirm());
        let began = Instant::now();
        let request = w.submit_at("one".into(), began).unwrap();
        let result = request.settle_at(Ok(json!({"ok":true,"revision":7,"result":{"completed":true,"replayed":false,"scope":"privatePair"}})), began);
        w.observe_deadline(began + Duration::from_secs(120));
        w.accept(result);
        assert_eq!(w.state(), State::Unknown);
        assert!(w.submit("new-id".into()).is_none());
    }
    #[test]
    fn production_response_first_late_delivery_cannot_beat_deadline_observation() {
        let mut w = ready();
        assert!(w.confirm());
        let began = Instant::now();
        let request = w.submit_at("original".into(), began).unwrap();
        let result = request.settle_at(Ok(json!({"ok":true,"revision":7,"result":{"completed":true,"replayed":false,"scope":"privatePair"}})), began);
        w.accept_at(result, began + Duration::from_secs(120));
        w.observe_deadline(began + Duration::from_secs(120));
        assert_eq!(w.state(), State::Unknown);
        assert!(w.submit("new-id".into()).is_none());
    }
}
