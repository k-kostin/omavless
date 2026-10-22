// SPDX-License-Identifier: MIT
use crate::{
    actions,
    app::{Action, App},
    model::Status,
    private_import::{Completion, Request, State, Workspace},
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Instant;
use zeroize::Zeroizing;

impl App {
    fn import_context_available(&self, now: Instant) -> bool {
        let available = self.import_enabled
            && self.actions_enabled
            && self.viewport_ready
            && self.fresh(now)
            && self.error.is_none()
            && !self.running
            && !self.unknown
            && self.pending.is_none()
            && !self.job.as_ref().is_some_and(|j| j.blocks_actions())
            && self.snapshot.as_ref().is_some_and(|s| {
                s.capabilities.private_import
                    && matches!(s.status(), Status::Connected | Status::Disconnected)
            });
        #[cfg(feature = "private-backup")]
        let available = available
            && !self.backup_unresolved()
            && !self.restore_unresolved()
            && !self.backup_open
            && !self.restore_open;
        available
    }
    pub fn import_available(&self, now: Instant) -> bool {
        self.import_context_available(now) && !self.import_unresolved()
    }
    pub fn import_unresolved(&self) -> bool {
        self.import.as_ref().is_some_and(|w| w.blocks_actions())
    }
    pub(crate) fn open_import(&mut self, now: Instant) {
        if !self.import_available(now) {
            self.notice = "tui.import_unavailable";
            return;
        }
        let Some(serial) = self.import_serial.checked_add(1) else {
            return;
        };
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        self.import = Workspace::new(&snapshot.metadata.instance_id, snapshot.revision, serial);
        self.import_summary = None;
        self.import_reasons = None;
        self.import_serial = serial;
        self.import_open = self.import.is_some();
        self.notice = "";
    }
    pub(crate) fn refresh_import_context(&mut self, now: Instant) {
        let available = self.import_context_available(now);
        let (instance, revision) = self
            .snapshot
            .as_ref()
            .map_or(("", 0), |s| (s.metadata.instance_id.as_str(), s.revision));
        if let Some(w) = &mut self.import {
            w.observe(instance, revision, available, now);
        }
    }
    pub(crate) fn import_key(&mut self, key: KeyEvent, now: Instant) -> Action {
        if key.code == KeyCode::Esc {
            if let Some(w) = &mut self.import {
                w.cancel();
            }
            self.import_open = false;
            self.notice = if self.import_unresolved() {
                self.import_notice_key()
            } else {
                ""
            };
            return Action::None;
        }
        if !self.viewport_ready || key.kind != KeyEventKind::Press {
            return Action::None;
        }
        // Ctrl+N has an unambiguous traditional terminal encoding; modified
        // Enter is supported opportunistically but not advertised as portable.
        if key.code == KeyCode::Char('n') && key.modifiers == KeyModifiers::CONTROL {
            if let Some(w) = &mut self.import {
                w.append("\n");
            }
            return Action::None;
        }
        if key.code == KeyCode::Char('u') && key.modifiers == KeyModifiers::CONTROL {
            if let Some(w) = &mut self.import {
                w.clear();
            }
            return Action::None;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Action::None;
        }
        let Some(w) = &mut self.import else {
            return Action::None;
        };
        match key.code {
            KeyCode::Char(ch) if w.state() == State::Editing => {
                if !w.append(&ch.to_string()) {
                    self.notice = "tui.import_input_invalid";
                }
            }
            KeyCode::Backspace => w.backspace(),
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                w.append("\n");
            }
            KeyCode::Enter if w.state() == State::Editing => {
                self.import_request = w.classify(now);
                if self.import_request.is_some() {
                    self.notice = "";
                    return Action::SubmitImport;
                }
                self.notice = "tui.import_input_invalid";
            }
            KeyCode::Enter if w.state() == State::Preview => {
                if let Some(operation) = actions::operation_id() {
                    self.import_request = w.confirm(operation, now);
                }
                if self.import_request.is_some() {
                    self.minimum_sample = Some(now);
                    return Action::SubmitImport;
                }
            }
            KeyCode::Char('q') => return Action::Close,
            _ => (),
        }
        Action::None
    }
    /// Explicit terminal Paste event only; never queries the OS clipboard.
    pub fn import_paste(&mut self, text: String) {
        self.paste_input(Zeroizing::new(text), Instant::now());
    }
    pub fn paste_input(&mut self, private: Zeroizing<String>, now: Instant) {
        if private.len() > crate::private_import::MAX_INPUT {
            self.notice = "tui.paste_too_large";
            return;
        }
        self.refresh_import_context(now);
        if self.import_open {
            if let Some(w) = &mut self.import
                && !w.append(&private)
            {
                self.notice = "tui.import_input_invalid";
            }
            return;
        }
        #[cfg(feature = "private-backup")]
        {
            if self.backup_open {
                if let Some(w) = &mut self.backup {
                    w.paste(&private);
                }
                return;
            }
            if self.restore_open {
                if let Some(w) = &mut self.restore {
                    w.paste(&private);
                }
                return;
            }
        }
        // Never replay pasted commands, shortcuts, Escape or Enter. Only
        // already-active existing editors receive their own character checks.
        if !self.import_unresolved() && (self.searching || self.route_editing) {
            for ch in private.chars().filter(|ch| !ch.is_control()) {
                self.key_at(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), now);
            }
        }
    }
    pub fn take_import_request(&mut self) -> Option<Request> {
        self.import_request.take()
    }
    pub(crate) fn import_notice_key(&self) -> &'static str {
        if self
            .import
            .as_ref()
            .is_some_and(|w| w.state() == State::Submitting)
        {
            "tui.import_pending_entry"
        } else {
            "tui.import_unknown"
        }
    }
    pub(crate) fn import_footer_key(&self) -> &'static str {
        if self
            .import
            .as_ref()
            .is_some_and(|w| w.state() == State::Submitting)
        {
            "tui.import_pending_footer"
        } else {
            "tui.import_unknown_footer"
        }
    }
    pub(crate) fn import_worker_lost(&mut self) {
        self.import_enabled = false;
        if let Some(w) = &mut self.import {
            w.worker_lost();
        }
    }
    pub fn finish_import(&mut self, completion: Completion) {
        self.refresh_import_context(Instant::now());
        if let Some(w) = &mut self.import {
            w.accept(completion);
            match w.state() {
                State::Completed => {
                    self.notice = "tui.import_saved";
                    self.minimum_sample = Some(Instant::now());
                    self.sampled_at = None;
                    if let Some((instance, revision)) = w.applied_context() {
                        self.accepted = Some((instance.into(), revision));
                    }
                }
                State::Rejected => self.notice = "tui.import_rejected",
                State::Unknown => self.notice = "tui.import_unknown",
                _ => (),
            }
        }
    }
}
