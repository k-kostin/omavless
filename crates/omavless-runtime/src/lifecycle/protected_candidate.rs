// SPDX-License-Identifier: MIT
//! Unregistered candidate owns the same executor, not another runtime owner.
//! No concrete transport or production constructor. Typed injected replies
//! establish conformance only, not peer/namespace/kernel authority. A real
//! port must enforce one original whole deadline and current authenticated peer.
use super::*;
use omavless_netguard::protocol::{Health, Mode, POLICY_VERSION, Protection, Request, Response};

trait ProtectionPort {
    /// No retry/reconnect. Error includes unknown delivery, timeout, late reply
    /// or channel loss; an error response is not a no-effect certificate.
    fn exchange(&mut self, request: Request) -> Result<Response, ()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Fresh,
    InFlight,
    Armed(u64),
    Closed,
    Poisoned,
}

struct ProtectedCandidate<H: LifecycleHost, P: ProtectionPort> {
    // No accessor/into_host/ordinary-executor extraction. On uncertain unwind
    // or abandoned armed session, forget the original graph, not copied proof.
    owned: Option<(LifecycleExecutor<H>, P)>,
    phase: Phase,
}

impl<H: LifecycleHost, P: ProtectionPort> Drop for ProtectedCandidate<H, P> {
    fn drop(&mut self) {
        if !matches!(self.phase, Phase::Fresh | Phase::Closed)
            && let Some(original) = self.owned.take()
        {
            std::mem::forget(original);
        }
    }
}

impl<H: LifecycleHost, P: ProtectionPort> ProtectedCandidate<H, P> {
    fn new(executor: LifecycleExecutor<H>, port: P) -> Self {
        Self {
            owned: Some((executor, port)),
            phase: Phase::Fresh,
        }
    }
    fn poison(&mut self) -> LifecycleError {
        self.phase = Phase::Poisoned;
        if let Some((executor, _)) = self.owned.as_mut() {
            executor.actual = ActualState::ManualRecoveryRequired;
        }
        LifecycleError::ManualRecoveryRequired
    }
    fn local<T>(
        &mut self,
        effect: impl FnOnce(&mut LifecycleExecutor<H>) -> Result<T, LifecycleError>,
    ) -> Result<T, LifecycleError> {
        self.phase = Phase::InFlight; // consumed BEFORE any callback/effect/panic
        let result = effect(
            &mut self
                .owned
                .as_mut()
                .ok_or(LifecycleError::ManualRecoveryRequired)?
                .0,
        );
        result.map_err(|_| self.poison())
    }
    fn exchange(&mut self, request: Request) -> Result<Response, LifecycleError> {
        self.phase = Phase::InFlight;
        let result = self
            .owned
            .as_mut()
            .ok_or(LifecycleError::ManualRecoveryRequired)?
            .1
            .exchange(request);
        result.map_err(|()| self.poison())
    }
    fn healthy(response: Response) -> Option<Protection> {
        match response {
            Response::Status {
                policy_version: POLICY_VERSION,
                protection,
                health: Health::Verified,
            } => Some(protection),
            _ => None,
        }
    }
    fn connect_full(&mut self, profile_id: &str) -> Result<LifecycleOutcome, LifecycleError> {
        if self.phase != Phase::Fresh {
            return Err(self.poison());
        }
        let current = self.local(|e| e.read())?;
        if current.connected {
            return Err(self.poison());
        } // replacement/startup unsupported
        let mut target = DesiredState {
            connected: true,
            profile_id: profile_id.to_owned(),
            mode: RoutingMode::Global,
            ..current.clone()
        };
        target.validate().map_err(|_| self.poison())?;
        self.local(|e| e.verify_empty(&current))?;
        self.local(|e| {
            e.host
                .protected_preflight(&target)
                .map_err(|_| LifecycleError::InvalidRequest)
        })?;
        let Some(Protection::Disarmed { closed_generation }) =
            Self::healthy(self.exchange(Request::Status {})?)
        else {
            return Err(self.poison());
        };
        target.generation = current
            .generation
            .max(closed_generation.unwrap_or(0))
            .checked_add(1)
            .filter(|n| *n < MAX_GENERATION)
            .ok_or_else(|| self.poison())?;
        // Reserve headroom for explicit disconnected intent as well. No wraps,
        // recycled attempts or trusted floor invented from a wire omission.
        self.phase = Phase::InFlight;
        let prepared = self
            .owned
            .as_mut()
            .ok_or(LifecycleError::ManualRecoveryRequired)?
            .0
            .host
            .prepare(&target);
        if prepared.is_err() {
            // Cleanup is allowed here only because NO Arm was attempted.
            self.local(|e| {
                e.host
                    .discard_prepared()
                    .map_err(|_| LifecycleError::ManualRecoveryRequired)
            })?;
            self.phase = Phase::Fresh;
            return Err(LifecycleError::TransitionFailedRestored);
        }
        let reserved = DesiredState {
            generation: target.generation,
            ..current
        };
        if self.local(|e| e.write(&reserved)).is_err() {
            // Known no Arm, but uncertain desired write cannot be reset/retried.
            self.local(|e| {
                e.host
                    .discard_prepared()
                    .map_err(|_| LifecycleError::ManualRecoveryRequired)
            })?;
            return Err(self.poison());
        }
        let response = self.exchange(Request::Arm {
            generation: target.generation,
            mode: Mode::Full,
        })?;
        if Self::healthy(response)
            != Some(Protection::Armed {
                generation: target.generation,
            })
        {
            return Err(self.poison());
        }
        self.local(|e| e.write(&target))?;
        self.local(|e| {
            e.host
                .start_prepared()
                .map_err(|_| LifecycleError::RecoveryFailed)
        })?;
        self.local(|e| e.verify_connected(&target))?;
        self.local(|e| {
            e.host
                .commit_prepared()
                .map_err(|_| LifecycleError::RecoveryFailed)
        })?;
        let e = &mut self
            .owned
            .as_mut()
            .ok_or(LifecycleError::ManualRecoveryRequired)?
            .0;
        e.actual = ActualState::Connected;
        self.phase = Phase::Armed(target.generation);
        Ok(e.outcome(&target, true))
    }
    fn disconnect(&mut self) -> Result<LifecycleOutcome, LifecycleError> {
        let Phase::Armed(armed_generation) = self.phase else {
            return Err(self.poison());
        };
        let current = self.local(|e| e.read())?;
        if !current.connected
            || current.generation != armed_generation
            || current.mode != RoutingMode::Global
        {
            return Err(self.poison());
        }
        let target = DesiredState {
            generation: armed_generation
                .checked_add(1)
                .filter(|n| *n <= MAX_GENERATION)
                .ok_or_else(|| self.poison())?,
            connected: false,
            profile_id: String::new(),
            ..current
        };
        self.local(|e| e.write(&target))?;
        self.local(|e| {
            e.host
                .stop_owned()
                .map_err(|_| LifecycleError::ManualRecoveryRequired)
        })?;
        self.local(|e| {
            e.host
                .discard_prepared()
                .map_err(|_| LifecycleError::ManualRecoveryRequired)
        })?;
        self.local(|e| e.verify_empty(&target))?;
        let response = self.exchange(Request::Disarm {
            generation: armed_generation,
        })?;
        if Self::healthy(response)
            != Some(Protection::Disarmed {
                closed_generation: Some(armed_generation),
            })
        {
            return Err(self.poison());
        }
        let e = &mut self
            .owned
            .as_mut()
            .ok_or(LifecycleError::ManualRecoveryRequired)?
            .0;
        e.actual = ActualState::Disconnected;
        self.phase = Phase::Closed;
        Ok(e.outcome(&target, true))
    }
}

#[cfg(test)]
mod tests;
