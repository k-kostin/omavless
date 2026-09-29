// SPDX-License-Identifier: MIT

//! Explicit disposable-VM check only. No private frame, setting, environment
//! value, helper stderr or internal error is ever rendered here.

fn main() {
    if std::env::args_os().len() != 1 {
        std::process::exit(2);
    }
    match omavless_s1_observer::observe_via_fixed_runner() {
        Ok(observation) => {
            let _ = observation; // still unverified; write admission always refuses
            println!("observer=unverified");
        }
        Err(omavless_s1_observer::RunnerError::HelperUnavailable) => {
            println!("observer=unavailable");
            std::process::exit(1);
        }
        Err(_) => {
            println!("observer=refused");
            std::process::exit(1);
        }
    }
}
