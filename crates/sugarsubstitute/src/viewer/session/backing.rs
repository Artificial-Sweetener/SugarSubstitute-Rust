// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select retained primary and underlay backings as one complete display source.

use super::*;
use sugarsubstitute_canvas::ViewProjection;

/// One actual retained backing and its mapping into the requested output camera.
#[derive(Clone, Copy)]
pub(super) struct SelectedLayer {
    pub(super) shown: Presented,
    pub(super) source: FrameStamp,
    pub(super) projection: ViewProjection,
}

/// A complete display source, optionally preserving overlapping detail above the fallback.
#[derive(Clone, Copy)]
pub(super) struct BackingSelection {
    pub(super) primary: SelectedLayer,
    pub(super) underlay: Option<SelectedLayer>,
}

impl DisplayEvidence {
    /// Prove that the accepted pixels fill all visible document samples with exact source cameras.
    pub(super) fn proves_complete(
        self,
        document: sugarsubstitute_image_ops::Extent,
    ) -> Result<bool, Box<dyn Error>> {
        if self.target.surface != self.source.surface
            || self.target.generation <= self.source.generation
            || self.projection.source() != self.coverage.viewport()
            || self.projection.target() != self.view
        {
            return Ok(false);
        }
        let primary_crop = self.coverage.crop(self.view, document)?;
        let Some(underlay) = self.underlay else {
            return Ok(primary_crop == Some(self.projection));
        };
        Ok(self.tier == BackingTier::Detail
            && underlay.source.surface == self.source.surface
            && underlay.source != self.source
            && self.target.generation > underlay.source.generation
            && underlay.projection.source() == underlay.coverage.viewport()
            && underlay.projection.target() == self.view
            && underlay.coverage.crop(self.view, document)? == Some(underlay.projection)
            && self.coverage.overlaps(self.view, document)?)
    }
}

/// Prefer retained detail whenever it covers any visible document pixels.
pub(super) fn choose_backing(
    detail: Option<(Presented, FrameStamp)>,
    fallback: Option<(Presented, FrameStamp)>,
    target: Viewport,
    document: sugarsubstitute_image_ops::Extent,
) -> Result<Option<BackingSelection>, Box<dyn Error>> {
    let fallback = fallback
        .map(|(shown, source)| {
            Ok::<_, Box<dyn Error>>(
                shown.coverage.crop(target, document)?.map(|projection| SelectedLayer {
                    shown,
                    source,
                    projection,
                }),
            )
        })
        .transpose()?
        .flatten();
    let Some((detail, source)) = detail else {
        return Ok(fallback.map(|primary| BackingSelection { primary, underlay: None }));
    };
    if let Some(projection) = detail.coverage.crop(target, document)? {
        return Ok(Some(BackingSelection {
            primary: SelectedLayer { shown: detail, source, projection },
            underlay: None,
        }));
    }
    if detail.coverage.overlaps(target, document)?
        && let Some(underlay) = fallback
    {
        return Ok(Some(BackingSelection {
            primary: SelectedLayer {
                shown: detail,
                source,
                projection: ViewProjection::new(detail.coverage.viewport(), target)?,
            },
            underlay: Some(underlay),
        }));
    }
    Ok(fallback.map(|primary| BackingSelection { primary, underlay: None }))
}

impl Session {
    /// Compare every sampled backing identity before reusing an acknowledged display.
    pub(super) fn display_selection_pending(&self, selection: BackingSelection) -> bool {
        self.expose_pending
            || self.display_source != Some(selection.primary.source)
            || self.display_view != Some(selection.primary.projection.target())
            || !self.display_evidence.is_some_and(|evidence| {
                evidence.underlay.map(|underlay| underlay.source)
                    == selection.underlay.map(|underlay| underlay.source)
            })
    }

    /// Select the best current backing without confusing partial detail with complete coverage.
    pub(super) fn select_backing(&self) -> Result<Option<BackingSelection>, Box<dyn Error>> {
        let Some(scene) = &self.scene else {
            return Ok(None);
        };
        let snapshot = scene.document.snapshot();
        let document = (snapshot.identity(), snapshot.revision());
        let target = self.tools.navigation().viewport();
        let extent = snapshot.extent();
        let layer = |shown: &Presented| -> Option<(Presented, FrameStamp)> {
            (shown.document == document && shown.source_revision == scene.source_revision)
                .then(|| self.backing_stamp(shown.tier).map(|source| (*shown, source)))
                .flatten()
        };
        choose_backing(
            self.presented.as_ref().and_then(layer),
            self.fallback_presented.as_ref().and_then(layer),
            target,
            extent,
        )
    }

    fn backing_stamp(&self, tier: BackingTier) -> Option<FrameStamp> {
        if self.options.gpu.is_some() {
            match tier {
                BackingTier::Fallback => self.fallback_gpu_front.as_ref(),
                BackingTier::Detail => self.gpu_front.as_ref(),
            }
            .and_then(GpuSurface::current)
        } else {
            match tier {
                BackingTier::Fallback => self.fallback_front.as_ref(),
                BackingTier::Detail => self.front.as_ref(),
            }
            .and_then(|surface| surface.current())
        }
    }
}
