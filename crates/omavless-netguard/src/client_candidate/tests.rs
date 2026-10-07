use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

#[derive(Default)]
struct Counts {
    creates: Cell<usize>,
    connects: Cell<usize>,
    writes: Cell<usize>,
    waits: Cell<usize>,
    live: Cell<usize>,
    peak: Cell<usize>,
    socket_drops: Cell<usize>,
    endpoint_drops: Cell<usize>,
}
struct Socket {
    input: RefCell<VecDeque<u8>>,
    output: RefCell<Vec<u8>>,
    counts: Rc<Counts>,
}
impl Drop for Socket {
    fn drop(&mut self) {
        self.counts.live.set(self.counts.live.get() - 1);
        self.counts
            .socket_drops
            .set(self.counts.socket_drops.get() + 1);
    }
}
struct Endpoint(Rc<Counts>);
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.0.endpoint_drops.set(self.0.endpoint_drops.get() + 1);
    }
}
struct Mock {
    base: Instant,
    now_calls: Cell<usize>,
    late_tick: Option<usize>,
    final_read_tick: Cell<usize>,
    clock: Rc<Cell<u64>>,
    counts: Rc<Counts>,
    replies: VecDeque<Vec<u8>>,
    peers: VecDeque<Peer>,
    check_count: usize,
    fail_check: Option<usize>,
    late_check: Option<usize>,
    late_create: bool,
    late_final_read: bool,
    late_write: bool,
    fail_connect: bool,
    pending: bool,
    fail_wait: bool,
    panic_connect: bool,
    block_write: bool,
    block_read: bool,
    chunk: usize,
}
fn root_peer() -> Peer {
    Peer {
        pid: 71,
        uid: 0,
        gid: 0,
    }
}
fn status(protection: Protection) -> Response {
    Response::Status {
        policy_version: protocol::POLICY_VERSION,
        protection,
        health: Health::Verified,
    }
}
fn framed(response: Response) -> Vec<u8> {
    let body = protocol::encode_response(response).unwrap();
    let mut out = (body.len() as u32).to_be_bytes().to_vec();
    out.extend(body);
    out
}
fn disarmed() -> Response {
    status(Protection::Disarmed {
        closed_generation: Some(7),
    })
}

#[cfg(feature = "netguard-client-diagnostics")]
#[test]
fn closed_diagnostics_distinguish_transport_response_and_postgate_without_extra_io() {
    let mut backend = Mock::new(vec![framed(disarmed())]);
    backend.fail_connect = true;
    let counts = backend.counts.clone();
    let mut client = Client::new(backend);
    assert_eq!(client.exchange(Request::Status {}), Err(REFUSE));
    assert_eq!(crate::client_diagnostic::last_token(), "connect");
    assert_eq!(counts.connects.get(), 1);
    assert_eq!(counts.writes.get(), 0);
    drop(client); // existing Poisoned path retains original graph
    assert_eq!(counts.socket_drops.get(), 0);

    let backend = Mock::new(vec![vec![]]);
    let mut client = Client::new(backend);
    assert_eq!(client.exchange(Request::Status {}), Err(REFUSE));
    assert_eq!(crate::client_diagnostic::last_token(), "read_prefix");

    let response = Response::Status {
        policy_version: protocol::POLICY_VERSION,
        protection: Protection::Emergency {},
        health: Health::ManualRecoveryRequired,
    };
    let mut client = Client::new(Mock::new(vec![framed(response)]));
    assert_eq!(client.exchange(Request::Status {}), Err(REFUSE));
    assert_eq!(crate::client_diagnostic::last_token(), "positive_response");

    let mut client = Client::new(Mock::new(vec![framed(disarmed())]));
    assert_eq!(client.exchange(Request::Status {}), Ok(disarmed()));
    assert_eq!(crate::client_diagnostic::last_token(), "final_peer");
}
impl Mock {
    fn new(replies: Vec<Vec<u8>>) -> Self {
        Self {
            base: Instant::now(),
            now_calls: Cell::new(0),
            late_tick: None,
            final_read_tick: Cell::new(0),
            clock: Rc::default(),
            counts: Rc::default(),
            replies: replies.into(),
            peers: VecDeque::new(),
            check_count: 0,
            fail_check: None,
            late_check: None,
            late_create: false,
            late_final_read: false,
            late_write: false,
            fail_connect: false,
            pending: false,
            fail_wait: false,
            panic_connect: false,
            block_write: false,
            block_read: false,
            chunk: usize::MAX,
        }
    }
}
impl Backend for Mock {
    type Endpoint = Endpoint;
    type Socket = Socket;
    fn now(&self) -> Instant {
        self.now_calls.set(self.now_calls.get() + 1);
        if self.late_tick == Some(self.now_calls.get()) {
            self.clock.set(2001);
        }
        self.base + Duration::from_millis(self.clock.get())
    }
    fn admit(&mut self) -> Result<Endpoint> {
        Ok(Endpoint(self.counts.clone()))
    }
    fn recheck(&mut self, _: &Endpoint) -> Result<()> {
        self.check_count += 1;
        if self.late_check == Some(self.check_count) {
            self.clock.set(2001);
        }
        if self.fail_check == Some(self.check_count) {
            Err(REFUSE)
        } else {
            Ok(())
        }
    }
    fn create(&mut self) -> Result<Socket> {
        self.counts.creates.set(self.counts.creates.get() + 1);
        self.counts.live.set(self.counts.live.get() + 1);
        self.counts
            .peak
            .set(self.counts.peak.get().max(self.counts.live.get()));
        if self.late_create {
            self.clock.set(2001);
        }
        Ok(Socket {
            input: RefCell::new(self.replies.pop_front().unwrap().into()),
            output: RefCell::default(),
            counts: self.counts.clone(),
        })
    }
    fn connect(&mut self, _: &Socket) -> Result<Connect> {
        self.counts.connects.set(self.counts.connects.get() + 1);
        assert!(!self.panic_connect, "fixed mock cut");
        if self.fail_connect {
            Err(REFUSE)
        } else if self.pending {
            Ok(Connect::Pending)
        } else {
            Ok(Connect::Complete)
        }
    }
    fn connected(&mut self, _: &Socket) -> Result<()> {
        Ok(())
    }
    fn wait(&mut self, _: &Socket, _: Interest, _: Duration) -> Result<()> {
        self.counts.waits.set(self.counts.waits.get() + 1);
        if self.fail_wait { Err(REFUSE) } else { Ok(()) }
    }
    fn peer(&mut self, _: &Socket) -> Result<Peer> {
        Ok(self.peers.pop_front().unwrap_or_else(root_peer))
    }
    fn write(&mut self, s: &Socket, b: &[u8]) -> Result<Progress> {
        self.counts.writes.set(self.counts.writes.get() + 1);
        if self.block_write {
            self.block_write = false;
            return Ok(Progress::WouldBlock);
        }
        let n = b.len().min(self.chunk);
        s.output.borrow_mut().extend_from_slice(&b[..n]);
        if self.late_write {
            self.clock.set(2001);
        }
        Ok(Progress::Bytes(n))
    }
    fn read(&mut self, s: &Socket, b: &mut [u8]) -> Result<Progress> {
        if self.block_read {
            self.block_read = false;
            return Ok(Progress::WouldBlock);
        }
        let mut input = s.input.borrow_mut();
        let n = b.len().min(input.len()).min(self.chunk);
        for byte in b.iter_mut().take(n) {
            *byte = input.pop_front().unwrap();
        }
        if input.is_empty() {
            self.final_read_tick.set(self.now_calls.get());
        }
        if self.late_final_read && input.is_empty() {
            self.clock.set(2001);
        }
        Ok(Progress::Bytes(n))
    }
}
fn poisoned(mut client: Client<Mock>, request: Request) {
    let counts = client.original.as_ref().unwrap().backend.counts.clone();
    assert_eq!(client.exchange(request), Err(REFUSE));
    assert_eq!(client.phase, Phase::Poisoned);
    let created = counts.creates.get();
    assert_eq!(client.exchange(Request::Status {}), Err(REFUSE));
    assert_eq!(counts.creates.get(), created);
    let live = counts.live.get();
    drop(client);
    assert_eq!(counts.live.get(), live);
    assert_eq!(counts.socket_drops.get(), created - live);
}

#[test]
fn lazy_fixed_constructor_has_no_io_or_caller_selected_inputs() {
    assert_eq!(PATH, "/run/omavless-netguard/control.sock");
    assert_eq!(BUDGET, Duration::from_secs(2));
    let _never_used = FixedClient::new(); // no Linux path/socket call
    let m = Mock::new(vec![]);
    let counts = m.counts.clone();
    poisoned(
        Client::new(m),
        Request::Arm {
            generation: 8,
            mode: Mode::Full,
        },
    );
    assert_eq!(counts.creates.get(), 0);
}
#[test]
fn nonroot_wrong_group_and_changed_peer_are_refused_before_write_or_retirement() {
    for peer in [
        Peer {
            uid: 1000,
            ..root_peer()
        },
        Peer {
            gid: 995,
            ..root_peer()
        },
        Peer {
            pid: 0,
            ..root_peer()
        },
    ] {
        let mut m = Mock::new(vec![framed(disarmed())]);
        m.peers.push_back(peer);
        let counts = m.counts.clone();
        poisoned(Client::new(m), Request::Status {});
        assert_eq!(counts.writes.get(), 0);
    }
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.peers.extend([
        root_peer(),
        Peer {
            pid: 72,
            ..root_peer()
        },
    ]);
    poisoned(Client::new(m), Request::Status {});
}
#[test]
fn path_substitution_and_late_postgate_keep_the_original_socket() {
    for stage in [2, 3] {
        let mut m = Mock::new(vec![framed(disarmed())]);
        m.fail_check = Some(stage);
        poisoned(Client::new(m), Request::Status {});
    }
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.late_check = Some(3);
    poisoned(Client::new(m), Request::Status {});
}
#[test]
fn positive_late_create_connect_refusal_or_interrupted_wait_never_retries() {
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.late_create = true;
    let counts = m.counts.clone();
    poisoned(Client::new(m), Request::Status {});
    assert_eq!(counts.connects.get(), 0);
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.fail_connect = true;
    let counts = m.counts.clone();
    poisoned(Client::new(m), Request::Status {});
    assert_eq!(counts.connects.get(), 1);
    assert_eq!(counts.waits.get(), 0);
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.pending = true;
    m.fail_wait = true;
    poisoned(Client::new(m), Request::Status {});
}
#[test]
fn one_cursor_partial_write_read_and_wouldblock_share_the_original_budget() {
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.chunk = 2;
    m.block_write = true;
    m.block_read = true;
    m.pending = true;
    let counts = m.counts.clone();
    let mut c = Client::new(m);
    assert_eq!(c.exchange(Request::Status {}), Ok(disarmed()));
    assert_eq!(counts.connects.get(), 1);
    assert_eq!(counts.waits.get(), 3);
    let output = c
        .original
        .as_ref()
        .unwrap()
        .socket
        .as_ref()
        .unwrap()
        .output
        .borrow();
    let expected = protocol::encode_request(Request::Status {}).unwrap();
    assert_eq!(&output[..4], &(expected.len() as u32).to_be_bytes());
    assert_eq!(&output[4..], expected);
    drop(output);
    drop(c);
    assert_eq!(counts.live.get(), 0);
}
#[test]
fn final_positive_read_or_write_after_deadline_cannot_report_success() {
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.late_final_read = true;
    poisoned(Client::new(m), Request::Status {});
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.late_write = true;
    poisoned(Client::new(m), Request::Status {});
}
#[test]
fn malformed_old_oversized_truncated_error_and_manual_reply_poison_before_return() {
    let mut old = framed(disarmed());
    let text = String::from_utf8(old[4..].to_vec())
        .unwrap()
        .replace("\"version\":2", "\"version\":1");
    old = (text.len() as u32).to_be_bytes().to_vec();
    old.extend(text.as_bytes());
    let mut duplicate = Vec::new();
    let body = br#"{"version":2,"version":2,"payload":{"result":"error","code":"unavailable"}}"#;
    duplicate.extend((body.len() as u32).to_be_bytes());
    duplicate.extend(body);
    for wire in [
        vec![0, 0, 0, 0],
        (8193u32).to_be_bytes().to_vec(),
        vec![0, 0, 0, 8, b'{'],
        old,
        duplicate,
        framed(Response::Error {
            code: protocol::ErrorCode::ManualRecoveryRequired,
        }),
        framed(Response::Status {
            policy_version: protocol::POLICY_VERSION,
            protection: Protection::Disarmed {
                closed_generation: Some(7),
            },
            health: Health::ManualRecoveryRequired,
        }),
    ] {
        poisoned(Client::new(Mock::new(vec![wire])), Request::Status {});
    }
}
#[test]
fn healthy_distinct_operations_retire_only_completed_stream_and_bound_peak_at_one() {
    let m = Mock::new(vec![
        framed(disarmed()),
        framed(status(Protection::Armed { generation: 8 })),
        framed(status(Protection::Disarmed {
            closed_generation: Some(8),
        })),
    ]);
    let counts = m.counts.clone();
    let mut c = Client::new(m);
    c.exchange(Request::Status {}).unwrap();
    assert_eq!(counts.live.get(), 1);
    assert_eq!(counts.socket_drops.get(), 0);
    c.exchange(Request::Arm {
        generation: 8,
        mode: Mode::Full,
    })
    .unwrap();
    assert_eq!(counts.live.get(), 1);
    assert_eq!(counts.socket_drops.get(), 1);
    c.exchange(Request::Disarm { generation: 8 }).unwrap();
    assert_eq!(counts.live.get(), 1);
    assert_eq!(counts.socket_drops.get(), 2);
    assert_eq!(counts.peak.get(), 1);
    drop(c);
    assert_eq!(counts.socket_drops.get(), 3);
    assert_eq!(counts.endpoint_drops.get(), 1);
}
#[test]
fn arm_and_disarm_mismatch_are_rejected_by_client_not_just_runtime_wrapper() {
    for (request, reply) in [
        (
            Request::Arm {
                generation: 8,
                mode: Mode::Full,
            },
            status(Protection::Armed { generation: 9 }),
        ),
        (
            Request::Disarm { generation: 8 },
            status(Protection::Disarmed {
                closed_generation: None,
            }),
        ),
        (
            Request::Disarm { generation: 8 },
            status(Protection::Disarmed {
                closed_generation: Some(7),
            }),
        ),
    ] {
        let m = Mock::new(vec![framed(disarmed()), framed(reply)]);
        let mut c = Client::new(m);
        c.exchange(Request::Status {}).unwrap();
        poisoned(c, request);
    }
}
#[test]
fn later_peer_change_and_unexpected_panic_preserve_original_owners() {
    let mut m = Mock::new(vec![
        framed(disarmed()),
        framed(status(Protection::Armed { generation: 8 })),
    ]);
    m.peers.extend([
        root_peer(),
        root_peer(),
        Peer {
            pid: 72,
            ..root_peer()
        },
    ]);
    let mut c = Client::new(m);
    c.exchange(Request::Status {}).unwrap();
    poisoned(
        c,
        Request::Arm {
            generation: 8,
            mode: Mode::Full,
        },
    );
    let mut m = Mock::new(vec![framed(disarmed())]);
    m.panic_connect = true;
    let counts = m.counts.clone();
    let mut c = Client::new(m);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || c.exchange(Request::Status {})
        ))
        .is_err()
    );
    assert_eq!(c.phase, Phase::InFlight);
    drop(c);
    assert_eq!(counts.socket_drops.get(), 0);
}

#[test]
fn late_decode_or_final_success_gate_keeps_owned_socket_before_positive_projection() {
    let mut baseline = Client::new(Mock::new(vec![framed(disarmed())]));
    baseline.exchange(Request::Status {}).unwrap();
    let b = &baseline.original.as_ref().unwrap().backend;
    let post_decode_tick = b.final_read_tick.get() + 3;
    let final_tick = b.now_calls.get();
    for tick in [post_decode_tick, final_tick] {
        let mut m = Mock::new(vec![framed(disarmed())]);
        m.late_tick = Some(tick);
        poisoned(Client::new(m), Request::Status {});
    }
}
