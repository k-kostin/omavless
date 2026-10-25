// SPDX-License-Identifier: MIT
//! Explicit Product-only set client; shares transport/row sanitization, never Single Confirm.
use super::*;
const SET_SCOPE: &str = "development_owned_connection_set_close";
const SET_METHODS: [&str; 4] = [
    "development.connection_sets.snapshot",
    "development.connection_sets.prepare",
    "development.connection_sets.confirm",
    "development.connection_sets.receipt",
];

impl Workspace {
    #[doc(hidden)]
    pub fn selected_set_result(&self) -> Option<(&'static str, &[&'static str])> {
        (self.set_mode && !self.set_results.is_empty())
            .then(|| (self.locale.text(self.notice), self.set_results.as_slice()))
    }

    pub fn new_selected_set(locale: Locale) -> Self {
        let mut result = Self::new(locale);
        result.set_mode = true;
        result
    }
    pub(super) fn workspace_scope(&self) -> &'static str {
        if self.set_mode { SET_SCOPE } else { SCOPE }
    }
    fn set_scroll_max(&self) -> u16 {
        let (width, height) = self.set_viewport.get();
        Paragraph::new(self.set_text())
            .wrap(Wrap { trim: false })
            .line_count(width.saturating_sub(2).max(1))
            .saturating_sub(usize::from(height.saturating_sub(14)))
            .min(usize::from(u16::MAX)) as u16
    }
    fn set_scroll_key(&mut self, key: KeyCode) -> bool {
        let max = self.set_scroll_max();
        self.set_scroll = self.set_scroll.min(max);
        match key {
            KeyCode::Down | KeyCode::Char('j') => {
                self.set_scroll = self.set_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.set_scroll = self.set_scroll.saturating_sub(1),
            KeyCode::PageDown => self.set_scroll = self.set_scroll.saturating_add(8),
            KeyCode::PageUp => self.set_scroll = self.set_scroll.saturating_sub(8),
            KeyCode::Home => self.set_scroll = 0,
            KeyCode::End => self.set_scroll = max,
            _ => return false,
        }
        self.set_scroll = self.set_scroll.min(max);
        true
    }
    pub(super) fn input_set(&mut self, key: KeyEvent, now: Instant, wide: bool) -> Input {
        if (self.terminal || self.submitted) && self.set_scroll_key(key.code) {
            return Input::None;
        }
        if self.terminal {
            return Input::None;
        }
        self.expire(now);
        if !self.enabled && self.started.is_none() {
            self.set_review = false;
            self.set_held.clear();
            self.set_selected.clear();
        }
        if self.phase.is_some() {
            return Input::None;
        }
        if self.submitted {
            if key.code == KeyCode::Char('u') {
                let operation = self
                    .pending
                    .as_ref()
                    .expect("submitted retains original")
                    .operation
                    .clone();
                return Input::Send(self.call(
                    Phase::Receipt,
                    SET_METHODS[3],
                    json!({"instanceId":self.instance,"operationId":operation}),
                ));
            }
            return Input::None;
        }
        if key.code == KeyCode::Esc {
            self.pending = None;
            self.prepared = false;
            self.set_review = false;
            self.started = None;
            self.set_scroll = 0;
            self.notice = "tui.set_close_select";
            return Input::None;
        }
        if self.set_review && self.set_scroll_key(key.code) {
            return Input::None;
        }
        if key.code == KeyCode::Char('r') && !self.set_review && !self.prepared {
            self.rows.clear();
            self.set_selected.clear();
            self.set_held.clear();
            self.set_results.clear();
            self.pending = None;
            self.enabled = false;
            self.started = Some(now);
            self.notice = "tui.dev_close_loading";
            let hello = crate::client::Read::Hello;
            return Input::Send(self.call(Phase::Hello, hello.method(), hello.params()));
        }
        if !wide || !self.enabled || self.rows.is_empty() {
            return Input::None;
        }
        if self.prepared {
            if key.code == KeyCode::Enter {
                let Some(operation) = crate::actions::operation_id() else {
                    self.refuse();
                    return Input::None;
                };
                let handles = self
                    .set_held
                    .iter()
                    .map(|i| self.rows[*i].handle.clone())
                    .collect::<Vec<_>>();
                let pending = self.pending.as_mut().expect("prepared retains original");
                pending.operation = operation;
                let params = json!({"instanceId":self.instance,"expectedRevision":self.revision,"operationId":pending.operation,"handles":handles,"ticket":pending.ticket});
                self.submitted = true;
                self.prepared = false;
                self.set_review = false;
                self.set_scroll = 0;
                self.notice = "tui.dev_close_pending";
                return Input::Send(self.call(Phase::Confirm, SET_METHODS[2], params));
            }
            return Input::None;
        }
        if self.set_review {
            if key.code == KeyCode::Enter {
                self.started = Some(now);
                let handles = self
                    .set_held
                    .iter()
                    .map(|i| self.rows[*i].handle.clone())
                    .collect::<Vec<_>>();
                return Input::Send(self.call(
                    Phase::Prepare,
                    SET_METHODS[1],
                    json!({"instanceId":self.instance,"handles":handles}),
                ));
            }
            return Input::None;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.rows.len() - 1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char(' ') => {
                if !self.set_selected.remove(&self.selected) {
                    self.set_selected.insert(self.selected);
                }
            }
            KeyCode::Char('a') => self.set_selected = (0..self.rows.len()).collect(),
            KeyCode::Char('c') => self.set_selected.clear(),
            KeyCode::Char('x') if !self.set_selected.is_empty() => {
                self.set_held = self.set_selected.iter().copied().collect();
                self.set_review = true;
                self.set_scroll = 0;
                self.notice = "tui.set_close_review";
            }
            _ => {}
        }
        Input::None
    }
    pub(super) fn accept_set(
        &mut self,
        response: Result<Value, ReadError>,
        now: Instant,
    ) -> Option<Call> {
        let phase = self.phase.take()?;
        if self.submitted {
            let parsed = response.ok().and_then(|v| self.parse_set_receipt(&v));
            match parsed {
                Some(("pending", _)) if !self.unknown => self.notice = "tui.dev_close_pending",
                Some(("unknown", members)) => {
                    self.unknown = true;
                    self.set_results = members;
                    self.notice = "tui.dev_close_unknown";
                }
                Some((aggregate, members)) if !self.unknown => {
                    self.set_results = members;
                    self.submitted = false;
                    self.terminal = true;
                    self.pending = None;
                    self.enabled = false;
                    self.started = None;
                    self.notice = match aggregate {
                        "all_closed" => "tui.set_close_all_closed",
                        "partial" => "tui.set_close_partial",
                        _ => "tui.set_close_refused",
                    };
                }
                _ => self.notice = "tui.dev_close_unknown",
            }
            return None;
        }
        if self
            .started
            .is_some_and(|s| now.saturating_duration_since(s) >= LIFE)
        {
            self.refuse();
            self.notice = "tui.dev_close_expired";
            return None;
        }
        let value = match response {
            Ok(v) if v["ok"] == true => v,
            _ => {
                self.refuse();
                return None;
            }
        };
        match phase {
            Phase::Hello => {
                let h = &value["result"];
                if h["version"] != 1 || h["runtimeOwnership"] != true {
                    self.refuse();
                    return None;
                }
                let Some(instance) = h["instanceId"].as_str().filter(|s| opaque(s)) else {
                    self.refuse();
                    return None;
                };
                if self.instance.as_deref() != Some(instance) {
                    self.revision = 0;
                }
                self.instance = Some(instance.to_owned());
                return Some(self.call(Phase::Capabilities, "capabilities.get", json!({})));
            }
            Phase::Capabilities => {
                let all = value["result"]["runtimeOwnership"] == true
                    && value["result"]["connectionCloseView"] == "inert-preview-v1"
                    && value["result"]["methods"].as_array().is_some_and(|a| {
                        a.len() <= 128 && SET_METHODS.iter().all(|m| a.iter().any(|v| v == m))
                    });
                if !all {
                    self.refuse();
                    return None;
                }
                self.view = CloseView::InertPreview;
                return Some(self.call(
                    Phase::Snapshot,
                    SET_METHODS[0],
                    json!({"instanceId":self.instance}),
                ));
            }
            Phase::Snapshot => {
                if !self.snapshot(&value) {
                    self.refuse();
                } else {
                    self.notice = if self.rows.is_empty() {
                        "tui.dev_close_empty"
                    } else {
                        "tui.set_close_select"
                    };
                }
            }
            Phase::Prepare => {
                if !self.prepare_set(&value) {
                    self.refuse();
                }
            }
            _ => self.refuse(),
        }
        None
    }
    fn prepare_set(&mut self, value: &Value) -> bool {
        let r = &value["result"];
        if !self.set_review
            || self.set_held.is_empty()
            || revision(value) != Some(self.revision)
            || !exact(
                r,
                &["schemaVersion", "scope", "instanceId", "ticket", "rows"],
            )
            || r["schemaVersion"] != 1
            || r["scope"] != SET_SCOPE
            || r["instanceId"].as_str() != self.instance.as_deref()
        {
            return false;
        }
        let expected = self
            .set_held
            .iter()
            .map(|i| json!({"handle":self.rows[*i].handle,"display":self.rows[*i].display}))
            .collect::<Vec<_>>();
        if r["rows"] != json!(expected) {
            return false;
        }
        let Some(ticket) = token(&r["ticket"]) else {
            return false;
        };
        self.pending = Some(Pending {
            handle: String::new(),
            ticket,
            operation: String::new(),
        });
        self.prepared = true;
        self.set_scroll = 0;
        self.notice = "tui.set_close_confirm";
        true
    }
    pub(super) fn parse_set_receipt(
        &self,
        value: &Value,
    ) -> Option<(&'static str, Vec<&'static str>)> {
        let r = &value["result"];
        let pending = self.pending.as_ref()?;
        if value["ok"] != true
            || revision(value)? < self.revision
            || !exact(
                r,
                &[
                    "schemaVersion",
                    "scope",
                    "instanceId",
                    "operationId",
                    "state",
                    "aggregate",
                    "members",
                    "receiptRevision",
                ],
            )
            || r["schemaVersion"] != 1
            || r["scope"] != SET_SCOPE
            || r["instanceId"].as_str() != self.instance.as_deref()
            || r["operationId"] != pending.operation
        {
            return None;
        }
        if r["state"] == "pending"
            && r["aggregate"].is_null()
            && r["members"].is_null()
            && r["receiptRevision"].is_null()
        {
            return Some(("pending", Vec::new()));
        }
        if r["state"] != "finished"
            || r["receiptRevision"].as_u64()? < self.revision
            || r["receiptRevision"].as_u64()? > revision(value)?
        {
            return None;
        }
        let raw = r["members"]
            .as_array()
            .filter(|a| a.len() == self.set_held.len() && !a.is_empty())?;
        let mut members = Vec::with_capacity(raw.len());
        let mut stopped = false;
        for (index, m) in raw.iter().enumerate() {
            if !exact(m, &["index", "outcome"]) || m["index"].as_u64() != Some(index as u64) {
                return None;
            }
            let outcome = [
                "closed",
                "missing",
                "changed",
                "unsupported",
                "unknown",
                "not_attempted",
            ]
            .into_iter()
            .find(|x| m["outcome"] == *x)?;
            if stopped && outcome != "not_attempted" {
                return None;
            }
            // A known first non-Closed stops the worker; uncertainty downgrades
            // every attempted member, so consecutive Unknown is permitted.
            if outcome != "closed" && outcome != "unknown" {
                stopped = true;
            }
            members.push(outcome);
        }
        let aggregate = if members.contains(&"unknown") {
            if members
                .iter()
                .any(|x| !matches!(*x, "unknown" | "not_attempted"))
            {
                return None;
            }
            "unknown"
        } else if members.iter().all(|x| *x == "closed") {
            "all_closed"
        } else if members.iter().all(|x| *x == "not_attempted") {
            "refused_before_write"
        } else {
            "partial"
        };
        if r["aggregate"] != aggregate {
            return None;
        }
        if members.iter().any(|m| matches!(*m, "closed" | "unknown"))
            && r["receiptRevision"].as_u64()? <= self.revision
        {
            return None;
        }
        Some((aggregate, members))
    }
    fn set_text(&self) -> String {
        let tr = |key| self.locale.text(key);
        let indices: Vec<_> = if self.set_review || self.prepared || self.submitted || self.terminal
        {
            self.set_held.clone()
        } else {
            (0..self.rows.len()).collect()
        };
        indices
            .iter()
            .enumerate()
            .filter_map(|(member, index)| {
                self.rows.get(*index).map(|row| {
                    let state = if let Some(result) = self.set_results.get(member) {
                        tr(match *result {
                            "closed" => "tui.set_close_member_closed",
                            "missing" => "tui.set_close_member_missing",
                            "changed" => "tui.set_close_member_changed",
                            "unsupported" => "tui.set_close_member_unsupported",
                            "unknown" => "tui.set_close_member_unknown",
                            _ => "tui.set_close_member_unattempted",
                        })
                    } else if self.set_selected.contains(index) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    format!("{} {}\n{}\n", member + 1, state, row.text)
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub(super) fn draw_set(&self, frame: &mut Frame<'_>, now: Instant) {
        self.set_viewport
            .set((frame.area().width, frame.area().height));
        let tr = |key| self.locale.text(key);
        let [head, body, footer] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Min(3),
            Constraint::Length(7),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(tr("tui.set_close_scope"))
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("OmaVLESS")),
            head,
        );
        let expired = !self.submitted
            && self
                .started
                .is_some_and(|s| now.saturating_duration_since(s) >= LIFE);
        if expired || frame.area().width < MIN_WIDTH || frame.area().height < MIN_HEIGHT {
            frame.render_widget(
                Paragraph::new(tr(if expired {
                    "tui.dev_close_expired"
                } else {
                    "tui.set_close_resize"
                }))
                .wrap(Wrap { trim: false }),
                body,
            );
        } else if self.set_review || self.prepared || self.submitted || self.terminal {
            let paragraph = Paragraph::new(self.set_text()).wrap(Wrap { trim: false });
            let max = paragraph
                .line_count(body.width.saturating_sub(2))
                .saturating_sub(usize::from(body.height.saturating_sub(2)));
            let scroll = usize::from(self.set_scroll).min(max) as u16;
            frame.render_widget(
                paragraph
                    .scroll((scroll, 0))
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        "{}: {}",
                        tr("tui.set_close_selected"),
                        self.set_held.len()
                    ))),
                body,
            );
        } else {
            let items = self
                .rows
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    ListItem::new(format!(
                        "{} {}",
                        if self.set_selected.contains(&index) {
                            "[x]"
                        } else {
                            "[ ]"
                        },
                        row.text
                    ))
                })
                .collect::<Vec<_>>();
            let mut state =
                ListState::default().with_selected((!items.is_empty()).then_some(self.selected));
            frame.render_stateful_widget(
                List::new(items).highlight_symbol("> ").block(
                    Block::default().borders(Borders::ALL).title(format!(
                        "{}: {}/{}",
                        tr("tui.set_close_selected"),
                        self.set_selected.len(),
                        self.rows.len()
                    )),
                ),
                body,
                &mut state,
            );
        }
        let constrained = frame.area().width < MIN_WIDTH || frame.area().height < MIN_HEIGHT;
        let keys = if self.phase.is_some() {
            "tui.dev_close_wait_keys"
        } else if self.submitted {
            "tui.set_close_receipt_keys"
        } else if self.terminal {
            "tui.set_close_result_keys"
        } else if constrained {
            "tui.set_close_resize_keys"
        } else if self.prepared && !expired {
            "tui.dev_close_confirm_keys"
        } else if self.set_review && !expired {
            "tui.set_close_review_keys"
        } else if expired || !self.enabled || self.rows.is_empty() {
            "tui.dev_close_refresh_keys"
        } else {
            "tui.set_close_keys"
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n{}\n{}",
                tr(if expired {
                    "tui.dev_close_expired"
                } else {
                    self.notice
                }),
                tr(keys),
                tr("tui.dev_close_exit")
            ))
            .wrap(Wrap { trim: false }),
            footer,
        );
    }
}
