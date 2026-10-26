// SPDX-License-Identifier: MIT
//! Explicit Product selected-set client fault cut, never runtime authority.
//!
//! The normal client owns every request and opaque operation. This state can
//! only remove one response observation after the authenticated full write.
//! It cannot grant a permit, alter a receipt, retry or reconnect a request.
use crate::{IO_TIMEOUT, ResponseDelivery, Result, RuntimePaths, call, call_with_delivery};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const CONFIRM: &str = "development.connection_sets.confirm";
const ARMED: u8 = 0;
const ATTEMPTED: u8 = 1;
const WRITTEN: u8 = 2;
const DISCARDED: u8 = 3;

pub(super) struct CutProgress(AtomicU8);

impl CutProgress {
    pub(super) fn written(&self) {
        self.0.store(WRITTEN, Ordering::Release);
    }

    pub(super) fn discarded(&self) {
        self.0.store(DISCARDED, Ordering::Release);
    }
}

/// Noncloneable one-shot transport cut for the explicit selected-set CLI.
/// Construction does not open a socket, select targets or create an operation.
pub struct FirstConfirmReplyLoss {
    progress: Arc<CutProgress>,
}

/// Observation only: no method can arm, reset or submit the cut.
/// The CLI prints this closed diagnostic only after terminal teardown.
pub struct ReplyLossReport {
    progress: Arc<CutProgress>,
}

impl FirstConfirmReplyLoss {
    pub fn arm() -> (Self, ReplyLossReport) {
        let progress = Arc::new(CutProgress(AtomicU8::new(ARMED)));
        (
            Self {
                progress: Arc::clone(&progress),
            },
            ReplyLossReport { progress },
        )
    }

    fn delivery(&mut self, method: &str) -> Result<ResponseDelivery> {
        if method != CONFIRM {
            return Ok(ResponseDelivery::Read);
        }
        // Consume before directory/peer/framing/connect/write failures. There
        // is no new attempt after a partial write or an unavailable response.
        self.progress
            .0
            .compare_exchange(ARMED, ATTEMPTED, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| crate::RuntimeError::Protocol)?;
        Ok(ResponseDelivery::Discard(Arc::clone(&self.progress)))
    }

    pub fn call(&mut self, paths: &RuntimePaths, method: &str, params: Value) -> Result<Value> {
        if method != CONFIRM {
            return call(paths, method, params);
        }
        let delivery = self.delivery(method)?;
        call_with_delivery(paths, method, params, IO_TIMEOUT, delivery)
    }
}

impl ReplyLossReport {
    #[must_use]
    pub fn diagnostic(&self) -> &'static str {
        match self.progress.0.load(Ordering::Acquire) {
            ARMED => "omavless.reply-loss.v1 stage=not_attempted full_request_written=false",
            ATTEMPTED => "omavless.reply-loss.v1 stage=attempted full_request_written=false",
            WRITTEN => "omavless.reply-loss.v1 stage=request_written full_request_written=true",
            DISCARDED => "omavless.reply-loss.v1 stage=reply_discarded full_request_written=true",
            _ => unreachable!("closed reply-loss state"),
        }
    }
}

#[cfg(test)]
mod tests;
