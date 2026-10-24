// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare clipped native surfaces with bounded work and retained pixel custody.
use super::*;
use sugarsubstitute_contracts::presentation::{
    FrameIdentity, FrameShape, FrameValidator, PreparedFrame,
};
mod image;
use image::Image;
const METADATA_BYTES: u64 = 512 * 1024;
const PIXELS_PER_STEP: usize = 32768;

/// One clipped scene in a native surface. Elements are relative to the clip's logical origin.
pub struct Layer {
    /// Whole-window logical viewport. Content outside it cannot affect another part of the window.
    pub viewport: Bounds,
    /// Ordered shared UI commands; native input peers are retained by the desktop separately.
    pub elements: Vec<Element>,
}

enum Stage {
    Output,
    Next,
    Clear,
    Paint,
    Composite,
    Validate,
    Done,
}

/// Incremental native surface painting, driven by the execution owner outside the UI thread.
/// Each step initializes/composites at most 32768 pixels, advances one shared paint cursor,
/// or executes one bounded font/vector operation. Dropping the painter cancels unpublished work.
pub struct FramePaint {
    surface: PaintSpec,
    shape: FrameShape,
    identity: FrameIdentity,
    layers: std::vec::IntoIter<Layer>,
    cursor: Option<SceneCursor>,
    output: Option<Image>,
    layer: Option<Image>,
    validator: Option<FrameValidator>,
    clip: [f32; 4],
    position: usize,
    stage: Stage,
    lease: MemoryLease,
    _metadata: MemoryClaim,
}

impl FramePaint {
    /// Output, reusable layer, two coverage masks, bounded font/vector scratch and command metadata.
    /// The caller additionally reserves native presentation copies and its own application snapshot.
    /// Validator handoff is charged twice without copying and fits the same two-image allowance.
    pub fn reservation_bytes(shape: FrameShape) -> u64 {
        shape.byte_len() as u64 * 5 / 2 + 17 * 1024 * 1024 + METADATA_BYTES
    }

    /// Adopt admitted command storage and validate every layer before any raster work.
    /// The caller must retain its original command admission until this owner acquires custody.
    /// # Errors
    /// Rejects excessive metadata, invalid layers/commands or insufficient aggregate memory credit.
    pub fn new(
        surface: PaintSpec,
        identity: FrameIdentity,
        mut layers: Vec<Layer>,
        lease: MemoryLease,
    ) -> io::Result<Self> {
        let shape = FrameShape::new(surface.width, surface.height, surface.scale.into())
            .map_err(io::Error::other)?;
        if layers.len() > 8 || !(0.5..=4.0).contains(&surface.scale) {
            return Err(io::Error::other("Invalid native surface layers"));
        }
        let metadata = lease.claim(METADATA_BYTES).map_err(io::Error::other)?;
        let mut bytes = layers.capacity().checked_mul(size_of::<Layer>()).ok_or_else(invalid)?;
        let mut commands = 0;
        for layer in &mut layers {
            let b = layer.viewport;
            b.validate(surface.scale).map_err(io::Error::other)?;
            if b.x < 0.0
                || b.y < 0.0
                || (b.x + b.width) * surface.scale > surface.width as f32 + 0.001
                || (b.y + b.height) * surface.scale > surface.height as f32 + 0.001
            {
                return Err(invalid());
            }
            bytes = bytes
                .checked_add(
                    layer
                        .elements
                        .capacity()
                        .checked_mul(size_of::<Element>())
                        .ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?;
            commands += layer.elements.len();
            for element in &mut layer.elements {
                if let Element::Label { text, .. } = element {
                    bytes = bytes.checked_add(text.capacity()).ok_or_else(invalid)?;
                }
                element.translate(b.x, b.y);
            }
            SceneCursor::validate(surface, &layer.elements)?;
        }
        if commands > 256 || bytes as u64 > METADATA_BYTES {
            return Err(invalid());
        }
        let output = Image::new(shape, &lease)?;
        let layer = Image::new(shape, &lease)?;
        Ok(Self {
            surface,
            shape,
            identity,
            layers: layers.into_iter(),
            cursor: None,
            output: Some(output),
            layer: Some(layer),
            validator: None,
            clip: [0.0; 4],
            position: 0,
            stage: Stage::Output,
            lease,
            _metadata: metadata,
        })
    }

    /// Advance one bounded preparation step. The retained font owner must use this surface's locale.
    /// # Errors
    /// Preserves font, vector, allocation and alpha-validation causes. A completed painter is inert.
    pub fn step(&mut self, resources: &mut SceneResources) -> io::Result<bool> {
        match self.stage {
            Stage::Output => {
                if self.output.as_mut().ok_or_else(invalid)?.clear()? {
                    self.stage = Stage::Next;
                }
            }
            Stage::Next => {
                if let Some(layer) = self.layers.next() {
                    let b = layer.viewport;
                    self.clip = [b.x, b.y, b.x + b.width, b.y + b.height]
                        .map(|value| value * self.surface.scale);
                    self.cursor = Some(SceneCursor::new(
                        self.surface,
                        layer.elements,
                        Some(self.lease.clone()),
                    )?);
                    self.layer.as_mut().ok_or_else(invalid)?.begin_clear();
                    self.stage = Stage::Clear;
                } else {
                    self.layer = None;
                    let (pixels, claim) = self.output.take().ok_or_else(invalid)?.finish()?;
                    self.validator = Some(
                        FrameValidator::new(self.shape, self.identity, pixels, self.lease.clone())
                            .map_err(io::Error::other)?,
                    );
                    drop(claim);
                    self.stage = Stage::Validate;
                }
            }
            Stage::Clear => {
                if self.layer.as_mut().ok_or_else(invalid)?.clear()? {
                    self.stage = Stage::Paint;
                }
            }
            Stage::Paint => {
                if self
                    .cursor
                    .as_mut()
                    .ok_or_else(invalid)?
                    .step(self.layer.as_mut().ok_or_else(invalid)?.pixels_mut()?, resources)?
                {
                    self.cursor = None;
                    self.position = 0;
                    self.stage = Stage::Composite;
                }
            }
            Stage::Composite => self.composite()?,
            Stage::Validate => {
                if self
                    .validator
                    .as_mut()
                    .ok_or_else(invalid)?
                    .validate_next(NonZeroUsize::MIN.saturating_add(PIXELS_PER_STEP - 1))
                    .map_err(io::Error::other)?
                {
                    self.stage = Stage::Done;
                    return Ok(true);
                }
            }
            Stage::Done => return Ok(true),
        }
        Ok(false)
    }

    /// Transfer only fully validated pixels; no copy, font work or scan occurs at publication.
    /// # Errors
    /// Rejects early publication or missing terminal storage.
    pub fn finish(mut self) -> io::Result<PreparedFrame> {
        if !matches!(self.stage, Stage::Done) {
            return Err(invalid());
        }
        self.validator.take().ok_or_else(invalid)?.finish().map_err(io::Error::other)
    }

    /// Clip premultiplied content without unpremultiplication or changes to fractional placement.
    fn composite(&mut self) -> io::Result<()> {
        let source = self.layer.as_mut().ok_or_else(invalid)?.pixels_mut()?.data();
        let output = self.output.as_mut().ok_or_else(invalid)?.pixels_mut()?.data_mut();
        let end = (self.position + PIXELS_PER_STEP).min(self.shape.byte_len() / 4);
        let [left, top, right, bottom] = self.clip;
        for index in self.position..end {
            let x = (index % self.surface.width as usize) as f32;
            let y = (index / self.surface.width as usize) as f32;
            let coverage = ((x + 1.0).min(right) - x.max(left)).clamp(0.0, 1.0)
                * ((y + 1.0).min(bottom) - y.max(top)).clamp(0.0, 1.0);
            let coverage = (coverage * 255.0).round() as u8;
            if coverage == 0 {
                continue;
            }
            let start = index * 4;
            let source = &source[start..start + 4];
            let destination = &mut output[start..start + 4];
            let inverse = 255 - crate::shapes::byte_product(source[3], coverage);
            for channel in 0..4 {
                destination[channel] = crate::shapes::byte_product(source[channel], coverage)
                    .saturating_add(crate::shapes::byte_product(destination[channel], inverse));
            }
        }
        self.position = end;
        if end == self.shape.byte_len() / 4 {
            self.stage = Stage::Next;
        }
        Ok(())
    }
}

/// Private invariant failures cannot publish partially prepared window contents.
fn invalid() -> io::Error {
    io::Error::other("Invalid native surface preparation")
}

#[cfg(test)]
mod tests;
