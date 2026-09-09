// SPDX-License-Identifier: MIT
//! Read-only opt-in: requires both installed owners already stopped.
use omavless_runtime::production_observation::ProductionOwnershipObserver;

#[test]
fn strict_empty_host_optin() {
    if std::env::var("OMAVLESS_TEST_EMPTY_HOST").as_deref() != Ok("1") {
        return;
    }
    let observer = ProductionOwnershipObserver::current().expect("trusted host paths required");
    // Churn invalidates a scan, not an empty result. Retry the whole snapshot.
    for _ in 0..3 {
        if observer.verify_empty().is_ok() {
            eprintln!("strict empty host: PASS (services inactive; core/TUN/controllers absent)");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("host emptiness could not be verified");
}
