// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize bounded presentation pacing evidence without implying physical display timing.

use serde::Serialize;
use std::time::Duration;
use sugarsubstitute_canvas_render::{PresentationStats, TimingSummary};

/// Bounded evidence from actual presentation preparation and native queue acknowledgements.
///
/// Native queue acceptance proves that the platform presentation API accepted an upload. It does
/// not establish compositor display time, physical scanout or synchronization to vertical blank.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct PresentationEvidence {
    submitted: u64,
    abandoned: u64,
    skipped_slots: u64,
    canonical_native_queue_acceptances: u64,
    projected_native_queue_acceptances: u64,
    input_to_native_queue_acceptance: TimingEvidence,
    native_queue_acceptance_cadence: TimingEvidence,
}

impl PresentationEvidence {
    /// Convert process-lifetime aggregates into stable integer-nanosecond proof evidence.
    pub(super) fn new(stats: PresentationStats) -> Self {
        Self {
            submitted: stats.submitted,
            abandoned: stats.abandoned,
            skipped_slots: stats.skipped_slots,
            canonical_native_queue_acceptances: stats.canonical_accepted,
            projected_native_queue_acceptances: stats.projected_accepted,
            input_to_native_queue_acceptance: stats.input_to_native.into(),
            native_queue_acceptance_cadence: stats.native_acceptance_cadence.into(),
        }
    }
}

/// GPU readback paths that materially affect every presentation-timing evidence window.
///
/// Canonical capture and native display verification are independent. Native queue timing never
/// establishes compositor display time, physical scanout or synchronization to vertical blank.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct PresentationTimingContext {
    /// Whether canonical GPU image preparation includes proof snapshot readback.
    pub(super) canonical_gpu_snapshot_readback: bool,
    /// Whether native GPU display preparation includes verification readback.
    pub(super) native_gpu_display_verification_readback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
struct TimingEvidence {
    samples: u64,
    total_nanoseconds: u64,
    maximum_nanoseconds: u64,
}

impl From<TimingSummary> for TimingEvidence {
    fn from(summary: TimingSummary) -> Self {
        Self {
            samples: summary.samples,
            total_nanoseconds: saturating_nanoseconds(summary.total),
            maximum_nanoseconds: saturating_nanoseconds(summary.maximum),
        }
    }
}

fn saturating_nanoseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_serializes_explicit_native_queue_units_and_readback_context()
    -> Result<(), serde_json::Error> {
        let evidence = PresentationEvidence::new(PresentationStats {
            submitted: 9,
            abandoned: 2,
            skipped_slots: 4,
            canonical_accepted: 3,
            projected_accepted: 4,
            input_to_native: TimingSummary {
                samples: 7,
                total: Duration::from_nanos(123_456_789),
                maximum: Duration::from_nanos(31_337_009),
            },
            native_acceptance_cadence: TimingSummary {
                samples: 6,
                total: Duration::from_nanos(99_999_997),
                maximum: Duration::from_nanos(16_666_667),
            },
            ..PresentationStats::default()
        });

        assert_eq!(
            serde_json::to_value(evidence)?,
            json!({
                "submitted": 9,
                "abandoned": 2,
                "skipped_slots": 4,
                "canonical_native_queue_acceptances": 3,
                "projected_native_queue_acceptances": 4,
                "input_to_native_queue_acceptance": {
                    "samples": 7,
                    "total_nanoseconds": 123_456_789,
                    "maximum_nanoseconds": 31_337_009
                },
                "native_queue_acceptance_cadence": {
                    "samples": 6,
                    "total_nanoseconds": 99_999_997,
                    "maximum_nanoseconds": 16_666_667
                }
            })
        );
        assert_eq!(
            serde_json::to_value(PresentationTimingContext {
                canonical_gpu_snapshot_readback: true,
                native_gpu_display_verification_readback: false,
            })?,
            json!({
                "canonical_gpu_snapshot_readback": true,
                "native_gpu_display_verification_readback": false
            })
        );
        Ok(())
    }

    #[test]
    fn duration_conversion_saturates_without_losing_aggregate_counts()
    -> Result<(), serde_json::Error> {
        let beyond_nanoseconds = Duration::new(u64::MAX, 999_999_999);
        let evidence = PresentationEvidence::new(PresentationStats {
            input_to_native: TimingSummary {
                samples: u64::MAX,
                total: beyond_nanoseconds,
                maximum: beyond_nanoseconds,
            },
            ..PresentationStats::default()
        });
        let serialized = serde_json::to_value(evidence)?;

        assert_eq!(serialized["input_to_native_queue_acceptance"]["samples"], u64::MAX);
        assert_eq!(serialized["input_to_native_queue_acceptance"]["total_nanoseconds"], u64::MAX);
        assert_eq!(serialized["input_to_native_queue_acceptance"]["maximum_nanoseconds"], u64::MAX);
        Ok(())
    }
}
