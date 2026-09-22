// SPDX-License-Identifier: MIT
use crate::{
    actions::{self, Command, Kind, Outcome, Request},
    i18n::Locale,
    model::{Actual, Mode, ReadError, Snapshot, Status},
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

pub const FRESH_FOR: Duration = Duration::from_secs(6);

pub struct App {
    pub palette: crate::theme::Palette,
    pub page: crate::inspection::Page,
    pub traffic_rates: Option<(u64, u64)>,
    pub inspection_scroll: u16,
    pub snapshot: Option<Snapshot>,
    pub sampled_at: Option<Instant>,
    pub error: Option<ReadError>,
    pub selected: Option<String>,
    pub query: String,
    pub favorites_only: bool,
    pub searching: bool,
    pub help: bool,
    pub locale: Locale,
    accepted: Option<(String, u64)>,
    pub actions_enabled: bool,
    pub viewport_ready: bool,
    pub confirmation: Option<Confirmation>,
    pub pending: Option<Request>,
    pub running: bool,
    pub unknown: bool,
    pub notice: &'static str,
    minimum_sample: Option<Instant>,
}
pub enum Confirmation {
    New(Command),
    Retry,
    Acknowledge { instance: String, revision: u64 },
}
#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    None,
    Refresh,
    Close,
    Submit,
}

impl App {
    pub fn new(locale: Locale) -> Self {
        Self {
            palette: crate::theme::Palette::default(),
            page: crate::inspection::Page::Profiles,
            traffic_rates: None,
            inspection_scroll: 0,
            snapshot: None,
            sampled_at: None,
            error: None,
            selected: None,
            query: String::new(),
            favorites_only: false,
            searching: false,
            help: false,
            locale,
            accepted: None,
            actions_enabled: false,
            viewport_ready: true,
            confirmation: None,
            pending: None,
            running: false,
            unknown: false,
            notice: "",
            minimum_sample: None,
        }
    }
    pub fn accept(&mut self, mut result: Result<Snapshot, ReadError>, started: Instant) {
        // A pre-command read cannot certify a post-command outcome.
        if self.minimum_sample.is_some_and(|minimum| started < minimum) {
            return;
        }
        if let (Some((instance, revision)), Ok(next)) = (&self.accepted, &result)
            && *instance == next.metadata.instance_id
            && next.revision < *revision
        {
            result = Err(ReadError::Changed);
        }
        match result {
            Ok(next) => {
                self.traffic_rates = self
                    .snapshot
                    .as_ref()
                    .filter(|old| {
                        self.fresh(started)
                            && old.metadata.instance_id == next.metadata.instance_id
                            && old.revision == next.revision
                    })
                    .and_then(|old| next.traffic.as_ref()?.rates(old.traffic.as_ref()?));
                let changed = self
                    .accepted
                    .as_ref()
                    .is_none_or(|(id, _)| *id != next.metadata.instance_id);
                if changed
                    || self
                        .selected
                        .as_ref()
                        .is_some_and(|id| !next.metadata.profiles.iter().any(|p| p.id == *id))
                {
                    self.selected = None;
                }
                self.accepted = Some((next.metadata.instance_id.clone(), next.revision));
                self.snapshot = Some(next);
                if self.selected.as_ref().is_some_and(|id| {
                    !self.visible().iter().any(|i| {
                        self.snapshot
                            .as_ref()
                            .is_some_and(|s| s.metadata.profiles[*i].id == *id)
                    })
                }) {
                    self.selected = None;
                }
                self.sampled_at = Some(started);
                self.error = None;
                if self.notice == "tui.action_applied" {
                    self.notice = "tui.action_complete";
                }
            }
            Err(error) => {
                self.traffic_rates = None;
                self.snapshot = None;
                self.sampled_at = None;
                self.selected = None;
                self.error = Some(error);
            }
        }
    }
    pub fn fresh(&self, now: Instant) -> bool {
        self.sampled_at
            .is_some_and(|then| now.saturating_duration_since(then) < FRESH_FOR)
    }
    pub fn status(&self, now: Instant) -> Status {
        if self.fresh(now) && !self.running {
            self.snapshot
                .as_ref()
                .map_or(Status::Unverified, Snapshot::status)
        } else {
            Status::Unverified
        }
    }
    pub fn visible(&self) -> Vec<usize> {
        self.snapshot.as_ref().map_or_else(Vec::new, |s| {
            crate::browsing::visible(s, &self.query, self.favorites_only)
        })
    }
    pub fn key(&mut self, key: KeyEvent) -> Action {
        self.key_at(key, Instant::now())
    }
    pub fn key_at(&mut self, key: KeyEvent, now: Instant) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Close;
        }
        if self.actions_enabled && !self.viewport_ready {
            if key.code == KeyCode::Char('q') {
                return Action::Close;
            }
            if key.code == KeyCode::Esc {
                self.confirmation = None;
            }
            return Action::None;
        }
        if self.confirmation.is_some() {
            match key.code {
                KeyCode::Esc => self.confirmation = None,
                KeyCode::Char('q') => return Action::Close,
                KeyCode::Enter if key.kind == KeyEventKind::Press => {
                    // Held keys must not both open and confirm a network action.
                    let operation = if matches!(self.confirmation, Some(Confirmation::New(_))) {
                        actions::operation_id()
                    } else {
                        Some(String::new())
                    };
                    if let Some(operation) = operation {
                        return self.confirm(now, operation);
                    }
                    self.notice = "tui.action_rejected";
                }
                _ => {}
            }
            return Action::None;
        }
        if self.help {
            if key.code == KeyCode::Char('q') {
                return Action::Close;
            }
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.help = false;
            }
            return Action::None;
        }
        if self.searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    self.query.pop();
                    self.selected = None;
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && !c.is_control()
                        && self.query.chars().count() < 80 =>
                {
                    self.query
                        .push_str(&crate::model::display(&c.to_string(), 1));
                    self.selected = None;
                }
                _ => {}
            }
            return Action::None;
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) && key.kind == KeyEventKind::Press {
            self.page = self
                .page
                .next(key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT));
            self.inspection_scroll = 0;
            return Action::Refresh;
        }
        if self.page != crate::inspection::Page::Profiles {
            match key.code {
                KeyCode::Char('q') => return Action::Close,
                KeyCode::Char('r') => return Action::Refresh,
                KeyCode::Char('?') => self.help = true,
                KeyCode::Down | KeyCode::Char('j') => {
                    self.inspection_scroll = (self.inspection_scroll + 1).min(24)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.inspection_scroll = self.inspection_scroll.saturating_sub(1)
                }
                KeyCode::Home => self.inspection_scroll = 0,
                KeyCode::Esc => {
                    self.page = crate::inspection::Page::Profiles;
                    return Action::Refresh;
                }
                _ => {}
            }
            return Action::None;
        }
        if self.actions_enabled
            && key.kind == KeyEventKind::Press
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('c') => self.prepare(Kind::Connect, None, now),
                KeyCode::Char('d') => self.prepare(Kind::Disconnect, None, now),
                KeyCode::Char('1') => self.prepare(Kind::Mode, Some(Mode::Global), now),
                KeyCode::Char('2') => self.prepare(Kind::Mode, Some(Mode::Rule), now),
                KeyCode::Char('3') => self.prepare(Kind::Mode, Some(Mode::Direct), now),
                KeyCode::Char('u') if self.unknown && !self.running => {
                    self.confirmation = Some(Confirmation::Retry)
                }
                KeyCode::Char('a') if self.can_acknowledge(now) => {
                    if let Some(s) = &self.snapshot {
                        self.confirmation = Some(Confirmation::Acknowledge {
                            instance: s.metadata.instance_id.clone(),
                            revision: s.revision,
                        });
                    }
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('q') => return Action::Close,
            KeyCode::Char('r') => return Action::Refresh,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('f')
                if key.kind == KeyEventKind::Press
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.favorites_only = !self.favorites_only;
                self.selected = None;
            }
            KeyCode::Esc => {
                self.query.clear();
                self.favorites_only = false;
                self.selected = None;
            }
            KeyCode::Down
            | KeyCode::Up
            | KeyCode::Char('j' | 'k' | 'G')
            | KeyCode::Home
            | KeyCode::End => {
                let rows = self.visible();
                let Some(s) = &self.snapshot else {
                    return Action::None;
                };
                if rows.is_empty() {
                    self.selected = None;
                    return Action::None;
                }
                let old = rows
                    .iter()
                    .position(|i| self.selected.as_ref() == Some(&s.metadata.profiles[*i].id));
                let next = match key.code {
                    KeyCode::Home => 0,
                    KeyCode::End | KeyCode::Char('G') => rows.len() - 1,
                    KeyCode::Up | KeyCode::Char('k') => old.unwrap_or(0).saturating_sub(1),
                    _ => old.map_or(0, |i| (i + 1).min(rows.len() - 1)),
                };
                self.selected = Some(s.metadata.profiles[rows[next]].id.clone());
            }
            _ => {}
        }
        Action::None
    }

    fn eligible(&self, kind: Kind, now: Instant) -> bool {
        self.actions_enabled
            && self.viewport_ready
            && self.pending.is_none()
            && self.fresh(now)
            && self.snapshot.as_ref().is_some_and(|s| {
                s.actions_available
                    && !matches!(
                        s.metadata.last_known_actual,
                        Actual::Starting | Actual::Stopping | Actual::Reconnecting
                    )
                    && (kind == Kind::Disconnect
                        || matches!(s.status(), Status::Connected | Status::Disconnected))
            })
    }

    fn can_acknowledge(&self, now: Instant) -> bool {
        self.unknown
            && !self.running
            && self.fresh(now)
            && self.viewport_ready
            && self.snapshot.as_ref().is_some_and(|s| {
                s.observation.facts.is_some()
                    && !matches!(
                        s.metadata.last_known_actual,
                        Actual::Starting | Actual::Stopping | Actual::Reconnecting
                    )
            })
    }

    pub fn prepare(&mut self, kind: Kind, mode: Option<Mode>, now: Instant) {
        if !self.eligible(kind, now) {
            self.notice = "tui.action_blocked";
            return;
        }
        let Some(s) = &self.snapshot else {
            return;
        };
        // Disconnect and mode always target the runtime's current session, never selection.
        let id = if kind == Kind::Connect {
            self.selected.as_deref().unwrap_or("")
        } else {
            &s.metadata.desired.profile_id
        };
        let profile = s.metadata.profiles.iter().find(|p| p.id == id);
        if kind == Kind::Connect
            && (profile.is_none_or(|p| p.missing)
                || !self
                    .visible()
                    .iter()
                    .any(|i| s.metadata.profiles[*i].id == id))
        {
            self.notice = "tui.select_available";
            return;
        }
        self.confirmation = Some(Confirmation::New(Command {
            instance: s.metadata.instance_id.clone(),
            revision: s.revision,
            kind,
            profile: id.into(),
            name: profile.map_or_else(String::new, |p| p.name.clone()),
            source: profile
                .and_then(|p| p.subscription_id.as_ref())
                .and_then(|id| s.metadata.subscriptions.iter().find(|sub| sub.id == *id))
                .map(|sub| sub.name.clone()),
            was_connected: s.metadata.desired.connected,
            mode: mode.unwrap_or(s.metadata.desired.mode),
        }));
        self.notice = "";
    }

    /// Explicit confirmation only; callers must never automate host authorization.
    pub fn confirm(&mut self, now: Instant, operation: String) -> Action {
        if !self.actions_enabled || !self.viewport_ready {
            return Action::None;
        }
        let Some(confirmation) = self.confirmation.take() else {
            return Action::None;
        };
        match confirmation {
            Confirmation::New(command) => {
                if !self.eligible(command.kind, now)
                    || !self.snapshot.as_ref().is_some_and(|s| {
                        s.metadata.instance_id == command.instance
                            && s.revision == command.revision
                            && (command.kind != Kind::Connect
                                || (self.selected.as_ref() == Some(&command.profile)
                                    && self
                                        .visible()
                                        .iter()
                                        .any(|i| s.metadata.profiles[*i].id == command.profile)
                                    && s.metadata.profiles.iter().any(|p| {
                                        p.id == command.profile
                                            && p.name == command.name
                                            && !p.missing
                                    })))
                    })
                {
                    self.notice = "tui.action_changed";
                    return Action::None;
                }
                self.pending = Request::new(command, operation);
                if self.pending.is_none() {
                    self.notice = "tui.action_rejected";
                    return Action::None;
                }
            }
            Confirmation::Retry => {
                if !self.unknown || self.running || self.pending.is_none() {
                    return Action::None;
                }
                // Retain every byte of the original request, including its old fences.
            }
            Confirmation::Acknowledge { instance, revision } => {
                if self.can_acknowledge(now)
                    && self.snapshot.as_ref().is_some_and(|s| {
                        s.metadata.instance_id == instance && s.revision == revision
                    })
                {
                    self.pending = None;
                    self.unknown = false;
                    self.notice = "tui.action_acknowledged";
                } else {
                    self.notice = "tui.action_changed";
                }
                return Action::None;
            }
        }
        self.running = true;
        self.notice = "tui.action_pending";
        self.sampled_at = None;
        self.minimum_sample = Some(now);
        Action::Submit
    }

    pub fn finish(&mut self, mut outcome: Outcome, now: Instant) {
        if !self.running || self.pending.is_none() {
            return;
        }
        // A rejection of an exact retry (e.g. restarted daemon/evicted receipt)
        // does not prove whether the original request was applied.
        if self.unknown && matches!(outcome, Outcome::Rejected(_)) {
            outcome = Outcome::Unknown;
        }
        self.running = false;
        self.sampled_at = None;
        self.minimum_sample = Some(now);
        self.unknown = outcome == Outcome::Unknown;
        self.notice = match outcome {
            Outcome::Applied => "tui.action_applied",
            Outcome::Rejected(key) => key,
            Outcome::Unknown => "tui.action_unknown",
        };
        if !self.unknown {
            self.pending = None;
        }
    }
}
