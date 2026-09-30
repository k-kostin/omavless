// SPDX-License-Identifier: MIT
//! Ephemeral, locally sampled TUN-rate history; no controller stream or disk writes.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub const WINDOW: Duration = Duration::from_secs(60);
pub const CAPACITY: usize = 40;

#[derive(Clone, Copy)]
pub struct Sample {
    pub at: Instant,
    pub upload: u64,
    pub download: u64,
}

#[derive(Default)]
pub struct History {
    samples: VecDeque<Sample>,
}

impl History {
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// A missing/invalid pair interrupts the history rather than drawing a
    /// false zero or joining samples across a runtime, revision or TUN reset.
    pub fn observe(&mut self, rates: Option<(u64, u64)>, at: Instant) {
        let Some((upload, download)) = rates else {
            self.clear();
            return;
        };
        if self.samples.back().is_some_and(|last| at <= last.at) {
            self.clear();
        }
        self.samples.push_back(Sample {
            at,
            upload,
            download,
        });
        while self.samples.len() > CAPACITY {
            self.samples.pop_front();
        }
        self.expire(at);
    }

    fn expire(&mut self, now: Instant) {
        while self
            .samples
            .front()
            .is_some_and(|first| now.saturating_duration_since(first.at) > WINDOW)
        {
            self.samples.pop_front();
        }
    }

    pub fn recent(&self, now: Instant) -> impl Iterator<Item = Sample> + '_ {
        self.samples.iter().copied().filter(move |sample| {
            sample.at <= now && now.saturating_duration_since(sample.at) <= WINDOW
        })
    }

    pub fn peak(&self, now: Instant, upload: bool) -> Option<u64> {
        self.recent(now)
            .map(|sample| {
                if upload {
                    sample.upload
                } else {
                    sample.download
                }
            })
            .max()
    }

    /// Relative to this line's own peak. A dot means an observed zero, never a
    /// missing sample; missing history is represented outside this function.
    pub fn sparkline(&self, now: Instant, upload: bool) -> Option<String> {
        let peak = self.peak(now, upload)?;
        let mut line = String::new();
        for sample in self.recent(now) {
            let value = if upload {
                sample.upload
            } else {
                sample.download
            };
            let level = if value == 0 {
                0
            } else if peak == 0 {
                1
            } else {
                (u128::from(value) * 7)
                    .div_ceil(u128::from(peak))
                    .clamp(1, 7) as usize
            };
            line.push(['·', '▁', '▂', '▃', '▄', '▅', '▆', '█'][level]);
        }
        Some(line)
    }
}
