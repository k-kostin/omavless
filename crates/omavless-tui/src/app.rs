// SPDX-License-Identifier: MIT
use crate::{
    i18n::Locale,
    model::{ReadError, Snapshot, Status},
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

pub const FRESH_FOR: Duration = Duration::from_secs(6);

pub struct App {
    pub snapshot: Option<Snapshot>,
    pub sampled_at: Option<Instant>,
    pub error: Option<ReadError>,
    pub selected: Option<String>,
    pub query: String,
    pub searching: bool,
    pub help: bool,
    pub locale: Locale,
    accepted: Option<(String, u64)>,
}
#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    None,
    Refresh,
    Close,
}

impl App {
    pub fn new(locale: Locale) -> Self {
        Self {
            snapshot: None,
            sampled_at: None,
            error: None,
            selected: None,
            query: String::new(),
            searching: false,
            help: false,
            locale,
            accepted: None,
        }
    }
    pub fn accept(&mut self, mut result: Result<Snapshot, ReadError>, started: Instant) {
        if let (Some((instance, revision)), Ok(next)) = (&self.accepted, &result)
            && *instance == next.metadata.instance_id
            && next.revision < *revision
        {
            result = Err(ReadError::Changed);
        }
        match result {
            Ok(next) => {
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
                self.sampled_at = Some(started);
                self.error = None;
            }
            Err(error) => {
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
        if self.fresh(now) {
            self.snapshot
                .as_ref()
                .map_or(Status::Unverified, Snapshot::status)
        } else {
            Status::Unverified
        }
    }
    pub fn visible(&self) -> Vec<usize> {
        let query = self.query.to_lowercase();
        self.snapshot.as_ref().map_or_else(Vec::new, |s| {
            s.metadata
                .profiles
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    crate::model::display(&p.name, 80)
                        .to_lowercase()
                        .contains(&query)
                })
                .map(|(i, _)| i)
                .collect()
        })
    }
    pub fn key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Close;
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
        match key.code {
            KeyCode::Char('q') => return Action::Close,
            KeyCode::Char('r') => return Action::Refresh,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Esc => {
                self.query.clear();
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
}
