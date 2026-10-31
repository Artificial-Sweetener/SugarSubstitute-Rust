// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the shared surface transaction with real canvas worker output.

use super::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::{
    canvas_context::CanvasContextRequest,
    canvas_surface::CanvasSurfaceMessage,
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::{LaunchId, MainIdentity},
};
use sugarsubstitute_desktop::{
    DesktopStartupCommand,
    canvas::{
        MainCanvasFrame, MainCanvasPresentation,
        placement::{PhysicalRect, WebViewAllocation},
    },
};

mod backend;
mod cancellation;
mod comparison;
mod input;
mod inspection;
mod jpeg;
mod overview;
mod preview;
mod reattachment;
mod replacement;
mod viewport;

struct ClockValue(AtomicU64);
impl Clock for ClockValue {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

fn request(
    session: &PanelSession,
    id: u64,
    operation: &str,
    extra: Value,
) -> Result<CanvasSurfaceMessage, Box<dyn Error>> {
    let mut value = json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":id, "type":operation, "revision":id,
        "address":{"sessionId":"canvas:workflow", "viewId":"view:one",
            "windowId":"window:one", "attachmentId":"attachment:one", "generation":1}
    });
    let fields = value.as_object_mut().ok_or("Request is not an object")?;
    for (key, value) in extra.as_object().ok_or("Extra fields are not an object")? {
        fields.insert(key.clone(), value.clone());
    }
    Ok(CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?)
}

fn reply(canvas: &mut EmbeddedCanvas, id: u64, status: &str) -> Result<(), Box<dyn Error>> {
    let command = canvas.take_command().ok_or("Surface reply is missing")?;
    check_reply(command, id, status)
}

/// Observe cancellation-lane consumption separately from the operation's terminal reply.
fn cancel_consumed(
    canvas: &mut EmbeddedCanvas,
    identity: MainIdentity,
) -> Result<(), Box<dyn Error>> {
    let Some(DesktopStartupCommand::CompletePanelIngress(receipt)) = canvas.take_command() else {
        return Err("Cancellation consumption receipt is missing".into());
    };
    assert_eq!(receipt.identity(), identity);
    Ok(())
}

fn check_reply(
    command: DesktopStartupCommand,
    id: u64,
    status: &str,
) -> Result<(), Box<dyn Error>> {
    let DesktopStartupCommand::SendSurface(reply) = command else {
        return Err("Surface reply is missing".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], id);
    assert_eq!(value["status"], status);
    Ok(())
}

/// Inspect upload readiness independently of retained native pixels.
fn prepared_frame(canvas: &EmbeddedCanvas) -> Option<MainCanvasFrame<'_>> {
    match canvas.presentation() {
        MainCanvasPresentation::Frame(frame) => Some(frame),
        MainCanvasPresentation::Hide | MainCanvasPresentation::Retain(_) => None,
    }
}

fn pump(canvas: &mut EmbeddedCanvas, client: &ExecutionClient) -> Result<(), Box<dyn Error>> {
    client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    canvas.poll()
}

/// Observe actual preparation completion rather than assuming immediate empty-scene admission.
fn wait_reply(
    canvas: &mut EmbeddedCanvas,
    client: &ExecutionClient,
    wake: &Wake,
    id: u64,
    status: &str,
) -> Result<(), Box<dyn Error>> {
    loop {
        let revision = wake.revision();
        pump(canvas, client)?;
        if let Some(command) = canvas.take_command() {
            return check_reply(command, id, status);
        }
        wake.wait_after(revision)?;
    }
}

fn image(path: &std::path::Path, color: [u8; 3]) -> Result<(), Box<dyn Error>> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&color)?;
    writer.finish()?;
    Ok(())
}

#[test]
fn shared_surface_waits_for_current_pixels_and_native_acceptance() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("canvas-qualification-output");
    std::fs::create_dir(&output)?;
    image(&output.join("red.png"), [255, 0, 0])?;
    image(&output.join("blue.png"), [0, 0, 255])?;
    let clock = Arc::new(ClockValue(AtomicU64::new(0)));
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let mut canvas = EmbeddedCanvas::new(
        directory.path().to_path_buf(),
        DiagnosticHealth::fixture(),
        clock.clone(),
        JobSequence::new(),
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
    );
    canvas.start(client.clone(), "en")?;
    while !canvas.reported_ready {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if !canvas.reported_ready {
            wake.wait_after(revision)?;
        }
    }
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(2 * 1024 * 1024, Box::new(Release));
    let context = CanvasContextRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":100, "type":"context.request"
    }))?)?;
    canvas.context(identity, context, memory.claim(64 * 1024)?)?;
    let Some(DesktopStartupCommand::SendCanvasContext(context)) = canvas.take_command() else {
        return Err("Canvas context is missing".into());
    };
    let value: Value = serde_json::from_slice(context.payload())?;
    assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], true);
    canvas.surface(
        identity,
        request(&panel, 1, "surface.prepare", json!({}))?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    wait_reply(&mut canvas, &client, &wake, 1, "ready")?;
    let geometry = json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":8,"height":8}, "clip":{"x":10,"y":20,"width":8,"height":8},
        "physicalPixelsPerCssPixel":1, "visible":true, "focused":false
    }});
    let allocation = WebViewAllocation {
        bounds: PhysicalRect { x: 0, y: 0, width: 100, height: 100 },
        physical_pixels_per_css_pixel: 1.0,
    };
    canvas.surface(
        identity,
        request(&panel, 2, "surface.geometry", geometry)?,
        Some(allocation),
        memory.claim(64 * 1024)?,
    )?;
    reply(&mut canvas, 2, "applied")?;
    canvas.surface(identity, request(&panel, 3, "surface.select", json!({
        "itemId":"result:red", "file":{"filename":"red.png", "subfolder":"", "type":"output"}
    }))?, None, memory.claim(64 * 1024)?)?;
    let selected = loop {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if let Some(command) = canvas.take_command() {
            break command;
        }
        wake.wait_after(revision)?;
    };
    check_reply(selected, 3, "applied")?;
    canvas.surface(
        identity,
        request(&panel, 4, "surface.commit", json!({}))?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    assert!(canvas.take_command().is_none());
    let first = prepared_frame(&canvas).ok_or("Prepared red frame is missing")?;
    let red = first.frame.pixels[36];
    assert!((red >> 16) & 0xff > 224 && red & 0xff < 16);
    let first_stamp = first.frame.target;
    canvas.presented(first_stamp)?;
    reply(&mut canvas, 4, "applied")?;

    clock.0.store(1_000_000_000, Ordering::SeqCst);
    canvas.surface(identity, request(&panel, 5, "surface.select", json!({
        "itemId":"result:blue", "file":{"filename":"blue.png", "subfolder":"", "type":"output"}
    }))?, None, memory.claim(64 * 1024)?)?;
    assert!(prepared_frame(&canvas).is_none());
    let MainCanvasPresentation::Retain(retained) = canvas.presentation() else {
        return Err("Pending replacement withdrew the acknowledged native image".into());
    };
    assert_eq!(retained.target, first_stamp);
    assert_eq!(retained.identity, identity);
    assert_eq!(retained.allocation, allocation);
    assert!(canvas.presented(first_stamp).is_err());
    assert!(canvas.take_command().is_none());
    loop {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if prepared_frame(&canvas).is_some() {
            break;
        }
        wake.wait_after(revision)?;
    }
    assert!(canvas.take_command().is_none());
    let second = prepared_frame(&canvas).ok_or("Prepared blue frame is missing")?;
    let blue = second.frame.pixels[36];
    assert!((blue >> 16) & 0xff < 16 && blue & 0xff > 224);
    let second_stamp = second.frame.target;
    assert_ne!(first_stamp, second_stamp);
    canvas.presented(second_stamp)?;
    reply(&mut canvas, 5, "applied")?;

    replacement::Replacement {
        canvas: &mut canvas,
        client: &client,
        wake: &wake,
        clock: &clock,
        panel: &panel,
        memory: &memory,
        identity,
        allocation,
    }
    .rollback(second_stamp)?;

    let hidden_geometry = json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":8,"height":8}, "clip":{"x":0,"y":0,"width":0,"height":0},
        "physicalPixelsPerCssPixel":1, "visible":false, "focused":false
    }});
    canvas.surface(
        identity,
        request(&panel, 6, "surface.geometry", hidden_geometry)?,
        Some(allocation),
        memory.claim(64 * 1024)?,
    )?;
    assert!(matches!(canvas.presentation(), MainCanvasPresentation::Hide));
    assert!(canvas.take_command().is_none());
    let mut foreign = identity;
    foreign.navigation = Generation::new(2)?;
    canvas.hidden(foreign)?;
    assert!(canvas.take_command().is_none());
    canvas.hidden(identity)?;
    reply(&mut canvas, 6, "applied")?;
    canvas.hidden(identity)?;
    assert!(canvas.take_command().is_none());

    let visible_geometry = json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":8,"height":8}, "clip":{"x":10,"y":20,"width":8,"height":8},
        "physicalPixelsPerCssPixel":1, "visible":true, "focused":false
    }});
    canvas.surface(
        identity,
        request(&panel, 7, "surface.geometry", visible_geometry)?,
        Some(allocation),
        memory.claim(64 * 1024)?,
    )?;
    assert!(canvas.take_command().is_none());
    let restored_stamp = loop {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if let Some(frame) = prepared_frame(&canvas) {
            break frame.frame.target;
        }
        wake.wait_after(revision)?;
    };
    assert!(canvas.take_command().is_none());
    canvas.presented(restored_stamp)?;
    reply(&mut canvas, 7, "applied")?;

    clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
    let resized_geometry = json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":10,"height":10}, "clip":{"x":10,"y":20,"width":10,"height":10},
        "physicalPixelsPerCssPixel":1, "visible":true, "focused":false
    }});
    canvas.surface(
        identity,
        request(&panel, 8, "surface.geometry", resized_geometry)?,
        Some(allocation),
        memory.claim(64 * 1024)?,
    )?;
    assert!(matches!(canvas.presentation(), MainCanvasPresentation::Hide));
    assert!(canvas.take_command().is_none());
    let cancellation = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":8, "type":"cancel"
    }))?)?;
    canvas.surface(identity, cancellation, None, memory.claim(64 * 1024)?)?;
    reply(&mut canvas, 8, "cancelled")?;
    cancel_consumed(&mut canvas, identity)?;
    let final_geometry = json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":12,"y":24},
        "viewport":{"width":12,"height":12}, "clip":{"x":12,"y":24,"width":12,"height":12},
        "physicalPixelsPerCssPixel":1, "visible":true, "focused":false
    }});
    canvas.surface(
        identity,
        request(&panel, 9, "surface.geometry", final_geometry)?,
        Some(allocation),
        memory.claim(64 * 1024)?,
    )?;
    assert!(canvas.take_command().is_none());
    let resized_stamp = loop {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if let Some(frame) = prepared_frame(&canvas) {
            let extent = frame.frame.validate()?;
            assert_eq!((extent.width(), extent.height()), (12, 12));
            assert_eq!(frame.placement.viewport.width, 12);
            assert_eq!(frame.placement.viewport.x, 12);
            break frame.frame.target;
        }
        wake.wait_after(revision)?;
    };
    assert_ne!(resized_stamp, restored_stamp);
    canvas.presented(resized_stamp)?;
    reply(&mut canvas, 9, "applied")?;
    assert!(canvas.take_command().is_none());
    canvas.surface(identity, request(&panel, 10, "surface.select", json!({
        "itemId":"result:missing", "file":{"filename":"missing.png", "subfolder":"", "type":"output"}
    }))?, None, memory.claim(64 * 1024)?)?;
    loop {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if let Some(command) = canvas.take_command() {
            check_reply(command, 10, "error")?;
            break;
        }
        wake.wait_after(revision)?;
    }
    let MainCanvasPresentation::Retain(retained) = canvas.presentation() else {
        return Err("Failed source replacement withdrew the acknowledged native image".into());
    };
    assert_eq!(retained.target, resized_stamp);
    canvas.surface(identity, request(&panel, 11, "surface.select", json!({
        "itemId":"result:red", "file":{"filename":"red.png", "subfolder":"", "type":"output"}
    }))?, None, memory.claim(64 * 1024)?)?;
    let cancellation = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":11, "type":"cancel"
    }))?)?;
    canvas.surface(identity, cancellation, None, memory.claim(64 * 1024)?)?;
    reply(&mut canvas, 11, "cancelled")?;
    cancel_consumed(&mut canvas, identity)?;
    let MainCanvasPresentation::Retain(retained) = canvas.presentation() else {
        return Err("Cancelled source replacement withdrew the acknowledged native image".into());
    };
    assert_eq!(retained.target, resized_stamp);
    replacement::Replacement {
        canvas: &mut canvas,
        client: &client,
        wake: &wake,
        clock: &clock,
        panel: &panel,
        memory: &memory,
        identity,
        allocation,
    }
    .cancel_prepared_selection()?;
    canvas.close_until(Moment::from_nanos(10_000_000_000))?;
    assert!(matches!(canvas.presentation(), MainCanvasPresentation::Hide));
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if canvas.settled()? {
            break;
        }
        wake.wait_after(revision)?;
    }
    canvas.finish()?;
    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Deadline overflow")?;
    loop {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if owner.poll_shutdown()?.settled {
            break;
        }
        if Instant::now() >= deadline {
            return Err("Execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}
