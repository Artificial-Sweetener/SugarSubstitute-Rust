// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify empty restoration and final-selection acknowledgement against actual native effects.
use super::*;

#[test]
fn withdrawing_the_first_preview_reports_empty_only_after_native_hide() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::empty()?;
    fixture.send(3, "surface.commit", json!({}))?;
    fixture.canvas.hidden(fixture.identity)?;
    fixture.take(3, "applied")?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    fixture.send(
        203,
        "surface.preview.withdraw",
        json!({"token":"preview:1","leaseRevision":200}),
    )?;
    fixture.take(203, "applied")?;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if matches!(fixture.canvas.presentation(), MainCanvasPresentation::Hide) {
            break;
        }
        fixture.wait_preview_progress(observed)?;
    }
    if !matches!(fixture.canvas.presentation(), MainCanvasPresentation::Hide) {
        return Err("First preview withdrawal never reached native hide custody".into());
    }
    assert!(change(&mut fixture)?.is_none());
    fixture.canvas.hidden(fixture.identity)?;
    let value = wait_change(&mut fixture, "cancelled")?;
    assert_eq!(value["change"]["displayed"]["type"], "empty");
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn restored_old_final_cannot_acknowledge_a_new_final_selection() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let preview = wait_frame(&mut fixture)?;
    fixture.canvas.presented(preview)?;
    wait_change(&mut fixture, "presented")?;
    fixture.select(300, "tall.png")?;
    let mut acknowledged = None;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            match command {
                DesktopStartupCommand::SendSurface(reply) => {
                    let value: Value = serde_json::from_slice(reply.payload())?;
                    assert_eq!(value["requestId"], 300);
                    assert_eq!(value["status"], "applied");
                    assert!(
                        acknowledged.is_some(),
                        "Selection replied before native acknowledgement"
                    );
                    let accepted = fixture.accepted()?;
                    assert_eq!(accepted.extent.width(), 160);
                    assert_eq!(accepted.extent.height(), 320);
                    drop(reply);
                    drain(&mut fixture)?;
                    return fixture.finish();
                }
                DesktopStartupCommand::SendSurfacePresentation(_) => {}
                _ => return Err("Unexpected final handoff command".into()),
            }
        }
        if let Some(frame) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.frame.target != preview)
            .filter(|frame| acknowledged != Some(frame.frame.target))
        {
            let target = frame.frame.target;
            fixture.canvas.presented(target)?;
            acknowledged = Some(target);
        } else {
            fixture.wait_preview_progress(observed)?;
        }
    }
    Err("New final selection did not acknowledge its actual native source".into())
}
