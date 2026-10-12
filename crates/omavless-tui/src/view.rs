// SPDX-License-Identifier: MIT
use crate::{
    app::{App, Confirmation},
    browsing,
    model::{Actual, Mode, ReadError, Status, display},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap},
};
use std::time::Instant;

/// Retain the actual wrapped scroll offset before the next key event, so Up
/// immediately works after End, resize or replacement by a shorter snapshot.
pub fn clamp_scroll(app: &mut App, width: u16, height: u16, now: Instant) {
    if app.page == crate::inspection::Page::Profiles || app.help || app.confirmation.is_some() {
        return;
    }
    let body_height = height.saturating_sub(if app.actions_enabled { 15 } else { 12 });
    let lines = inspection_lines(app, now);
    if app.page == crate::inspection::Page::Subscriptions && app.subscription_selection_moved {
        if let Some(index) = lines
            .iter()
            .position(|line| line.to_string().starts_with("> "))
        {
            app.inspection_scroll = Paragraph::new(lines[..index].to_vec())
                .wrap(Wrap { trim: false })
                .line_count(width.saturating_sub(2))
                .min(u16::MAX as usize) as u16;
        }
        app.subscription_selection_moved = false;
    }
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let limit = paragraph
        .line_count(width.saturating_sub(2))
        .saturating_sub(body_height as usize)
        .min(u16::MAX as usize) as u16;
    app.inspection_scroll = app.inspection_scroll.min(limit);
}

pub fn draw(frame: &mut Frame, app: &App, now: Instant) {
    #[cfg(feature = "private-backup")]
    if app.restore_open
        && let Some(restore) = &app.restore
    {
        draw_private_restore(frame, app, restore);
        return;
    }
    #[cfg(feature = "private-backup")]
    if app.backup_open
        && let Some(backup) = &app.backup
    {
        draw_private_backup(frame, app, backup);
        return;
    }
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
        Constraint::Length(if app.confirmation.is_some() {
            4
        } else if app.actions_enabled {
            8
        } else {
            5
        }),
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
            Confirmation::Job(_) | Confirmation::CancelJob => None,
        };
        let mut lines = Vec::new();
        let job_intent = match confirmation {
            Confirmation::Job(intent) => Some(intent),
            Confirmation::CancelJob => app.job.as_ref().map(|job| &job.intent),
            _ => None,
        };
        if let Some(intent) = job_intent {
            lines.push(Line::from(tr(intent.key())));
            lines.push(Line::from(format!(
                "{}: {} ({})",
                tr("tui.target"),
                if intent.label.is_empty() {
                    tr("tui.all_saved").into()
                } else {
                    display(&intent.label, 80)
                },
                intent.count
            )));
            lines.push(Line::from(tr(
                if intent.kind == crate::jobs::Kind::RefreshAll {
                    "tui.confirm_refresh_all"
                } else {
                    "tui.probe_scope"
                },
            )));
        }
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
            if !command.name.is_empty() && !command.profile.is_empty() {
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
            Confirmation::Job(_) => "tui.confirm_job",
            Confirmation::CancelJob => "tui.confirm_cancel_job",
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
        // Count rendered rows, including wrapping: End must show the last
        // subscription, not an empty trailing line or an unreachable tail.
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        let height = sections[1].height.saturating_sub(2) as usize;
        let max_scroll = paragraph
            .line_count(sections[1].width.saturating_sub(2))
            .saturating_sub(height)
            .min(u16::MAX as usize) as u16;
        let scroll = app.inspection_scroll.min(max_scroll);
        frame.render_widget(
            paragraph.scroll((scroll, 0)).block(
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
    let selected = if app.page == crate::inspection::Page::Subscriptions {
        app.snapshot
            .as_ref()
            .and_then(|s| {
                s.metadata
                    .subscriptions
                    .iter()
                    .find(|sub| Some(&sub.id) == app.selected_subscription.as_ref())
            })
            .map(|s| display(&s.name, 80))
            .unwrap_or_else(|| tr("tui.none").into())
    } else {
        app.snapshot
            .as_ref()
            .and_then(|s| {
                s.metadata
                    .profiles
                    .iter()
                    .find(|p| app.selected.as_ref() == Some(&p.id))
            })
            .map(|p| display(&p.name, 80))
            .unwrap_or_else(|| tr("tui.none").into())
    };
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
            "tui.profile_check_keys"
        })));
    }
    footer.push(Line::from(tr(page_keys_key(app))));
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
            Line::from(tr(page_keys_key(app))),
            Line::from(tr(if app.page == crate::inspection::Page::Jobs {
                "tui.job_keys"
            } else if app.page == crate::inspection::Page::Subscriptions
                && app
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.capabilities.subscription_usage)
            {
                if app.actions_enabled {
                    "tui.subscription_usage_keys"
                } else {
                    "tui.subscription_usage_read_keys"
                }
            } else if app.page == crate::inspection::Page::Subscriptions && app.actions_enabled {
                "tui.subscription_page_keys"
            } else if app.page == crate::inspection::Page::Settings {
                settings_keys_key(app)
            } else if app.page == crate::inspection::Page::Diagnostics {
                "tui.diagnostic_keys"
            } else if app.page == crate::inspection::Page::Host {
                "tui.host_keys"
            } else if app.page == crate::inspection::Page::RouteCheck {
                "tui.route_check_keys"
            } else if app.page == crate::inspection::Page::Connections {
                "tui.connection_rows_keys"
            } else if matches!(
                app.page,
                crate::inspection::Page::Connections
                    | crate::inspection::Page::Rules
                    | crate::inspection::Page::Providers
                    | crate::inspection::Page::CustomRules
            ) {
                "tui.operator_keys"
            } else {
                "tui.inspection_keys"
            })),
            Line::from(tr(if app.notice.is_empty() {
                if app.searching
                    && matches!(
                        app.page,
                        crate::inspection::Page::Connections
                            | crate::inspection::Page::Rules
                            | crate::inspection::Page::Providers
                            | crate::inspection::Page::CustomRules
                    )
                {
                    "tui.operator_search_exit"
                } else if app.page == crate::inspection::Page::Settings {
                    settings_scope_key(app)
                } else if app.page == crate::inspection::Page::Subscriptions
                    && app
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.capabilities.subscription_usage)
                {
                    "tui.usage_action_notice"
                } else if app.page == crate::inspection::Page::Subscriptions
                    || app.page == crate::inspection::Page::Jobs
                {
                    "tui.confirm_required"
                } else {
                    "tui.readonly"
                }
            } else {
                app.notice
            })),
            Line::from(tr(if app.actions_enabled {
                "tui.action_close_hint"
            } else {
                "tui.close_hint"
            })),
        ];
        if app.page == crate::inspection::Page::Subscriptions && app.actions_enabled {
            let target = app.snapshot.as_ref().and_then(|s| {
                s.metadata
                    .subscriptions
                    .iter()
                    .find(|sub| Some(&sub.id) == app.selected_subscription.as_ref())
            });
            footer.insert(
                0,
                Line::from(format!(
                    "{}: {}",
                    tr("tui.target"),
                    target
                        .map(|s| display(&s.name, 80))
                        .unwrap_or_else(|| tr("tui.none").into())
                )),
            );
        }
    }
    if app.confirmation.is_some() {
        footer = vec![
            Line::from(tr("tui.confirm_keys")),
            Line::from(tr("tui.auth_hint")),
            Line::from(tr("tui.action_close_hint")),
        ];
    }
    #[cfg(feature = "private-backup")]
    if app.restore_unresolved() {
        footer = vec![
            Line::from(tr("tui.restore_pending_entry")),
            Line::from(tr("tui.restore_unresolved_keys")),
        ];
        frame.render_widget(
            Paragraph::new(footer).wrap(Wrap { trim: false }),
            sections[2],
        );
        return;
    }
    #[cfg(feature = "private-backup")]
    if app.backup_unresolved() {
        footer = vec![
            Line::from(tr("tui.backup_pending_entry")),
            Line::from(tr("tui.backup_unresolved_keys")),
            Line::from(tr(
                if app
                    .backup
                    .as_ref()
                    .is_some_and(|w| w.state() == crate::private_backup::State::Unknown)
                {
                    "tui.backup_unknown"
                } else {
                    "tui.backup_submitted"
                },
            )),
        ];
        frame.render_widget(
            Paragraph::new(footer).wrap(Wrap { trim: false }),
            sections[2],
        );
        return;
    }
    frame.render_widget(Paragraph::new(footer), sections[2]);
}

fn inspection_lines(app: &App, now: Instant) -> Vec<Line<'static>> {
    use crate::inspection::{Page, bytes};
    let tr = |key| app.locale.text(key);
    let field = |key, value: String| Line::from(format!("{}: {value}", tr(key)));
    if app.page == Page::Jobs {
        let Some(job) = &app.job else {
            return vec![Line::from(tr("tui.job_empty"))];
        };
        let mut lines = vec![
            Line::from(tr(job.intent.key())),
            field(
                "tui.target",
                format!(
                    "{} ({})",
                    if job.intent.label.is_empty() {
                        tr("tui.all_saved").into()
                    } else {
                        display(&job.intent.label, 80)
                    },
                    job.intent.count
                ),
            ),
            Line::from(tr(job.notice)),
        ];
        if let Some(p) = job.tracker.progress {
            lines.push(field(
                "tui.progress",
                format!("{} / {}", p.completed, p.total),
            ));
        }
        lines.push(Line::from(tr("tui.job_close")));
        if job.intent.kind != crate::jobs::Kind::RefreshAll {
            lines.push(Line::from(tr("tui.probe_scope")));
        }
        if let Some(rows) = &job.rows {
            if let Some(s) = app
                .snapshot
                .as_ref()
                .filter(|_| app.fresh(now))
                .filter(|s| job.intent.matches(s))
            {
                for row in rows {
                    if let Some(p) = s.metadata.profiles.iter().find(|p| p.id == row.id) {
                        let result =
                            row.latency_ms
                                .map(|ms| format!("{ms} ms"))
                                .unwrap_or_else(|| {
                                    tr(if row.resolved {
                                        "tui.probe_unreachable"
                                    } else {
                                        "tui.probe_unresolved"
                                    })
                                    .into()
                                });
                        lines.push(Line::from(format!("{}: {result}", display(&p.name, 80))));
                    }
                }
            } else {
                lines.push(Line::from(tr("tui.job_results_stale")));
            }
        }
        return lines;
    }
    // Presentation settings work offline and never imply runtime readiness.
    if app.page == Page::Settings {
        let language = if app.settings.language == crate::settings::Language::Automatic {
            format!(
                "{} ({})",
                tr(app.settings.language.key()),
                tr(if app.locale == crate::i18n::Locale::Ru {
                    "tui.language_ru"
                } else {
                    "tui.language_en"
                })
            )
        } else {
            tr(app.settings.language.key()).into()
        };
        #[allow(unused_mut)] // augmented only in the explicit Backup client build
        let mut settings = vec![
            Line::from(tr("tui.settings_scope")),
            Line::from(""),
            field("tui.settings_language", language),
            Line::from(tr("tui.language_hint")),
            Line::from(""),
            field("tui.settings_theme", tr(app.settings.theme.key()).into()),
            Line::from(tr("tui.theme_hint")),
            Line::from(""),
            Line::from(tr("tui.settings_reset")),
        ];
        #[cfg(feature = "private-backup")]
        if app.restore_enabled {
            settings.push(Line::from(""));
            settings.push(Line::from(tr("tui.restore_scope")));
            settings.push(Line::from(tr(if app.restore_unresolved() {
                "tui.restore_pending_entry"
            } else if app.restore_available(now) {
                "tui.restore_entry"
            } else {
                "tui.restore_unavailable"
            })));
        }
        #[cfg(feature = "private-backup")]
        if app.backup_enabled {
            settings.push(Line::from(""));
            settings.push(Line::from(tr("tui.backup_scope")));
            settings.push(Line::from(tr(if app.backup_unresolved() {
                "tui.backup_pending_entry"
            } else if app.backup_available(now) {
                "tui.backup_entry"
            } else {
                "tui.backup_unavailable"
            })));
        }
        return settings;
    }
    // Session history remains readable when the daemon is unavailable/stale;
    // the separate header continues to describe current freshness/health.
    if app.page == Page::Activity {
        let mut lines = vec![Line::from(tr("tui.activity_scope"))];
        if app.activity.older_events_discarded() {
            lines.push(Line::from(tr("tui.activity_discarded")));
        }
        for entry in app.activity.newest_first() {
            let seconds = entry.elapsed_seconds;
            lines.push(Line::from(format!(
                "[{:02}:{:02}:{:02}] {}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60,
                tr(entry.event.key())
            )));
        }
        if lines.len() == 1 {
            lines.push(Line::from(tr("tui.activity_empty")));
        }
        return lines;
    }
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
        Page::Connections => {
            let mut lines = vec![Line::from(tr("tui.connection_rows_scope"))];
            lines.push(field(
                "tui.connection_order",
                tr(app.connection_order.key()).into(),
            ));
            let Some(connections) = &s.connection_rows else {
                lines.push(Line::from(tr(
                    if app.snapshot_page == Some(Page::Connections) {
                        "tui.metric_unavailable"
                    } else {
                        "tui.connection_rows_loading"
                    },
                )));
                return lines;
            };
            lines.push(field(
                "tui.active_connections",
                connections.total.to_string(),
            ));
            lines.push(field(
                "tui.connection_rows_shown",
                connections.rows.len().to_string(),
            ));
            let matches = crate::connection_browsing::rows(
                connections,
                &app.operator_query,
                app.connection_order,
            );
            if !app.operator_query.is_empty() || app.searching {
                lines.push(field(
                    "tui.operator_filter",
                    display(&app.operator_query, 80),
                ));
                lines.push(field("tui.matching_rows", matches.len().to_string()));
            }
            if connections.truncated {
                lines.push(Line::from(tr("tui.connection_rows_truncated")));
            }
            lines.push(Line::from(""));
            if matches.is_empty() {
                lines.push(Line::from(tr(if connections.rows.is_empty() {
                    "tui.connection_rows_empty"
                } else {
                    "tui.no_operator_matches"
                })));
            }
            for row in matches {
                let destination = row
                    .host
                    .as_deref()
                    .or(row.ip.as_deref())
                    .map(|destination| display(destination, 64))
                    .unwrap_or_else(|| tr("tui.metric_unavailable").to_owned());
                let endpoint = row
                    .port
                    .map_or(destination.clone(), |port| format!("{destination}:{port}"));
                lines.push(Line::from(format!(
                    "{} · {} · {}",
                    endpoint,
                    row.network,
                    tr(match row.route {
                        "direct" => "tui.connection_route_direct",
                        "blocked" => "tui.connection_route_blocked",
                        "vpn" => "tui.connection_route_vpn",
                        _ => "tui.connection_unclassified",
                    })
                )));
                if row.host.is_some()
                    && let Some(ip) = &row.ip
                {
                    lines.push(Line::from(format!("  {}", display(ip, 64))));
                }
            }
            lines
        }
        Page::RouteCheck => {
            use crate::route_inspection::{Outcome, Source, Status};
            let mut lines = vec![
                Line::from(tr("tui.route_check_scope")),
                Line::from(""),
                field("tui.route_check_query", display(&app.route_query, 80)),
                Line::from(tr(if app.route_editing {
                    "tui.route_check_editing"
                } else {
                    "tui.route_check_instruction"
                })),
                Line::from(""),
            ];
            match &app.route_result {
                Some(Status::Observed(result)) => {
                    if let Some(checked_at) = app.route_checked_at {
                        let (key, count) = crate::inspection::observed_age(checked_at, now);
                        let age = count.map_or_else(
                            || tr(key).to_owned(),
                            |n| tr(key).replace("{count}", &n.to_string()),
                        );
                        lines.push(field("tui.route_check_age", age));
                    }
                    let outcome = match result.outcome {
                        Outcome::Vpn => "tui.connection_route_vpn",
                        Outcome::Direct => "tui.connection_route_direct",
                        Outcome::Block => "tui.connection_route_blocked",
                        Outcome::Unknown => "tui.connection_unclassified",
                    };
                    let source = match result.source {
                        Source::Mode => "tui.route_source_mode",
                        Source::Custom => "tui.route_source_custom",
                        Source::Live => "tui.route_source_live",
                        Source::Disconnected => "tui.route_source_disconnected",
                    };
                    lines.push(field("tui.route_check_result", tr(outcome).to_owned()));
                    lines.push(field("tui.route_check_source", tr(source).to_owned()));
                    if !result.rule_type.is_empty() {
                        lines.push(field(
                            "tui.route_check_rule",
                            display(&result.rule_type, 80),
                        ));
                    }
                    if !result.rule_payload.is_empty() {
                        lines.push(field(
                            "tui.route_check_match",
                            display(&result.rule_payload, 80),
                        ));
                    }
                }
                Some(Status::Unavailable) => {
                    lines.push(Line::from(tr("tui.route_check_unavailable")))
                }
                Some(Status::Unsupported) => {
                    lines.push(Line::from(tr("tui.route_check_unsupported")))
                }
                Some(Status::InvalidInput) => lines.push(Line::from(tr("tui.route_check_invalid"))),
                None => lines.push(Line::from(tr("tui.route_check_empty"))),
            }
            lines
        }
        Page::Traffic => {
            let mut lines = vec![
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
                field(
                    "tui.active_connections",
                    s.active_connections
                        .map(|n| n.to_string())
                        .unwrap_or_else(unknown),
                ),
                Line::from(tr("tui.connection_count_scope")),
            ];
            if let Some(overview) = s.connection_overview {
                lines.push(Line::from(""));
                lines.push(Line::from(tr("tui.connection_categories_scope")));
                lines.push(field(
                    "tui.connection_network_counts",
                    format!(
                        "TCP {} · UDP {} · {} {}",
                        overview.tcp,
                        overview.udp,
                        tr("tui.connection_other"),
                        overview.other_network
                    ),
                ));
                lines.push(field(
                    "tui.connection_outcome_counts",
                    format!(
                        "DIRECT {} · REJECT {} · PROXY {} · {} {}",
                        overview.direct,
                        overview.blocked,
                        overview.vpn,
                        tr("tui.connection_unclassified"),
                        overview.unclassified
                    ),
                ));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.traffic_history_scope")));
            let samples = app.traffic_history.recent(now).count();
            if samples == 0 {
                lines.push(Line::from(tr("tui.traffic_history_unavailable")));
            } else {
                lines.push(field("tui.traffic_history_samples", samples.to_string()));
                for (upload, label) in [
                    (true, "tui.upload_history"),
                    (false, "tui.download_history"),
                ] {
                    let graph = app
                        .traffic_history
                        .sparkline(now, upload)
                        .unwrap_or_default();
                    let peak = app.traffic_history.peak(now, upload).unwrap_or_default();
                    lines.push(
                        Line::from(format!(
                            "{}: {}  ({}: {}/s)",
                            tr(label),
                            graph,
                            tr("tui.traffic_history_peak"),
                            bytes(peak)
                        ))
                        .style(if upload {
                            Style::default().fg(app.palette.accent)
                        } else {
                            Style::default().fg(app.palette.foreground)
                        }),
                    );
                }
                if app.traffic_history.has_long_trend(now) {
                    lines.push(Line::from(""));
                    lines.push(Line::from(tr("tui.traffic_history_long_scope")));
                    for (upload, label) in [
                        (true, "tui.upload_history"),
                        (false, "tui.download_history"),
                    ] {
                        let graph = app
                            .traffic_history
                            .sparkline_window(now, upload, crate::traffic_history::LONG_WINDOW)
                            .unwrap_or_default();
                        let peak = app
                            .traffic_history
                            .peak_window(now, upload, crate::traffic_history::LONG_WINDOW)
                            .unwrap_or_default();
                        lines.push(
                            Line::from(format!(
                                "{}: {}  ({}: {}/s)",
                                tr(label),
                                graph,
                                tr("tui.traffic_history_peak"),
                                bytes(peak)
                            ))
                            .style(if upload {
                                Style::default().fg(app.palette.accent)
                            } else {
                                Style::default().fg(app.palette.foreground)
                            }),
                        );
                    }
                }
            }
            lines
        }
        Page::Details => {
            let p = s
                .metadata
                .profiles
                .iter()
                .find(|p| app.selected.as_ref() == Some(&p.id));
            let Some(p) = p else {
                return vec![Line::from(tr("tui.select_details"))];
            };
            let details = s
                .profile_details
                .as_ref()
                .filter(|d| d.profile_id() == p.id);
            vec![
                field("tui.action_selected", display(&p.name, 80)),
                field(
                    "tui.protocol",
                    details
                        .and_then(|d| d.protocol)
                        .map(str::to_owned)
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.transport",
                    details
                        .and_then(|d| d.transport)
                        .map(str::to_owned)
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.security",
                    details
                        .and_then(|d| d.security)
                        .map(str::to_owned)
                        .unwrap_or_else(unknown),
                ),
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
                field("tui.core", "Mihomo".into()),
                field("tui.cap_probe", boolean(s.capabilities.profile_probe)),
                field(
                    "tui.cap_refresh",
                    boolean(s.capabilities.subscription_refresh),
                ),
                Line::from(tr("tui.details_categories")),
                Line::from(tr("tui.health")),
            ]
        }
        Page::Diagnostics => {
            let facts = s.observation.facts.as_ref();
            let mut lines = vec![
                Line::from(tr("tui.doctor_scope")),
                field(
                    "tui.doctor_requested",
                    boolean(s.observation.desired.connected),
                ),
                field(
                    "tui.doctor_last_actual",
                    tr(match s.observation.last_known_actual {
                        Actual::Disconnected => "tui.doctor_disconnected",
                        Actual::Starting => "tui.doctor_starting",
                        Actual::Connected => "tui.doctor_connected",
                        Actual::Reconnecting => "tui.doctor_reconnecting",
                        Actual::Stopping => "tui.doctor_stopping",
                        Actual::Failed => "tui.doctor_failed",
                        Actual::ManualRecoveryRequired => "tui.doctor_recovery",
                    })
                    .into(),
                ),
                field(
                    "tui.doctor_profile_match",
                    facts
                        .map(|f| boolean(f.desired_profile_matches_owned))
                        .unwrap_or_else(unknown),
                ),
                Line::from(tr("tui.doctor_inventory_scope")),
                Line::from(""),
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
                Line::from(""),
                Line::from(tr("tui.core_log_scope")),
                field(
                    "tui.core_log_dns",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| d.dns.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_log_tls",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| d.tls.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_log_timeout",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| d.timeout.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_log_connection",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| d.connection.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_log_other",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| d.other.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_log_oversized",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| d.oversized.to_string())
                        .unwrap_or_else(unknown),
                ),
                field(
                    "tui.core_log_incomplete",
                    s.core_diagnostics
                        .as_ref()
                        .map(|d| boolean(d.incomplete))
                        .unwrap_or_else(unknown),
                ),
            ];
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.core_log_recent")));
            if let Some(hints) = &s.core_log_hints {
                if hints.items.is_empty() {
                    lines.push(Line::from(tr("tui.core_log_no_recent")));
                }
                for hint in hints.items.iter().rev().take(8).rev() {
                    let key = match hint.category {
                        "dns" => "tui.core_log_dns",
                        "tls" => "tui.core_log_tls",
                        "timeout" => "tui.core_log_timeout",
                        "connection" => "tui.core_log_connection",
                        "other" => "tui.core_log_other",
                        "oversized" => "tui.core_log_oversized",
                        _ => "tui.metric_unavailable",
                    };
                    lines.push(Line::from(format!("#{} {}", hint.sequence, tr(key))));
                }
                if hints.incomplete {
                    lines.push(Line::from(tr("tui.core_log_incomplete")));
                }
            } else {
                lines.push(Line::from(tr("tui.metric_unavailable")));
            }
            lines.push(Line::from(tr("tui.core_log_recent_scope")));
            lines.push(field(
                "tui.core_log_finished",
                s.core_diagnostics
                    .as_ref()
                    .map(|d| boolean(d.finished))
                    .unwrap_or_else(unknown),
            ));
            lines.push(Line::from(tr("tui.core_log_finished_scope")));
            lines.push(Line::from(tr("tui.health")));
            lines.push(Line::from(tr("tui.no_killswitch")));
            lines.push(Line::from(tr("tui.diagnostic_drilldown")));
            lines
        }
        Page::Host => {
            let Some(host) = &s.host_support else {
                return vec![Line::from(tr("tui.metric_unavailable"))];
            };
            let state = |value: Option<bool>| value.map_or_else(unknown, boolean);
            vec![
                Line::from(tr("tui.host_scope")),
                Line::from(""),
                field("tui.host_core_installed", state(host.core_installed)),
                field("tui.host_core_capabilities", state(host.core_capabilities)),
                field("tui.host_tun_device", state(host.tun_device)),
                Line::from(""),
                field("tui.host_runtime_loaded", state(host.runtime_unit_loaded)),
                field("tui.host_runtime_active", state(host.runtime_unit_active)),
                field("tui.host_runtime_enabled", state(host.runtime_unit_enabled)),
                Line::from(""),
                field("tui.host_store", state(host.store_present)),
                field("tui.host_template", state(host.template_present)),
                field("tui.host_config", state(host.generated_config_present)),
                field(
                    "tui.host_runtime_unit",
                    state(host.package_runtime_unit_present),
                ),
                field(
                    "tui.host_login_unit",
                    state(host.package_login_unit_present),
                ),
                Line::from(""),
                Line::from(tr("tui.host_limit")),
            ]
        }
        Page::Rules => {
            let Some(rules) = &s.rules else {
                return vec![Line::from(tr("tui.metric_unavailable"))];
            };
            let query = app.operator_query.to_lowercase();
            let matches: Vec<_> = rules
                .items
                .iter()
                .filter(|row| {
                    query.is_empty()
                        || row.kind.to_lowercase().contains(&query)
                        || row.payload.to_lowercase().contains(&query)
                        || row.target.to_lowercase().contains(&query)
                })
                .collect();
            let mut lines = vec![
                Line::from(tr("tui.rules_scope")),
                field("tui.loaded_total", rules.total.to_string()),
                field("tui.loaded_projection", rules.items.len().to_string()),
                field("tui.matching_rows", matches.len().to_string()),
            ];
            if rules.truncated {
                lines.push(Line::from(tr("tui.backend_truncated")));
            }
            if !app.operator_query.is_empty() || app.searching {
                lines.push(field(
                    "tui.operator_filter",
                    display(&app.operator_query, 80),
                ));
            }
            lines.push(Line::from(""));
            if matches.is_empty() {
                lines.push(Line::from(tr(if rules.total == 0 {
                    "tui.no_loaded_rules"
                } else {
                    "tui.no_operator_matches"
                })));
            }
            const DISPLAY_LIMIT: usize = 120;
            for row in matches.iter().take(DISPLAY_LIMIT) {
                lines.push(Line::from(format!(
                    "{} [{}] {}",
                    display(&row.kind, 80),
                    row.target,
                    display(&row.payload, 512)
                )));
            }
            if matches.len() > DISPLAY_LIMIT {
                lines.push(Line::from(tr("tui.refine_rule_filter")));
            }
            lines
        }
        Page::Providers => {
            let Some(providers) = &s.providers else {
                return vec![Line::from(tr("tui.metric_unavailable"))];
            };
            let query = app.operator_query.to_lowercase();
            let matches: Vec<_> = providers
                .items
                .iter()
                .filter(|row| {
                    query.is_empty()
                        || row.name.to_lowercase().contains(&query)
                        || row.behavior.to_lowercase().contains(&query)
                        || row.status.contains(&query)
                })
                .collect();
            let mut lines = vec![
                Line::from(tr("tui.providers_scope")),
                field("tui.loaded_total", providers.total.to_string()),
                field("tui.loaded_projection", providers.items.len().to_string()),
                field("tui.matching_rows", matches.len().to_string()),
            ];
            if providers.truncated {
                lines.push(Line::from(tr("tui.backend_truncated")));
            }
            if !app.operator_query.is_empty() || app.searching {
                lines.push(field(
                    "tui.operator_filter",
                    display(&app.operator_query, 80),
                ));
            }
            lines.push(Line::from(""));
            if matches.is_empty() {
                lines.push(Line::from(tr(if providers.total == 0 {
                    "tui.no_loaded_providers"
                } else {
                    "tui.no_operator_matches"
                })));
            }
            const DISPLAY_LIMIT: usize = 60;
            for row in matches.iter().take(DISPLAY_LIMIT) {
                let count = row.rule_count.map_or_else(unknown, |n| n.to_string());
                lines.push(
                    Line::from(display(&row.name, 160))
                        .style(Style::default().add_modifier(Modifier::BOLD)),
                );
                lines.push(Line::from(format!(
                    "  {} · {} · {}: {count}",
                    display(&row.behavior, 80),
                    tr(match row.status {
                        "loaded" => "tui.provider_loaded",
                        "empty" => "tui.provider_empty",
                        _ => "tui.provider_unknown",
                    }),
                    tr("tui.provider_rules")
                )));
                lines.push(field(
                    "tui.provider_updated",
                    if row.updated_at.is_empty() {
                        unknown()
                    } else {
                        display(&row.updated_at, 80)
                    },
                ));
                lines.push(field("tui.provider_refreshable", boolean(row.refreshable)));
                lines.push(Line::from(""));
            }
            if matches.len() > DISPLAY_LIMIT {
                lines.push(Line::from(tr("tui.refine_provider_filter")));
            }
            lines
        }
        Page::CustomRules => {
            let Some(custom) = &s.custom_rules else {
                return vec![Line::from(tr("tui.metric_unavailable"))];
            };
            let query = app.operator_query.to_lowercase();
            let matches: Vec<_> = custom
                .items
                .iter()
                .filter(|row| {
                    query.is_empty()
                        || row.value.to_lowercase().contains(&query)
                        || row.kind.contains(&query)
                        || row.action.to_lowercase().contains(&query)
                })
                .collect();
            let mut lines = vec![
                Line::from(tr("tui.custom_rules_scope")),
                field("tui.configured_total", custom.items.len().to_string()),
                field("tui.matching_rows", matches.len().to_string()),
            ];
            if !app.operator_query.is_empty() || app.searching {
                lines.push(field(
                    "tui.operator_filter",
                    display(&app.operator_query, 80),
                ));
            }
            lines.push(Line::from(""));
            if matches.is_empty() {
                lines.push(Line::from(tr(if custom.items.is_empty() {
                    "tui.no_custom_rules"
                } else {
                    "tui.no_operator_matches"
                })));
            }
            const DISPLAY_LIMIT: usize = 80;
            for row in matches.iter().take(DISPLAY_LIMIT) {
                lines.push(Line::from(format!(
                    "{} → {} · {}",
                    row.kind, row.action, row.value
                )));
            }
            if matches.len() > DISPLAY_LIMIT {
                lines.push(Line::from(tr("tui.refine_custom_filter")));
            }
            lines
        }
        Page::Subscriptions => {
            let mut lines = vec![Line::from(tr(if s.capabilities.subscription_usage {
                "tui.subscription_usage_scope"
            } else {
                "tui.subscription_scope"
            }))];
            if s.metadata.subscriptions.is_empty() {
                lines.push(Line::from(tr("tui.no_subscriptions")));
            }
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|d| u64::try_from(d.as_millis()).ok());
            for sub in &s.metadata.subscriptions {
                let profiles: Vec<_> = s
                    .metadata
                    .profiles
                    .iter()
                    .filter(|p| p.subscription_id.as_ref() == Some(&sub.id))
                    .collect();
                let missing = profiles.iter().filter(|p| p.missing).count();
                let (key, count) =
                    crate::inspection::saved_age(sub.updated_at, now_ms.unwrap_or(0));
                let age = count.map_or_else(
                    || tr(key).to_owned(),
                    |n| {
                        // Only this bounded numeric slot is interpolated; private
                        // provider text never becomes a format string.
                        tr(key).replace("{count}", &n.to_string())
                    },
                );
                lines.push(
                    Line::from(format!(
                        "{}{}",
                        if Some(&sub.id) == app.selected_subscription.as_ref() {
                            "> "
                        } else {
                            ""
                        },
                        display(&sub.name, 80)
                    ))
                    .style(Style::default().add_modifier(Modifier::BOLD)),
                );
                lines.push(Line::from(format!(
                    "  {}: {} · {}: {missing}",
                    tr("tui.saved_profiles"),
                    profiles.len(),
                    tr("tui.feed_missing_count")
                )));
                lines.push(Line::from(format!("  {}: {age}", tr("tui.saved_update"))));
                if let Some(attempt) = app.subscription_attempts.get(&sub.id) {
                    lines.push(Line::from(format!(
                        "  {}: {}",
                        tr("tui.session_attempt"),
                        tr(attempt)
                    )));
                }
                if Some(&sub.id) == app.selected_subscription.as_ref() {
                    use crate::subscription_usage::Status;
                    match &app.usage_result {
                        Some(Status::Reported(usage)) => {
                            lines.push(Line::from(format!("  {}", tr("tui.usage_reported"))));
                            for (key, value) in [
                                ("tui.upload_history", usage.upload),
                                ("tui.download_history", usage.download),
                                ("tui.usage_total", usage.total),
                            ] {
                                lines.push(Line::from(format!(
                                    "  {}: {}",
                                    tr(key),
                                    crate::inspection::bytes(value)
                                )));
                            }
                            if let Some(remaining) = usage.remaining() {
                                lines.push(Line::from(format!(
                                    "  {}: {}",
                                    tr("tui.usage_remaining"),
                                    crate::inspection::bytes(remaining)
                                )));
                            }
                            lines.push(Line::from(format!(
                                "  {}: {}",
                                tr("tui.usage_expiry"),
                                usage
                                    .expiry_utc
                                    .as_deref()
                                    .unwrap_or(tr("tui.metric_unavailable"))
                            )));
                            lines.push(Line::from(tr("tui.usage_private_notice")));
                        }
                        Some(Status::NotProvided) => {
                            lines.push(Line::from(tr("tui.usage_not_provided")))
                        }
                        Some(Status::Unavailable) => {
                            lines.push(Line::from(tr("tui.usage_unavailable")))
                        }
                        Some(Status::Unsupported) => {
                            lines.push(Line::from(tr("tui.usage_unsupported")))
                        }
                        Some(Status::Loading) => lines.push(Line::from(tr("tui.usage_loading"))),
                        None if app.usage_request.is_some() => {
                            lines.push(Line::from(tr("tui.usage_loading")))
                        }
                        None => {}
                    }
                }
                lines.push(Line::from(""));
            }
            lines
        }
        Page::Profiles | Page::Activity | Page::Settings | Page::Jobs => Vec::new(),
    }
}

fn page_keys_key(_app: &App) -> &'static str {
    #[cfg(feature = "private-backup")]
    if _app.backup_enabled {
        return "tui.page_backup_keys";
    }
    "tui.page_keys"
}

fn settings_scope_key(_app: &App) -> &'static str {
    #[cfg(feature = "private-backup")]
    if _app.restore_enabled {
        return "tui.settings_private_restore";
    }
    #[cfg(feature = "private-backup")]
    if _app.backup_enabled {
        return "tui.settings_private_backup";
    }
    "tui.settings_local"
}

fn settings_keys_key(_app: &App) -> &'static str {
    #[cfg(feature = "private-backup")]
    if _app.restore_enabled {
        return "tui.settings_restore_keys";
    }
    "tui.settings_keys"
}

#[cfg(feature = "private-backup")]
fn draw_private_restore(frame: &mut Frame, app: &App, restore: &crate::private_restore::Workspace) {
    use crate::private_restore::{Field, State};
    let tr = |key| app.locale.text(key);
    let area = frame.area();
    let mut lines = vec![
        Line::from(tr("tui.restore_scope")),
        Line::from(tr("tui.restore_private")),
        Line::from(""),
    ];
    let field = |key, value: &str, selected| {
        Line::from(format!(
            "{}{}: {}",
            if restore.state() == State::Editing && restore.field() == selected {
                "> "
            } else {
                "  "
            },
            tr(key),
            value
        ))
    };
    lines.push(field(
        "tui.restore_archive",
        restore.archive(),
        Field::Archive,
    ));
    match restore.state() {
        State::Editing => {
            lines.push(field(
                "tui.backup_passphrase",
                restore.masked(),
                Field::Passphrase,
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.restore_edit_keys")));
        }
        State::Confirming => {
            if let Some((profiles, subscriptions)) = restore.counts() {
                lines.push(Line::from(
                    tr("tui.restore_counts")
                        .replace("{profiles}", &profiles.to_string())
                        .replace("{subscriptions}", &subscriptions.to_string()),
                ));
            }
            lines.push(Line::from(tr("tui.restore_preview_data")));
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.restore_replace")));
            lines.push(Line::from(tr("tui.restore_off")));
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.restore_confirm")));
        }
        state => {
            lines.push(Line::from(""));
            lines.push(Line::from(tr(match state {
                State::Previewing => "tui.restore_previewing",
                State::Submitted => "tui.restore_submitted",
                State::Completed => "tui.restore_completed",
                State::Denied => "tui.restore_denied",
                State::Unknown => "tui.restore_unknown",
                State::PreviewUnavailable => "tui.restore_preview_unavailable",
                State::Cancelled => "tui.restore_cancelled",
                _ => "tui.restore_changed",
            })));
        }
    }
    if !app.notice.is_empty() {
        lines.push(Line::from(tr(app.notice)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(tr("tui.restore_return")));
    let content = if area.width >= 70 && area.height >= 24 {
        lines
    } else {
        vec![
            Line::from(tr("tui.action_resize")),
            Line::from(tr("tui.restore_return")),
        ]
    };
    frame.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(tr("tui.restore_title")),
            )
            .style(app.palette.normal()),
        area,
    );
}

#[cfg(feature = "private-backup")]
fn draw_private_backup(frame: &mut Frame, app: &App, backup: &crate::private_backup::Workspace) {
    use crate::private_backup::{Field, State};
    let tr = |key| app.locale.text(key);
    let area = frame.area();
    let mut lines = vec![
        Line::from(tr("tui.backup_scope")),
        Line::from(tr("tui.backup_private")),
        Line::from(tr("tui.backup_loss")),
        Line::from(""),
    ];
    let field = |name, value: &str, selected: Field| {
        Line::from(format!(
            "{}{}: {}",
            if backup.state() == State::Editing && backup.field() == selected {
                "> "
            } else {
                "  "
            },
            tr(name),
            value
        ))
    };
    lines.push(field(
        "tui.backup_destination",
        backup.destination(),
        Field::Destination,
    ));
    match backup.state() {
        State::Editing => {
            lines.push(field(
                "tui.backup_passphrase",
                backup.masked(Field::Passphrase),
                Field::Passphrase,
            ));
            lines.push(field(
                "tui.backup_repeat",
                backup.masked(Field::Repeat),
                Field::Repeat,
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.backup_edit_keys")));
        }
        State::Confirming => {
            lines.push(Line::from(tr("tui.backup_matched")));
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.backup_confirm")));
        }
        state => {
            lines.push(Line::from(tr(match state {
                State::Submitted => "tui.backup_submitted",
                State::Completed => "tui.backup_completed",
                State::Denied => "tui.backup_denied",
                State::Unknown => "tui.backup_unknown",
                _ => "tui.backup_changed",
            })));
            lines.push(Line::from(""));
            lines.push(Line::from(tr("tui.backup_return")));
        }
    }
    if !app.notice.is_empty() {
        lines.push(Line::from(tr(app.notice)));
    }
    let content = if area.width >= 70 && area.height >= 24 {
        lines
    } else {
        vec![
            Line::from(tr("tui.action_resize")),
            Line::from(tr("tui.backup_return")),
        ]
    };
    frame.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(tr("tui.backup_title")),
            )
            .style(app.palette.normal()),
        area,
    );
}
