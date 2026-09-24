// SPDX-License-Identifier: MIT
//! Client-session presentation only. No persistence, environment writes or IPC.
use crate::{i18n::Locale, theme::Palette};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Language {
    #[default]
    Automatic,
    English,
    Russian,
}
impl Language {
    pub fn key(self) -> &'static str {
        match self {
            Self::Automatic => "tui.language_auto",
            Self::English => "tui.language_en",
            Self::Russian => "tui.language_ru",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Theme {
    #[default]
    Automatic,
    Default,
}
impl Theme {
    pub fn key(self) -> &'static str {
        match self {
            Self::Automatic => "tui.theme_auto",
            Self::Default => "tui.theme_default",
        }
    }
}

pub struct Settings {
    pub language: Language,
    pub theme: Theme,
    startup_locale: Locale,
    detected_palette: Palette,
}
impl Settings {
    pub fn new(startup_locale: Locale) -> Self {
        Self {
            language: Language::Automatic,
            theme: Theme::Automatic,
            startup_locale,
            detected_palette: Palette::default(),
        }
    }
    pub fn locale(&self) -> Locale {
        match self.language {
            Language::Automatic => self.startup_locale,
            Language::English => Locale::En,
            Language::Russian => Locale::Ru,
        }
    }
    pub fn palette(&self) -> Palette {
        match self.theme {
            Theme::Automatic => self.detected_palette,
            Theme::Default => Palette::default(),
        }
    }
    pub fn update_palette(&mut self, palette: Palette) {
        self.detected_palette = palette;
    }
    pub fn next_language(&mut self) {
        self.language = match self.language {
            Language::Automatic => Language::English,
            Language::English => Language::Russian,
            Language::Russian => Language::Automatic,
        };
    }
    pub fn next_theme(&mut self) {
        self.theme = match self.theme {
            Theme::Automatic => Theme::Default,
            Theme::Default => Theme::Automatic,
        };
    }
    pub fn reset(&mut self) {
        self.language = Language::Automatic;
        self.theme = Theme::Automatic;
    }
}
