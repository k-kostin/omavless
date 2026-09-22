// SPDX-License-Identifier: MIT
use omavless_tui::{
    app::App,
    client,
    i18n::Locale,
    theme::{self, Palette},
    view,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use std::{
    fs,
    os::unix::fs::symlink,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
mod support;
const DARK: &str = "background = \"#101020\"\nforeground = \"#ffffff\"\naccent = \"#00ffff\"\nselection = \"#123456\"\n";
const LIGHT: &str = "background = \"#ffffff\"\nforeground = \"#101020\"\naccent = \"#0011ff\"\nselection = \"#bbbbbb\"\n";
#[test]
fn complete_dark_light_rgb_palettes_and_atomic_fallback() {
    assert_eq!(
        Palette::parse(DARK).unwrap().background,
        Color::Rgb(16, 16, 32)
    );
    assert_eq!(
        Palette::parse(LIGHT).unwrap().background,
        Color::Rgb(255, 255, 255)
    );
    for value in [
        "".into(),
        DARK.replace("#101020", "red"),
        DARK.replace("#ffffff", "#101020"),
        DARK.replace("#123456", "#ffffff"),
        DARK.replace("accent", "unknown"),
        format!("{DARK}accent = \"#abcdef\"\n"),
        format!("{DARK}\u{1b}"),
        " ".repeat(16 * 1024 + 1),
    ] {
        assert!(Palette::parse(&value).is_none());
    }
    assert!(Palette::parse(&format!("# safe comment\nmode = \"dark\"\n{DARK}")).is_some());
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "omavless-tui-theme-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::create_dir_all(dir.join(".local/state/omarchy/current")).unwrap();
        Self(dir)
    }
    fn target(&self) -> PathBuf {
        self.0.join(".local/state/omarchy/current/theme")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn directory_symlink_replacement_is_reloaded_without_host_config_changes() {
    let f = Fixture::new();
    assert_eq!(theme::load(&f.0), Palette::default());
    for (dir, palette) in [("dark", DARK), ("light", LIGHT)] {
        fs::create_dir(f.0.join(dir)).unwrap();
        fs::write(f.0.join(dir).join("colors.toml"), palette).unwrap();
    }
    symlink(f.0.join("dark"), f.target()).unwrap();
    assert_eq!(theme::load(&f.0), Palette::parse(DARK).unwrap());
    let replacement = f.0.join(".local/state/omarchy/current/next");
    symlink(f.0.join("light"), &replacement).unwrap();
    fs::rename(replacement, f.target()).unwrap();
    assert_eq!(theme::load(&f.0), Palette::parse(LIGHT).unwrap());
    fs::write(f.target().join("colors.toml"), "partial").unwrap();
    assert_eq!(theme::load(&f.0), Palette::default());
}
#[test]
fn unsafe_or_oversized_palette_is_not_read_or_evaluated() {
    let f = Fixture::new();
    fs::create_dir(f.target()).unwrap();
    let file = f.target().join("colors.toml");
    fs::write(&file, vec![b'a'; 16 * 1024 + 1]).unwrap();
    assert_eq!(theme::load(&f.0), Palette::default());
    fs::write(&file, [255]).unwrap();
    assert_eq!(theme::load(&f.0), Palette::default());
    fs::remove_file(&file).unwrap();
    symlink("/dev/zero", &file).unwrap();
    assert_eq!(theme::load(&f.0), Palette::default());
    fs::remove_file(&file).unwrap();
    nix::unistd::mkfifo(&file, nix::sys::stat::Mode::S_IRUSR).unwrap();
    let start = Instant::now();
    assert_eq!(theme::load(&f.0), Palette::default());
    assert!(start.elapsed().as_secs() < 1);
    assert_eq!(
        theme::load(std::path::Path::new("relative")),
        Palette::default()
    );
}
#[test]
fn reload_changes_presentation_not_selection_search_or_runtime_state() {
    let now = Instant::now();
    let mut a = App::new(Locale::En);
    a.accept(client::load(&mut |r| Ok(support::response(r))), now);
    a.selected = Some("fixture-a".into());
    a.query = "Fixture".into();
    a.inspection_scroll = 2;
    for palette in [
        Palette::parse(DARK).unwrap(),
        Palette::parse(LIGHT).unwrap(),
        Palette::default(),
    ] {
        a.palette = palette;
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| view::draw(f, &a, now)).unwrap();
        assert!(
            t.backend()
                .buffer()
                .content
                .iter()
                .any(|c| c.bg == palette.selection)
        );
        assert_eq!(a.selected.as_deref(), Some("fixture-a"));
        assert_eq!(a.query, "Fixture");
        assert_eq!(a.inspection_scroll, 2);
        assert_eq!(a.snapshot.as_ref().unwrap().revision, 7);
        assert!(a.pending.is_none());
    }
}
