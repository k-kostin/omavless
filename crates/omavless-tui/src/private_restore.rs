// SPDX-License-Identifier: MIT
//! Experimental Restore client DATA. The runtime owns authentication/admission.
use crate::model::ReadError;
use serde_json::{Value, json};
use std::{
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

const MAX_PATH: usize = 160;
const MAX_SECRET: usize = 1024;
static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Archive,
    Passphrase,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Editing,
    Previewing,
    Confirming,
    Submitted,
    Completed,
    Denied,
    Unknown,
    PreviewUnavailable,
    Changed,
    Cancelled,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preview,
    Restore,
}
struct Credentials {
    archive: Zeroizing<String>,
    passphrase: Zeroizing<String>,
}
struct Preview {
    profiles: u64,
    subscriptions: u64,
    digest: String,
}
/// No Debug/Clone/Serialize: secrets remain private and requests move once.
pub struct Workspace {
    nonce: u64,
    instance: String,
    revision: u64,
    archive: Zeroizing<String>,
    passphrase: Zeroizing<String>,
    credentials: Option<Credentials>,
    preview: Option<Preview>,
    field: Field,
    state: State,
    operation: Option<String>,
    until: Option<Instant>,
    // Worker correlation fence, not backend authority. Cancellation, hidden
    // UI, header loss and clock expiry cannot mean that worker has returned.
    inflight: Option<Phase>,
}
impl Workspace {
    pub fn new(instance: &str, revision: u64) -> Option<Self> {
        if !opaque(instance, 128) || revision >= i64::MAX as u64 {
            return None;
        }
        let nonce = NEXT_WORKSPACE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .ok()?;
        Some(Self {
            nonce,
            instance: instance.into(),
            revision,
            archive: Zeroizing::new(String::with_capacity(MAX_PATH)),
            passphrase: Zeroizing::new(String::with_capacity(MAX_SECRET)),
            credentials: None,
            preview: None,
            field: Field::Archive,
            state: State::Editing,
            operation: None,
            until: None,
            inflight: None,
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn field(&self) -> Field {
        self.field
    }
    pub fn archive(&self) -> &str {
        &self.archive
    }
    pub fn masked(&self) -> &'static str {
        if self.passphrase.is_empty() {
            ""
        } else {
            "********"
        }
    }
    pub fn counts(&self) -> Option<(u64, u64)> {
        self.preview.as_ref().map(|p| (p.profiles, p.subscriptions))
    }
    pub fn unresolved(&self) -> bool {
        self.inflight.is_some()
            || matches!(
                self.state,
                State::Previewing | State::Submitted | State::Unknown
            )
    }
    pub fn next_field(&mut self) {
        if self.state == State::Editing {
            self.field = if self.field == Field::Archive {
                Field::Passphrase
            } else {
                Field::Archive
            };
        }
    }
    pub fn push(&mut self, ch: char) -> bool {
        if self.inflight.is_some()
            || self.state != State::Editing
            || ch.is_control()
            || matches!(ch,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{200e}'|'\u{200f}'|'\u{061c}')
        {
            return false;
        }
        let (text, limit) = if self.field == Field::Archive {
            (&mut self.archive, MAX_PATH)
        } else {
            (&mut self.passphrase, MAX_SECRET)
        };
        if text.len() + ch.len_utf8() > limit {
            return false;
        }
        text.push(ch);
        true
    }
    pub fn backspace(&mut self) {
        if self.state != State::Editing {
            return;
        }
        let (text, limit) = if self.field == Field::Archive {
            (&mut self.archive, MAX_PATH)
        } else {
            (&mut self.passphrase, MAX_SECRET)
        };
        if let Some((end, _)) = text.char_indices().next_back() {
            let mut next = Zeroizing::new(String::with_capacity(limit));
            next.push_str(&text[..end]);
            *text = next;
        }
    }
    fn wipe(&mut self) {
        self.archive.zeroize();
        self.passphrase.zeroize();
        self.credentials = None;
        self.preview = None;
    }
    pub fn cancel(&mut self) {
        if matches!(
            self.state,
            State::Editing | State::Previewing | State::Confirming
        ) {
            self.wipe();
            self.state = State::Cancelled;
        }
    }
    pub fn check_header(&mut self, instance: &str, revision: u64, available: bool) {
        if matches!(
            self.state,
            State::Editing | State::Previewing | State::Confirming
        ) && (!available || instance != self.instance || revision != self.revision)
        {
            self.wipe();
            self.state = State::Changed;
        }
        // A known different daemon cannot make a late original reply current.
        if self.state == State::Submitted && opaque(instance, 128) && instance != self.instance {
            self.state = State::Unknown;
        }
    }
    pub fn begin_preview(&mut self) -> Option<Request> {
        self.begin_preview_at(Instant::now())
    }
    pub(crate) fn begin_preview_at(&mut self, now: Instant) -> Option<Request> {
        if self.state != State::Editing
            || !path_valid(&self.archive)
            || !(12..=MAX_SECRET).contains(&self.passphrase.len())
        {
            return None;
        }
        let until = now.checked_add(Duration::from_secs(120))?;
        self.state = State::Previewing;
        self.inflight = Some(Phase::Preview);
        self.until = Some(until);
        Some(Request {
            nonce: self.nonce,
            instance: self.instance.clone(),
            revision: self.revision,
            phase: Phase::Preview,
            operation: None,
            digest: None,
            credentials: Credentials {
                archive: Zeroizing::new(self.archive.to_string()),
                passphrase: std::mem::take(&mut self.passphrase),
            },
            until,
        })
    }
    pub fn submit(&mut self, operation: String) -> Option<Request> {
        self.submit_at(operation, Instant::now())
    }
    pub(crate) fn submit_at(&mut self, operation: String, now: Instant) -> Option<Request> {
        if self.inflight.is_some() || self.state != State::Confirming || !opaque(&operation, 64) {
            return None;
        }
        let until = now.checked_add(Duration::from_secs(120))?;
        let digest = self.preview.as_ref()?.digest.clone();
        let credentials = self.credentials.take()?;
        self.state = State::Submitted;
        self.inflight = Some(Phase::Restore);
        self.operation = Some(operation.clone());
        self.until = Some(until);
        Some(Request {
            nonce: self.nonce,
            instance: self.instance.clone(),
            revision: self.revision,
            phase: Phase::Restore,
            operation: Some(operation),
            digest: Some(digest),
            credentials,
            until,
        })
    }
    pub fn accept(&mut self, completion: Completion) {
        self.accept_at(completion, Instant::now());
    }
    fn accept_at(&mut self, completion: Completion, now: Instant) {
        self.observe_deadline(now);
        let Some(expected) = self.inflight else {
            return;
        };
        if completion.nonce != self.nonce
            || completion.instance != self.instance
            || completion.revision != self.revision
            || completion.phase != expected
            || Some(completion.until) != self.until
            || completion.operation != self.operation
        {
            // A foreign result cannot release the original worker fence.
            return;
        }
        self.inflight = None;
        if !matches!(
            (self.state, expected),
            (State::Previewing, Phase::Preview) | (State::Submitted, Phase::Restore)
        ) {
            return;
        }
        self.state = completion.state;
        if self.state == State::Confirming {
            self.credentials = completion.credentials;
            self.preview = completion.preview;
        } else {
            self.credentials = None;
            self.passphrase.zeroize();
        }
    }
    pub(crate) fn observe_deadline(&mut self, now: Instant) {
        if self.until.is_some_and(|end| now >= end) {
            match self.state {
                State::Previewing => {
                    self.state = State::PreviewUnavailable;
                    self.credentials = None;
                }
                State::Submitted => self.state = State::Unknown,
                _ => (),
            }
        }
    }
    pub(crate) fn worker_disconnected(&mut self) {
        if let Some(phase) = self.inflight.take() {
            self.credentials = None;
            self.passphrase.zeroize();
            self.state = if phase == Phase::Preview {
                State::PreviewUnavailable
            } else {
                State::Unknown
            };
        }
    }
}
fn opaque(text: &str, limit: usize) -> bool {
    !text.is_empty() && text.len() <= limit && text.bytes().all(|b| (33..=126).contains(&b))
}
fn path_valid(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_PATH
        && Path::new(text).is_absolute()
        && !text.ends_with('/')
        && !text.contains("//")
        && !text.contains("/./")
        && !text.ends_with("/.")
        && !text.chars().any(char::is_control)
        && !Path::new(text)
            .components()
            .any(|p| matches!(p, Component::CurDir | Component::ParentDir))
}
fn digest_valid(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn known_denial(value: &Value) -> bool {
    value["ok"] == false
        && matches!(
            value["error"]["code"].as_str(),
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
}
/// One owned fixed request. JSON/terminal-library temporaries are not all wiped.
pub struct Request {
    nonce: u64,
    instance: String,
    revision: u64,
    phase: Phase,
    operation: Option<String>,
    digest: Option<String>,
    credentials: Credentials,
    until: Instant,
}
pub struct Completion {
    nonce: u64,
    instance: String,
    revision: u64,
    phase: Phase,
    operation: Option<String>,
    until: Instant,
    state: State,
    credentials: Option<Credentials>,
    preview: Option<Preview>,
}
impl Request {
    pub fn method(&self) -> &'static str {
        if self.phase == Phase::Preview {
            "backup.preview"
        } else {
            "backup.restore_previewed"
        }
    }
    pub fn deadline(&self) -> Instant {
        self.until
    }
    pub fn remaining(&self) -> Option<Duration> {
        self.until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    }
    pub fn params(&self) -> Value {
        let mut params = json!({"schema":1,"archive":self.credentials.archive.as_str(),"passphrase":self.credentials.passphrase.as_str(),"instanceId":self.instance,"expectedRevision":self.revision});
        if self.phase == Phase::Restore {
            params["confirmation"] = "replace-previewed-current-private-pair".into();
            params["operationId"] = json!(self.operation);
            params["expectedCiphertextDigest"] = json!(self.digest);
        }
        params
    }
    pub fn settle(self, response: Result<Value, ReadError>) -> Completion {
        self.settle_at(response, Instant::now())
    }
    fn settle_at(self, response: Result<Value, ReadError>, now: Instant) -> Completion {
        let mut preview = None;
        let state = response.ok().filter(|_| now < self.until).map_or(
            if self.phase == Phase::Preview {
                State::PreviewUnavailable
            } else {
                State::Unknown
            },
            |v| {
                if self.phase == Phase::Preview {
                    let result = &v["result"];
                    if v["ok"] == true
                        && v["revision"].as_u64() == Some(self.revision)
                        && result.as_object().is_some_and(|o| o.len() == 4)
                        && result["scope"] == "privatePair"
                        && result["profiles"]
                            .as_u64()
                            .is_some_and(|n| n <= omavless_domain::store::MAX_PROFILES as u64)
                        && result["subscriptions"]
                            .as_u64()
                            .is_some_and(|n| n <= omavless_domain::store::MAX_SUBSCRIPTIONS as u64)
                        && result["ciphertextDigest"]
                            .as_str()
                            .is_some_and(digest_valid)
                    {
                        preview = Some(Preview {
                            profiles: result["profiles"].as_u64().unwrap(),
                            subscriptions: result["subscriptions"].as_u64().unwrap(),
                            digest: result["ciphertextDigest"].as_str().unwrap().into(),
                        });
                        State::Confirming
                    } else {
                        State::PreviewUnavailable
                    }
                } else if v["ok"] == true
                    && v["revision"].as_u64() == self.revision.checked_add(1)
                    && v["result"].as_object().is_some_and(|o| o.len() == 3)
                    && v["result"]["completed"] == true
                    && v["result"]["replayed"] == false
                    && v["result"]["scope"] == "privatePair"
                {
                    State::Completed
                } else if known_denial(&v) {
                    State::Denied
                } else {
                    State::Unknown
                }
            },
        );
        Completion {
            nonce: self.nonce,
            instance: self.instance,
            revision: self.revision,
            phase: self.phase,
            operation: self.operation,
            until: self.until,
            state,
            credentials: if state == State::Confirming {
                Some(self.credentials)
            } else {
                None
            },
            preview,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edit() -> Workspace {
        let mut w = Workspace::new("synthetic", 7).unwrap();
        for ch in "/private/public-fixture.ovb".chars() {
            assert!(w.push(ch));
        }
        w.next_field();
        for ch in "synthetic-only-secret".chars() {
            assert!(w.push(ch));
        }
        w
    }
    fn preview_value(digest: &str) -> Value {
        json!({"ok":true,"revision":7,"result":{"profiles":3,"subscriptions":1,"scope":"privatePair","ciphertextDigest":digest}})
    }
    fn confirmed(now: Instant) -> Workspace {
        let mut w = edit();
        let r = w.begin_preview_at(now).unwrap();
        w.accept_at(r.settle_at(Ok(preview_value(&"a".repeat(64))), now), now);
        assert_eq!(w.state(), State::Confirming);
        w
    }
    fn completed() -> Value {
        json!({"ok":true,"revision":8,"result":{"completed":true,"replayed":false,"scope":"privatePair"}})
    }
    #[test]
    fn preview_is_exact_five_fields_then_restore_binds_immutable_digest_once() {
        let now = Instant::now();
        let mut w = edit();
        let r = w.begin_preview_at(now).unwrap();
        assert_eq!(r.method(), "backup.preview");
        assert_eq!(r.params().as_object().unwrap().len(), 5);
        assert!(r.params().get("operationId").is_none());
        assert!(w.begin_preview_at(now).is_none());
        w.accept_at(r.settle_at(Ok(preview_value(&"a".repeat(64))), now), now);
        assert!(!w.push('x'));
        let r = w.submit_at("one-op".into(), now).unwrap();
        assert_eq!(r.method(), "backup.restore_previewed");
        assert_eq!(r.params().as_object().unwrap().len(), 8);
        assert_eq!(r.params()["expectedCiphertextDigest"], "a".repeat(64));
        assert!(w.submit("another".into()).is_none());
        w.accept_at(r.settle_at(Ok(completed()), now), now);
        assert_eq!(w.state(), State::Completed);
    }
    #[test]
    fn bad_counts_digest_scope_and_wrong_revision_never_confirm() {
        for (key, value) in [
            ("profiles", json!(257)),
            ("subscriptions", json!(65)),
            ("ciphertextDigest", json!("A".repeat(64))),
            ("scope", json!("allSettings")),
        ] {
            let now = Instant::now();
            let mut w = edit();
            let r = w.begin_preview_at(now).unwrap();
            let mut v = preview_value(&"a".repeat(64));
            v["result"][key] = value;
            w.accept_at(r.settle_at(Ok(v), now), now);
            assert_eq!(w.state(), State::PreviewUnavailable);
            assert!(w.submit("never".into()).is_none());
            assert!(w.credentials.is_none());
        }
        let now = Instant::now();
        let mut w = edit();
        let r = w.begin_preview_at(now).unwrap();
        let mut v = preview_value(&"a".repeat(64));
        v["revision"] = 8.into();
        w.accept_at(r.settle_at(Ok(v), now), now);
        assert_eq!(w.state(), State::PreviewUnavailable);
    }
    #[test]
    fn cancellation_stale_context_and_foreign_workspace_destroy_preview_credentials() {
        let now = Instant::now();
        let mut w = confirmed(now);
        w.cancel();
        assert_eq!(w.state(), State::Cancelled);
        assert!(w.credentials.is_none() && w.archive.is_empty());
        let mut w = confirmed(now);
        w.check_header("synthetic", 8, true);
        assert_eq!(w.state(), State::Changed);
        assert!(w.credentials.is_none());
        let mut a = edit();
        let original = a.begin_preview_at(now).unwrap();
        a.cancel();
        let mut b = edit();
        let _own = b.begin_preview_at(now).unwrap();
        b.accept_at(
            original.settle_at(Ok(preview_value(&"a".repeat(64))), now),
            now,
        );
        assert_eq!(b.state(), State::Previewing);
        assert!(b.unresolved()); // foreign result did not release OWN worker
        assert!(b.credentials.is_none());
    }
    #[test]
    fn preview_lost_reply_cannot_restore_and_submit_lost_reply_never_rearms() {
        let mut w = edit();
        let r = w.begin_preview().unwrap();
        w.accept(r.settle(Err(ReadError::Unavailable)));
        assert_eq!(w.state(), State::PreviewUnavailable);
        assert!(w.submit("never".into()).is_none());
        let now = Instant::now();
        let mut w = confirmed(now);
        let r = w.submit_at("one".into(), now).unwrap();
        w.accept_at(r.settle_at(Err(ReadError::Unavailable), now), now);
        assert_eq!(w.state(), State::Unknown);
        w.cancel();
        w.check_header("synthetic", 7, true);
        assert!(w.submit("new".into()).is_none());
        assert!(w.begin_preview().is_none());
    }
    #[test]
    fn response_first_late_delivery_never_wins_deadline() {
        let now = Instant::now();
        let late = now + Duration::from_secs(121);
        let mut w = confirmed(now);
        let r = w.submit_at("once".into(), now).unwrap();
        let result = r.settle_at(Ok(completed()), now);
        w.accept_at(result, late);
        w.observe_deadline(late);
        assert_eq!(w.state(), State::Unknown);
        let mut w = edit();
        let r = w.begin_preview_at(now).unwrap();
        let result = r.settle_at(Ok(preview_value(&"a".repeat(64))), now);
        w.accept_at(result, late);
        assert_eq!(w.state(), State::PreviewUnavailable);
        assert!(w.credentials.is_none());
    }
    #[test]
    fn unavailable_header_does_not_invent_other_daemon_but_known_foreign_seals() {
        let now = Instant::now();
        let mut w = confirmed(now);
        let r = w.submit_at("one".into(), now).unwrap();
        w.check_header("", 0, false);
        assert_eq!(w.state(), State::Submitted);
        w.accept_at(r.settle_at(Ok(completed()), now), now);
        assert_eq!(w.state(), State::Completed);
        let mut w = confirmed(now);
        let r = w.submit_at("one".into(), now).unwrap();
        w.check_header("actual-other", 7, true);
        w.accept_at(r.settle_at(Ok(completed()), now), now);
        assert_eq!(w.state(), State::Unknown);
    }
    #[test]
    fn known_denials_differ_from_manual_recovery_and_replayed_or_old_success() {
        let now = Instant::now();
        for (code, state) in [
            ("conflict", State::Denied),
            ("invalid_argument", State::Denied),
            ("manual_recovery_required", State::Unknown),
        ] {
            let mut w = confirmed(now);
            let r = w.submit_at("once".into(), now).unwrap();
            w.accept_at(
                r.settle_at(
                    Ok(json!({"ok":false,"error":{"code":code,"message":"do-not-render"}})),
                    now,
                ),
                now,
            );
            assert_eq!(w.state(), state);
            assert!(w.submit("new".into()).is_none());
        }
        for change in ["revision", "replay"] {
            let mut w = confirmed(now);
            let r = w.submit_at("once".into(), now).unwrap();
            let mut v = completed();
            if change == "revision" {
                v["revision"] = 7.into();
            } else {
                v["result"]["replayed"] = true.into();
            }
            w.accept_at(r.settle_at(Ok(v), now), now);
            assert_eq!(w.state(), State::Unknown);
        }
    }
    #[test]
    fn bounded_masked_editor_never_formats_secret_or_accepts_unsafe_paths() {
        let mut w = edit();
        assert_eq!(w.masked(), "********");
        assert!(!w.push('\n'));
        assert!(!w.push('\u{202e}'));
        w.backspace();
        assert_eq!(w.masked(), "********");
        for path in ["relative", "/", "/a/../b", "/a//b"] {
            let mut w = Workspace::new("synthetic", 0).unwrap();
            for ch in path.chars() {
                w.push(ch);
            }
            w.next_field();
            for ch in "synthetic-only-secret".chars() {
                w.push(ch);
            }
            assert!(w.begin_preview().is_none());
        }
    }
    #[test]
    fn cancelled_or_expired_preview_keeps_worker_fence_until_exact_completion_or_disconnect() {
        let now = Instant::now();
        let mut w = edit();
        let original = w.begin_preview_at(now).unwrap();
        w.cancel();
        assert_eq!(w.state(), State::Cancelled);
        assert!(w.unresolved());
        assert!(w.archive.is_empty());
        w.observe_deadline(now + Duration::from_secs(121));
        assert!(w.unresolved());
        w.accept_at(original.settle_at(Err(ReadError::Unavailable), now), now);
        assert!(!w.unresolved());
        assert_eq!(w.state(), State::Cancelled);
        assert!(w.credentials.is_none());
        let mut w = edit();
        let _original = w.begin_preview_at(now).unwrap();
        w.worker_disconnected();
        assert!(!w.unresolved());
        assert_eq!(w.state(), State::PreviewUnavailable);
        let mut w = confirmed(now);
        let _original = w.submit_at("one".into(), now).unwrap();
        w.worker_disconnected();
        assert_eq!(w.state(), State::Unknown);
        assert!(w.unresolved());
    }
}
