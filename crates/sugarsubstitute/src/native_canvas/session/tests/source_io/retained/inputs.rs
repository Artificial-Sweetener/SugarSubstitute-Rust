// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise synced input documents through real jobs, ComfyUI-file adapters and retirement.

use super::*;
use crate::native_canvas::session::{
    SelectedOriginals,
    acquisition::SourceAcquirer,
    inputs::{
        ComfyFile, InputDocumentId, InputReply, InputSnapshot, MaskChannel, MaskSpec, SurfaceBase,
        SurfaceOutcome, SurfaceSpec, SyncedSurface,
    },
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use sugarsubstitute_canvas::{DocumentSnapshot, LayerContent, LayerId, MaskAppearance};

mod edits;
mod masks;
mod route;

const WORKFLOW: &str = "workflow-0b6f0c84-6c5e-4b0f-9d0c-2c1d0f1e2a3b";

/// A ComfyUI input file served from a fixture image; its transfer settles once acquired.
struct FileSource {
    path: PathBuf,
    settled: AtomicBool,
    acquisitions: AtomicU64,
}

impl SourceAcquirer for FileSource {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        self.acquisitions.fetch_add(1, Ordering::SeqCst);
        self.settled.store(true, Ordering::Release);
        Ok(SelectedOriginals::single(ImageSource::file(self.path.clone())))
    }

    fn cancel(&self) {}

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        self.settled.store(true, Ordering::Release);
        Ok(())
    }

    fn settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }
}

/// Write an RGBA PNG whose pixels `pixel` chooses.
fn image(
    directory: &Path,
    name: &str,
    extent: Extent,
    pixel: impl Fn(u32, u32) -> [u8; 4],
) -> Result<PathBuf, Box<dyn Error>> {
    let path = directory.join(name);
    let mut data = Vec::new();
    for y in 0..extent.height() {
        for x in 0..extent.width() {
            data.extend(pixel(x, y));
        }
    }
    let mut encoder =
        png::Encoder::new(std::fs::File::create(&path)?, extent.width(), extent.height());
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&data)?;
    writer.finish()?;
    Ok(path)
}

/// A file reference and its adapter, which a test keeps to observe acquisition.
fn file(reference: &str, path: &Path) -> (ComfyFile, Arc<FileSource>) {
    let source = Arc::new(FileSource {
        path: path.to_path_buf(),
        settled: AtomicBool::new(false),
        acquisitions: AtomicU64::new(0),
    });
    (ComfyFile { reference: Arc::from(reference), acquirer: source.clone() }, source)
}

fn appearance(color: [u8; 3], opacity: f32) -> Result<MaskAppearance, Box<dyn Error>> {
    Ok(MaskAppearance::new(color, opacity)?)
}

fn mask(key: &str, initial: Option<ComfyFile>) -> Result<MaskSpec, Box<dyn Error>> {
    Ok(MaskSpec {
        key: Arc::from(key),
        label: Arc::from(format!("{key} label")),
        appearance: appearance([0, 120, 215], 0.5)?,
        channel: MaskChannel::Red,
        initial,
    })
}

fn surface(key: &str, base: SurfaceBase, masks: Vec<MaskSpec>) -> SurfaceSpec {
    SurfaceSpec { key: Arc::from(key), label: Arc::from(format!("{key} image")), base, masks }
}

impl Fixture {
    /// Whether a job or disposal will still wake the test.
    fn inputs_busy(&self) -> bool {
        let session = &self.session;
        session.inputs.running()
            || session.loading.is_some()
            || session.rendering.is_some()
            || session.displaying.is_some()
            || session.retirement.is_some()
            || session.acquisition_retirement.is_some()
    }

    /// Deliver completions until input work, route changes, frames and disposal are idle.
    fn settle_inputs(&mut self) -> Result<(), Box<dyn Error>> {
        let mut idle = 0;
        while idle < 100 {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if let Some(cause) = &self.session.failed {
                return Err(format!("Canvas failed while settling inputs: {cause}").into());
            }
            let pending = !self.session.inputs.idle()
                || self.session.pending_retirement.is_some()
                || self.session.pending_open.is_some();
            if !pending && !self.inputs_busy() {
                return Ok(());
            }
            if self.inputs_busy() {
                let session = &self.session;
                let state = format!(
                    "input job {}, loading {}, rendering {}, displaying {}, retirement {}, \
                     acquisition {}",
                    session.inputs.running(),
                    session.loading.is_some(),
                    session.rendering.is_some(),
                    session.displaying.is_some(),
                    session.retirement.is_some(),
                    session.acquisition_retirement.is_some(),
                );
                self.wake
                    .wait_after(observed)
                    .map_err(|cause| format!("Input work stalled ({state}): {cause}"))?;
                idle = 0;
            } else {
                idle += 1;
            }
        }
        Err("Input work did not settle".into())
    }

    /// Sync and settle, returning the sync's reply.
    fn sync(&mut self, surfaces: Vec<SurfaceSpec>) -> Result<InputReply, Box<dyn Error>> {
        let ticket = self.session.sync_inputs(WORKFLOW, surfaces)?;
        self.settle_inputs()?;
        let replies = self.session.take_input_replies();
        let reply = replies.into_iter().find(|reply| reply.ticket == ticket);
        reply.ok_or_else(|| "The sync did not reply".into())
    }

    /// The current content of a synced document.
    fn snapshot(&self, document: InputDocumentId) -> Result<DocumentSnapshot, Box<dyn Error>> {
        self.session
            .input_content(document)
            .ok_or_else(|| "The input document is unavailable".into())
    }
}

/// The synced state of the surface with `key`.
fn synced<'a>(reply: &'a InputReply, key: &str) -> Result<&'a SyncedSurface, Box<dyn Error>> {
    reply
        .surfaces
        .iter()
        .find_map(|outcome| match outcome {
            SurfaceOutcome::Synced(surface) if &*surface.key == key => Some(surface),
            _ => None,
        })
        .ok_or_else(|| format!("Surface {key} did not sync: {:?}", reply.surfaces).into())
}

/// The layer a synced mask key maps to.
fn layer(surface: &SyncedSurface, key: &str) -> Result<LayerId, Box<dyn Error>> {
    surface
        .masks
        .iter()
        .find(|mask| &*mask.key == key)
        .map(|mask| mask.layer)
        .ok_or_else(|| format!("Mask {key} is absent").into())
}

/// A mask layer's coverage.
fn coverage(
    snapshot: &DocumentSnapshot,
    layer: LayerId,
) -> Result<sugarsubstitute_image_ops::Coverage, Box<dyn Error>> {
    snapshot
        .layers()
        .iter()
        .find(|candidate| candidate.id() == layer)
        .and_then(|candidate| match candidate.content() {
            LayerContent::Coverage { coverage, .. } => Some(coverage.clone()),
            _ => None,
        })
        .ok_or_else(|| "The mask layer is absent".into())
}

#[test]
fn sync_creates_file_and_blank_surfaces_and_keeps_identities_across_syncs()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        let red = image(&directory, "red.png", Extent::new(4, 3)?, |_, _| [255, 0, 0, 255])?;
        let (base, base_source) = file("input/red.png", &red);
        let reply = fixture.sync(vec![
            surface(
                "image",
                SurfaceBase::File(base),
                vec![mask("first", None)?, mask("second", None)?],
            ),
            surface("canvas", SurfaceBase::Blank(Extent::new(8, 6)?), vec![mask("only", None)?]),
        ])?;
        assert_eq!(base_source.acquisitions.load(Ordering::SeqCst), 1);
        assert_eq!(&*reply.workflow, WORKFLOW);
        let image_surface = synced(&reply, "image")?.clone();
        let canvas = synced(&reply, "canvas")?.clone();
        assert_eq!((image_surface.extent, canvas.extent), (Extent::new(4, 3)?, Extent::new(8, 6)?));
        assert_eq!(layer(&image_surface, "first")?.get(), 2);
        assert_eq!(layer(&image_surface, "second")?.get(), 3);
        assert_eq!(layer(&canvas, "only")?.get(), 2);
        assert_ne!(image_surface.document, canvas.document);
        for document in [image_surface.document, canvas.document] {
            assert!(document.get() >= 1 << 63, "input identities never meet output selections");
        }
        assert_ne!(image_surface.geometry, canvas.geometry);
        let snapshot = fixture.snapshot(image_surface.document)?;
        let names: Vec<_> = snapshot.layers().iter().map(|layer| layer.name().to_owned()).collect();
        assert_eq!(names, ["image image", "first label", "second label"]);
        assert!(snapshot.layers()[0].locked());
        assert!(matches!(snapshot.layers()[0].content(), LayerContent::Raster { .. }));
        assert!(!snapshot.layers()[1].locked());
        assert_eq!(coverage(&snapshot, LayerId::new(2)?)?.uniform(), Some(0));
        let backing = fixture.snapshot(canvas.document)?;
        assert!(matches!(backing.layers()[0].content(), LayerContent::VectorRectangle(_)));
        assert!(backing.layers()[0].locked());

        // Reordering, adding and removing masks keeps every remaining identity; a later blank
        // size is not applied, and an unchanged base is not acquired again.
        let (same_base, same_source) = file("input/red.png", &red);
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(same_base),
            vec![mask("third", None)?, mask("second", None)?],
        )])?;
        assert_eq!(same_source.acquisitions.load(Ordering::SeqCst), 0);
        let resynced = synced(&reply, "image")?;
        assert_eq!(resynced.document, image_surface.document);
        assert_eq!(layer(resynced, "second")?.get(), 3);
        assert_eq!(layer(resynced, "third")?.get(), 4);
        let ids: Vec<_> = fixture
            .snapshot(resynced.document)?
            .layers()
            .iter()
            .map(|layer| layer.id().get())
            .collect();
        assert_eq!(ids, [1, 4, 3]);
        assert!(fixture.snapshot(canvas.document).is_err(), "unlisted surfaces are retired");
        assert!(matches!(
            fixture.session.input_snapshot(canvas.document, None),
            InputSnapshot::Unavailable
        ));
        let reply = fixture.sync(vec![surface(
            "canvas",
            SurfaceBase::Blank(Extent::new(8, 6)?),
            vec![mask("only", None)?],
        )])?;
        let recreated = synced(&reply, "canvas")?;
        assert_ne!(recreated.document, canvas.document, "a retired surface returns as a new one");
        assert!(
            fixture
                .session
                .sync_inputs(
                    WORKFLOW,
                    vec![
                        surface("same", SurfaceBase::Blank(Extent::new(2, 2)?), Vec::new()),
                        surface("same", SurfaceBase::Blank(Extent::new(2, 2)?), Vec::new()),
                    ]
                )
                .is_err()
        );
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn a_changed_base_keeps_masks_at_its_size_and_blanks_them_at_another() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        let red = image(&directory, "red.png", Extent::new(4, 3)?, |_, _| [255, 0, 0, 255])?;
        let green = image(&directory, "green.png", Extent::new(4, 3)?, |_, _| [0, 255, 0, 255])?;
        let wide = image(&directory, "wide.png", Extent::new(6, 5)?, |_, _| [0, 0, 255, 255])?;
        let (base, _) = file("input/red.png", &red);
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(base),
            vec![mask("mask", None)?],
        )])?;
        let synced_surface = synced(&reply, "image")?.clone();
        let document = synced_surface.document;
        let mask_layer = layer(&synced_surface, "mask")?;
        let original_asset = fixture.snapshot(document)?.layers()[0].content().clone();
        // Paint the mask through the document's own edit history.
        let memory =
            fixture.session.resources.as_ref().ok_or("Canvas storage absent")?.memory.clone();
        {
            let (state, _) =
                fixture.session.input_document_mut(document).ok_or("Document absent")?;
            let scene = state.scene.as_mut().ok_or("Scene absent")?;
            let snapshot = scene.document.snapshot();
            let current = snapshot
                .layers()
                .iter()
                .find(|layer| layer.id() == mask_layer)
                .ok_or("Mask absent")?;
            let LayerContent::Coverage { coverage, appearance } = current.content() else {
                return Err("Mask is not coverage".into());
            };
            let mut painted = coverage.clone();
            painted.update(
                sugarsubstitute_image_ops::Region::new(1, 1, 2, 1)?,
                &memory,
                |_, _, _| u16::MAX,
            )?;
            let content = LayerContent::Coverage { coverage: painted, appearance: *appearance };
            scene.document.apply(sugarsubstitute_canvas::DocumentEdit::Replace(
                current.clone().with_content(content),
            ))?;
        }
        let (changed, _) = file("input/green.png", &green);
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(changed),
            vec![mask("mask", None)?],
        )])?;
        let rebased = synced(&reply, "image")?;
        assert_eq!(rebased.document, document);
        let snapshot = fixture.snapshot(document)?;
        assert_ne!(snapshot.layers()[0].content(), &original_asset);
        assert_eq!(coverage(&snapshot, mask_layer)?.value(1, 1), u16::MAX, "masks are kept");
        {
            let (state, _) =
                fixture.session.input_document_mut(document).ok_or("Document absent")?;
            let scene = state.scene.as_mut().ok_or("Scene absent")?;
            assert!(scene.document.undo()?);
            let undone = scene.document.snapshot();
            assert_ne!(
                undone.layers()[0].content(),
                &original_asset,
                "undo never restores a replaced base"
            );
            assert_eq!(coverage(&undone, mask_layer)?.uniform(), Some(0));
            assert!(scene.document.redo()?);
        }
        let (resized, _) = file("input/wide.png", &wide);
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(resized),
            vec![mask("mask", None)?],
        )])?;
        let widened = synced(&reply, "image")?;
        assert_eq!(widened.extent, Extent::new(6, 5)?);
        assert_ne!(widened.geometry, synced_surface.geometry);
        let snapshot = fixture.snapshot(document)?;
        let blank = coverage(&snapshot, mask_layer)?;
        assert_eq!((blank.extent(), blank.uniform()), (Extent::new(6, 5)?, Some(0)));
        let (state, _) = fixture.session.input_document_mut(document).ok_or("Document absent")?;
        assert!(!state.scene.as_mut().ok_or("Scene absent")?.document.undo()?);
        // A file that cannot be acquired leaves the document and reports the failure.
        let missing = directory.join("missing.png");
        let (absent, _) = file("input/missing.png", &missing);
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(absent),
            vec![mask("mask", None)?],
        )])?;
        assert!(matches!(&reply.surfaces[0], SurfaceOutcome::Failed { key, failure }
        if &**key == "image" && !failure.to_string().is_empty()));
        assert_eq!(fixture.snapshot(document)?.extent(), Extent::new(6, 5)?);
    }
    let _directory = fixture.finish()?;
    Ok(())
}
