// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build and change input document content on admitted workers.
//!
//! These functions run only inside owned jobs. They acquire ComfyUI files through inert host
//! adapters, import them through the shared image cache and charge retained content to the
//! canvas's retained lease; scratch buffers are charged to the job.
use super::*;
use sugarsubstitute_canvas::{Document, LayerContent};
use sugarsubstitute_image_ops::{Coverage, Pixel, TILE_EDGE, VectorRectangle, coverage_from_u8};

/// Channel value of the blank backing, RGB(24,24,24) in sRGB.
const BLANK_BACKING: u8 = 24;

/// One input job's result.
pub(in crate::native_canvas::session) enum Product {
    /// A new document holding only its base layer.
    Created(Scene),
    /// A replacement base image.
    Rebased {
        /// Shared reader of the imported image.
        source: work::Source,
        /// The original it was imported from.
        original: ImageSource,
    },
    /// A decoded mask file.
    Imported(MaskImport),
    /// Every mask's coverage at the new extent.
    Resized(Vec<(LayerId, Coverage)>),
}

/// A mask file read through its loader channel.
pub(in crate::native_canvas::session) enum MaskImport {
    /// Coverage at the expected extent.
    Loaded(Coverage),
    /// The file's extent, which differs from the expected one.
    Mismatch(Extent),
}

/// The base a new document starts from.
pub(super) enum CreateBase {
    File(Arc<dyn acquisition::SourceAcquirer>),
    Blank(Extent),
}

impl MaskChannel {
    /// Coverage value of one straight RGBA sample, as `LoadImageMask` reads it.
    pub(in crate::native_canvas::session) const fn read(
        self,
        [red, green, blue, alpha]: [u8; 4],
    ) -> u8 {
        match self {
            Self::Red => red,
            Self::Green => green,
            Self::Blue => blue,
            Self::Alpha => 255 - alpha,
        }
    }
}

/// The locked base layer showing an imported image.
pub(super) fn raster_base(
    asset: sugarsubstitute_image_ops::AssetId,
    extent: Extent,
    label: &str,
) -> Result<Layer, WorkError> {
    Ok(Layer::new(LayerId::new(BASE_LAYER)?, LayerContent::Raster { asset, extent })?
        .with_name(label)?
        .with_locked(true))
}

/// The locked RGB(24,24,24) base layer of a mask-only surface.
pub(super) fn blank_base(extent: Extent, label: &str) -> Result<Layer, WorkError> {
    let value = f32::from(BLANK_BACKING) / 255.0;
    let linear = ((value + 0.055) / 1.055).powf(2.4);
    let bounds = [0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())];
    let shape = VectorRectangle::new(bounds, Pixel::linear(linear, linear, linear, 1.0)?)?;
    Ok(Layer::new(LayerId::new(BASE_LAYER)?, LayerContent::VectorRectangle(shape))?
        .with_name(label)?
        .with_locked(true))
}

/// Acquire one input file; inputs never carry comparisons or overview cells.
fn acquire(
    acquirer: &dyn acquisition::SourceAcquirer,
    resources: &Resources,
    context: &JobContext,
) -> Result<ImageSource, WorkError> {
    let originals = acquirer.acquire(resources.cache.clone(), &resources.memory, context)?;
    if originals.compared.is_some() || originals.overview.is_some() {
        return Err(WorkError::invalid("An input file is a single image"));
    }
    Ok(originals.primary)
}

/// Acquire and import one image through the shared cache.
fn import(
    acquirer: &dyn acquisition::SourceAcquirer,
    resources: &Resources,
    context: &JobContext,
) -> Result<(sugarsubstitute_image_io::CachedRaster, ImageSource), WorkError> {
    let original = acquire(acquirer, resources, context)?;
    let now = sugarsubstitute_supervision::cache::utc_seconds()?;
    let raster = original.import_image(
        resources.cache.clone(),
        CacheReuse::Compatible,
        &resources.memory,
        now,
        context,
    )?;
    Ok((raster, original))
}

/// Create a document holding only its locked base layer; masks are provisioned afterwards.
pub(super) fn create(
    identity: InputDocumentId,
    label: &str,
    base: CreateBase,
    resources: &Resources,
    context: &JobContext,
) -> Result<Product, WorkError> {
    let (layer, extent, source, original) = match base {
        CreateBase::File(acquirer) => {
            let (source, original) = rebase(acquirer.as_ref(), resources, context)?;
            let descriptor = source.descriptor();
            let extent = descriptor.layout().extent();
            (raster_base(descriptor.asset(), extent, label)?, extent, Some(source), Some(original))
        }
        CreateBase::Blank(extent) => (blank_base(extent, label)?, extent, None, None),
    };
    let mut document = Document::new(identity.get(), extent, &resources.memory)?;
    document.reload(extent, &[layer])?;
    Ok(Product::Created(Scene {
        document,
        source,
        source_revision: 0,
        original,
        compared: None,
        placed: Vec::new(),
    }))
}

/// Acquire and import a base image.
pub(super) fn rebase(
    acquirer: &dyn acquisition::SourceAcquirer,
    resources: &Resources,
    context: &JobContext,
) -> Result<(work::Source, ImageSource), WorkError> {
    let (raster, original) = import(acquirer, resources, context)?;
    Ok((sugarsubstitute_image_io::SharedRaster::new(raster, &resources.memory)?, original))
}

/// Read a mask file through `channel` when its extent is `expected`.
pub(super) fn import_mask(
    acquirer: &dyn acquisition::SourceAcquirer,
    channel: MaskChannel,
    expected: Extent,
    resources: &Resources,
    context: &JobContext,
) -> Result<MaskImport, WorkError> {
    let (mut raster, _original) = import(acquirer, resources, context)?;
    let extent = raster.descriptor().layout().extent();
    if extent != expected {
        return Ok(MaskImport::Mismatch(extent));
    }
    let tile = (TILE_EDGE * TILE_EDGE) as usize;
    let _scratch = context.memory().claim((tile * 6) as u64 + 128)?;
    let mut samples = vec![[0_u8; 4]; tile];
    let mut values = vec![0_u16; tile];
    let mut coverage = Coverage::new(extent, 0, &resources.memory)?;
    for part in extent.region().tiles() {
        let count = part.pixels() as usize;
        raster.read_samples8(part, &mut samples[..count], context)?;
        for (value, sample) in values[..count].iter_mut().zip(&samples[..count]) {
            *value = coverage_from_u8(channel.read(*sample));
        }
        if values[..count].iter().any(|value| *value != 0) {
            coverage.write(part, &values[..count], &resources.memory)?;
        }
    }
    Ok(MaskImport::Loaded(coverage))
}

/// Every mask's coverage at `extent`, placed by the anchor or resampled.
pub(super) fn resize(
    masks: Vec<(LayerId, Coverage)>,
    from: Extent,
    extent: Extent,
    mode: ResizeMode,
    resources: &Resources,
    context: &JobContext,
) -> Result<Product, WorkError> {
    let offset = match mode {
        ResizeMode::Canvas(anchor) => Some(anchor_offset(from, extent, anchor)),
        ResizeMode::Scale(_) => None,
    };
    let mut resized = Vec::with_capacity(masks.len());
    for (layer, coverage) in masks {
        let coverage = match (mode, offset) {
            (ResizeMode::Scale(resampling), _) => {
                coverage.resample(extent, resampling, &resources.memory, context)?
            }
            (ResizeMode::Canvas(_), Some(offset)) => {
                coverage.reframe(extent, offset, &resources.memory, context)?
            }
            (ResizeMode::Canvas(_), None) => {
                return Err(WorkError::invalid("Canvas resize lost its anchor"));
            }
        };
        resized.push((layer, coverage));
    }
    Ok(Product::Resized(resized))
}

/// Where a canvas of `from` lands inside `to` when `anchor` stays fixed, in target pixels.
/// Odd differences around a center put the extra pixel after the content.
fn anchor_offset(from: Extent, to: Extent, anchor: ResizeAnchor) -> [i64; 2] {
    let halves = anchor.halves();
    let difference = [
        i64::from(to.width()) - i64::from(from.width()),
        i64::from(to.height()) - i64::from(from.height()),
    ];
    [0, 1].map(|axis| (difference[axis] * halves[axis]).div_euclid(2))
}

/// Uncovered coverage at `extent`, the content of every new mask.
pub(super) fn blank_mask(extent: Extent, memory: &MemoryLease) -> Result<Coverage, WorkError> {
    Ok(Coverage::new(extent, 0, memory)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_anchor_keeps_its_point_of_the_canvas_fixed() -> Result<(), Box<dyn Error>> {
        let (from, to) = (Extent::new(4, 4)?, Extent::new(7, 2)?);
        let expected = [
            (ResizeAnchor::TopLeft, [0, 0]),
            (ResizeAnchor::Top, [1, 0]),
            (ResizeAnchor::TopRight, [3, 0]),
            (ResizeAnchor::Left, [0, -1]),
            (ResizeAnchor::Center, [1, -1]),
            (ResizeAnchor::Right, [3, -1]),
            (ResizeAnchor::BottomLeft, [0, -2]),
            (ResizeAnchor::Bottom, [1, -2]),
            (ResizeAnchor::BottomRight, [3, -2]),
        ];
        for (anchor, offset) in expected {
            assert_eq!(anchor_offset(from, to, anchor), offset, "{anchor:?}");
        }
        Ok(())
    }
}
