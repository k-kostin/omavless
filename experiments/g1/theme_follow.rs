// SPDX-License-Identifier: MIT
//! Synthetic consumer lifecycle only; never releases the sampler's I/O permit.
use crate::theme_source::MAX_THEME_SEQUENCE;
pub const POLL_MS: u64 = 500;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    generation: u64,
    request: u64,
}
#[derive(Default)]
pub struct Follow {
    pub following: bool,
    generation: u64,
    inflight: Option<Ticket>,
    request: u64,
    last_source: u64,
}
impl Follow {
    pub fn mode(&mut self, following: bool) -> bool {
        if !following {
            self.following = false;
        }
        let Some(generation) = self
            .generation
            .checked_add(1)
            .filter(|n| *n <= MAX_THEME_SEQUENCE)
        else {
            return false;
        };
        self.generation = generation;
        self.following = following;
        // Manual/Cancel NEVER clears an in-progress read reservation.
        true
    }
    pub fn begin(&mut self) -> Option<Ticket> {
        if !self.following || self.inflight.is_some() {
            return None;
        }
        let Some(request) = self
            .request
            .checked_add(1)
            .filter(|n| *n <= MAX_THEME_SEQUENCE)
        else {
            self.following = false;
            return None;
        };
        self.request = request;
        self.inflight = Some(Ticket {
            generation: self.generation,
            request,
        });
        self.inflight
    }
    pub fn complete(&mut self, ticket: Ticket, source_sequence: u64) -> bool {
        if self.inflight != Some(ticket) {
            return false;
        }
        // Only the matching completed operation frees consumer admission.
        self.inflight = None;
        if !self.following
            || self.generation != ticket.generation
            || source_sequence <= self.last_source
            || source_sequence > MAX_THEME_SEQUENCE
        {
            return false;
        }
        self.last_source = source_sequence;
        true
    }
    pub fn failed(&mut self, ticket: Ticket) -> bool {
        if self.inflight != Some(ticket) {
            return false;
        }
        self.inflight = None;
        if !self.following || self.generation != ticket.generation {
            return false;
        }
        self.mode(false);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_cannot_free_read_and_old_reply_cannot_repaint() {
        let mut state = Follow::default();
        assert!(state.mode(true));
        let old = state.begin().unwrap();
        assert!(state.begin().is_none());
        assert!(state.mode(false));
        assert!(state.mode(true));
        assert!(state.begin().is_none());
        assert!(!state.complete(
            Ticket {
                generation: old.generation,
                request: old.request + 1
            },
            1
        ));
        assert!(state.begin().is_none());
        assert!(!state.complete(old, 1));
        let current = state.begin().unwrap();
        assert!(state.complete(current, 2));
        assert!(!state.complete(current, 3));
        let stale = state.begin().unwrap();
        assert!(!state.complete(current, 3)); // Earlier request in SAME generation.
        assert!(state.begin().is_none());
        assert!(!state.complete(stale, 2));
        let bounded = state.begin().unwrap();
        assert!(!state.complete(bounded, MAX_THEME_SEQUENCE + 1));
        let fault = state.begin().unwrap();
        assert!(state.failed(fault));
        assert!(!state.following && state.begin().is_none());
        assert_eq!(POLL_MS, 500);
        state.generation = MAX_THEME_SEQUENCE;
        state.following = true;
        assert!(!state.mode(false) && !state.following);
    }
}
