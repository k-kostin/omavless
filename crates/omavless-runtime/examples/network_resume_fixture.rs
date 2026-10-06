// SPDX-License-Identifier: MIT
//! Disabled-by-default fixed owned fixture; never an installed runtime command.
fn main() {
    if std::env::args_os().len() != 1 {
        eprintln!("This fixed developer fixture takes no arguments");
        std::process::exit(2);
    }
    println!("{}", omavless_runtime::developer_network_resume_fixture());
}
