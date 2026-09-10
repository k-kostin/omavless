// SPDX-License-Identifier: MIT
//! Fixed sysfs counters attributed by the authenticated parent-owned controller.
//! No controller stream, connections, device-name request or private data.
use crate::mutation_protocol::MutationProtocolError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq, Eq)]
pub struct TrafficCounters {
    pub identity: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub sampled_at_ms: u64,
}

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "runtime.traffic" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn project(sample: Option<TrafficCounters>) -> Value {
    json!({"schemaVersion":1,"scope":"controller_attributed_tun_counters","availability":if sample.is_some() {"observed"} else {"unavailable"},
        "sample":sample.map(|s| json!({"identity":s.identity,"rxBytes":s.rx_bytes,"txBytes":s.tx_bytes,"sampledAtMs":s.sampled_at_ms}))})
}

fn bounded(path: &Path, max: usize, deadline: Instant) -> Option<String> {
    if Instant::now() >= deadline {
        return None;
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take((max + 1) as u64).read_to_end(&mut bytes).ok()?;
    if bytes.len() > max || Instant::now() >= deadline {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn integer(path: &Path, deadline: Instant) -> Option<u64> {
    let text = bounded(path, 32, deadline)?;
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: u64 = text.parse().ok()?;
    // Exact JavaScript integer range; no silently rounded totals in the UI.
    (value <= 9_007_199_254_740_991).then_some(value)
}

#[cfg(test)]
fn interface(proc_root: &Path, pid: u32, deadline: Instant) -> Option<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut count = 0;
    for entry in fs::read_dir(proc_root.join(pid.to_string()).join("fdinfo")).ok()? {
        count += 1;
        if count > 1024 || Instant::now() >= deadline {
            return None;
        }
        let entry = entry.ok()?;
        if !entry
            .file_name()
            .to_str()?
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let contents = bounded(&entry.path(), 4096, deadline)?;
        let mut found = false;
        for line in contents.lines() {
            if let Some(value) = line.strip_prefix("iff:") {
                if found {
                    return None;
                }
                found = true;
                let value = value.trim();
                if value.is_empty()
                    || value.len() > 15
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    || value == "."
                    || value == ".."
                {
                    return None;
                }
                names.insert(value.to_owned());
            }
        }
    }
    if names.len() != 1 {
        return None;
    }
    names.into_iter().next()
}

/// Caller verifies live owned child/controller both before and after this read.
#[cfg(test)]
pub(crate) fn read(
    proc_root: &Path,
    sys_net: &Path,
    pid: u32,
    generation: u64,
) -> Option<TrafficCounters> {
    let deadline = Instant::now() + Duration::from_millis(100);
    let name = interface(proc_root, pid, deadline)?;
    let sample = read_device(sys_net, pid, generation, &name)?;
    (interface(proc_root, pid, deadline)? == name).then_some(sample)
}

/// Device is supplied only by the PID-authenticated private controller, never IPC.
/// Caller brackets this with fresh controller identity/device and lifecycle checks.
pub(crate) fn read_device(
    sys_net: &Path,
    pid: u32,
    generation: u64,
    name: &str,
) -> Option<TrafficCounters> {
    let deadline = Instant::now() + Duration::from_millis(100);
    if !valid_device(name) {
        return None;
    }
    let path = sys_net.join(name);
    let flags = bounded(&path.join("tun_flags"), 32, deadline)?;
    let flags = u32::from_str_radix(flags.trim().strip_prefix("0x")?, 16).ok()?;
    if flags & 3 != 1 {
        return None;
    }
    let index = integer(&path.join("ifindex"), deadline)?;
    if index == 0 {
        return None;
    }
    let rx = integer(&path.join("statistics/rx_bytes"), deadline)?;
    let tx = integer(&path.join("statistics/tx_bytes"), deadline)?;
    if integer(&path.join("ifindex"), deadline)? != index {
        return None;
    }
    let token = format!("{pid}:{generation}:{index}");
    let identity = format!("{:x}", Sha256::digest(token.as_bytes()));
    static CLOCK: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let sampled_at_ms =
        u64::try_from(CLOCK.get_or_init(Instant::now).elapsed().as_millis()).ok()?;
    if sampled_at_ms > 9_007_199_254_740_991 {
        return None;
    }
    Some(TrafficCounters {
        identity,
        rx_bytes: rx,
        tx_bytes: tx,
        sampled_at_ms,
    })
}

pub(crate) fn controller_device(payload: &Value) -> Option<&str> {
    if payload["tun"]["enable"] != true {
        return None;
    }
    let name = payload["tun"]["device"].as_str()?;
    valid_device(name).then_some(name)
}

fn valid_device(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omavless-traffic-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir_all(path.join("proc/123/fdinfo")).unwrap();
            fs::create_dir_all(path.join("net/Meta/statistics")).unwrap();
            fs::write(
                path.join("proc/123/fdinfo/3"),
                "pos:\t0\nflags:\t02\niff:\tMeta\n",
            )
            .unwrap();
            fs::write(path.join("net/Meta/tun_flags"), "0x1001\n").unwrap();
            fs::write(path.join("net/Meta/ifindex"), "7\n").unwrap();
            fs::write(path.join("net/Meta/statistics/rx_bytes"), "123456\n").unwrap();
            fs::write(path.join("net/Meta/statistics/tx_bytes"), "654321\n").unwrap();
            Self(path)
        }
        fn read(&self) -> Option<TrafficCounters> {
            read(&self.0.join("proc"), &self.0.join("net"), 123, 4)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn original_sysfs_rx_tx_semantics_and_counter_resets_are_preserved() {
        let f = Fixture::new();
        let first = f.read().unwrap();
        // These are the same two files and orientation used by Service.qml's
        // trafficScript. No controller up/down rate is substituted for totals.
        assert_eq!((first.rx_bytes, first.tx_bytes), (123456, 654321));
        fs::write(f.0.join("net/Meta/statistics/rx_bytes"), "0\n").unwrap();
        assert_eq!(f.read().unwrap().rx_bytes, 0);
        assert_eq!(f.read().unwrap().identity, first.identity);
        fs::write(f.0.join("net/Meta/ifindex"), "8\n").unwrap();
        assert_ne!(f.read().unwrap().identity, first.identity);
        assert_ne!(
            read(&f.0.join("proc"), &f.0.join("net"), 123, 5)
                .unwrap()
                .identity,
            f.read().unwrap().identity
        );
    }
    #[test]
    fn shared_qml_oracle_corpus_preserves_exact_kernel_totals() {
        let cases: Value =
            serde_json::from_str(include_str!("../../../tests/traffic-cases.json")).unwrap();
        let f = Fixture::new();
        for case in cases.as_array().unwrap() {
            fs::write(
                f.0.join("net/Meta/statistics/rx_bytes"),
                format!("{}\n", case["rx"]),
            )
            .unwrap();
            fs::write(
                f.0.join("net/Meta/statistics/tx_bytes"),
                format!("{}\n", case["tx"]),
            )
            .unwrap();
            let actual = f.read().unwrap();
            assert_eq!(actual.rx_bytes, case["rx"].as_u64().unwrap());
            assert_eq!(actual.tx_bytes, case["tx"].as_u64().unwrap());
        }
    }
    #[test]
    fn invisible_ambiguous_wrong_or_traversing_tun_is_unavailable() {
        for body in [
            "pos:\t0\n",
            "iff:\t../private\n",
            "iff:\tMeta\niff:\tMeta\n",
            "iff:\tprivate endpoint\n",
            "iff:\tother\n",
        ] {
            let f = Fixture::new();
            fs::write(f.0.join("proc/123/fdinfo/3"), body).unwrap();
            assert!(f.read().is_none());
        }
        let f = Fixture::new();
        fs::write(f.0.join("proc/123/fdinfo/4"), "iff:\tother\n").unwrap();
        assert!(f.read().is_none());
        fs::write(f.0.join("proc/123/fdinfo/4"), "iff:\tMeta\n").unwrap();
        assert!(f.read().is_some());
        fs::remove_dir_all(f.0.join("proc/123/fdinfo")).unwrap();
        assert!(f.read().is_none());
    }
    #[test]
    fn bounds_non_tun_symlink_and_malformed_counters_fail_closed() {
        for value in [
            "-1",
            "1.5",
            "NaN",
            "9007199254740992",
            "99999999999999999999999999999999999",
        ] {
            let f = Fixture::new();
            fs::write(f.0.join("net/Meta/statistics/rx_bytes"), value).unwrap();
            assert!(f.read().is_none());
        }
        let f = Fixture::new();
        fs::write(f.0.join("net/Meta/tun_flags"), "0x1002\n").unwrap();
        assert!(f.read().is_none());
        fs::write(f.0.join("net/Meta/tun_flags"), "0x1001\n").unwrap();
        fs::remove_file(f.0.join("net/Meta/statistics/rx_bytes")).unwrap();
        std::os::unix::fs::symlink("tx_bytes", f.0.join("net/Meta/statistics/rx_bytes")).unwrap();
        assert!(f.read().is_none());
        let f = Fixture::new();
        fs::write(f.0.join("proc/123/fdinfo/3"), "x".repeat(4097)).unwrap();
        assert!(f.read().is_none());
        assert!(bounded(&f.0.join("net/Meta/ifindex"), 32, Instant::now()).is_none());
    }
    #[test]
    fn fixed_request_and_bounded_projection_never_expose_device_or_path() {
        for params in [json!({}), json!({"device":"private-token"}), json!([])] {
            let request = json!({"api":"omavless.control","version":1,"id":"test","method":"runtime.traffic","params":params});
            assert_eq!(validate(&request).is_ok(), params == json!({}));
        }
        let unavailable = project(None);
        assert_eq!(unavailable["availability"], "unavailable");
        assert!(unavailable["sample"].is_null());
        let f = Fixture::new();
        let value = project(f.read());
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(encoded.len() < 400);
        assert!(!encoded.contains("Meta"));
        assert!(!encoded.contains("/proc"));
        assert_eq!(value["sample"]["identity"].as_str().unwrap().len(), 64);
    }
    #[test]
    fn controller_attribution_does_not_require_dumpable_capability_core() {
        let f = Fixture::new();
        fs::remove_dir_all(f.0.join("proc")).unwrap();
        let payload = json!({"tun":{"enable":true,"device":"Meta"}});
        let device = controller_device(&payload).unwrap();
        let sample = read_device(&f.0.join("net"), 123, 4, device).unwrap();
        assert_eq!(sample.rx_bytes, 123456);
        for value in [
            json!({}),
            json!({"tun":{"enable":false,"device":"Meta"}}),
            json!({"tun":{"enable":true,"device":"../private"}}),
            json!({"tun":{"enable":true,"device":""}}),
        ] {
            assert!(controller_device(&value).is_none());
        }
        assert!(read_device(&f.0.join("net"), 123, 4, "other").is_none());
        assert!(read_device(&f.0.join("net"), 123, 4, "../private").is_none());
    }
}
