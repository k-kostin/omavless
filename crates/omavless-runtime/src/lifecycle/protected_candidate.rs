// SPDX-License-Identifier: MIT
//! Unregistered candidate owns the same executor, not another runtime owner.
//! Explicit developer composition uses the fixed client; no product constructor
//! is registered. Mock replies establish conformance, never kernel authority.
//! Native coverage issuance remains closed before any real validation or Arm.
use super::*;
use omavless_netguard::protocol::{Health, Mode, POLICY_VERSION, Protection, Request, Response};

trait ProtectionPort {
    /// No retry/reconnect. Error includes unknown delivery, timeout, late reply
    /// or channel loss; an error response is not a no-effect certificate.
    fn exchange(&mut self, request: Request) -> Result<Response, ()>;
}

/// Private consuming boundary: ordinary host prepare/start cannot satisfy it.
mod sealed {
    pub trait Sealed {}
}
impl sealed::Sealed for crate::native_host::NativeLifecycleHost {}
pub(crate) trait ProtectedHost: LifecycleHost + sealed::Sealed {
    type Admission;
    fn prepare_admitted(
        &mut self,
        desired: &DesiredState,
    ) -> Result<Self::Admission, HostStepError>;
    fn recheck_admission(&self, admission: &Self::Admission) -> Result<(), HostStepError>;
    fn start_admitted(&mut self, admission: Self::Admission) -> Result<(), HostStepError>;
    fn commit_protected(&mut self) -> Result<(), HostStepError>;
    fn discard_protected(&mut self) -> Result<(), HostStepError>;
}

// No production constructor/registration. Native protected readiness remains
// unavailable; this binds only the reviewed fixed-purpose optional client.
impl ProtectionPort for omavless_netguard::client_candidate::FixedClient {
    fn exchange(&mut self, request: Request) -> Result<Response, ()> {
        Self::exchange(self, request).map_err(|_| ())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Fresh,
    InFlight,
    Armed(u64),
    Closed,
    Poisoned,
}

struct ProtectedCandidate<H: ProtectedHost, P: ProtectionPort> {
    // No accessor/into_host/ordinary-executor extraction. On uncertain unwind
    // or abandoned armed session, forget the original graph, not copied proof.
    owned: Option<(LifecycleExecutor<H>, P)>,
    phase: Phase,
    admission: Option<H::Admission>,
}

impl<H: ProtectedHost, P: ProtectionPort> Drop for ProtectedCandidate<H, P> {
    fn drop(&mut self) {
        if !matches!(self.phase, Phase::Fresh | Phase::Closed)
            && let Some(original) = self.owned.take()
        {
            std::mem::forget(original);
            if let Some(admission) = self.admission.take() {
                std::mem::forget(admission);
            }
        }
    }
}

impl<H: ProtectedHost, P: ProtectionPort> ProtectedCandidate<H, P> {
    fn new(executor: LifecycleExecutor<H>, port: P) -> Self {
        Self {
            owned: Some((executor, port)),
            phase: Phase::Fresh,
            admission: None,
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
            .prepare_admitted(&target);
        // Even before Arm, an uncertain validator can remain alive. No generic
        // discard/restored/retry transition is permitted for this failure.
        self.admission = Some(prepared.map_err(|_| self.poison())?);
        let reserved = DesiredState {
            generation: target.generation,
            ..current
        };
        if self.local(|e| e.write(&reserved)).is_err() {
            // Known no Arm, but uncertain desired write cannot be reset/retried.
            self.local(|e| {
                e.host
                    .discard_protected()
                    .map_err(|_| LifecycleError::ManualRecoveryRequired)
            })?;
            return Err(self.poison());
        }
        self.owned
            .as_ref()
            .ok_or(LifecycleError::ManualRecoveryRequired)?
            .0
            .host
            .recheck_admission(
                self.admission
                    .as_ref()
                    .ok_or(LifecycleError::ManualRecoveryRequired)?,
            )
            .map_err(|_| self.poison())?;
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
        let admission = self
            .admission
            .take()
            .ok_or(LifecycleError::ManualRecoveryRequired)?;
        self.local(|e| {
            e.host
                .start_admitted(admission)
                .map_err(|_| LifecycleError::RecoveryFailed)
        })?;
        self.local(|e| e.verify_connected(&target))?;
        self.local(|e| {
            e.host
                .commit_protected()
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
                .discard_protected()
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

/// Explicit SOURCE developer driver: consumes the caller's existing native
/// executor; no service installation, alternate host, registration or default
/// constructor. Coverage issuance remains closed, so no real validation/Arm
/// can currently follow preparation. No error path automatically disconnects.
#[cfg(feature = "netguard-native-scenario")]
pub fn native_roundtrip(
    executor: LifecycleExecutor<crate::native_host::NativeLifecycleHost>,
    profile_id: &str,
) -> Result<LifecycleOutcome, LifecycleError> {
    let mut owner = ProtectedCandidate::new(
        executor,
        omavless_netguard::client_candidate::FixedClient::new(),
    );
    owner.connect_full(profile_id)?;
    owner.disconnect()
}
