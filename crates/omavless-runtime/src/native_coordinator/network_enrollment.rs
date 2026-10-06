// SPDX-License-Identifier: MIT
//! Explicit fresh developer setup, never a production/restart provisioner.

use super::*;
use crate::developer_network_resume::Clock;
#[cfg(test)]
use crate::lifecycle::LifecycleOutcome;
use crate::network_recovery_receipt::{Fence, Journal, Phase, Receipt, Refused};
use crate::network_resume::{BarrierSlot, Context, Files, RecoveryBarrier, Source, SourceOrigin};
#[cfg(test)]
use nix::fcntl::OFlag;
use omavless_store::{PrivateCreateOutcome, atomic_create_private, atomic_replace_private};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
#[cfg(test)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnchorPhase {
    Fresh,
    Awaiting,
    Consumed,
}
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FirstPermitFault {
    AnchorBefore,
    AnchorAfter,
    ReadyBefore,
    ReadyAfter,
    Readback,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Anchor {
    schema: u8,
    setup: u64,
    phase: AnchorPhase,
    fence: Option<Fence>,
}

// No Clone/Debug/deserialize/from-path constructor. Retain the original
// create-exclusive directory inode, not a claim based on its current name.
pub(crate) struct FreshSetupAuthority {
    base: PathBuf,
    directory: File,
    anchor: Anchor,
    #[cfg(test)]
    pub(crate) fault: Option<FirstPermitFault>,
}
impl FreshSetupAuthority {
    #[cfg(test)]
    pub(crate) fn create() -> Result<Self, Refused> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let setup = NEXT.fetch_add(1, Ordering::Relaxed);
        if setup == 0 || setup == u64::MAX {
            return Err(Refused);
        }
        let base = std::env::temp_dir().join(format!("ce-{}-{setup}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .map_err(|_| Refused)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags((OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW).bits())
            .open(&base)
            .map_err(|_| Refused)?;
        for child in ["runtime", "state", "config", "state/omavless"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(child))
                .map_err(|_| Refused)?;
        }
        let authority = Self {
            base,
            directory,
            anchor: Anchor {
                schema: 1,
                setup,
                phase: AnchorPhase::Fresh,
                fence: None,
            },
            fault: None,
        };
        let uid = nix::unistd::geteuid().as_raw();
        if atomic_create_private(
            &authority.path(),
            &serde_json::to_vec(&authority.anchor).map_err(|_| Refused)?,
            uid,
        )
        .map_err(|_| Refused)?
            != PrivateCreateOutcome::Created
        {
            return Err(Refused);
        }
        authority.verify(uid)?;
        Ok(authority)
    }
    #[cfg(test)]
    pub(crate) fn base(&self) -> &Path {
        &self.base
    }
    fn path(&self) -> PathBuf {
        self.base
            .join("state/omavless/network-resume-enrollment.json")
    }
    fn verify(&self, uid: u32) -> Result<(), Refused> {
        let original = self.directory.metadata().map_err(|_| Refused)?;
        let now = fs::symlink_metadata(&self.base).map_err(|_| Refused)?;
        if !now.is_dir()
            || now.file_type().is_symlink()
            || now.uid() != uid
            || now.mode() & 0o7777 != 0o700
            || original.dev() != now.dev()
            || original.ino() != now.ino()
        {
            return Err(Refused);
        }
        let path = self.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| Refused)?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.uid() != uid
            || meta.mode() & 0o7777 != 0o600
            || meta.len() > 1024
        {
            return Err(Refused);
        }
        let raw = omavless_store::read_private_utf8(&path, uid).map_err(|_| Refused)?;
        let record: Anchor = serde_json::from_str(&raw).map_err(|_| Refused)?;
        if record != self.anchor {
            return Err(Refused);
        }
        Ok(())
    }
    fn advance(&mut self, phase: AnchorPhase, fence: Fence, uid: u32) -> Result<(), Refused> {
        self.verify(uid)?;
        #[cfg(test)]
        if phase == AnchorPhase::Consumed && self.fault == Some(FirstPermitFault::AnchorBefore) {
            return Err(Refused);
        }
        let next = Anchor {
            phase,
            fence: Some(fence),
            ..self.anchor
        };
        atomic_replace_private(
            &self.path(),
            &serde_json::to_vec(&next).map_err(|_| Refused)?,
            uid,
        )
        .map_err(|_| Refused)?;
        #[cfg(test)]
        if phase == AnchorPhase::Consumed && self.fault == Some(FirstPermitFault::AnchorAfter) {
            return Err(Refused);
        }
        self.anchor = next;
        self.verify(uid)
    }
}

pub(crate) struct AwaitingConnect {
    setup: FreshSetupAuthority,
    pub(super) context: Context,
    pub(super) last_tick: u64,
    source: SourceOrigin,
}
struct CompletedConnectAuthority {
    awaiting: Box<AwaitingConnect>,
    context: Context,
}

type Complete<H> = fn(
    &mut OfflineNativeCoordinator<H>,
    &MigrationLock,
    &mut Source,
    &Clock,
    Box<AwaitingConnect>,
    &NativeOwnerExecution,
) -> Result<(), Refused>;
pub(super) struct ConnectEnrollmentBorrow<'a, H> {
    source: &'a mut Source,
    clock: &'a Clock,
    complete: Complete<H>,
}
impl<'a, H: network_resume::ResumeBinding> ConnectEnrollmentBorrow<'a, H> {
    pub(super) fn new(source: &'a mut Source, clock: &'a Clock) -> Self {
        Self {
            source,
            clock,
            complete: complete::<H>,
        }
    }
}
impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    #[cfg(test)]
    pub(crate) fn install_awaiting_connect_locked(
        &mut self,
        lease: &MigrationLock,
        mut setup: FreshSetupAuthority,
        enrollment: crate::developer_network_resume::Enrollment,
        source: &mut Source,
        tick: u64,
    ) -> Result<LifecycleOutcome, Refused> {
        let crate::developer_network_resume::Enrollment {
            boot,
            instance,
            epoch,
        } = enrollment;
        if self.resume_barrier.installed() {
            return Err(Refused);
        }
        self.resume_barrier = BarrierSlot::Blocked;
        self.transaction
            .lifecycle_mut()
            .install_network_recovery_guard()
            .map_err(|_| Refused)?;
        let context = self.resume_context_locked(lease, boot, instance, epoch)?;
        let expected_desired = crate::desired::DesiredPaths::below(&setup.base.join("state"));
        let expected_cutover = crate::cutover::CutoverPaths::below(
            &setup.base.join("runtime"),
            &setup.base.join("state"),
            self.uid(),
        );
        if !context.fence.valid()
            || context.desired.connected
            || self.actual() != ActualState::Disconnected
            || self.transaction.desired_paths() != &expected_desired
            || self.store_path() != setup.base.join("config/profiles.json")
            || self.transaction.cutover_paths().runtime_base != expected_cutover.runtime_base
            || self.transaction.cutover_paths().state_directory != expected_cutover.state_directory
            || self.transaction.cutover_paths().operation_lock != expected_cutover.operation_lock
            || self.transaction.cutover_paths().ownership_marker
                != expected_cutover.ownership_marker
            || !source.matches(context.fence)
        {
            return Err(Refused);
        }
        source.quiescent()?;
        let raw = omavless_store::read_private_utf8(self.store_path(), self.uid())
            .map_err(|_| Refused)?;
        let store =
            omavless_domain::private_store::parse_private_store(&raw).map_err(|_| Refused)?;
        if store.active_profile_id().is_some() {
            return Err(Refused);
        }
        let receipt = self
            .transaction
            .desired_paths()
            .directory
            .join("network-resume-receipt.json");
        if !matches!(fs::symlink_metadata(&receipt), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            return Err(Refused);
        }
        setup.verify(self.uid())?;
        setup.advance(AnchorPhase::Awaiting, context.fence, self.uid())?;
        let outcome = self
            .transaction
            .lifecycle_mut()
            .observe_network_only(&context.desired)
            .map_err(|_| Refused)?;
        let after = self.resume_context_locked(lease, boot, instance, epoch)?;
        if after.fence != context.fence
            || after.desired != context.desired
            || after.store_digest != context.store_digest
            || outcome.actual != ActualState::Disconnected
            || outcome.changed
        {
            return Err(Refused);
        }
        source.quiescent()?;
        self.resume_barrier = BarrierSlot::AwaitingConnect(Box::new(AwaitingConnect {
            setup,
            context,
            last_tick: tick,
            source: source.origin(),
        }));
        Ok(outcome)
    }
    pub(super) fn begin_connect_enrollment(
        &mut self,
        lease: &MigrationLock,
    ) -> Option<Box<AwaitingConnect>> {
        if !matches!(self.resume_barrier, BarrierSlot::AwaitingConnect(_)) {
            return None;
        }
        let BarrierSlot::AwaitingConnect(awaiting) =
            std::mem::replace(&mut self.resume_barrier, BarrierSlot::InFlight)
        else {
            return None;
        };
        let expected = &awaiting.context;
        let current = self.resume_context_locked(
            lease,
            expected.fence.boot,
            expected.fence.owner_instance,
            expected.fence.network_epoch,
        );
        if current.is_ok_and(|c| {
            c.fence == expected.fence
                && c.desired == expected.desired
                && c.store_digest == expected.store_digest
        }) && !expected.desired.connected
            && self.actual() == ActualState::Disconnected
            && awaiting.setup.verify(self.uid()).is_ok()
        {
            Some(awaiting)
        } else {
            self.resume_barrier = BarrierSlot::Blocked;
            None
        }
    }
    pub(super) fn complete_connect_enrollment(
        &mut self,
        lease: &MigrationLock,
        awaiting: Option<Box<AwaitingConnect>>,
        borrow: Option<ConnectEnrollmentBorrow<'_, H>>,
        result: &Result<NativeOwnerExecution, NativeOwnerError>,
    ) {
        let Some(awaiting) = awaiting else {
            return;
        };
        let succeeded = match (borrow, result) {
            (Some(borrow), Ok(result)) => {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (borrow.complete)(self, lease, borrow.source, borrow.clock, awaiting, result)
                }))
                .is_ok_and(|result| result.is_ok())
            }
            _ => false,
        };
        if !succeeded {
            self.resume_barrier = BarrierSlot::Blocked;
        }
    }
    pub(super) fn cancel_connect_enrollment(&mut self) {
        if matches!(self.resume_barrier, BarrierSlot::AwaitingConnect(_)) {
            self.resume_barrier = BarrierSlot::Cancelled;
        }
    }
    pub(super) fn poll_awaiting_connect(&mut self, source: &mut Source, tick: u64) -> bool {
        let BarrierSlot::AwaitingConnect(awaiting) = &self.resume_barrier else {
            return false;
        };
        let fence = awaiting.context.fence;
        if tick < awaiting.last_tick
            || !source.same_origin(&awaiting.source)
            || !source.matches(fence)
            || source.drain_for_enrollment().is_err()
        {
            self.resume_barrier = BarrierSlot::Blocked;
            return true;
        }
        let current = self.resume_lease().and_then(|lease| {
            self.resume_context_locked(
                &lease,
                fence.boot,
                fence.owner_instance,
                fence.network_epoch,
            )
        });
        match current {
            Err(_) => self.resume_barrier = BarrierSlot::Blocked,
            Ok(current)
                if !(current.fence == awaiting.context.fence
                    && current.desired == awaiting.context.desired
                    && current.store_digest == awaiting.context.store_digest) =>
            {
                self.resume_barrier = BarrierSlot::Cancelled
            }
            Ok(_) => {
                if let BarrierSlot::AwaitingConnect(awaiting) = &mut self.resume_barrier {
                    awaiting.last_tick = tick;
                }
            }
        }
        true
    }
}

fn complete<H: network_resume::ResumeBinding>(
    owner: &mut OfflineNativeCoordinator<H>,
    lease: &MigrationLock,
    source: &mut Source,
    clock: &Clock,
    awaiting: Box<AwaitingConnect>,
    execution: &NativeOwnerExecution,
) -> Result<(), Refused> {
    let NativeOwnerExecution::Applied {
        cached,
        outcome: Ok(NativeMutationOutcome::Connection(outcome)),
    } = execution
    else {
        return Err(Refused);
    };
    let old = awaiting.context.fence;
    if !source.same_origin(&awaiting.source) || clock.now() < awaiting.last_tick {
        return Err(Refused);
    }
    if !outcome.changed
        || cached.error.is_some()
        || old.desired_revision.checked_add(1) != Some(cached.revision)
        || owner.revision() != cached.revision
        || owner.actual() != ActualState::Connected
    {
        return Err(Refused);
    }
    let context =
        owner.resume_context_locked(lease, old.boot, old.owner_instance, old.network_epoch)?;
    if !context.desired.connected
        || context.fence.desired_revision != cached.revision
        || !source.matches(context.fence)
    {
        return Err(Refused);
    }
    let mut authority = CompletedConnectAuthority { awaiting, context };
    source.drain_for_enrollment()?;
    let mut port = network_resume::Port::new(owner, lease, &authority.context, clock.now());
    port.verify_healthy_binding()?;
    source.quiescent()?; // No further drain after the fresh complete observation.
    authority.awaiting.setup.advance(
        AnchorPhase::Consumed,
        authority.context.fence,
        owner.uid(),
    )?;
    source.quiescent()?;
    let path = owner
        .transaction
        .desired_paths()
        .directory
        .join("network-resume-receipt.json");
    let ready = Receipt {
        schema: 1,
        fence: authority.context.fence,
        phase: Phase::Ready,
    };
    #[cfg(test)]
    if authority.awaiting.setup.fault == Some(FirstPermitFault::ReadyBefore) {
        return Err(Refused);
    }
    if atomic_create_private(
        &path,
        &serde_json::to_vec(&ready).map_err(|_| Refused)?,
        owner.uid(),
    )
    .map_err(|_| Refused)?
        != PrivateCreateOutcome::Created
    {
        return Err(Refused);
    }
    #[cfg(test)]
    if authority.awaiting.setup.fault == Some(FirstPermitFault::ReadyAfter) {
        return Err(Refused);
    }
    let mut files = Files {
        receipt: &path,
        lease,
        paths: owner.transaction.cutover_paths(),
        uid: owner.uid(),
        fail_before: false,
        fail_after: false,
        fault_phase: None,
    };
    if files.load()? != ready {
        return Err(Refused);
    }
    #[cfg(test)]
    if authority.awaiting.setup.fault == Some(FirstPermitFault::Readback) {
        return Err(Refused);
    }
    source.quiescent()?;
    let current =
        owner.resume_context_locked(lease, old.boot, old.owner_instance, old.network_epoch)?;
    if current.fence != authority.context.fence
        || current.desired != authority.context.desired
        || current.store_digest != authority.context.store_digest
    {
        return Err(Refused);
    }
    authority.awaiting.setup.verify(owner.uid())?;
    owner.resume_barrier = BarrierSlot::Installed(Box::new(RecoveryBarrier::new(
        authority.context,
        path,
        clock.now(),
    )?));
    Ok(())
}
