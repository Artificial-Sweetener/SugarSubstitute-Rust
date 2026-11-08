// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise effect revisions at real draw completion, native acceptance and pan-upload boundaries.
use super::*;
use sugarsubstitute_canvas_render::backdrop::BackdropRegion;

fn region(sigma: f32) -> Result<BackdropRegion, Box<dyn Error>> {
    Ok(BackdropRegion::new([5.5, 6.25, 96.0, 28.0], 8.0, sigma)?)
}
#[test]
fn backdrop_material_revisions_are_session_owned_and_identical_regions_do_no_work()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    assert_eq!(session.set_backdrops(Vec::new())?, 0);
    assert_eq!(session.set_backdrops(vec![region(4.0)?])?, 1);
    session.expose_pending = false;
    assert_eq!(session.set_backdrops(vec![region(4.0)?])?, 1);
    assert!(!session.expose_pending);
    assert!(session.set_backdrops(vec![region(4.0)?; 3]).is_err());
    assert_eq!(session.backdrops.generation, 1);
    assert_eq!(session.set_backdrops(vec![region(6.0)?])?, 2);
    assert_eq!(session.set_backdrops(Vec::new())?, 3);
    assert_eq!(session.presented_backdrop_generation(), None);
    session.close()?;
    assert_eq!(session.presented_backdrop_generation(), None);
    assert!(session.set_backdrops(Vec::new()).is_err());
    Ok(())
}
#[test]
fn changed_backdrop_rejects_an_already_completed_draw_before_native_publication()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, target) = surface::cpu_draw(true)?;
    loop {
        let observed = fixture.wake.revision();
        let runtime = fixture.session.runtime.as_ref().ok_or("Draw execution absent")?;
        runtime.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        let drawing = fixture.session.displaying.as_ref().ok_or("Draw identity absent")?;
        if matches!(drawing.job.status().phase, JobPhase::Terminal(TerminalKind::Success)) {
            break;
        }
        fixture.wake.wait_after(observed)?;
    }
    assert_eq!(fixture.session.set_backdrops(vec![region(4.0)?])?, 1);
    consume(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.document.cpu_pending.is_none());
    assert!(fixture.session.document.pending_cpu_evidence.is_none());
    assert!(fixture.session.document.native_presented.is_none());
    assert_eq!(fixture.session.presented_backdrop_generation(), None);
    assert!(fixture.session.did_present_cpu(target).is_err());
    assert!(fixture.session.failed.is_none());
    Ok(())
}
#[test]
fn backdrop_receipt_advances_only_native_acceptance_and_changed_geometry_withdraws_pending_pixels()
-> Result<(), Box<dyn Error>> {
    let (mut headless, _) = surface::cpu_draw(false)?;
    consume(&mut headless.session, &headless.wake)?;
    assert_eq!(headless.session.presented_backdrop_generation(), None);
    let (mut fixture, target) = surface::cpu_draw(true)?;
    consume(&mut fixture.session, &fixture.wake)?;
    assert_eq!(fixture.session.presented_backdrop_generation(), None);
    fixture.session.did_present_cpu(target)?;
    assert_eq!(fixture.session.presented_backdrop_generation(), Some(0));
    let (mut pending, obsolete) = surface::cpu_draw(true)?;
    consume(&mut pending.session, &pending.wake)?;
    assert_eq!(pending.session.set_backdrops(vec![region(4.0)?])?, 1);
    assert!(pending.session.document.cpu_pending.is_none());
    assert!(pending.session.did_present_cpu(obsolete).is_err());
    assert_eq!(pending.session.presented_backdrop_generation(), None);
    Ok(())
}
#[test]
fn backdrop_and_clearing_frames_never_pan_blit_baked_viewport_pixels() -> Result<(), Box<dyn Error>>
{
    let (mut fixture, target) = surface::cpu_draw(true)?;
    consume(&mut fixture.session, &fixture.wake)?;
    fixture.session.did_present_cpu(target)?;
    let previous = fixture.session.document.display_evidence.ok_or("Display evidence absent")?;
    let mut view = previous.view;
    view.pan_by([1.0, 0.0])?;
    let current = DisplayEvidence { view, ..previous };
    assert!(fixture.session.cpu_display_damage(current)?.0.blit().is_some());
    let effected = DisplayEvidence { backdrop_active: true, ..current };
    let (damage, base) = fixture.session.cpu_display_damage(effected)?;
    assert!(damage.blit().is_none());
    assert!(base.is_none());
    fixture.session.document.display_evidence =
        Some(DisplayEvidence { backdrop_active: true, ..previous });
    let (damage, base) = fixture.session.cpu_display_damage(current)?;
    assert!(damage.blit().is_none());
    assert!(base.is_none());
    assert!(damage.dirty().pixels() >= view.surface().pixels());
    Ok(())
}

#[test]
fn backdrop_revocation_clears_native_receipt_and_preserves_monotonic_reattachment()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    let first = session.set_backdrops(vec![region(4.0)?])?;
    session.revoke_presentation()?;
    let revoked = session.backdrops.generation;
    assert!(revoked > first);
    assert!(session.backdrops.regions.is_empty());
    assert_eq!(session.presented_backdrop_generation(), None);
    session.revoke_presentation()?;
    assert_eq!(session.backdrops.generation, revoked);
    assert!(session.set_backdrops(vec![region(4.0)?])? > revoked);
    session.close()?;
    assert!(session.backdrops.regions.is_empty());
    assert_eq!(session.presented_backdrop_generation(), None);
    Ok(())
}
