// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reproject unpublished previews against current effect geometry before native promotion.
use super::*;
use sugarsubstitute_canvas_render::backdrop::BackdropRegion;

#[test]
fn preview_backdrop_change_reprojects_without_admitting_the_obsolete_native_target()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let final_target = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(final_target)?;
    let original = {
        let snapshot = fixture
            .session
            .document
            .scene
            .as_ref()
            .ok_or("Final document absent")?
            .document
            .snapshot();
        (snapshot.identity(), snapshot.revision())
    };
    let region = |top| BackdropRegion::new([0.0, top, 8.0, 4.0], 1.0, 1.0);
    let first = fixture.session.set_backdrops(vec![region(0.0)?])?;
    let token = fixture.preview([0, 0, 255])?;
    let obsolete = fixture.offered_preview(token)?;
    let before_change = fixture.session.presented_backdrop_generation();
    assert!(before_change.is_some());
    assert!(fixture.session.preview_frame_acknowledgeable(obsolete));
    let second = fixture.session.set_backdrops(vec![region(4.0)?])?;
    assert!(second > first);
    assert_eq!(fixture.session.presented_backdrop_generation(), before_change);
    assert!(!fixture.session.preview_frame_acknowledgeable(obsolete));
    assert!(fixture.session.did_present_cpu(obsolete).is_err());
    assert_eq!(fixture.session.presented_backdrop_generation(), before_change);
    let target = fixture.offered_preview(token)?;
    assert!(target.generation > obsolete.generation);

    fixture.session.did_present_cpu(target)?;
    assert_eq!(fixture.session.presented_backdrop_generation(), Some(second));
    fixture.withdraw_preview_and_wait()?;
    let restored = {
        let snapshot = fixture
            .session
            .document
            .scene
            .as_ref()
            .ok_or("Restored final scene absent")?
            .document
            .snapshot();
        (snapshot.identity(), snapshot.revision())
    };
    assert_eq!(restored, original);
    let _directory = fixture.finish()?;
    Ok(())
}

/// Keep a real effected CPU preview in the GPU-requested embedded composition path. The external
/// device boundary is inert: exposure must reuse existing native-accepted pixels without GPU work.
#[cfg(windows)]
#[test]
fn embedded_composition_exposure_reuses_accepted_backdrop_pixels_until_material_changes()
-> Result<(), Box<dyn Error>> {
    use crate::native_canvas::{
        gpu_policy::{EmbeddedGpuPolicy, GpuRequest},
        session::attachment::Presentation,
    };
    let mut fixture = Fixture::start(true)?;
    let final_target = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(final_target)?;
    let region = |sigma| BackdropRegion::new([0.0, 0.0, 8.0, 4.0], 1.0, sigma);
    let generation = fixture.session.set_backdrops(vec![region(1.0)?])?;
    let token = fixture.preview([0, 0, 255])?;
    let target = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(target)?;
    fixture.preview_idle()?;
    assert_eq!(fixture.session.presented_backdrop_generation(), Some(generation));
    fixture.session.presentation = Presentation::Composition;
    fixture.session.config.gpu =
        Some(GpuRequest::Embedded(EmbeddedGpuPolicy::Adapter("vulkan:10de:2b85".parse()?)));
    assert!(fixture.session.document.cpu_only);
    let before = fixture.session.frame_revision;
    let pixels =
        fixture.session.selected_cpu_frame().ok_or("Accepted preview absent")?.0.pixels.as_ptr();
    for _ in 0..4 {
        fixture.session.expose();
        fixture.session.input(Input::Drawable(true))?;
        fixture.preview_step()?;
    }
    assert!(!fixture.session.expose_pending);
    assert!(fixture.session.displaying.is_none());
    assert_eq!(fixture.session.frame_revision, before);
    let frame = fixture.session.selected_cpu_frame().ok_or("Retained preview absent")?.0;
    assert_eq!(frame.target, target);
    assert_eq!(frame.pixels.as_ptr(), pixels);
    assert!(!frame.pending);
    assert_eq!(fixture.session.presented_backdrop_generation(), Some(generation));
    let changed = fixture.session.set_backdrops(vec![region(2.0)?])?;
    assert!(changed > generation);
    let mut fresh = None;
    for _ in 0..500 {
        fixture.preview_step()?;
        if let Some(target) = fixture
            .session
            .selected_cpu_frame()
            .filter(|(frame, _)| frame.pending)
            .map(|(frame, _)| frame.target)
        {
            fresh = Some(target);
            break;
        }
    }
    let fresh = fresh.ok_or("Changed material did not produce a fresh frame")?;
    assert!(fresh.generation > target.generation);
    fixture.session.did_present_cpu(fresh)?;
    assert_eq!(fixture.session.presented_backdrop_generation(), Some(changed));
    let _directory = fixture.finish()?;
    Ok(())
}
