// SPDX-License-Identifier: MIT
//! Opt-in TUI client; no store, service, core or network ownership.
pub mod actions;
pub mod activity;
pub mod app;
pub mod browsing;
pub mod client;
pub mod i18n;
pub mod inspection;
pub mod job_ui;
pub mod jobs;
pub mod model;
pub mod settings;
pub mod theme;
pub mod view;

use app::{Action, App};
use client::Read;
use model::ReadError;
use ratatui::crossterm::event::{self, Event};
use serde_json::Value;
use std::{
    io::{IsTerminal, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Never reports rejected IPC data, private names, terminal escapes or paths.
pub fn run(
    read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
) -> Result<(), &'static str> {
    run_client(
        read,
        None::<fn(&actions::Request) -> Result<Value, ReadError>>,
        None::<fn(&jobs::Call) -> Result<Value, ReadError>>,
    )
}

/// The adapter must use the existing instance/revision-fenced plugin.action
/// transport with its 120-second deadline. This client never starts a runtime.
pub fn run_actions(
    read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
    mutate: impl FnMut(&actions::Request) -> Result<Value, ReadError> + Send + 'static,
) -> Result<(), &'static str> {
    run_client(
        read,
        Some(mutate),
        None::<fn(&jobs::Call) -> Result<Value, ReadError>>,
    )
}

/// All three adapters contact the same owner; jobs use fixed unary calls.
pub fn run_full(
    read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
    mutate: impl FnMut(&actions::Request) -> Result<Value, ReadError> + Send + 'static,
    jobs: impl FnMut(&jobs::Call) -> Result<Value, ReadError> + Send + 'static,
) -> Result<(), &'static str> {
    run_client(read, Some(mutate), Some(jobs))
}

fn run_client(
    mut read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
    mutate: Option<impl FnMut(&actions::Request) -> Result<Value, ReadError> + Send + 'static>,
    jobs: Option<impl FnMut(&jobs::Call) -> Result<Value, ReadError> + Send + 'static>,
) -> Result<(), &'static str> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err("OmaVLESS TUI requires an interactive terminal");
    }
    let mut terminal = ratatui::try_init().map_err(|_| "Could not initialize OmaVLESS terminal")?;
    // This entry point owns the CLI process. Never echo panic payloads/private
    // state or panic recursively while reporting to a revoked terminal.
    std::panic::set_hook(Box::new(|_| {
        let _ = ratatui::try_restore();
        let _ = writeln!(
            std::io::stderr(),
            "OmaVLESS terminal client stopped unexpectedly"
        );
    }));
    struct Restore(Vec<signal_hook::SigId>);
    impl Drop for Restore {
        fn drop(&mut self) {
            // restore() prints on failure and can panic when stderr is the
            // already-closed PTY, including a recursive panic in its own hook.
            let _ = ratatui::try_restore();
            for id in self.0.drain(..) {
                signal_hook::low_level::unregister(id);
            }
        }
    }
    let mut guard = Restore(Vec::new());
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        guard.0.push(
            signal_hook::flag::register(signal, stop.clone())
                .map_err(|_| "Could not initialize terminal signal handling")?,
        );
    }
    let (request, requests) = mpsc::sync_channel::<(inspection::Page, Option<String>)>(1);
    let (results, receive) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("tui-read".into())
        .spawn(move || {
            while let Ok((page, selected)) = requests.recv() {
                let started = Instant::now();
                if results
                    .send((
                        started,
                        page,
                        selected.clone(),
                        client::load_page_for(&mut read, page, selected.as_deref()),
                    ))
                    .is_err()
                {
                    break;
                }
            }
        })
        .map_err(|_| "Could not start read-only terminal client")?;
    // Crossterm's event reader can remain inside a platform poll after terminal
    // teardown. Never let that reader own the UI/signal/exit loop.
    let (input, keys) = mpsc::sync_channel(16);
    thread::Builder::new()
        .name("tui-input".into())
        .spawn(move || {
            loop {
                let event = event::read().map_err(|_| "Could not read terminal input");
                let failed = event.is_err();
                if input.send(event).is_err() || failed {
                    break;
                }
            }
        })
        .map_err(|_| "Could not start terminal input")?;
    let mut app = App::new(i18n::Locale::current());
    // Theme sampling is independent of IPC (which can block). A capacity-one
    // channel coalesces filesystem changes; no watcher/path/log surface added.
    let (palettes, palette_updates) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("tui-theme".into())
        .spawn(move || {
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
            loop {
                let palette = home
                    .as_ref()
                    .map_or_else(theme::Palette::default, |p| theme::load(p));
                match palettes.try_send(palette) {
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                    _ => thread::sleep(Duration::from_secs(2)),
                }
            }
        })
        .map_err(|_| "Could not start terminal theme reader")?;
    app.actions_enabled = mutate.is_some();
    app.jobs_enabled = jobs.is_some();
    let (job_calls, job_requests) = mpsc::sync_channel::<(job_ui::Phase, jobs::Call)>(1);
    let (job_results, job_responses) = mpsc::sync_channel(1);
    if let Some(mut call) = jobs {
        thread::Builder::new()
            .name("tui-job".into())
            .spawn(move || {
                while let Ok((phase, request)) = job_requests.recv() {
                    if job_results.send((phase, call(&request))).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| "Could not start terminal job client")?;
    }
    let (submit, submissions) = mpsc::sync_channel::<actions::Request>(1);
    let (finished, finishes) = mpsc::sync_channel(1);
    if let Some(mut mutate) = mutate {
        thread::Builder::new()
            .name("tui-action".into())
            .spawn(move || {
                while let Ok(request) = submissions.recv() {
                    let outcome = request.outcome(mutate(&request));
                    if finished.send(outcome).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| "Could not start terminal action client")?;
    }
    let mut due = Instant::now();
    let mut pending = false;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if let Ok((phase, value)) = job_responses.try_recv() {
            if let Some(job) = &mut app.job {
                job.accept(phase, value);
            }
            due = now;
        }
        if let Some(job) = &mut app.job
            && let Some(phase) = job.next(now)
            && let Some(call) = job.call(phase)
            && job_calls.try_send((phase, call)).is_err()
        {
            job.accept(phase, Err(ReadError::Unavailable));
        }
        if let Ok(palette) = palette_updates.try_recv() {
            app.update_palette(palette);
        }
        if let Ok(outcome) = finishes.try_recv() {
            app.finish(outcome, now);
            due = now;
        }
        if let Ok((started, page, selected, result)) = receive.try_recv() {
            app.accept(result, started);
            pending = false;
            due = if page == app.page && selected == app.selected {
                now + Duration::from_secs(3)
            } else {
                now
            };
        }
        if !pending && now >= due && request.try_send((app.page, app.selected.clone())).is_ok() {
            pending = true;
        }
        terminal
            .draw(|f| {
                app.viewport_ready = f.area().width >= 70 && f.area().height >= 24;
                view::clamp_scroll(&mut app, f.area().width, f.area().height, now);
                view::draw(f, &app, now);
            })
            .map_err(|_| "Could not draw OmaVLESS terminal")?;
        match keys.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(Event::Key(key))) => match app.key_at(key, now) {
                Action::Close => break,
                Action::Refresh => due = Instant::now(),
                Action::None => {}
                action @ (Action::StartJob | Action::PollJob | Action::CancelJob) => {
                    let phase = match action {
                        Action::StartJob => job_ui::Phase::Start,
                        Action::CancelJob => job_ui::Phase::Cancel,
                        _ => job_ui::Phase::Poll,
                    };
                    if let Some(job) = &mut app.job
                        && (action != Action::PollJob || job.tracker.manual_poll_due(now))
                        && let Some(call) = job.call(phase)
                        && job_calls.try_send((phase, call)).is_err()
                    {
                        job.accept(phase, Err(ReadError::Unavailable));
                    }
                }
                Action::Submit => {
                    if let Some(request) = app.pending.clone()
                        && submit.try_send(request).is_err()
                    {
                        app.finish(actions::Outcome::Unknown, now);
                    }
                }
            },
            Ok(Ok(_)) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(Err(message)) => return Err(message),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("Terminal input stopped"),
        }
    }
    // Deliberately no join on a slow read: terminal close must remain immediate.
    // Dropping channels stops workers after their current read. The CLI process
    // exits without joining a blocked terminal/backend read thread.
    Ok(())
}
