// SPDX-License-Identifier: MIT
//! Owned unprivileged child smoke only; NOT the strict ancillary adapter.
//!
//! Pinned rustix 1.1.5 hides unknown ancillary kinds and oversized credentials.
//! This checkpoint therefore cannot authenticate a production/root transport.
//! Returned owner graphs are retained until test-process exit. The existing
//! Process/LocalParent partial-acquisition limitation is not eliminated here.

use super::*;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::sockopt::{set_socket_passcred, socket_peercred};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, UCred, recvmsg,
    sendmsg, socketpair,
};
use std::io::{IoSlice, IoSliceMut};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::process::ExitStatusExt;

const REQUEST: &[u8] = b"T4-SMOKE-ONE-CONSULT-00000001";
const REPLY: &[u8] = b"T4-SMOKE-ONE-REPLY-00000001";
const MAX_NAME: usize = 4096;
const WORKER: &str =
    "restore_abort_cli::stopped_owner::retained_parent_prototype::kernel_smoke::owned_child_worker";

// No Drop-based close, wait, signal or cleanup after late/unknown outcomes.
// This bounded HOST test leaks its small owner graph until process exit even
// on success. It is not a reusable production resource-lifetime strategy.
struct Retained<T>(ManuallyDrop<T>);
impl<T> Retained<T> {
    fn new(value: T) -> Self {
        Self(ManuallyDrop::new(value))
    }
}
impl<T> Deref for Retained<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> DerefMut for Retained<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

fn gate(end: Instant) -> Result<()> {
    if Instant::now() >= end {
        Err(())
    } else {
        Ok(())
    }
}

// An inner Process observation may shorten, never renew, the original outer
// grant. Existing Process checks remain sampled; this is not IO cancellation.
fn clipped_budget(end: Instant) -> Result<Budget> {
    gate(end)?;
    let mut budget = Budget::new();
    budget.until = budget.until.min(end);
    budget.check()?;
    Ok(budget)
}

fn ready<F: AsFd>(socket: &F, flags: PollFlags, end: Instant) -> Result<()> {
    gate(end)?;
    let remaining = end.checked_duration_since(Instant::now()).ok_or(())?;
    let timeout = Timespec::try_from(remaining).map_err(|_| ())?;
    let mut descriptors = [PollFd::new(socket, flags)];
    gate(end)?;
    let count = poll(&mut descriptors, Some(&timeout)).map_err(|_| ())?;
    gate(end)?;
    // HUP is permitted only alongside an already available complete frame.
    let returned = descriptors[0].revents();
    if count != 1 || !returned.contains(flags) || !(returned - (flags | PollFlags::HUP)).is_empty()
    {
        return Err(());
    }
    Ok(())
}

fn send<F: AsFd>(socket: &F, bytes: &[u8], rights: &[BorrowedFd<'_>], end: Instant) -> Result<()> {
    ready(socket, PollFlags::OUT, end)?;
    let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut control = SendAncillaryBuffer::new(&mut storage);
    if !rights.is_empty() && !control.push(SendAncillaryMessage::ScmRights(rights)) {
        return Err(());
    }
    gate(end)?;
    let written = sendmsg(
        socket,
        &[IoSlice::new(bytes)],
        &mut control,
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )
    .map_err(|_| ())?;
    gate(end)?;
    if written != bytes.len() {
        return Err(());
    }
    Ok(())
}

struct Received {
    bytes: Vec<u8>,
    credentials: Vec<UCred>,
    rights: Retained<Vec<File>>,
    flags: ReturnFlags,
}

fn receive<F: AsFd>(socket: &F, end: Instant) -> Result<Received> {
    ready(socket, PollFlags::IN, end)?;
    let mut bytes = vec![0; REPLY.len() + MAX_NAME + 1];
    let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3), ScmCredentials(1))];
    let mut control = RecvAncillaryBuffer::new(&mut storage);
    let mut rights = Retained::new(Vec::new());
    let mut credentials = Vec::new();
    let mut unknown = false;
    gate(end)?;
    let result = recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut control,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )
    .map_err(|_| ())?;
    // Ownership transfer MUST precede deadline/shape refusal. Drain all
    // library-visible FDs before RecvAncillaryBuffer could close them on Drop.
    // Unknown ancillary types/credential lengths are NOT visible through this
    // pinned API: this explicit gap is why no strict transport is claimed.
    for message in control.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(files) => rights.extend(files.map(File::from)),
            RecvAncillaryMessage::ScmCredentials(value) => credentials.push(value),
            other => {
                std::mem::forget(other);
                unknown = true;
            }
        }
    }
    gate(end)?;
    if unknown || result.bytes > bytes.len() || result.address.is_some() {
        return Err(());
    }
    bytes.truncate(result.bytes);
    Ok(Received {
        bytes,
        credentials,
        rights,
        flags: result.flags,
    })
}

fn shape(message: &Received, peer: (i32, u32, u32), reply: bool) -> Result<()> {
    let [credential] = message.credentials.as_slice() else {
        return Err(());
    };
    if (
        credential.pid.as_raw_nonzero().get(),
        credential.uid.as_raw(),
        credential.gid.as_raw(),
    ) != peer
        || !(message.flags - (ReturnFlags::CMSG_CLOEXEC | ReturnFlags::EOR)).is_empty()
    {
        return Err(());
    }
    if reply {
        if message.rights.len() != 3
            || !message.bytes.starts_with(REPLY)
            || message.bytes.len() <= REPLY.len()
            || message.bytes.len() > REPLY.len() + MAX_NAME
        {
            return Err(());
        }
    } else if message.bytes != REQUEST || !message.rights.is_empty() {
        return Err(());
    }
    Ok(())
}

// Exercises the same strict Process flow with actual transferred kernel Files,
// but does NOT supply a transport parent to Process or admit StoppedOwner.
fn compare_current(message: &Received, pid: u32, end: Instant) -> Result<()> {
    gate(end)?;
    let root = Retained::new(File::open("/proc").map_err(|_| ())?);
    gate(end)?;
    let mut budget = clipped_budget(end)?;
    gate(end)?;
    let current = Retained::new(Process::capture(&root, pid, &mut budget)?);
    gate(end)?;
    let image = message.rights[0].metadata().map_err(|_| ())?;
    gate(end)?;
    if !executable_identity(&image, &current.executable_identity)
        || message.bytes[REPLY.len()..] != *current.executable_name
    {
        return Err(());
    }
    gate(end)?;
    current.recheck(&root, &mut budget)?;
    gate(end)?;
    for (index, name) in ["ns/pid", "ns/user"].into_iter().enumerate() {
        gate(end)?;
        let namespace = Retained::new(current.current_namespace(&root, name, &mut budget)?);
        gate(end)?;
        let received = message.rights[index + 1].metadata().map_err(|_| ())?;
        gate(end)?;
        let named = namespace.metadata().map_err(|_| ())?;
        gate(end)?;
        if !identity(&received, &named) {
            return Err(());
        }
    }
    Ok(())
}

#[test]
#[ignore = "fixed worker entered only by the bounded owned-child smoke"]
fn owned_child_worker() {
    assert_eq!(
        std::env::var("OMAVLESS_T4_PAIR_SMOKE").as_deref(),
        Ok("one")
    );
    let end = Instant::now() + Duration::from_secs(8);
    let socket = std::io::stdin(); // retained inherited original, not fd-number authority
    let peer = (
        nix::unistd::getppid().as_raw(),
        nix::unistd::getuid().as_raw(),
        nix::unistd::getgid().as_raw(),
    );
    send(&socket, REQUEST, &[], end).unwrap();
    let reply = receive(&socket, end).unwrap();
    shape(&reply, peer, true).unwrap();
    compare_current(&reply, u32::try_from(peer.0).unwrap(), end).unwrap();
    gate(end).unwrap();
}

#[test]
#[ignore = "first owned-child attempt NONPASS; corrected fresh child requires source review"]
fn owned_child_credentials_and_original_files_smoke() {
    let end = Instant::now() + Duration::from_secs(8);
    let mut capture_budget = clipped_budget(end).unwrap();
    let parent = Retained::new(
        LocalParent::capture_local(
            File::open("/proc").unwrap(),
            std::process::id(),
            &mut capture_budget,
        )
        .unwrap(),
    );
    gate(end).unwrap();
    let (server, client) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let server = Retained::new(File::from(server));
    let client = Retained::new(File::from(client));
    gate(end).unwrap();
    set_socket_passcred(&*server, true).unwrap();
    gate(end).unwrap();
    set_socket_passcred(&*client, true).unwrap();
    gate(end).unwrap();
    // Demonstrates why socket creation-time peer credentials do not identify
    // the eventual child sending an actual kernel-credentialed request.
    let creator = socket_peercred(&*server).unwrap();
    gate(end).unwrap();
    assert_eq!(
        creator.pid.as_raw_nonzero().get(),
        std::process::id() as i32
    );
    let mut child_endpoint = Retained::new(Some(client.try_clone().unwrap()));
    gate(end).unwrap();
    let executable = format!("/proc/self/fd/{}", parent.original.executable.as_raw_fd());
    let mut command = Retained::new(Command::new(executable));
    command
        .args(["--exact", WORKER, "--ignored", "--test-threads=1"])
        .env("OMAVLESS_T4_PAIR_SMOKE", "one")
        .stdin(Stdio::from(child_endpoint.take().unwrap()))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    gate(end).unwrap();
    let mut child = Retained::new(command.spawn().unwrap());
    gate(end).unwrap();
    let child_pid = Pid::from_raw(i32::try_from(child.id()).unwrap());
    assert_ne!(creator.pid.as_raw_nonzero().get(), child_pid.as_raw());
    let request = receive(&*server, end).unwrap();
    shape(
        &request,
        (
            child_pid.as_raw(),
            nix::unistd::getuid().as_raw(),
            nix::unistd::getgid().as_raw(),
        ),
        false,
    )
    .unwrap();
    gate(end).unwrap();
    let mut consultation_budget = clipped_budget(end).unwrap();
    let mut bundle = Retained::new(
        parent
            .consult(
                &parent.root,
                parent.original.pid,
                &parent.original.directory,
                parent.original.start,
                &mut consultation_budget,
            )
            .unwrap(),
    );
    let mut files = Retained::new(Vec::new());
    files.push(bundle.executable.take().unwrap());
    files.push(bundle.pid_namespace.take().unwrap());
    files.push(bundle.user_namespace.take().unwrap());
    gate(end).unwrap();
    let mut reply = REPLY.to_vec();
    let name = bundle.executable_name.take().unwrap();
    assert!(!name.is_empty() && name.len() <= MAX_NAME);
    reply.extend_from_slice(&name);
    send(
        &*server,
        &reply,
        &[files[0].as_fd(), files[1].as_fd(), files[2].as_fd()],
        end,
    )
    .unwrap();
    loop {
        gate(end).unwrap();
        let status = waitid(
            Id::Pid(child_pid),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT | WaitPidFlag::WNOHANG,
        )
        .unwrap();
        gate(end).unwrap();
        match status {
            WaitStatus::StillAlive => std::thread::sleep(Duration::from_millis(1)),
            WaitStatus::Exited(pid, 0) if pid == child_pid => break,
            _ => panic!("owned child uncertain/nonzero; retained without reap or signal"),
        }
    }
    gate(end).unwrap();
    let terminal = child.wait().unwrap();
    gate(end).unwrap();
    assert_eq!(terminal.into_raw(), 0);
    assert_eq!(parent.consultations.get(), 1);
}

#[test]
fn inner_process_budget_cannot_renew_outer_grant() {
    let end = Instant::now() + Duration::from_millis(50);
    let budget = clipped_budget(end).unwrap();
    assert!(budget.until <= end);
    let long_end = Instant::now() + Duration::from_secs(8);
    let budget = clipped_budget(long_end).unwrap();
    assert!(budget.until <= long_end);
    assert!(budget.until < long_end);
    assert!(clipped_budget(Instant::now()).is_err());
    let mut expired = budget;
    expired.until = Instant::now();
    assert!(expired.check().is_err());
}

fn local_pair_receiver_shape_without_child(client_passcred: bool) -> bool {
    let end = Instant::now() + Duration::from_secs(2);
    let (server, client) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let server = Retained::new(File::from(server));
    let client = Retained::new(File::from(client));
    gate(end).unwrap();
    set_socket_passcred(&*server, true).unwrap();
    gate(end).unwrap();
    if client_passcred {
        set_socket_passcred(&*client, true).unwrap();
        gate(end).unwrap();
    }
    send(&*client, REQUEST, &[], end).unwrap();
    ready(&*server, PollFlags::IN, end).unwrap();
    let mut bytes = [0; 128];
    let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3), ScmCredentials(1))];
    let mut control = RecvAncillaryBuffer::new(&mut storage);
    let mut rights = Retained::new(Vec::new());
    let mut credentials = Vec::new();
    gate(end).unwrap();
    let result = recvmsg(
        &*server,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut control,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    for message in control.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(files) => rights.extend(files.map(File::from)),
            RecvAncillaryMessage::ScmCredentials(value) => credentials.push(value),
            other => {
                std::mem::forget(other);
                panic!("unexpected visible ancillary kind");
            }
        }
    }
    gate(end).unwrap();
    assert_eq!(&bytes[..result.bytes], REQUEST);
    assert_eq!(credentials.len(), 1);
    assert!(rights.is_empty());
    assert_eq!(credentials[0].pid, rustix::process::getpid());
    assert!((result.flags - (ReturnFlags::CMSG_CLOEXEC | ReturnFlags::EOR)).is_empty());
    // Closed HOST kernel metadata only. This new local pair is unrelated to
    // the failed owned-child attempt; no old child/socket is queried here.
    let address_present = result.address.is_some();
    gate(end).unwrap();
    address_present
}

#[test]
fn own_local_pair_receiver_shape_without_child() {
    println!(
        "OWNED_LOCAL_PAIR_ADDRESS_PRESENT={}",
        local_pair_receiver_shape_without_child(false)
    );
}

#[test]
fn own_local_pair_both_passcred_receiver_shape_without_child() {
    println!(
        "OWNED_LOCAL_PAIR_BOTH_PASSCRED_ADDRESS_PRESENT={}",
        local_pair_receiver_shape_without_child(true)
    );
}

#[test]
fn visible_frame_shapes_refuse_mismatch_without_kernel_retry() {
    let credential = UCred {
        pid: rustix::process::getpid(),
        uid: rustix::process::getuid(),
        gid: rustix::process::getgid(),
    };
    let peer = (
        credential.pid.as_raw_nonzero().get(),
        credential.uid.as_raw(),
        credential.gid.as_raw(),
    );
    let mut frame = Received {
        bytes: REQUEST.to_vec(),
        credentials: vec![credential],
        rights: Retained::new(vec![]),
        flags: ReturnFlags::empty(),
    };
    shape(&frame, peer, false).unwrap();
    assert!(shape(&frame, (peer.0 + 1, peer.1, peer.2), false).is_err());
    frame.bytes.push(0);
    assert!(shape(&frame, peer, false).is_err());
    frame.bytes = REQUEST.to_vec();
    frame.credentials.push(credential);
    assert!(shape(&frame, peer, false).is_err());
    frame.credentials.clear();
    assert!(shape(&frame, peer, false).is_err());
    frame.credentials.push(credential);
    frame.flags = ReturnFlags::CTRUNC;
    assert!(shape(&frame, peer, false).is_err());
    frame.flags = ReturnFlags::TRUNC;
    assert!(shape(&frame, peer, false).is_err());
    frame.flags = ReturnFlags::empty();
    assert!(shape(&frame, peer, true).is_err());
    assert!(gate(Instant::now()).is_err());
}
