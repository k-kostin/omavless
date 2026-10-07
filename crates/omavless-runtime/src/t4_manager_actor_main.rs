// SPDX-License-Identifier: MIT
//! Explicit opt-in developer entry; never called by the installed daemon.

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if arguments == ["--actor"] {
        omavless_runtime::manager_actor_service::actor_entry()
    } else if arguments == ["--actor-canonical"] {
        omavless_runtime::manager_actor_service::actor_canonical_entry()
    } else if arguments == ["--actor-mixed-writer"] {
        omavless_runtime::manager_actor_service::actor_mixed_writer_entry()
    } else if arguments == ["--actor-inspector"] {
        omavless_runtime::manager_actor_service::actor_inspector_entry()
    } else if arguments == ["--actor-fixed-rollback"] {
        omavless_runtime::manager_actor_service::actor_fixed_rollback_entry()
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
        [arg] if arg == "--observe-canonical-stopped" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CanonicalStopped)
        }
        [arg] if arg == "--authenticate-canonical-backup" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CanonicalAuthenticate)
        }
        [arg] if arg == "--stage-canonical-synthetic-backup" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CanonicalStage)
        }
        [arg] if arg == "--commit-canonical-synthetic-backup" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CanonicalCommit)
        }
        [arg] if arg == "--inspect-fixed-interrupted-transaction" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CanonicalInterruptedInspection)
        }
        [arg] if arg == "--rollback-fixed-mixed-transaction" => {
            Some(omavless_runtime::manager_actor_service::DeveloperScenario::CanonicalFixedMixedRollback)
        }
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
