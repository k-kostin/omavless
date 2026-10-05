// SPDX-License-Identifier: MIT
//! Explicit opt-in developer entry; never called by the installed daemon.

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if arguments == ["--actor"] {
        omavless_runtime::manager_actor_service::actor_entry()
    } else if arguments == ["--observe-manager"] {
        omavless_runtime::manager_actor_service::supervisor_entry()
    } else if let Some(scenario) = match arguments.as_slice() {
        [arg] if arg == "--capacity-three" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CapacityThree)
        }
        [arg] if arg == "--capacity-fourth" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CapacityFourth)
        }
        [arg] if arg == "--wrong-nonce-after-first" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::WrongNonceAfterFirst)
        }
        [arg] if arg == "--partial-after-first" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::PartialAfterFirst)
        }
        [arg] if arg == "--disconnect-after-first" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::DisconnectAfterFirst)
        }
        [arg] if arg == "--authenticate-backup" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::AuthenticateBackup)
        }
        [arg] if arg == "--stage-authenticated-backup" => Some(
            omavless_runtime::manager_actor_service::DeveloperScenario::StageAuthenticatedBackup,
        ),
        _ => None,
    } {
        omavless_runtime::manager_actor_service::supervisor_scenario(scenario)
    } else {
        Err(omavless_runtime::manager_actor_service::Unavailable)
    };
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
