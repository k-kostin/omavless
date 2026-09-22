// SPDX-License-Identifier: MIT
//! Opt-in read-only TUI; no store, service, core, network or mutation ownership.
pub mod app;
pub mod client;
pub mod i18n;
pub mod model;
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
    mut read: impl FnMut(Read) -> Result<Value, ReadError> + Send + 'static,
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
    let (request, requests) = mpsc::sync_channel::<()>(1);
    let (results, receive) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("tui-read".into())
        .spawn(move || {
            while requests.recv().is_ok() {
                let started = Instant::now();
                if results.send((started, client::load(&mut read))).is_err() {
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
    let mut due = Instant::now();
    let mut pending = false;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if let Ok((started, result)) = receive.try_recv() {
            app.accept(result, started);
            pending = false;
            due = now + Duration::from_secs(3);
        }
        if !pending && now >= due && request.try_send(()).is_ok() {
            pending = true;
        }
        terminal
            .draw(|f| view::draw(f, &app, now))
            .map_err(|_| "Could not draw OmaVLESS terminal")?;
        match keys.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(Event::Key(key))) => match app.key(key) {
                Action::Close => break,
                Action::Refresh => due = Instant::now(),
                Action::None => {}
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
