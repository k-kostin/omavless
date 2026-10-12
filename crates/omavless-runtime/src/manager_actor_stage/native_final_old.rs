// SPDX-License-Identifier: MIT
//! Fresh closure-only OLD qualification inside the SAME native engine ledger.
//! Records are comparison DATA; no candidate mutation stack is activated.
use super::*;
#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
#[path = "native_final_old_tests.rs"]
mod tests;

impl NativeEngine {
    pub(super) fn final_old_observation_captured(&self) -> bool {
        self.recovery
            && self.singleton_locked
            && !self.sealed
            && self.recovery_completion == RecoveryCompletionSelection::FinalOld
            && self.final_old_captured
            && self.stage_name.is_none()
            && self.lower.original[Slot::Scratch3 as usize].is_some()
            && self.expected[Slot::Scratch3 as usize]
                .is_some_and(|(size, _)| size == crate::restore_closure_model::RECORD_BYTES)
            && self.lower.original[Slot::Scratch2 as usize].is_none()
            && self.lower.io.original(Slot::Scratch2).is_err()
            && self.expected[Slot::Scratch2 as usize].is_none()
            && !self.unlinked[Slot::Scratch2 as usize]
    }

    pub(super) fn final_old_qualified(&self) -> bool {
        self.final_old_observation_captured()
            && self.fresh_final_old.as_ref().is_some_and(|qualified| {
                self.expected[Slot::Scratch3 as usize]
                    == Some((
                        crate::restore_closure_model::RECORD_BYTES,
                        qualified.closure_digest,
                    ))
            })
    }

    pub(super) fn check_final_old_retired_sources(
        &self,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if self.lower.live != [LiveRole::Old; 2]
            || self.current_store
            || self.stage_name.is_some()
            || self.lower.original[Slot::StageDirectory as usize].is_some()
            || self.lower.original[Slot::Intent as usize].is_some()
            || self.lower.original[Slot::Terminal as usize].is_some()
            || self.lower.original[Slot::Scratch2 as usize].is_some()
            || self.lower.io.original(Slot::Scratch2).is_ok()
            || self.expected[Slot::Scratch2 as usize].is_some()
            || self.unlinked[Slot::Scratch2 as usize]
        {
            return Err(Unavailable);
        }
        let state = self.lower.io.original(Slot::State)?;
        for name in [
            PENDING_DIRECTORY,
            INTENT,
            TERMINAL,
            crate::restore_retirement_candidate::RECEIPT_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
            "routing-preset.pending.json",
        ] {
            tick(until)?;
            if !matches!(
                fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(Unavailable);
            }
        }
        let config = self.lower.io.original(Slot::Config)?;
        for (_, name) in NATIVE_REPLACEMENTS.into_iter().chain(NATIVE_ROLLBACKS) {
            tick(until)?;
            if !matches!(
                fstatat(config, name, AtFlags::AT_SYMLINK_NOFOLLOW),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(Unavailable);
            }
        }
        Ok(())
    }

    fn capture_final_old_data(
        &mut self,
        new: [&[u8]; 2],
        generation: u64,
        desired: &[u8],
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.recovery
            || self.sealed
            || !self.singleton_locked
            || self.recovery_completion != RecoveryCompletionSelection::FinalOld
            || self.final_old_captured
            || self.completed
            || self.retirement
        {
            return Err(FirstError::Admission);
        }
        for slot in [Slot::Owner, Slot::Desired, Slot::Login] {
            self.lower
                .io
                .original(slot)
                .map_err(|_| FirstError::Admission)?;
            if self.lower.original[slot as usize].is_none() {
                return Err(FirstError::Admission);
            }
        }
        self.check_final_old_retired_sources(until)
            .map_err(|_| FirstError::Admission)?;
        let raw = self.recover_member(
            Slot::State,
            Slot::Scratch3,
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::RECORD_BYTES,
            until,
        )?;
        let closure = crate::restore_closure_model::ClosureRecord::decode(&raw)
            .map_err(|_| FirstError::Admission)?;
        let old_store = self.read_original_live(0, until)?;
        let old_template = self.read_original_live(1, until)?;
        if closure.receipt().terminal().phase() != DecisionPhase::Aborted
            || !closure
                .receipt()
                .terminal()
                .matches_owner_desired(generation, Some(desired))
            || !crate::restore_executor_candidate::successor::rotation::final_review::output_binding(
                closure.receipt(),
                [old_store.as_slice(), old_template.as_slice()],
                new,
            )
        {
            return Err(FirstError::Admission);
        }
        self.scan_catalogue(Slot::Config, Some(0), &[], until)
            .map_err(|_| FirstError::Admission)?;
        self.scan_catalogue(Slot::State, Some(1), &[], until)
            .map_err(|_| FirstError::Admission)?;
        self.catalogues_captured = true;
        // This first finite route admits NO previous/torn/completed history or
        // other restore prefix. Later history/prefix recovery is separate scope.
        let state = &self.catalogues[1];
        if !(0..state.count).all(|index| {
            let name = &state.names[index][..state.lengths[index]];
            !(name.starts_with(b"restore-") || name.starts_with(b".restore-"))
                || name == crate::restore_closure_model::CLOSURE_MEMBER.as_bytes()
        }) || !(0..self.catalogues[0].count).all(|index| {
            let config = &self.catalogues[0];
            let name = &config.names[index][..config.lengths[index]];
            !(name.starts_with(b"restore-") || name.starts_with(b".restore-"))
        }) {
            return Err(FirstError::Admission);
        }
        self.check_final_old_retired_sources(until)
            .map_err(|_| FirstError::Admission)?;
        self.capture_history(until)?; // capacity/free target BEFORE Stop or publication
        if self.audit.target().map_err(|_| FirstError::Admission)? != native_history::NAMES[0] {
            return Err(FirstError::Admission);
        }
        self.check_original_bytes(until)?;
        self.check_bindings(until)
            .map_err(|_| FirstError::Admission)?;
        self.final_old_captured = true; // only the pinned closure observation exception
        Ok(())
    }

    fn qualify_final_old_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.final_old_observation_captured()
            || self.completed
            || self.retirement
            || self.fresh_final_old.is_some()
            || !self.audit.attempted()
        {
            return Err(FirstError::Admission);
        }
        self.gate(origin, until)?;
        self.retire_recovery_endpoint(origin, cut, until)?;
        // Syntax/checksum is not old fsync proof. Newly sync every pinned source
        // and its held parents under this invocation's guard and original gates.
        for (index, slot) in [
            Slot::OldStore,
            Slot::OldTemplate,
            Slot::Scratch3,
            Slot::Owner,
            Slot::Desired,
            Slot::Login,
            Slot::Config,
            Slot::State,
            Slot::Root,
            Slot::Run,
        ]
        .into_iter()
        .enumerate()
        {
            self.gate(origin, until)?;
            self.lower
                .io
                .perform(
                    slot,
                    || tick(until),
                    |file| file.sync_all().map_err(|_| Unavailable),
                )
                .map_err(|_| FirstError::StillFenced)?;
            cut(NativeStep::FinalOldSourceSynced(index))?;
            self.gate(origin, until)?;
        }
        self.check_final_old_retired_sources(until)
            .map_err(|_| FirstError::StillFenced)?;
        self.gate(origin, until)?;
        let (size, closure_digest) =
            self.expected[Slot::Scratch3 as usize].ok_or(FirstError::StillFenced)?;
        if size != crate::restore_closure_model::RECORD_BYTES {
            return Err(FirstError::StillFenced);
        }
        // No fallible work between final original checks and this concrete new
        // qualification. Never represent the missing receipt as an unlinked FD.
        self.fresh_final_old = Some(FreshFinalOld { closure_digest });
        self.retirement = true;
        self.completed = true;
        Ok(())
    }

    pub(crate) fn qualify_native_final_old(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        backup: &omavless_domain::private_backup::OpenedBackup,
        quiescence: &mut NativeOldRecoveryQuiescence,
    ) -> Result<(), FirstError> {
        if crate::product_scope::backup_only() {
            return Err(FirstError::Admission);
        }
        if self.sealed || self.uid.is_some() {
            return Err(FirstError::StillFenced);
        }
        self.recovery = true;
        self.recovery_completion = RecoveryCompletionSelection::FinalOld;
        let until = Instant::now() + std::time::Duration::from_secs(45);
        let result = (|| {
            self.uid = Some(origin.uid());
            self.gid = Some(nix::unistd::getgid().as_raw());
            self.lower
                .io
                .native_admit()
                .map_err(|_| FirstError::Admission)?;
            for (index, slot) in [Slot::Config, Slot::State, Slot::Run]
                .into_iter()
                .enumerate()
            {
                self.clone_file(slot, origin.directory(index)?, true, until)
                    .map_err(|_| FirstError::Admission)?;
            }
            self.capture_recovery_singleton(until)?;
            origin.check_old_recovery_lease()?;
            for (index, slot) in [Slot::Owner, Slot::Desired, Slot::Login]
                .into_iter()
                .enumerate()
            {
                // This initial closure-only route requires the packaged Login
                // boundary, not fabricated receipt/optional-absence admission.
                let file = origin.member(index)?.ok_or(FirstError::Admission)?;
                self.clone_file(slot, file, false, until)
                    .map_err(|_| FirstError::Admission)?;
            }
            for (index, (slot, _)) in LIVE.into_iter().enumerate() {
                self.clone_file(slot, origin.live(index)?, false, until)
                    .map_err(|_| FirstError::Admission)?;
            }
            let off = backup
                .restore_store_off()
                .map_err(|_| FirstError::Prepare)?;
            self.capture_final_old_data(
                [&off, backup.template()],
                origin.generation(),
                origin.desired_bytes(),
                until,
            )?;
            origin.check_old_recovery_lease()?;
            self.check_bindings(until)
                .map_err(|_| FirstError::Admission)?;
            quiescence.qualify_after_exclusive(self.view())?;
            origin.check_old_recovery_lease()?;
            self.check_bindings(until)
                .map_err(|_| FirstError::Admission)?;
            self.qualify_final_old_inner(
                &mut NativeRecoveryGate {
                    origin,
                    quiescence: Some(quiescence),
                },
                &mut |_| Ok(()),
                until,
            )
        })();
        if result.is_err() {
            self.revoke_native();
        }
        result
    }
}
