// SPDX-License-Identifier: MIT
//! Framework-neutral synthetic colors. No filesystem or production theme API.
pub const MAX_THEME_BYTES: usize = 4096;
pub const MAX_THEME_SEQUENCE: u64 = (1 << 53) - 1;
pub const REQUIRED: [&str; 9] = [
    "background",
    "foreground",
    "accent",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
];
pub const OPTIONAL: [&str; 6] = [
    "selection",
    "lighter_background",
    "dark_background",
    "light_foreground",
    "bright_foreground",
    "dark_foreground",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Good,
    Missing,
    Malformed,
    Oversize,
    Unavailable,
}
impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Missing => "missing",
            Self::Malformed => "malformed",
            Self::Oversize => "oversize",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    light: bool,
    colors: [u32; 9],
    optional: [Option<u32>; 6],
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            light: false,
            colors: [
                0x1a1b26, 0xc0caf5, 0x7aa2f7, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7,
                0x7dcfff,
            ],
            optional: [None; 6],
        }
    }
}
impl Palette {
    pub fn mode(&self) -> &'static str {
        if self.light { "light" } else { "dark" }
    }
    pub fn fields(&self) -> impl Iterator<Item = (&'static str, u32)> + '_ {
        REQUIRED.into_iter().zip(self.colors).chain(
            OPTIONAL
                .into_iter()
                .zip(self.optional)
                .filter_map(|(key, value)| value.map(|value| (key, value))),
        )
    }
    // Only validated fixed keys/color values are reconstructed, never raw input.
    pub fn text(&self) -> String {
        use std::fmt::Write;
        let mut text = format!("mode = \"{}\"\n", self.mode());
        for (key, value) in self.fields() {
            writeln!(text, "{key} = \"#{value:06x}\"").expect("write to string");
        }
        text
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub status: Status,
    pub palette: Palette,
}
impl Prepared {
    pub fn fallback(status: Status) -> Self {
        Self {
            status,
            palette: Palette::default(),
        }
    }
    pub fn from_bytes(source: Option<&[u8]>) -> Self {
        let Some(source) = source else {
            return Self::fallback(Status::Missing);
        };
        if source.len() > MAX_THEME_BYTES {
            return Self::fallback(Status::Oversize);
        }
        let Ok(source) = std::str::from_utf8(source) else {
            return Self::fallback(Status::Malformed);
        };
        Self::parse(source).unwrap_or_else(|| Self::fallback(Status::Malformed))
    }
    fn parse(source: &str) -> Option<Self> {
        if !source.is_ascii()
            || source
                .bytes()
                .any(|b| b.is_ascii_control() && !matches!(b, b'\n' | b'\r' | b'\t'))
        {
            return None;
        }
        let mut mode = None;
        let mut colors = [None; 9];
        let mut optional = [None; 6];
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
            if key == "mode" {
                if mode.is_some() {
                    return None;
                }
                mode = Some(match value {
                    "dark" => false,
                    "light" => true,
                    _ => return None,
                });
                continue;
            }
            let hex = value.strip_prefix('#')?;
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let color = u32::from_str_radix(hex, 16).ok()?;
            let slot = if let Some(index) = REQUIRED.iter().position(|name| *name == key) {
                &mut colors[index]
            } else {
                &mut optional[OPTIONAL.iter().position(|name| *name == key)?]
            };
            if slot.replace(color).is_some() {
                return None;
            }
        }
        let mut palette = Palette {
            light: mode?,
            colors: [0; 9],
            optional,
        };
        for (index, color) in colors.into_iter().enumerate() {
            palette.colors[index] = color?;
        }
        Some(Self {
            status: Status::Good,
            palette,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub sequence: u64,
    pub prepared: Prepared,
}
pub struct Delivery {
    pub sample: Sample,
    pub palette_changed: bool,
    pub status_changed: bool,
}
pub struct ThemeUpdates {
    pub sequence: u64,
    pub current: Sample,
    pending: Option<Sample>,
}
impl Default for ThemeUpdates {
    fn default() -> Self {
        Self {
            sequence: 0,
            current: Sample {
                sequence: 0,
                prepared: Prepared::fallback(Status::Unavailable),
            },
            pending: None,
        }
    }
}
impl ThemeUpdates {
    pub fn receive(&mut self, sequence: u64, prepared: Prepared) -> bool {
        if sequence <= self.sequence
            || sequence > MAX_THEME_SEQUENCE
            || (prepared.status != Status::Good && prepared.palette != Palette::default())
        {
            return false;
        }
        self.sequence = sequence;
        self.pending = Some(Sample { sequence, prepared });
        true
    }
    pub fn flush(&mut self) -> Option<Delivery> {
        let sample = self.pending.take()?;
        let palette_changed = sample.prepared.palette != self.current.prepared.palette;
        let status_changed = sample.prepared.status != self.current.prepared.status;
        self.current = sample.clone();
        Some(Delivery {
            sample,
            palette_changed,
            status_changed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(name: &str) -> String {
        let file = include_str!("shell/palette.js");
        file.split_once(&format!("export const {name} = `"))
            .unwrap()
            .1
            .split_once('`')
            .unwrap()
            .0
            .into()
    }
    #[test]
    fn shared_whole_palette_cases() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("theme-fixtures.json")).unwrap();
        assert_eq!(fixture["maxBytes"], MAX_THEME_BYTES);
        assert_eq!(fixture["required"], serde_json::json!(REQUIRED));
        assert_eq!(fixture["optional"], serde_json::json!(OPTIONAL));
        let dark = source("DARK");
        let light = source("LIGHT");
        let broken = source("BROKEN");
        let dark_prepared = Prepared::from_bytes(Some(dark.as_bytes()));
        assert_eq!(
            Prepared::from_bytes(Some(dark_prepared.palette.text().as_bytes())).palette,
            dark_prepared.palette
        );
        assert_eq!(dark_prepared.palette, Palette::default());
        for (key, value) in Palette::default().fields() {
            assert_eq!(fixture["default"][key], format!("#{value:06x}"));
        }
        for case in fixture["cases"].as_array().unwrap() {
            let bytes = match case["input"].as_str().unwrap() {
                "dark" => Some(dark.clone()),
                "light" => Some(light.clone()),
                "broken" => Some(broken.clone()),
                "missing" => None,
                "oversize" => Some(" ".repeat(MAX_THEME_BYTES + 1)),
                "rgba" => Some(dark.replace("#7aa2f7", "#7aa2f7ff")),
                "duplicate" => Some(format!("{dark}\naccent = \"#ffffff\"")),
                "unknown" => Some(format!("{dark}\nname = \"Public fixture\"")),
                "prototype" => Some(format!("{dark}\n__proto__ = \"#ffffff\"")),
                "control" => Some(format!("{dark}\n#\u{1b}")),
                _ => panic!("unknown synthetic case"),
            };
            let prepared = Prepared::from_bytes(bytes.as_deref().map(str::as_bytes));
            assert_eq!(prepared.status.name(), case["status"].as_str().unwrap());
            assert_eq!(prepared.palette.mode(), case["mode"].as_str().unwrap());
            if prepared.status != Status::Good {
                assert_eq!(prepared.palette, Palette::default());
            }
        }
        assert_eq!(
            Prepared::from_bytes(Some(&[0xff])).status,
            Status::Malformed
        );
        assert_eq!(
            Prepared::from_bytes(Some("é".repeat(3000).as_bytes())).status,
            Status::Oversize
        );
    }
    #[test]
    fn latest_slot_admission_and_content_reuse() {
        let mut updates = ThemeUpdates::default();
        let dark = Prepared::from_bytes(Some(source("DARK").as_bytes()));
        let light = Prepared::from_bytes(Some(source("LIGHT").as_bytes()));
        assert!(updates.receive(1, dark.clone()));
        let delivery = updates.flush().unwrap();
        assert!(!delivery.palette_changed && delivery.status_changed);
        for sequence in 2..=100_000 {
            assert!(updates.receive(sequence, light.clone()));
        }
        assert!(!updates.receive(99_999, Prepared::fallback(Status::Malformed)));
        assert!(!updates.receive(MAX_THEME_SEQUENCE + 1, dark.clone()));
        assert!(!updates.receive(
            100_001,
            Prepared {
                status: Status::Missing,
                palette: light.palette.clone()
            }
        ));
        let delivery = updates.flush().unwrap();
        assert_eq!(delivery.sample.sequence, 100_000);
        assert!(delivery.palette_changed);
        assert!(updates.flush().is_none());
        assert!(updates.receive(100_001, light));
        let delivery = updates.flush().unwrap();
        assert!(!delivery.palette_changed && !delivery.status_changed);
        assert!(updates.receive(100_002, Prepared::fallback(Status::Oversize)));
        let delivery = updates.flush().unwrap();
        assert!(delivery.palette_changed && delivery.status_changed);
        assert_eq!(delivery.sample.prepared.palette, Palette::default());
    }
}
