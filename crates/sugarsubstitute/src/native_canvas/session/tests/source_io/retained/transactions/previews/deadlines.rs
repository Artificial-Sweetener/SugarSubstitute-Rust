// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original preview cutoffs and caller-retained disposal refusal.

use super::*;

#[test]
fn a_preview_waiting_for_admission_keeps_its_original_deadline_and_expiry_preserves_final()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let mut source = Some(fixture.encoded([0, 0, 255])?);
    let deadline = fixture.session.preview_now().checked_add(Duration::from_millis(1))?;
    let token =
        fixture.session.offer_preview(&mut source, deadline)?.token().ok_or("Preview refused")?;
    fixture.clock.0.fetch_add(2_000_000, Ordering::SeqCst);
    for _ in 0..500 {
        fixture.preview_step()?;
        if fixture.session.preview_status(token) == PreviewStatus::Failed {
            break;
        }
    }
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Failed);
    assert_eq!(fixture.session.document.native_presented, Some(red));
    assert!(!fixture.session.source_failed());
    let mut late = Some(fixture.encoded([0, 255, 0])?);
    assert_eq!(fixture.session.offer_preview(&mut late, deadline)?, PreviewOffer::Expired);
    assert!(late.is_some());
    while !fixture.session.retire_preview_original(&mut late)? {
        fixture.preview_step()?;
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn cancelled_native_bytes_keep_their_identical_allocation_until_storage_custody_is_available()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let memory = fixture.session.preview_resident().ok_or("Resident admission absent")?;
    let bytes = Vec::with_capacity(64);
    let pointer = bytes.as_ptr();
    let mut bytes = Some((bytes, memory.claim(64 + 128)?));
    fixture.session.pending_retirement = Some(Retired::default());
    fixture.session.deferred_retirement = Some(Retired::default());
    let cutoff = fixture.session.preview_now().checked_add(Duration::from_secs(5))?;
    fixture.session.retirement_deadline = Some(cutoff);
    fixture.session.deferred_retirement_deadline = Some(cutoff);
    assert!(!fixture.session.retire_preview_bytes(&mut bytes)?);
    assert_eq!(bytes.as_ref().ok_or("Refused byte input lost")?.0.as_ptr(), pointer);
    for _ in 0..500 {
        fixture.preview_step()?;
        if fixture.session.retire_preview_bytes(&mut bytes)? {
            break;
        }
    }
    assert!(bytes.is_none());
    drop(memory);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn a_new_final_source_fences_an_already_offered_preview_without_accepting_its_old_frame()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let token = fixture.preview([0, 0, 255])?;
    let obsolete = fixture.offered_preview(token)?;
    let mut green = Some(fixture.encoded([0, 255, 0])?);
    assert!(fixture.session.offer_source(&mut green, CacheReuse::Compatible, None)?);
    assert!(fixture.session.selected_cpu_frame().is_none());
    assert!(fixture.session.did_present_cpu(obsolete).is_err());
    fixture.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
    let (current, _, pixel) = fixture.pending_frame()?;
    assert!(current.generation > obsolete.generation);
    fixture.session.did_present_cpu(current)?;
    assert!((pixel >> 8) & 0xff > 224 && pixel & 0xff < 16);
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Cancelled);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn cutoff_fences_new_borrows_but_an_exact_already_uploaded_preview_can_acknowledge()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let first_final_io = fixture.session.first_fallback_preparation_io;
    let mut source = Some(fixture.encoded([0, 0, 255])?);
    let deadline = fixture.session.preview_now().checked_add(Duration::from_secs(3))?;
    let token =
        fixture.session.offer_preview(&mut source, deadline)?.token().ok_or("Preview refused")?;
    let target = fixture.offered_preview(token)?;
    let borrowed = fixture.session.selected_cpu_frame().ok_or("Offered preview absent")?.0.target;
    assert_eq!(borrowed, target);
    fixture.clock.0.store(deadline.as_nanos(), Ordering::SeqCst);
    assert_ne!(fixture.session.selected_cpu_frame().map(|(frame, _)| frame.target), Some(target));
    fixture.session.did_present_cpu(borrowed)?;
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Presented(target));
    assert_eq!(fixture.session.first_fallback_preparation_io, first_final_io);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn withdrawing_pending_preview_under_retirement_saturation_never_resurrects_its_original()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let token = fixture.preview([0, 0, 255])?;
    fixture.session.pending_retirement = Some(Retired::default());
    fixture.session.deferred_retirement = Some(Retired::default());
    let cutoff = fixture.session.preview_now().checked_add(Duration::from_secs(5))?;
    fixture.session.retirement_deadline = Some(cutoff);
    fixture.session.deferred_retirement_deadline = Some(cutoff);
    fixture.session.withdraw_preview()?;
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Cancelled);
    for _ in 0..500 {
        fixture.preview_step()?;
        if !fixture.session.preview_pending() {
            break;
        }
    }
    assert!(!fixture.session.preview_pending());
    assert!(fixture.session.preview_restored_empty());
    assert!(fixture.session.document.scene.as_ref().is_none_or(|scene| scene.source.is_none()));
    assert!(fixture.session.selected_cpu_frame().is_none());
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn a_new_final_selected_while_preview_is_accepted_keeps_its_latest_source_intent()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let token = fixture.preview([0, 0, 255])?;
    let blue = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(blue)?;
    fixture.session.withdraw_preview()?;
    let mut green = Some(fixture.encoded([0, 255, 0])?);
    assert!(fixture.session.offer_source(&mut green, CacheReuse::Compatible, None)?);
    let selected = fixture.session.source_intent_revision();
    fixture.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
    let (target, _, pixel) = fixture.pending_frame()?;
    assert_eq!(fixture.session.source_intent_revision(), selected);
    assert!(!fixture.session.preview_active());
    assert_ne!(target, red);
    assert_ne!(target, blue);
    assert!((pixel >> 8) & 0xff > 224 && pixel & 0xff < 16);
    fixture.session.did_present_cpu(target)?;
    assert_eq!(
        fixture.session.accepted_camera().ok_or("New final absent")?.source_revision,
        selected
    );
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn failed_resize_keeps_accepted_pixels_and_new_preview_and_size_intents_can_recover()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let token = fixture.preview([0, 255, 0])?;
    let green = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(green)?;
    fixture.preview_idle()?;
    fixture.session.resize(Extent::new(16_384, 16_384)?)?;
    fixture.preview_idle()?;
    assert_eq!(fixture.session.document.native_presented, Some(green));
    assert!(!fixture.session.source_failed());
    fixture.session.resize(Extent::new(12, 8)?)?;
    fixture.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
    let (recovered, extent, _) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(12, 8)?);
    fixture.session.did_present_cpu(recovered)?;
    let token = fixture.preview([0, 0, 255])?;
    let blue = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(blue)?;
    assert_eq!(
        fixture.session.document.tools.navigation().viewport().surface(),
        Extent::new(12, 8)?
    );
    fixture.session.resize(Extent::new(14, 8)?)?;
    fixture.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
    let (resized, extent, _) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(14, 8)?);
    fixture.session.did_present_cpu(resized)?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn failed_offer_bookkeeping_retains_complete_candidate_until_owned_close()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    fixture.session.input(Input::Drawable(false))?;
    let _token = fixture.preview([0, 0, 255])?;
    fixture.preview_idle()?;
    fixture.session.frame_revision = u64::MAX;
    fixture.session.input(Input::Drawable(true))?;
    assert!(fixture.session.poll(Duration::ZERO).is_err());
    assert!(fixture.session.preview_pending());
    assert_eq!(fixture.session.document.native_presented, Some(red));
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn external_preview_disposal_adopts_the_original_close_cutoff_before_session_closes()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let memory = fixture.session.preview_resident().ok_or("Resident admission absent")?;
    let cutoff = fixture.session.preview_now().checked_add(Duration::from_secs(1))?;
    assert_eq!(fixture.session.adopt_preview_retirement_cutoff(cutoff), cutoff);
    assert!(!fixture.session.is_closing());
    let mut bytes = Some((Vec::with_capacity(64), memory.claim(64 + 128)?));
    assert!(fixture.session.retire_preview_bytes(&mut bytes)?);
    assert_eq!(fixture.session.retirement_deadline, Some(cutoff));
    let later = cutoff.checked_add(Duration::from_secs(30))?;
    assert_eq!(fixture.session.adopt_preview_retirement_cutoff(later), cutoff);
    assert_eq!(fixture.session.retirement_deadline, Some(cutoff));
    assert!(!fixture.session.is_closing());
    drop(memory);
    let _directory = fixture.finish()?;
    Ok(())
}
