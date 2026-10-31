// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify latest-candidate failure, exact lease rejection and actual restored display snapshots.
use super::*;

/// One native adapter step: upload a pending frame, acknowledge a new hide, or stay idle.
enum NativeStep {
    Upload(FrameStamp, PhysicalRect),
    Hide,
    Idle,
}

/// Mirror the native adapter until the geometry request settles: upload every pending frame,
/// acknowledge each transition to hidden exactly once and collect every published observation.
fn settle_geometry(
    fixture: &mut Fixture,
    id: u64,
) -> Result<(Vec<Value>, Option<PhysicalRect>), Box<dyn Error>> {
    let mut changes = Vec::new();
    let (mut applied, mut hidden, mut uploaded) = (false, false, None);
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            match command {
                DesktopStartupCommand::SendSurfacePresentation(change) => {
                    changes.push(serde_json::from_slice::<Value>(change.payload())?);
                }
                DesktopStartupCommand::SendSurface(reply) => {
                    let value: Value = serde_json::from_slice(reply.payload())?;
                    assert_eq!(value["requestId"], id);
                    assert_eq!(value["status"], "applied");
                    applied = true;
                }
                _ => return Err("Unexpected geometry settlement command".into()),
            }
        }
        let step = match fixture.canvas.presentation() {
            MainCanvasPresentation::Frame(frame) if frame.content.pending() => {
                NativeStep::Upload(frame.content.target(), frame.placement.viewport)
            }
            MainCanvasPresentation::Hide if !hidden => NativeStep::Hide,
            _ => NativeStep::Idle,
        };
        match step {
            NativeStep::Upload(target, viewport) => {
                hidden = false;
                fixture.canvas.presented(target)?;
                uploaded = Some(viewport);
            }
            NativeStep::Hide => {
                hidden = true;
                fixture.canvas.hidden(fixture.identity)?;
            }
            NativeStep::Idle if applied => return Ok((changes, uploaded)),
            NativeStep::Idle => fixture.wait_preview_progress(observed)?,
        }
    }
    Err("Geometry request never settled on native pixels".into())
}

#[test]
fn resizing_a_displayed_locked_preview_refits_it_at_fit_instead_of_cancelling_it()
-> Result<(), Box<dyn Error>> {
    for superseded in [true, false] {
        let mut fixture = Fixture::new()?;
        fixture.commit(4)?;
        watch(&mut fixture, 100, true)?;
        transfer(&mut fixture, 200, "preview:1", JPEG)?;
        let target = wait_frame(&mut fixture)?;
        fixture.canvas.presented(target)?;
        let shown = wait_change(&mut fixture, "presented")?;
        if superseded {
            fixture.geometry(323, 80, 40)?;
            fixture.take(323, "applied")?;
            fixture.cancel(323)?;
            cancel_consumed(&mut fixture.canvas, fixture.identity)?;
        }
        fixture.geometry(324, 60, 30)?;
        let (changes, uploaded) = settle_geometry(&mut fixture, 324)?;
        assert!(
            changes.iter().all(|value| value["change"]["status"] != "cancelled"),
            "Resize withdrew the displayed preview: {changes:?}"
        );
        let last = changes.last().ok_or("Refit preview was never observed")?;
        assert_eq!(last["change"]["status"], "presented");
        assert_eq!(last["change"]["token"], "preview:1");
        assert_eq!(last["change"]["displayed"]["type"], "preview");
        assert_eq!(last["change"]["displayed"]["token"], "preview:1");
        assert_eq!(last["change"]["displayed"]["viewport"]["mode"], "fit");
        let scale =
            |value: &Value| value["change"]["displayed"]["viewport"]["physicalScale"].as_f64();
        assert!(scale(last).is_some_and(|refit| scale(&shown).is_some_and(|old| refit < old)));
        assert_eq!(uploaded, Some(PhysicalRect { x: 10, y: 20, width: 60, height: 30 }));
        assert_eq!(
            fixture.accepted()?.viewport.surface(),
            sugarsubstitute_image_ops::Extent::new(60, 30)?
        );
        drain(&mut fixture)?;
        fixture.finish()?;
    }
    Ok(())
}

#[test]
fn failed_latest_preview_keeps_the_previous_actual_preview_display_and_navigation_lock()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    transfer(&mut fixture, 300, "preview:2", &[0xff, 0xd8, 0xff, 0xd9])?;
    let value = wait_change(&mut fixture, "failed")?;
    assert_eq!(value["change"]["token"], "preview:2");
    assert_eq!(value["change"]["displayed"]["type"], "preview");
    assert_eq!(value["change"]["displayed"]["token"], "preview:1");
    fixture.viewport(303, json!({"type":"zoom","factor":2}))?;
    fixture.take(303, "unavailable")?;
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn declared_jpeg_rejects_actual_png_and_never_publishes_it() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let camera = fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&[1, 2, 3])?;
        writer.finish()?;
    }
    begin(&mut fixture, 200, "preview:1", bytes.len())?;
    fixture.send(
        201,
        "surface.preview.chunk",
        json!({"token":"preview:1","leaseRevision":200,"offset":0,"data":encoded(&bytes)}),
    )?;
    fixture.take(201, "applied")?;
    fixture.send(202, "surface.preview.end", json!({"token":"preview:1","leaseRevision":200}))?;
    fixture.take(202, "error")?;
    let value = wait_change(&mut fixture, "failed")?;
    assert_eq!(value["change"]["displayed"]["type"], "final");
    assert_eq!(value["change"]["displayed"]["selectionRevision"], 3);
    assert_eq!(fixture.accepted()?, camera);
    assert!(prepared_frame(&fixture.canvas).is_none_or(|frame| !frame.content.pending()));
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn legal_png_begin_is_unavailable_before_allocation_and_preserves_the_shown_preview()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    let camera = fixture.accepted()?;
    let memory = fixture
        .canvas
        .session
        .as_ref()
        .and_then(Session::preview_resident)
        .ok_or("Preview memory owner is absent")?;
    let available = match memory.claim(u64::MAX) {
        Err(cause) => cause.available_bytes,
        Ok(_) => return Err("Preview test reservation unexpectedly admits u64::MAX".into()),
    };
    fixture.send(
        300,
        "surface.preview.begin",
        json!({"preview":{"generation":0,"promptId":"prompt:one","sourceId":"source:one","runOrder":1,"sequence":2,"token":"preview:2"},"byteLength":64*1024*1024,"format":"png"}),
    )?;
    fixture.take(300, "unavailable")?;
    assert!(memory.claim(available).is_ok());
    assert_eq!(fixture.accepted()?, camera);
    assert_eq!(prepared_frame(&fixture.canvas).map(|frame| frame.content.target()), Some(target));
    assert!(change(&mut fixture)?.is_none());
    begin(&mut fixture, 301, "preview:2", 3)?;
    drop(memory);
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn foreign_chunk_preserves_current_transfer_and_matching_gap_aborts_it_explicitly()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    begin(&mut fixture, 200, "preview:1", 3)?;
    fixture.send(
        201,
        "surface.preview.chunk",
        json!({"token":"preview:2","leaseRevision":200,"offset":0,"data":"AQID"}),
    )?;
    fixture.take(201, "stale")?;
    fixture.send(
        202,
        "surface.preview.chunk",
        json!({"token":"preview:1","leaseRevision":200,"offset":1,"data":"AQID"}),
    )?;
    fixture.take(202, "error")?;
    let value = wait_change(&mut fixture, "failed")?;
    assert_eq!(value["change"]["displayed"]["type"], "final");
    assert_eq!(value["change"]["displayed"]["selectionRevision"], 3);
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn original_begin_deadline_wakes_and_expires_without_chunk_renewal() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    begin(&mut fixture, 200, "preview:1", 3)?;
    assert!(fixture.canvas.next_deadline().is_some());
    fixture.advance(29_999);
    fixture.send(
        201,
        "surface.preview.chunk",
        json!({"token":"preview:1","leaseRevision":200,"offset":0,"data":"AQID"}),
    )?;
    fixture.take(201, "applied")?;
    fixture.advance(1);
    let value = wait_change(&mut fixture, "failed")?;
    assert_eq!(value["change"]["token"], "preview:1");
    assert_eq!(value["change"]["displayed"]["type"], "final");
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn withdrawing_a_preview_refit_by_resizing_restores_the_navigated_final_camera()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":1.25}))?;
    let navigated = fixture.complete(5)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    fixture.geometry(324, 60, 30)?;
    settle_geometry(&mut fixture, 324)?;
    fixture.geometry(325, 80, 40)?;
    settle_geometry(&mut fixture, 325)?;
    fixture.send(
        326,
        "surface.preview.withdraw",
        json!({"token":"preview:1","leaseRevision":200}),
    )?;
    let mut restored = None;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurfacePresentation(change) = command {
                let value: Value = serde_json::from_slice(change.payload())?;
                if value["change"]["displayed"]["type"] == "final" {
                    restored = Some(value);
                }
            }
        }
        if restored.is_some() {
            break;
        }
        match fixture.canvas.presentation() {
            MainCanvasPresentation::Frame(frame) if frame.content.pending() => {
                fixture.canvas.presented(frame.content.target())?;
            }
            MainCanvasPresentation::Hide => fixture.canvas.hidden(fixture.identity)?,
            _ => fixture.wait_preview_progress(observed)?,
        }
    }
    let restored = restored.ok_or("Withdrawal never reported the restored final")?;
    assert_eq!(restored["change"]["displayed"]["viewport"], navigated["viewport"]);
    fixture.viewport(327, json!({"type":"read"}))?;
    let read = fixture.complete(327)?;
    assert_eq!(read["viewport"], navigated["viewport"]);
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn a_navigated_final_accepts_a_locked_fit_preview() -> Result<(), Box<dyn Error>> {
    for commands in [
        vec![
            json!({"type":"zoom","factor":2}),
            json!({"type":"pan","units":"viewport","x":0.25,"y":0}),
        ],
        vec![json!({"type":"native"})],
    ] {
        let mut fixture = Fixture::new()?;
        fixture.commit(4)?;
        for (offset, command) in commands.into_iter().enumerate() {
            let id = 5 + offset as u64;
            fixture.viewport(id, command)?;
            fixture.complete(id)?;
        }
        watch(&mut fixture, 100, true)?;
        transfer(&mut fixture, 200, "preview:1", JPEG)?;
        let target = wait_frame(&mut fixture)?;
        fixture.canvas.presented(target)?;
        let value = wait_change(&mut fixture, "presented")?;
        assert_eq!(value["change"]["displayed"]["type"], "preview");
        assert_eq!(value["change"]["displayed"]["viewport"]["mode"], "fit");
        drain(&mut fixture)?;
        fixture.finish()?;
    }
    Ok(())
}

#[test]
fn withdrawing_an_incomplete_transfer_over_a_final_admits_the_next_preview()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    begin(&mut fixture, 200, "preview:1", 3)?;
    fixture.send(
        201,
        "surface.preview.withdraw",
        json!({"token":"preview:1","leaseRevision":200}),
    )?;
    fixture.take(201, "applied")?;
    let value = wait_change(&mut fixture, "cancelled")?;
    assert_eq!(value["change"]["token"], "preview:1");
    assert_eq!(value["change"]["displayed"]["type"], "final");
    begin(&mut fixture, 300, "preview:2", 3)?;
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn withdrawing_a_preparing_candidate_over_a_final_admits_the_next_preview()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let final_camera = fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    for _ in 0..500 {
        if fixture.canvas.surface.has_preview_candidate() {
            break;
        }
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if !fixture.canvas.surface.has_preview_candidate() {
            fixture.wait_preview_progress(observed)?;
        }
    }
    if !fixture.canvas.surface.has_preview_candidate() {
        return Err("Completed preview transfer never reached Session admission".into());
    }
    fixture.send(
        203,
        "surface.preview.withdraw",
        json!({"token":"preview:1","leaseRevision":200}),
    )?;
    fixture.take(203, "applied")?;
    let value = wait_change(&mut fixture, "cancelled")?;
    assert_eq!(value["change"]["token"], "preview:1");
    assert_eq!(value["change"]["displayed"]["type"], "final");
    assert!(!fixture.canvas.surface.has_preview_candidate());
    begin(&mut fixture, 300, "preview:2", 3)?;
    assert_eq!(fixture.accepted()?.viewport, final_camera.viewport);
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn withdrawing_staged_latest_reports_restored_final_only_after_actual_native_ack()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let final_camera = fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    begin(&mut fixture, 300, "preview:2", 3)?;
    fixture.send(
        301,
        "surface.preview.withdraw",
        json!({"token":"preview:2","leaseRevision":300}),
    )?;
    fixture.take(301, "applied")?;
    let value = wait_change(&mut fixture, "cancelled")?;
    assert_eq!(value["change"]["token"], "preview:2");
    assert_eq!(value["change"]["displayed"]["token"], "preview:1");
    let sequence = value["change"]["sequence"].as_u64().ok_or("Missing change sequence")?;
    let mut restored = None;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        if let Some(frame) =
            prepared_frame(&fixture.canvas).filter(|frame| frame.content.target() != target)
        {
            restored = Some(frame.content.target());
            break;
        }
        fixture.wait_preview_progress(observed)?;
    }
    let target = restored.ok_or("Preview withdrawal never offered the restored final frame")?;
    assert!(change(&mut fixture)?.is_none());
    fixture.canvas.presented(target)?;
    let value = wait_change(&mut fixture, "cancelled")?;
    assert_eq!(value["change"]["token"], "preview:2");
    assert_eq!(value["change"]["displayed"]["type"], "final");
    assert_eq!(value["change"]["displayed"]["selectionRevision"], 3);
    assert!(value["change"]["sequence"].as_u64().is_some_and(|new| new > sequence));
    assert_eq!(fixture.accepted()?.viewport, final_camera.viewport);
    drain(&mut fixture)?;
    fixture.finish()
}
