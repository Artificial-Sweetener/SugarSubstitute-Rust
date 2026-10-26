// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact proof-motion cadence, coalescing and gesture-lifetime checks.

use super::*;

fn time(nanoseconds: u64) -> Duration {
    Duration::from_nanos(nanoseconds)
}

fn step(motion: &mut Motion, now: u64) -> Result<MotionStep, MotionError> {
    motion.next(time(now))?.ok_or(MotionError::TimeOverflow)
}

#[test]
fn phased_motion_returns_the_delivered_exponent_exactly_to_zero() -> Result<(), MotionError> {
    let mut motion = Motion::new(time(101), time(10), time(40))?;
    assert_eq!(motion.next(time(100))?, None);
    assert_eq!(motion.next_deadline(), Some(time(101)));

    let begin = step(&mut motion, 101)?;
    assert_eq!(begin, MotionStep { steps: 0.0, phase: ScrollPhase::Begin });
    assert_eq!(motion.next(time(110))?, None);
    assert_eq!(motion.next_deadline(), Some(time(111)));

    let first = step(&mut motion, 111)?;
    let peak = step(&mut motion, 121)?;
    let returning = step(&mut motion, 131)?;
    let end = step(&mut motion, 141)?;
    assert_eq!(first.phase, ScrollPhase::Update);
    assert_eq!(peak.phase, ScrollPhase::Update);
    assert_eq!(returning.phase, ScrollPhase::Update);
    assert_eq!(end.phase, ScrollPhase::End);
    assert!(first.steps > 0.0 && peak.steps > 0.0);
    assert!(returning.steps < 0.0 && end.steps < 0.0);
    assert!((first.steps + peak.steps - 1.5).abs() < 1.0e-12);
    assert_eq!(begin.steps + first.steps + peak.steps + returning.steps + end.steps, 0.0);
    assert!(motion.finished());
    assert_eq!(motion.next_deadline(), None);
    assert_eq!(motion.next(time(200))?, None);
    Ok(())
}

#[test]
fn late_polls_emit_only_the_newest_due_delta_without_backlog() -> Result<(), MotionError> {
    let mut motion = Motion::new(Duration::ZERO, time(10), time(100))?;
    let begin = step(&mut motion, 0)?;
    assert_eq!(begin.phase, ScrollPhase::Begin);

    let late = step(&mut motion, 35)?;
    assert_eq!(late.phase, ScrollPhase::Update);
    assert_eq!(motion.next(time(35))?, None);
    assert_eq!(motion.next_deadline(), Some(time(40)));
    let expected = 0.75 * (1.0 - (std::f64::consts::TAU * 0.3).cos());
    assert!((late.steps - expected).abs() < 1.0e-12);

    let near_end = step(&mut motion, 99)?;
    assert_eq!(near_end.phase, ScrollPhase::Update);
    assert_eq!(motion.next_deadline(), Some(time(100)));
    let end = step(&mut motion, 150)?;
    assert_eq!(end.phase, ScrollPhase::End);
    assert_eq!(begin.steps + late.steps + near_end.steps + end.steps, 0.0);
    Ok(())
}

#[test]
fn backward_time_and_invalid_or_overflowing_configuration_are_rejected() -> Result<(), MotionError>
{
    assert!(matches!(
        Motion::new(Duration::ZERO, Duration::ZERO, time(1)),
        Err(MotionError::ZeroPeriod)
    ));
    assert!(matches!(
        Motion::new(Duration::ZERO, time(1), Duration::ZERO),
        Err(MotionError::ZeroDuration)
    ));
    assert!(matches!(Motion::new(Duration::MAX, time(1), time(1)), Err(MotionError::TimeOverflow)));
    let mut motion = Motion::new(time(5), time(10), time(100))?;
    assert_eq!(motion.next(time(20))?.map(|value| value.phase), Some(ScrollPhase::Begin));
    assert_eq!(motion.next(time(19)), Err(MotionError::ClockRegression));
    Ok(())
}

#[test]
fn fractional_120_and_144_hertz_periods_keep_exact_nanosecond_slots() -> Result<(), MotionError> {
    for period in [time(8_333_333), time(6_944_444)] {
        let start = time(17);
        let duration = time(100_000_000);
        let mut motion = Motion::new(start, period, duration)?;
        assert_eq!(step(&mut motion, 17)?.phase, ScrollPhase::Begin);
        assert_eq!(motion.next_deadline(), start.checked_add(period));

        let three_periods = checked_multiply(period, 3)?;
        let late = start
            .checked_add(three_periods)
            .and_then(|value| value.checked_add(time(97)))
            .ok_or(MotionError::TimeOverflow)?;
        let update = motion.next(late)?.ok_or(MotionError::TimeOverflow)?;
        assert_eq!(update.phase, ScrollPhase::Update);
        assert!(update.steps > 0.0 && update.steps <= 1.5);
        assert_eq!(motion.next_deadline(), start.checked_add(checked_multiply(period, 4)?));
        assert_eq!(motion.next(late)?, None);
    }
    Ok(())
}
