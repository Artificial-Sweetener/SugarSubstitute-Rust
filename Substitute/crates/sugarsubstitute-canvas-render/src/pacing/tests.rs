// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic pacing state and nanosecond cadence checks.

use super::*;
use sugarsubstitute_contracts::execution::Generation;

fn time(nanoseconds: u64) -> Duration {
    Duration::from_nanos(nanoseconds)
}

fn stamp(surface: u64, generation: u64) -> Result<FrameStamp, Box<dyn std::error::Error>> {
    Ok(FrameStamp { surface: SurfaceId::new(surface)?, generation: Generation::new(generation)? })
}

#[test]
fn busy_foreign_duplicate_and_backward_observations_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let mut pacing = PresentationPacer::new(time(16_666_667))?;
    let first = stamp(7, 1)?;
    let foreign = stamp(7, 2)?;

    pacing.submitted(first, PresentationMode::Canonical, time(3), time(11))?;
    assert_eq!(
        pacing.readiness(time(11))?,
        PresentationReadiness::Busy { target: first, stage: PresentationStage::Preparing }
    );
    assert!(matches!(
        pacing.submitted(foreign, PresentationMode::Projected, time(11), time(11)),
        Err(PacingError::Busy { target, .. }) if target == first
    ));
    assert_eq!(
        pacing.prepared(foreign, time(12)),
        Err(PacingError::ForeignTarget { expected: first, observed: foreign })
    );
    assert_eq!(pacing.prepared(first, time(10)), Err(PacingError::ClockRegression));
    assert_eq!(
        pacing.native_accepted(first, time(12)),
        Err(PacingError::WrongStage {
            expected: PresentationStage::AwaitingNative,
            actual: PresentationStage::Preparing,
        })
    );
    pacing.prepared(first, time(12))?;
    assert_eq!(
        pacing.prepared(first, time(13)),
        Err(PacingError::WrongStage {
            expected: PresentationStage::Preparing,
            actual: PresentationStage::AwaitingNative,
        })
    );
    assert_eq!(
        pacing.native_accepted(foreign, time(14)),
        Err(PacingError::ForeignTarget { expected: first, observed: foreign })
    );
    pacing.native_accepted(first, time(14))?;
    assert_eq!(pacing.native_accepted(first, time(15)), Err(PacingError::NoPendingPresentation));
    Ok(())
}

#[test]
fn missed_slots_are_coalesced_and_later_work_recovers_original_fractional_phase()
-> Result<(), Box<dyn std::error::Error>> {
    let period = time(6_944_444);
    let mut pacing = PresentationPacer::new(period)?;
    let first = stamp(3, 1)?;
    pacing.submitted(first, PresentationMode::Projected, time(101), time(101))?;
    pacing.prepared(first, time(102))?;
    pacing.native_accepted(first, time(103))?;
    assert_eq!(pacing.next_deadline(time(103))?, Some(time(6_944_545)));

    let late = time(27_777_880);
    assert_eq!(pacing.readiness(late)?, PresentationReadiness::Ready);
    let second = stamp(3, 2)?;
    pacing.submitted(second, PresentationMode::Projected, time(20_000_003), late)?;
    assert_eq!(pacing.stats().skipped_slots, 3);
    assert_eq!(pacing.next_deadline(late)?, None);
    pacing.prepared(second, time(27_777_881))?;
    pacing.native_accepted(second, time(27_777_882))?;
    assert_eq!(pacing.next_deadline(time(27_777_882))?, Some(time(34_722_321)));
    assert_eq!(
        pacing.readiness(time(34_722_320))?,
        PresentationReadiness::WaitingUntil(time(34_722_321))
    );
    Ok(())
}

#[test]
fn refresh_change_rephases_deadline_without_disturbing_retained_frame()
-> Result<(), Box<dyn std::error::Error>> {
    let mut pacing = PresentationPacer::new(time(16_666_667))?;
    let first = stamp(5, 1)?;
    pacing.submitted(first, PresentationMode::Canonical, time(1_000_003), time(1_000_003))?;
    pacing.set_frame_period(time(8_333_333), time(2_000_007))?;
    assert_eq!(
        pacing.readiness(time(2_000_007))?,
        PresentationReadiness::Busy { target: first, stage: PresentationStage::Preparing }
    );
    pacing.prepared(first, time(2_000_009))?;
    pacing.native_accepted(first, time(2_000_011))?;
    assert_eq!(pacing.next_deadline(time(2_000_011))?, Some(time(9_333_336)));
    assert_eq!(pacing.period(), time(8_333_333));
    Ok(())
}

#[test]
fn aggregate_evidence_tracks_queue_acceptance_without_sample_history()
-> Result<(), Box<dyn std::error::Error>> {
    let mut pacing = PresentationPacer::new(time(10))?;
    let first = stamp(9, 1)?;
    pacing.submitted(first, PresentationMode::Canonical, time(2), time(4))?;
    pacing.prepared(first, time(7))?;
    pacing.native_accepted(first, time(11))?;

    let second = stamp(9, 2)?;
    pacing.submitted(second, PresentationMode::Projected, time(12), time(14))?;
    pacing.prepared(second, time(17))?;
    pacing.native_accepted(second, time(25))?;

    let stats = pacing.stats();
    assert_eq!(stats.submitted, 2);
    assert_eq!(stats.canonical_accepted, 1);
    assert_eq!(stats.projected_accepted, 1);
    assert_eq!(stats.last_native_acceptance, Some(time(25)));
    assert_eq!(
        stats.input_to_native,
        TimingSummary { samples: 2, total: time(22), maximum: time(13) }
    );
    assert_eq!(
        stats.native_acceptance_cadence,
        TimingSummary { samples: 1, total: time(14), maximum: time(14) }
    );
    Ok(())
}

#[test]
fn abandoned_work_releases_only_its_exact_slot_and_does_not_create_backlog()
-> Result<(), Box<dyn std::error::Error>> {
    let mut pacing = PresentationPacer::new(time(10))?;
    let first = stamp(11, 1)?;
    pacing.submitted(first, PresentationMode::Projected, Duration::ZERO, Duration::ZERO)?;
    assert_eq!(
        pacing.abandoned(stamp(11, 2)?, time(1)),
        Err(PacingError::ForeignTarget { expected: first, observed: stamp(11, 2)? })
    );
    pacing.abandoned(first, time(1))?;
    assert_eq!(pacing.readiness(time(9))?, PresentationReadiness::WaitingUntil(time(10)));
    assert_eq!(pacing.next_deadline(time(9))?, Some(time(10)));
    assert_eq!(pacing.readiness(time(35))?, PresentationReadiness::Ready);
    assert_eq!(pacing.next_deadline(time(35))?, None);
    let second = stamp(11, 2)?;
    pacing.submitted(second, PresentationMode::Canonical, time(35), time(35))?;
    assert_eq!(pacing.stats().skipped_slots, 2);
    assert_eq!(pacing.stats().abandoned, 1);
    assert_eq!(pacing.next_deadline(time(35))?, None);
    Ok(())
}

#[test]
fn target_surface_and_period_validation_are_stable() -> Result<(), Box<dyn std::error::Error>> {
    assert!(matches!(PresentationPacer::new(Duration::ZERO), Err(PacingError::ZeroPeriod)));
    let mut pacing = PresentationPacer::new(time(10))?;
    let first = stamp(13, 2)?;
    pacing.submitted(first, PresentationMode::Canonical, Duration::ZERO, Duration::ZERO)?;
    pacing.prepared(first, time(1))?;
    pacing.native_accepted(first, time(2))?;
    assert_eq!(
        pacing.submitted(stamp(14, 3)?, PresentationMode::Canonical, time(10), time(10)),
        Err(PacingError::ForeignSurface)
    );
    assert_eq!(pacing.set_frame_period(Duration::ZERO, time(3)), Err(PacingError::ZeroPeriod));
    assert_eq!(pacing.set_frame_period(time(5), time(1)), Err(PacingError::ClockRegression));
    Ok(())
}

#[test]
fn taking_stats_starts_a_new_window_without_changing_pacing_or_custody()
-> Result<(), Box<dyn std::error::Error>> {
    let mut pacing = PresentationPacer::new(time(10))?;
    let first = stamp(15, 1)?;
    pacing.submitted(first, PresentationMode::Canonical, Duration::ZERO, Duration::ZERO)?;
    pacing.prepared(first, time(1))?;
    pacing.native_accepted(first, time(2))?;

    let pending = stamp(15, 2)?;
    pacing.submitted(pending, PresentationMode::Projected, time(25), time(30))?;
    let previous = pacing.take_stats();
    assert_eq!(previous.submitted, 2);
    assert_eq!(previous.skipped_slots, 2);
    assert_eq!(previous.canonical_accepted, 1);
    assert_eq!(previous.last_native_acceptance, Some(time(2)));
    assert_eq!(pacing.stats(), PresentationStats::default());
    assert_eq!(
        pacing.readiness(time(30))?,
        PresentationReadiness::Busy { target: pending, stage: PresentationStage::Preparing }
    );
    assert_eq!(pacing.next_deadline(time(30))?, None);
    assert_eq!(pacing.prepared(pending, time(29)), Err(PacingError::ClockRegression));

    pacing.prepared(pending, time(31))?;
    pacing.native_accepted(pending, time(32))?;
    let after_pending = pacing.stats();
    assert_eq!(after_pending.submitted, 0);
    assert_eq!(after_pending.projected_accepted, 1);
    assert_eq!(after_pending.input_to_native.total, time(7));
    assert_eq!(after_pending.native_acceptance_cadence.samples, 0);
    assert_eq!(after_pending.last_native_acceptance, Some(time(32)));
    assert_eq!(pacing.next_deadline(time(32))?, Some(time(40)));

    let third = stamp(15, 3)?;
    pacing.submitted(third, PresentationMode::Canonical, time(39), time(40))?;
    pacing.prepared(third, time(41))?;
    pacing.native_accepted(third, time(42))?;
    assert_eq!(pacing.stats().native_acceptance_cadence.samples, 1);
    assert_eq!(pacing.stats().native_acceptance_cadence.total, time(10));
    assert_eq!(
        pacing.submitted(stamp(16, 1)?, PresentationMode::Canonical, time(42), time(42)),
        Err(PacingError::ForeignSurface)
    );
    Ok(())
}
