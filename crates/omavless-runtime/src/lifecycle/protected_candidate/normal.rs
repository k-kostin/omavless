//! One persistent port/phase on the SAME registered owner. Operations borrow
//! its original executor only for that call; no borrowed executor escapes.
//! The caller installs whole-owner/server/operation-lease retaining custody
//! BEFORE entering here. This module cannot construct that authority.
use super::*;

pub(crate) struct NormalSession<P = omavless_netguard::client_candidate::FixedClient> {
    port: Option<P>,
    phase: Phase,
}

impl NormalSession {
    pub(crate) fn new() -> Self {
        Self {
            port: Some(omavless_netguard::client_candidate::FixedClient::new()),
            phase: Phase::Fresh,
        }
    }
}

impl<P: ProtectionPort> NormalSession<P> {
    #[cfg(test)]
    pub(super) fn for_test(port: P) -> Self {
        Self {
            port: Some(port),
            phase: Phase::Fresh,
        }
    }
    pub(crate) fn is_fresh(&self) -> bool {
        self.phase == Phase::Fresh
    }
    pub(crate) fn is_armed(&self) -> bool {
        matches!(self.phase, Phase::Armed(_))
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.phase == Phase::Closed
    }
    pub(crate) fn retention_required(&self) -> bool {
        !matches!(self.phase, Phase::Fresh | Phase::Closed)
    }

    pub(crate) fn connect<H: ProtectedHost>(
        &mut self,
        executor: &mut LifecycleExecutor<H>,
        profile: &str,
        origin: &mut dyn FnMut() -> Result<(), LifecycleError>,
    ) -> Result<LifecycleOutcome, LifecycleError> {
        if !self.is_fresh() {
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        // Seal the persistent slot BEFORE any move/callback/unwind. Failure
        // cannot turn an empty port slot into another allocation/retry.
        self.phase = Phase::Poisoned;
        let mut operation = ProtectedCandidate {
            owned: Some((
                Executor::Borrowed(executor),
                self.port.take().ok_or(LifecycleError::ManualRecoveryRequired)?,
            )),
            phase: Phase::Fresh,
            admission: None,
            interval: None,
            origin: Some(origin),
        };
        let outcome = operation.connect_full(profile)?;
        let (_, port) = operation
            .owned
            .take()
            .ok_or(LifecycleError::ManualRecoveryRequired)?;
        // A positive lower operation returns its SAME completed original port,
        // never a reconstructed session or caller-supplied success receipt.
        self.port = Some(port);
        self.phase = operation.phase;
        Ok(outcome)
    }

    pub(crate) fn disconnect<H: ProtectedHost>(
        &mut self,
        executor: &mut LifecycleExecutor<H>,
        origin: &mut dyn FnMut() -> Result<(), LifecycleError>,
    ) -> Result<LifecycleOutcome, LifecycleError> {
        let Phase::Armed(generation) = self.phase else {
            return Err(LifecycleError::ManualRecoveryRequired);
        };
        self.phase = Phase::Poisoned;
        let mut operation = ProtectedCandidate {
            owned: Some((
                Executor::Borrowed(executor),
                self.port.take().ok_or(LifecycleError::ManualRecoveryRequired)?,
            )),
            phase: Phase::Armed(generation),
            admission: None,
            interval: None,
            origin: Some(origin),
        };
        operation.origin_check()?;
        // Preserve the complete live binding/package/store/directory guards;
        // no product traffic child or observer interval is requested.
        operation.local(|executor| {
            let current = executor.read()?;
            executor
                .host
                .recheck_interval(&current)
                .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
            executor.verify_connected(&current)
        })?;
        operation.phase = Phase::Armed(generation);
        let outcome = operation.disconnect()?;
        let (_, port) = operation
            .owned
            .take()
            .ok_or(LifecycleError::ManualRecoveryRequired)?;
        self.port = Some(port);
        self.phase = operation.phase;
        Ok(outcome)
    }
}

impl<P> Drop for NormalSession<P> {
    fn drop(&mut self) {
        if !matches!(self.phase, Phase::Fresh | Phase::Closed)
            && let Some(port) = self.port.take()
        {
            std::mem::forget(port);
        }
    }
}
