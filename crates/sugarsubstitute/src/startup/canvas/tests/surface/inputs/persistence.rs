// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore input documents across a restart and rebuild them when the archive is corrupt.

use super::*;

/// The session directory below a fixture anchor.
fn session_directory(anchor: &std::path::Path) -> std::path::PathBuf {
    anchor.join("desktop-data").join("input-canvas")
}

impl Inputs {
    /// Let autosave debounce periods elapse on the fixture clock and deliver completions until
    /// `saved` holds. The clock advances again only after a quiet stretch, so a running save never
    /// outlives its deadline on the fixture clock; a save that could not be admitted retries
    /// after the next quiet period.
    fn save_until(&mut self, saved: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(30);
        // Observe the latest change before its quiet period elapses.
        self.ask("context.request", None)?;
        while !saved() {
            if Instant::now() >= deadline {
                return Err("The input documents were not saved".into());
            }
            self.clock.0.fetch_add(3_000_000_000, Ordering::SeqCst);
            let quiet = Instant::now() + Duration::from_secs(2);
            while !saved() && Instant::now() < quiet {
                let observed = self.wake.revision();
                self.ask("context.request", None)?;
                self.wait_briefly(observed);
            }
        }
        Ok(())
    }

    /// Drag the primary mouse button through document points of the shown input document, as
    /// the desktop adapter delivers a routed gesture.
    fn stroke(&mut self, points: &[[f64; 2]]) -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_canvas::{PointerSample, ToolButton, ToolInput, ToolModifiers};
        use sugarsubstitute_desktop::canvas::{Input, Lifetime};
        let session = self.canvas.session.as_mut().ok_or("The canvas has no session")?;
        let button = |pressed| {
            Input::Tool(ToolInput::Button {
                button: ToolButton::Primary,
                pressed,
                modifiers: ToolModifiers::NONE,
            })
        };
        for (index, point) in points.iter().enumerate() {
            let device = session.device_at(*point)?;
            let sample = PointerSample::mouse(device, ToolModifiers::NONE);
            session.input(Input::Tool(ToolInput::Pointer(sample)))?;
            if index == 0 {
                session.input(button(true))?;
            }
        }
        session.input(button(false))
    }
}

#[test]
fn a_brush_stroke_made_through_editing_is_restored_after_a_restart() -> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let archive = session_directory(directory.path()).join("input-documents.ssdoc");
    let mut first = Inputs::start(directory.path())?;
    let result = first.workflow(sync(None))?;
    let image = synced(&result, "image")?.clone();
    let empty = layer(&image, "empty")?;
    first.workflow(activate(Some("image"), Some("empty"), "input"))?;
    first.context_until(|document| document["id"] == image["document"]["id"])?;
    first.save_until(|| archive.is_file())?;
    let saved = std::fs::read(&archive)?;
    let mask = json!([{"kind":"layer", "layerId":empty}]);
    let blank = first.ask("source.request", Some(capture_command(&image["document"], &mask)))?;

    let brush = first.ask("edit.request", Some(json!({"type":"tool", "tool":"brush"})))?;
    assert_eq!(brush["result"]["state"]["tool"], "brush", "{brush}");
    first.stroke(&[[2.0, 3.0], [6.0, 3.0]])?;
    let state = first.ask("edit.request", Some(json!({"type":"state"})))?;
    assert_eq!(state["result"]["state"]["history"]["undo"], true, "{state}");
    let painted = first.ask("source.request", Some(capture_command(&image["document"], &mask)))?;
    assert_ne!(
        painted["result"]["snapshots"][0]["pixels"], blank["result"]["snapshots"][0]["pixels"],
        "the stroke painted the mask"
    );
    first.save_until(|| std::fs::read(&archive).is_ok_and(|bytes| bytes != saved))?;
    first.close()?;

    let mut second = Inputs::start(directory.path())?;
    second.workflow(sync(None))?;
    second.context_until(|document| document["id"] == image["document"]["id"])?;
    let restored =
        second.ask("source.request", Some(capture_command(&image["document"], &mask)))?;
    assert_eq!(
        restored["result"]["snapshots"][0]["pixels"],
        painted["result"]["snapshots"][0]["pixels"]
    );
    second.close()
}

#[test]
fn restored_documents_keep_their_identities_layers_and_mask_content_across_a_restart()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let archive = session_directory(directory.path()).join("input-documents.ssdoc");
    let mut first = Inputs::start(directory.path())?;
    let result = first.workflow(sync(Some("mask.png")))?;
    let image = synced(&result, "image")?.clone();
    let resize = json!({"type":"resize", "workflow":WORKFLOW, "surface":"latent", "width":7,
        "height":3, "mode":{"kind":"canvas", "anchor":"center"}});
    let resized = first.workflow(resize)?;
    let latent = synced(&resized, "latent")?.clone();
    first.workflow(activate(Some("image"), Some("painted"), "input"))?;
    first.context_until(|document| document["id"] == image["document"]["id"])?;
    let mask = json!([{"kind":"layer", "layerId":"layer-2"}]);
    let painted = first.ask("source.request", Some(capture_command(&image["document"], &mask)))?;
    first.save_until(|| archive.is_file())?;
    first.close()?;
    // The same reference now names a different file; the restored mask must not reload it.
    let output = directory.path().join("canvas-qualification-output");
    rgba(&output.join("mask.png"), 8, 6, |_, _| [255, 0, 0, 255])?;

    let mut second = Inputs::start(directory.path())?;
    let result = second.workflow(sync(Some("mask.png")))?;
    assert_eq!(synced(&result, "image")?, &image);
    assert_eq!(synced(&result, "latent")?, &latent);
    // The archived Input route returns once the image's base has loaded again.
    second.context_until(|document| document["id"] == image["document"]["id"])?;
    let restored =
        second.ask("source.request", Some(capture_command(&image["document"], &mask)))?;
    assert_eq!(
        restored["result"]["snapshots"][0]["pixels"],
        painted["result"]["snapshots"][0]["pixels"]
    );
    let current = second.ask("source.request", Some(json!({"type":"context"})))?;
    assert_eq!(current["result"]["currentLayerId"], "layer-2");
    // A surface created after the restart continues the archived identity allocation.
    let mut extended = surfaces(None);
    let added = json!({"key":"later", "label":"Later", "base":{"kind":"blank", "width":2,
        "height":2}, "masks":[]});
    extended.as_array_mut().ok_or("Surfaces are not a list")?.push(added);
    let grown =
        second.workflow(json!({"type":"sync", "workflow":WORKFLOW, "surfaces":extended}))?;
    let ids = ["image", "latent", "later"]
        .map(|key| synced(&grown, key).map(|surface| surface["document"]["id"].clone()));
    let [image_id, latent_id, later_id] = ids;
    assert_ne!(later_id?, image_id?);
    assert_ne!(synced(&grown, "later")?["document"]["id"], latent_id?);
    second.close()
}

#[test]
fn a_corrupt_archive_is_kept_as_evidence_while_documents_rebuild_from_the_workflow()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let session = session_directory(directory.path());
    std::fs::create_dir_all(&session)?;
    std::fs::write(session.join("input-documents.ssdoc"), b"not an input-document archive")?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(None))?;
    assert_eq!(result["kind"], "synced", "{result}");
    assert_eq!(layer(synced(&result, "image")?, "painted")?, "layer-2");
    let evidence = || {
        std::fs::read_dir(&session).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with("input-documents.rejected-") && name.ends_with(".ssdoc")
            })
        })
    };
    inputs.save_until(|| {
        evidence()
            && std::fs::read(session.join("input-documents.ssdoc"))
                .is_ok_and(|bytes| bytes.starts_with(b"SSINDOC\0"))
    })?;
    inputs.close()
}

#[test]
fn closing_saves_edits_made_before_the_autosave_debounce_elapses() -> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let archive = session_directory(directory.path()).join("input-documents.ssdoc");
    let mut first = Inputs::start(directory.path())?;
    first.workflow(sync(None))?;
    first.save_until(|| archive.is_file())?;
    let saved = std::fs::read(&archive)?;
    // The edit lands on the fixture clock with no time for its debounce to elapse.
    let resize = json!({"type":"resize", "workflow":WORKFLOW, "surface":"latent", "width":7,
        "height":3, "mode":{"kind":"scale", "resampling":"nearest"}});
    let resized = first.workflow(resize)?;
    let latent = synced(&resized, "latent")?.clone();
    first.close()?;
    assert_ne!(std::fs::read(&archive)?, saved, "close saved the edit");

    let mut second = Inputs::start(directory.path())?;
    let result = second.workflow(sync(None))?;
    assert_eq!(synced(&result, "latent")?, &latent);
    second.close()
}

fn capture_command(document: &Value, targets: &Value) -> Value {
    json!({"type":"capture", "document":document, "targets":targets, "maximumSize":0})
}
