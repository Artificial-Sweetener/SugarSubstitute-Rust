// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve source custody and the pending inspection when a restored transform is refused.

use super::*;

/// A camera outside interactive pan bounds, independent of the one-pixel fixture source.
fn camera(center: [f64; 2]) -> NavigationCommand {
    NavigationCommand::Frame { center, span: [0.1; 2] }
}

#[test]
fn invalid_loaded_source_camera_retains_the_preceding_document_and_inspection()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    let original = fixture.session.document.shown_key();
    let viewport = fixture.session.document.tools.navigation().viewport();
    let mut source = Some(fixture.output([0, 0, 255])?);
    assert!(fixture.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
    let invalid = NavigationCommand::Frame { center: [0.5; 2], span: [1e-20; 2] };
    fixture.session.frame_selection(Some(invalid));
    let intent = fixture.session.selection;
    loop {
        let observed = fixture.wake.revision();
        fixture.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        fixture.session.poll(Duration::ZERO)?;
        if fixture.session.failed.is_some() {
            break;
        }
        fixture.wake.wait_after(observed);
    }
    assert_eq!(fixture.session.document.shown_key(), original);
    assert_eq!(fixture.session.document.tools.navigation().viewport(), viewport);
    assert_eq!(fixture.session.selection_frame(intent), Some(invalid));
    assert!(fixture.session.failed.as_ref().is_some_and(|cause| !cause.to_string().is_empty()));
    let cause = fixture.finish().err().ok_or("Expected the retained camera failure at teardown")?;
    assert!(cause.to_string().contains("Invalid camera scale"));
    Ok(())
}

#[test]
fn invalid_retained_source_camera_keeps_both_documents_in_their_previous_custody()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 0, 255], "b")?;
    let original = fixture.session.document.shown_key();
    let viewport = fixture.session.document.tools.navigation().viewport();
    assert!(fixture.session.reselect_retained(&key("a"))?);
    let invalid = NavigationCommand::Frame { center: [0.5; 2], span: [1e-20; 2] };
    fixture.session.frame_selection(Some(invalid));
    let intent = fixture.session.selection;
    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.failed.is_some());
    assert_eq!(fixture.session.document.shown_key(), original);
    assert_eq!(fixture.session.document.tools.navigation().viewport(), viewport);
    assert!(fixture.session.retained.contains(&key("a")));
    assert_eq!(fixture.session.selection_frame(intent), Some(invalid));
    fixture.session.cancel_selection()?;
    assert!(fixture.session.initial_frame.is_none());
    let cause = fixture.finish().err().ok_or("Expected the retained camera failure at teardown")?;
    assert!(cause.to_string().contains("Invalid camera scale"));
    Ok(())
}

#[test]
fn reselected_source_camera_preserves_off_image_center_through_physical_resize()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 0, 255], "b")?;
    assert!(fixture.session.reselect_retained(&key("a"))?);
    let center = [-3.25, 2.75];
    fixture.session.frame_selection(Some(camera(center)));
    fixture.settle()?;
    assert!(fixture.session.initial_frame.is_none());
    for surface in [Extent::new(16, 12)?, Extent::new(8, 8)?] {
        fixture.session.resize(surface)?;
        fixture.settle()?;
        let accepted = fixture.session.accepted_camera().ok_or("Restored camera absent")?;
        let panel = [f64::from(surface.width()) * 0.5, f64::from(surface.height()) * 0.5];
        let actual = accepted.viewport.document_at(panel)?;
        assert!((actual[0] - center[0]).abs() < 1e-9 && (actual[1] - center[1]).abs() < 1e-9);
        assert_eq!(accepted.viewport.zoom(), 10.0);
    }
    fixture.finish()?;
    Ok(())
}

#[test]
fn denied_retained_camera_preserves_the_current_document_and_pending_frame()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    let now = fixture.session.navigation_time();
    let document = &mut fixture.session.document;
    let scene = document.scene.as_ref().map(|scene| &scene.document);
    document.tools.set_policy(true, false, scene, now)?;
    fixture.show([0, 0, 255], "b")?;
    let original = fixture.session.document.shown_key();
    let viewport = fixture.session.document.tools.navigation().viewport();
    assert!(fixture.session.reselect_retained(&key("a"))?);
    let frame = camera([-3.25, 2.75]);
    fixture.session.frame_selection(Some(frame));
    let intent = fixture.session.selection;
    fixture.session.poll(Duration::ZERO)?;
    assert!(
        fixture
            .session
            .failed
            .as_ref()
            .is_some_and(|cause| cause.to_string().contains("owned by another operation"))
    );
    assert_eq!(fixture.session.document.shown_key(), original);
    assert_eq!(fixture.session.document.tools.navigation().viewport(), viewport);
    assert!(fixture.session.retained.contains(&key("a")));
    assert_eq!(fixture.session.selection_frame(intent), Some(frame));
    let cause = fixture.finish().err().ok_or("Expected the retained refusal at teardown")?;
    assert!(cause.to_string().contains("owned by another operation"));
    Ok(())
}

/// Start a real pan capture after the selected source's accepted camera can pan.
fn capture_selected_pan(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    use sugarsubstitute_canvas::{ToolButton, ToolInput, ToolModifiers};
    let now = fixture.session.navigation_time();
    fixture.session.document.tools.navigate(camera([0.5; 2]), now)?;
    fixture.settle()?;
    let now = fixture.session.navigation_time();
    let document = &mut fixture.session.document;
    let scene = document.scene.as_mut().map(|scene| &mut scene.document);
    document.tools.input(
        ToolInput::Button {
            button: ToolButton::Primary,
            pressed: true,
            modifiers: ToolModifiers::NONE,
        },
        scene,
        now,
    )?;
    assert!(document.tools.captured());
    Ok(())
}

/// Exercise the same document-change handoff for ordinary Fit and explicit saved cameras.
fn captured_selection_handoff(
    parked: bool,
    frame: Option<NavigationCommand>,
) -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    capture_selected_pan(&mut fixture)?;
    let original = fixture.session.document.shown_key();
    if parked {
        fixture.show([0, 0, 255], "b")?;
        assert!(
            fixture
                .session
                .retained
                .document(&key("a"))
                .is_some_and(|document| document.tools.captured())
        );
    }
    assert!(fixture.session.reselect_retained(&key("a"))?);
    fixture.session.frame_selection(frame);
    let intent = fixture.session.selection;
    let settled = fixture.settle().map_err(|cause| cause.to_string());
    let shown = fixture.session.document.shown_key();
    let revision = fixture.session.document.scene.as_ref().map(|scene| scene.source_revision);
    let captured = fixture.session.document.tools.captured();
    let pending = fixture.session.selection_frame(intent);
    let accepted = fixture.session.accepted_camera();
    let retained_outgoing = !parked || fixture.session.retained.contains(&key("b"));
    let closed = fixture.finish().map_err(|cause| cause.to_string());
    assert!(settled.is_ok(), "Selection must use the document-change tool lifecycle: {settled:?}");
    assert!(closed.is_ok(), "Selection teardown must observe success: {closed:?}");
    assert_eq!(shown, original);
    assert_eq!(revision, Some(intent));
    assert!(!captured);
    assert_eq!(pending, None);
    assert!(retained_outgoing);
    let accepted = accepted.ok_or("Selected source camera absent")?;
    let surface = accepted.viewport.surface();
    let center = accepted
        .viewport
        .document_at([f64::from(surface.width()) * 0.5, f64::from(surface.height()) * 0.5])?;
    let expected = if frame.is_some() { [-3.25, 2.75] } else { [0.5; 2] };
    assert!((center[0] - expected[0]).abs() < 1e-9 && (center[1] - expected[1]).abs() < 1e-9);
    assert_eq!(accepted.viewport.zoom(), if frame.is_some() { 10.0 } else { 8.0 });
    Ok(())
}

#[test]
fn captured_parked_fit_selection_uses_document_change_cancellation() -> Result<(), Box<dyn Error>> {
    captured_selection_handoff(true, None)
}

#[test]
fn captured_current_selection_applies_the_frame_after_document_change_cancellation()
-> Result<(), Box<dyn Error>> {
    captured_selection_handoff(false, Some(camera([-3.25, 2.75])))
}

#[test]
fn captured_parked_selection_applies_the_frame_after_document_change_cancellation()
-> Result<(), Box<dyn Error>> {
    captured_selection_handoff(true, Some(camera([-3.25, 2.75])))
}
