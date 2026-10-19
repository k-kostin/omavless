// SPDX-License-Identifier: MIT
//! Closed Complete-present NEW shapes. Records are comparison DATA only.
use super::*;
use crate::restore_closure_model::{CLOSURE_MEMBER, RECORD_BYTES as CLOSURE_BYTES};
use crate::restore_disposition_complete_model::{COMPLETE_BYTES, COMPLETE_MEMBER, CompleteRecord};
use crate::restore_disposition_ticket_model::{TICKET_BYTES, TICKET_MEMBER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewCompletePresence {
    ClosureTicketComplete,
    TicketComplete,
    CompleteOnly,
}
impl NewCompletePresence {
    pub(super) fn closure(self) -> bool {
        self == Self::ClosureTicketComplete
    }
    pub(super) fn ticket(self) -> bool {
        self != Self::CompleteOnly
    }
}
pub(super) struct ObservedNewComplete {
    presence: NewCompletePresence,
    complete: [u8; 32],
    closure: [u8; 32],
    ticket: [u8; 32],
    captured: bool,
}
pub(super) enum NativeCompleteRole {
    None,
    Observed(ObservedNewComplete),
    OwnedCut,
}
pub(super) struct FreshNewComplete {
    presence: NewCompletePresence,
    complete: [u8; 32],
}

impl NativeEngine {
    fn complete_presence(&self) -> Result<NewCompletePresence, Unavailable> {
        match &self.complete_role {
            NativeCompleteRole::Observed(data) => Ok(data.presence),
            _ => Err(Unavailable),
        }
    }
    pub(super) fn complete_member_observed(&self, slot: Slot) -> bool {
        match &self.complete_role {
            NativeCompleteRole::None => false,
            NativeCompleteRole::OwnedCut => {
                !self.recovery
                    && !self.sealed
                    && self.committed
                    && self.retirement
                    && self.retirement_permission == NativeRetirementPermission::CompletedCutOnly
                    && !self.completed
                    && !self.disposition
                    && !self.ordinary_handoff
                    && matches!(slot, Slot::Scratch0 | Slot::Scratch1)
            }
            NativeCompleteRole::Observed(data) => {
                if !self.recovery
                    || !self.singleton_locked
                    || self.sealed
                    || self.recovery_completion != RecoveryCompletionSelection::NewComplete
                    || !self.complete_capture_attempted
                    || self.complete_shape != Some(data.presence)
                {
                    return false;
                }
                let wanted = match slot {
                    Slot::Scratch1 => Some((COMPLETE_BYTES, data.complete)),
                    Slot::Scratch0 if data.presence.ticket() => Some((TICKET_BYTES, data.ticket)),
                    Slot::Scratch3 if data.presence.closure() => {
                        Some((CLOSURE_BYTES, data.closure))
                    }
                    _ => None,
                };
                wanted.is_some()
                    && self.expected[slot as usize] == wanted
                    && self.lower.original[slot as usize].is_some()
            }
        }
    }
    pub(super) fn new_complete_observation_captured(&self) -> bool {
        matches!(&self.complete_role, NativeCompleteRole::Observed(data) if data.captured
            && ((!data.presence.closure() && self.role_never_acquired(Slot::Scratch3))
                || (data.presence.closure() && self.complete_member_observed(Slot::Scratch3)))
            && ((!data.presence.ticket() && self.role_never_acquired(Slot::Scratch0))
                || (data.presence.ticket() && self.complete_member_observed(Slot::Scratch0))))
            && self.complete_member_observed(Slot::Scratch1)
            && self.stage_name.is_none()
            && self.lower.original[Slot::Scratch2 as usize].is_none()
            && self.expected[Slot::Scratch2 as usize].is_none()
            && !self.unlinked[Slot::Scratch2 as usize]
    }
    fn role_never_acquired(&self, slot: Slot) -> bool {
        self.lower.original[slot as usize].is_none()
            && self.lower.io.original(slot).is_err()
            && self.expected[slot as usize].is_none()
            && !self.unlinked[slot as usize]
    }
    pub(super) fn new_complete_qualified(&self) -> bool {
        self.new_complete_observation_captured()
            && self.fresh_new_complete.as_ref().is_some_and(|proof| {
                self.complete_presence().ok() == Some(proof.presence)
                    && self.expected[Slot::Scratch1 as usize]
                        == Some((COMPLETE_BYTES, proof.complete))
            })
    }
    fn never_acquired_absent(
        &self,
        slot: Slot,
        name: &str,
        until: Instant,
    ) -> Result<(), Unavailable> {
        tick(until)?;
        if self.lower.original[slot as usize].is_some()
            || self.lower.io.original(slot).is_ok()
            || self.expected[slot as usize].is_some()
            || self.unlinked[slot as usize]
            || !matches!(
                fstatat(
                    self.lower.io.original(Slot::State)?,
                    name,
                    AtFlags::AT_SYMLINK_NOFOLLOW
                ),
                Err(nix::errno::Errno::ENOENT)
            )
        {
            return Err(Unavailable);
        }
        tick(until)
    }
    pub(super) fn check_new_complete_retired_sources(
        &self,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.check_final_output_retired_sources(until)?;
        let presence = self.complete_presence()?;
        if !presence.closure() {
            self.never_acquired_absent(Slot::Scratch3, CLOSURE_MEMBER, until)?;
        }
        if !presence.ticket() {
            self.never_acquired_absent(Slot::Scratch0, TICKET_MEMBER, until)?;
        }
        Ok(())
    }
    fn read_complete_data(&self, until: Instant) -> Result<CompleteRecord, FirstError> {
        let mut bytes = [0; COMPLETE_BYTES];
        tick(until).map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .original(Slot::Scratch1)
            .map_err(|_| FirstError::StillFenced)?
            .read_exact_at(&mut bytes, 0)
            .map_err(|_| FirstError::StillFenced)?;
        tick(until).map_err(|_| FirstError::StillFenced)?;
        if self.expected[Slot::Scratch1 as usize]
            != Some((COMPLETE_BYTES, Sha256::digest(bytes).into()))
        {
            return Err(FirstError::StillFenced);
        }
        CompleteRecord::decode(&bytes).ok_or(FirstError::StillFenced)
    }
    pub(super) fn capture_new_complete_data(
        &mut self,
        new: [&[u8]; 2],
        generation: u64,
        desired: &[u8],
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.recovery
            || self.sealed
            || !self.singleton_locked
            || self.recovery_completion != RecoveryCompletionSelection::NewComplete
            || !matches!(self.complete_role, NativeCompleteRole::None)
            || self.completed
            || self.retirement
            || self.disposition
            || self.complete_capture_attempted
        {
            return Err(FirstError::Admission);
        }
        self.complete_capture_attempted = true; // BEFORE any shape sample; no resampling on error
        for slot in [Slot::Owner, Slot::Desired, Slot::Login] {
            self.lower
                .io
                .original(slot)
                .map_err(|_| FirstError::Admission)?;
            if self.lower.original[slot as usize].is_none() {
                return Err(FirstError::Admission);
            }
        }
        self.check_final_output_retired_sources(until)
            .map_err(|_| FirstError::Admission)?;
        let state = self
            .lower
            .io
            .original(Slot::State)
            .map_err(|_| FirstError::Admission)?;
        let present = |name| match fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(nix::errno::Errno::ENOENT) => Ok(false),
            _ => Err(FirstError::Admission),
        };
        // The closed classification is sampled ONCE, never after failure.
        let presence = match (
            present(CLOSURE_MEMBER)?,
            present(TICKET_MEMBER)?,
            present(COMPLETE_MEMBER)?,
        ) {
            (true, true, true) => NewCompletePresence::ClosureTicketComplete,
            (false, true, true) => NewCompletePresence::TicketComplete,
            (false, false, true) => NewCompletePresence::CompleteOnly,
            _ => return Err(FirstError::Admission),
        };
        self.complete_shape = Some(presence); // closed original classification BEFORE record opens
        let bytes = self.recover_member(
            Slot::State,
            Slot::Scratch1,
            COMPLETE_MEMBER,
            COMPLETE_BYTES,
            until,
        )?;
        let complete = CompleteRecord::decode(&bytes).ok_or(FirstError::Admission)?;
        let ticket = complete.ticket_data();
        let closure = ticket.closure_data();
        let current_store = self.read_original_live(0, until)?;
        let current_template = self.read_original_live(1, until)?;
        if [current_store.as_slice(), current_template.as_slice()] != new
            || closure.receipt().terminal().phase() != DecisionPhase::Committed
            || !ticket.matches(
                closure,
                self.uid.ok_or(FirstError::Admission)?,
                generation,
                Some(desired),
            )
            || !crate::restore_executor_candidate::successor::rotation::final_review::output_binding(
                closure.receipt(),
                [current_store.as_slice(), current_template.as_slice()],
                new,
            )
        {
            return Err(FirstError::Admission);
        }
        if presence.closure() {
            let actual = self.recover_member(
                Slot::State,
                Slot::Scratch3,
                CLOSURE_MEMBER,
                CLOSURE_BYTES,
                until,
            )?;
            if actual.as_slice() != closure.encode() {
                return Err(FirstError::Admission);
            }
        }
        if presence.ticket() {
            let actual = self.recover_member(
                Slot::State,
                Slot::Scratch0,
                TICKET_MEMBER,
                TICKET_BYTES,
                until,
            )?;
            if actual.as_slice() != ticket.encode() {
                return Err(FirstError::Admission);
            }
        }
        self.complete_role = NativeCompleteRole::Observed(ObservedNewComplete {
            presence,
            complete: Sha256::digest(&bytes).into(),
            closure: Sha256::digest(closure.encode()).into(),
            ticket: Sha256::digest(ticket.encode()).into(),
            captured: false,
        });
        self.check_new_complete_retired_sources(until)
            .map_err(|_| FirstError::Admission)?;
        self.scan_catalogue(Slot::Config, Some(0), &[], until)
            .map_err(|_| FirstError::Admission)?;
        self.scan_catalogue(Slot::State, Some(1), &[], until)
            .map_err(|_| FirstError::Admission)?;
        self.catalogues_captured = true;
        for (index, catalogue) in self.catalogues.iter().enumerate() {
            if !(0..catalogue.count).all(|i| {
                let name = &catalogue.names[i][..catalogue.lengths[i]];
                !(name.starts_with(b"restore-") || name.starts_with(b".restore-"))
                    || (index == 1
                        && (name == COMPLETE_MEMBER.as_bytes()
                            || (presence.closure() && name == CLOSURE_MEMBER.as_bytes())
                            || (presence.ticket() && name == TICKET_MEMBER.as_bytes())
                            || native_history::NAMES
                                .iter()
                                .any(|allowed| allowed.as_bytes() == name)))
            }) {
                return Err(FirstError::Admission);
            }
        }
        self.complete_catalogue_index = Some(
            self.catalogues[1]
                .index(COMPLETE_MEMBER.as_bytes())
                .ok_or(FirstError::Admission)?,
        );
        self.capture_history(until)?;
        self.check_original_bytes(until)?;
        self.check_bindings(until)
            .map_err(|_| FirstError::Admission)?;
        let NativeCompleteRole::Observed(data) = &mut self.complete_role else {
            return Err(FirstError::Admission);
        };
        data.captured = true;
        Ok(())
    }
    pub(super) fn qualify_new_complete_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.new_complete_observation_captured()
            || self.fresh_new_complete.is_some()
            || self.completed
            || self.retirement
            || self.disposition
            || self.retirement_permission != NativeRetirementPermission::Complete
        {
            return Err(FirstError::Admission);
        }
        let presence = self
            .complete_presence()
            .map_err(|_| FirstError::StillFenced)?;
        self.gate(origin, until)?;
        self.retire_recovery_endpoint(origin, cut, until)?;
        let sources = [
            Slot::OldStore,
            Slot::OldTemplate,
            Slot::Scratch1,
            Slot::Owner,
            Slot::Desired,
            Slot::Login,
            Slot::Config,
            Slot::State,
            Slot::Root,
            Slot::Run,
            Slot::Scratch0,
            Slot::Scratch3,
        ];
        for (index, slot) in sources
            .into_iter()
            .take(if presence.closure() {
                12
            } else if presence.ticket() {
                11
            } else {
                10
            })
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
            cut(NativeStep::NewCompleteSourceSynced(index))?;
            self.gate(origin, until)?;
        }
        self.check_new_complete_retired_sources(until)
            .map_err(|_| FirstError::StillFenced)?;
        self.gate(origin, until)?;
        let NativeCompleteRole::Observed(data) = &self.complete_role else {
            return Err(FirstError::StillFenced);
        };
        self.fresh_new_complete = Some(FreshNewComplete {
            presence,
            complete: data.complete,
        });
        self.retirement = true;
        self.completed = true;
        Ok(())
    }
    pub(crate) fn qualify_native_new_complete(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        backup: &omavless_domain::private_backup::OpenedBackup,
        quiescence: &mut NativeOldRecoveryQuiescence,
    ) -> Result<(), FirstError> {
        if !crate::product_scope::restore_enabled() {
            return Err(FirstError::Admission);
        }
        if self.sealed || self.uid.is_some() {
            return Err(FirstError::StillFenced);
        }
        self.recovery = true;
        self.recovery_completion = RecoveryCompletionSelection::NewComplete;
        let until = Instant::now() + std::time::Duration::from_secs(45);
        let result = (|| {
            self.capture_fresh_output_originals(origin, until)?;
            let off = backup
                .restore_store_off()
                .map_err(|_| FirstError::Prepare)?;
            self.capture_new_complete_data(
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
            self.qualify_new_complete_inner(
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
    pub(super) fn check_new_complete_completed_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.new_complete_qualified()
            || !self.completed
            || self.sealed
            || self.ordinary_handoff
            || self.retirement_permission != NativeRetirementPermission::Complete
        {
            return Err(FirstError::StillFenced);
        }
        self.check_new_complete_retired_sources(until)
            .map_err(|_| FirstError::StillFenced)?;
        let presence = self
            .complete_presence()
            .map_err(|_| FirstError::StillFenced)?;
        if (presence.closure() && self.unlinked[Slot::Scratch3 as usize] != self.disposition_done)
            || (presence.ticket()
                && self.unlinked[Slot::Scratch0 as usize] != self.disposition_done)
        {
            return Err(FirstError::StillFenced);
        }
        self.gate(origin, until)
    }
    pub(super) fn prevalidate_complete_catalogue_index(&self) -> Result<usize, FirstError> {
        let index = self
            .complete_catalogue_index
            .ok_or(FirstError::StillFenced)?;
        if index >= self.catalogues[1].count
            || self.catalogues[1].removed[index]
            || self.catalogues[1].index(COMPLETE_MEMBER.as_bytes()) != Some(index)
        {
            return Err(FirstError::StillFenced);
        }
        Ok(index)
    }
    pub(super) fn dispose_new_complete_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        uid: u32,
        generation: u64,
        desired: &[u8],
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.new_complete_qualified()
            || self.disposition
            || !self.completed
            || self.sealed
            || self.uid != Some(uid)
            || self.ordinary_handoff
            || self.retirement_permission != NativeRetirementPermission::Complete
        {
            return Err(FirstError::StillFenced);
        }
        let presence = self
            .complete_presence()
            .map_err(|_| FirstError::StillFenced)?;
        self.disposition = true; // consume BEFORE first unlink/rename, never create/recreate
        self.gate(origin, until)?;
        let complete = self.read_complete_data(until)?;
        let ticket = complete.ticket_data();
        if !ticket.matches(ticket.closure_data(), uid, generation, Some(desired)) {
            return Err(FirstError::StillFenced);
        }
        self.lower
            .verify_member(
                Slot::State,
                Slot::Scratch1,
                COMPLETE_MEMBER,
                &complete.encode(),
                until,
            )
            .map_err(|_| FirstError::StillFenced)?;
        if presence.closure() {
            self.own_unlink(
                Slot::State,
                Slot::Scratch3,
                CLOSURE_MEMBER,
                false,
                origin,
                until,
            )?;
            cut(NativeStep::DispositionClosureUnlinked)?;
        }
        if presence.ticket() {
            self.own_unlink(
                Slot::State,
                Slot::Scratch0,
                TICKET_MEMBER,
                false,
                origin,
                until,
            )?;
            cut(NativeStep::DispositionTicketUnlinked)?;
        }
        self.finish_native_history(origin, &complete, cut, until)
    }

    fn retired_for_complete<'e, 'o, 's, H: LifecycleHost>(
        &'e mut self,
        origin: &'o mut NativeSessionOrigin<'s, H>,
        prepared: &PreparedRestorePair,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
        until: Instant,
    ) -> Result<RetiredForComplete<'e, 'o, 's, H>, FirstError> {
        let checked = self.checked_committed_retirement(
            origin,
            prepared,
            NativeRetirementPermission::CompletedCutOnly,
            until,
        )?;
        self.retire_native_terminal_ungranted(
            origin,
            RetainedTerminal {
                members: checked.members,
                intent: &checked.intent,
                terminal: &checked.terminal,
                choice: NativeTerminalChoice::NewCommitted,
            },
            cut,
            until,
        )?;
        self.gate(origin, until)?;
        if self.retirement_permission != NativeRetirementPermission::CompletedCutOnly
            || !self.retirement
            || self.completed
            || self.disposition
            || self.sealed
            || !matches!(self.complete_role, NativeCompleteRole::None)
            || self.ticket_role != NativeTicketRole::None
        {
            return Err(FirstError::StillFenced);
        }
        Ok(RetiredForComplete {
            engine: self,
            origin,
            desired: checked.desired,
            until,
        })
    }
    pub(crate) fn execute_native_committed_complete_cut<H: LifecycleHost>(
        &mut self,
        origin: &mut NativeSessionOrigin<'_, H>,
        prepared: &PreparedRestorePair,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        if !crate::product_scope::restore_enabled() {
            return Err(FirstError::Admission);
        }
        if self.sealed || self.uid.is_some() {
            return Err(FirstError::StillFenced);
        }
        self.retirement_permission = NativeRetirementPermission::CompletedCutOnly; // BEFORE Stage
        let until = Instant::now() + std::time::Duration::from_secs(45);
        let result = (|| {
            match self.execute_native_mode(origin, prepared, cut, false)? {
                NativeForwardOutcome::Committed => {}
                NativeForwardOutcome::IntentPaused(_) => return Err(FirstError::StillFenced),
            }
            self.retired_for_complete(origin, prepared, cut, until)?
                .publish_complete_then_seal(cut)
        })();
        self.revoke_native();
        result
    }
}

/// Distinct operation-local capability. Ticket's capability/OwnedCut role is
/// NOT widened. Only actual original retirement constructs this mutable loan.
struct RetiredForComplete<'e, 'o, 's, H: LifecycleHost> {
    engine: &'e mut NativeEngine,
    origin: &'o mut NativeSessionOrigin<'s, H>,
    desired: zeroize::Zeroizing<Vec<u8>>,
    until: Instant,
}
impl<H: LifecycleHost> Drop for RetiredForComplete<'_, '_, '_, H> {
    fn drop(&mut self) {
        self.engine.revoke_native();
    } // flags only, never resource drop/calls/locks
}
impl<H: LifecycleHost> RetiredForComplete<'_, '_, '_, H> {
    fn publish_complete_then_seal(
        self,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        use crate::restore_disposition_ticket_model::Ticket;
        self.engine.complete_role = NativeCompleteRole::OwnedCut; // consuming continuation BEFORE effects
        self.engine.gate(self.origin, self.until)?;
        let mut bytes = [0; CLOSURE_BYTES];
        self.engine
            .lower
            .io
            .original(Slot::Scratch3)
            .map_err(|_| FirstError::StillFenced)?
            .read_exact_at(&mut bytes, 0)
            .map_err(|_| FirstError::StillFenced)?;
        self.engine.gate(self.origin, self.until)?;
        let closure = crate::restore_closure_model::ClosureRecord::decode(&bytes)
            .map_err(|_| FirstError::StillFenced)?;
        if closure.receipt().terminal().phase() != DecisionPhase::Committed {
            return Err(FirstError::StillFenced);
        }
        let ticket = Ticket::from_bound_closure(
            &closure,
            self.origin.uid(),
            self.origin.generation(),
            Some(&self.desired),
        )
        .ok_or(FirstError::StillFenced)?;
        self.engine.write(
            Slot::State,
            Slot::Scratch0,
            TICKET_MEMBER,
            &ticket.encode(),
            self.origin,
            self.until,
        )?;
        cut(NativeStep::DispositionTicket)?;
        let complete = CompleteRecord::from_ticket(&ticket).ok_or(FirstError::StillFenced)?;
        self.engine.write(
            Slot::State,
            Slot::Scratch1,
            COMPLETE_MEMBER,
            &complete.encode(),
            self.origin,
            self.until,
        )?;
        self.engine
            .lower
            .verify_member(
                Slot::State,
                Slot::Scratch1,
                COMPLETE_MEMBER,
                &complete.encode(),
                self.until,
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.engine.gate(self.origin, self.until)?;
        cut(NativeStep::DispositionComplete)?;
        Err(FirstError::StillFenced) // even always-OK callback has NO unlink/history/handoff continuation
    }
}
