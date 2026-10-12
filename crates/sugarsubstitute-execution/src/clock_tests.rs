// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove presentation mapping retains the actual execution clock epoch.
use super::*;
#[test]
fn mapped_instants_preserve_zero_epoch_and_nanosecond_offsets() -> Result<(), TimeError> {
    let epoch = Instant::now();
    let clock = MonotonicClock { epoch };
    assert_eq!(clock.instant_for(Moment::from_nanos(0))?, epoch);
    let first = clock.instant_for(Moment::from_nanos(17))?;
    let second = clock.instant_for(Moment::from_nanos(119))?;
    assert_eq!(first.duration_since(epoch), Duration::from_nanos(17));
    assert_eq!(second.duration_since(first), Duration::from_nanos(102));
    Ok(())
}
#[test]
fn observed_moment_maps_into_the_same_real_clock_interval() -> Result<(), TimeError> {
    let before = Instant::now();
    let clock = MonotonicClock::new();
    let observed = clock.now();
    let mapped = clock.instant_for(observed)?;
    let after = Instant::now();
    assert!(mapped >= before);
    assert!(mapped <= after);
    assert_eq!(clock.instant_for(Moment::from_nanos(0))?, clock.epoch);
    Ok(())
}
