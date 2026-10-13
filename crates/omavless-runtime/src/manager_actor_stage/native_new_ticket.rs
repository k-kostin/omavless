// SPDX-License-Identifier: MIT
//! Existing-ticket observations and one operation-local warm ticket publisher.
//! No record constructor, copied grant or completion continuation is exposed.
use super::*;

impl NativeEngine {
    pub(super) fn ticket_observation_owned(&self) -> bool {
        match self.ticket_role {
            NativeTicketRole::None => false,
            NativeTicketRole::ObservedNew(digest) => {
                self.recovery
                    && self.singleton_locked
                    && !self.sealed
                    && self.recovery_completion == RecoveryCompletionSelection::NewTicket
                    && self.stage_name.is_none()
                    && self.lower.original[Slot::Scratch0 as usize].is_some()
                    && self.expected[Slot::Scratch0 as usize]
                        == Some((
                            crate::restore_disposition_ticket_model::TICKET_BYTES,
                            digest,
                        ))
                    && self.lower.original[Slot::Scratch3 as usize].is_some()
            }
            NativeTicketRole::OwnedCut => {
                !self.recovery
                    && self.retirement_permission == NativeRetirementPermission::NewTicketCutOnly
                    && self.committed
                    && self.retirement
                    && !self.completed
                    && !self.disposition
                    && !self.ordinary_handoff
                    && !self.sealed
            }
        }
    }
    pub(super) fn new_ticket_observation_captured(&self) -> bool {
        self.final_output_observation_captured(FinalOutputChoice::NewTicketCommitted)
            && self.ticket_observation_owned()
    }
    pub(super) fn new_ticket_qualified(&self) -> bool {
        self.new_ticket_observation_captured()
            && self.fresh_new_ticket.as_ref().is_some_and(|proof| {
                self.expected[Slot::Scratch3 as usize]
                    == Some((
                        crate::restore_closure_model::RECORD_BYTES,
                        proof.closure_digest,
                    ))
                    && self.expected[Slot::Scratch0 as usize]
                        == Some((
                            crate::restore_disposition_ticket_model::TICKET_BYTES,
                            proof.ticket_digest,
                        ))
            })
    }
    pub(super) fn verify_observed_new_ticket(
        &mut self,
        closure: &crate::restore_closure_model::ClosureRecord,
        uid: u32,
        generation: u64,
        desired: &[u8],
        until: Instant,
    ) -> Result<crate::restore_disposition_ticket_model::Ticket, FirstError> {
        use crate::restore_disposition_ticket_model::{TICKET_BYTES, TICKET_MEMBER, Ticket};
        if !self.new_ticket_qualified() || self.unlinked[Slot::Scratch0 as usize] {
            return Err(FirstError::StillFenced);
        }
        let mut bytes = [0; TICKET_BYTES];
        tick(until).map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .original(Slot::Scratch0)
            .map_err(|_| FirstError::StillFenced)?
            .read_exact_at(&mut bytes, 0)
            .map_err(|_| FirstError::StillFenced)?;
        tick(until).map_err(|_| FirstError::StillFenced)?;
        if self.expected[Slot::Scratch0 as usize]
            != Some((TICKET_BYTES, Sha256::digest(bytes).into()))
        {
            return Err(FirstError::StillFenced);
        }
        self.lower
            .verify_member(Slot::State, Slot::Scratch0, TICKET_MEMBER, &bytes, until)
            .map_err(|_| FirstError::StillFenced)?;
        let ticket = Ticket::decode(&bytes).ok_or(FirstError::StillFenced)?;
        if !ticket.matches(closure, uid, generation, Some(desired)) {
            return Err(FirstError::StillFenced);
        }
        Ok(ticket) // comparison DATA, never a completion or publisher grant
    }

    fn retired_for_ticket<'e, 'o, 's, H: LifecycleHost>(
        &'e mut self,
        origin: &'o mut NativeSessionOrigin<'s, H>,
        prepared: &PreparedRestorePair,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
        until: Instant,
    ) -> Result<RetiredForTicket<'e, 'o, 's, H>, FirstError> {
        let checked = self.checked_committed_retirement(
            origin,
            prepared,
            NativeRetirementPermission::NewTicketCutOnly,
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
        if self.retirement_permission != NativeRetirementPermission::NewTicketCutOnly
            || !self.retirement
            || self.completed
            || self.disposition
            || self.sealed
            || self.ticket_role != NativeTicketRole::None
        {
            return Err(FirstError::StillFenced);
        }
        Ok(RetiredForTicket {
            engine: self,
            origin,
            desired: checked.desired,
            until,
        })
    }
    pub(crate) fn execute_native_committed_ticket_cut<H: LifecycleHost>(
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
        self.retirement_permission = NativeRetirementPermission::NewTicketCutOnly; // BEFORE Stage
        let until = Instant::now() + std::time::Duration::from_secs(45);
        let result = (|| {
            match self.execute_native_mode(origin, prepared, cut, false)? {
                NativeForwardOutcome::Committed => {}
                NativeForwardOutcome::IntentPaused(_) => return Err(FirstError::StillFenced),
            }
            self.retired_for_ticket(origin, prepared, cut, until)?
                .publish_one_then_seal(cut)
        })();
        self.revoke_native();
        result
    }
}

/// Constructed ONLY by original native retirement + final original gates. It
/// cannot escape the mutable engine/session borrows and has no Clone/decoder.
/// The only continuation consumes it before any ticket effect. All exits seal.
struct RetiredForTicket<'e, 'o, 's, H: LifecycleHost> {
    engine: &'e mut NativeEngine,
    origin: &'o mut NativeSessionOrigin<'s, H>,
    desired: zeroize::Zeroizing<Vec<u8>>,
    until: Instant,
}
impl<H: LifecycleHost> Drop for RetiredForTicket<'_, '_, '_, H> {
    fn drop(&mut self) {
        self.engine.revoke_native(); // only irreversible flags; no calls/locks/allocation
    }
}
impl<H: LifecycleHost> RetiredForTicket<'_, '_, '_, H> {
    fn publish_one_then_seal(
        self,
        cut: &mut impl FnMut(NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        use crate::restore_disposition_ticket_model::{TICKET_MEMBER, Ticket};
        self.engine.ticket_role = NativeTicketRole::OwnedCut; // consumed before first effect
        self.engine.gate(self.origin, self.until)?;
        let mut bytes = [0; crate::restore_closure_model::RECORD_BYTES];
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
        self.engine
            .lower
            .verify_member(
                Slot::State,
                Slot::Scratch0,
                TICKET_MEMBER,
                &ticket.encode(),
                self.until,
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.engine.gate(self.origin, self.until)?;
        cut(NativeStep::DispositionTicket)?; // exact fully verified still-fenced CUT DATA
        Err(FirstError::StillFenced) // always-OK/unreached callback cannot continue
    }
}
