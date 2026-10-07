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
#[cfg(feature = "private-backup")]
pub mod private_backup;
pub mod route_inspection;
pub mod settings;
pub mod subscription_usage;
pub mod theme;
pub mod traffic_history;
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

#[cfg(feature = "private-backup")]
type BackupAdapter = Box<dyn FnMut(private_backup::Request) -> private_backup::Completion + Send>;
#[cfg(not(feature = "private-backup"))]
type BackupAdapter = ();

/// Ratatui 0.30's Terminal destructor uses eprintln! if cursor restoration
/// fails. After a PTY is revoked both that write and the destructor can panic.
/// Contain only teardown here, not rendering/IPC failures; retain normal drops
/// and the privacy-safe panic hook, without leaking the terminal with forget().
struct TerminalGuard(Option<ratatui::DefaultTerminal>);

fn drop_terminal_safely<T>(value: T) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value)));
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if let Some(terminal) = self.0.take() {
            drop_terminal_safely(terminal);
        }
    }
}

impl std::ops::Deref for TerminalGuard {
    type Target = ratatui::DefaultTerminal;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("terminal owned until teardown")
    }
}

impl std::ops::DerefMut for TerminalGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("terminal owned until teardown")
    }
}

/// Never reports rejected IPC data, private names, terminal escapes or paths.
pub fn run(
    read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
) -> Result<(), &'static str> {
    run_client(
        read,
        None::<fn(&actions::Request) -> Result<Value, ReadError>>,
        None::<fn(&jobs::Call) -> Result<Value, ReadError>>,
        None,
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
        None,
    )
}

/// All three adapters contact the same owner; jobs use fixed unary calls.
pub fn run_full(
    read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
    mutate: impl FnMut(&actions::Request) -> Result<Value, ReadError> + Send + 'static,
    jobs: impl FnMut(&jobs::Call) -> Result<Value, ReadError> + Send + 'static,
) -> Result<(), &'static str> {
    run_client(read, Some(mutate), Some(jobs), None)
}

/// Explicit opt-in client only. Backup callback has one fixed semantic method;
/// the runtime remains sole source/destination/lease/admission owner.
#[cfg(feature = "private-backup")]
pub fn run_full_private_backup(
    read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
    mutate: impl FnMut(&actions::Request) -> Result<Value, ReadError> + Send + 'static,
    jobs: impl FnMut(&jobs::Call) -> Result<Value, ReadError> + Send + 'static,
    backup: impl FnMut(private_backup::Request) -> private_backup::Completion + Send + 'static,
) -> Result<(), &'static str> {
    run_client(read, Some(mutate), Some(jobs), Some(Box::new(backup)))
}

fn run_client(
    mut read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
    mutate: Option<impl FnMut(&actions::Request) -> Result<Value, ReadError> + Send + 'static>,
    jobs: Option<impl FnMut(&jobs::Call) -> Result<Value, ReadError> + Send + 'static>,
    _backup: Option<BackupAdapter>,
) -> Result<(), &'static str> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err("OmaVLESS TUI requires an interactive terminal");
    }
    let mut terminal = TerminalGuard(Some(
        ratatui::try_init().map_err(|_| "Could not initialize OmaVLESS terminal")?,
    ));
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
    let (request, requests) = mpsc::sync_channel::<(
        inspection::Page,
        Option<String>,
        Option<route_inspection::Request>,
        Option<subscription_usage::Request>,
    )>(1);
    let (results, receive) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("tui-read".into())
        .spawn(move || {
            while let Ok((page, selected, route, usage)) = requests.recv() {
                let started = Instant::now();
                let result = client::load_page_for_route(
                    &mut read,
                    page,
                    selected.as_deref(),
                    route.as_ref(),
                )
                .and_then(|(snapshot, route_status)| {
                    subscription_usage::load(&mut read, page, snapshot, usage.as_ref())
                        .map(|(snapshot, usage_status)| (snapshot, route_status, usage_status))
                });
                if results
                    .send((started, page, selected.clone(), route, usage, result))
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
    #[cfg(feature = "private-backup")]
    let (backup_calls, backup_requests) = mpsc::sync_channel::<private_backup::Request>(1);
    #[cfg(feature = "private-backup")]
    let (backup_results, backup_responses) = mpsc::sync_channel(1);
    #[cfg(feature = "private-backup")]
    if let Some(mut backup) = _backup {
        app.backup_enabled = true;
        thread::Builder::new()
            .name("tui-private-backup".into())
            .spawn(move || {
                while let Ok(request) = backup_requests.recv() {
                    if backup_results.send(backup(request)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| "Could not start private Backup client")?;
    }
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
        #[cfg(feature = "private-backup")]
        {
            if let Ok(result) = backup_responses.try_recv() {
                app.finish_backup(result);
            }
            app.refresh_backup_context(now);
        }
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
        if let Ok((started, page, selected, route, usage, result)) = receive.try_recv() {
            match result {
                Ok((snapshot, route_status, usage_status)) => {
                    if app.accept_for(page, selected.as_deref(), Ok(snapshot), started) {
                        app.accept_route(
                            route.as_ref().map(route_inspection::Request::target),
                            route_status,
                            started,
                        );
                        app.accept_usage(usage.as_ref(), usage_status, started);
                    }
                }
                Err(error) => {
                    app.accept_for(page, selected.as_deref(), Err(error), started);
                }
            }
            pending = false;
            due = if app.route_request.is_some() || app.usage_request.is_some() {
                now
            } else if page == app.page && selected == app.selected {
                now + Duration::from_secs(3)
            } else {
                now
            };
        }
        if !pending
            && now >= due
            && request
                .try_send((
                    app.page,
                    app.selected.clone(),
                    app.route_request.clone(),
                    app.usage_request.clone(),
                ))
                .is_ok()
        {
            app.route_request = None;
            app.usage_request = None;
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
                #[cfg(feature = "private-backup")]
                Action::SubmitBackup => {
                    if let Some(request) = app.take_backup_request()
                        && let Err(error) = backup_calls.try_send(request)
                    {
                        // The original is returned, not copied/retried under a new ID.
                        let (mpsc::TrySendError::Full(request)
                        | mpsc::TrySendError::Disconnected(request)) = error;
                        app.finish_backup(request.settle(Err(ReadError::Unavailable)));
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

#[cfg(test)]
mod terminal_drop_tests {
    use super::drop_terminal_safely;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn teardown_panic_is_contained_without_skipping_drop() {
        struct PanickingDrop(Arc<AtomicUsize>);
        impl Drop for PanickingDrop {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
                panic!("synthetic revoked terminal");
            }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        drop_terminal_safely(PanickingDrop(drops.clone()));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
