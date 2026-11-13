// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bring a loaded input document in line with its surface's synced structure.
use super::*;
use sugarsubstitute_canvas::{LayerContent, Provision, ToolCancelReason};

/// The source and original a replaced base held, disposed of on the retirement worker.
pub(super) type Replaced = (Option<work::Source>, Option<ImageSource>);

/// A rebase that did not complete, with the source and original the document does not keep:
/// the new ones when the document stayed on its old base, else the replaced ones.
pub(super) struct RebaseFailure {
    pub(super) cause: Box<dyn Error>,
    pub(super) unadopted: Replaced,
}

/// A base image or blank backing about to replace a document's base layer.
pub(super) enum NewBase {
    Raster { source: work::Source, original: ImageSource },
    Blank(Extent),
}

impl Surface {
    /// Provision the document so its base name and its masks (presence, names, order and any
    /// newly synced appearance) match this surface. New masks start uncovered.
    pub(super) fn reconcile(
        &mut self,
        document: &mut document::DocumentState,
        memory: &MemoryLease,
    ) -> Result<(), Box<dyn Error>> {
        let Some(scene) = document.scene.as_mut() else {
            return Ok(());
        };
        let snapshot = scene.document.snapshot();
        let extent = snapshot.extent();
        let base = LayerId::new(BASE_LAYER)?;
        let mut changes = Vec::new();
        if snapshot.layers().iter().any(|layer| layer.id() == base && layer.name() != &*self.label)
        {
            changes.push(Provision::Rename { layer: base, name: self.label.clone() });
        }
        for layer in snapshot.layers() {
            if layer.id() != base && !self.masks.iter().any(|mask| mask.layer == layer.id()) {
                changes.push(Provision::Remove(layer.id()));
            }
        }
        for (index, mask) in self.masks.iter_mut().enumerate() {
            match snapshot.layers().iter().find(|layer| layer.id() == mask.layer) {
                None => {
                    let coverage = build::blank_mask(extent, memory)?;
                    let content = LayerContent::Coverage { coverage, appearance: mask.appearance };
                    let layer = Layer::new(mask.layer, content)?.with_name(&mask.label)?;
                    changes.push(Provision::Insert { index: index + 1, layer });
                }
                Some(layer) => {
                    if layer.name() != &*mask.label {
                        changes.push(Provision::Rename {
                            layer: mask.layer,
                            name: mask.label.clone(),
                        });
                    }
                    if mask.appearance_pending {
                        changes.push(Provision::Appearance {
                            layer: mask.layer,
                            appearance: mask.appearance,
                        });
                    }
                }
            }
            mask.appearance_pending = false;
            changes.push(Provision::Reorder { layer: mask.layer, index: index + 1 });
        }
        scene.document.provision(&changes)?;
        if document
            .tools
            .target()
            .is_some_and(|target| !self.masks.iter().any(|mask| mask.layer == target))
        {
            document.tools.set_target(None);
        }
        Ok(())
    }

    /// Replace the document's base. At the same extent the masks are kept; at another extent
    /// every mask becomes uncovered, as a mask of the wrong size cannot be kept. Returns the
    /// replaced source and original for worker disposal. A content change the memory budget
    /// cannot hold leaves the document on its old base and returns the new one for disposal.
    pub(super) fn rebase(
        &self,
        document: &mut document::DocumentState,
        base: NewBase,
        memory: &MemoryLease,
        now: Duration,
    ) -> Result<Replaced, Box<RebaseFailure>> {
        let (layer, extent, source, original) = match base {
            NewBase::Raster { source, original } => {
                let descriptor = source.descriptor();
                let extent = descriptor.layout().extent();
                let layer = build::raster_base(descriptor.asset(), extent, &self.label);
                (layer, extent, Some(source), Some(original))
            }
            NewBase::Blank(extent) => (build::blank_base(extent, &self.label), extent, None, None),
        };
        let staged = layer
            .map_err(Box::<dyn Error>::from)
            .and_then(|layer| self.replace_base(document, layer, extent, memory));
        let (resized, scene) = match (staged, document.scene.as_mut()) {
            (Ok(resized), Some(scene)) => (resized, scene),
            (staged, _) => {
                let cause = staged.err().unwrap_or_else(|| "Input document is unavailable".into());
                return Err(Box::new(RebaseFailure { cause, unadopted: (source, original) }));
            }
        };
        let replaced = (
            std::mem::replace(&mut scene.source, source),
            std::mem::replace(&mut scene.original, original),
        );
        if resized {
            let content = document.scene.as_ref().map(|scene| &scene.document);
            if let Err(cause) = document.tools.set_content(Some(extent), content, now) {
                return Err(Box::new(RebaseFailure { cause: cause.into(), unadopted: replaced }));
            }
        }
        Ok(replaced)
    }

    /// Put `layer` in place of the base layer of `document`'s content at `extent`, clearing
    /// every mask when the extent changes. Returns whether it changed.
    fn replace_base(
        &self,
        document: &mut document::DocumentState,
        layer: Layer,
        extent: Extent,
        memory: &MemoryLease,
    ) -> Result<bool, Box<dyn Error>> {
        let scene = document.scene.as_mut().ok_or("Input document is unavailable")?;
        let snapshot = scene.document.snapshot();
        let resized = extent != snapshot.extent();
        if resized {
            let mut layers = vec![layer];
            for existing in snapshot.layers() {
                if let LayerContent::Coverage { appearance, .. } = existing.content() {
                    let coverage = build::blank_mask(extent, memory)?;
                    let content = LayerContent::Coverage { coverage, appearance: *appearance };
                    layers.push(existing.clone().with_content(content));
                }
            }
            scene.document.reload(extent, &layers)?;
        } else {
            scene.document.provision(&[Provision::Content {
                layer: layer.id(),
                content: layer.content().clone(),
            }])?;
        }
        Ok(resized)
    }
}

/// Cancel a document's gestures before its custody moves.
pub(super) fn settle_tools(
    document: &mut document::DocumentState,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    let scene = document.scene.as_ref().map(|scene| &scene.document);
    document.tools.cancel(ToolCancelReason::DocumentChanged, scene, now)?;
    Ok(())
}
