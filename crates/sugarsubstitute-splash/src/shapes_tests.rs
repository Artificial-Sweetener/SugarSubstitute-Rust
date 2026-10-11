// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify native silhouette coverage and fallible coverage-buffer allocation.

use super::{coverage, rounded};

/// Preserve independently clamped radii and the native painter's thin fill coverage.
#[test]
fn narrow_fill_retains_reference_elliptical_ends() -> Result<(), Box<dyn std::error::Error>> {
    for (scale, rows) in [
        (1.0, vec![[153, 0], [231, 0], [231, 0], [153, 0]]),
        (1.5, vec![[159, 39], [217, 88], [243, 115], [243, 115], [217, 88], [159, 39]]),
    ] {
        let path = rounded(0.0, 0.0, 1.0, 4.0, 2.0, scale).ok_or("Cannot construct thin fill")?;
        let mask = coverage(&path, 2, 6)?;
        for (row, expected) in rows.into_iter().enumerate() {
            for (actual, expected) in mask.data()[row * 2..row * 2 + 2].iter().zip(expected) {
                assert!(actual.abs_diff(expected) <= 1);
                if expected == 0 {
                    assert_eq!(*actual, 0);
                }
            }
        }
    }
    Ok(())
}
/// Report an unavailable required buffer instead of claiming a partial frame succeeded.
#[test]
fn coverage_allocation_failure_is_observable() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        rounded(0.0, 0.0, 1.0, 4.0, 2.0, 1.0).ok_or("Cannot construct allocation fixture")?;
    for (width, height) in [(0, 1), (1, 0), (u32::MAX, u32::MAX)] {
        assert!(coverage(&path, width, height).is_err());
    }
    let error =
        coverage(&path, u32::MAX, u32::MAX).err().ok_or("Allocation unexpectedly succeeded")?;
    assert!(std::error::Error::source(&error).is_some());
    Ok(())
}
