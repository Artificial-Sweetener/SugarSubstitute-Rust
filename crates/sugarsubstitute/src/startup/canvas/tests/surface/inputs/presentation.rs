// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Input route presents its document, whether it has an image base or a blank one, and
//! routes canvas input on an attachment that has never shown an output, as it does after an
//! output selection, and the Output route hides it again.

use super::super::viewport::fixture::{Execution, Fixture};
use super::*;
use crate::startup::canvas::RoutedInput;
use sugarsubstitute_desktop::canvas::{MainCanvasGesture, MainCanvasInput};

/// Opaque red, the colour of the fixture's `wide.png`.
const RED: u32 = 0xffff_0000;
/// The sRGB 24,24,24 backing a blank input document draws.
const BACKING: u32 = 0xff18_1818;

/// One file surface over the fixture's 320x160 red image.
fn image_surface() -> Value {
    json!({"type":"sync", "workflow":WORKFLOW, "surfaces":[
        {"key":"image", "label":"Load Image",
         "base":{"kind":"file", "file":input_file("wide.png")},
         "masks":[{"key":"painted", "label":"Painted", "color":"#00ff00", "opacity":0.5,
                   "initial":null, "channel":"red"}]}
    ]})
}

/// One mask-only synthetic surface over a blank 160x80 base, the viewport's aspect.
fn blank_surface() -> Value {
    json!({"type":"sync", "workflow":WORKFLOW, "surfaces":[
        {"key":"latent", "label":"Empty Latent",
         "base":{"kind":"blank", "width":160, "height":80},
         "masks":[{"key":"region", "label":"Region", "color":"#0000ff", "opacity":0.5,
                   "initial":null, "channel":"red"}]}
    ]})
}

/// A prepared visible attachment the page committed while no output was selected, and the
/// lease carrying native lane credit for its workflow requests.
fn committed_empty() -> Result<(Fixture, MemoryLease), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(3, "surface.commit", json!({}))?;
    assert!(matches!(fixture.canvas.presentation(), MainCanvasPresentation::Hide));
    let identity = fixture.identity;
    fixture.canvas.hidden(identity)?;
    fixture.take(3, "applied")?;
    Ok((fixture, MemoryLease::new(64 * 1024 * 1024, Box::new(Release))))
}

/// Route a wheel notch and present its frames, requiring the gesture to reach the document
/// and the region to stay published.
fn zoom(fixture: &mut Fixture, phase: &str) -> Result<(), Box<dyn Error>> {
    let wheel =
        MainCanvasInput { identity: fixture.identity, gesture: MainCanvasGesture::Wheel(1.0) };
    assert_eq!(fixture.canvas.input(wheel)?, RoutedInput::Applied, "{phase}");
    present(fixture, phase)?;
    assert!(
        fixture.canvas.input_region().is_some(),
        "{phase}: the zoomed document keeps its region"
    );
    Ok(())
}

/// Return to the Output route with no selected output, which hides the attachment again.
fn leave(fixture: &mut Fixture, memory: &MemoryLease, id: u64) -> Result<(), Box<dyn Error>> {
    workflow(fixture, memory, id, activate(None, None, "output"))?;
    hide(fixture)?;
    assert!(!input_shown(fixture));
    assert!(fixture.canvas.input_region().is_none());
    let refused =
        MainCanvasInput { identity: fixture.identity, gesture: MainCanvasGesture::Wheel(1.0) };
    assert_eq!(fixture.canvas.input(refused)?, RoutedInput::Refused);
    Ok(())
}

/// Send one `workflow.request` the way the page does and pump until its reply arrives,
/// returning its result. `memory` carries the native lane credit the reply returns.
fn workflow(
    fixture: &mut Fixture,
    memory: &MemoryLease,
    id: u64,
    command: Value,
) -> Result<Value, Box<dyn Error>> {
    ask(fixture, memory, id, "workflow.request", command)
}

/// Send one native canvas request of `kind` the way the page does and pump until its reply
/// arrives, returning its result.
fn ask(
    fixture: &mut Fixture,
    memory: &MemoryLease,
    id: u64,
    kind: &str,
    command: Value,
) -> Result<Value, Box<dyn Error>> {
    let message = json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":fixture.panel.as_str(),
        "requestId":id, "type":kind, "command":command
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
            return Err(format!("Canvas request {id} was not answered").into());
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

/// Send one shared-viewport camera command the way the page's keys do and pump, presenting
/// every frame, until its reply arrives; returns the reply's status. A request the host holds
/// without ever answering fails once no canvas work or deadline remains.
fn camera(fixture: &mut Fixture, id: u64, command: Value) -> Result<String, Box<dyn Error>> {
    fixture.viewport(id, command)?;
    for _ in 0..500 {
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurface(reply) = command {
                let value: Value = serde_json::from_slice(reply.payload())?;
                if value["requestId"] == id {
                    return Ok(value["status"].as_str().unwrap_or_default().to_owned());
                }
            }
        }
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some(target) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target())
        {
            fixture.canvas.presented(target)?;
            continue;
        }
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => {
                if let Some(DesktopStartupCommand::SendSurface(reply)) =
                    fixture.canvas.take_command()
                {
                    let value: Value = serde_json::from_slice(reply.payload())?;
                    return Ok(value["status"].as_str().unwrap_or_default().to_owned());
                }
                return Err(format!("Viewport request {id} was held without an answer").into());
            }
            Execution::Idle | Execution::Delivering => fixture.advance(100),
            Execution::Running => fixture.wake.wait_after(observed)?,
        }
    }
    Err(format!("Viewport request {id} never settled: {}", state(fixture)?).into())
}

fn input_shown(fixture: &Fixture) -> bool {
    fixture.canvas.session.as_ref().is_some_and(Session::input_shown)
}

#[test]
fn the_input_route_presents_and_routes_input_before_any_output_is_selected()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));

    let pixels = present(&mut fixture, "image")?;
    assert!(input_shown(&fixture));
    let red = pixels.iter().filter(|&&pixel| pixel == RED).count();
    assert!(red * 2 > pixels.len(), "the input document's image is shown: {red}/{}", pixels.len());
    let region = fixture.canvas.input_region().ok_or("The input document has no input region")?;
    assert_eq!(region.placement.clip, PhysicalRect { x: 10, y: 20, width: 80, height: 40 });
    assert!(!region.drag_out, "an input document is not an output to drag out");
    zoom(&mut fixture, "image wheel")?;
    leave(&mut fixture, &memory, 102)?;
    fixture.finish()
}

/// The page's zoom and fit keys address the attachment's shared viewport; while the Input route
/// shows its document they move that document's camera and are answered, never left waiting.
#[test]
fn shared_viewport_commands_on_the_input_route_are_answered() -> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    assert!(input_shown(&fixture));
    assert_eq!(camera(&mut fixture, 102, json!({"type":"zoom","factor":1.25}))?, "applied");
    assert_eq!(camera(&mut fixture, 103, json!({"type":"fit"}))?, "applied");
    leave(&mut fixture, &memory, 104)?;
    fixture.finish()
}

/// A mask-only synthetic surface has a blank base and no source image; the Input route still
/// presents its backing and routes input to it.
#[test]
fn the_input_route_presents_and_routes_input_on_a_mask_only_surface() -> Result<(), Box<dyn Error>>
{
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, blank_surface())?["kind"], "synced");
    let activated =
        workflow(&mut fixture, &memory, 101, activate(Some("latent"), Some("region"), "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));

    let pixels = present(&mut fixture, "blank")?;
    assert!(input_shown(&fixture));
    let backing = pixels.iter().filter(|&&pixel| pixel == BACKING).count();
    assert!(
        backing * 2 > pixels.len(),
        "the blank document's backing is shown: {backing}/{}, first {:x?}",
        pixels.len(),
        pixels.first()
    );
    let region = fixture.canvas.input_region().ok_or("The blank document has no input region")?;
    assert!(!region.drag_out, "an input document is not an output to drag out");
    zoom(&mut fixture, "blank wheel")?;
    leave(&mut fixture, &memory, 102)?;
    fixture.finish()
}

/// The same keys after an output was selected and shown, the usual order in the app: the
/// output's camera evidence must not leave the Input document's camera request waiting.
#[test]
fn shared_viewport_commands_on_the_input_route_after_an_output_are_answered()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let memory = MemoryLease::new(64 * 1024 * 1024, Box::new(Release));
    assert_eq!(camera(&mut fixture, 5, json!({"type":"zoom","factor":1.25}))?, "applied");
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    assert!(input_shown(&fixture));
    assert_eq!(camera(&mut fixture, 102, json!({"type":"zoom","factor":1.25}))?, "applied");
    assert_eq!(camera(&mut fixture, 103, json!({"type":"fit"}))?, "applied");
    fixture.finish()
}

/// The same keys while a selection's ants march over the Input document, as after the user
/// drew one: every camera command is answered.
#[test]
fn shared_viewport_commands_beside_a_selection_on_the_input_route_are_answered()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_canvas::{PointerSample, ToolModifiers};
    use sugarsubstitute_desktop::canvas::MainCanvasButton;
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    let tool = json!({"type":"tool", "tool":"select-rectangle"});
    ask(&mut fixture, &memory, 102, "edit.request", tool)?;
    let identity = fixture.identity;
    let button = |pressed| MainCanvasGesture::Button {
        button: MainCanvasButton::Primary,
        pressed,
        modifiers: ToolModifiers::NONE,
    };
    let pointer =
        |x, y| MainCanvasGesture::Pointer(PointerSample::mouse([x, y], ToolModifiers::NONE));
    for gesture in
        [pointer(30.0, 28.0), button(true), pointer(50.0, 40.0), pointer(70.0, 50.0), button(false)]
    {
        fixture.canvas.input(MainCanvasInput { identity, gesture })?;
    }
    // The ants keep a deadline, so present a bounded number of rounds instead of settling.
    for _ in 0..40 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some(target) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target())
        {
            fixture.canvas.presented(target)?;
        } else if fixture.execution()? == Execution::Running {
            fixture.wake.wait_after(observed)?;
        } else {
            fixture.advance(50);
        }
    }
    let state = ask(&mut fixture, &memory, 103, "edit.request", json!({"type":"state"}))?;
    assert!(!state["state"]["selection"].is_null(), "a selection was drawn: {state}");
    assert_eq!(camera(&mut fixture, 104, json!({"type":"zoom","factor":1.25}))?, "applied");
    assert_eq!(camera(&mut fixture, 105, json!({"type":"fit"}))?, "applied");
    fixture.finish()
}

/// The same keys after the user chose another file for the shown surface, which rebases its
/// document under the Input route: every camera command is answered.
#[test]
fn shared_viewport_commands_after_a_rebased_input_document_are_answered()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    let rebased = json!({"type":"sync", "workflow":WORKFLOW, "surfaces":[
        {"key":"image", "label":"Load Image",
         "base":{"kind":"file", "file":input_file("green.png")},
         "masks":[{"key":"painted", "label":"Painted", "color":"#00ff00", "opacity":0.5,
                   "initial":null, "channel":"red"}]}
    ]});
    assert_eq!(workflow(&mut fixture, &memory, 102, rebased)?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 103, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "rebased")?;
    assert_eq!(camera(&mut fixture, 104, json!({"type":"zoom","factor":1.25}))?, "applied");
    assert_eq!(camera(&mut fixture, 105, json!({"type":"fit"}))?, "applied");
    fixture.finish()
}

/// The page reads the camera after each geometry change. When the Input route's display still
/// describes the document it replaced, as a document swap leaves it until the incoming
/// document's frame lands, the read draws that frame and settles instead of waiting forever.
#[test]
fn a_camera_read_on_the_input_route_settles_after_its_display_described_another_document()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    fixture.canvas.session.as_mut().ok_or("The canvas session is absent")?.adopt_foreign_display();
    assert_eq!(camera(&mut fixture, 102, json!({"type":"read"}))?, "applied");
    assert_eq!(camera(&mut fixture, 103, json!({"type":"zoom","factor":1.25}))?, "applied");
    fixture.finish()
}

/// The page reports focus through geometry when the viewport takes the keys, just before the
/// first key's camera command: the command is still answered.
#[test]
fn a_camera_command_right_after_focus_geometry_on_the_input_route_is_answered()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    fixture.send(
        102,
        "surface.geometry",
        json!({"geometry":{
            "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
            "viewport":{"width":80,"height":40}, "clip":{"x":10,"y":20,"width":80,"height":40},
            "physicalPixelsPerCssPixel":1.0, "visible":true, "focused":true
        }}),
    )?;
    fixture.viewport(103, json!({"type":"zoom","factor":1.25}))?;
    let mut statuses = Vec::new();
    for _ in 0..500 {
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurface(reply) = command {
                let value: Value = serde_json::from_slice(reply.payload())?;
                statuses.push((value["requestId"].clone(), value["status"].clone()));
            }
        }
        if statuses.iter().any(|(id, _)| id == 103) {
            break;
        }
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some(target) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target())
        {
            fixture.canvas.presented(target)?;
            continue;
        }
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => break,
            Execution::Idle | Execution::Delivering => fixture.advance(100),
            Execution::Running => fixture.wake.wait_after(observed)?,
        }
    }
    assert_eq!(statuses, vec![(json!(102), json!("applied")), (json!(103), json!("applied"))]);
    fixture.finish()
}

/// A camera key pressed while Space holds temporary navigation is still answered.
#[test]
fn a_camera_command_during_held_navigation_on_the_input_route_is_answered()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = committed_empty()?;
    assert_eq!(workflow(&mut fixture, &memory, 100, image_surface())?["kind"], "synced");
    let activated = workflow(&mut fixture, &memory, 101, activate(Some("image"), None, "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    present(&mut fixture, "image")?;
    ask(&mut fixture, &memory, 102, "edit.request", json!({"type":"navigation","held":true}))?;
    ask(&mut fixture, &memory, 103, "edit.request", json!({"type":"navigation","held":false}))?;
    assert_eq!(camera(&mut fixture, 104, json!({"type":"zoom","factor":1.25}))?, "applied");
    ask(&mut fixture, &memory, 105, "edit.request", json!({"type":"navigation","held":true}))?;
    assert_eq!(camera(&mut fixture, 106, json!({"type":"zoom","factor":1.25}))?, "applied");
    ask(&mut fixture, &memory, 107, "edit.request", json!({"type":"navigation","held":false}))?;
    fixture.finish()
}
