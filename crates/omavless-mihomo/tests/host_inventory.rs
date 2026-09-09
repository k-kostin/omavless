// SPDX-License-Identifier: MIT
//! Explicit opt-in read-only host smoke; never emits process/interface names.
use omavless_mihomo::observation::{processes_named_strict, tun_interface_count_strict};
use std::path::Path;

#[test]
fn strict_real_host_inventory_optin() {
    if std::env::var("OMAVLESS_TEST_HOST_INVENTORY").as_deref() != Ok("1") {
        return;
    }
    // Process churn can invalidate a snapshot. Retry the entire bounded scan;
    // never treat a failed scan as a successful empty result.
    for _ in 0..3 {
        if let (Ok(cores), Ok(tuns)) = (
            processes_named_strict(Path::new("/proc"), "mihomo"),
            tun_interface_count_strict(Path::new("/sys/class/net")),
        ) {
            eprintln!(
                "strict host inventory complete: core_count={}, tun_count={tuns}",
                cores.len()
            );
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("strict host inventory remained incomplete");
}
