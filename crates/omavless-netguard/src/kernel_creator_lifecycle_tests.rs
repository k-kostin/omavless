use super::*;
use crate::enrollment::EnrollmentBinding;
use crate::locked_state::{ExchangeError, LockedState};
use crate::protocol::{
    ErrorCode, Health, Mode, Protection, Request, Response, decode_response, encode_request,
};
use crate::receipt::NamespaceObservation;
use crate::root_state::RootStateStore;
use crate::session_owner_candidate::{SessionOwner, SessionProgress};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

const CHILD: &str = "kernel_observer::creator_lifecycle::tests::lifecycle_child";
const WRITER: &str = "kernel_observer::creator_lifecycle::tests::lifecycle_crash_writer";
const ARM: Request = Request::Arm {
    generation: 7,
    mode: Mode::Full,
};
const DISARM: Request = Request::Disarm { generation: 7 };
const REFUSED: ErrorCode = ErrorCode::ManualRecoveryRequired;

fn ack(request: &[u8], port: u32, code: i32) -> Vec<u8> {
    message(
        2,
        0,
        u32n(&request[8..12]).unwrap(),
        port,
        &[
            code.to_ne_bytes().to_vec(),
            if code == 0 {
                request[..16].to_vec()
            } else {
                request.to_vec()
            },
        ]
        .concat(),
    )
}
#[test]
fn replacement_is_one_generation_fenced_delete_and_complete_exclusive_full_create() {
    let requests = full_batch(7, 10, Some(9)).unwrap();
    assert_eq!(requests.len(), 15);
    assert_eq!(u16n(&requests[0][4..6]).unwrap(), 16);
    assert_eq!(&requests[0][24..28], &7_u32.to_be_bytes());
    assert_eq!(u16n(&requests[1][4..6]).unwrap(), NFT + 2);
    assert_eq!(&requests[1][24..32], &9_u64.to_be_bytes());
    assert_eq!(u16n(&requests[2][4..6]).unwrap(), NFT);
    assert_eq!(u16n(&requests[2][6..8]).unwrap(), 0x605);
    assert_eq!(u16n(&requests[14][4..6]).unwrap(), 17);
    for (i, request) in requests.iter().enumerate() {
        assert_eq!(u32n(&request[8..12]).unwrap(), 10 + i as u32);
        assert_ne!(u16n(&request[6..8]).unwrap() & 4, 0);
    }
    for (genid, sequence, old) in [
        (0, 1, None),
        (1, 0, None),
        (1, u32::MAX - 14, None),
        (1, 1, Some(0)),
    ] {
        assert!(full_batch(genid, sequence, old).is_err());
    }
}
#[test]
fn all_operation_success_without_commit_end_or_with_late_error_never_succeeds() {
    for old in [None, Some(9)] {
        let requests = full_batch(7, 10, old).unwrap();
        let mut replies = BatchReplies::new(requests.clone(), 42).unwrap();
        let operation_acks: Vec<_> = requests[..requests.len() - 1]
            .iter()
            .flat_map(|r| ack(r, 42, 0))
            .collect();
        replies
            .receive(
                &operation_acks,
                Some(NetlinkAddr::new(0, 0)),
                MsgFlags::empty(),
            )
            .unwrap();
        assert!(!replies.complete());
        replies
            .receive(
                &ack(&requests[0], 42, -22),
                Some(NetlinkAddr::new(0, 0)),
                MsgFlags::empty(),
            )
            .unwrap_err();
        assert!(!replies.complete());
        replies
            .receive(
                &ack(requests.last().unwrap(), 42, 0),
                Some(NetlinkAddr::new(0, 0)),
                MsgFlags::empty(),
            )
            .unwrap_err();
    }
}
#[test]
fn collector_refuses_forged_duplicate_truncated_wrong_sender_and_generation_error() {
    let requests = full_batch(7, 10, Some(9)).unwrap();
    let a = ack(&requests[0], 42, 0);
    for length in 0..a.len() {
        let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
        assert!(
            c.receive(
                &a[..length],
                Some(NetlinkAddr::new(0, 0)),
                MsgFlags::empty()
            )
            .is_err()
        );
    }
    for offset in [0, 4, 6, 8, 12, 16, 20, 24, 28, 32] {
        let mut bad = a.clone();
        bad[offset] ^= 1;
        let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
        assert!(
            c.receive(&bad, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
                .is_err()
        );
    }
    let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
    c.receive(&a, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
        .unwrap();
    assert!(
        c.receive(&a, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
            .is_err()
    );
    let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
    assert!(
        c.receive(&a, Some(NetlinkAddr::new(1, 0)), MsgFlags::empty())
            .is_err()
    );
    let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
    c.receive(
        &ack(&requests[0], 42, -85),
        Some(NetlinkAddr::new(0, 0)),
        MsgFlags::empty(),
    )
    .unwrap();
    assert!(c.complete() && c.changed);
    let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
    assert!(
        c.receive(
            &ack(&requests[1], 42, -85),
            Some(NetlinkAddr::new(0, 0)),
            MsgFlags::empty()
        )
        .is_err()
    );
    let mut c = BatchReplies::new(requests.clone(), 42).unwrap();
    for request in requests.iter().rev() {
        c.receive(
            &ack(request, 42, 0),
            Some(NetlinkAddr::new(0, 0)),
            MsgFlags::empty(),
        )
        .unwrap();
    }
    assert!(c.complete());
}

#[test]
fn receive_truncation_flags_poison_create_and_replace_collectors_permanently() {
    for old in [None, Some(9)] {
        for flags in [
            MsgFlags::MSG_TRUNC,
            MsgFlags::MSG_CTRUNC,
            MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC,
        ] {
            let requests = full_batch(7, 10, old).unwrap();
            let bytes = ack(&requests[0], 42, 0);
            let mut replies = BatchReplies::new(requests.clone(), 42).unwrap();
            assert!(
                replies
                    .receive(&bytes, Some(NetlinkAddr::new(0, 0)), flags)
                    .is_err()
            );
            assert!(replies.poisoned && !replies.complete());
            assert!(replies.acks.iter().all(|ack| !ack));
            for request in &requests {
                assert!(
                    replies
                        .receive(
                            &ack(request, 42, 0),
                            Some(NetlinkAddr::new(0, 0)),
                            MsgFlags::empty()
                        )
                        .is_err()
                );
            }
            assert!(!replies.complete());
        }
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("k1lc-{}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join("omavless-netguard"))
            .unwrap();
        Self(path)
    }
    fn state(&self, enrolled: bool) -> LockedState {
        let metadata = std::fs::metadata(&self.0).unwrap();
        let owner = (metadata.uid(), metadata.gid());
        let mut state = LockedState::from_root(
            RootStateStore::open_test_parent(File::open(&self.0).unwrap(), owner, 1001).unwrap(),
        );
        if enrolled {
            let path = self.0.join("omavless-netguard/enrollment-v1.json");
            if !path.exists() {
                std::fs::write(&path, b"{\"version\":1,\"enrolled_uid\":1001}").unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
            state.bind_fixture_enrollment(
                EnrollmentBinding::open_test_parent(File::open(&self.0).unwrap(), owner).unwrap(),
            );
        }
        state
    }
    fn records(&self) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let dir = self.0.join("omavless-netguard");
        (
            std::fs::read(dir.join("armed-v1.json")).ok(),
            std::fs::read(dir.join("table-receipt-v1.json")).ok(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn armed(response: Response) {
    assert!(matches!(
        response,
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            health: Health::Verified,
            ..
        }
    ));
}
fn disarmed(response: Response) {
    assert!(matches!(
        response,
        Response::Status {
            protection: Protection::Disarmed {},
            health: Health::Verified,
            ..
        }
    ));
}
fn socket_request(
    path: &PathBuf,
    owner: &mut SessionOwner<FixtureCreator>,
    request: Request,
) -> Response {
    let mut client = UnixStream::connect(path).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let request = encode_request(request).unwrap();
    client
        .write_all(&(request.len() as u32).to_be_bytes())
        .unwrap();
    client.write_all(&request).unwrap();
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    let mut prefix = [0; 4];
    client.read_exact(&mut prefix).unwrap();
    let length = u32::from_be_bytes(prefix) as usize;
    assert!(length <= crate::protocol::MAX_FRAME_BYTES);
    let mut frame = vec![0; length];
    client.read_exact(&mut frame).unwrap();
    decode_response(&frame).unwrap()
}

#[test]
#[ignore = "VM-only actual creator/private-writer/service composition in disposable namespaces"]
fn creator_lifecycle_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_LIFECYCLE_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&[
        "lifecycle",
        "service",
        "lost-create",
        "lost-replace",
        "lost-delete",
        "orphan",
        "drift",
        "stale-create",
        "stale-replace",
        "lost-socket-reply",
        "crash-create",
        "crash-replace",
        "crash-delete",
    ]);
    println!("K1_CREATOR_LIFECYCLE_VM_PASS");
}
#[test]
#[ignore = "VM-only actual recvmsg MSG_TRUNC after fixed create/replace/delete sends"]
fn real_receive_truncation_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_RECV_TRUNC_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&["trunc-create", "trunc-replace", "trunc-delete"]);
    println!("K1_REAL_RECEIVE_TRUNCATION_VM_PASS");
}
#[test]
#[ignore = "VM-only actual END ACK observer loss after unchanged BEGIN/operation delivery"]
fn real_end_ack_observer_loss_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_END_ACK_LOSS_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&["end-create", "end-replace", "end-delete"]);
    println!("K1_REAL_END_ACK_OBSERVER_LOSS_VM_PASS");
}
fn isolated_cases(cases: &[&str]) {
    let parent = namespace_file().unwrap();
    let identity = namespace_identity(&parent).unwrap();
    for case in cases {
        let mut guard = ChildGuard(
            Command::new("/usr/bin/unshare")
                .args([
                    "--user",
                    "--map-user=1001",
                    "--map-group=1001",
                    "--keep-caps",
                    "--net",
                    "--",
                ])
                .arg(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", CHILD, "--nocapture"])
                .env_clear()
                .env("OMAVLESS_K1_LIFECYCLE_CHILD", case)
                .env("TMPDIR", std::env::temp_dir())
                .stdin(Stdio::from(parent.try_clone().unwrap()))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = guard.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "isolated lifecycle deadline");
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut output = Vec::new();
        guard
            .0
            .stdout
            .take()
            .unwrap()
            .take(32769)
            .read_to_end(&mut output)
            .unwrap();
        let mut error = Vec::new();
        guard
            .0
            .stderr
            .take()
            .unwrap()
            .take(32769)
            .read_to_end(&mut error)
            .unwrap();
        assert!(output.len() <= 32768 && error.len() <= 32768);
        assert!(
            status.success(),
            "isolated lifecycle case {case} refused: {}",
            String::from_utf8_lossy(&error)
        );
        assert_eq!(
            String::from_utf8_lossy(&output)
                .matches("K1_CREATOR_LIFECYCLE_CHILD_PASS")
                .count(),
            1
        );
        if case.starts_with("trunc-") {
            assert_eq!(
                String::from_utf8_lossy(&output)
                    .matches(&format!("K1_ACTUAL_MSG_TRUNC_{case}_PASS"))
                    .count(),
                1
            );
        }
        if case.starts_with("end-") {
            assert_eq!(
                String::from_utf8_lossy(&output)
                    .matches(&format!("K1_ACTUAL_END_ACK_OBSERVER_LOSS_{case}_PASS"))
                    .count(),
                1
            );
        }
        assert_eq!(namespace_identity(&parent).unwrap(), identity);
        assert_eq!(
            namespace_identity(&namespace_file().unwrap()).unwrap(),
            identity
        );
    }
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "internal disposable lifecycle child; direct invocation refuses"]
fn lifecycle_child() {
    let case = std::env::var("OMAVLESS_K1_LIFECYCLE_CHILD").unwrap();
    assert!(matches!(
        case.as_str(),
        "lifecycle"
            | "service"
            | "lost-create"
            | "lost-replace"
            | "lost-delete"
            | "orphan"
            | "drift"
            | "stale-create"
            | "stale-replace"
            | "lost-socket-reply"
            | "crash-create"
            | "crash-replace"
            | "crash-delete"
            | "trunc-create"
            | "trunc-replace"
            | "trunc-delete"
            | "end-create"
            | "end-replace"
            | "end-delete"
    ));
    let parent = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
    let identity = namespace_identity(&namespace_file().unwrap()).unwrap();
    assert_ne!(namespace_identity(&parent).unwrap(), identity);
    assert_eq!(nix::unistd::geteuid().as_raw(), 1001);
    let interfaces = std::fs::read_to_string("/proc/thread-self/net/dev").unwrap();
    assert_eq!(
        interfaces
            .lines()
            .skip(2)
            .map(|v| v.split(':').next().unwrap().trim())
            .collect::<Vec<_>>(),
        ["lo"]
    );
    let fixture = Fixture::new();
    let mut creator = FixtureCreator::open(fixture.0.clone()).unwrap();
    // Canonical is used ONLY to enter the existing synthetic LockedState seam.
    // This synthetic local epoch cannot leave cfg(test) or authorize production.
    let namespace = NamespaceObservation::Canonical(creator.epoch);
    if case.starts_with("crash-") {
        crash_case(&fixture, &parent, &case, namespace);
        println!("K1_CREATOR_LIFECYCLE_CHILD_PASS");
        return;
    }
    let mut state = fixture.state(case == "service" || case == "lost-socket-reply");
    if case.starts_with("trunc-") || case.starts_with("end-") {
        receive_failure_case(&fixture, state, creator, namespace, &case);
    } else if case == "service" {
        let path = fixture.0.join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let mut owner = SessionOwner::from_prebound(listener, state, creator, namespace).unwrap();
        armed(socket_request(&path, &mut owner, ARM));
        let original = owner.test_kernel().created;
        armed(socket_request(&path, &mut owner, ARM));
        assert_ne!(owner.test_kernel().created, original);
        disarmed(socket_request(&path, &mut owner, DISARM));
        let records = fixture.records();
        disarmed(socket_request(&path, &mut owner, DISARM));
        assert_eq!(fixture.records(), records);
        assert_eq!(owner.test_kernel().effects, 3);
        drop(owner);
    } else if case == "lost-socket-reply" {
        let path = fixture.0.join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let mut owner = SessionOwner::from_prebound(listener, state, creator, namespace).unwrap();
        let (mut client, server) = UnixStream::pair().unwrap();
        let request = encode_request(ARM).unwrap();
        client
            .write_all(&(request.len() as u32).to_be_bytes())
            .unwrap();
        client.write_all(&request).unwrap();
        drop(client);
        assert!(matches!(
            owner.test_poll_with(|_| Ok(server)),
            SessionProgress::Refused(ExchangeError::ReplyDeliveryUnknown(_))
        ));
        let records = fixture.records();
        armed(socket_request(&path, &mut owner, Request::Status {}));
        assert_eq!(owner.test_kernel().effects, 1);
        assert_eq!(fixture.records(), records);
        // No automatic Arm retry: explicit matching disarm is a new user intent.
        disarmed(socket_request(&path, &mut owner, DISARM));
    } else if case == "drift" {
        armed(state.request(ARM, namespace, &mut creator).unwrap());
        inject_drift(&mut creator);
        let effects = creator.effects;
        let records = fixture.records();
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(
                state.request(request, namespace, &mut creator),
                Err(REFUSED)
            );
        }
        assert!(creator.session.poisoned && creator.created.is_none());
        assert_eq!(creator.effects, effects);
        assert_eq!(fixture.records(), records);
        assert_eq!(
            LocalReadSession::open().unwrap().inspect().unwrap(),
            LocalTablePresence::PresentUntrusted
        );
    } else if case.starts_with("stale-") {
        if case == "stale-replace" {
            armed(state.request(ARM, namespace, &mut creator).unwrap());
        }
        let before = creator.created;
        creator.change_generation_before_send = true;
        assert_eq!(state.request(ARM, namespace, &mut creator), Err(REFUSED));
        let records = fixture.records();
        let effects = creator.effects;
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(
                state.request(request, namespace, &mut creator),
                Err(REFUSED)
            );
        }
        assert_eq!(creator.effects, effects);
        assert_eq!(fixture.records(), records);
        let mut independent = LocalReadSession::open().unwrap();
        let (_, _, observed) = independent.inspect_policy_inventory_once().unwrap();
        assert_eq!(observed.map(|t| t.handle), before);
        assert_foreign_present();
    } else if case == "orphan" {
        armed(state.request(ARM, namespace, &mut creator).unwrap());
        let records = fixture.records();
        drop(creator); // owner,persist retains kernel policy without live causality
        drop(state);
        let mut orphan = FixtureCreator::open(fixture.0.clone()).unwrap();
        assert_eq!(
            orphan.session.inspect_policy_inventory().unwrap(),
            LocalPolicyInventory::OtherUntrusted
        );
        assert_eq!(
            orphan.session.inspect_rules().unwrap(),
            LocalRuleInventory::ExactRulesUntrusted(Policy::FullVpn)
        );
        let mut state = fixture.state(false);
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(state.request(request, namespace, &mut orphan), Err(REFUSED));
        }
        assert_eq!(orphan.effects, 0);
        assert_eq!(fixture.records(), records);
    } else if case.starts_with("lost-") {
        if case != "lost-create" {
            armed(state.request(ARM, namespace, &mut creator).unwrap());
        }
        creator.lose_reply = true;
        let request = if case == "lost-delete" { DISARM } else { ARM };
        assert_eq!(
            state.request(request, namespace, &mut creator),
            Err(REFUSED)
        );
        let effects = creator.effects;
        let records = fixture.records();
        drop(state);
        let mut state = fixture.state(false);
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(
                state.request(request, namespace, &mut creator),
                Err(REFUSED)
            );
        }
        assert_eq!(creator.effects, effects);
        assert_eq!(fixture.records(), records);
        let mut independent = LocalReadSession::open().unwrap();
        assert_eq!(
            independent.inspect().unwrap(),
            if case == "lost-delete" {
                LocalTablePresence::Absent
            } else {
                LocalTablePresence::PresentUntrusted
            }
        );
    } else {
        armed(state.request(ARM, namespace, &mut creator).unwrap());
        let first = creator.created;
        drop(state); // lock/file writer restart while actual creator remains live
        let mut state = fixture.state(false);
        armed(
            state
                .request(Request::Status {}, namespace, &mut creator)
                .unwrap(),
        );
        armed(state.request(ARM, namespace, &mut creator).unwrap());
        assert_ne!(creator.created, first);
        disarmed(state.request(DISARM, namespace, &mut creator).unwrap());
        let records = fixture.records();
        drop(state);
        let mut state = fixture.state(false);
        disarmed(state.request(DISARM, namespace, &mut creator).unwrap());
        assert_eq!(
            state.request(ARM, namespace, &mut creator),
            Err(ErrorCode::GenerationConflict)
        );
        assert_eq!(creator.effects, 3);
        assert_eq!(fixture.records(), records);
    }
    assert_eq!(
        namespace_identity(&namespace_file().unwrap()).unwrap(),
        identity
    );
    println!("K1_CREATOR_LIFECYCLE_CHILD_PASS");
}

fn receive_failure_case(
    fixture: &Fixture,
    mut state: LockedState,
    mut creator: FixtureCreator,
    namespace: NamespaceObservation,
    case: &str,
) {
    let end_loss = case.starts_with("end-");
    let operation = case
        .strip_prefix(if end_loss { "end-" } else { "trunc-" })
        .unwrap();
    assert!(matches!(operation, "create" | "replace" | "delete"));
    if operation != "create" {
        armed(state.request(ARM, namespace, &mut creator).unwrap());
    }
    let old_handle = creator.created;
    let socket = creator.session.socket.as_raw_fd();
    let socket_metadata = nix::sys::stat::fstat(&creator.session.socket).unwrap();
    let local = creator.session.local;
    let identity = creator.session.identity;
    let before = creator.effects;
    if end_loss {
        creator.end_ack_loss.arm(socket);
    } else {
        creator.receive_fault.arm(socket);
    }
    assert_eq!(
        state.request(
            if operation == "delete" { DISARM } else { ARM },
            namespace,
            &mut creator
        ),
        Err(REFUSED)
    );
    // Only the real receive path can record either fault. END loss also proves
    // all BEGIN/operation ACKs reached the existing unchanged collector.
    assert_eq!(creator.receive_fault.observed(), usize::from(!end_loss));
    assert_eq!(
        creator.end_ack_loss.observed(),
        if end_loss { (1, true) } else { (0, false) }
    );
    assert!(creator.session.poisoned && creator.created.is_none());
    assert!(!creator.lose_reply);
    assert_eq!(creator.session.socket.as_raw_fd(), socket);
    let after = nix::sys::stat::fstat(&creator.session.socket).unwrap();
    assert_eq!(
        (after.st_dev, after.st_ino),
        (socket_metadata.st_dev, socket_metadata.st_ino)
    );
    assert_eq!(getsockname::<NetlinkAddr>(socket).unwrap(), local);
    assert_eq!(creator.session.identity, identity);
    assert_eq!(creator.effects, before + 1);
    let records = fixture.records();
    let receipt = crate::receipt::decode(records.1.as_ref().unwrap()).unwrap();
    assert_eq!(
        receipt.operation(),
        if operation == "create" { 1 } else { 2 }
    );
    assert_eq!(
        receipt.state(),
        match operation {
            "create" => crate::receipt::ReceiptState::PendingCreate,
            "replace" => crate::receipt::ReceiptState::PendingReplace {
                old_handle: old_handle.unwrap()
            },
            "delete" => crate::receipt::ReceiptState::PendingDelete {
                old_handle: old_handle.unwrap()
            },
            _ => unreachable!(),
        }
    );
    if operation == "create" {
        assert!(records.0.is_none());
    } else {
        let marker: serde_json::Value =
            serde_json::from_slice(records.0.as_ref().unwrap()).unwrap();
        assert_eq!(marker["generation"], 7);
        assert_eq!(marker["armed"], operation != "delete");
    }
    for _ in 0..3 {
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(
                state.request(request, namespace, &mut creator),
                Err(REFUSED)
            );
        }
    }
    assert_eq!(creator.effects, before + 1);
    assert_eq!(creator.receive_fault.observed(), usize::from(!end_loss));
    assert_eq!(
        creator.end_ack_loss.observed(),
        if end_loss { (1, true) } else { (0, false) }
    );
    assert_eq!(fixture.records(), records);

    // A DIFFERENT socket observes actual surviving/absent kernel state only.
    // It cannot restore the poisoned creator or publish a terminal receipt.
    let mut independent = LocalReadSession::open().unwrap();
    assert_ne!(independent.socket.as_raw_fd(), socket);
    assert_eq!(independent.identity, identity);
    for _ in 0..2 {
        assert_eq!(
            independent.inspect().unwrap(),
            if operation == "delete" {
                LocalTablePresence::Absent
            } else {
                LocalTablePresence::PresentUntrusted
            }
        );
        if operation != "delete" {
            let (_, _, table) = independent.inspect_policy_inventory_once().unwrap();
            let table = table.unwrap();
            assert!(table.handle != 0);
            if operation == "replace" {
                assert_ne!(Some(table.handle), old_handle);
            }
            assert_eq!(
                independent.inspect_rules().unwrap(),
                LocalRuleInventory::ExactRulesUntrusted(Policy::FullVpn)
            );
        }
    }
    drop(state);
    let mut state = fixture.state(false);
    for request in [Request::Status {}, ARM, DISARM] {
        assert_eq!(
            state.request(request, namespace, &mut creator),
            Err(REFUSED)
        );
    }
    assert_eq!(creator.effects, before + 1);
    assert_eq!(fixture.records(), records);
    drop(state);
    drop(creator);
    let mut reopened = FixtureCreator::open(fixture.0.clone()).unwrap();
    let mut state = fixture.state(false);
    for _ in 0..2 {
        for request in [Request::Status {}, ARM, DISARM] {
            assert_eq!(
                state.request(request, namespace, &mut reopened),
                Err(REFUSED)
            );
        }
    }
    assert_eq!(reopened.effects, 0);
    assert!(reopened.created.is_none());
    assert_eq!(fixture.records(), records);
    if end_loss {
        println!("K1_ACTUAL_END_ACK_OBSERVER_LOSS_{case}_PASS");
    } else {
        println!("K1_ACTUAL_MSG_TRUNC_{case}_PASS");
    }
}

fn only_loopback() {
    let interfaces = std::fs::read_to_string("/proc/thread-self/net/dev").unwrap();
    assert_eq!(
        interfaces
            .lines()
            .skip(2)
            .map(|v| v.split(':').next().unwrap().trim())
            .collect::<Vec<_>>(),
        ["lo"]
    );
}
pub(super) fn fixed_foreign_change() {
    only_loopback();
    assert!(
        Command::new("/usr/bin/timeout")
            .env_clear()
            .args([
                "2",
                "/usr/bin/nft",
                "add",
                "table",
                "inet",
                "k1_lifecycle_foreign"
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}
fn assert_foreign_present() {
    only_loopback();
    assert!(
        Command::new("/usr/bin/timeout")
            .env_clear()
            .args([
                "2",
                "/usr/bin/nft",
                "list",
                "table",
                "inet",
                "k1_lifecycle_foreign"
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}
fn inject_drift(creator: &mut FixtureCreator) {
    let deadline = Instant::now() + Duration::from_secs(1);
    let (_, generation, _) = creator.inspect().unwrap();
    let first = creator.session.next_sequence;
    let begin = message(
        16,
        5,
        first,
        0,
        &[vec![0, 0, 0, 10], attribute(1, &generation.to_be_bytes())].concat(),
    );
    let append = message(
        NFT + 6,
        0xc05,
        first + 1,
        0,
        &[
            vec![1, 0, 0, 0],
            attribute(1, TABLE),
            attribute(2, b"output_guard\0"),
            crate::emergency_wire::nested(4, &crate::emergency_wire::verdict(1)),
        ]
        .concat(),
    );
    let end = message(17, 5, first + 2, 0, &[0, 0, 0, 10]);
    creator
        .send_batch(vec![begin, append, end], deadline)
        .unwrap();
}
fn crash_case(fixture: &Fixture, parent: &File, case: &str, namespace: NamespaceObservation) {
    let mut guard = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", WRITER])
            .env_clear()
            .env("OMAVLESS_K1_CRASH_WRITE", case)
            .env("OMAVLESS_K1_CRASH_PATH", &fixture.0)
            .env("OMAVLESS_K1_CRASH_HOLDER", std::process::id().to_string())
            .env("TMPDIR", std::env::temp_dir())
            .stdin(Stdio::from(parent.try_clone().unwrap()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    while !fixture.0.join("effect-cut").exists() {
        assert!(
            guard.0.try_wait().unwrap().is_none(),
            "isolated writer exited before checkpoint"
        );
        assert!(
            Instant::now() < deadline,
            "isolated writer checkpoint deadline"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        std::fs::read(fixture.0.join("effect-cut")).unwrap(),
        b"K1_EFFECT_CUT\n"
    );
    assert!(matches!(
        RootStateStore::open_test_parent(File::open(&fixture.0).unwrap(), (1001, 1001), 1001),
        Err(crate::root_state::StateError::Busy)
    ));
    guard.0.kill().unwrap();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(guard.0.wait().unwrap().signal(), Some(9));
    let records = fixture.records();
    let value: serde_json::Value = serde_json::from_slice(records.1.as_ref().unwrap()).unwrap();
    assert_eq!(
        value["phase"],
        match case {
            "crash-create" => "pending_create",
            "crash-replace" => "pending_replace",
            _ => "pending_delete",
        }
    );
    let mut creator = FixtureCreator::open(fixture.0.clone()).unwrap();
    assert_eq!(
        creator.session.inspect().unwrap(),
        if case == "crash-delete" {
            LocalTablePresence::Absent
        } else {
            LocalTablePresence::PresentUntrusted
        }
    );
    if case != "crash-delete" {
        assert_eq!(
            creator.session.inspect_rules().unwrap(),
            LocalRuleInventory::ExactRulesUntrusted(Policy::FullVpn)
        );
    }
    let mut state = fixture.state(false);
    for request in [Request::Status {}, ARM, DISARM] {
        assert_eq!(
            state.request(request, namespace, &mut creator),
            Err(REFUSED)
        );
    }
    assert_eq!(creator.effects, 0);
    assert_eq!(fixture.records(), records);
}
#[test]
#[ignore = "internal writer; parent namespace and arbitrary path invocation refuse"]
fn lifecycle_crash_writer() {
    let case = std::env::var("OMAVLESS_K1_CRASH_WRITE").unwrap();
    assert!(matches!(
        case.as_str(),
        "crash-create" | "crash-replace" | "crash-delete"
    ));
    let parent = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
    assert_ne!(
        namespace_identity(&parent).unwrap(),
        namespace_identity(&namespace_file().unwrap()).unwrap()
    );
    assert_eq!(nix::unistd::geteuid().as_raw(), 1001);
    only_loopback();
    let path = PathBuf::from(std::env::var_os("OMAVLESS_K1_CRASH_PATH").unwrap());
    let holder: u32 = std::env::var("OMAVLESS_K1_CRASH_HOLDER")
        .unwrap()
        .parse()
        .unwrap();
    assert!(holder > 0 && holder != std::process::id());
    assert_eq!(path, std::env::temp_dir().join(format!("k1lc-{holder}")));
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_dir() && metadata.uid() == 1001 && metadata.mode() & 0o7777 == 0o700);
    // The holder owns cleanup. A failed child must not remove borrowed state.
    let fixture = std::mem::ManuallyDrop::new(Fixture(path));
    let mut creator = FixtureCreator::open(fixture.0.clone()).unwrap();
    let namespace = NamespaceObservation::Canonical(creator.epoch);
    let mut state = fixture.state(false);
    if case != "crash-create" {
        armed(state.request(ARM, namespace, &mut creator).unwrap());
    }
    creator.cut_after_effect = true;
    let request = if case == "crash-delete" { DISARM } else { ARM };
    state.request(request, namespace, &mut creator).unwrap();
    panic!("writer returned from crash checkpoint");
}
#[test]
fn child_direct_invocation_refuses_before_socket_or_fixture_creation() {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", CHILD])
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("K1_CREATOR_LIFECYCLE_CHILD_PASS"));
}
