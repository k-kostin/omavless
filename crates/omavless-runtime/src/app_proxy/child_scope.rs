// SPDX-License-Identifier: MIT

//! Explicit child-scope research only. No manager/desktop reads or writes,
//! no real core, no generic executable/path/endpoint input, no IPC registration.
//! A continuously held listener serves one fixed HTTP fixture. It is NOT
//! kernel peer authentication or a ready/owned Mihomo capability.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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
const REQUEST: &[u8] = b"GET http://fixture.invalid/child-scope HTTP/1.1\r\nHost: fixture.invalid\r\nConnection: close\r\n\r\n";
const RESPONSE: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nchild-scope-ok";
const LIMIT: usize = 4096;

fn child_receipt(output: &[u8], error: &[u8]) -> bool {
    if output.len() > LIMIT || !error.is_empty() {
        return false;
    }
    let Ok(text) = std::str::from_utf8(output) else {
        return false;
    };
    let lines: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
    if lines.len() != 3
        || lines[0] != "running 1 test"
        || lines[1] != format!("test {CONSUMER} ... ok")
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

struct OwnedFixtureProxy {
    listener: Option<TcpListener>,
    child: Option<Child>,
    end: Instant,
    attempted: bool,
    observed_request: bool,
    completion_unknown: bool,
}

impl OwnedFixtureProxy {
    fn bind() -> Result<Self> {
        let end = Instant::now() + Duration::from_secs(10);
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| Error::Unavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| Error::Unavailable)?;
        gate(end)?;
        Ok(Self {
            listener: Some(listener),
            child: None,
            end,
            attempted: false,
            observed_request: false,
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
        if proxy {
            // The original listener remains bound before, during and after
            // this descriptive endpoint lookup; no reserve/drop/rebind gap.
            let address = self
                .listener
                .as_ref()
                .ok_or(Error::Unavailable)?
                .local_addr()
                .map_err(|_| Error::Unavailable)?;
            if !address.ip().is_loopback() || address.port() == 0 {
                return Err(Error::Unavailable);
            }
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

    fn serve_one(&mut self) -> Result<()> {
        let mut stream = loop {
            gate(self.end)?;
            match self.listener.as_ref().ok_or(Error::Unavailable)?.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return Err(Error::Unavailable),
            }
        };
        socket_deadline(&stream, self.end)?;
        if read_header(&mut stream, self.end)? != REQUEST {
            return Err(Error::InvalidFrame);
        }
        self.observed_request = true;
        stream.write_all(RESPONSE).map_err(|_| Error::Unavailable)?;
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
        let result =
            if proxy { self.serve_one() } else { Ok(()) }.and_then(|()| self.settled_child());
        if !parent.unchanged() {
            return Err(Error::ParentChanged);
        }
        result
    }

    fn retain_unknown(&mut self) {
        // No reconstruction, follow-up wait or retry. This fixed experiment
        // has at most one child and one bound listener; retain their returned
        // originals until this dedicated test process exits, not across death.
        if let Some(child) = self.child.take() {
            std::mem::forget(child);
        }
        if let Some(listener) = self.listener.take() {
            std::mem::forget(listener);
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
        assert!(proxy.child.is_none());
        assert_eq!(proxy.run_once(true), Err(Error::Consumed));
        let mut baseline = OwnedFixtureProxy::bind().expect("independent fixture listener");
        baseline
            .run_once(false)
            .expect("independent baseline child");
        assert!(!baseline.observed_request);
        assert!(parent.unchanged(), "parent proxy environment changed");
    }

    #[test]
    #[ignore = "fixed child entry, only launched by the separately selected parent fixture"]
    fn fixed_consumer() {
        let mode = std::env::var(MODE).unwrap_or_else(|_| panic!("fixed child selector missing"));
        if mode == "baseline" {
            assert!(KEYS.into_iter().all(|key| std::env::var_os(key).is_none()));
            return;
        }
        assert!(mode == "http", "unknown fixed child selector");
        let endpoint =
            std::env::var("http_proxy").unwrap_or_else(|_| panic!("fixed child proxy absent"));
        let address = endpoint
            .strip_prefix("http://")
            .expect("fixed child scheme");
        let address: std::net::SocketAddr = address.parse().expect("fixed child endpoint");
        assert!(address.ip().is_loopback() && address.port() != 0);
        for key in ["HTTP_PROXY", "https_proxy", "HTTPS_PROXY"] {
            if std::env::var(key).ok().as_deref() != Some(endpoint.as_str()) {
                panic!("fixed child proxy fields disagree");
            }
        }
        let end = Instant::now() + Duration::from_secs(8);
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
            .expect("owned loopback proxy connection");
        socket_deadline(&stream, end).expect("owned consumer socket deadline");
        stream.write_all(REQUEST).expect("fixed consumer request");
        let mut response = Vec::new();
        stream
            .take((LIMIT + 1) as u64)
            .read_to_end(&mut response)
            .expect("fixed consumer response");
        assert!(response == RESPONSE, "fixed consumer response mismatch");
        gate(end).expect("fixed consumer deadline");
    }
}
