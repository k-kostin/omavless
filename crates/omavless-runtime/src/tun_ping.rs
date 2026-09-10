// SPDX-License-Identifier: MIT
//! One fixed, revocable iputils probe. No shell or unbound/source-route fallback.
use crate::desired::DesiredState;
use crate::mutation_protocol::MutationProtocolError;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use serde_json::{Value, json};
use std::io::{ErrorKind, Read};
use std::net::IpAddr;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MAX_INPUT: usize = 256;
pub(crate) const DEADLINE: Duration = Duration::from_secs(3);
const OUTPUT_LIMIT: usize = 4096;
const REAP_BUDGET: Duration = Duration::from_millis(200);

pub fn canonical_host(input: &str) -> Option<String> {
    if input.len() > MAX_INPUT || !input.is_ascii() {
        return None;
    }
    let line = input
        .strip_suffix("\r\n")
        .or_else(|| input.strip_suffix('\n'))
        .unwrap_or(input);
    if line.bytes().any(|b| b.is_ascii_control()) {
        return None;
    }
    let host = line.trim();
    if host.is_empty() || host.len() > 253 {
        return None;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip.is_unspecified() || ip.is_multicast() || ip.is_loopback() {
            return None;
        }
        return Some(ip.to_string());
    }
    if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let domain = host.strip_suffix('.').unwrap_or(host);
    if domain.split('.').any(|part| {
        part.is_empty()
            || part.len() > 63
            || !part.as_bytes()[0].is_ascii_alphanumeric()
            || !part.as_bytes()[part.len() - 1].is_ascii_alphanumeric()
            || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return None;
    }
    Some(domain.to_ascii_lowercase())
}

pub(crate) fn host(request: &Value) -> Result<String, MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "runtime.ping" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if params.len() != 1 {
        return Err(MutationProtocolError::InvalidArgument);
    }
    params
        .get("host")
        .and_then(Value::as_str)
        .and_then(canonical_host)
        .ok_or(MutationProtocolError::InvalidArgument)
}

#[derive(Default)]
struct Slot {
    epoch: u64,
    child: Option<Child>,
}

/// Host-owned fixed ping child, shared only with its bounded read worker.
/// No caller can register an executable or a PID. Intentionally not Debug.
#[derive(Default)]
pub struct PingSlot(Mutex<Slot>);

fn reap(slot: &mut Slot) -> bool {
    let Some(child) = slot.child.as_mut() else {
        return true;
    };
    // try_wait caches completion. Never signal a numeric PID after reaping it.
    match child.try_wait() {
        Ok(Some(_)) => {
            slot.child = None;
            return true;
        }
        Ok(None) => (),
        Err(_) => return false,
    }
    if child.kill().is_err() {
        return false;
    }
    let until = Instant::now() + REAP_BUDGET;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                slot.child = None;
                return true;
            }
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(2)),
            _ => return false,
        }
    }
}

impl PingSlot {
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let _ = std::panic::catch_unwind(|| {
            let _guard = self.0.lock().unwrap();
            panic!("synthetic ping-slot poison");
        });
    }
    pub(crate) fn epoch(&self) -> Option<u64> {
        Some(self.0.lock().ok()?.epoch)
    }
    /// Must succeed before the host stops/replaces/reuses its core/TUN.
    pub(crate) fn revoke(&self) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(next) = slot.epoch.checked_add(1) else {
            return false;
        };
        slot.epoch = next;
        reap(&mut slot)
    }
}
impl Drop for PingSlot {
    fn drop(&mut self) {
        let slot = self
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = reap(slot);
    }
}

pub struct Binding {
    pub(crate) device: String,
    pub(crate) identity: String,
    pub(crate) slot: Arc<PingSlot>,
    pub(crate) epoch: u64,
}
impl PartialEq for Binding {
    fn eq(&self, other: &Self) -> bool {
        self.device == other.device
            && self.identity == other.identity
            && self.epoch == other.epoch
            && Arc::ptr_eq(&self.slot, &other.slot)
    }
}
impl Eq for Binding {}

#[derive(PartialEq, Eq)]
pub(crate) struct Context {
    pub binding: Binding,
    pub desired: DesiredState,
    pub store_digest: [u8; 32],
    pub config_digest: [u8; 32],
    pub pid: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sample {
    Reply(f64),
    Loss,
    Unavailable,
}
fn project(sample: Sample) -> Value {
    let (availability, code, value) = match sample {
        Sample::Reply(ms) => ("observed", "ok", json!({"outcome":"reply","latencyMs":ms})),
        Sample::Loss => (
            "observed",
            "timeout",
            json!({"outcome":"loss","latencyMs":null}),
        ),
        Sample::Unavailable => ("unavailable", "probe_unavailable", Value::Null),
    };
    json!({"schemaVersion":1,"scope":"controller_attributed_tun_icmp","availability":availability,"code":code,"sample":value})
}

fn parse_output(code: Option<i32>, bytes: &[u8]) -> Sample {
    if bytes.len() > OUTPUT_LIMIT {
        return Sample::Unavailable;
    }
    if code == Some(1) {
        return Sample::Loss;
    }
    if code != Some(0) {
        return Sample::Unavailable;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Sample::Unavailable;
    };
    let mut summaries = text
        .lines()
        .filter(|line| line.starts_with("rtt ") || line.starts_with("round-trip "));
    let Some(line) = summaries.next() else {
        return Sample::Unavailable;
    };
    if summaries.next().is_some() {
        return Sample::Unavailable;
    }
    // Canonical iputils LC_ALL=C: min/avg/max/mdev = n/n/n/n ms.
    let Some((_, values)) = line.split_once(" = ") else {
        return Sample::Unavailable;
    };
    let Some(values) = values.strip_suffix(" ms") else {
        return Sample::Unavailable;
    };
    let values = values.split('/').collect::<Vec<_>>();
    if values.len() != 4 {
        return Sample::Unavailable;
    }
    let parsed = values
        .iter()
        .map(|v| v.parse::<f64>())
        .collect::<Result<Vec<_>, _>>();
    let Ok(values) = parsed else {
        return Sample::Unavailable;
    };
    if values
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0 || *v > 3000.0)
    {
        return Sample::Unavailable;
    }
    Sample::Reply(values[1])
}

struct ProbeGuard<'a> {
    slot: &'a PingSlot,
    epoch: u64,
}
impl Drop for ProbeGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.0.lock()
            && slot.epoch == self.epoch
        {
            // Failed reaping keeps the occupied slot; lifecycle revoke must
            // prove cleanup before it may replace the TUN.
            let _ = reap(&mut slot);
        }
    }
}

pub(crate) fn collect(binding: &Binding, host: &str, deadline: Instant) -> Value {
    project(execute(binding, host, deadline, "/usr/bin/ping"))
}

// tool is internal and literal in production; synthetic tests substitute only
// their own fixed fixture. It is never accepted by a method, CLI or host config.
fn execute(
    binding: &Binding,
    host: &str,
    deadline: Instant,
    tool: impl AsRef<std::ffi::OsStr>,
) -> Sample {
    if canonical_host(host).as_deref() != Some(host) || Instant::now() >= deadline {
        return Sample::Unavailable;
    }
    if binding.device.is_empty()
        || binding.device.len() > 15
        || !binding
            .device
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        || binding.device == "."
        || binding.device == ".."
    {
        return Sample::Unavailable;
    }
    let mut output = {
        let Ok(mut slot) = binding.slot.0.lock() else {
            return Sample::Unavailable;
        };
        if slot.epoch != binding.epoch || slot.child.is_some() {
            return Sample::Unavailable;
        }
        let child = Command::new(tool)
            .args([
                "-n",
                "-q",
                "-c",
                "1",
                "-W",
                "2",
                "-I",
                &binding.device,
                "--",
                host,
            ])
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            return Sample::Unavailable;
        };
        let output = child.stdout.take();
        slot.child = Some(child);
        output
    };
    let _guard = ProbeGuard {
        slot: &binding.slot,
        epoch: binding.epoch,
    };
    let Some(ref mut output) = output else {
        return Sample::Unavailable;
    };
    let Ok(flags) = fcntl(&*output, FcntlArg::F_GETFL) else {
        return Sample::Unavailable;
    };
    if fcntl(
        &*output,
        FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
    )
    .is_err()
    {
        return Sample::Unavailable;
    }
    let mut bytes = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Sample::Unavailable;
        }
        let status = {
            let Ok(mut slot) = binding.slot.0.lock() else {
                return Sample::Unavailable;
            };
            if slot.epoch != binding.epoch {
                return Sample::Unavailable;
            }
            let Some(child) = slot.child.as_mut() else {
                return Sample::Unavailable;
            };
            match child.try_wait() {
                Ok(status) => status,
                Err(_) => return Sample::Unavailable,
            }
        };
        let mut buffer = [0u8; 1024];
        let eof = match output.read(&mut buffer) {
            Ok(0) => true,
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
                false
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => false,
            Err(_) => return Sample::Unavailable,
        };
        if bytes.len() > OUTPUT_LIMIT {
            return Sample::Unavailable;
        }
        if let Some(status) = status
            && eof
        {
            return parse_output(status.code(), &bytes);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    fn binding() -> Binding {
        Binding {
            device: "tun-test".into(),
            identity: "opaque".into(),
            slot: Arc::default(),
            epoch: 0,
        }
    }
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new(script: &str) -> Self {
            let root = crate::test_temp::directory("tun-ping").unwrap();
            let path = root.join("ping");
            fs::write(&path, format!("#!/bin/bash\n{script}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(root)
        }
        fn tool(&self) -> std::path::PathBuf {
            self.0.join("ping")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    const REPLY: &[u8] =
        b"PING private.example (192.0.2.1)\nrtt min/avg/max/mdev = 12.5/12.5/12.5/0.0 ms\n";

    #[test]
    fn ping_target_bounds_and_private_cli_semantics() {
        for (input, expected) in [
            ("1.1.1.1", "1.1.1.1"),
            ("2001:db8::1\n", "2001:db8::1"),
            (" Example.COM.\r\n", "example.com"),
        ] {
            assert_eq!(canonical_host(input).as_deref(), Some(expected));
        }
        for input in [
            "",
            "\n",
            "example.com\n\n",
            "x\ny",
            "x\ty",
            "https://private.invalid/key",
            "fe80::1%eth0",
            "::",
            "127.0.0.1",
            "224.0.0.1",
            "999.1.1.1",
            "-n",
            "$(id)",
            "a/b",
            "домен.рф",
        ] {
            assert!(canonical_host(input).is_none(), "invalid target accepted");
        }
        assert!(canonical_host(&"a".repeat(257)).is_none());
        assert!(canonical_host(&format!("{}.example", "a".repeat(64))).is_none());
        let args = ["runtime".into(), "ping".into()];
        let (method, params) = crate::semantic_cli::parse_semantic_ping(&args, "example.com\n")
            .unwrap()
            .into_parts();
        assert_eq!(method, "runtime.ping");
        assert!(params["host"] == "example.com");
        assert!(
            crate::semantic_cli::parse_semantic_ping(
                &["runtime".into(), "ping".into(), "private".into()],
                "1.1.1.1"
            )
            .is_err()
        );
    }
    #[test]
    fn ping_request_is_exact_and_never_an_arbitrary_command() {
        let request = |params| {
            omavless_control_protocol::make_request("ping-test", "runtime.ping", params).unwrap()
        };
        assert!(host(&request(json!({"host":"1.1.1.1"}))).is_ok());
        for params in [
            json!({}),
            json!({"host":1}),
            json!({"host":"1.1.1.1","device":"eth0"}),
            json!({"host":"https://private.invalid/password=secret"}),
            json!({"command":"private"}),
        ] {
            assert!(host(&request(params)).is_err());
        }
    }
    #[test]
    fn ping_response_distinguishes_loss_from_unavailable_and_never_echoes() {
        assert_eq!(parse_output(Some(0), REPLY), Sample::Reply(12.5));
        assert_eq!(parse_output(Some(1), b"private error"), Sample::Loss);
        assert_eq!(parse_output(Some(2), REPLY), Sample::Unavailable);
        for raw in [
            "",
            "rtt x = NaN/NaN/NaN/NaN ms",
            "rtt x = 1/3001/1/0 ms",
            "rtt x = 1/-1/1/0 ms",
            "rtt x = 1/1/1/0 ms\nrtt x = 1/1/1/0 ms",
        ] {
            assert_eq!(parse_output(Some(0), raw.as_bytes()), Sample::Unavailable);
        }
        assert_eq!(
            parse_output(Some(1), &[b'x'; OUTPUT_LIMIT + 1]),
            Sample::Unavailable
        );
        let public = project(parse_output(Some(0), REPLY)).to_string();
        for private in ["private.example", "192.0.2.1", "tun-test"] {
            assert!(!public.contains(private));
        }
        assert!(public.len() < 300);
    }
    #[test]
    fn ping_rtt_matches_original_qml_awk_oracle() {
        use std::io::Write;
        let source = include_str!("../../../plugin/Service.qml");
        let oracle = "/^rtt|^round-trip/ {print $5; exit}";
        assert!(source.contains(oracle));
        for prefix in ["rtt min/avg/max/mdev", "round-trip min/avg/max/stddev"] {
            for number in ["0.000", "0.125", "12.500", "1999.900", "3000.000"] {
                let fixture = format!("{prefix} = {number}/{number}/{number}/0.000 ms\n");
                let mut child = Command::new("awk")
                    .args(["-F/", oracle])
                    .env("LC_ALL", "C")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap();
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(fixture.as_bytes())
                    .unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(output.status.success());
                let expected: f64 = std::str::from_utf8(&output.stdout)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                assert_eq!(
                    parse_output(Some(0), fixture.as_bytes()),
                    Sample::Reply(expected)
                );
            }
        }
    }
    #[test]
    fn ping_exec_fixed_arguments_and_no_fallback() {
        let fixture = Fixture::new(
            "[[ $LC_ALL = C && $# = 10 && $1 = -n && $2 = -q && $3 = -c && $4 = 1 && $5 = -W && $6 = 2 && $7 = -I && $8 = tun-test && $9 = -- && ${10} = 1.1.1.1 ]] || exit 2\nprintf 'rtt min/avg/max/mdev = 1/1/1/0 ms\\n'",
        );
        let binding = binding();
        assert_eq!(
            execute(
                &binding,
                "1.1.1.1",
                Instant::now() + DEADLINE,
                fixture.tool()
            ),
            Sample::Reply(1.0)
        );
        assert!(binding.slot.0.lock().unwrap().child.is_none());
        let refused = Fixture::new("printf 'private-key' >&2\nexit 2");
        assert_eq!(
            execute(
                &binding,
                "1.1.1.1",
                Instant::now() + DEADLINE,
                refused.tool()
            ),
            Sample::Unavailable
        );
        assert_eq!(
            execute(
                &binding,
                "1.1.1.1",
                Instant::now() + DEADLINE,
                "/no-such-fixed-ping"
            ),
            Sample::Unavailable
        );
    }
    #[test]
    fn ping_deadline_and_oversize_reap_child() {
        for script in ["exec sleep 10", "while true; do printf '%01000d' 0; done"] {
            let fixture = Fixture::new(script);
            let binding = binding();
            let start = Instant::now();
            assert_eq!(
                execute(
                    &binding,
                    "1.1.1.1",
                    start + Duration::from_millis(100),
                    fixture.tool()
                ),
                Sample::Unavailable
            );
            assert!(start.elapsed() < Duration::from_secs(1));
            assert!(binding.slot.0.lock().unwrap().child.is_none());
        }
    }
    #[test]
    fn ping_revoke_before_spawn_rejects_stale_ticket_without_child() {
        let binding = binding();
        assert!(binding.slot.revoke());
        let fixture = Fixture::new("exit 0");
        assert_eq!(
            execute(
                &binding,
                "1.1.1.1",
                Instant::now() + DEADLINE,
                fixture.tool()
            ),
            Sample::Unavailable
        );
        assert!(binding.slot.0.lock().unwrap().child.is_none());
    }
    #[test]
    fn ping_revoke_reaps_running_child_before_successor() {
        let fixture = Fixture::new("exec sleep 10");
        let binding = binding();
        let gate = Arc::clone(&binding.slot);
        let worker = std::thread::spawn(move || {
            execute(
                &binding,
                "1.1.1.1",
                Instant::now() + DEADLINE,
                fixture.tool(),
            )
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        let pid = loop {
            if let Some(child) = gate.0.lock().unwrap().child.as_ref() {
                break child.id();
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert!(gate.revoke());
        assert!(gate.0.lock().unwrap().child.is_none());
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        assert_eq!(worker.join().unwrap(), Sample::Unavailable);
        let next = Binding {
            device: "tun-test".into(),
            identity: "successor".into(),
            epoch: gate.epoch().unwrap(),
            slot: gate,
        };
        let success = Fixture::new("printf 'rtt min/avg/max/mdev = 1/1/1/0 ms\\n'");
        assert_eq!(
            execute(&next, "1.1.1.1", Instant::now() + DEADLINE, success.tool()),
            Sample::Reply(1.0)
        );
    }
}
