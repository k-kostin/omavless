// SPDX-License-Identifier: MIT
//! Private dormant service wiring. The event stream and time are constructed
//! by an owned fixture, never selected by the control protocol or normal bind.

use crate::lifecycle::LifecycleHost;
use crate::native_coordinator::network_resume::ResumeBinding;
use crate::network_resume::{Source, Status};
use crate::production_owner::ProductionNativeOwner;
use omavless_control_protocol::StableErrorCode;
use serde_json::{Value, json};
use std::time::Instant;

pub(crate) const METHOD: &str = "developer.network_resume.get";
pub(crate) const MAX_FRAMES_PER_WAKE: usize = 4;

pub(crate) struct Enrollment {
    pub(crate) boot: [u8; 16],
    pub(crate) instance: [u8; 16],
    pub(crate) epoch: u64,
}
pub(crate) struct OwnerInputs {
    pub(crate) desired: crate::desired::DesiredPaths,
    pub(crate) store: std::path::PathBuf,
    pub(crate) cutover: crate::cutover::CutoverPaths,
    pub(crate) enrollment: Enrollment,
}

pub(crate) struct Clock {
    origin: Instant,
    #[cfg(test)]
    injected: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
}
impl Clock {
    pub(crate) fn monotonic() -> Self {
        Self {
            origin: Instant::now(),
            #[cfg(test)]
            injected: None,
        }
    }
    pub(crate) fn now(&self) -> u64 {
        #[cfg(test)]
        if let Some(clock) = &self.injected {
            return clock.load(std::sync::atomic::Ordering::Acquire);
        }
        self.origin.elapsed().as_secs()
    }
    #[cfg(test)]
    pub(crate) fn injected(clock: std::sync::Arc<std::sync::atomic::AtomicU64>) -> Self {
        Self {
            origin: Instant::now(),
            injected: Some(clock),
        }
    }
}

pub(crate) struct Driver {
    pub(crate) source: Source,
    pub(crate) clock: Clock,
    terminal: bool,
}
impl Driver {
    pub(crate) fn new(source: Source, clock: Clock) -> Self {
        Self {
            source,
            clock,
            terminal: false,
        }
    }
}

type Wake<H> = fn(&mut ProductionNativeOwner<H>, &mut Driver);
type Get<H> = fn(&ProductionNativeOwner<H>, &Driver) -> Value;
type Lost<H> = fn(&mut ProductionNativeOwner<H>, &mut Driver);

pub(crate) struct Registration<H> {
    pub(crate) driver: Driver,
    wake: Wake<H>,
    get: Get<H>,
    lost: Lost<H>,
}
impl<H: LifecycleHost> Registration<H> {
    pub(crate) fn new(driver: Driver) -> Self
    where
        H: ResumeBinding,
    {
        // Only these fixed monomorphized library hooks are manufactured; no
        // caller callback, function address, descriptor or IPC value is accepted.
        Self {
            driver,
            wake: wake::<H>,
            get: get::<H>,
            lost: lost::<H>,
        }
    }
    pub(crate) fn wake(&mut self, owner: &mut ProductionNativeOwner<H>) {
        (self.wake)(owner, &mut self.driver);
    }
    pub(crate) fn get(&self, owner: &ProductionNativeOwner<H>) -> Value {
        (self.get)(owner, &self.driver)
    }
    pub(crate) fn lost(&mut self, owner: &mut ProductionNativeOwner<H>) {
        (self.lost)(owner, &mut self.driver);
    }
}

fn lost<H: ResumeBinding>(owner: &mut ProductionNativeOwner<H>, driver: &mut Driver) {
    driver.terminal = true;
    owner.batch_coordinator().resume_service_lost();
}
fn get<H: ResumeBinding>(owner: &ProductionNativeOwner<H>, driver: &Driver) -> Value {
    // Pure cached projection: no clock read, peek, drain, file read, observation
    // or initialization. It is not a current host-source/connection health claim.
    json!({"schema":1,"scope":"dormant_network_resume","registered":true,
        "terminal":driver.terminal,"state":owner.network_resume_status()})
}
fn wake<H: ResumeBinding>(owner: &mut ProductionNativeOwner<H>, driver: &mut Driver) {
    if driver.terminal {
        return;
    }
    for _ in 0..MAX_FRAMES_PER_WAKE {
        match driver.source.readable() {
            Ok(false) => break,
            Err(_) => {
                lost(owner, driver);
                return;
            }
            Ok(true) => owner
                .batch_coordinator()
                .resume_receive(&mut driver.source, driver.clock.now()),
        }
        if terminal(owner.network_resume_status()) {
            driver.terminal = true;
            return;
        }
    }
    match driver.source.readable() {
        Ok(true) => return, // retain bounded backlog; never skip a later Suspend
        Err(_) => {
            lost(owner, driver);
            return;
        }
        Ok(false) => {}
    }
    owner.batch_coordinator().resume_poll(
        &mut driver.source,
        driver.clock.now(),
        false,
        false,
        None,
    );
    if terminal(owner.network_resume_status()) {
        driver.terminal = true;
    }
}
fn terminal(status: Status) -> bool {
    matches!(
        status,
        Status::Cancelled | Status::Recovered | Status::ManualRecovery | Status::SourceUnavailable
    )
}

pub(crate) fn validate_get(request: &Value, instance: &str) -> Result<(), StableErrorCode> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| StableErrorCode::InvalidArgument)?;
    let params = request["params"]
        .as_object()
        .ok_or(StableErrorCode::InvalidArgument)?;
    if request["method"] != METHOD || params.len() != 1 {
        return Err(StableErrorCode::InvalidArgument);
    }
    if params.get("instanceId").and_then(Value::as_str) != Some(instance) {
        return Err(StableErrorCode::Conflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_get_parser_rejects_clock_profile_fd_path_and_old_instance_input() {
        let valid = crate::make_request("get", METHOD, json!({"instanceId":"current"})).unwrap();
        assert_eq!(validate_get(&valid, "current"), Ok(()));
        assert_eq!(
            validate_get(&valid, "other"),
            Err(StableErrorCode::Conflict)
        );
        for field in ["clock", "profileId", "fd", "path", "command"] {
            let mut request = valid.clone();
            request["params"][field] = json!("not-a-producer-option");
            assert_eq!(
                validate_get(&request, "current"),
                Err(StableErrorCode::InvalidArgument)
            );
        }
    }
}
