// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Feature relationships, deterministic ties, hold and release, and smart guides.
use super::*;
use crate::tests::harness::TestResult;
use crate::tools::TransformTool;
use crate::tools::tests::editing::{MASK, canvas, run};

/// The first mask `[20, 20, 60, 60]` of a 200×200 document moving among `others`.
fn moving(
    others: &[[u32; 4]],
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<MoveSnap, crate::CanvasError> {
    let masks: Vec<[u32; 4]> =
        std::iter::once([20, 20, 60, 60]).chain(others.iter().copied()).collect();
    let canvas = canvas(context, TransformTool::new(), &masks)?;
    let snapshot = canvas.document.snapshot();
    Ok(MoveSnap::begin(&snapshot, LayerId::new(MASK)?, false, [20.0, 20.0, 60.0, 60.0]))
}

#[test]
fn only_the_canvas_centre_takes_another_feature_and_shows_a_guide() -> TestResult {
    run(|context| {
        let mut snap = moving(&[], context)?;
        // The box's centre 5 pixels from the canvas edge does not snap to it.
        assert_eq!(snap.resolve([155.0, 0.0], 1.0, false), [155.0, 0.0]);
        assert!(snap.guides().is_empty());
        // The box's start 3 pixels from the canvas centre snaps onto it.
        assert_eq!(snap.resolve([77.0, 0.0], 1.0, false), [80.0, 0.0]);
        let guide = SnapGuide { start: [100.0, 0.0], end: [100.0, 200.0] };
        assert_eq!(snap.guides(), [guide]);
        // Ctrl releases the snap and its guide.
        assert_eq!(snap.resolve([77.0, 0.0], 1.0, true), [77.0, 0.0]);
        assert!(snap.guides().is_empty());
        Ok(())
    })
}

#[test]
fn ties_take_the_lower_position_then_the_lower_layer_and_snaps_hold_until_12_pixels() -> TestResult
{
    run(|context| {
        // Starts at 45 and 55 are both 5 pixels from the moved start at 50.
        let mut snap = moving(&[[45, 150, 60, 190], [55, 150, 70, 190]], context)?;
        assert_eq!(snap.resolve([30.0, 0.0], 1.0, false), [25.0, 0.0]);
        assert_eq!(snap.guides(), [SnapGuide { start: [45.0, 20.0], end: [45.0, 190.0] }]);
        // The snap holds within 12 device pixels. Past that the moved end 2 pixels from the
        // canvas centre outranks the start 2 pixels from the first layer's end by priority.
        assert_eq!(snap.resolve([36.0, 0.0], 1.0, false), [25.0, 0.0]);
        assert_eq!(snap.resolve([38.0, 0.0], 1.0, false), [40.0, 0.0]);
        assert_eq!(snap.guides(), [SnapGuide { start: [100.0, 0.0], end: [100.0, 200.0] }]);
        // Two layers starting at one position: the lower identity's guide is shown.
        let mut snap = moving(&[[45, 150, 60, 170], [45, 100, 60, 120]], context)?;
        assert_eq!(snap.resolve([30.0, 0.0], 1.0, false), [25.0, 0.0]);
        assert_eq!(snap.guides(), [SnapGuide { start: [45.0, 20.0], end: [45.0, 170.0] }]);
        // Thresholds are device pixels: at 4× zoom, 5 document pixels are 20 device pixels.
        let mut snap = moving(&[[45, 150, 60, 170]], context)?;
        assert_eq!(snap.resolve([30.0, 0.0], 4.0, false), [30.0, 0.0]);
        Ok(())
    })
}

#[test]
fn snapping_both_axes_shows_a_vertical_then_a_horizontal_guide() -> TestResult {
    run(|context| {
        let mut snap = moving(&[], context)?;
        // Start 3 from the canvas left edge and 2 from its top edge.
        assert_eq!(snap.resolve([-17.0, -18.0], 1.0, false), [-20.0, -20.0]);
        let guides = [
            SnapGuide { start: [0.0, 0.0], end: [0.0, 200.0] },
            SnapGuide { start: [0.0, 0.0], end: [200.0, 0.0] },
        ];
        assert_eq!(snap.guides(), guides);
        Ok(())
    })
}
