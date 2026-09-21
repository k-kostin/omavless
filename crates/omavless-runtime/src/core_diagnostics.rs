// SPDX-License-Identifier: MIT

//! Bounded, in-memory classification of our child's stdout/stderr. Raw lines
//! never leave this module: no journal, file, IPC text, or credential redaction
//! by regex. Counts are hints from log text, NOT connection or health verdicts.

use serde::Serialize;
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_LINE: usize = 4096;
const DRAIN_READS: usize = 16;
const IDLE: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
enum Counter {
    Dns,
    Tls,
    Timeout,
    Connection,
    OtherWarning,
    Oversized,
}

#[derive(Default)]
struct Counts {
    values: [AtomicU32; 6],
    read_failed: AtomicBool,
    finished: AtomicBool,
    incomplete: AtomicBool,
}

impl Counts {
    fn increment(&self, counter: Counter) {
        let _ =
            self.values[counter as usize].fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.saturating_add(1))
            });
    }
}

/// Copyable only as fixed public categories/counts, with no raw log storage.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreDiagnostics {
    scope: &'static str,
    dns_errors: u32,
    tls_errors: u32,
    timeout_errors: u32,
    connection_errors: u32,
    other_warnings: u32,
    oversized_lines: u32,
    read_failed: bool,
    finished: bool,
    incomplete: bool,
}

#[derive(Clone, Default)]
pub(crate) struct DiagnosticReader(Arc<Counts>);

impl DiagnosticReader {
    pub(crate) fn snapshot(&self) -> CoreDiagnostics {
        // Acquire before reading counters so a finished snapshot includes the
        // worker's final drain. Live snapshots remain approximate samples.
        let finished = self.0.finished.load(Ordering::Acquire);
        let value = |kind: Counter| self.0.values[kind as usize].load(Ordering::Relaxed);
        CoreDiagnostics {
            scope: "latest_owned_core_log_counts",
            dns_errors: value(Counter::Dns),
            tls_errors: value(Counter::Tls),
            timeout_errors: value(Counter::Timeout),
            connection_errors: value(Counter::Connection),
            other_warnings: value(Counter::OtherWarning),
            oversized_lines: value(Counter::Oversized),
            read_failed: self.0.read_failed.load(Ordering::Relaxed),
            finished,
            incomplete: self.0.incomplete.load(Ordering::Relaxed),
        }
    }
}

// Deliberately not Debug/Serialize. Oversized lines are discarded in full,
// rather than treating their tail as another log record.
#[derive(Default)]
struct Lines {
    bytes: Vec<u8>,
    oversized: bool,
}

impl Lines {
    fn push(&mut self, bytes: &[u8], counts: &Counts) {
        for &byte in bytes {
            if byte == b'\n' {
                self.finish(counts);
            } else if !self.oversized {
                if self.bytes.len() == MAX_LINE {
                    self.bytes.fill(0);
                    self.bytes.clear();
                    self.oversized = true;
                } else {
                    self.bytes.push(byte.to_ascii_lowercase());
                }
            }
        }
    }

    fn finish(&mut self, counts: &Counts) {
        if self.oversized {
            counts.increment(Counter::Oversized);
        } else if let Some(category) = classify(&self.bytes) {
            counts.increment(category);
        }
        self.bytes.fill(0);
        self.bytes.clear();
        self.oversized = false;
    }
}

fn classify(line: &[u8]) -> Option<Counter> {
    let has = |token: &[u8]| line.windows(token.len()).any(|part| part == token);
    // Do not count a successful dial, a config echo or informational DNS line
    // as a failure. Unknown log formats remain unclassified, not healthy.
    if ![
        b"level=warning".as_slice(),
        b"level=error",
        b"level=fatal",
        b"[warn]",
        b"[error]",
        b"[fatal]",
    ]
    .iter()
    .any(|token| has(token))
    {
        return None;
    }
    Some(
        if [
            b"dns resolve failed".as_slice(),
            b"couldn't find ip",
            b"no such host",
            b"dns exchange failed",
        ]
        .iter()
        .any(|token| has(token))
        {
            Counter::Dns
        } else if [
            b"tls handshake".as_slice(),
            b"x509:",
            b"certificate verify",
            b"reality verification",
            b"reality handshake",
            b"tls:",
        ]
        .iter()
        .any(|token| has(token))
        {
            Counter::Tls
        } else if has(b"timeout") || has(b"timed out") || has(b"deadline exceeded") {
            Counter::Timeout
        } else if [
            b"connection refused".as_slice(),
            b"connection reset",
            b"network is unreachable",
            b"no route to host",
            b"unexpected eof",
        ]
        .iter()
        .any(|token| has(token))
        {
            Counter::Connection
        } else {
            Counter::OtherWarning
        },
    )
}

pub(crate) struct Capture {
    reader: DiagnosticReader,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Capture {
    /// Anonymous socket pair: no public /tmp path, listener or on-disk logs.
    /// Start the collector before spawning the child, so failure cannot leave
    /// a running unsupervised core behind.
    pub(crate) fn start() -> io::Result<(Self, UnixStream)> {
        let (mut input, output) = UnixStream::pair()?;
        input.set_nonblocking(true)?;
        let reader = DiagnosticReader::default();
        let counts = Arc::clone(&reader.0);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("core-log-counts".into())
            .spawn(move || {
                let mut lines = Lines::default();
                let mut buffer = [0_u8; MAX_LINE];
                loop {
                    let ending = stopping.load(Ordering::Acquire);
                    let mut empty = false;
                    for _ in 0..DRAIN_READS {
                        match input.read(&mut buffer) {
                            Ok(0) => {
                                lines.finish(&counts);
                                counts.finished.store(true, Ordering::Release);
                                return;
                            }
                            Ok(size) => {
                                lines.push(&buffer[..size], &counts);
                                buffer[..size].fill(0);
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                empty = true;
                                break;
                            }
                            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                            Err(_) => {
                                counts.read_failed.store(true, Ordering::Relaxed);
                                counts.incomplete.store(true, Ordering::Relaxed);
                                lines.finish(&counts);
                                counts.finished.store(true, Ordering::Release);
                                return;
                            }
                        }
                    }
                    if ending {
                        // At most 64 KiB final drain, even with an inherited open
                        // writer or a flooding child. Never block lifecycle stop.
                        lines.finish(&counts);
                        counts.incomplete.store(true, Ordering::Relaxed);
                        counts.finished.store(true, Ordering::Release);
                        return;
                    }
                    thread::sleep(if empty {
                        IDLE
                    } else {
                        Duration::from_millis(1)
                    });
                }
            })?;
        Ok((
            Self {
                reader,
                stop,
                worker: Some(worker),
            },
            output,
        ))
    }

    pub(crate) fn reader(&self) -> DiagnosticReader {
        self.reader.clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Instant;

    #[test]
    fn categories_are_bounded_private_and_not_a_health_claim() {
        let counts = Counts::default();
        let mut lines = Lines::default();
        for line in [
            "level=warning msg=DNS resolve failed: couldn't find IP https://private.invalid/credential\n",
            "level=error msg=TLS handshake error secret-profile\n",
            "level=warning msg=dial timed out 192.0.2.1\n",
            "level=warning msg=connection refused private-token\n",
            "level=error msg=something else private-token\n",
            "level=info msg=DNS resolve failed\n",
        ] {
            for chunk in line.as_bytes().chunks(7) {
                lines.push(chunk, &counts);
            }
        }
        let view = DiagnosticReader(Arc::new(counts));
        let value = serde_json::to_value(view.snapshot()).unwrap();
        for key in [
            "dnsErrors",
            "tlsErrors",
            "timeoutErrors",
            "connectionErrors",
            "otherWarnings",
        ] {
            assert_eq!(value[key], 1);
        }
        let encoded = value.to_string();
        for private in ["private", "credential", "192.0.2.1", "secret-profile"] {
            assert!(!encoded.contains(private));
        }
        assert!(encoded.len() < 512);
        assert!(value.get("healthy").is_none());
    }

    #[test]
    fn huge_invalid_utf8_and_unterminated_lines_never_grow_or_reframe() {
        let counts = Counts::default();
        let mut lines = Lines::default();
        for _ in 0..2048 {
            lines.push(&[0xff; 1024], &counts);
            assert!(lines.bytes.len() <= MAX_LINE);
        }
        lines.push(b"level=error msg=TLS handshake error\n", &counts);
        lines.push(b"level=warning msg=dial timed out", &counts);
        lines.finish(&counts);
        assert_eq!(
            counts.values[Counter::Oversized as usize].load(Ordering::Relaxed),
            1
        );
        assert_eq!(
            counts.values[Counter::Tls as usize].load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            counts.values[Counter::Timeout as usize].load(Ordering::Relaxed),
            1
        );
        counts.values[0].store(u32::MAX, Ordering::Relaxed);
        counts.increment(Counter::Dns);
        assert_eq!(counts.values[0].load(Ordering::Relaxed), u32::MAX);
    }

    #[test]
    fn capture_drains_and_drop_does_not_wait_for_inherited_writer() {
        let (capture, mut output) = Capture::start().unwrap();
        let reader = capture.reader();
        output
            .write_all(b"level=warning msg=DNS resolve failed private-token\n")
            .unwrap();
        let before = Instant::now();
        drop(capture); // writer intentionally still open
        assert!(before.elapsed() < Duration::from_secs(1));
        assert_eq!(reader.snapshot().dns_errors, 1);
        assert!(reader.snapshot().finished);
        assert!(reader.snapshot().incomplete);
    }

    #[test]
    fn sustained_output_is_drained_without_unbounded_storage() {
        let (capture, mut output) = Capture::start().unwrap();
        output
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let reader = capture.reader();
        let line = b"level=warning msg=dial timed out private-token\n";
        let chunk = line.repeat(1000);
        for _ in 0..20 {
            output.write_all(&chunk).unwrap();
        }
        drop(output);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !reader.snapshot().finished && Instant::now() < deadline {
            thread::sleep(IDLE);
        }
        assert_eq!(reader.snapshot().timeout_errors, 20000);
        assert!(reader.snapshot().finished);
        assert!(!reader.snapshot().incomplete);
        assert!(!reader.snapshot().read_failed);
    }

    #[test]
    fn eof_flushes_tail_without_journal_or_disk() {
        let (capture, mut output) = Capture::start().unwrap();
        let reader = capture.reader();
        output
            .write_all(b"level=error msg=connection reset")
            .unwrap();
        drop(output);
        let deadline = Instant::now() + Duration::from_secs(1);
        while !reader.snapshot().finished && Instant::now() < deadline {
            thread::sleep(IDLE);
        }
        assert!(reader.snapshot().finished);
        assert_eq!(reader.snapshot().connection_errors, 1);
    }
}
