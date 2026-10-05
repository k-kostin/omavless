// SPDX-License-Identifier: MIT
//! Explicit developer fixture, never installed or invoked by normal runtime.
use omavless_netguard::{
    client_candidate::{ClientError, FixedClient},
    protocol::{Health, Mode, POLICY_VERSION, Protection, Request, Response},
};
use std::process::ExitCode;

const STEPS: [(Request, Protection); 4] = [
    (
        Request::Status {},
        Protection::Disarmed {
            closed_generation: None,
        },
    ),
    (
        Request::Arm {
            generation: 1,
            mode: Mode::Full,
        },
        Protection::Armed { generation: 1 },
    ),
    (
        Request::Disarm { generation: 1 },
        Protection::Disarmed {
            closed_generation: Some(1),
        },
    ),
    (
        Request::Status {},
        Protection::Disarmed {
            closed_generation: Some(1),
        },
    ),
];
const REFUSALS: [&str; 4] = [
    "K1_SCENARIO_INITIAL_STATUS_REFUSED",
    "K1_SCENARIO_ARM_REFUSED",
    "K1_SCENARIO_DISARM_REFUSED",
    "K1_SCENARIO_FINAL_STATUS_REFUSED",
];

// Exactly four distinct operations on the SAME owner. Every error or unexpected
// positive is terminal: no retry, compensating disarm, or diagnostic query.
fn sequence(
    mut exchange: impl FnMut(Request) -> Result<Response, ClientError>,
) -> Result<(), usize> {
    for (index, (request, protection)) in STEPS.into_iter().enumerate() {
        let expected = Response::Status {
            policy_version: POLICY_VERSION,
            protection,
            health: Health::Verified,
        };
        if exchange(request) != Ok(expected) {
            return Err(index);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    // Refuse inputs before constructing even the lazy fixed client. A genuine
    // enrolled nonroot account in the canonical VM environment is prerequisite.
    if std::env::args_os().nth(1).is_some() || nix::unistd::geteuid().is_root() {
        println!("K1_SCENARIO_INVOCATION_REFUSED");
        return ExitCode::from(2);
    }
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        let mut client = FixedClient::new();
        sequence(|request| client.exchange(request))
    });
    let (label, exit) = match result {
        Ok(Ok(())) => ("K1_SCENARIO_FOUR_EXCHANGES_VERIFIED", 0),
        Ok(Err(index)) => (REFUSALS[index], 2),
        Err(_) => ("K1_SCENARIO_PANIC_UNKNOWN", 2),
    };
    // One finite public literal, only AFTER operations end. No response, path,
    // identity, error text, environment, or private payload is printed.
    println!("{label}");
    ExitCode::from(exit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_netguard::protocol::ErrorCode;

    fn expected(index: usize) -> Response {
        Response::Status {
            policy_version: POLICY_VERSION,
            protection: STEPS[index].1,
            health: Health::Verified,
        }
    }

    #[test]
    fn exact_order_and_single_owner_four_operations() {
        let mut calls = Vec::new();
        assert_eq!(
            sequence(|request| {
                let response = expected(calls.len());
                calls.push(request);
                Ok(response)
            }),
            Ok(())
        );
        assert_eq!(calls, STEPS.map(|step| step.0));
    }

    #[test]
    fn every_error_stops_at_original_cut() {
        for cut in 0..4 {
            let mut calls = Vec::new();
            assert_eq!(
                sequence(|request| {
                    let index = calls.len();
                    calls.push(request);
                    if index == cut {
                        Err(ClientError::UnavailableOrUnknown)
                    } else {
                        Ok(expected(index))
                    }
                }),
                Err(cut)
            );
            assert_eq!(
                calls,
                STEPS[..=cut].iter().map(|step| step.0).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn all_nonexact_responses_stop_without_followup() {
        let protections = [
            Protection::Disarmed {
                closed_generation: None,
            },
            Protection::Disarmed {
                closed_generation: Some(0),
            },
            Protection::Disarmed {
                closed_generation: Some(1),
            },
            Protection::Disarmed {
                closed_generation: Some(2),
            },
            Protection::Armed { generation: 0 },
            Protection::Armed { generation: 1 },
            Protection::Armed { generation: 2 },
            Protection::Emergency {},
        ];
        let mut responses = vec![Response::Error {
            code: ErrorCode::Unavailable,
        }];
        for protection in protections {
            for health in [Health::Verified, Health::ManualRecoveryRequired] {
                for policy_version in [0, POLICY_VERSION, POLICY_VERSION + 1] {
                    responses.push(Response::Status {
                        policy_version,
                        protection,
                        health,
                    });
                }
            }
        }
        for cut in 0..4 {
            for response in responses
                .iter()
                .copied()
                .filter(|response| *response != expected(cut))
            {
                let mut count = 0;
                assert_eq!(
                    sequence(|request| {
                        assert_eq!(request, STEPS[count].0);
                        let result = if count == cut {
                            response
                        } else {
                            expected(count)
                        };
                        count += 1;
                        Ok(result)
                    }),
                    Err(cut)
                );
                assert_eq!(count, cut + 1);
            }
        }
    }
}
