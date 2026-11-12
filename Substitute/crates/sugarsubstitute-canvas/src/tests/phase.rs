// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Integer navigation preserves one camera phase until an explicit scale or fractional rebase.
use super::*;

#[test]
fn integer_pans_compose_without_fractional_zoom_drift_or_unnecessary_repainting() -> TestResult {
    let surface = Extent::new(640, 480)?;
    for zoom in [0.048828125, 0.1, 0.37, 1.21, 1.37, 2.75] {
        let mut view = Viewport::new(surface)?;
        view.zoom_at(zoom, [177.25, 83.75])?;
        let initial = view;
        let point = initial.document_at([128.5, 127.5])?;
        let mut total = [0.0; 2];
        for delta in [[13.0, -7.0], [-5.0, 3.0], [3.0, 19.0], [-17.0, -15.0], [6.0, 0.0]] {
            let previous = view;
            view.pan_by(delta)?;
            total[0] += delta[0];
            total[1] += delta[1];
            let damage = view.repair_from(previous)?;
            assert!(damage.blit().is_some());
            assert!(damage.dirty().pixels() < surface.pixels());
            assert_eq!(view.untranslated(), initial);
            assert_eq!(view.document_at([128.5 + total[0], 127.5 + total[1]])?, point);
            let unchanged = view;
            view.zoom_at(1.0, [177.25, 83.75])?;
            view.resize(surface)?;
            assert_eq!(view, unchanged);
        }
        assert_eq!(view, initial);
    }
    Ok(())
}

#[test]
fn translation_overflow_and_fractional_rebases_discard_reuse_then_resume_exact_panning()
-> TestResult {
    let surface = Extent::new(640, 480)?;
    let mut view = Viewport::new(surface)?;
    view.zoom_at(1.37, [0.0; 2])?;
    view.pan_by([f64::from(i32::MAX - 10), 0.0])?;
    let before = view;
    view.pan_by([32.0, 1.0])?;
    assert_eq!(view.pixel_translation(), [0; 2]);
    assert_eq!(view.repair_from(before)?.dirty().pixels(), surface.pixels());
    let before = view;
    view.pan_by([0.25, 0.0])?;
    assert_eq!(view.repair_from(before)?.dirty().pixels(), surface.pixels());
    let before = view;
    view.pan_by([3.0, -5.0])?;
    assert!(view.repair_from(before)?.blit().is_some());
    let mut distant = Viewport::new(surface)?;
    distant.pan_by([-1e308, 0.0])?;
    distant.zoom_at(16384.0, [0.0; 2])?;
    assert!(!distant.intersects_document(Extent::new(1_000_000_000, 1_000_000_000)?));
    Ok(())
}
