// SPDX-License-Identifier: MIT
fn main() {
    if std::env::args_os().len() != 1 {
        std::process::exit(2);
    }
    if omavless_s1_observer::probe_manager_continuity_read_only().is_ok() {
        println!("manager_continuity=observed_unverified");
    } else {
        println!("manager_continuity=refused");
        std::process::exit(1);
    }
}
