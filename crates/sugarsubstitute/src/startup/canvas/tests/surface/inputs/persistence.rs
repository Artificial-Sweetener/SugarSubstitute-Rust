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
    /// `saved` holds; a save that could not be admitted retries after a further quiet period.
    fn save_until(&mut self, saved: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !saved() {
            if Instant::now() >= deadline {
                return Err("The input documents were not saved".into());
            }
            // Observe the latest change before its quiet period elapses.
            self.ask("context.request", None)?;
            self.clock.0.fetch_add(3_000_000_000, Ordering::SeqCst);
            self.ask("context.request", None)?;
        }
        Ok(())
    }
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

fn capture_command(document: &Value, targets: &Value) -> Value {
    json!({"type":"capture", "document":document, "targets":targets, "maximumSize":0})
}
