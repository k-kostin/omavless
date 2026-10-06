// SPDX-License-Identifier: MIT
//! Unregistered candidate owns the same executor, not another runtime owner.
//! Explicit developer composition uses the fixed client; no product constructor
//! is registered. Mock replies establish conformance, never kernel authority.
//! Native coverage issuance requires the qualified private policy, original
//! validation and same-owner postchecks before any Arm.
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
    type Interval;
    fn prepare_admitted(
        &mut self,
        desired: &DesiredState,
    ) -> Result<Self::Admission, HostStepError>;
    fn recheck_admission(&self, admission: &Self::Admission) -> Result<(), HostStepError>;
    fn start_admitted(&mut self, admission: Self::Admission) -> Result<(), HostStepError>;
    fn commit_protected(&mut self) -> Result<(), HostStepError>;
    fn discard_protected(&mut self) -> Result<(), HostStepError>;
    fn begin_interval(&mut self, desired: &DesiredState) -> Result<Self::Interval, HostStepError>;
    fn complete_interval(&mut self, interval: &mut Self::Interval) -> Result<(), HostStepError>;
    fn recheck_interval(&mut self, desired: &DesiredState) -> Result<(), HostStepError>;
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

enum Executor<'a, H> {
    Owned(LifecycleExecutor<H>),
    Borrowed(&'a mut LifecycleExecutor<H>),
}
impl<H> std::ops::Deref for Executor<'_, H> {
    type Target = LifecycleExecutor<H>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(e) => e,
            Self::Borrowed(e) => e,
        }
    }
}
impl<H> std::ops::DerefMut for Executor<'_, H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Owned(e) => e,
            Self::Borrowed(e) => e,
        }
    }
}

struct ProtectedCandidate<'a, H: ProtectedHost, P: ProtectionPort> {
    // No accessor/into_host/ordinary-executor extraction. On uncertain unwind
    // or abandoned armed session, forget the original graph, not copied proof.
    owned: Option<(Executor<'a, H>, P)>,
    phase: Phase,
    admission: Option<H::Admission>,
    interval: Option<H::Interval>,
    origin: Option<&'a mut dyn FnMut() -> Result<(), LifecycleError>>,
}

impl<H: ProtectedHost, P: ProtectionPort> Drop for ProtectedCandidate<'_, H, P> {
    fn drop(&mut self) {
        if !matches!(self.phase, Phase::Fresh | Phase::Closed)
            && let Some(original) = self.owned.take()
        {
            std::mem::forget(original);
            if let Some(admission) = self.admission.take() {
                std::mem::forget(admission);
            }
            if let Some(interval) = self.interval.take() {
                std::mem::forget(interval);
            }
        }
    }
}

impl<H: ProtectedHost, P: ProtectionPort> ProtectedCandidate<'_, H, P> {
    fn new(executor: LifecycleExecutor<H>, port: P) -> Self {
        Self {
            owned: Some((Executor::Owned(executor), port)),
            phase: Phase::Fresh,
            admission: None,
            interval: None,
            origin: None,
        }
    }
    fn origin_check(&mut self) -> Result<(), LifecycleError> {
        if self.origin.as_mut().is_some_and(|check| check().is_err()) {
            return Err(self.poison());
        }
        Ok(())
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
        self.origin_check()?;
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
        self.origin_check()?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        if request == (Request::Status {}) {
            crate::protected_native_diagnostic::mark(
                crate::protected_native_diagnostic::Cut::StatusExchange,
            );
        }
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
        let current = self.local(|e| {
            #[cfg(all(test, feature = "netguard-native-scenario"))]
            crate::protected_native_diagnostic::mark(
                crate::protected_native_diagnostic::Cut::DesiredRead,
            );
            e.read()
        })?;
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
        self.local(|e| {
            #[cfg(all(test, feature = "netguard-native-scenario"))]
            crate::protected_native_diagnostic::mark(
                crate::protected_native_diagnostic::Cut::EmptyObservation,
            );
            e.verify_empty(&current)
        })?;
        self.local(|e| {
            #[cfg(all(test, feature = "netguard-native-scenario"))]
            crate::protected_native_diagnostic::mark(
                crate::protected_native_diagnostic::Cut::ProtectedEligibility,
            );
            e.host
                .protected_preflight(&target)
                .map_err(|_| LifecycleError::InvalidRequest)
        })?;
        let status = self.exchange(Request::Status {})?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::StatusInterpretation,
        );
        let Some(Protection::Disarmed { closed_generation }) = Self::healthy(status) else {
            return Err(self.poison());
        };
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::GenerationReservation,
        );
        target.generation = current
            .generation
            .max(closed_generation.unwrap_or(0))
            .checked_add(1)
            .filter(|n| *n < MAX_GENERATION)
            .ok_or_else(|| self.poison())?;
        // Reserve headroom for explicit disconnected intent as well. No wraps,
        // recycled attempts or trusted floor invented from a wire omission.
        self.phase = Phase::InFlight;
        self.origin_check()?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::HostPreparation,
        );
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
    fn observe_interval(&mut self) -> Result<(), LifecycleError> {
        let Phase::Armed(generation) = self.phase else {
            return Err(self.poison());
        };
        if self.interval.is_some() {
            return Err(self.poison());
        }
        let desired = self.local(|e| e.read())?;
        if !desired.connected
            || desired.generation != generation
            || desired.mode != RoutingMode::Global
        {
            return Err(self.poison());
        }
        self.local(|e| e.verify_connected(&desired))?;
        self.local(|e| {
            e.host
                .recheck_interval(&desired)
                .map_err(|_| LifecycleError::ManualRecoveryRequired)
        })?;
        let interval = self.local(|e| {
            e.host
                .begin_interval(&desired)
                .map_err(|_| LifecycleError::ManualRecoveryRequired)
        })?;
        self.interval = Some(interval);
        self.origin_check()?;
        let result = self
            .owned
            .as_mut()
            .ok_or(LifecycleError::ManualRecoveryRequired)?
            .0
            .host
            .complete_interval(
                self.interval
                    .as_mut()
                    .ok_or(LifecycleError::ManualRecoveryRequired)?,
            );
        result.map_err(|_| self.poison())?;
        self.local(|e| {
            if e.read()? != desired {
                return Err(LifecycleError::ManualRecoveryRequired);
            }
            e.host
                .recheck_interval(&desired)
                .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
            e.verify_connected(&desired)
        })?;
        self.origin_check()?;
        self.phase = Phase::Armed(generation);
        Ok(())
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
        // A distinct final observation follows only the positively completed
        // Disarm. It is never a retry or a query that repairs an uncertain reply.
        if Self::healthy(self.exchange(Request::Status {})?)
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

/// Called only inside the consuming real-owner custody guard. The borrowed
/// executor never escapes or replaces its owning transaction. On uncertainty
/// the caller retains that WHOLE owner as well as singleton and migration lease.
#[cfg(feature = "netguard-native-scenario")]
pub(crate) fn borrowed_native_roundtrip(
    executor: &mut LifecycleExecutor<crate::native_host::NativeLifecycleHost>,
    profile_id: &str,
    origin: &mut dyn FnMut() -> Result<(), LifecycleError>,
) -> Result<LifecycleOutcome, LifecycleError> {
    let mut candidate = ProtectedCandidate {
        owned: Some((
            Executor::Borrowed(executor),
            omavless_netguard::client_candidate::FixedClient::new(),
        )),
        phase: Phase::Fresh,
        admission: None,
        interval: None,
        origin: Some(origin),
    };
    candidate.connect_full(profile_id)?;
    candidate.observe_interval()?;
    candidate.disconnect()
}

#[cfg(test)]
mod tests;
