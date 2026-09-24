// SPDX-License-Identifier: MIT
//! Optional Omarchy palette, never shell/config evaluation or runtime policy.
use ratatui::style::{Color, Style};
use std::{
    collections::BTreeMap, fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub foreground: Color,
    pub background: Color,
    pub accent: Color,
    pub selection: Color,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            foreground: Color::White,
            background: Color::Black,
            accent: Color::Cyan,
            selection: Color::DarkGray,
        }
    }
}
impl Palette {
    pub fn normal(self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }
    pub fn selected(self) -> Style {
        Style::default().fg(self.foreground).bg(self.selection)
    }

    /// Only the four fixed RGB values are consumed. This is not a general TOML
    /// evaluator; unsupported syntax fails to a complete palette, not a mix.
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() > 16 * 1024
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return None;
        }
        let mut values = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            if !["foreground", "background", "accent", "selection"].contains(&key) {
                continue;
            }
            let value = value.trim();
            let color = value
                .strip_prefix('"')?
                .strip_suffix('"')?
                .strip_prefix('#')?;
            if color.len() != 6
                || !color.bytes().all(|b| b.is_ascii_hexdigit())
                || values.contains_key(key)
            {
                return None;
            }
            values.insert(
                key,
                Color::Rgb(
                    u8::from_str_radix(&color[..2], 16).ok()?,
                    u8::from_str_radix(&color[2..4], 16).ok()?,
                    u8::from_str_radix(&color[4..], 16).ok()?,
                ),
            );
        }
        let p = Self {
            foreground: *values.get("foreground")?,
            background: *values.get("background")?,
            accent: *values.get("accent")?,
            selection: *values.get("selection")?,
        };
        if p.foreground == p.background || p.foreground == p.selection {
            return None;
        }
        Some(p)
    }
}

pub fn load(home: &Path) -> Palette {
    fn read(home: &Path) -> Option<Palette> {
        if !home.is_absolute() {
            return None;
        }
        // Follow Omarchy's replaceable theme-directory link; refuse symlink or
        // special-file palette targets. Nonblocking open prevents FIFO hangs.
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NONBLOCK | nix::libc::O_NOFOLLOW)
            .open(home.join(".local/state/omarchy/current/theme/colors.toml"))
            .ok()?;
        let meta = file.metadata().ok()?;
        if !meta.is_file() || meta.len() > 16 * 1024 {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(16 * 1024 + 1).read_to_end(&mut bytes).ok()?;
        Palette::parse(std::str::from_utf8(&bytes).ok()?)
    }
    read(home).unwrap_or_default()
}
