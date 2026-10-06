// SPDX-License-Identifier: MIT

//! Explicit child-scope research only. No manager/desktop reads or writes,
//! no real core, no generic executable/path/endpoint input, no IPC registration.
//! Continuously held origin/proxy listeners serve one pinned HTTP client. It is NOT
//! kernel peer authentication or a ready/owned Mihomo capability.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const KEYS: [&str; 10] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "ftp_proxy",
    "FTP_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];
const CONSUMER: &str = "app_proxy::child_scope::tests::fixed_consumer";
const MODE: &str = "OMAVLESS_TEST_CHILD_PROXY_SCOPE";
const ORIGIN: &str = "OMAVLESS_TEST_CHILD_PROXY_ORIGIN";
const DIRECT_BODY: &[u8] = b"child-scope-direct";
const PROXY_BODY: &[u8] = b"child-scope-proxied";
const LIMIT: usize = 4096;

#[cfg(target_os = "linux")]
mod isolated_core;

fn fixture_address(listener: &TcpListener) -> Result<SocketAddr> {
    let address = listener.local_addr().map_err(|_| Error::Unavailable)?;
    if address.ip() != std::net::Ipv4Addr::LOCALHOST || address.port() == 0 {
        return Err(Error::Unavailable);
    }
    Ok(address)
}

// Header order/casing is incidental, but the method, target and complete
// allowlisted field set must describe exactly the one fixed request.
fn request_matches(raw: &[u8], address: SocketAddr, connect: bool) -> bool {
    let Ok(text) = std::str::from_utf8(raw) else {
        return false;
    };
    let Some(text) = text.strip_suffix("\r\n\r\n") else {
        return false;
    };
    let mut lines = text.split("\r\n");
    let first = if connect {
        format!("CONNECT {address} HTTP/1.1")
    } else {
        "GET /child-scope HTTP/1.1".to_owned()
    };
    if lines.next() != Some(first.as_str()) {
        return false;
    }
    let authority = address.to_string();
    let mut host = false;
    let mut policy = false;
    for line in lines {
        let Some((name, value)) = line.split_once(": ") else {
            return false;
        };
        if name.eq_ignore_ascii_case("host") && value == authority && !host {
            host = true;
        } else if !policy
            && ((connect && name.eq_ignore_ascii_case("proxy-connection") && value == "Keep-Alive")
                || (!connect && name.eq_ignore_ascii_case("connection") && value == "close"))
        {
            policy = true;
        } else {
            return false;
        }
    }
    host && policy
}

fn fixture_response(proxy: bool) -> Vec<u8> {
    let body = if proxy { PROXY_BODY } else { DIRECT_BODY };
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn child_receipt(output: &[u8], error: &[u8]) -> bool {
    fixed_case_receipt(output, error, CONSUMER)
}

fn fixed_case_receipt(output: &[u8], error: &[u8], selected: &str) -> bool {
    if output.len() > LIMIT || !error.is_empty() {
        return false;
    }
    let Ok(text) = std::str::from_utf8(output) else {
        return false;
    };
    let lines: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
    if lines.len() != 3
        || lines[0] != "running 1 test"
        || lines[1] != format!("test {selected} ... ok")
    {
        return false;
    }
    let Some(summary) =
        lines[2].strip_prefix("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ")
    else {
        return false;
    };
    let Some((filtered, elapsed)) = summary.split_once(" filtered out; finished in ") else {
        return false;
    };
    let Some(elapsed) = elapsed.strip_suffix('s') else {
        return false;
    };
    filtered.parse::<u32>().is_ok()
        && elapsed
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && (0.0..=10.0).contains(&value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Error {
    Unavailable,
    Expired,
    Consumed,
    InvalidFrame,
    ChildFailed,
    ParentChanged,
}

type Result<T> = std::result::Result<T, Error>;

// Never Debug/serialize a captured value; prior proxy variables may be secrets.
struct ParentProxyEnvironment([Option<OsString>; 10]);

impl ParentProxyEnvironment {
    fn capture() -> Self {
        Self(KEYS.map(std::env::var_os))
    }

    fn unchanged(&self) -> bool {
        self.0 == Self::capture().0
    }
}

fn gate(end: Instant) -> Result<()> {
    if Instant::now() < end {
        Ok(())
    } else {
        Err(Error::Expired)
    }
}

fn socket_deadline(stream: &TcpStream, end: Instant) -> Result<()> {
    gate(end)?;
    let remaining = end.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Expired);
    }
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|_| Error::Unavailable)?;
    stream
        .set_write_timeout(Some(remaining))
        .map_err(|_| Error::Unavailable)?;
    gate(end)
}

fn read_header(stream: &mut TcpStream, end: Instant) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    loop {
        gate(end)?;
        if raw.len() >= LIMIT {
            return Err(Error::InvalidFrame);
        }
        socket_deadline(stream, end)?;
        let mut byte = [0];
        if stream.read(&mut byte).map_err(|_| Error::Unavailable)? != 1 {
            return Err(Error::InvalidFrame);
        }
        gate(end)?;
        raw.push(byte[0]);
        if raw.ends_with(b"\r\n\r\n") {
            return Ok(raw);
        }
    }
}

fn read_response(stream: &mut TcpStream, end: Instant) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    loop {
        socket_deadline(stream, end)?;
        let mut byte = [0];
        let length = stream.read(&mut byte).map_err(|_| Error::Unavailable)?;
        gate(end)?;
        if length == 0 {
            return Ok(raw);
        }
        if raw.len() >= LIMIT {
            return Err(Error::InvalidFrame);
        }
        raw.push(byte[0]);
    }
}

struct OwnedFixtureProxy {
    listener: Option<TcpListener>,
    origin: Option<TcpListener>,
    child: Option<Child>,
    end: Instant,
    attempted: bool,
    observed_request: bool,
    observed_origin: bool,
    completion_unknown: bool,
}

impl OwnedFixtureProxy {
    fn bind() -> Result<Self> {
        let end = Instant::now() + Duration::from_secs(10);
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| Error::Unavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| Error::Unavailable)?;
        let origin = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| Error::Unavailable)?;
        origin
            .set_nonblocking(true)
            .map_err(|_| Error::Unavailable)?;
        gate(end)?;
        Ok(Self {
            listener: Some(listener),
            origin: Some(origin),
            child: None,
            end,
            attempted: false,
            observed_request: false,
            observed_origin: false,
            completion_unknown: false,
        })
    }

    fn command(&self, proxy: bool) -> Result<Command> {
        gate(self.end)?;
        let mut command = Command::new(std::env::current_exe().map_err(|_| Error::Unavailable)?);
        command
            .args(["--exact", CONSUMER, "--ignored", "--test-threads=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Every edit belongs to Command's child environment, never std::env.
        for key in KEYS {
            command.env_remove(key);
        }
        command.env(MODE, if proxy { "http" } else { "baseline" });
        let origin = fixture_address(self.origin.as_ref().ok_or(Error::Unavailable)?)?;
        command.env(ORIGIN, origin.to_string());
        if proxy {
            // The original listener remains bound before, during and after
            // this descriptive endpoint lookup; no reserve/drop/rebind gap.
            let address = fixture_address(self.listener.as_ref().ok_or(Error::Unavailable)?)?;
            let endpoint = format!("http://{address}");
            command
                .env("http_proxy", &endpoint)
                .env("HTTP_PROXY", &endpoint)
                .env("https_proxy", &endpoint)
                .env("HTTPS_PROXY", &endpoint)
                .env("no_proxy", "")
                .env("NO_PROXY", "");
        }
        gate(self.end)?;
        Ok(command)
    }

    fn accept(&self, listener: &TcpListener) -> Result<TcpStream> {
        let stream = loop {
            gate(self.end)?;
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return Err(Error::Unavailable),
            }
        };
        socket_deadline(&stream, self.end)?;
        gate(self.end)?;
        Ok(stream)
    }

    fn serve_origin(&mut self, proxy: bool) -> Result<()> {
        let listener = self.origin.as_ref().ok_or(Error::Unavailable)?;
        let address = fixture_address(listener)?;
        let mut stream = self.accept(listener)?;
        if !request_matches(&read_header(&mut stream, self.end)?, address, false) {
            return Err(Error::InvalidFrame);
        }
        self.observed_origin = true;
        socket_deadline(&stream, self.end)?;
        stream
            .write_all(&fixture_response(proxy))
            .map_err(|_| Error::Unavailable)?;
        gate(self.end)
    }

    fn serve_one(&mut self, proxy: bool) -> Result<()> {
        if !proxy {
            return self.serve_origin(false);
        }
        let origin = fixture_address(self.origin.as_ref().ok_or(Error::Unavailable)?)?;
        let mut child = self.accept(self.listener.as_ref().ok_or(Error::Unavailable)?)?;
        // Pinned ureq 3.4.0 uses CONNECT even for plain HTTP. A successful
        // CONNECT alone is insufficient: observe the actual GET in its tunnel.
        if !request_matches(&read_header(&mut child, self.end)?, origin, true) {
            return Err(Error::InvalidFrame);
        }
        socket_deadline(&child, self.end)?;
        child
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .map_err(|_| Error::Unavailable)?;
        gate(self.end)?;
        let request = read_header(&mut child, self.end)?;
        if !request_matches(&request, origin, false) {
            return Err(Error::InvalidFrame);
        }
        self.observed_request = true;
        // The proxy forwards only that validated request to its own still-held
        // origin; CONNECT cannot select another destination or become a relay API.
        let remaining = self.end.saturating_duration_since(Instant::now());
        gate(self.end)?;
        let mut upstream =
            TcpStream::connect_timeout(&origin, remaining.min(Duration::from_secs(2)))
                .map_err(|_| Error::Unavailable)?;
        socket_deadline(&upstream, self.end)?;
        upstream
            .write_all(&request)
            .map_err(|_| Error::Unavailable)?;
        self.serve_origin(true)?;
        let response = read_response(&mut upstream, self.end)?;
        if response != fixture_response(true) {
            return Err(Error::InvalidFrame);
        }
        socket_deadline(&child, self.end)?;
        child.write_all(&response).map_err(|_| Error::Unavailable)?;
        gate(self.end)
    }

    fn no_extra_connections(&self) -> Result<()> {
        for listener in [&self.listener, &self.origin] {
            match listener.as_ref().ok_or(Error::Unavailable)?.accept() {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                _ => return Err(Error::InvalidFrame),
            }
        }
        gate(self.end)
    }

    fn settled_child(&mut self) -> Result<()> {
        loop {
            gate(self.end)?;
            let child = self.child.as_mut().ok_or(Error::Unavailable)?;
            let status = match child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    self.completion_unknown = true;
                    return Err(Error::Unavailable);
                }
            };
            match status {
                Some(status) => {
                    if !status.success() {
                        return Err(Error::ChildFailed);
                    }
                    let mut output = Vec::new();
                    child
                        .stdout
                        .take()
                        .ok_or(Error::Unavailable)?
                        .take((LIMIT + 1) as u64)
                        .read_to_end(&mut output)
                        .map_err(|_| Error::Unavailable)?;
                    let mut error = Vec::new();
                    child
                        .stderr
                        .take()
                        .ok_or(Error::Unavailable)?
                        .take((LIMIT + 1) as u64)
                        .read_to_end(&mut error)
                        .map_err(|_| Error::Unavailable)?;
                    // This fixed consumer never spawns descendants or prints
                    // environment/private values. Require one real case, not
                    // an empty test selector or an unbounded output capture.
                    if !child_receipt(&output, &error) {
                        return Err(Error::ChildFailed);
                    }
                    gate(self.end)?;
                    self.child.take();
                    return Ok(());
                }
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn run_once(&mut self, proxy: bool) -> Result<()> {
        if self.attempted {
            return Err(Error::Consumed);
        }
        self.attempted = true;
        gate(self.end)?;
        let parent = ParentProxyEnvironment::capture();
        let child = self
            .command(proxy)?
            .spawn()
            .map_err(|_| Error::Unavailable)?;
        self.child = Some(child);
        gate(self.end)?;
        let result = self
            .serve_one(proxy)
            .and_then(|()| self.settled_child())
            .and_then(|()| self.no_extra_connections());
        if !parent.unchanged() {
            return Err(Error::ParentChanged);
        }
        result
    }

    fn retain_unknown(&mut self) {
        // No reconstruction, follow-up wait or retry. This fixed experiment
        // has at most one child and two bound listeners; retain their returned
        // originals until this dedicated test process exits, not across death.
        if let Some(child) = self.child.take() {
            std::mem::forget(child);
        }
        if let Some(listener) = self.listener.take() {
            std::mem::forget(listener);
        }
        if let Some(origin) = self.origin.take() {
            std::mem::forget(origin);
        }
    }
}

impl Drop for OwnedFixtureProxy {
    fn drop(&mut self) {
        // Scope is only an exclusively owned, fixed read-only fixture child.
        // No arbitrary process lookup, descendants or product recovery.
        if self.completion_unknown {
            self.retain_unknown();
            return;
        }
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if child.kill().is_err() || child.wait().is_err() {
                        self.retain_unknown();
                    }
                }
                Err(_) => self.retain_unknown(),
            }
        }
        // listener drops only after the fixed child completion/cleanup attempt.
        // Failure here is not proof of successful product rollback.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_headers_admit_only_fixed_origin_and_connect() {
        let address: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let get = "GET /child-scope HTTP/1.1\r\nhost: 127.0.0.1:12345\r\nconnection: close\r\n\r\n";
        let connect = "CONNECT 127.0.0.1:12345 HTTP/1.1\r\nHost: 127.0.0.1:12345\r\nProxy-Connection: Keep-Alive\r\n\r\n";
        assert!(request_matches(get.as_bytes(), address, false));
        assert!(request_matches(connect.as_bytes(), address, true));
        assert!(!request_matches(get.as_bytes(), address, true));
        assert!(!request_matches(connect.as_bytes(), address, false));
        for wrong in [
            get.replace("GET ", "POST "),
            get.replace("/child-scope", "/other"),
            get.replace("127.0.0.1:12345", "127.0.0.1:12346"),
            get.replace("host: ", "host: 127.0.0.1:12345\r\nhost: "),
            get.replace(
                "connection: close",
                "connection: close\r\nauthorization: synthetic",
            ),
            get.replace("\r\n", "\n"),
            format!("{get}extra"),
        ] {
            assert!(!request_matches(wrong.as_bytes(), address, false));
        }
        assert!(!request_matches(&[255], address, false));
        assert_ne!(fixture_response(false), fixture_response(true));
    }

    #[test]
    fn child_receipt_requires_exact_one_case_and_never_echoes_private_bytes() {
        let good = format!(
            "\nrunning 1 test\ntest {CONSUMER} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 999 filtered out; finished in 0.01s\n\n"
        );
        assert!(child_receipt(good.as_bytes(), b""));
        for wrong in [
            good.replace("running 1 test", "running 0 tests"),
            good.replace("1 passed", "0 passed"),
            good.replace("0 failed", "1 failed"),
            good.replace("0.01s", "NaNs"),
            good.replace("0.01s", "11s"),
            format!("private-value\n{good}"),
            format!("{good}{good}"),
            good.replace(CONSUMER, "other::case"),
        ] {
            assert!(!child_receipt(wrong.as_bytes(), b""));
        }
        assert!(!child_receipt(good.as_bytes(), b"private-value"));
        assert!(!child_receipt(&[255], b""));
    }

    #[test]
    fn command_overrides_are_child_only_and_do_not_inherit_proxy_secrets() {
        let parent = ParentProxyEnvironment::capture();
        let scope = OwnedFixtureProxy::bind().expect("owned fixture listener");
        let command = scope.command(true).expect("fixed command");
        let rows: Vec<_> = command.get_envs().collect();
        for key in ["ftp_proxy", "FTP_PROXY", "all_proxy", "ALL_PROXY"] {
            if !rows
                .iter()
                .any(|(name, value)| *name == key && value.is_none())
            {
                panic!("fixed inherited proxy removal missing");
            }
        }
        for key in ["no_proxy", "NO_PROXY"] {
            if !rows
                .iter()
                .any(|(name, value)| *name == key && value.is_some_and(|value| value.is_empty()))
            {
                panic!("fixed child bypass policy missing");
            }
        }
        assert!(parent.unchanged(), "parent proxy environment changed");
    }

    #[test]
    fn consumed_or_expired_scope_never_launches_a_child() {
        let mut scope = OwnedFixtureProxy::bind().expect("owned fixture listener");
        scope.attempted = true;
        assert_eq!(scope.run_once(true), Err(Error::Consumed));
        assert!(scope.child.is_none());
        scope.attempted = false;
        scope.end = Instant::now();
        assert_eq!(scope.run_once(true), Err(Error::Expired));
        assert!(scope.child.is_none());
        assert_eq!(scope.run_once(true), Err(Error::Consumed));
    }

    #[test]
    #[ignore = "ROOT-selected owned loopback/child fixture only; no VM or global settings"]
    fn child_scope_http_and_baseline_exchange() {
        let parent = ParentProxyEnvironment::capture();
        let mut proxy = OwnedFixtureProxy::bind().expect("owned fixture listener");
        proxy
            .run_once(true)
            .expect("bounded original proxy exchange");
        assert!(proxy.observed_request);
        assert!(proxy.observed_origin);
        assert!(proxy.child.is_none());
        assert_eq!(proxy.run_once(true), Err(Error::Consumed));
        let mut baseline = OwnedFixtureProxy::bind().expect("independent fixture listener");
        baseline
            .run_once(false)
            .expect("independent baseline child");
        assert!(!baseline.observed_request);
        assert!(baseline.observed_origin);
        assert!(parent.unchanged(), "parent proxy environment changed");
    }

    #[test]
    #[ignore = "fixed child entry, only launched by the separately selected parent fixture"]
    pub(super) fn fixed_consumer() {
        let mode = std::env::var(MODE).unwrap_or_else(|_| panic!("fixed child selector missing"));
        if mode == "baseline" {
            assert!(KEYS.into_iter().all(|key| std::env::var_os(key).is_none()));
        } else {
            assert!(mode == "http", "unknown fixed child selector");
        }
        let origin = std::env::var(ORIGIN).expect("fixed child origin absent");
        let address: SocketAddr = origin.parse().expect("fixed child origin");
        assert!(address.ip() == std::net::Ipv4Addr::LOCALHOST && address.port() != 0);
        assert!(
            origin == address.to_string(),
            "noncanonical fixed child origin"
        );
        let end = Instant::now() + Duration::from_secs(8);
        // Config::default discovers proxies from the CHILD environment itself.
        // No manual proxy read, explicit Proxy value or custom connector exists.
        // Production subscription transport's proxy(None) remains unchanged.
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .timeout_connect(Some(Duration::from_secs(2)))
            .max_redirects(0)
            .max_response_header_size(LIMIT)
            .user_agent("")
            .accept("")
            .accept_encoding("")
            .build()
            .new_agent();
        let mut response = agent
            .get(format!("http://{address}/child-scope"))
            .header("Connection", "close")
            .call()
            .unwrap_or_else(|_| panic!("fixed client request failed"));
        assert!(response.status() == 200, "fixed client status mismatch");
        let body = response
            .body_mut()
            .with_config()
            .limit(LIMIT as u64)
            .read_to_vec()
            .unwrap_or_else(|_| panic!("fixed client body failed"));
        let expected = if mode == "http" {
            PROXY_BODY
        } else {
            DIRECT_BODY
        };
        assert!(body == expected, "fixed client route body mismatch");
        gate(end).expect("fixed consumer deadline");
    }
}
