// SPDX-License-Identifier: MIT
//! Small isolated T2 catalog trial. No global locale, extraction or network I/O.
use std::{collections::BTreeMap, sync::LazyLock};

static CATALOG: LazyLock<BTreeMap<String, BTreeMap<String, String>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../locales.json")).expect("trusted compiled TUI catalog")
});

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Locale {
    En,
    Ru,
}
impl Locale {
    pub fn parse(value: &str) -> Self {
        let language = value.split(['_', '-', '.']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("ru") {
            Self::Ru
        } else {
            Self::En
        }
    }
    pub fn current() -> Self {
        ["OMAVLESS_LOCALE", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|key| std::env::var(key).ok().filter(|s| !s.is_empty()))
            .map_or(Self::En, |s| Self::parse(&s))
    }
    pub fn text(self, key: &str) -> &'static str {
        CATALOG
            .get(key)
            .and_then(|entry| {
                entry
                    .get(if self == Self::Ru { "ru" } else { "en" })
                    .or_else(|| entry.get("en"))
            })
            .map_or("Missing translation", String::as_str)
    }
}
