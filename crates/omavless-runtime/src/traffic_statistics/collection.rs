// SPDX-License-Identifier: MIT
//! Capture and final fences are memory-only. All durable-owner/desired reads
//! and all controller/sysfs collection run outside the general dispatcher.
use crate::{RuntimeDispatcher, cutover, desired, native_host::StatsReadContext};
use std::sync::{Mutex, Weak};

#[derive(Clone)]
pub(crate) struct Capture {
    pub(crate) desired: desired::DesiredPaths,
    pub(crate) ownership: cutover::CutoverPaths,
    pub(crate) uid: u32,
    pub(crate) revision: u64,
    pub(crate) owner_generation: u64,
}

impl Capture {
    pub(crate) fn desired(&self) -> Option<desired::DesiredState> {
        // Validate the existing private parent without creating or repairing it.
        super::file::Store::new(&self.desired.directory, self.uid).ok()?;
        let marker = cutover::read_marker_existing(&self.ownership, self.uid).ok()?;
        if marker.phase() != cutover::OwnershipPhase::Rust
            || marker.generation() != self.owner_generation
        {
            return None;
        }
        desired::read_desired_snapshot(&self.desired, self.uid).ok()
    }
}

pub(crate) fn collect(
    dispatcher: &Weak<Mutex<RuntimeDispatcher>>,
) -> Option<crate::traffic::TrafficCounters> {
    let capture = {
        let owner = dispatcher.upgrade()?;
        let mut dispatcher = owner.try_lock().ok()?;
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return None;
        };
        owner.stats_capture()?
    };
    let desired = capture.desired()?;
    if !desired.connected {
        return None;
    }
    let context: StatsReadContext = {
        let owner = dispatcher.upgrade()?;
        let mut dispatcher = owner.try_lock().ok()?;
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return None;
        };
        owner.stats_context(&capture, &desired)?
    };
    let sample = context.collect()?;
    if capture.desired()? != desired {
        return None;
    }
    let valid = {
        let owner = dispatcher.upgrade()?;
        let mut dispatcher = owner.try_lock().ok()?;
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return None;
        };
        owner.stats_current(&capture, &desired, &context)
    };
    valid.then_some(sample)
}
