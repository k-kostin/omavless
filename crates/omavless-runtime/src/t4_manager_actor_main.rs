// SPDX-License-Identifier: MIT
//! Explicit opt-in developer entry; never called by the installed daemon.

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if arguments == ["--actor"] {
        omavless_runtime::manager_actor_service::actor_entry()
    } else if arguments == ["--observe-manager"] {
        omavless_runtime::manager_actor_service::supervisor_entry()
    } else {
        Err(omavless_runtime::manager_actor_service::Unavailable)
    };
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
