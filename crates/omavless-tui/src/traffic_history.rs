// SPDX-License-Identifier: MIT
//! Ephemeral, locally sampled TUN-rate history; no controller stream or disk writes.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub const WINDOW: Duration = Duration::from_secs(60);
pub const LONG_WINDOW: Duration = Duration::from_secs(300);
pub const CAPACITY: usize = 300;
const MAX_SPARKLINE: usize = 40;
const MAX_SAMPLE_GAP: Duration = Duration::from_secs(12);

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
        if self.samples.back().is_some_and(|last| {
            at <= last.at || at.saturating_duration_since(last.at) > MAX_SAMPLE_GAP
        }) {
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
            .is_some_and(|first| now.saturating_duration_since(first.at) > LONG_WINDOW)
        {
            self.samples.pop_front();
        }
    }

    pub fn recent(&self, now: Instant) -> impl Iterator<Item = Sample> + '_ {
        self.recent_window(now, WINDOW)
    }

    pub fn recent_window(
        &self,
        now: Instant,
        window: Duration,
    ) -> impl Iterator<Item = Sample> + '_ {
        self.samples.iter().copied().filter(move |sample| {
            sample.at <= now && now.saturating_duration_since(sample.at) <= window.min(LONG_WINDOW)
        })
    }

    pub fn peak(&self, now: Instant, upload: bool) -> Option<u64> {
        self.peak_window(now, upload, WINDOW)
    }

    pub fn peak_window(&self, now: Instant, upload: bool, window: Duration) -> Option<u64> {
        self.recent_window(now, window)
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
        self.sparkline_window(now, upload, WINDOW)
    }

    pub fn sparkline_window(&self, now: Instant, upload: bool, window: Duration) -> Option<String> {
        let values: Vec<u64> = self
            .recent_window(now, window)
            .map(|sample| {
                if upload {
                    sample.upload
                } else {
                    sample.download
                }
            })
            .collect();
        let peak = values.iter().copied().max()?;
        let plotted: Vec<u64> = if values.len() <= MAX_SPARKLINE {
            values
        } else {
            (0..MAX_SPARKLINE)
                .map(|bucket| {
                    let start = bucket * values.len() / MAX_SPARKLINE;
                    let end = (bucket + 1) * values.len() / MAX_SPARKLINE;
                    values[start..end].iter().copied().max().unwrap_or_default()
                })
                .collect()
        };
        let mut line = String::new();
        for value in plotted {
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

    pub fn has_long_trend(&self, now: Instant) -> bool {
        self.recent_window(now, LONG_WINDOW)
            .next()
            .is_some_and(|first| now.saturating_duration_since(first.at) > WINDOW)
    }
}
