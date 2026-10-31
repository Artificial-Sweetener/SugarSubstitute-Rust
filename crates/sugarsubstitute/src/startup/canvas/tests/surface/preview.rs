// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive preview transport through real image work and exact native frame/hide acknowledgements.
use super::viewport::fixture::Fixture;
use super::*;
use sugarsubstitute_canvas::FrameStamp;
mod closing;
mod restoration;
mod transactions;

pub(super) const JPEG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sugarsubstitute-image-io/src/tests/jpeg/fixtures/420.jpg"
));

pub(super) fn watch(fixture: &mut Fixture, id: u64, enabled: bool) -> Result<(), Box<dyn Error>> {
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(
        &json!({"protocol":"sugarcubes.canvas","version":3,"session":fixture.panel.as_str(),"requestId":id,"type":"surface.presentation.request","schema":1,"enabled":enabled}),
    )?)?;
    fixture.canvas.surface(fixture.identity, message, None, fixture.memory.claim(64 * 1024)?)?;
    let value = fixture.take(id, "available")?;
    assert_eq!(value["enabled"], enabled);
    Ok(())
}
fn encoded(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 { ALPHABET[(value & 63) as usize] as char } else { '=' });
    }
    output
}
fn begin(fixture: &mut Fixture, id: u64, token: &str, length: usize) -> Result<(), Box<dyn Error>> {
    let sequence =
        token.strip_prefix("preview:").ok_or("Invalid test preview token")?.parse::<u64>()?;
    fixture.send(id,"surface.preview.begin",json!({"preview":{"generation":0,"promptId":"prompt:one","sourceId":"source:one","runOrder":1,"sequence":sequence,"token":token},"byteLength":length,"format":"jpeg"}))?;
    fixture.take(id, "applied")?;
    Ok(())
}
pub(super) fn transfer(
    fixture: &mut Fixture,
    id: u64,
    token: &str,
    bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    begin(fixture, id, token, bytes.len())?;
    fixture.send(
        id + 1,
        "surface.preview.chunk",
        json!({"token":token,"leaseRevision":id,"offset":0,"data":encoded(bytes)}),
    )?;
    fixture.take(id + 1, "applied")?;
    fixture.send(id + 2, "surface.preview.end", json!({"token":token,"leaseRevision":id}))?;
    fixture.take(id + 2, "applied")?;
    Ok(())
}
fn change(fixture: &mut Fixture) -> Result<Option<Value>, Box<dyn Error>> {
    let Some(command) = fixture.canvas.take_command() else { return Ok(None) };
    match command {
        DesktopStartupCommand::SendSurfacePresentation(change) => {
            Ok(Some(serde_json::from_slice(change.payload())?))
        }
        DesktopStartupCommand::SendSurface(reply) => {
            Err(format!("Unexpected ordinary reply: {}", std::str::from_utf8(reply.payload())?)
                .into())
        }
        _ => Err("Unexpected preview command".into()),
    }
}
pub(super) fn wait_frame(fixture: &mut Fixture) -> Result<FrameStamp, Box<dyn Error>> {
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        if let Some(frame) = prepared_frame(&fixture.canvas).filter(|frame| frame.content.pending())
        {
            let target = frame.content.target();
            if fixture
                .canvas
                .session
                .as_ref()
                .is_some_and(|session| session.preview_frame_acknowledgeable(target))
            {
                return Ok(target);
            }
            fixture.canvas.presented(target)?;
        }
        fixture.wait_preview_progress(observed)?;
    }
    Err("Preview never reached native presentation custody".into())
}
pub(super) fn wait_change(fixture: &mut Fixture, status: &str) -> Result<Value, Box<dyn Error>> {
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some(value) = change(fixture)?
            && value["change"]["status"] == status
        {
            return Ok(value);
        }
        fixture.wait_preview_progress(observed)?;
    }
    Err("Preview did not publish its expected terminal observation".into())
}
pub(super) fn drain(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    while change(fixture)?.is_some() {}
    Ok(())
}

#[test]
fn first_preview_needs_no_final_selection_and_bootstraps_real_graphics()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(3, "surface.commit", json!({}))?;
    assert!(matches!(fixture.canvas.presentation(), MainCanvasPresentation::Hide));
    assert!(fixture.canvas.take_command().is_none());
    fixture.canvas.hidden(fixture.identity)?;
    fixture.take(3, "applied")?;
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
    assert!(fixture.canvas.next_deadline().is_some_and(|deadline| deadline <= Instant::now()));
    assert!(change(&mut fixture)?.is_none());
    let target = wait_frame(&mut fixture)?;
    assert!(
        fixture.canvas.session.as_ref().is_some_and(|session| session.host_graphics().is_some())
    );
    fixture.canvas.presented(target)?;
    let value = wait_change(&mut fixture, "presented")?;
    assert_eq!(value["change"]["displayed"]["type"], "preview");
    assert_eq!(value["change"]["token"], "preview:1");
    fixture.viewport(203, json!({"type":"read"}))?;
    let value = fixture.take(203, "applied")?;
    assert_eq!(value["viewport"]["mode"], "fit");
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn actual_preview_upload_acknowledgement_can_cross_the_original_deadline()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.advance(30_000);
    fixture.canvas.presented(target)?;
    let value = wait_change(&mut fixture, "presented")?;
    assert_eq!(value["change"]["displayed"]["type"], "preview");
    drain(&mut fixture)?;
    fixture.finish()
}
