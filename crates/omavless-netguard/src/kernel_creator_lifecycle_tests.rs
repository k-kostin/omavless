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
    assert!(c.complete() && c.changed());
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
            assert!(replies.poisoned() && !replies.complete());
            assert!(replies.acks().iter().all(|ack| !ack));
            for request in requests.iter() {
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

#[test]
fn every_begin_or_operation_loss_requires_actual_remaining_ack_delivery_then_seals() {
    // Pure constructed transcripts exercise every create/replacement request;
    // actual kernel delivery belongs only to the ignored VM gate below.
    for old in [None, Some(9)] {
        let requests = full_batch(7, 10, old).unwrap();
        for target in 0..requests.len() - 1 {
            for reverse in [false, true] {
                let mut loss = prefix_ack_loss::OneShotPrefixAckLoss::default();
                loss.arm(7, target);
                let mut replies = BatchReplies::new(requests.clone(), 42).unwrap();
                let mut order: Vec<_> = (0..requests.len()).collect();
                if reverse {
                    order.reverse();
                }
                for (position, index) in order.iter().copied().enumerate() {
                    let bytes = ack(&requests[index], 42, 0);
                    let delivered = loss
                        .deliver(
                            7,
                            &bytes,
                            Some(NetlinkAddr::new(0, 0)),
                            MsgFlags::empty(),
                            &requests[target],
                            42,
                        )
                        .unwrap();
                    if !delivered.is_empty() {
                        assert_eq!(&*delivered, bytes);
                        replies
                            .receive(&delivered, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
                            .unwrap();
                    }
                    assert!(!replies.complete());
                    assert_eq!(
                        replies.finish_prefix_loss(&mut loss).unwrap(),
                        position + 1 == order.len()
                    );
                }
                assert_eq!(loss.observed(), (1, true));
                assert!(replies.poisoned() && !replies.changed() && !replies.acks()[target]);
                assert!(
                    replies
                        .acks()
                        .iter()
                        .enumerate()
                        .all(|(i, accepted)| i == target || *accepted)
                );
                for request in requests.iter() {
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
}

struct Fixture(PathBuf, bool);
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
        Self(path, false)
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
        if self.1 && std::thread::panicking() {
            return; // New crash gate retains Pending evidence until containment/review.
        }
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
#[ignore = "VM-only owned writer SIGKILL after send return before first ACK receive"]
fn real_send_return_create_death_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_SEND_RETURN_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&["send-create"]);
    println!("K1_SEND_RETURN_CREATE_VM_PASS scenarios=1");
}
#[test]
#[ignore = "VM-only owned replacement writer SIGKILL after send before receive"]
fn real_send_return_replace_death_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_SEND_RETURN_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&["send-replace"]);
    println!("K1_SEND_RETURN_REPLACE_VM_PASS scenarios=1");
}
#[test]
#[ignore = "VM-only owned deletion writer SIGKILL after send before receive"]
fn real_send_return_delete_death_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_SEND_RETURN_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&["send-delete"]);
    println!("K1_SEND_RETURN_DELETE_VM_PASS scenarios=1");
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
#[test]
#[ignore = "VM-only actual BEGIN/operation ACK observer loss with END and all other ACKs delivered"]
fn real_prefix_ack_observer_loss_in_disposable_vm() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_PREFIX_ACK_LOSS_VM").as_deref(),
        Ok("1")
    );
    isolated_cases(&[
        "begin-create",
        "begin-replace",
        "begin-delete",
        "operation-create",
        "operation-replace",
        "operation-delete",
    ]);
    println!("K1_REAL_PREFIX_ACK_OBSERVER_LOSS_VM_PASS scenarios=6");
}
fn prefix_loss_receipt(case: &str) -> Option<String> {
    let target = match case {
        "begin-create" | "begin-replace" | "begin-delete" => 0,
        "operation-create" | "operation-replace" | "operation-delete" => 1,
        _ => return None,
    };
    Some(format!(
        "K1_ACTUAL_PREFIX_ACK_OBSERVER_LOSS_{case}_PASS target={target} consumed=1 remaining_delivered=1 effects=1 retries=0 reopened_effects=0"
    ))
}
fn valid_prefix_loss_receipt(output: &[u8], case: &str) -> bool {
    let (Ok(output), Some(expected)) = (std::str::from_utf8(output), prefix_loss_receipt(case))
    else {
        return false;
    };
    output.lines().filter(|line| *line == expected).count() == 1
        && output
            .lines()
            .filter(|line| line.starts_with("K1_ACTUAL_PREFIX_ACK_OBSERVER_LOSS_"))
            .count()
            == 1
}
#[test]
fn prefix_loss_receipts_require_exact_case_once_and_every_count() {
    let expected = prefix_loss_receipt("begin-create").unwrap();
    assert!(valid_prefix_loss_receipt(
        expected.as_bytes(),
        "begin-create"
    ));
    for bad in [
        String::new(),
        format!("{expected}\n{expected}"),
        prefix_loss_receipt("operation-create").unwrap(),
        expected.replace("consumed=1", "consumed=0"),
        expected.replace("remaining_delivered=1", "remaining_delivered=0"),
        expected.replace("effects=1", "effects=2"),
        expected.replace("retries=0", "retries=1"),
        expected.replace("reopened_effects=0", "reopened_effects=1"),
        format!("{expected} extra"),
        format!("prefix {expected}"),
        format!(
            "{expected}\n{}",
            prefix_loss_receipt("begin-delete").unwrap()
        ),
    ] {
        assert!(!valid_prefix_loss_receipt(bad.as_bytes(), "begin-create"));
    }
    assert!(!valid_prefix_loss_receipt(&[0xff], "begin-create"));
    assert!(!valid_prefix_loss_receipt(
        expected.as_bytes(),
        "end-create"
    ));
}
fn fixed_group_members() -> Vec<i32> {
    let group = nix::unistd::getpgrp().as_raw();
    let mut members = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap() {
        let entry = entry.unwrap();
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        let text = match std::fs::read_to_string(entry.path().join("stat")) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => panic!("process-group inventory unavailable"),
        };
        let fields = text.rsplit_once(") ").unwrap().1;
        let observed: i32 = fields.split_whitespace().nth(2).unwrap().parse().unwrap();
        if observed == group {
            members.push(pid);
        }
    }
    members.sort_unstable();
    members
}
fn assert_fixed_group(expected: &[i32]) {
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(
        fixed_group_members(),
        expected,
        "unsettled fixture descendants"
    );
}
fn drain_pipe(pipe: &mut impl Read, bytes: &mut Vec<u8>) -> bool {
    loop {
        let mut block = [0_u8; 4096];
        match pipe.read(&mut block) {
            Ok(0) => return true,
            Ok(length) => {
                bytes.extend_from_slice(&block[..length]);
                assert!(bytes.len() <= 32768, "fixture output limit");
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return false,
            Err(_) => panic!("fixture output unavailable"),
        }
    }
}
fn isolated_send_case(parent: &File, case: &str) {
    assert!(send_return_cut::SendKind::from_case(case).is_some());
    // External fixed supervisor owns this group. Never create a nested PGID
    // which could evade its cancellation. No arbitrary command is accepted.
    let leader = std::process::id() as i32;
    assert_eq!(nix::unistd::getpgrp().as_raw(), leader);
    assert_fixed_group(&[leader]);
    let mut child = Command::new("/usr/bin/unshare")
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
        .unwrap();
    // std::process::Child does not signal/reap on Drop. Any failure below
    // deliberately leaves its anchor/evidence to the outer group supervisor.
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    use nix::fcntl::{FcntlArg, OFlag, fcntl};
    fcntl(&stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
    fcntl(&stderr, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
    let (mut output, mut error) = (Vec::new(), Vec::new());
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < deadline, "send-return holder deadline");
        let stdout_eof = drain_pipe(&mut stdout, &mut output);
        let stderr_eof = drain_pipe(&mut stderr, &mut error);
        use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
        let status = waitid(
            Id::Pid(nix::unistd::Pid::from_raw(child.id() as i32)),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
        )
        .expect("holder anchor unavailable; outer supervisor must contain");
        if status != WaitStatus::StillAlive && stdout_eof && stderr_eof {
            assert_fixed_group(&[leader, child.id() as i32]);
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(child.wait().unwrap().success(), "send-return holder failed");
    assert_fixed_group(&[leader]);
    assert!(valid_send_return_receipt(&output, case));
    let text = std::str::from_utf8(&output).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| *line == "K1_CREATOR_LIFECYCLE_CHILD_PASS")
            .count(),
        1
    );
    println!(
        "{}",
        text.lines()
            .find(|line| line.starts_with("K1_ACTUAL_SEND_RETURN_DEATH_"))
            .unwrap()
    );
}

fn isolated_cases(cases: &[&str]) {
    let parent = namespace_file().unwrap();
    let identity = namespace_identity(&parent).unwrap();
    for case in cases {
        if send_return_cut::SendKind::from_case(case).is_some() {
            isolated_send_case(&parent, case);
            assert_eq!(namespace_identity(&parent).unwrap(), identity);
            assert_eq!(
                namespace_identity(&namespace_file().unwrap()).unwrap(),
                identity
            );
            continue;
        }
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
        if case.starts_with("begin-") || case.starts_with("operation-") {
            assert!(
                valid_prefix_loss_receipt(&output, case),
                "missing or invalid exact prefix-loss receipt"
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
            | "begin-create"
            | "begin-replace"
            | "begin-delete"
            | "operation-create"
            | "operation-replace"
            | "operation-delete"
            | "send-create"
            | "send-replace"
            | "send-delete"
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
    let mut fixture = Fixture::new();
    fixture.1 = send_return_cut::SendKind::from_case(&case).is_some();
    let mut creator = FixtureCreator::open(fixture.0.clone()).unwrap();
    // Canonical is used ONLY to enter the existing synthetic LockedState seam.
    // This synthetic local epoch cannot leave cfg(test) or authorize production.
    let namespace = NamespaceObservation::Canonical(creator.epoch);
    if case.starts_with("send-") {
        send_return_case(&fixture, &parent, &case, namespace);
        println!("K1_CREATOR_LIFECYCLE_CHILD_PASS");
        return;
    }
    if case.starts_with("crash-") {
        crash_case(&fixture, &parent, &case, namespace);
        println!("K1_CREATOR_LIFECYCLE_CHILD_PASS");
        return;
    }
    let mut state = fixture.state(case == "service" || case == "lost-socket-reply");
    if case.starts_with("trunc-")
        || case.starts_with("end-")
        || case.starts_with("begin-")
        || case.starts_with("operation-")
    {
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
    let prefix = if case.starts_with("begin-") {
        Some(("begin-", 0))
    } else if case.starts_with("operation-") {
        Some(("operation-", 1))
    } else {
        None
    };
    let operation = case
        .strip_prefix(prefix.map_or(if end_loss { "end-" } else { "trunc-" }, |p| p.0))
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
    if let Some((_, target)) = prefix {
        creator.prefix_ack_loss.arm(socket, target);
    } else if end_loss {
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
    assert_eq!(
        creator.receive_fault.observed(),
        usize::from(!end_loss && prefix.is_none())
    );
    assert_eq!(
        creator.prefix_ack_loss.observed(),
        if prefix.is_some() {
            (1, true)
        } else {
            (0, false)
        }
    );
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
    assert_eq!(
        creator.receive_fault.observed(),
        usize::from(!end_loss && prefix.is_none())
    );
    assert_eq!(
        creator.prefix_ack_loss.observed(),
        if prefix.is_some() {
            (1, true)
        } else {
            (0, false)
        }
    );
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
    if prefix.is_some() {
        println!("{}", prefix_loss_receipt(case).unwrap());
    } else if end_loss {
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
    creator
        .send_batch(
            super::super::atomic_batch::drift_batch(generation, first).unwrap(),
            deadline,
        )
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

/// Never reap merely to poll a checkpoint. Unknown wait ownership cannot
/// authorize a signal. The outer fixed group supervisor contains failure paths.
struct UnreapedWriter {
    child: Child,
    reaped: bool,
    anchor_known: bool,
}
impl UnreapedWriter {
    fn status(&mut self) -> nix::Result<nix::sys::wait::WaitStatus> {
        use nix::sys::wait::{Id, WaitPidFlag, waitid};
        if !self.anchor_known {
            return Err(nix::errno::Errno::ECHILD);
        }
        let result = waitid(
            Id::Pid(nix::unistd::Pid::from_raw(self.child.id() as i32)),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
        );
        if result.is_err() {
            self.anchor_known = false;
        }
        result
    }
    fn kill_and_reap(&mut self) {
        assert_eq!(
            self.status().unwrap(),
            nix::sys::wait::WaitStatus::StillAlive
        );
        if self.child.kill().is_err() {
            self.anchor_known = false;
            panic!("writer signal ownership uncertain");
        }
        use std::os::unix::process::ExitStatusExt;
        let status = match self.child.wait() {
            Ok(status) => status,
            Err(_) => {
                self.anchor_known = false;
                panic!("writer reap uncertain");
            }
        };
        self.reaped = true;
        self.anchor_known = false;
        assert_eq!(status.signal(), Some(9));
    }
}
impl Drop for UnreapedWriter {
    fn drop(&mut self) {
        if self.reaped || !self.anchor_known {
            return;
        }
        if let Ok(status) = self.status() {
            if status == nix::sys::wait::WaitStatus::StillAlive && self.child.kill().is_err() {
                return;
            }
            let _ = self.child.wait();
        }
        // On unknown ownership, no kill/reap. Failure of the outer owned-group
        // cleanup remains NONPASS, never a successful checkpoint/recovery.
    }
}
#[test]
#[ignore = "internal fixed ordinary child, no namespace or network operation"]
fn send_return_anchor_worker() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_ANCHOR_WORKER").as_deref(),
        Ok("1")
    );
    std::thread::sleep(Duration::from_secs(10));
}
fn anchor_worker() -> UnreapedWriter {
    UnreapedWriter {
        child: Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "kernel_observer::creator_lifecycle::tests::send_return_anchor_worker",
            ])
            .env_clear()
            .env("OMAVLESS_K1_ANCHOR_WORKER", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
        reaped: false,
        anchor_known: true,
    }
}
#[test]
fn send_return_writer_keeps_actual_child_unreaped_until_exact_kill() {
    let mut writer = anchor_worker();
    assert_eq!(
        writer.status().unwrap(),
        nix::sys::wait::WaitStatus::StillAlive
    );
    assert_eq!(
        writer.status().unwrap(),
        nix::sys::wait::WaitStatus::StillAlive
    );
    writer.kill_and_reap();
    assert!(writer.reaped);
    assert_eq!(writer.status(), Err(nix::errno::Errno::ECHILD));
}
#[test]
fn send_return_writer_drop_reaps_only_its_still_owned_child() {
    let writer = anchor_worker();
    let pid = nix::unistd::Pid::from_raw(writer.child.id() as i32);
    drop(writer);
    use nix::sys::wait::{Id, WaitPidFlag, waitid};
    assert_eq!(
        waitid(
            Id::Pid(pid),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT
        ),
        Err(nix::errno::Errno::ECHILD)
    );
}
#[test]
fn send_return_writer_latches_lost_anchor_and_preserves_failed_fixture() {
    let mut writer = anchor_worker();
    writer.child.kill().unwrap();
    writer.child.wait().unwrap(); // Test simulates a reaper outside the guard.
    assert_eq!(writer.status(), Err(nix::errno::Errno::ECHILD));
    assert!(!writer.anchor_known);
    assert_eq!(writer.status(), Err(nix::errno::Errno::ECHILD));
    drop(writer); // Latched uncertainty must not query, signal or reap again.
    let mut fixture = Fixture::new();
    fixture.1 = true;
    let retained = fixture.0.clone();
    let failure = std::panic::catch_unwind(move || {
        let _retained_during_unwind = fixture;
        panic!("synthetic holder failure");
    });
    assert!(failure.is_err() && retained.is_dir());
    std::fs::remove_dir_all(retained).unwrap(); // Only this owned synthetic fixture.
}

fn send_return_receipt(case: &str, observed: &str) -> Option<String> {
    send_return_cut::SendKind::from_case(case)?;
    if !matches!(observed, "absent" | "present_untrusted") {
        return None;
    }
    Some(format!(
        "K1_ACTUAL_SEND_RETURN_DEATH_{case}_PASS sends=1 acks=0 readbacks=0 killed=1 observed={observed} reopened_effects=0 retries=0"
    ))
}
fn valid_send_return_receipt(output: &[u8], case: &str) -> bool {
    let Ok(text) = std::str::from_utf8(output) else {
        return false;
    };
    let receipts: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("K1_ACTUAL_SEND_RETURN_DEATH_"))
        .collect();
    receipts.len() == 1
        && ["absent", "present_untrusted"]
            .into_iter()
            .any(|observed| send_return_receipt(case, observed).as_deref() == Some(receipts[0]))
}
#[test]
fn send_return_receipts_require_exact_untrusted_outcome_and_counts() {
    for case in ["send-create", "send-replace", "send-delete"] {
        for observed in ["absent", "present_untrusted"] {
            let expected = send_return_receipt(case, observed).unwrap();
            assert!(valid_send_return_receipt(expected.as_bytes(), case));
            for bad in [
                format!("{expected}\n{expected}"),
                format!("{expected} extra"),
                format!("prefix {expected}"),
                expected.replace("sends=1", "sends=2"),
                expected.replace("acks=0", "acks=1"),
                expected.replace("readbacks=0", "readbacks=1"),
                expected.replace("killed=1", "killed=0"),
                expected.replace("reopened_effects=0", "reopened_effects=1"),
                expected.replace("retries=0", "retries=1"),
                expected.replace(observed, "owned"),
            ] {
                assert!(!valid_send_return_receipt(bad.as_bytes(), case));
            }
            assert!(!valid_send_return_receipt(
                expected.as_bytes(),
                "crash-create"
            ));
        }
    }
    assert!(!valid_send_return_receipt(&[0xff], "send-create"));
}
fn send_return_case(fixture: &Fixture, parent: &File, case: &str, namespace: NamespaceObservation) {
    let kind = send_return_cut::SendKind::from_case(case).unwrap();
    let mut writer = UnreapedWriter {
        child: Command::new(std::env::current_exe().unwrap())
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
        reaped: false,
        anchor_known: true,
    };
    let deadline = Instant::now() + Duration::from_secs(8);
    let checkpoint = fixture.0.join("send-return-cut");
    while !checkpoint.exists() {
        assert_eq!(
            writer.status().unwrap(),
            nix::sys::wait::WaitStatus::StillAlive
        );
        assert!(Instant::now() < deadline, "send-return checkpoint deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(std::fs::read(&checkpoint).unwrap(), kind.checkpoint());
    let metadata = std::fs::symlink_metadata(&checkpoint).unwrap();
    assert!(metadata.is_file() && metadata.uid() == 1001 && metadata.mode() & 0o7777 == 0o600);
    let records = fixture.records();
    let pending: serde_json::Value = serde_json::from_slice(records.1.as_ref().unwrap()).unwrap();
    assert_eq!(pending["phase"], kind.phase());
    assert!(matches!(
        RootStateStore::open_test_parent(File::open(&fixture.0).unwrap(), (1001, 1001), 1001),
        Err(crate::root_state::StateError::Busy)
    ));
    writer.kill_and_reap();
    assert_eq!(fixture.records(), records);
    // A successful send return is not commit proof. Classify the actual kernel
    // result without converting parser failure into absence or orphan ownership.
    let mut fresh = FixtureCreator::open(fixture.0.clone()).unwrap();
    let (inventory, _, table) = fresh.session.inspect_policy_inventory_once().unwrap();
    let observed = match inventory {
        LocalPolicyInventory::TableAbsent => {
            assert!(table.is_none());
            "absent"
        }
        LocalPolicyInventory::OtherUntrusted => {
            let table = table.unwrap();
            assert_eq!(table.flags, 4); // persist remains; dead socket OWNER is released
            assert_eq!(table.owner, None);
            assert_eq!(
                fresh.session.inspect_rules().unwrap(),
                LocalRuleInventory::ExactRulesUntrusted(Policy::FullVpn)
            );
            "present_untrusted"
        }
        _ => panic!("unexpected ownership-shaped post-death inventory"),
    };
    assert!(fresh.created.is_none());
    let mut state = fixture.state(false);
    for request in [Request::Status {}, ARM, DISARM] {
        assert_eq!(state.request(request, namespace, &mut fresh), Err(REFUSED));
    }
    assert_eq!(fresh.effects, 0);
    assert_eq!(fixture.records(), records);
    // No writer/descendant may survive into removal of this private fixture.
    assert_fixed_group(&[nix::unistd::getpgrp().as_raw(), std::process::id() as i32]);
    println!("{}", send_return_receipt(case, observed).unwrap());
}
#[test]
#[ignore = "internal writer; parent namespace and arbitrary path invocation refuse"]
fn lifecycle_crash_writer() {
    let case = std::env::var("OMAVLESS_K1_CRASH_WRITE").unwrap();
    assert!(matches!(
        case.as_str(),
        "crash-create"
            | "crash-replace"
            | "crash-delete"
            | "send-create"
            | "send-replace"
            | "send-delete"
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
    let fixture = std::mem::ManuallyDrop::new(Fixture(path, true));
    let mut creator = FixtureCreator::open(fixture.0.clone()).unwrap();
    let namespace = NamespaceObservation::Canonical(creator.epoch);
    let mut state = fixture.state(false);
    if !matches!(case.as_str(), "crash-create" | "send-create") {
        armed(state.request(ARM, namespace, &mut creator).unwrap());
    }
    if let Some(kind) = send_return_cut::SendKind::from_case(&case) {
        creator
            .send_cut
            .arm(creator.session.socket.as_raw_fd(), kind, fixture.0.clone());
    } else {
        creator.cut_after_effect = true;
    }
    let request = if matches!(case.as_str(), "crash-delete" | "send-delete") {
        DISARM
    } else {
        ARM
    };
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
