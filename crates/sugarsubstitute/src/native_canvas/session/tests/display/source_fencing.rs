// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe old display terminals without granting them authority over replacement content.
use super::*;

#[test]
fn replaced_source_display_failure_does_not_fail_the_selected_document()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } = pending_draw_outcome(true, false)?;
    session.document.scene.as_mut().ok_or("Missing selected scene")?.source_revision = 8;
    session.selection = 8;
    session.source_sequence = 8;
    let title = session.title.clone();
    consume(&mut session, &wake)?;
    assert!(session.failed.is_none());
    assert_eq!(session.title, title);
    assert!(session.document.display_evidence.is_none());
    assert_eq!(session.pacing.stats().abandoned, 1);
    Ok(())
}

#[test]
fn current_source_display_failure_preserves_its_original_failure() -> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } = pending_draw_outcome(true, false)?;
    consume(&mut session, &wake)?;
    assert_eq!(
        session.failed.as_ref().ok_or("Current display failure was lost")?.to_string(),
        "Display boundary failure"
    );
    Ok(())
}

#[test]
fn successful_display_rejects_payload_from_another_admitted_document() -> Result<(), Box<dyn Error>>
{
    let Fixture { mut session, wake, _directory, .. } = pending_draw_outcome(false, true)?;
    let result = consume(&mut session, &wake);
    assert!(result.is_err());
    assert!(session.document.display_evidence.is_none());
    assert!(session.pending_retirement.as_ref().is_some_and(|retired| retired._drawing.is_some()));
    Ok(())
}

#[test]
fn foreign_source_cpu_display_retires_pixels_without_recycling_into_replacement()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, _) = surface::cpu_draw(true)?;
    fixture.session.document.scene.as_mut().ok_or("Missing selected scene")?.source_revision = 8;
    fixture.session.selection = 8;
    fixture.session.source_sequence = 8;
    fixture.session.document.display_buffer_extent = Some(fixture.view.surface());
    consume(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.document.display_buffers.iter().all(Option::is_none));
    assert!(fixture.session.document.cpu_projection.is_none());
    assert!(fixture.session.document.cpu_pending.is_none());
    assert!(fixture.session.pending_retirement.as_ref().is_some_and(|retired| {
        retired
            ._drawing
            .as_ref()
            .is_some_and(|draw| matches!(&draw.pixels, work::DrawPixels::Cpu(_)))
    }));
    assert!(fixture.session.failed.is_none());
    Ok(())
}
