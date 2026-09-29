// SPDX-License-Identifier: MIT

//! Disposable-VM observation only. No process identity or bus response output.

fn main() {
    if std::env::args_os().len() != 1 {
        std::process::exit(2);
    }
    match omavless_s1_observer::probe_auth_sender_read_only() {
        Ok(probe) => {
            println!("auth_sender=observed_unverified");
            println!(
                "listener_matches_sender={}",
                probe.listener_matches_sender()
            );
        }
        Err(_) => {
            println!("auth_sender=refused");
            std::process::exit(1);
        }
    }
}
