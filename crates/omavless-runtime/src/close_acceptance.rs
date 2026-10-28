// SPDX-License-Identifier: MIT
//! Explicit acceptance-image scenarios, absent from ordinary builds.
//! One arm per owner; effect and terminal-retirement claims are disjoint.
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    BeforeAttempt,
    FinalProof,
    BetweenMembers,
    LateFinish,
    LatePreviewPublication,
}

#[derive(Clone, Copy)]
pub(crate) enum Marker {
    BeforeAttempt,
    FinalProof,
    BarrierEntered,
    BarrierReleased,
    LatePublicationReady,
    LatePublicationDelivered,
    LatePublicationRefused,
    LateFinishRefused,
}
// No Rust stderr lock and no shared-FD flag change. Journald's stream socket
// supports MSG_DONTWAIT; unsupported/full outputs drop the marker. Installed
// admission requires its exact delivered line, never an inferred emission.
pub(crate) fn marker_capable() -> bool {
    marker_capable_to(io::stderr())
}
fn marker_capable_to(fd: impl std::os::fd::AsFd) -> bool {
    rustix::net::sockopt::socket_type(&fd).is_ok_and(|kind| kind == rustix::net::SocketType::STREAM)
        && rustix::net::sockopt::socket_domain(&fd)
            .is_ok_and(|kind| kind == rustix::net::AddressFamily::UNIX)
        && rustix::net::send(
            fd,
            b"",
            rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
        )
        .is_ok_and(|n| n == 0)
}

pub(crate) fn marker(marker: Marker) -> bool {
    emit_marker_to(io::stderr(), marker)
}
fn emit_marker_to(fd: impl std::os::fd::AsFd, marker: Marker) -> bool {
    let line: &[u8] = match marker {
        Marker::BeforeAttempt => b"omavless.close-cut.v1 stage=before_attempt channel=closed\n",
        Marker::FinalProof => b"omavless.close-cut.v1 stage=final_proof channel=closed\n",
        Marker::LatePublicationReady => {
            b"omavless.close-cut.v1 stage=late_publication retired=ready\n"
        }
        Marker::LatePublicationDelivered => {
            b"omavless.close-cut.v1 stage=late_publication result=late\n"
        }
        Marker::LatePublicationRefused => {
            b"omavless.close-cut.v1 stage=late_publication owner=refused\n"
        }
        Marker::LateFinishRefused => {
            b"omavless.close-cut.v1 stage=late_finish retirement=refused\n"
        }
        Marker::BarrierEntered => b"omavless.close-cut.v1 stage=between_members barrier=entered\n",
        Marker::BarrierReleased => {
            b"omavless.close-cut.v1 stage=between_members barrier=released\n"
        }
    };
    rustix::net::send(
        fd,
        line,
        rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
    )
    .is_ok_and(|n| n == line.len())
}

pub(crate) struct Claim {
    pub(crate) scenario: Scenario,
    pub(crate) release: Option<Arc<Release>>,
}
pub(crate) struct Arm {
    scenario: Scenario,
    claimed: AtomicBool,
    release: Option<Arc<Release>>,
}
impl Arm {
    pub(crate) fn new(scenario: Scenario) -> io::Result<Self> {
        let release = if scenario == Scenario::BetweenMembers {
            Some(Arc::new(Release::new()?))
        } else {
            None
        };
        Ok(Self {
            scenario,
            claimed: AtomicBool::new(false),
            release,
        })
    }
    #[cfg(test)]
    pub(crate) fn is_claimed(&self) -> bool {
        self.claimed.load(Ordering::Acquire)
    }
    #[cfg(test)]
    pub(crate) fn release_for_test(&self) -> Arc<Release> {
        self.release.as_ref().unwrap().clone()
    }
    pub(crate) fn claim(&self) -> Option<Claim> {
        if matches!(
            self.scenario,
            Scenario::LateFinish | Scenario::LatePreviewPublication
        ) {
            return None;
        }
        self.claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Claim {
                scenario: self.scenario,
                release: self.release.clone(),
            })
    }
    pub(crate) fn claim_retirement(&self) -> Option<RetirementClaim> {
        let kind = match self.scenario {
            Scenario::LateFinish => RetirementKind::LateFinish,
            Scenario::LatePreviewPublication => RetirementKind::LatePublication,
            _ => return None,
        };
        self.claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| RetirementClaim { kind })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetirementKind {
    LateFinish,
    LatePublication,
}
// Noncloneable owner claim. No caller can create it or select a duration.
pub(crate) struct RetirementClaim {
    kind: RetirementKind,
}
impl RetirementClaim {
    pub(crate) fn kind(&self) -> RetirementKind {
        self.kind
    }
}
pub(crate) fn wait_past_original(deadline: Instant) {
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        if left.is_zero() {
            break;
        }
        std::thread::sleep(left);
    }
}

// One owned socket pair and one fixed signal handler per selected owner, not
// per preview/member. No pathname, inherited/caller FD or application payload.
// A signal can only release a test wait; it never supplies native authority.
#[cfg(test)]
pub(crate) static SIGNAL_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) struct Release {
    reader: UnixStream,
    registration: Option<signal_hook::SigId>,
    started: AtomicBool,
    entered: AtomicBool,
}
impl Release {
    fn new() -> io::Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        writer.set_nonblocking(true)?;
        let registration =
            signal_hook::low_level::pipe::register(signal_hook::consts::SIGUSR2, writer)?;
        Ok(Self {
            reader,
            registration: Some(registration),
            started: AtomicBool::new(false),
            entered: AtomicBool::new(false),
        })
    }
    #[cfg(test)]
    pub(crate) fn entered(&self) -> bool {
        self.entered.load(Ordering::Acquire)
    }
    pub(crate) fn wait(&self, deadline: Instant) -> Result<(), ()> {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        if self.started.swap(true, Ordering::AcqRel) || Instant::now() >= deadline {
            return Err(());
        }
        // A preloaded/early signal is not a qualified release of this wait.
        let mut reader = &self.reader;
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => (),
            _ => return Err(()),
        }
        let _ = marker(Marker::BarrierEntered);
        self.entered.store(true, Ordering::Release);
        loop {
            let left = deadline.checked_duration_since(Instant::now()).ok_or(())?;
            let span = Timespec {
                tv_sec: left.as_secs().try_into().map_err(|_| ())?,
                tv_nsec: left.subsec_nanos().into(),
            };
            let mut fds = [PollFd::new(&self.reader, PollFlags::IN)];
            match poll(&mut fds, Some(&span)) {
                Err(rustix::io::Errno::INTR) => continue,
                Ok(1) if fds[0].revents() == PollFlags::IN => (),
                _ => return Err(()),
            }
            if reader.read(&mut byte).map_err(|_| ())? != 1 || Instant::now() >= deadline {
                return Err(());
            }
            let _ = marker(Marker::BarrierReleased);
            return Ok(());
        }
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(id) = self.registration.take() {
            signal_hook::low_level::unregister(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn acceptance_retirement_claims_are_disjoint_one_shot_and_deadline_preserving() {
        for scenario in [Scenario::LateFinish, Scenario::LatePreviewPublication] {
            let arm = Arc::new(Arm::new(scenario).unwrap());
            assert!(arm.claim().is_none());
            assert!(!arm.is_claimed());
            let joins: Vec<_> = (0..8)
                .map(|_| {
                    let a = arm.clone();
                    std::thread::spawn(move || a.claim_retirement().is_some())
                })
                .collect();
            assert_eq!(
                joins
                    .into_iter()
                    .filter_map(|j| j.join().ok())
                    .filter(|v| *v)
                    .count(),
                1
            );
            assert!(arm.claim_retirement().is_none());
            assert!(arm.claim().is_none());
        }
        for scenario in [Scenario::BeforeAttempt, Scenario::FinalProof] {
            let arm = Arm::new(scenario).unwrap();
            assert!(arm.claim_retirement().is_none());
            assert!(!arm.is_claimed());
            assert!(arm.claim().is_some());
        }
        let deadline = Instant::now() + Duration::from_millis(5);
        wait_past_original(deadline);
        assert!(Instant::now() >= deadline);
    }
    #[test]
    fn acceptance_arm_has_one_winner_across_concurrent_captures() {
        for scenario in [Scenario::BeforeAttempt, Scenario::FinalProof] {
            let arm = Arc::new(Arm::new(scenario).unwrap());
            let joins: Vec<_> = (0..8)
                .map(|_| {
                    let arm = arm.clone();
                    std::thread::spawn(move || arm.claim().is_some())
                })
                .collect();
            assert_eq!(
                joins
                    .into_iter()
                    .map(|j| j.join().unwrap())
                    .filter(|v| *v)
                    .count(),
                1
            );
            assert!(arm.claim().is_none());
        }
    }
    #[test]
    fn acceptance_signal_barrier_releases_once_and_never_renews_expired_deadline() {
        let _signals = SIGNAL_TESTS.lock().unwrap();
        let arm = Arm::new(Scenario::BetweenMembers).unwrap();
        let claim = arm.claim().unwrap();
        let release = claim.release.unwrap();
        let waiting = release.clone();
        let join =
            std::thread::spawn(move || waiting.wait(Instant::now() + Duration::from_secs(1)));
        let until = Instant::now() + Duration::from_secs(1);
        while !release.entered.load(Ordering::Acquire) {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        signal_hook::low_level::raise(signal_hook::consts::SIGUSR2).unwrap();
        assert!(join.join().unwrap().is_ok());
        assert!(
            release
                .wait(Instant::now() + Duration::from_secs(1))
                .is_err()
        );
        assert!(arm.claim().is_none());
        let expired = Release::new().unwrap();
        assert!(
            expired
                .wait(Instant::now() - Duration::from_millis(1))
                .is_err()
        );
    }
    #[test]
    fn acceptance_barrier_refuses_preloaded_signal_eof_and_unreleased_deadline() {
        let _signals = SIGNAL_TESTS.lock().unwrap();
        let early = Release::new().unwrap();
        signal_hook::low_level::raise(signal_hook::consts::SIGUSR2).unwrap();
        assert!(
            early
                .wait(Instant::now() + Duration::from_millis(20))
                .is_err()
        );
        assert!(!early.entered());
        assert!(
            early
                .wait(Instant::now() + Duration::from_millis(20))
                .is_err()
        );
        drop(early);
        let mut closed = Release::new().unwrap();
        assert!(signal_hook::low_level::unregister(
            closed.registration.take().unwrap()
        ));
        assert_eq!((&closed.reader).read(&mut [0u8; 1]).unwrap(), 0);
        assert!(
            closed
                .wait(Instant::now() + Duration::from_millis(20))
                .is_err()
        );
        assert!(!closed.entered());
        let missing = Release::new().unwrap();
        assert!(
            missing
                .wait(Instant::now() + Duration::from_millis(5))
                .is_err()
        );
        assert!(missing.entered());
    }
    #[test]
    fn acceptance_marker_drops_full_owned_socket_without_changing_its_flags() {
        use rustix::net::{SendFlags, send};
        let (writer, _reader) = UnixStream::pair().unwrap();
        let flags = rustix::fs::fcntl_getfl(&writer).unwrap();
        let mut full = false;
        for _ in 0..1024 {
            match send(
                &writer,
                &[0u8; 8192],
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            ) {
                Err(rustix::io::Errno::AGAIN) => {
                    full = true;
                    break;
                }
                Ok(n) if n > 0 => (),
                other => panic!("unexpected owned saturation result: {other:?}"),
            }
        }
        assert!(
            full,
            "owned nonblocking test socket did not reach backpressure"
        );
        assert!(!emit_marker_to(&writer, Marker::FinalProof));
        assert_eq!(rustix::fs::fcntl_getfl(&writer).unwrap(), flags);
    }
    #[test]
    fn acceptance_marker_never_waits_for_a_contended_rust_stderr_lock() {
        let (locked_tx, locked_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let holder = std::thread::spawn(move || {
            let _lock = io::stderr().lock();
            locked_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(3));
        });
        locked_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
        let emitting = std::thread::spawn(move || {
            let _ = done_tx.send(marker(Marker::BarrierEntered));
        });
        let result = done_rx.recv_timeout(Duration::from_secs(1));
        let _ = release_tx.send(());
        holder.join().unwrap();
        emitting.join().unwrap();
        assert!(result.is_ok(), "marker waited for shared Rust stderr lock");
    }
    #[test]
    fn acceptance_marker_requires_stream_socket_and_never_claims_unsupported_delivery() {
        let (writer, mut reader) = UnixStream::pair().unwrap();
        assert!(marker_capable_to(&writer));
        assert!(emit_marker_to(&writer, Marker::FinalProof));
        let expected = b"omavless.close-cut.v1 stage=final_proof channel=closed\n";
        let mut actual = vec![0u8; expected.len()];
        reader.read_exact(&mut actual).unwrap();
        assert_eq!(actual, expected);
        let null = std::fs::File::open("/dev/null").unwrap();
        assert!(!marker_capable_to(&null));
        assert!(!emit_marker_to(&null, Marker::FinalProof));
    }
}
