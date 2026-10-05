// SPDX-License-Identifier: MIT
fn main() {
    // The explicit developer binary has no default fallback or shell executor.
    std::process::exit(omavless_netguard::service_core::entry());
}
