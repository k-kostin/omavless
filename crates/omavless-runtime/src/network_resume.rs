// SPDX-License-Identifier: MIT
//! Executable developer continuation: authenticated owned Unix fixture source,
//! bounded event owner, durable receipt, and the original coordinator port.
//! Test/developer feature only. Nothing subscribes to a real host event bus.

use crate::desired::DesiredState;
use crate::network_recovery_receipt::{
    Admission, Fence, Journal, Observation, Phase, Receipt, Refused, decode,
};
use crate::network_transition_plan::{self as plan, Attempt, Decision, Hint};
use nix::sys::socket::{MsgFlags, getsockopt, recv, sockopt::PeerCredentials};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

/// Fixed isolated developer scenario only. No caller-selected path, command,
/// endpoint, event source, profile or host callback is accepted. This seeds
/// Ready solely inside a newly created owned synthetic fixture directory.
#[cfg(feature = "network-resume-fixture")]
pub fn developer_network_resume_fixture() -> serde_json::Value {
    fixture::run_developer_fixture()
}

pub(crate) struct Context {
    pub(crate) fence: Fence,
    // Never formatted or released in status; captures exact profile/mode/intent.
    pub(crate) desired: DesiredState,
    pub(crate) store_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Kind {
    Suspend,
    Resume,
    NetworkChanged,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    sequence: u64,
    kind: Kind,
}

/// Fixture attribution only: kernel-reported peer PID/UID, a pinned owned
/// stream, and trusted receiver boot/instance. PID is not process-lifetime proof
/// and this does not establish logind/NetworkManager/netlink authenticity.
struct Source {
    reader: BufReader<UnixStream>,
    boot: [u8; 16],
    instance: [u8; 16],
    sequence: u64,
    lost: bool,
    #[cfg(test)]
    after_newline_pause: Duration,
}

impl Source {
    fn quiescent(&mut self) -> Result<(), Refused> {
        if self.lost {
            return Err(Refused);
        }
        // The effect boundary must see a live, completely drained source.
        // A queued event is not silently skipped while recovering an old hint.
        let mut byte = [0; 1];
        let ready = self.reader.buffer().is_empty()
            && matches!(
                recv(
                    self.reader.get_ref().as_raw_fd(),
                    &mut byte,
                    MsgFlags::MSG_PEEK | MsgFlags::MSG_DONTWAIT
                ),
                Err(nix::errno::Errno::EAGAIN)
            );
        if !ready {
            self.lost = true;
            return Err(Refused);
        }
        Ok(())
    }
    fn owned(stream: UnixStream, pid: u32, uid: u32, context: &Context) -> Result<Self, Refused> {
        let peer = getsockopt(&stream, PeerCredentials).map_err(|_| Refused)?;
        if peer.pid() as u32 != pid
            || peer.uid() != uid
            || context.fence.boot == [0; 16]
            || context.fence.owner_instance == [0; 16]
        {
            return Err(Refused);
        }
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|_| Refused)?;
        Ok(Self {
            reader: BufReader::new(stream),
            boot: context.fence.boot,
            instance: context.fence.owner_instance,
            sequence: 0,
            lost: false,
            #[cfg(test)]
            after_newline_pause: Duration::ZERO,
        })
    }

    fn next(&mut self) -> Result<Option<Kind>, Refused> {
        if self.lost {
            return Err(Refused);
        }
        let result = (|| {
            let mut bytes = Vec::new();
            let deadline = Instant::now() + Duration::from_millis(100);
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() || bytes.len() == 256 {
                    return Err(Refused);
                }
                self.reader
                    .get_ref()
                    .set_read_timeout(Some(remaining))
                    .map_err(|_| Refused)?;
                let mut byte = [0; 1];
                if self.reader.read(&mut byte).map_err(|_| Refused)? != 1 {
                    return Err(Refused);
                }
                #[cfg(test)]
                if byte[0] == b'\n' && !self.after_newline_pause.is_zero() {
                    std::thread::sleep(self.after_newline_pause);
                }
                // A final byte arriving around timeout rounding or a paused
                // reader cannot be admitted after the original frame budget.
                if Instant::now() >= deadline {
                    return Err(Refused);
                }
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            let frame: Frame = serde_json::from_slice(&bytes).map_err(|_| Refused)?;
            if frame.sequence == 0 {
                return Err(Refused);
            }
            // A duplicate or reordered reply is discarded, not a newer epoch.
            if frame.sequence <= self.sequence {
                return Ok(None);
            }
            if self.sequence.checked_add(1) != Some(frame.sequence) {
                return Err(Refused);
            }
            self.sequence = frame.sequence;
            Ok(Some(frame.kind))
        })();
        if result.is_err() {
            self.lost = true;
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Idle,
    Paused,
    Checking,
    Cancelled,
    ObserveOnly,
    Recovered,
    ManualRecovery,
    SourceUnavailable,
}

struct Pending {
    hint: Hint,
    first_tick: u64,
}

/// Construct once for one owner/established Ready fence. There is no rearm or
/// source-reconnect method. Stable-epoch provisioning is outside this fixture.
struct EventOwner {
    fence: Fence,
    admission: Admission,
    pending: Option<Pending>,
    last_tick: Option<u64>,
    paused: bool,
    status: Status,
}

impl EventOwner {
    fn status_projection(&self) -> serde_json::Value {
        // Fixed coarse state only; no boot/owner IDs, profile, digest, SSID,
        // address, raw event payload or claim of DNS/route health.
        serde_json::json!({"schema":1,"state":self.status})
    }
    fn new(context: &Context) -> Result<Self, Refused> {
        Ok(Self {
            fence: context.fence,
            admission: Admission::new(context.fence)?,
            pending: None,
            last_tick: None,
            paused: false,
            status: Status::Idle,
        })
    }

    fn available(&mut self, tick: u64) -> bool {
        if matches!(
            self.status,
            Status::ManualRecovery | Status::SourceUnavailable | Status::Recovered
        ) || self.last_tick.is_some_and(|previous| tick < previous)
        {
            self.pending = None;
            if self.last_tick.is_some_and(|previous| tick < previous) {
                self.status = Status::SourceUnavailable;
            }
            return false;
        }
        self.last_tick = Some(tick);
        true
    }

    fn receive(&mut self, source: &mut Source, context: &Context, tick: u64) {
        if !self.available(tick) {
            return;
        }
        if source.boot != self.fence.boot || source.instance != self.fence.owner_instance {
            self.pending = None;
            self.status = Status::SourceUnavailable;
            return;
        }
        let kind = match source.next() {
            Ok(Some(kind)) => kind,
            Ok(None) => return,
            Err(_) => {
                self.pending = None;
                self.status = Status::SourceUnavailable;
                return;
            }
        };
        if context.fence != self.fence || !context.desired.connected {
            self.pending = None;
            self.status = Status::Cancelled;
            return;
        }
        if kind == Kind::Suspend {
            self.pending = None;
            self.paused = true;
            self.status = Status::Paused;
            return;
        }
        if self.paused && kind != Kind::Resume {
            return;
        }
        self.paused = false;
        let first_tick = self
            .pending
            .as_ref()
            .map_or(tick, |pending| pending.first_tick);
        self.pending = Some(Pending {
            first_tick,
            hint: Hint {
                owner_generation: self.fence.owner_generation,
                desired_revision: self.fence.desired_revision,
                network_epoch: self.fence.network_epoch,
                last_hint_tick: tick,
            },
        });
        self.status = Status::Checking;
    }

    fn poll(
        &mut self,
        source: &mut Source,
        tick: u64,
        journal: &mut impl Journal,
        port: &mut impl Observation,
    ) {
        if !self.available(tick) || self.paused {
            return;
        }
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        if tick
            .checked_sub(pending.first_tick)
            .is_none_or(|age| age > plan::MAX_HINT_AGE_SECS)
        {
            self.pending = None;
            self.status = Status::Cancelled;
            return;
        }
        let hint = pending.hint;
        if tick < hint.last_hint_tick.saturating_add(plan::QUIET_SECS) {
            return;
        }
        if source.boot != self.fence.boot
            || source.instance != self.fence.owner_instance
            || source.quiescent().is_err()
        {
            self.pending = None;
            self.status = Status::SourceUnavailable;
            return;
        }
        let mut port = SourcePort { source, port };
        // Read actual current intent only after stabilization. This current
        // receipt proof is advisory; Admission rechecks under the SAME lease.
        let Ok((fence, mut current)) = port.current() else {
            self.pending = None;
            self.status = Status::ManualRecovery;
            return;
        };
        if fence != self.fence {
            self.pending = None;
            self.status = Status::Cancelled;
            return;
        }
        current.attempt = match journal.load() {
            Ok(receipt)
                if receipt.schema == 1
                    && receipt.fence == self.fence
                    && receipt.phase == Phase::Ready =>
            {
                Attempt::NoneProven
            }
            _ => Attempt::OutcomeUnknown,
        };
        self.status = match plan::plan(hint, current) {
            Decision::WaitUntil(_) => return,
            Decision::Off | Decision::Stale => Status::Cancelled,
            Decision::ObserveOnly => Status::ObserveOnly,
            Decision::ManualRecovery => Status::ManualRecovery,
            Decision::CandidateOnce => {
                if self.admission.attempt(hint, journal, &mut port).is_ok() {
                    Status::Recovered
                } else {
                    Status::ManualRecovery
                }
            }
        };
        self.pending = None;
    }
}

struct SourcePort<'a, P> {
    source: &'a mut Source,
    port: &'a mut P,
}
impl<P: Observation> Observation for SourcePort<'_, P> {
    fn current(&mut self) -> Result<(Fence, plan::Current), Refused> {
        self.source.quiescent()?;
        self.port.current()
    }
    fn synthetic_effect(&mut self) -> Result<(), Refused> {
        self.source.quiescent()?;
        self.port.synthetic_effect()
    }
}

/// Only used below a trusted exclusive fixture directory and under the original
/// MigrationLock. It intentionally does not claim pinned-dirfd/power-loss or
/// rollback-resistant production provisioning. Absent state is never seeded.
struct Files<'a> {
    receipt: &'a Path,
    lease: &'a crate::cutover::MigrationLock,
    paths: &'a crate::cutover::CutoverPaths,
    uid: u32,
    fail_before: bool,
    fail_after: bool,
    fault_phase: Option<Phase>,
}

impl Journal for Files<'_> {
    fn load(&mut self) -> Result<Receipt, Refused> {
        if !self.lease.authorizes(self.paths, self.uid) {
            return Err(Refused);
        }
        // Bound allocation before entering the general private-store reader.
        let metadata = std::fs::symlink_metadata(self.receipt).map_err(|_| Refused)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != self.uid
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.len() > 1024
        {
            return Err(Refused);
        }
        let raw = omavless_store::read_private_utf8(self.receipt, self.uid).map_err(|_| Refused)?;
        decode(raw.as_bytes())
    }
    fn replace_synced(&mut self, expected: Receipt, next: Receipt) -> Result<(), Refused> {
        let inject = self.fault_phase.is_none_or(|phase| phase == next.phase);
        if self.load()? != expected || (inject && self.fail_before) {
            return Err(Refused);
        }
        omavless_store::atomic_replace_private(
            self.receipt,
            &serde_json::to_vec(&next).map_err(|_| Refused)?,
            self.uid,
        )
        .map_err(|_| Refused)?;
        if (inject && self.fail_after) || self.load()? != next {
            return Err(Refused);
        }
        Ok(())
    }
}

mod fixture;
