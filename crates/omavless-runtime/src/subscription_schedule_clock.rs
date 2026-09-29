// SPDX-License-Identifier: MIT

//! Inactive, pure comparison of caller-supplied clock observations.
//! This module neither reads clocks nor establishes continuity. Its advisory
//! result cannot authorize admission or settle an interrupted attempt.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSample {
    pub wall_secs: u64,
    pub elapsed_secs: u64,
}

/// A caller assertion, not evidence manufactured from matching timestamps.
/// `SameProcess` also requires uninterrupted clock provenance: process identity
/// alone does not prove that no suspend or clock-source change occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockContinuity {
    SameProcess,
    Restarted,
    Resumed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockBlock {
    MissingAnchor,
    Restarted,
    Resumed,
    UnknownContinuity,
    WallRegressed,
    ElapsedRegressed,
    ClockDisagreement,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElapsedDecision {
    Wait { remaining_secs: u64 },
    Elapsed,
}

/// Assess a delay without I/O, persistence, or a runtime caller.
///
/// Exact equal whole-second deltas are intentionally conservative. A future
/// clock adapter must define sample bracketing, drift tolerance, clock source,
/// and suspend detection separately. Equal deltas do not prove trusted time.
/// Discontinuity always refuses, including at a zero delay. This stateless
/// helper never reseeds an anchor: callers must retain an unresolved block
/// rather than turn a later apparently consistent pair into recovery evidence.
pub fn assess_elapsed(
    anchor: Option<ClockSample>,
    current: ClockSample,
    continuity: ClockContinuity,
    required_delay_secs: u64,
) -> Result<ElapsedDecision, ClockBlock> {
    match continuity {
        ClockContinuity::SameProcess => {}
        ClockContinuity::Restarted => return Err(ClockBlock::Restarted),
        ClockContinuity::Resumed => return Err(ClockBlock::Resumed),
        ClockContinuity::Unknown => return Err(ClockBlock::UnknownContinuity),
    }
    let anchor = anchor.ok_or(ClockBlock::MissingAnchor)?;
    let wall_delta = current
        .wall_secs
        .checked_sub(anchor.wall_secs)
        .ok_or(ClockBlock::WallRegressed)?;
    let elapsed_delta = current
        .elapsed_secs
        .checked_sub(anchor.elapsed_secs)
        .ok_or(ClockBlock::ElapsedRegressed)?;
    if wall_delta != elapsed_delta {
        return Err(ClockBlock::ClockDisagreement);
    }
    // Reject an unrepresentable deadline in either supplied clock domain.
    anchor
        .wall_secs
        .checked_add(required_delay_secs)
        .ok_or(ClockBlock::Overflow)?;
    anchor
        .elapsed_secs
        .checked_add(required_delay_secs)
        .ok_or(ClockBlock::Overflow)?;
    if elapsed_delta >= required_delay_secs {
        Ok(ElapsedDecision::Elapsed)
    } else {
        Ok(ElapsedDecision::Wait {
            remaining_secs: required_delay_secs - elapsed_delta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANCHOR: ClockSample = ClockSample {
        wall_secs: 10_000,
        elapsed_secs: 500,
    };

    fn advanced(delta: u64) -> ClockSample {
        ClockSample {
            wall_secs: ANCHOR.wall_secs + delta,
            elapsed_secs: ANCHOR.elapsed_secs + delta,
        }
    }

    #[test]
    fn exact_deadline_and_equal_samples() {
        for (delta, expected) in [
            (
                0,
                ElapsedDecision::Wait {
                    remaining_secs: 300,
                },
            ),
            (299, ElapsedDecision::Wait { remaining_secs: 1 }),
            (300, ElapsedDecision::Elapsed),
            (301, ElapsedDecision::Elapsed),
        ] {
            assert_eq!(
                assess_elapsed(
                    Some(ANCHOR),
                    advanced(delta),
                    ClockContinuity::SameProcess,
                    300
                ),
                Ok(expected)
            );
        }
        assert_eq!(
            assess_elapsed(Some(ANCHOR), ANCHOR, ClockContinuity::SameProcess, 0),
            Ok(ElapsedDecision::Elapsed)
        );
    }

    #[test]
    fn neither_missing_anchor_nor_discontinuity_is_elapsed() {
        assert_eq!(
            assess_elapsed(None, ANCHOR, ClockContinuity::SameProcess, 0),
            Err(ClockBlock::MissingAnchor)
        );
        for (continuity, expected) in [
            (ClockContinuity::Restarted, ClockBlock::Restarted),
            (ClockContinuity::Resumed, ClockBlock::Resumed),
            (ClockContinuity::Unknown, ClockBlock::UnknownContinuity),
        ] {
            // Even later matching samples and zero delay cannot clear a block.
            for delta in [0, 300, 86_400] {
                assert_eq!(
                    assess_elapsed(Some(ANCHOR), advanced(delta), continuity, 0),
                    Err(expected)
                );
            }
        }
    }

    #[test]
    fn clock_steps_and_unequal_progress_refuse() {
        for (wall_secs, elapsed_secs, expected) in [
            (9_999, 800, ClockBlock::WallRegressed),
            (10_300, 499, ClockBlock::ElapsedRegressed),
            (90_000, 800, ClockBlock::ClockDisagreement),
            (10_299, 800, ClockBlock::ClockDisagreement),
            (10_300, 799, ClockBlock::ClockDisagreement),
        ] {
            assert_eq!(
                assess_elapsed(
                    Some(ANCHOR),
                    ClockSample {
                        wall_secs,
                        elapsed_secs
                    },
                    ClockContinuity::SameProcess,
                    300,
                ),
                Err(expected)
            );
        }
    }

    #[test]
    fn integer_limits_never_wrap_to_elapsed() {
        for anchor in [
            ClockSample {
                wall_secs: u64::MAX,
                elapsed_secs: 0,
            },
            ClockSample {
                wall_secs: 0,
                elapsed_secs: u64::MAX,
            },
        ] {
            assert_eq!(
                assess_elapsed(Some(anchor), anchor, ClockContinuity::SameProcess, 1),
                Err(ClockBlock::Overflow)
            );
        }
        let zero = ClockSample {
            wall_secs: 0,
            elapsed_secs: 0,
        };
        let max = ClockSample {
            wall_secs: u64::MAX,
            elapsed_secs: u64::MAX,
        };
        assert_eq!(
            assess_elapsed(Some(zero), max, ClockContinuity::SameProcess, u64::MAX),
            Ok(ElapsedDecision::Elapsed)
        );
        assert_eq!(
            assess_elapsed(Some(max), max, ClockContinuity::SameProcess, 0),
            Ok(ElapsedDecision::Elapsed)
        );
    }
}
