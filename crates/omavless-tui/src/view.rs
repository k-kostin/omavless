// SPDX-License-Identifier: MIT
use crate::{
    app::{App, Confirmation},
    browsing,
    model::{Mode, ReadError, Status, display},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap},
};
use std::time::Instant;

pub fn draw(frame: &mut Frame, app: &App, now: Instant) {
    let tr = |key| app.locale.text(key);
    let status = app.status(now);
    let status_text = if app.running {
        tr("tui.action_pending")
    } else if let Some(e) = app.error {
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
    frame.render_widget(Block::default().style(app.palette.normal()), area);
    if area.width < if app.actions_enabled { 70 } else { 50 }
        || area.height < if app.actions_enabled { 24 } else { 14 }
    {
        frame.render_widget(
            Paragraph::new(format!(
                "OmaVLESS\n{status_text}\n{}\n{}",
                tr(if app.actions_enabled {
                    "tui.action_resize"
                } else {
                    "tui.resize"
                }),
                tr(if app.searching && app.actions_enabled {
                    "tui.action_search_exit"
                } else if app.searching {
                    "tui.search_exit_hint"
                } else if app.actions_enabled {
                    "tui.action_close_hint"
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
        Constraint::Length(if app.actions_enabled { 8 } else { 5 }),
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
        Paragraph::new(header).block(Block::default().borders(Borders::ALL).title(format!(
            " OmaVLESS · {} ",
            tr(if app.actions_enabled {
                "tui.controls"
            } else {
                "tui.readonly"
            })
        ))),
        sections[0],
    );
    if let Some(confirmation) = &app.confirmation {
        let command = match confirmation {
            Confirmation::New(command) => Some(command),
            Confirmation::Retry => app.pending.as_ref().map(|r| &r.command),
            Confirmation::Acknowledge { .. } => None,
        };
        let mut lines = Vec::new();
        if let Some(command) = command {
            lines.push(Line::from(tr(command.kind.key())));
            lines.push(Line::from(format!(
                "{}: {}",
                tr("tui.target"),
                if command.name.is_empty() {
                    tr("tui.current_session").into()
                } else {
                    display(&command.name, 80)
                }
            )));
            if matches!(
                command.kind,
                crate::actions::Kind::Connect | crate::actions::Kind::Mode
            ) {
                lines.push(Line::from(tr(command.mode.key())));
            }
            if !command.name.is_empty() {
                lines.push(Line::from(format!(
                    "{}: {}",
                    tr("tui.source"),
                    command
                        .source
                        .as_ref()
                        .map(|s| display(s, 80))
                        .unwrap_or_else(|| tr("tui.local").into())
                )));
            }
            if command.kind == crate::actions::Kind::Mode && !command.was_connected {
                lines.push(Line::from(tr("tui.saved_mode_only")));
            }
        }
        lines.push(Line::from(tr(match confirmation {
            Confirmation::New(command)
                if command.kind == crate::actions::Kind::RefreshSubscription =>
            {
                "tui.confirm_subscription_refresh"
            }
            Confirmation::New(_) => "tui.confirm_network",
            Confirmation::Retry => "tui.confirm_retry",
            Confirmation::Acknowledge { .. } => "tui.confirm_ack",
        })));
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(tr("tui.confirm")),
            ),
            sections[1],
        );
    } else if app.help {
        frame.render_widget(
            Paragraph::new(tr(if app.actions_enabled {
                "tui.action_help"
            } else {
                "tui.help"
            }))
            .block(Block::default().borders(Borders::ALL).title(" ? "))
            .wrap(Wrap { trim: false }),
            sections[1],
        );
    } else if app.page != crate::inspection::Page::Profiles {
        let lines = inspection_lines(app, now);
        // Even at a large viewport/key-repeat, never scroll past all content.
        let scroll = app
            .inspection_scroll
            .min(lines.len().saturating_sub(1) as u16);
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(tr(app.page.key())),
                ),
            sections[1],
        );
    } else {
        let visible = app.visible();
        let entries = app
            .snapshot
            .as_ref()
            .map_or_else(Vec::new, |s| browsing::rows(s, &visible));
        let rows: Vec<Row> = app.snapshot.as_ref().map_or_else(Vec::new, |s| {
            entries
                .iter()
                .map(|entry| {
                    let browsing::Row::Profile(i) = entry else {
                        let browsing::Row::Group { name, count } = entry else {
                            unreachable!()
                        };
                        let name = name
                            .as_ref()
                            .map(|n| display(n, 80))
                            .unwrap_or_else(|| tr("tui.local_profiles").into());
                        return Row::new(vec![format!("{name} ({count})"), String::new()])
                            .style(Style::default().add_modifier(Modifier::BOLD));
                    };
                    let p = &s.metadata.profiles[*i];
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
                            "  {}{}",
                            if p.favorite { "* " } else { "" },
                            display(&p.name, 80)
                        ),
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
        let title = format!(
            " {} ({count}) ",
            tr(if app.favorites_only {
                "tui.favorites"
            } else {
                "tui.profiles"
            })
        );
        if rows.is_empty() {
            let key = if app.snapshot.is_none() {
                "tui.unavailable"
            } else if app.favorites_only {
                "tui.no_favorites"
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
                entries.iter().position(|row| match row {
                    browsing::Row::Profile(i) => {
                        app.selected.as_ref() == Some(&s.metadata.profiles[*i].id)
                    }
                    browsing::Row::Group { .. } => false,
                })
            });
            let table = Table::new(
                rows,
                [Constraint::Percentage(75), Constraint::Percentage(25)],
            )
            .header(
                Row::new(vec![tr("tui.profiles"), ""])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(Block::default().borders(Borders::ALL).title(title))
            .row_highlight_style(app.palette.selected())
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
    let mut footer = vec![
        Line::from(format!(
            "{}: {}{}",
            tr("tui.search"),
            display(&app.query, 80),
            if app.searching { "▏" } else { "" }
        )),
        Line::from(format!(
            "{}: {selected}",
            tr(if app.actions_enabled {
                "tui.action_selected"
            } else {
                "tui.selected"
            })
        )),
        Line::from(tr("tui.keys")),
    ];
    if app.actions_enabled {
        footer.push(Line::from(tr("tui.action_keys")));
        footer.push(Line::from(tr(if app.confirmation.is_some() {
            "tui.confirm_keys"
        } else if app.notice.is_empty() {
            "tui.confirm_required"
        } else {
            app.notice
        })));
        footer.push(Line::from(tr(if app.unknown {
            "tui.unknown_keys"
        } else if app.confirmation.is_some() || app.running {
            "tui.auth_hint"
        } else {
            "tui.subscription_keys"
        })));
    }
    footer.push(Line::from(tr("tui.page_keys")));
    footer.push(Line::from(tr(if app.searching && app.actions_enabled {
        "tui.action_search_exit"
    } else if app.searching {
        "tui.search_exit_hint"
    } else if app.actions_enabled {
        "tui.action_close_hint"
    } else {
        "tui.close_hint"
    })));
    // Fixed rows: long private names/search must never push the exit hint away.
    if app.page != crate::inspection::Page::Profiles && app.confirmation.is_none() {
        footer = vec![
            Line::from(tr("tui.page_keys")),
            Line::from(tr("tui.inspection_keys")),
            Line::from(tr(if app.notice.is_empty() {
                "tui.readonly"
            } else {
                app.notice
            })),
            Line::from(tr(if app.actions_enabled {
                "tui.action_close_hint"
            } else {
                "tui.close_hint"
            })),
        ];
    }
    frame.render_widget(Paragraph::new(footer), sections[2]);
}

fn inspection_lines(app: &App, now: Instant) -> Vec<Line<'static>> {
    use crate::inspection::{Page, bytes};
    let tr = |key| app.locale.text(key);
    let field = |key, value: String| Line::from(format!("{}: {value}", tr(key)));
    let Some(s) = app
        .snapshot
        .as_ref()
        .filter(|_| app.fresh(now) && !app.running)
    else {
        return vec![Line::from(tr("tui.stale"))];
    };
    let unknown = || tr("tui.metric_unavailable").to_owned();
    let boolean = |value: bool| tr(if value { "tui.yes" } else { "tui.no" }).to_owned();
    match app.page {
        Page::Traffic => vec![
            field(
                "tui.upload_rate",
                app.traffic_rates
                    .map(|(u, _)| format!("{}/s", bytes(u)))
                    .unwrap_or_else(unknown),
            ),
            field(
                "tui.download_rate",
                app.traffic_rates
                    .map(|(_, d)| format!("{}/s", bytes(d)))
                    .unwrap_or_else(unknown),
            ),
            field(
                "tui.upload_total",
                s.traffic
                    .as_ref()
                    .map(|t| bytes(t.upload))
                    .unwrap_or_else(unknown),
            ),
            field(
                "tui.download_total",
                s.traffic
                    .as_ref()
                    .map(|t| bytes(t.download))
                    .unwrap_or_else(unknown),
            ),
            Line::from(tr("tui.traffic_scope")),
            Line::from(tr("tui.traffic_reset")),
            Line::from(tr("tui.connections_deferred")),
        ],
        Page::Details => {
            let p = s
                .metadata
                .profiles
                .iter()
                .find(|p| app.selected.as_ref() == Some(&p.id));
            let Some(p) = p else {
                return vec![Line::from(tr("tui.select_details"))];
            };
            vec![
                field("tui.action_selected", display(&p.name, 80)),
                field(
                    "tui.source",
                    p.subscription_id
                        .as_ref()
                        .and_then(|id| s.metadata.subscriptions.iter().find(|sub| sub.id == *id))
                        .map(|sub| display(&sub.name, 80))
                        .unwrap_or_else(|| tr("tui.local").into()),
                ),
                field("tui.favorite", boolean(p.favorite)),
                field("tui.feed_available", boolean(!p.missing)),
                field(
                    "tui.is_connected",
                    if matches!(s.status(), Status::Unverified | Status::Recovery) {
                        unknown()
                    } else {
                        boolean(
                            s.status() == Status::Connected
                                && s.metadata.desired.profile_id == p.id,
                        )
                    },
                ),
                Line::from(tr("tui.details_privacy")),
                Line::from(tr("tui.health")),
            ]
        }
        Page::Diagnostics => {
            let facts = s.observation.facts.as_ref();
            vec![
                field("tui.core", "Mihomo".into()),
                field(
                    "tui.core_running",
                    facts
                        .map(|f| boolean(f.owned_core_running))
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.controller",
                    facts
                        .map(|f| boolean(f.owned_controller_config_verified))
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_count",
                    facts
                        .map(|f| f.visible_mihomo_count.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.tun_count",
                    facts
                        .map(|f| f.managed_tun_count.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.rules_count",
                    s.diagnostics
                        .as_ref()
                        .map(|d| d.rules.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.providers_count",
                    s.diagnostics
                        .as_ref()
                        .map(|d| d.providers.to_string())
                        .unwrap_or_else(unknown),
                ),
                Line::from(tr("tui.health")),
                Line::from(tr("tui.no_killswitch")),
            ]
        }
        Page::Profiles => Vec::new(),
    }
}
