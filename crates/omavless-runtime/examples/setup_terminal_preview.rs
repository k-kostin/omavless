// SPDX-License-Identifier: MIT
// Inert visual-review fixture: reuses production text, no setup/host operation.
#[path = "../src/setup_upgrade082/terminal_copy.rs"]
mod terminal_copy;

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() > 2 || args.is_empty() {
        eprintln!("Usage: setup_terminal_preview prepare|upgrade|prepared [en|ru]");
        std::process::exit(2);
    }
    let locale = if let Some(label) = args.get(1) {
        if label != "en" && label != "ru" {
            std::process::exit(2);
        }
        terminal_copy::Locale::from_label(label)
    } else {
        terminal_copy::Locale::current()
    };
    let mut out = std::io::stdout().lock();
    use std::io::Write;
    writeln!(out, "[READ-ONLY TEXT PREVIEW — no setup or installation]\n")?;
    match args[0].as_str() {
        "prepare" => terminal_copy::prepare_prompt(&mut out, locale),
        "upgrade" => {
            terminal_copy::upgrade_prompt(&mut out, locale, env!("CARGO_PKG_VERSION"), true)
        }
        "prepared" => terminal_copy::prepared_prompt(&mut out, locale),
        _ => {
            std::process::exit(2);
        }
    }
}
