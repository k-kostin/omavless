// SPDX-License-Identifier: MIT
use super::*;
use crate::{RuntimeError, call_stream_with_delivery, write_client_request};
use nix::unistd::Uid;
use omavless_control_protocol::{
    FrameKind, decode_request, encode_request, encode_response, make_request, read_unary_frame,
    success_response, write_unary_frame,
};
use serde_json::json;
use std::cell::Cell;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

const RECEIPT: &str = "development.connection_sets.receipt";

struct Fixture(PathBuf);
impl Fixture {
    fn socket() -> (Self, RuntimePaths, UnixListener) {
        let base = crate::test_temp::directory_under(
            &crate::test_temp::home_parent().unwrap(),
            "reply-loss",
        )
        .unwrap();
        let paths = RuntimePaths::below(&base);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&paths.directory)
            .unwrap();
        let listener = UnixListener::bind(&paths.socket).unwrap();
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        (Self(base), paths, listener)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Called only after the bounded owned server thread has joined.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn accept(listener: &UnixListener) -> UnixStream {
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < end => {
                thread::sleep(Duration::from_millis(2));
            }
            _ => panic!("bounded owned socket accept failed"),
        }
    }
}

fn request(stream: &mut UnixStream) -> Value {
    read_unary_frame(stream, FrameKind::Request)
        .and_then(|bytes| decode_request(&bytes))
        .unwrap()
}

fn respond(stream: &mut UnixStream, request: &Value, result: Value) {
    let reply = success_response(request["id"].as_str().unwrap(), 8, result).unwrap();
    let bytes = encode_response(&reply).unwrap();
    write_unary_frame(stream, &bytes, FrameKind::Response).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
}

#[test]
fn owned_socket_discards_one_confirm_without_reading_and_resolves_original_receipt() {
    let (_fixture, paths, listener) = Fixture::socket();
    let operation = "a".repeat(64);
    let params = json!({"instanceId":"fixture-instance","operationId":operation,
        "ticket":"b".repeat(64),"handles":["c".repeat(64),"d".repeat(64)],"expectedRevision":7});
    let original = params.clone();
    thread::scope(|scope| {
        let peer = scope.spawn(|| {
            let mut first = accept(&listener);
            let confirm = request(&mut first);
            assert_eq!(confirm["method"], CONFIRM);
            assert_eq!(confirm["params"], original);
            // Keep the first socket open and send NO response. Receipt must
            // arrive independently, so a client which reads here cannot pass.
            let mut second = accept(&listener);
            let receipt = request(&mut second);
            assert_eq!(receipt["method"], RECEIPT);
            assert_eq!(receipt["params"], json!({"instanceId":"fixture-instance","operationId":operation}));
            respond(&mut second, &receipt, json!({"schemaVersion":1,
                "scope":"development_owned_connection_set_close", "instanceId":"fixture-instance",
                "operationId":operation,"state":"finished","receiptRevision":8,"aggregate":"all_closed",
                "members":[{"index":0,"outcome":"closed"},{"index":1,"outcome":"closed"}]}));
            assert_eq!(first.read(&mut [0_u8; 1]).unwrap(), 0);
        });
        let (mut cut, report) = FirstConfirmReplyLoss::arm();
        assert_eq!(
            cut.call(&paths, CONFIRM, params.clone()),
            Err(RuntimeError::Io)
        );
        assert_eq!(report.progress.0.load(Ordering::Acquire), DISCARDED);
        assert_eq!(
            cut.call(&paths, CONFIRM, params),
            Err(RuntimeError::Protocol)
        );
        let receipt = cut
            .call(
                &paths,
                RECEIPT,
                json!({"instanceId":"fixture-instance","operationId":operation}),
            )
            .unwrap();
        assert_eq!(receipt["result"]["operationId"], operation);
        assert_eq!(
            receipt["result"]["members"],
            json!([
            {"index":0,"outcome":"closed"},{"index":1,"outcome":"closed"}])
        );
        assert_eq!(report.progress.0.load(Ordering::Acquire), DISCARDED);
        peer.join().unwrap();
    });
}

#[test]
fn ordinary_and_unrelated_calls_read_responses_and_do_not_consume_cut() {
    let (_fixture, paths, listener) = Fixture::socket();
    let methods = [
        "status.get",
        "development.connection_sets.snapshot",
        "development.connection_sets.prepare",
        RECEIPT,
        "development.connections.confirm",
    ];
    thread::scope(|scope| {
        let peer = scope.spawn(|| {
            for expected in methods.into_iter().chain([CONFIRM]) {
                let mut stream = accept(&listener);
                let request = request(&mut stream);
                assert_eq!(request["method"], expected);
                respond(&mut stream, &request, json!({"normalResponse":true}));
            }
        });
        let (mut cut, report) = FirstConfirmReplyLoss::arm();
        for method in methods {
            assert_eq!(
                cut.call(&paths, method, json!({})).unwrap()["result"]["normalResponse"],
                true
            );
            assert_eq!(report.progress.0.load(Ordering::Acquire), ARMED);
        }
        assert_eq!(
            call(&paths, CONFIRM, json!({})).unwrap()["result"]["normalResponse"],
            true
        );
        peer.join().unwrap();
    });
}

#[test]
fn path_refusal_consumes_cut_before_connect_and_never_rearms() {
    let (_fixture, paths, listener) = Fixture::socket();
    fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o755)).unwrap();
    let (mut cut, report) = FirstConfirmReplyLoss::arm();
    assert_eq!(
        cut.call(&paths, CONFIRM, json!({})),
        Err(RuntimeError::UnsafeRuntimeDirectory)
    );
    assert_eq!(report.progress.0.load(Ordering::Acquire), ATTEMPTED);
    fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        cut.call(&paths, CONFIRM, json!({})),
        Err(RuntimeError::Protocol)
    );
    assert!(matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
}

#[test]
fn absent_socket_and_wrong_socket_mode_consume_without_sending_or_rearming() {
    for absent in [false, true] {
        let (_fixture, paths, listener) = Fixture::socket();
        if absent {
            fs::remove_file(&paths.socket).unwrap();
        } else {
            fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o666)).unwrap();
        }
        let (mut cut, report) = FirstConfirmReplyLoss::arm();
        assert!(cut.call(&paths, CONFIRM, json!({})).is_err());
        assert_eq!(report.progress.0.load(Ordering::Acquire), ATTEMPTED);
        assert_eq!(
            cut.call(&paths, CONFIRM, json!({})),
            Err(RuntimeError::Protocol)
        );
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
        );
    }
}

#[test]
fn wrong_peer_and_encoding_failure_never_write_private_bytes() {
    for wrong_peer in [true, false] {
        let (client, mut peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let (mut cut, report) = FirstConfirmReplyLoss::arm();
        let delivery = cut.delivery(CONFIRM).unwrap();
        let uid = Uid::current().as_raw();
        let params = if wrong_peer {
            json!({"private":"fixture-only"})
        } else {
            json!({"oversize":"x".repeat(omavless_control_protocol::MAX_REQUEST_FRAME_BYTES + 1)})
        };
        let result = call_stream_with_delivery(
            client,
            if wrong_peer { uid.wrapping_add(1) } else { uid },
            CONFIRM,
            params,
            Duration::from_secs(1),
            delivery,
        );
        assert_eq!(
            result,
            Err(if wrong_peer {
                RuntimeError::PermissionDenied
            } else {
                RuntimeError::Protocol
            })
        );
        assert_eq!(peer.read(&mut [0_u8; 1]).unwrap(), 0);
        assert_eq!(report.progress.0.load(Ordering::Acquire), ATTEMPTED);
        assert!(cut.delivery(CONFIRM).is_err());
    }
}

struct Writer {
    bytes: Vec<u8>,
    limit: usize,
    chunk: usize,
    failure: Option<io::ErrorKind>,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() >= self.limit {
            return self
                .failure
                .map_or(Ok(0), |kind| Err(io::Error::from(kind)));
        }
        let n = bytes
            .len()
            .min(self.chunk)
            .min(self.limit - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn frame() -> Vec<u8> {
    encode_request(
        &make_request(
            "fixture-request",
            CONFIRM,
            json!({"operationId":"a".repeat(64)}),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn complete_short_writes_and_successful_half_close_are_required_for_discarded() {
    let (mut cut, report) = FirstConfirmReplyLoss::arm();
    let delivery = cut.delivery(CONFIRM).unwrap();
    let bytes = frame();
    let mut writer = Writer {
        bytes: Vec::new(),
        limit: usize::MAX,
        chunk: 3,
        failure: None,
    };
    let half_closes = Cell::new(0);
    assert_eq!(
        write_client_request(
            &mut writer,
            &bytes,
            &delivery,
            Instant::now() + Duration::from_secs(1),
            |writer| {
                assert_eq!(writer.bytes, bytes);
                assert_eq!(report.progress.0.load(Ordering::Acquire), WRITTEN);
                half_closes.set(half_closes.get() + 1);
                Ok(())
            }
        ),
        Ok(true)
    );
    assert_eq!(half_closes.get(), 1);
    // The I/O leaf does not certify stream disposal. The actual transport
    // publishes DISCARDED only after dropping its original UnixStream.
    assert_eq!(report.progress.0.load(Ordering::Acquire), WRITTEN);
    assert!(cut.delivery(CONFIRM).is_err());
}

#[test]
fn incomplete_write_zero_error_and_timeout_never_claim_full_write_or_half_close() {
    for failure in [
        None,
        Some(io::ErrorKind::BrokenPipe),
        Some(io::ErrorKind::TimedOut),
    ] {
        for limit in [0, 7] {
            let (mut cut, report) = FirstConfirmReplyLoss::arm();
            let delivery = cut.delivery(CONFIRM).unwrap();
            let mut writer = Writer {
                bytes: Vec::new(),
                limit,
                chunk: 3,
                failure,
            };
            assert_eq!(
                write_client_request(
                    &mut writer,
                    &frame(),
                    &delivery,
                    Instant::now() + Duration::from_secs(1),
                    |_| panic!("half-close after incomplete frame")
                ),
                Err(RuntimeError::Io)
            );
            assert_eq!(writer.bytes.len(), limit);
            assert_eq!(report.progress.0.load(Ordering::Acquire), ATTEMPTED);
            assert!(cut.delivery(CONFIRM).is_err());
        }
    }
}

#[test]
fn half_close_failure_or_expired_deadline_is_written_but_not_reply_discarded() {
    for half_close_fails in [true, false] {
        let (mut cut, report) = FirstConfirmReplyLoss::arm();
        let delivery = cut.delivery(CONFIRM).unwrap();
        let mut writer = Writer {
            bytes: Vec::new(),
            limit: usize::MAX,
            chunk: 3,
            failure: None,
        };
        let end = if half_close_fails {
            Instant::now() + Duration::from_secs(1)
        } else {
            Instant::now() - Duration::from_secs(1)
        };
        assert_eq!(
            write_client_request(&mut writer, &frame(), &delivery, end, |_| {
                if half_close_fails {
                    Err(io::Error::from(io::ErrorKind::BrokenPipe))
                } else {
                    Ok(())
                }
            }),
            Err(RuntimeError::Io)
        );
        assert_eq!(writer.bytes, frame());
        assert_eq!(report.progress.0.load(Ordering::Acquire), WRITTEN);
        assert!(cut.delivery(CONFIRM).is_err());
    }
}

#[test]
fn malformed_frame_is_refused_before_write_and_diagnostic_is_closed_public_text() {
    let (mut cut, report) = FirstConfirmReplyLoss::arm();
    let delivery = cut.delivery(CONFIRM).unwrap();
    let mut writer = Writer {
        bytes: Vec::new(),
        limit: usize::MAX,
        chunk: 3,
        failure: None,
    };
    assert_eq!(
        write_client_request(
            &mut writer,
            b"not-json",
            &delivery,
            Instant::now() + Duration::from_secs(1),
            |_| panic!("half-close on invalid frame")
        ),
        Err(RuntimeError::Io)
    );
    assert!(writer.bytes.is_empty());
    for (stage, expected) in [
        (ARMED, "not_attempted full_request_written=false"),
        (ATTEMPTED, "attempted full_request_written=false"),
        (WRITTEN, "request_written full_request_written=true"),
        (DISCARDED, "reply_discarded full_request_written=true"),
    ] {
        report.progress.0.store(stage, Ordering::Release);
        assert_eq!(
            report.diagnostic(),
            format!("omavless.reply-loss.v1 stage={expected}")
        );
        assert!(report.diagnostic().len() <= 70);
    }
}
