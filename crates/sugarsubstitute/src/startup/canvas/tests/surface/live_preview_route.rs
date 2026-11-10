// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Switch to the Output route while a live preview is being received over an Input document.
use super::preview::{JPEG, drain, transfer, watch};
use super::viewport::fixture::{Execution, Fixture, image};
use super::*;

const WORKFLOW: &str = "workflow-live-preview";

/// Everything the canvas has told the page since the test began.
#[derive(Default)]
struct Heard {
    workflow: Vec<Value>,
    surface: Vec<Value>,
    changes: Vec<Value>,
}

impl Heard {
    /// The terminal `workflow.reply` result of request `id`, once it was delivered.
    fn result(&self, id: u64) -> Option<&Value> {
        self.workflow.iter().find(|reply| reply["requestId"] == id).map(|reply| &reply["result"])
    }

    /// Whether the surface request `id` was answered.
    fn answered(&self, id: u64) -> bool {
        self.surface.iter().any(|reply| reply["requestId"] == id)
    }

    /// Whether a preview presentation change with `status` was published.
    fn changed(&self, status: &str) -> bool {
        self.changes.iter().any(|change| change["change"]["status"] == status)
    }
}

/// Submit one workflow command exactly as the page does.
fn workflow(fixture: &mut Fixture, id: u64, command: Value) -> Result<(), Box<dyn Error>> {
    let request = CanvasNativeRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":fixture.panel.as_str(),
        "requestId":id, "type":"workflow.request", "command":command
    }))?)?;
    fixture.canvas.native(
        fixture.identity,
        request,
        fixture.memory.claim(64 * 1024)?,
        Default::default(),
    )
}

/// Collect every command the canvas queued for the page.
fn listen(fixture: &mut Fixture, heard: &mut Heard) -> Result<(), Box<dyn Error>> {
    while let Some(command) = fixture.canvas.take_command() {
        match command {
            DesktopStartupCommand::SendCanvasNative(reply) => {
                heard.workflow.push(serde_json::from_slice(reply.payload())?);
            }
            DesktopStartupCommand::SendSurfacePresentation(change) => {
                heard.changes.push(serde_json::from_slice(change.payload())?);
            }
            DesktopStartupCommand::SendSurface(reply) => {
                heard.surface.push(serde_json::from_slice(reply.payload())?);
            }
            DesktopStartupCommand::SendCanvasNotification(_) => {}
            _ => return Err("Unexpected command while the page listened".into()),
        }
    }
    Ok(())
}

/// Deliver completions and the native adapter's frame and hide acknowledgements until `done`
/// holds.
fn drive(
    fixture: &mut Fixture,
    heard: &mut Heard,
    done: impl Fn(&Heard) -> bool,
) -> Result<(), Box<dyn Error>> {
    let mut hidden = false;
    for _ in 0..1_000 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        listen(fixture, heard)?;
        if done(heard) {
            return Ok(());
        }
        match fixture.canvas.presentation() {
            MainCanvasPresentation::Frame(frame) if frame.content.pending() => {
                let target = frame.content.target();
                hidden = false;
                fixture.canvas.presented(target)?;
                continue;
            }
            MainCanvasPresentation::Hide if !hidden => {
                hidden = true;
                fixture.canvas.hidden(fixture.identity)?;
                continue;
            }
            _ => {}
        }
        match fixture.execution()? {
            Execution::Running => fixture.wake.wait_after(observed),
            Execution::Idle | Execution::Delivering => fixture.advance(100),
        }
    }
    Err(format!(
        "The canvas never reached the expected state; workflow {:?}, surface {:?}, changes {:?}",
        heard.workflow, heard.surface, heard.changes
    )
    .into())
}

/// An empty, committed attachment with an Input document shown, as a workflow with only an
/// input image opens: nothing was ever selected for the Output route.
fn input_shown_without_output() -> Result<(Fixture, Heard), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    image(&fixture.output("base.png"), 8, 6, [90, 120, 200])?;
    fixture.send(3, "surface.commit", json!({}))?;
    fixture.canvas.hidden(fixture.identity)?;
    fixture.take(3, "applied")?;
    let mut heard = Heard::default();
    let surfaces = json!([{
        "key":"image", "label":"Load Image", "masks":[],
        "base":{"kind":"file", "file":{"filename":"base.png", "subfolder":"", "type":"input"}}
    }]);
    workflow(&mut fixture, 10, json!({"type":"sync", "workflow":WORKFLOW, "surfaces":surfaces}))?;
    drive(&mut fixture, &mut heard, |heard| heard.result(10).is_some())?;
    assert_eq!(heard.result(10).map(|result| &result["kind"]), Some(&json!("synced")));
    workflow(
        &mut fixture,
        11,
        json!({"type":"activate", "workflow":WORKFLOW, "route":"input", "surface":"image", "mask":null}),
    )?;
    drive(&mut fixture, &mut heard, |heard| heard.result(11).is_some())?;
    // The route reply precedes the swap; wait until the Input document is actually on screen.
    fixture.settle()?;
    let shown = fixture.canvas.session.as_ref().is_some_and(|session| session.input_shown());
    assert!(shown, "The Input document never reached the screen");
    Ok((fixture, heard))
}

/// Ask for the Output route.
fn show_output(fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
    workflow(
        fixture,
        id,
        json!({"type":"activate", "workflow":WORKFLOW, "route":"output", "surface":null, "mask":null}),
    )
}

#[test]
fn output_route_opened_while_a_preview_is_received_shows_that_preview() -> Result<(), Box<dyn Error>>
{
    let (mut fixture, mut heard) = input_shown_without_output()?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    show_output(&mut fixture, 12)?;
    drive(&mut fixture, &mut heard, |heard| {
        heard.result(12).is_some() && heard.changed("presented")
    })?;
    assert_eq!(heard.result(12), Some(&json!({"kind":"activated"})));
    drain(&mut fixture)?;
    fixture.finish()
}

/// The Output route of a workflow that never offered an output, with its empty document on
/// screen in place of the Input document and a live preview watched but not yet received.
fn empty_output_route() -> Result<(Fixture, Heard), Box<dyn Error>> {
    let (mut fixture, mut heard) = input_shown_without_output()?;
    watch(&mut fixture, 100, true)?;
    show_output(&mut fixture, 12)?;
    drive(&mut fixture, &mut heard, |heard| heard.result(12).is_some())?;
    assert_eq!(heard.result(12), Some(&json!({"kind":"activated"})));
    fixture.settle()?;
    assert!(
        fixture.canvas.session.as_ref().is_some_and(|session| !session.input_shown()),
        "The empty Output document never replaced the Input document"
    );
    Ok((fixture, heard))
}

#[test]
fn first_preview_on_the_empty_output_route_is_shown() -> Result<(), Box<dyn Error>> {
    let (mut fixture, mut heard) = empty_output_route()?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    drive(&mut fixture, &mut heard, |heard| heard.changed("presented"))?;
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn withdrawn_preview_leaves_the_empty_output_route_serving() -> Result<(), Box<dyn Error>> {
    let (mut fixture, mut heard) = empty_output_route()?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    drive(&mut fixture, &mut heard, |heard| heard.changed("presented"))?;
    fixture.send(
        203,
        "surface.preview.withdraw",
        json!({"token":"preview:1","leaseRevision":200}),
    )?;
    drive(&mut fixture, &mut heard, |heard| heard.answered(203) && heard.changed("cancelled"))?;
    drain(&mut fixture)?;
    fixture.finish()
}
