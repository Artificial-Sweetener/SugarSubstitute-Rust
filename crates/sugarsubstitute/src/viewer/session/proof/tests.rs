// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact presentation-statistics windows across pending native display custody.

use super::*;

fn stamp(generation: u64) -> Result<FrameStamp, Box<dyn Error>> {
    Ok(FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(generation)? })
}

#[test]
fn current_gpu_expose_preparation_cannot_split_measurement_window_before_ack()
-> Result<(), Box<dyn Error>> {
    let target = stamp(1)?;
    let mut pacing = PresentationPacer::new(Duration::from_nanos(16_666_667))?;
    pacing.submitted(target, PresentationMode::Canonical, Duration::ZERO, Duration::ZERO)?;

    assert!(take_settled_presentation_stats(&mut pacing, Duration::ZERO)?.is_none());
    assert_eq!(pacing.stats().submitted, 1);
    pacing.prepared(target, Duration::from_nanos(1))?;
    assert!(take_settled_presentation_stats(&mut pacing, Duration::from_nanos(1))?.is_none());
    assert_eq!(pacing.stats().submitted, 1);

    pacing.native_accepted(target, Duration::from_nanos(2))?;
    let completed = take_settled_presentation_stats(&mut pacing, Duration::from_nanos(2))?
        .ok_or("Acknowledged presentation did not release the proof measurement boundary")?;
    assert_eq!(completed.submitted, 1);
    assert_eq!(completed.canonical_accepted, 1);
    assert_eq!(pacing.stats(), Default::default());
    Ok(())
}
