// SPDX-License-Identifier: MIT
use crate::{
    app::App,
    model::{Mode, ReadError, Status, display},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap},
};
use std::time::Instant;

pub fn draw(frame: &mut Frame, app: &App, now: Instant) {
    let tr = |key| app.locale.text(key);
    let status = app.status(now);
    let status_text = if let Some(e) = app.error {
        tr(match e {
            ReadError::Unavailable => "tui.unavailable",
            ReadError::Invalid => "tui.invalid",
            ReadError::Incompatible => "tui.incompatible",
            ReadError::Changed => "tui.changed",
        })
    } else if app.snapshot.is_some() && !app.fresh(now) {
        tr("tui.stale")
    } else {
        tr(match status {
            Status::Connected => "status.connected",
            Status::Disconnected => "status.disconnected",
            Status::Recovery => "tui.recovery",
            Status::Unverified => "tui.unverified",
        })
    };
    let area = frame.area();
    if area.width < 50 || area.height < 14 {
        frame.render_widget(
            Paragraph::new(format!(
                "OmaVLESS\n{status_text}\n{}\n{}",
                tr("tui.resize"),
                tr(if app.searching {
                    "tui.search_exit_hint"
                } else {
                    "tui.close_hint"
                })
            ))
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let sections = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(3),
        Constraint::Length(5),
    ])
    .split(area);
    let active = app
        .snapshot
        .as_ref()
        .filter(|_| status == Status::Connected)
        .and_then(|s| {
            s.metadata
                .profiles
                .iter()
                .find(|p| p.id == s.metadata.desired.profile_id)
        })
        .map(|p| display(&p.name, 80))
        .unwrap_or_else(|| {
            tr(if status == Status::Disconnected {
                "tui.none"
            } else {
                "tui.unverified"
            })
            .into()
        });
    let mode = app
        .snapshot
        .as_ref()
        .map(|s| {
            tr(match s.metadata.desired.mode {
                Mode::Rule => "mode.routing",
                Mode::Global => "mode.full_vpn",
                Mode::Direct => "mode.direct",
            })
        })
        .unwrap_or("—");
    let header = vec![
        Line::from(format!("{status_text}  |  {mode}")),
        Line::from(format!("{}: {active}", tr("tui.active"))),
        Line::from(tr("tui.health")),
    ];
    frame.render_widget(
        Paragraph::new(header).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" OmaVLESS · {} ", tr("tui.readonly"))),
        ),
        sections[0],
    );
    if app.help {
        frame.render_widget(
            Paragraph::new(tr("tui.help"))
                .block(Block::default().borders(Borders::ALL).title(" ? "))
                .wrap(Wrap { trim: false }),
            sections[1],
        );
    } else {
        let visible = app.visible();
        let rows: Vec<Row> = app.snapshot.as_ref().map_or_else(Vec::new, |s| {
            visible
                .iter()
                .map(|i| {
                    let p = &s.metadata.profiles[*i];
                    let source = p
                        .subscription_id
                        .as_ref()
                        .and_then(|id| s.metadata.subscriptions.iter().find(|sub| &sub.id == id))
                        .map(|sub| display(&sub.name, 80))
                        .unwrap_or_else(|| tr("tui.local").into());
                    let badge =
                        if status == Status::Connected && p.id == s.metadata.desired.profile_id {
                            tr("status.connected")
                        } else if p.missing {
                            tr("tui.missing")
                        } else {
                            ""
                        };
                    Row::new(vec![
                        format!(
                            "{}{}",
                            if p.favorite { "* " } else { "" },
                            display(&p.name, 80)
                        ),
                        source,
                        badge.into(),
                    ])
                })
                .collect()
        });
        let count = if app.snapshot.is_some() {
            visible.len().to_string()
        } else {
            "—".into()
        };
        let title = format!(" {} ({count}) ", tr("tui.profiles"));
        if rows.is_empty() {
            let key = if app.snapshot.is_none() {
                "tui.unavailable"
            } else if app.query.is_empty() {
                "tui.empty"
            } else {
                "tui.no_matches"
            };
            frame.render_widget(
                Paragraph::new(tr(key))
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL).title(title)),
                sections[1],
            );
        } else {
            let selected = app.snapshot.as_ref().and_then(|s| {
                visible
                    .iter()
                    .position(|i| app.selected.as_ref() == Some(&s.metadata.profiles[*i].id))
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ],
            )
            .header(
                Row::new(vec![tr("tui.profiles"), tr("tui.source"), ""])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(Block::default().borders(Borders::ALL).title(title))
            .row_highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
            .highlight_symbol("> ");
            frame.render_stateful_widget(
                table,
                sections[1],
                &mut TableState::default().with_selected(selected),
            );
        }
    }
    let selected = app
        .snapshot
        .as_ref()
        .and_then(|s| {
            s.metadata
                .profiles
                .iter()
                .find(|p| app.selected.as_ref() == Some(&p.id))
        })
        .map(|p| display(&p.name, 80))
        .unwrap_or_else(|| tr("tui.none").into());
    let footer = vec![
        Line::from(format!(
            "{}: {}{}",
            tr("tui.search"),
            display(&app.query, 80),
            if app.searching { "▏" } else { "" }
        )),
        Line::from(format!("{}: {selected}", tr("tui.selected"))),
        Line::from(tr("tui.keys")),
        Line::from(tr(if app.searching {
            "tui.search_exit_hint"
        } else {
            "tui.close_hint"
        })),
    ];
    // Fixed rows: long private names/search must never push the exit hint away.
    frame.render_widget(Paragraph::new(footer), sections[2]);
}
