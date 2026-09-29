// SPDX-License-Identifier: MIT

use std::io::{IsTerminal, Write};

fn main() {
    // Private protocol for a future fixed-purpose runner. Never print settings
    // or GLib/D-Bus errors to the terminal on an invalid invocation.
    if std::env::args_os().skip(1).collect::<Vec<_>>()
        != [std::ffi::OsString::from("--private-observe-v1")]
        || std::io::stdout().is_terminal()
    {
        std::process::exit(2);
    }
    let frame = match omavless_s1_observer::observe_read_only()
        .and_then(|observation| observation.encode_private_frame())
    {
        Ok(frame) => frame,
        Err(_) => std::process::exit(1),
    };
    if std::io::stdout().write_all(&frame).is_err() {
        std::process::exit(1);
    }
}
