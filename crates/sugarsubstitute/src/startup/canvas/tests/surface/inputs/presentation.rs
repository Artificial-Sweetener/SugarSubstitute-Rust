// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Input route presents its document and routes canvas input on an attachment that has
//! never shown an output, as it does after an output selection, and the Output route hides
//! it again.

use super::super::viewport::fixture::{Execution, Fixture};
use super::*;
use crate::startup::canvas::RoutedInput;
use sugarsubstitute_desktop::canvas::{MainCanvasGesture, MainCanvasInput};

/// Opaque red, the colour of the fixture's `wide.png`.
const RED: u32 = 0xffff_0000;

/// One file surface over the fixture's 320x160 red image.
fn surfaces() -> Value {
    json!({"type":"sync", "workflow":WORKFLOW, "surfaces":[
        {"key":"image", "label":"Load Image",
         "base":{"kind":"file", "file":input_file("wide.png")},
         "masks":[{"key":"painted", "label":"Painted", "color":"#00ff00", "opacity":0.5,
                   "initial":null, "channel":"red"}]}
    ]})
}

/// Send one `workflow.request` the way the page does and pump until its reply arrives,
/// returning its result. `memory` carries the native lane credit the reply returns.
fn workflow(
    fixture: &mut Fixture,
    memory: &MemoryLease,
    id: u64,
    command: Value,
) -> Result<Value, Box<dyn Error>> {
    let message = json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":fixture.panel.as_str(),
        "requestId":id, "type":"workflow.request", "command":command
    });
    let request = CanvasNativeRequest::decode(&serde_json::to_vec(&message)?)?;
    fixture.canvas.native(
        fixture.identity,
        request,
        memory.claim(64 * 1024)?,
        Default::default(),
    )?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            match command {
                DesktopStartupCommand::SendCanvasNative(reply) => {
                    let mut value: Value = serde_json::from_slice(reply.payload())?;
                    if value["requestId"] != id {
                        return Err(
                            format!("Unexpected reply while waiting for {id}: {value}").into()
                        );
                    }
                    return Ok(value["result"].take());
                }
                DesktopStartupCommand::SendCanvasNotification(_) => {}
                _ => return Err(format!("Unexpected command while waiting for {id}").into()),
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("Workflow request {id} was not answered").into());
        }
        if fixture.execution()? == Execution::Running {
            fixture.wake.wait_after(observed)?;
        }
    }
}

/// Present every frame the surface offers until no work remains, returning the pixels of the
/// last presented frame. Fails when the surface never offers one.
fn present(fixture: &mut Fixture, phase: &str) -> Result<Vec<u32>, Box<dyn Error>> {
    let mut shown = None;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some((target, pixels)) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| (frame.content.target(), frame.content.cpu_pixels().to_vec()))
        {
            fixture.canvas.presented(target)?;
            shown = Some(pixels);
            continue;
        }
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => {
                return shown
                    .ok_or_else(|| format!("{phase}: the surface never presented a frame").into());
            }
            Execution::Idle | Execution::Delivering => fixture.advance(100),
            Execution::Running => fixture.wake.wait_after(observed)?,
        }
    }
    Err(format!("{phase}: canvas work never settled: {}", state(fixture)?).into())
}

/// Describe what keeps the canvas from settling, for a stall diagnostic.
fn state(fixture: &Fixture) -> Result<String, Box<dyn Error>> {
    let presentation = match fixture.canvas.presentation() {
        MainCanvasPresentation::Hide => "hide".to_owned(),
        MainCanvasPresentation::Retain(_) => "retain".to_owned(),
        MainCanvasPresentation::Frame(frame) => {
            format!("frame(pending={})", frame.content.pending())
        }
    };
    Ok(format!(
        "presentation {presentation}, execution {:?}, deadline {:?}, input shown {}, jobs [{}]",
        fixture.execution()?,
        fixture.canvas.next_deadline(),
        input_shown(fixture),
        fixture.scope_jobs(ScopeId::new(1)?)?
    ))
}

/// Pump until the surface withdraws its native pixels, acknowledging the hide as the desktop
/// does once composition removed them.
fn hide(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        match fixture.canvas.presentation() {
            MainCanvasPresentation::Hide => {
                let identity = fixture.identity;
                return fixture.canvas.hidden(identity);
            }
            MainCanvasPresentation::Frame(frame) if frame.content.pending() => {
                let target = frame.content.target();
                fixture.canvas.presented(target)?;
                continue;
            }
            MainCanvasPresentation::Frame(_) | MainCanvasPresentation::Retain(_) => {}
        }
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => {
                return Err("The Output route kept the input document's pixels".into());
            }
            Execution::Idle | Execution::Delivering => fixture.advance(100),
            Execution::Running => fixture.wake.wait_after(observed)?,
        }
    }
    Err(format!("Canvas work never settled: {}", state(fixture)?).into())
}

fn input_shown(fixture: &Fixture) -> bool {
    fixture.canvas.session.as_ref().is_some_and(Session::input_shown)
}

#[test]
fn the_input_route_presents_and_routes_input_before_any_output_is_selected()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    let memory = MemoryLease::new(64 * 1024 * 1024, Box::new(Release));
    // The page commits its attachment while no output has been selected.
    fixture.send(3, "surface.commit", json!({}))?;
    assert!(matches!(fixture.canvas.presentation(), MainCanvasPresentation::Hide));
    let identity = fixture.identity;
    fixture.canvas.hidden(identity)?;
    fixture.take(3, "applied")?;
    assert_eq!(workflow(&mut fixture, &memory, 100, surfaces())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));

    let pixels = present(&mut fixture, "image")?;
    assert!(input_shown(&fixture));
    let red = pixels.iter().filter(|&&pixel| pixel == RED).count();
    assert!(red * 2 > pixels.len(), "the input document's image is shown: {red}/{}", pixels.len());
    let region = fixture.canvas.input_region().ok_or("The input document has no input region")?;
    assert_eq!(region.placement.clip, PhysicalRect { x: 10, y: 20, width: 80, height: 40 });
    assert!(!region.drag_out, "an input document is not an output to drag out");
    let wheel = MainCanvasInput { identity, gesture: MainCanvasGesture::Wheel(1.0) };
    assert_eq!(fixture.canvas.input(wheel)?, RoutedInput::Applied);
    present(&mut fixture, "wheel")?;
    assert!(fixture.canvas.input_region().is_some(), "the zoomed document keeps its region");

    // The Output route has no output to show, so the attachment hides again.
    workflow(&mut fixture, &memory, 102, activate(None, None, "output"))?;
    hide(&mut fixture)?;
    assert!(!input_shown(&fixture));
    assert!(fixture.canvas.input_region().is_none());
    let refused = MainCanvasInput { identity, gesture: MainCanvasGesture::Wheel(1.0) };
    assert_eq!(fixture.canvas.input(refused)?, RoutedInput::Refused);
    fixture.finish()
}
