// SPDX-License-Identifier: MIT
//! Explicit opt-in client of the existing development socket workspace.
//! No runtime/core ownership, raw IDs, automatic refresh, resend or fallback.
use crate::{i18n::Locale, model::ReadError};
use ratatui::crossterm::event::Event;
pub use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

const METHODS: [&str; 4] = [
    "development.connections.snapshot",
    "development.connections.prepare",
    "development.connections.confirm",
    "development.connections.receipt",
];
const SCOPE: &str = "development_owned_single_connection_close";
const LIFE: Duration = Duration::from_secs(5);
const MIN_WIDTH: u16 = 70;
const MIN_HEIGHT: u16 = 24;

// No Debug/Serialize: original instance, handles/ticket and private display stay
// in this window and the authenticated socket. They never enter activity logs.
#[derive(Clone)]
pub struct Call {
    method: &'static str,
    params: Value,
}
impl Call {
    pub fn method(&self) -> &'static str {
        self.method
    }
    pub fn params(&self) -> Value {
        self.params.clone()
    }
}

struct Row {
    handle: String,
    display: Value,
    text: String,
}
struct Pending {
    handle: String,
    ticket: String,
    operation: String,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Hello,
    Capabilities,
    Snapshot,
    Prepare,
    Confirm,
    Receipt,
}
pub enum Input {
    None,
    Close,
    Send(Call),
}

/// At most 128 private rows, one confirmation and one non-evicted operation.
/// After submission only receipt reads are possible until a verified terminal
/// result. Unknown never admits a new operation in this window.
pub struct Workspace {
    locale: Locale,
    instance: Option<String>,
    enabled: bool,
    rows: Vec<Row>,
    selected: usize,
    revision: u64,
    started: Option<Instant>,
    phase: Option<Phase>,
    pending: Option<Pending>,
    prepared: bool,
    submitted: bool,
    unknown: bool,
    notice: &'static str,
}

fn opaque(text: &str) -> bool {
    !text.is_empty() && text.len() <= 128 && text.bytes().all(|b| (33..=126).contains(&b))
}
fn token(value: &Value) -> Option<String> {
    let text = value.as_str()?;
    (text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && text.bytes().any(|b| b != b'0'))
    .then(|| text.to_owned())
}
fn exact(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k)))
}
fn revision(value: &Value) -> Option<u64> {
    value["revision"].as_u64().filter(|r| *r <= i64::MAX as u64)
}
fn display(value: &Value) -> Option<String> {
    // Use the existing private projection parser, but require exactly the one
    // row captured WITH this handle, not a row from runtime.connection_rows.
    if !exact(value, &["host", "ip", "port", "network", "route"]) {
        return None;
    }
    let wrapped = json!({"ok":true,"result":{"schemaVersion":1,
        "scope":"owned_core_private_connection_rows","availability":"observed",
        "total":1,"shown":1,"truncated":false,"rows":[value]}});
    let parsed = crate::inspection::ConnectionRows::parse(&wrapped)?;
    if parsed.total != 1 || parsed.truncated || parsed.rows.len() != 1 {
        return None;
    }
    let row = &parsed.rows[0];
    if *value
        != json!({"host":row.host,"ip":row.ip,"port":row.port,
        "network":row.network,"route":row.route})
    {
        return None;
    }
    let destination = row.host.as_deref().or(row.ip.as_deref()).unwrap_or("—");
    let mut safe = crate::model::display(destination, 120);
    if row.host.is_none()
        && destination
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_ipv6())
    {
        safe = format!("[{safe}]");
    }
    let endpoint = row
        .port
        .map_or(safe.clone(), |port| format!("{safe}:{port}"));
    let mut text = format!("{endpoint} · {} · {}", row.network, row.route);
    if row.host.is_some()
        && let Some(ip) = &row.ip
    {
        text.push_str(&format!("\n{}", crate::model::display(ip, 120)));
    }
    Some(text)
}

impl Workspace {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            instance: None,
            enabled: false,
            rows: Vec::new(),
            selected: 0,
            revision: 0,
            started: None,
            phase: None,
            pending: None,
            prepared: false,
            submitted: false,
            unknown: false,
            notice: "tui.dev_close_refresh",
        }
    }
    fn call(&mut self, phase: Phase, method: &'static str, params: Value) -> Call {
        self.phase = Some(phase);
        Call { method, params }
    }
    fn expire(&mut self, now: Instant) {
        if !self.submitted
            && self.phase.is_none()
            && self
                .started
                .is_some_and(|started| now.saturating_duration_since(started) >= LIFE)
        {
            self.rows.clear();
            self.pending = None;
            self.prepared = false;
            self.enabled = false;
            self.started = None;
            self.notice = "tui.dev_close_expired";
        }
    }
    pub fn input(&mut self, key: KeyEvent, now: Instant, wide: bool) -> Input {
        if matches!(key.code, KeyCode::Char('q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Input::Close;
        }
        if key.kind != KeyEventKind::Press {
            return Input::None;
        }
        self.expire(now);
        if self.phase.is_some() {
            return Input::None;
        }
        if self.submitted {
            return if key.code == KeyCode::Char('u') {
                let operation = self
                    .pending
                    .as_ref()
                    .expect("submitted retains request")
                    .operation
                    .clone();
                Input::Send(self.call(
                    Phase::Receipt,
                    METHODS[3],
                    json!({"instanceId":self.instance,"operationId":operation}),
                ))
            } else {
                Input::None
            };
        }
        if key.code == KeyCode::Char('r') {
            self.rows.clear();
            self.pending = None;
            self.prepared = false;
            self.enabled = false;
            self.started = Some(now);
            let hello = crate::client::Read::Hello;
            return Input::Send(self.call(Phase::Hello, hello.method(), hello.params()));
        }
        if key.code == KeyCode::Esc {
            self.pending = None;
            self.prepared = false;
            return Input::None;
        }
        if !wide || !self.enabled || self.rows.is_empty() {
            return Input::None;
        }
        if self.prepared {
            if key.code == KeyCode::Enter {
                let Some(operation) = crate::actions::operation_id() else {
                    self.pending = None;
                    self.prepared = false;
                    self.notice = "tui.dev_close_unavailable";
                    return Input::None;
                };
                let pending = self.pending.as_mut().expect("prepared retains ticket");
                pending.operation = operation;
                let params = json!({"instanceId":self.instance,"expectedRevision":self.revision,
                    "operationId":pending.operation,"handle":pending.handle,"ticket":pending.ticket});
                // Mark uncertainty BEFORE handing even one request to a worker.
                self.submitted = true;
                self.prepared = false;
                self.notice = "tui.dev_close_pending";
                return Input::Send(self.call(Phase::Confirm, METHODS[2], params));
            }
            return Input::None;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.rows.len() - 1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('x') => {
                let handle = self.rows[self.selected].handle.clone();
                return Input::Send(self.call(
                    Phase::Prepare,
                    METHODS[1],
                    json!({"instanceId":self.instance,"handle":handle}),
                ));
            }
            _ => {}
        }
        Input::None
    }
    pub fn accept(&mut self, response: Result<Value, ReadError>, now: Instant) -> Option<Call> {
        let phase = self.phase.take()?;
        if self.submitted {
            let result = response.ok().and_then(|v| self.receipt(&v));
            match result {
                Some("pending") if !self.unknown => self.notice = "tui.dev_close_pending",
                Some("unknown") => {
                    self.unknown = true;
                    self.notice = "tui.dev_close_unknown";
                }
                Some(outcome) if !self.unknown => {
                    self.notice = match outcome {
                        "closed" => "tui.dev_close_closed",
                        "missing" => "tui.dev_close_missing",
                        _ => "tui.dev_close_refused",
                    };
                    self.submitted = false;
                    self.pending = None;
                    self.rows.clear();
                    self.enabled = false;
                    self.started = None;
                }
                // Transport ambiguity preserves the request but a later exact
                // terminal receipt can resolve it. A VERIFIED Unknown above
                // is sticky and cannot be acknowledged as success.
                _ => self.notice = "tui.dev_close_unknown",
            }
            return None;
        }
        if self
            .started
            .is_some_and(|started| now.saturating_duration_since(started) >= LIFE)
        {
            self.refuse();
            self.notice = "tui.dev_close_expired";
            return None;
        }
        let value = match response {
            Ok(value) if value["ok"] == true => value,
            _ => {
                self.refuse();
                return None;
            }
        };
        let good = match phase {
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
                let methods = value["result"]["methods"].as_array();
                let all = value["result"]["runtimeOwnership"] == true
                    && methods.is_some_and(|m| {
                        m.len() <= 128
                            && METHODS
                                .iter()
                                .all(|expected| m.iter().any(|v| v == expected))
                    });
                if !all {
                    self.refuse();
                    return None;
                }
                return Some(self.call(
                    Phase::Snapshot,
                    METHODS[0],
                    json!({"instanceId":self.instance}),
                ));
            }
            Phase::Snapshot => self.snapshot(&value),
            Phase::Prepare => self.prepare(&value),
            _ => false,
        };
        if !good {
            self.refuse();
        } else {
            self.expire(now);
        }
        None
    }
    fn refuse(&mut self) {
        self.rows.clear();
        self.pending = None;
        self.prepared = false;
        self.enabled = false;
        self.started = None;
        self.notice = "tui.dev_close_unavailable";
    }
    fn snapshot(&mut self, value: &Value) -> bool {
        let result = &value["result"];
        let Some(revision) = revision(value) else {
            return false;
        };
        if revision < self.revision {
            return false;
        }
        if !exact(result, &["schemaVersion", "scope", "instanceId", "rows"])
            || result["schemaVersion"] != 1
            || result["scope"] != SCOPE
            || result["instanceId"].as_str() != self.instance.as_deref()
        {
            return false;
        }
        let Some(rows) = result["rows"].as_array().filter(|r| r.len() <= 128) else {
            return false;
        };
        let mut parsed = Vec::with_capacity(rows.len());
        let mut seen = BTreeSet::new();
        for row in rows {
            if !exact(row, &["handle", "display"]) {
                return false;
            }
            let Some(handle) = token(&row["handle"]) else {
                return false;
            };
            if !seen.insert(handle.clone()) {
                return false;
            }
            let Some(text) = display(&row["display"]) else {
                return false;
            };
            parsed.push(Row {
                handle,
                display: row["display"].clone(),
                text,
            });
        }
        self.rows = parsed;
        self.revision = revision;
        self.selected = 0;
        self.enabled = true;
        self.notice = "tui.dev_close_select";
        true
    }
    fn prepare(&mut self, value: &Value) -> bool {
        let result = &value["result"];
        let Some(row) = self.rows.get(self.selected) else {
            return false;
        };
        if revision(value) != Some(self.revision)
            || !exact(
                result,
                &[
                    "schemaVersion",
                    "scope",
                    "instanceId",
                    "handle",
                    "ticket",
                    "display",
                ],
            )
            || result["schemaVersion"] != 1
            || result["scope"] != SCOPE
            || result["instanceId"].as_str() != self.instance.as_deref()
            || result["handle"] != row.handle
            || result["display"] != row.display
        {
            return false;
        }
        let Some(ticket) = token(&result["ticket"]) else {
            return false;
        };
        self.pending = Some(Pending {
            handle: row.handle.clone(),
            ticket,
            operation: String::new(),
        });
        self.prepared = true;
        self.notice = "tui.dev_close_confirm";
        true
    }
    fn receipt(&self, value: &Value) -> Option<&'static str> {
        let result = &value["result"];
        let pending = self.pending.as_ref()?;
        if value["ok"] != true
            || revision(value)? < self.revision
            || !exact(
                result,
                &[
                    "schemaVersion",
                    "scope",
                    "instanceId",
                    "operationId",
                    "state",
                    "outcome",
                    "receiptRevision",
                ],
            )
            || result["schemaVersion"] != 1
            || result["scope"] != SCOPE
            || result["instanceId"].as_str() != self.instance.as_deref()
            || result["operationId"] != pending.operation
        {
            return None;
        }
        if result["state"] == "pending"
            && result["outcome"].is_null()
            && result["receiptRevision"].is_null()
        {
            return Some("pending");
        }
        if result["state"] != "finished"
            || result["receiptRevision"].as_u64()? < self.revision
            || result["receiptRevision"].as_u64()? > revision(value)?
        {
            return None;
        }
        [
            "closed",
            "missing",
            "changed",
            "unsupported",
            "unknown",
            "refused_before_write",
            "missing_attestation",
        ]
        .into_iter()
        .find(|outcome| result["outcome"] == *outcome)
    }
    pub fn draw(&self, frame: &mut Frame<'_>, now: Instant) {
        let tr = |key| self.locale.text(key);
        let [head, body, footer] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Min(3),
            Constraint::Length(7),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(tr("tui.dev_close_scope"))
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
                    "tui.dev_close_resize"
                }))
                .wrap(Wrap { trim: false }),
                body,
            );
        } else if self.prepared {
            // Show the FULL selected target, not a clipped list label. Movement
            // is disabled until confirmation is cancelled or submitted.
            let target = &self.rows[self.selected].text;
            frame.render_widget(
                Paragraph::new(format!(
                    "{}\n{target}\n\n{}",
                    tr("tui.dev_close_selected"),
                    tr("tui.dev_close_confirm")
                ))
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL)),
                body,
            );
        } else if self.enabled && self.rows.is_empty() {
            frame.render_widget(Paragraph::new(tr("tui.dev_close_empty")), body);
        } else {
            let items: Vec<_> = self
                .rows
                .iter()
                .map(|r| ListItem::new(r.text.clone()))
                .collect();
            let mut state =
                ListState::default().with_selected((!items.is_empty()).then_some(self.selected));
            frame.render_stateful_widget(
                List::new(items).highlight_symbol("> ").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(tr("tui.dev_close_rows")),
                ),
                body,
                &mut state,
            );
        }
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n{}\n{}\n{}",
                tr(if expired {
                    "tui.dev_close_expired"
                } else {
                    self.notice
                }),
                tr(if self.submitted {
                    "tui.dev_close_receipt_keys"
                } else {
                    "tui.dev_close_keys"
                }),
                tr("tui.dev_close_exit"),
                tr("tui.dev_close_not_health")
            ))
            .wrap(Wrap { trim: false }),
            footer,
        );
    }
}

/// The production adapter supplies only the fixed authenticated unary client.
/// One bounded worker; exiting drops client channels, never runtime commands.
pub fn run(
    mut call: impl FnMut(&Call) -> Result<Value, ReadError> + Send + 'static,
) -> Result<(), &'static str> {
    use std::{
        io::IsTerminal,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
    };
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err("OmaVLESS TUI requires an interactive terminal");
    }
    let mut terminal = super::TerminalGuard(Some(
        ratatui::try_init().map_err(|_| "Could not initialize OmaVLESS terminal")?,
    ));
    std::panic::set_hook(Box::new(|_| {
        let _ = ratatui::try_restore();
    }));
    let stop = Arc::new(AtomicBool::new(false));
    struct Signals(Vec<signal_hook::SigId>);
    impl Drop for Signals {
        fn drop(&mut self) {
            for id in self.0.drain(..) {
                signal_hook::low_level::unregister(id);
            }
        }
    }
    let mut signals = Signals(Vec::new());
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signals.0.push(
            signal_hook::flag::register(signal, stop.clone())
                .map_err(|_| "Could not initialize terminal signal handling")?,
        );
    }
    let _signals = signals;
    let (send, requests) = mpsc::sync_channel::<Call>(1);
    let (results, receive) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("tui-development-close".into())
        .spawn(move || {
            while let Ok(request) = requests.recv() {
                if results.send(call(&request)).is_err() {
                    break;
                }
            }
        })
        .map_err(|_| "Could not start terminal client")?;
    let (keys, input) = mpsc::sync_channel(16);
    thread::Builder::new()
        .name("tui-development-input".into())
        .spawn(move || {
            loop {
                let event = ratatui::crossterm::event::read();
                let failed = event.is_err();
                if keys.send(event).is_err() || failed {
                    break;
                }
            }
        })
        .map_err(|_| "Could not start terminal input")?;
    let mut workspace = Workspace::new(Locale::current());
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if let Ok(result) = receive.try_recv()
            && let Some(request) = workspace.accept(result, now)
            && send.try_send(request).is_err()
        {
            workspace.accept(Err(ReadError::Unavailable), now);
        }
        terminal
            .draw(|f| workspace.draw(f, now))
            .map_err(|_| "Could not draw OmaVLESS terminal")?;
        match input.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(Event::Key(key))) => match workspace.input(
                key,
                Instant::now(),
                terminal
                    .size()
                    .is_ok_and(|s| s.width >= MIN_WIDTH && s.height >= MIN_HEIGHT),
            ) {
                Input::Close => break,
                Input::Send(request) => {
                    if send.try_send(request).is_err() {
                        workspace.accept(Err(ReadError::Unavailable), now);
                    }
                }
                Input::None => {}
            },
            Ok(Err(_)) => return Err("Could not read terminal input"),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn send(input: Input) -> Call {
        match input {
            Input::Send(call) => call,
            _ => panic!("expected fixed request"),
        }
    }
    fn ok(result: Value) -> Result<Value, ReadError> {
        Ok(json!({"ok":true,"revision":7,"result":result}))
    }
    fn row(handle: &str) -> Value {
        json!({"handle":handle,"display":{"host":"same.invalid","ip":null,
            "port":443,"network":"tcp","route":"direct"}})
    }
    fn snapshot() -> Value {
        json!({"schemaVersion":1,"scope":SCOPE,"instanceId":"owner",
            "rows":[row(&"1".repeat(64)),row(&"2".repeat(64))]})
    }
    fn ready(workspace: &mut Workspace, now: Instant) {
        let hello = send(workspace.input(key(KeyCode::Char('r')), now, true));
        assert_eq!(hello.method(), "system.hello");
        assert_eq!(hello.params(), json!({"versions":[1]}));
        let call = workspace
            .accept(
                ok(json!({"version":1,"runtimeOwnership":true,"instanceId":"owner"})),
                now,
            )
            .unwrap();
        assert_eq!(call.method(), "capabilities.get");
        let call = workspace
            .accept(ok(json!({"runtimeOwnership":true,"methods":METHODS})), now)
            .unwrap();
        assert_eq!(call.method(), METHODS[0]);
        assert_eq!(call.params(), json!({"instanceId":"owner"}));
        assert!(workspace.accept(ok(snapshot()), now).is_none());
        assert!(workspace.enabled);
    }
    fn prepared(workspace: &mut Workspace, now: Instant) {
        let call = send(workspace.input(key(KeyCode::Char('x')), now, true));
        let handle = call.params()["handle"].clone();
        let display = workspace.rows[workspace.selected].display.clone();
        workspace.accept(
            ok(json!({"schemaVersion":1,"scope":SCOPE,"instanceId":"owner",
            "handle":handle,"ticket":"3".repeat(64),"display":display})),
            now,
        );
        assert!(workspace.prepared);
    }
    fn receipt(workspace: &Workspace, outcome: Option<&str>) -> Value {
        json!({"schemaVersion":1,"scope":SCOPE,"instanceId":"owner",
            "operationId":workspace.pending.as_ref().unwrap().operation,
            "state":if outcome.is_some(){"finished"}else{"pending"},
            "outcome":outcome,"receiptRevision":outcome.map(|_|7)})
    }
    #[test]
    fn all_four_capabilities_are_required_and_no_startup_refresh_is_implicit() {
        let now = Instant::now();
        for missing in METHODS {
            let mut workspace = Workspace::new(Locale::En);
            assert!(workspace.phase.is_none());
            assert!(workspace.rows.is_empty());
            send(workspace.input(key(KeyCode::Char('r')), now, true));
            workspace.accept(
                ok(json!({"version":1,"runtimeOwnership":true,"instanceId":"owner"})),
                now,
            );
            let methods: Vec<_> = METHODS.into_iter().filter(|m| *m != missing).collect();
            assert!(
                workspace
                    .accept(ok(json!({"runtimeOwnership":true,"methods":methods})), now)
                    .is_none()
            );
            assert!(!workspace.enabled);
        }
    }
    #[test]
    fn equal_display_selection_uses_exact_opaque_row_not_destination_or_index_on_wire() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        assert_eq!(workspace.rows[0].text, workspace.rows[1].text);
        workspace.input(key(KeyCode::Down), now, true);
        prepared(&mut workspace, now);
        let mut repeated = key(KeyCode::Enter);
        repeated.kind = KeyEventKind::Repeat;
        assert!(matches!(workspace.input(repeated, now, true), Input::None));
        let call = send(workspace.input(key(KeyCode::Enter), now, true));
        assert_eq!(call.method(), METHODS[2]);
        assert_eq!(call.params()["handle"], "2".repeat(64));
        assert_eq!(call.params()["expectedRevision"], 7);
        assert!(!call.params().to_string().contains("same.invalid"));
        assert!(matches!(
            workspace.input(key(KeyCode::Enter), now, true),
            Input::None
        ));
        assert!(workspace.submitted);
    }
    #[test]
    fn malformed_duplicate_tokens_or_wrong_instance_snapshot_never_admits() {
        let now = Instant::now();
        for case in 0..4 {
            let mut value = snapshot();
            let mut workspace = Workspace::new(Locale::En);
            ready(&mut workspace, now);
            workspace.phase = Some(Phase::Snapshot);
            match case {
                0 => value["rows"][1]["handle"] = value["rows"][0]["handle"].clone(),
                1 => value["rows"][0]["rawControllerId"] = json!("forbidden"),
                2 => value["rows"][0]["display"]["network"] = json!("unknown-network"),
                _ => value["rows"] = json!(vec![row(&"1".repeat(64)); 129]),
            }
            workspace.accept(ok(value), now);
            assert!(!workspace.enabled);
        }
        for bad in [
            json!("0".repeat(64)),
            json!("A".repeat(64)),
            json!("1".repeat(63)),
            json!(null),
        ] {
            let mut workspace = Workspace::new(Locale::En);
            ready(&mut workspace, now);
            let mut value = snapshot();
            value["rows"][0]["handle"] = bad;
            workspace.phase = Some(Phase::Snapshot);
            workspace.accept(ok(value), now);
            assert!(!workspace.enabled);
        }
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        let mut value = snapshot();
        value["instanceId"] = json!("new-owner");
        workspace.phase = Some(Phase::Snapshot);
        workspace.accept(ok(value), now);
        assert!(!workspace.enabled);
    }
    #[test]
    fn stale_revision_wrong_handle_and_changed_display_prepare_refuse() {
        let now = Instant::now();
        for field in ["revision", "handle", "display", "instanceId", "ticket"] {
            let mut workspace = Workspace::new(Locale::En);
            ready(&mut workspace, now);
            send(workspace.input(key(KeyCode::Char('x')), now, true));
            let mut result = json!({"schemaVersion":1,"scope":SCOPE,"instanceId":"owner",
                "handle":"1".repeat(64),"ticket":"3".repeat(64),"display":workspace.rows[0].display});
            match field {
                "revision" => {}
                "handle" => result["handle"] = json!("2".repeat(64)),
                "display" => result["display"]["host"] = json!("other.invalid"),
                "instanceId" => result["instanceId"] = json!("new-owner"),
                _ => result["ticket"] = json!("0".repeat(64)),
            }
            let mut response = ok(result).unwrap();
            if field == "revision" {
                response["revision"] = json!(8);
            }
            workspace.accept(Ok(response), now);
            assert!(!workspace.prepared);
            assert!(workspace.rows.is_empty());
        }
    }
    #[test]
    fn expiry_resize_cancel_and_navigation_cannot_send_confirm() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        prepared(&mut workspace, now);
        assert!(matches!(
            workspace.input(key(KeyCode::Enter), now, false),
            Input::None
        ));
        workspace.input(key(KeyCode::Esc), now, true);
        assert!(!workspace.prepared);
        prepared(&mut workspace, now);
        assert!(matches!(
            workspace.input(key(KeyCode::Enter), now + LIFE, true),
            Input::None
        ));
        assert!(workspace.rows.is_empty());
        assert!(matches!(
            workspace.input(key(KeyCode::Char('q')), now + LIFE, true),
            Input::Close
        ));
    }
    #[test]
    fn ambiguous_confirm_retains_operation_and_receipt_can_resolve_without_resend() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        prepared(&mut workspace, now);
        let original = send(workspace.input(key(KeyCode::Enter), now, true));
        workspace.accept(Err(ReadError::Unavailable), now + LIFE);
        for keycode in [KeyCode::Enter, KeyCode::Char('r'), KeyCode::Char('x')] {
            assert!(matches!(
                workspace.input(key(keycode), now + LIFE, true),
                Input::None
            ));
        }
        let call = send(workspace.input(key(KeyCode::Char('u')), now + LIFE, true));
        assert_eq!(call.method(), METHODS[3]);
        assert_eq!(
            call.params()["operationId"],
            original.params()["operationId"]
        );
        workspace.accept(ok(receipt(&workspace, Some("closed"))), now + LIFE);
        assert!(!workspace.submitted);
        assert!(!workspace.enabled);
        assert_eq!(workspace.notice, "tui.dev_close_closed");
    }
    #[test]
    fn verified_unknown_is_sticky_and_invalid_receipts_never_acknowledge() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        prepared(&mut workspace, now);
        send(workspace.input(key(KeyCode::Enter), now, true));
        workspace.accept(ok(receipt(&workspace, Some("unknown"))), now);
        assert!(workspace.unknown && workspace.submitted);
        for outcome in ["closed", "missing", "refused_before_write"] {
            send(workspace.input(key(KeyCode::Char('u')), now, true));
            workspace.accept(ok(receipt(&workspace, Some(outcome))), now);
            assert!(workspace.unknown && workspace.submitted);
        }
        let mut other = Workspace::new(Locale::En);
        ready(&mut other, now);
        prepared(&mut other, now);
        send(other.input(key(KeyCode::Enter), now, true));
        let mut wrong = receipt(&other, Some("closed"));
        wrong["operationId"] = json!("foreign");
        other.accept(ok(wrong), now);
        assert!(other.submitted);
    }
    #[test]
    fn en_ru_render_sanitizes_private_text_and_never_displays_tokens() {
        for locale in [Locale::En, Locale::Ru] {
            let now = Instant::now();
            let mut workspace = Workspace::new(locale);
            ready(&mut workspace, now);
            prepared(&mut workspace, now);
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal.draw(|f| workspace.draw(f, now)).unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(text.contains("same.invalid"));
            assert!(text.contains(locale.text("tui.dev_close_selected")));
            assert!(!text.contains(&"1".repeat(64)));
            assert!(!text.contains(&"3".repeat(64)));
            assert!(!text.contains("Missing translation"));
        }
        let mut bad = row(&"1".repeat(64))["display"].clone();
        bad["host"] = json!("\u{1b}[31m");
        assert!(display(&bad).is_none());
    }

    #[test]
    fn late_snapshot_and_late_prepare_cannot_extend_original_client_lifetime() {
        let now = Instant::now();
        for phase in [
            Phase::Hello,
            Phase::Capabilities,
            Phase::Snapshot,
            Phase::Prepare,
        ] {
            let mut workspace = Workspace::new(Locale::En);
            ready(&mut workspace, now);
            let value = if phase == Phase::Snapshot {
                snapshot()
            } else {
                json!({"schemaVersion":1,"scope":SCOPE,"instanceId":"owner","handle":"1".repeat(64),
                    "ticket":"3".repeat(64),"display":workspace.rows[0].display})
            };
            workspace.phase = Some(phase);
            assert!(workspace.accept(ok(value), now + LIFE).is_none());
            assert!(workspace.rows.is_empty());
            assert!(workspace.pending.is_none());
            assert!(matches!(
                workspace.input(key(KeyCode::Enter), now + LIFE, true),
                Input::None
            ));
        }
    }

    #[test]
    fn same_instance_revision_regression_refuses_a_fresh_snapshot() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        workspace.phase = Some(Phase::Snapshot);
        let mut response = ok(snapshot()).unwrap();
        response["revision"] = json!(6);
        workspace.accept(Ok(response), now);
        assert!(workspace.rows.is_empty());
        assert!(!workspace.enabled);
    }

    #[test]
    fn ipv6_target_is_unambiguous_and_spent_snapshot_is_not_rendered_as_observed_empty() {
        let target =
            json!({"host":null,"ip":"2001:db8::1","port":443,"network":"tcp","route":"vpn"});
        assert_eq!(display(&target).unwrap(), "[2001:db8::1]:443 · tcp · vpn");
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        prepared(&mut workspace, now);
        send(workspace.input(key(KeyCode::Enter), now, true));
        workspace.accept(ok(receipt(&workspace, Some("closed"))), now);
        let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
        terminal.draw(|f| workspace.draw(f, now)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(!text.contains(workspace.locale.text("tui.dev_close_empty")));
        assert!(text.contains(workspace.locale.text("tui.dev_close_not_health")));
    }

    #[test]
    fn complete_128_row_catalogue_is_bounded_and_receipt_identity_revision_state_are_exact() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        let mut value = snapshot();
        value["rows"] = json!(
            (1..=128)
                .map(|i| row(&format!("{i:064x}")))
                .collect::<Vec<_>>()
        );
        workspace.phase = Some(Phase::Snapshot);
        workspace.accept(ok(value), now);
        assert_eq!(workspace.rows.len(), 128);
        prepared(&mut workspace, now);
        send(workspace.input(key(KeyCode::Enter), now, true));
        let good = receipt(&workspace, Some("closed"));
        for field in [
            "instanceId",
            "operationId",
            "receiptRevision",
            "state",
            "outcome",
            "extra",
        ] {
            let mut bad = good.clone();
            bad[field] = match field {
                "receiptRevision" => json!(6),
                _ => json!("foreign"),
            };
            assert!(workspace.receipt(&ok(bad).unwrap()).is_none());
        }
        assert_eq!(
            workspace.receipt(&ok(receipt(&workspace, None)).unwrap()),
            Some("pending")
        );
        assert_eq!(workspace.receipt(&ok(good).unwrap()), Some("closed"));
    }

    #[test]
    fn full_long_selected_target_and_distinct_ip_are_visible_at_minimum_confirmation_size() {
        let now = Instant::now();
        let mut workspace = Workspace::new(Locale::En);
        ready(&mut workspace, now);
        let host = format!("{}.different.invalid", "a".repeat(100));
        let target =
            json!({"host":host,"ip":"203.0.113.99","port":443,"network":"tcp","route":"direct"});
        workspace.rows[0].display = target.clone();
        workspace.rows[0].text = display(&target).unwrap();
        prepared(&mut workspace, now);
        let mut terminal = Terminal::new(TestBackend::new(MIN_WIDTH, MIN_HEIGHT)).unwrap();
        terminal.draw(|f| workspace.draw(f, now)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("different.invalid"));
        assert!(text.contains("203.0.113.99"));
        assert!(text.contains("Enter"));
        assert!(text.contains("Esc"));
    }
}
