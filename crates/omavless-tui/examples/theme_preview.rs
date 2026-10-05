// SPDX-License-Identifier: MIT
//! Synthetic presentation only. No runtime, private store or host theme writes.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{app::App, client, i18n::Locale, theme::Palette, view};
use ratatui::crossterm::event::{self, Event, KeyCode};
use std::time::Instant;
fn main() {
    let light = std::env::args().nth(1).as_deref() == Some("light");
    let mut app = App::new(Locale::current());
    app.accept(
        client::load(&mut |r| Ok(support::response(r))),
        Instant::now(),
    );
    app.selected = Some("fixture-a".into());
    app.palette = if light {
        Palette::parse("foreground=\"#202020\"\nbackground=\"#ffffff\"\naccent=\"#0000aa\"\nselection=\"#b0c4de\"").unwrap()
    } else {
        Palette::default()
    };
    let mut t = ratatui::init();
    loop {
        let now = Instant::now();
        app.sampled_at = Some(now);
        t.draw(|f| view::draw(f, &app, now)).unwrap();
        if matches!(event::read().unwrap(),Event::Key(k) if k.code==KeyCode::Char('q')) {
            break;
        }
    }
    ratatui::restore();
}
