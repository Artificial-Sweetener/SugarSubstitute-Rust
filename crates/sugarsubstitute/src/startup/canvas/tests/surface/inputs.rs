// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Provision, show and capture workflow input documents through the native protocol, with real
//! execution and qualification fixture files standing in for ComfyUI input files.

use super::*;

mod identities;
mod persistence;
mod presentation;
mod sources;

/// The workflow every fixture syncs.
const WORKFLOW: &str = "workflow-6f1d1b2e-3a4c-4d5e-8f90-a1b2c3d4e5f6";

/// A started canvas whose fixture folder holds the input files, driven by native requests.
struct Inputs {
    canvas: EmbeddedCanvas,
    clock: Arc<ClockValue>,
    client: ExecutionClient,
    wake: Arc<Wake>,
    owner: ExecutionOwner,
    identity: MainIdentity,
    panel: PanelSession,
    memory: MemoryLease,
    next: u64,
    /// `source.changed` notifications delivered while waiting for replies.
    changes: Vec<Value>,
}

impl Inputs {
    /// Start a canvas anchored at `anchor`, whose fixture folder must already hold its files.
    fn start(anchor: &std::path::Path) -> Result<Self, Box<dyn Error>> {
        let clock = Arc::new(ClockValue(AtomicU64::new(0)));
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let mut canvas = EmbeddedCanvas::new(
            anchor.to_path_buf(),
            DiagnosticHealth::fixture(),
            clock.clone(),
            JobSequence::new(),
            ScopeSpec { id: ScopeId::new(1)?, deadline: None },
            funding()?,
        );
        canvas.start(
            client.clone(),
            "en",
            crate::native_canvas::gpu_policy::EmbeddedGpuPolicy::Cpu,
        )?;
        while !canvas.reported_ready {
            let observed = wake.revision();
            pump(&mut canvas, &client)?;
            if !canvas.reported_ready {
                wake.wait_after(observed)?;
            }
        }
        let identity = identity()?;
        Ok(Self {
            canvas,
            clock,
            client,
            wake,
            owner,
            identity,
            panel: PanelSession::for_main(identity)?,
            memory: MemoryLease::new(64 * 1024 * 1024, Box::new(Release)),
            next: 100,
            changes: Vec::new(),
        })
    }

    /// Send one native request of `kind`, with `command` when the family takes one.
    fn send(&mut self, kind: &str, command: Option<Value>) -> Result<u64, Box<dyn Error>> {
        self.next += 1;
        let mut message = json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":self.panel.as_str(),
            "requestId":self.next, "type":kind
        });
        if let Some(command) = command {
            message["command"] = command;
        }
        let request = CanvasNativeRequest::decode(&serde_json::to_vec(&message)?)?;
        self.canvas.native(
            self.identity,
            request,
            self.memory.claim(64 * 1024)?,
            Default::default(),
        )?;
        Ok(self.next)
    }

    /// Deliver completions until the reply to request `id` arrives.
    fn reply(&mut self, id: u64) -> Result<Value, Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let observed = self.wake.revision();
            pump(&mut self.canvas, &self.client)?;
            // Drain every command so notifications queued behind the reply are observed too.
            let mut answer = None;
            while let Some(command) = self.canvas.take_command() {
                match command {
                    DesktopStartupCommand::SendCanvasNative(reply) => {
                        let value: Value = serde_json::from_slice(reply.payload())?;
                        if value["requestId"] != id || answer.is_some() {
                            return Err(format!(
                                "Unexpected reply while waiting for {id}: {value}"
                            )
                            .into());
                        }
                        answer = Some(value);
                    }
                    DesktopStartupCommand::SendCanvasNotification(notification) => {
                        self.changes.push(serde_json::from_slice(notification.payload())?);
                    }
                    _ => {}
                }
            }
            if let Some(answer) = answer {
                return Ok(answer);
            }
            if Instant::now() >= deadline {
                return Err(format!("Request {id} was not answered").into());
            }
            self.wait_briefly(observed);
        }
    }

    /// Send a request and wait for its reply.
    fn ask(&mut self, kind: &str, command: Option<Value>) -> Result<Value, Box<dyn Error>> {
        let id = self.send(kind, command)?;
        self.reply(id)
    }

    /// Run a workflow command and return its result.
    fn workflow(&mut self, command: Value) -> Result<Value, Box<dyn Error>> {
        Ok(self.ask("workflow.request", Some(command))?["result"].take())
    }

    /// Ask for the context until `accept` takes its document, delivering completions between.
    fn context_until(&mut self, accept: impl Fn(&Value) -> bool) -> Result<Value, Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let context = self.ask("context.request", None)?;
            if accept(&context["context"]["document"]) {
                return Ok(context);
            }
            if Instant::now() >= deadline {
                return Err(format!("The context never matched: {context}").into());
            }
            let observed = self.wake.revision();
            pump(&mut self.canvas, &self.client)?;
            self.wait_briefly(observed);
        }
    }

    /// Wait up to a few milliseconds for a completion wake; work that needs no job, such as a
    /// route change waiting for the next poll, proceeds on the following pump.
    fn wait_briefly(&self, observed: u64) {
        let guard = self.wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(
            self.wake
                .changed
                .wait_timeout_while(guard, Duration::from_millis(20), |value| *value == observed),
        );
    }

    /// Press the primary mouse button at the first of `points` of the shown input document and
    /// drag it through the rest, as the desktop adapter delivers a routed gesture; the button
    /// stays pressed until [`Self::release`].
    fn drag(&mut self, points: &[[f64; 2]]) -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_canvas::{PointerSample, ToolInput, ToolModifiers};
        use sugarsubstitute_desktop::canvas::{Input, Lifetime};
        let session = self.canvas.session.as_mut().ok_or("The canvas has no session")?;
        for (index, point) in points.iter().enumerate() {
            let device = session.device_at(*point)?;
            let sample = PointerSample::mouse(device, ToolModifiers::NONE);
            session.input(Input::Tool(ToolInput::Pointer(sample)))?;
            if index == 0 {
                session.input(primary_button(true))?;
            }
        }
        Ok(())
    }

    /// Release the primary mouse button a [`Self::drag`] pressed, ending its gesture.
    fn release(&mut self) -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_desktop::canvas::Lifetime;
        let session = self.canvas.session.as_mut().ok_or("The canvas has no session")?;
        session.input(primary_button(false))
    }

    /// Close the canvas and its execution owner after every job settled.
    fn close(mut self) -> Result<(), Box<dyn Error>> {
        let cutoff = Moment::from_nanos(self.clock.0.load(Ordering::SeqCst) + 10_000_000_000);
        self.canvas.close_until(cutoff)?;
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if self.canvas.settled()? {
                break;
            }
            self.wake.wait_after(observed)?;
        }
        self.canvas.finish()?;
        self.owner.request_shutdown(cutoff);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if self.owner.poll_shutdown()?.settled {
                break;
            }
            if Instant::now() >= deadline {
                return Err("Execution did not settle".into());
            }
            std::hint::spin_loop();
        }
        self.owner.finish_shutdown()?;
        Ok(())
    }
}

/// The primary mouse button changing to `pressed`, as routed tool input.
fn primary_button(pressed: bool) -> sugarsubstitute_desktop::canvas::Input {
    use sugarsubstitute_canvas::{ToolButton, ToolInput, ToolModifiers};
    sugarsubstitute_desktop::canvas::Input::Tool(ToolInput::Button {
        button: ToolButton::Primary,
        pressed,
        modifiers: ToolModifiers::NONE,
    })
}

/// Write an RGBA fixture image whose pixels `pixel` chooses.
fn rgba(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixel: impl Fn(u32, u32) -> [u8; 4],
) -> Result<(), Box<dyn Error>> {
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            data.extend(pixel(x, y));
        }
    }
    let mut encoder = png::Encoder::new(std::fs::File::create(path)?, width, height);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&data)?;
    writer.finish()?;
    Ok(())
}

/// A fixture folder below a fresh anchor holding the base image and a half-covered mask.
fn anchor() -> Result<tempfile::TempDir, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("canvas-qualification-output");
    std::fs::create_dir(&output)?;
    rgba(&output.join("base.png"), 8, 6, |x, y| [x as u8 * 30, y as u8 * 40, 100, 255])?;
    rgba(&output.join("mask.png"), 8, 6, |x, _| [if x < 4 { 255 } else { 0 }, 0, 0, 255])?;
    Ok(directory)
}

fn input_file(name: &str) -> Value {
    json!({"filename":name, "subfolder":"", "type":"input"})
}

/// An image surface with a loaded and an empty mask, and a blank mask-only surface.
fn surfaces(initial: Option<&str>) -> Value {
    json!([
        {"key":"image", "label":"Load Image",
         "base":{"kind":"file", "file":input_file("base.png")},
         "masks":[
            {"key":"painted", "label":"Painted", "color":"#ff0000", "opacity":0.5,
             "initial":initial.map(input_file), "channel":"red"},
            {"key":"empty", "label":"Empty", "color":"#00ff00", "opacity":0.5,
             "initial":null, "channel":"alpha"}]},
        {"key":"latent", "label":"Empty Latent",
         "base":{"kind":"blank", "width":5, "height":4},
         "masks":[{"key":"region", "label":"Region", "color":"#0000ff", "opacity":0.5,
                   "initial":null, "channel":"red"}]}
    ])
}

fn sync(initial: Option<&str>) -> Value {
    json!({"type":"sync", "workflow":WORKFLOW, "surfaces":surfaces(initial)})
}

/// The synced surface with `key`.
fn synced<'a>(result: &'a Value, key: &str) -> Result<&'a Value, Box<dyn Error>> {
    result["surfaces"]
        .as_array()
        .and_then(|surfaces| surfaces.iter().find(|surface| surface["key"] == key))
        .ok_or_else(|| format!("Surface {key} did not sync: {result}").into())
}

/// The layer a synced mask key maps to.
fn layer(surface: &Value, key: &str) -> Result<String, Box<dyn Error>> {
    surface["masks"]
        .as_array()
        .and_then(|masks| masks.iter().find(|mask| mask["key"] == key))
        .and_then(|mask| mask["layerId"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("Mask {key} has no layer: {surface}").into())
}

fn activate(surface: Option<&str>, mask: Option<&str>, route: &str) -> Value {
    json!({"type":"activate", "workflow":WORKFLOW, "route":route, "surface":surface, "mask":mask})
}

#[test]
fn workflow_sync_maps_every_key_to_a_document_and_layer_after_its_work_settles()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let context = inputs.ask("context.request", None)?;
    assert_eq!(
        context["context"]["canvas"]["capabilities"],
        json!({"displayOutput":true, "insertImageLayer":false, "positionImageLayer":false,
            "geometry":true, "editing":true, "workflowSurfaces":true})
    );
    assert_eq!(context["context"]["document"], Value::Null);
    let result = inputs.workflow(sync(Some("mask.png")))?;
    assert_eq!(result["kind"], "synced", "{result}");
    let image = synced(&result, "image")?;
    assert_eq!(image["document"]["width"], 8);
    assert_eq!(image["document"]["height"], 6);
    assert_eq!(layer(image, "painted")?, "layer-2");
    assert_eq!(layer(image, "empty")?, "layer-3");
    let latent = synced(&result, "latent")?;
    assert_eq!(
        (latent["document"]["width"].clone(), latent["document"]["height"].clone()),
        (json!(5), json!(4))
    );
    assert_ne!(image["document"]["id"], latent["document"]["id"]);
    // A later sync keeps every identity; listing fewer surfaces retires the rest.
    let again = inputs.workflow(sync(None))?;
    assert_eq!(synced(&again, "image")?, image);
    let only = json!({"type":"sync", "workflow":WORKFLOW,
        "surfaces":[surfaces(None)[1].clone()]});
    let retired = inputs.workflow(only)?;
    assert_eq!(retired["surfaces"].as_array().map(Vec::len), Some(1));
    assert_eq!(synced(&retired, "latent")?, latent);
    // A missing base file fails the sync with a typed failure and changes nothing else.
    let missing = json!({"type":"sync", "workflow":WORKFLOW, "surfaces":[
        {"key":"broken", "label":"Broken", "base":{"kind":"file", "file":input_file("absent.png")},
         "masks":[]}]});
    let failed = inputs.workflow(missing)?;
    assert_eq!(
        (failed["kind"].clone(), failed["status"].clone()),
        (json!("failure"), json!("error"))
    );
    let unknown = inputs.workflow(activate(Some("nowhere"), None, "input"))?;
    assert_eq!(unknown["status"], "stale");
    let invalid = inputs.workflow(json!({"type":"sync", "workflow":WORKFLOW, "surfaces":[
        surfaces(None)[1].clone(), surfaces(None)[1].clone()]}))?;
    assert_eq!(invalid["status"], "error");
    inputs.close()
}

#[test]
fn activation_shows_input_documents_in_the_context_until_the_output_route_returns()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(Some("mask.png")))?;
    let image = synced(&result, "image")?.clone();
    let latent = synced(&result, "latent")?.clone();
    let activated = inputs.workflow(activate(Some("image"), Some("painted"), "input"))?;
    assert_eq!(activated, json!({"kind":"activated"}));
    let shown = inputs.context_until(|document| document["id"] == image["document"]["id"])?;
    assert_eq!(shown["context"]["document"], image["document"]);
    let geometry = inputs.ask("geometry.request", Some(json!({"type":"context"})))?;
    assert_eq!(
        geometry["result"],
        json!({"kind":"context", "supported":true, "resize":false, "selectionSupported":true,
            "document":image["document"], "selection":null})
    );
    let resize = inputs.ask(
        "geometry.request",
        Some(json!({"type":"resize", "document":image["document"], "size":{"width":4,"height":4}})),
    )?;
    assert_eq!(resize["result"]["status"], "unsupported");
    inputs.workflow(activate(Some("latent"), Some("region"), "input"))?;
    inputs.context_until(|document| document["id"] == latent["document"]["id"])?;
    assert_eq!(inputs.workflow(activate(None, None, "output"))?, json!({"kind":"activated"}));
    inputs.context_until(Value::is_null)?;
    // Appearance and hover apply to synced masks; unknown masks are stale.
    let appearance = json!({"type":"appearance", "workflow":WORKFLOW, "masks":[
        {"surface":"image", "mask":"painted", "color":"#123456", "opacity":0.25}]});
    assert_eq!(inputs.workflow(appearance)?, json!({"kind":"applied"}));
    let hover = json!({"type":"hover", "workflow":WORKFLOW, "surface":"image", "mask":"empty"});
    assert_eq!(inputs.workflow(hover)?, json!({"kind":"applied"}));
    let unknown = json!({"type":"hover", "workflow":WORKFLOW, "surface":"image", "mask":"gone"});
    assert_eq!(inputs.workflow(unknown)?["status"], "stale");
    // Resizing a blank surface replies with its new geometry; a file surface cannot resize.
    let resize = |surface: &str| {
        json!({"type":"resize", "workflow":WORKFLOW, "surface":surface, "width":7, "height":3,
            "mode":{"kind":"canvas", "anchor":"center"}})
    };
    let resized = inputs.workflow(resize("latent"))?;
    let document = &synced(&resized, "latent")?["document"];
    assert_eq!((document["width"].clone(), document["height"].clone()), (json!(7), json!(3)));
    assert_eq!(document["id"], latent["document"]["id"]);
    assert_ne!(document["geometryRevision"], latent["document"]["geometryRevision"]);
    assert_eq!(inputs.workflow(resize("image"))?["status"], "error");
    inputs.close()
}

#[test]
fn picking_a_mask_file_again_after_its_loader_moved_on_reloads_it() -> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(Some("mask.png")))?;
    let image = synced(&result, "image")?.clone();
    let painted = layer(&image, "painted")?;
    inputs.workflow(activate(Some("image"), Some("painted"), "input"))?;
    inputs.context_until(|document| document["id"] == image["document"]["id"])?;
    let mask = json!([{"kind":"layer", "layerId":painted}]);
    let read = |inputs: &mut Inputs| -> Result<Value, Box<dyn Error>> {
        let command = json!({"type":"capture", "document":image["document"], "targets":mask,
            "maximumSize":0});
        Ok(inputs.ask("source.request", Some(command))?["result"]["snapshots"][0]["pixels"].take())
    };
    let loaded = read(&mut inputs)?;
    // Painting changes the mask; a sync without the file leaves the painting in place.
    let brush = inputs.ask("edit.request", Some(json!({"type":"tool", "tool":"brush"})))?;
    assert_eq!(brush["result"]["state"]["tool"], "brush", "{brush}");
    inputs.drag(&[[5.0, 1.0], [7.0, 1.0], [7.0, 4.0]])?;
    inputs.release()?;
    inputs.workflow(sync(None))?;
    let edited = read(&mut inputs)?;
    assert_ne!(edited, loaded, "the stroke changed the mask");
    // Picking the same file again loads it again, replacing the painting.
    inputs.workflow(sync(Some("mask.png")))?;
    assert_eq!(read(&mut inputs)?, loaded);
    inputs.close()
}

/// A sync of many blank surfaces whose key map exceeds the lane credit's inline bound, as
/// SugarCubes declares them for a large regional workflow.
fn wide_sync(count: usize) -> Value {
    let surfaces: Vec<Value> = (0..count)
        .map(|index| {
            let masks: Vec<Value> = (0..4)
                .map(|entry| {
                    json!({"key":format!("[\"[\\\"{}\\\",\\\"mask\\\"]\",\"5b0c7a3e-{index:04}-4f6d-9a1b-c2d3e4f5a6{entry:02}\"]", 900 + index),
                       "label":format!("Region {index}.{entry}"), "color":"#ff0000", "opacity":0.5,
                       "initial":null, "channel":"red"})
                })
                .collect();
            json!({"key":format!("@synthetic/{:016x}", 0xb01c_0000_u64 + index as u64),
                "label":format!("Bulk {index}"), "base":{"kind":"blank", "width":8, "height":8},
                "masks":masks})
        })
        .collect();
    json!({"type":"sync", "workflow":WORKFLOW, "surfaces":surfaces})
}

#[test]
fn a_sync_whose_key_map_exceeds_the_inline_reply_bound_is_answered_and_the_canvas_keeps_serving()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    for count in [3, 40, 64] {
        let id = inputs.send("workflow.request", Some(wide_sync(count)))?;
        let reply = inputs.reply(id)?;
        assert!(
            count == 3
                || serde_json::to_vec(&reply)?.len()
                    > sugarsubstitute_desktop::CanvasNativeReply::INLINE_PAYLOAD_BYTES,
            "{count} surfaces must exceed the inline bound"
        );
        let result = &reply["result"];
        assert_eq!(result["kind"], "synced", "{result}");
        assert_eq!(result["surfaces"].as_array().map(Vec::len), Some(count));
        let masks: usize = result["surfaces"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|surface| surface["masks"].as_array().map_or(0, Vec::len))
            .sum();
        assert_eq!(masks, count * 4);
        // The canvas still answers its next request.
        assert!(inputs.ask("context.request", None)?["context"].is_object());
    }
    inputs.close()
}
