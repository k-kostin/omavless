// SPDX-License-Identifier: MIT
//! Presentation only. Locale never changes setup admission or consent tokens.
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Locale {
    En,
    Ru,
}

impl Locale {
    pub(super) fn from_label(label: &str) -> Self {
        match label {
            "ru" | "ru_RU" | "ru-RU" => Self::Ru,
            _ => Self::En,
        }
    }

    pub(super) fn current() -> Self {
        Self::from_label(&std::env::var("OMAVLESS_LOCALE").unwrap_or_default())
    }
}

pub(super) fn prepare_prompt(out: &mut impl Write, locale: Locale) -> io::Result<()> {
    let text = match locale {
        Locale::En => {
            "Prepare component update\n\nOmaVLESS must be shut down before updating.\nIf needed, this step safely stops the OmaVLESS DNS helper. A separate system permission request may appear.\nPackages are not installed yet. Profiles, settings and existing helper access are kept.\nThe application and VPN will not start.\n\nType PREPARE to continue, or press Enter to cancel:"
        }
        Locale::Ru => {
            "Подготовка обновления компонентов\n\nПеред обновлением OmaVLESS должен быть выключен.\nПри необходимости этот шаг безопасно останавливает DNS-помощник OmaVLESS. Может появиться отдельный запрос разрешения системы.\nПакеты пока не устанавливаются. Профили, настройки и существующий доступ к помощнику сохраняются.\nПриложение и VPN не запускаются.\n\nВведите PREPARE для продолжения или нажмите Enter для отмены:"
        }
    };
    writeln!(out, "{text}")
}

pub(super) fn upgrade_prompt(
    out: &mut impl Write,
    locale: Locale,
    version: &str,
    delivered: bool,
) -> io::Result<()> {
    match locale {
        Locale::En => writeln!(
            out,
            "Update OmaVLESS to {version}\n\nInstall two packages together: OmaVLESS and its VPN core with the DNS helper.\nProfiles and settings are kept. The application and VPN stay off.\nAfter installation, Set up DNS helper and Start OmaVLESS are separate steps.\n\nType {} to continue, or press Enter to cancel:",
            if delivered {
                "UPGRADE"
            } else {
                "UPGRADE SINGLE-USER VM"
            }
        ),
        Locale::Ru => writeln!(
            out,
            "Обновление OmaVLESS до {version}\n\nУстановить вместе два пакета: приложение OmaVLESS и ядро VPN с DNS-помощником.\nПрофили и настройки сохраняются. Приложение и VPN остаются выключенными.\nПосле установки отдельно выполните «Настроить DNS-помощник» и «Запустить OmaVLESS».\n\nВведите {} для продолжения или нажмите Enter для отмены:",
            if delivered {
                "UPGRADE"
            } else {
                "UPGRADE SINGLE-USER VM"
            }
        ),
    }
}

pub(super) fn prepared_prompt(out: &mut impl Write, locale: Locale) -> io::Result<()> {
    let text = match locale {
        Locale::En => {
            "Preparation complete\n\nPackages, profiles, settings and existing helper access are unchanged.\nNext, install two packages together: OmaVLESS and its VPN core with the DNS helper.\nIf you cancel now, the old packages remain. Cancelling does not restart the DNS helper.\nThe application and VPN will not start.\n\nType UPGRADE to install, or press Enter to cancel:"
        }
        Locale::Ru => {
            "Подготовка завершена\n\nПакеты, профили, настройки и существующий доступ к помощнику не изменены.\nСледующий шаг — установить вместе два пакета: приложение OmaVLESS и ядро VPN с DNS-помощником.\nПри отмене останутся старые пакеты. DNS-помощник не будет запущен.\nПриложение и VPN не запускаются.\n\nВведите UPGRADE для установки или нажмите Enter для отмены:"
        }
    };
    writeln!(out, "{text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_is_bounded_presentation_with_english_fallback() {
        for label in ["ru", "ru_RU", "ru-RU"] {
            assert_eq!(Locale::from_label(label), Locale::Ru);
        }
        for label in ["", "en", "de", "RU", "ru;command", "../ru"] {
            assert_eq!(Locale::from_label(label), Locale::En);
        }
    }

    #[test]
    fn every_prompt_names_effect_consent_and_cancellation_in_both_locales() {
        for locale in [Locale::En, Locale::Ru] {
            let mut before = Vec::new();
            prepare_prompt(&mut before, locale).unwrap();
            let before = String::from_utf8(before).unwrap();
            assert!(before.contains("PREPARE") && before.contains("Enter"));
            assert!(!before.contains("UPGRADE"));
            let mut after = Vec::new();
            prepared_prompt(&mut after, locale).unwrap();
            let after = String::from_utf8(after).unwrap();
            assert!(after.contains("UPGRADE") && after.contains("Enter"));
            assert!(after.contains(if locale == Locale::En {
                "does not restart the DNS helper"
            } else {
                "помощник не будет запущен"
            }));
            for delivered in [false, true] {
                let mut first = Vec::new();
                upgrade_prompt(&mut first, locale, "0.9.8", delivered).unwrap();
                let first = String::from_utf8(first).unwrap();
                assert!(first.contains("0.9.8") && first.contains("Enter"));
                assert_eq!(first.contains("SINGLE-USER VM"), !delivered);
                assert!(first.contains(if locale == Locale::En {
                    "Start OmaVLESS"
                } else {
                    "Запустить OmaVLESS"
                }));
            }
            for text in [before, after] {
                for jargon in ["idle-gate", "broker", "enrollment", "socket", "journal"] {
                    assert!(!text.contains(jargon));
                }
            }
        }
    }

    #[test]
    fn broken_prompt_output_is_not_ignored() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert!(prepare_prompt(&mut Broken, Locale::En).is_err());
        assert!(prepared_prompt(&mut Broken, Locale::Ru).is_err());
        assert!(upgrade_prompt(&mut Broken, Locale::En, "0.9.8", true).is_err());
    }
}
