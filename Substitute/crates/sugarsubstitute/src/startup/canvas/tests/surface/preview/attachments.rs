// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove one workflow tab's preview custody never refuses the next workflow tab's previews.
use super::super::host::{geometry, select};
use super::*;

/// Address `extra` to the second workflow tab's attachment.
fn second(mut extra: Value) -> Value {
    extra["address"] = json!({
        "sessionId":"canvas:second", "viewId":"view:two", "windowId":"window:one",
        "attachmentId":"attachment:2", "generation":2
    });
    extra
}

/// Act as the native adapter until the canvas answers `id`: upload each pending frame and
/// acknowledge each hide once, passing over presentation observations.
fn answer(fixture: &mut Fixture, id: u64) -> Result<String, Box<dyn Error>> {
    let mut hidden = false;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurface(reply) = command {
                let value: Value = serde_json::from_slice(reply.payload())?;
                if value["requestId"] == id {
                    return Ok(value["status"].as_str().ok_or("Reply has no status")?.into());
                }
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
            NativeStep::Upload(target, _) => {
                hidden = false;
                fixture.canvas.presented(target)?;
            }
            NativeStep::Hide => {
                hidden = true;
                fixture.canvas.hidden(fixture.identity)?;
            }
            NativeStep::Idle => fixture.wait_preview_progress(observed)?,
        }
    }
    Err(format!("Request {id} was never answered").into())
}

/// Show a preview on the first workflow tab, switch to a second tab that shows a final, and
/// return the host's answer to that tab's first preview. With `in_flight`, a further preview of
/// the first tab is still on its way to the screen when the person switches.
fn first_preview_after_switch(in_flight: bool) -> Result<String, Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    drain(&mut fixture)?;
    if in_flight {
        transfer(&mut fixture, 210, "preview:2", JPEG)?;
    }
    fixture.send(220, "surface.release", json!({}))?;
    assert_eq!(answer(&mut fixture, 220)?, "applied");

    fixture.send(221, "surface.prepare", second(json!({})))?;
    assert_eq!(answer(&mut fixture, 221)?, "ready");
    fixture.send(222, "surface.geometry", second(geometry(80, 40, true)))?;
    assert_eq!(answer(&mut fixture, 222)?, "applied");
    fixture.send(223, "surface.select", second(select("tall.png")))?;
    assert_eq!(answer(&mut fixture, 223)?, "applied");
    fixture.send(224, "surface.commit", second(json!({})))?;
    assert_eq!(answer(&mut fixture, 224)?, "applied");

    fixture.send(
        230,
        "surface.preview.begin",
        second(json!({"preview":{"generation":1,"promptId":"prompt:two","sourceId":"source:one",
            "runOrder":1,"sequence":3,"token":"preview:3"},"byteLength":JPEG.len(),"format":"jpeg"})),
    )?;
    let status = answer(&mut fixture, 230)?;
    drain(&mut fixture)?;
    fixture.finish()?;
    Ok(status)
}

#[test]
fn the_next_workflow_tab_previews_after_a_shown_preview() -> Result<(), Box<dyn Error>> {
    assert_eq!(first_preview_after_switch(false)?, "applied");
    Ok(())
}

#[test]
fn the_next_workflow_tab_previews_after_a_switch_during_a_preview_transfer()
-> Result<(), Box<dyn Error>> {
    assert_eq!(first_preview_after_switch(true)?, "applied");
    Ok(())
}
