// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose bounded native form scenes through the shared widget owners.
//! Application code supplies content and placement; this module owns rendering.

use crate::bitmap::Bitmap;
use crate::{
    accent::AccentPalette,
    icons::FluentIcon,
    progress::{ProgressBar, ProgressCursor, ProgressFrame},
    text::{FontOwner, GlyphPaint, LabelLayout, LabelPaintSpec, PaintSpec},
    vector::{VectorAsset, VectorPixels, VectorViewport},
    widgets::{Bounds, Decoration, DecorationCursor},
};
use std::{io, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use tiny_skia::{PixmapPaint, Transform};

mod frame;
pub use frame::{FramePaint, Layer};

/// One immutable command; native peers own input, accessibility and text editing.
pub enum Element {
    /// Shared Fluent form decoration.
    Decoration {
        /// Logical widget placement.
        bounds: Bounds,
        /// Palette and observed interaction state.
        appearance: Decoration,
    },
    /// Shaped label. User text is copied unchanged by the composition owner.
    Label {
        /// Immutable text, limited to 4096 UTF-16 units.
        text: String,
        /// Shared typography and logical placement.
        layout: LabelLayout,
    },
    /// A real bundled Fluent icon with its authored viewport.
    Icon {
        /// Logical icon viewport.
        bounds: Bounds,
        /// Authoritative icon identity.
        icon: FluentIcon,
        /// Render the glyph with the resolved surface mode's on-accent foreground.
        on_accent: bool,
    },
    /// Embedded product artwork, parsed with the shared resource restrictions.
    Artwork {
        /// Logical artwork viewport.
        bounds: Bounds,
        /// Compiled product asset.
        source: &'static str,
    },
    /// Shared progress rendering with application-owned completion.
    Progress {
        /// Logical progress placement and treatment.
        bar: ProgressBar,
        /// Observed completion/activity.
        frame: ProgressFrame,
    },
}

impl Element {
    /// Move logical placement and its explicit text clip together. The scene validates the
    /// resulting geometry before allocating or painting; this operation creates no native effects.
    pub fn translate(&mut self, x: f32, y: f32) {
        match self {
            Self::Decoration { bounds, .. }
            | Self::Icon { bounds, .. }
            | Self::Artwork { bounds, .. } => {
                bounds.x += x;
                bounds.y += y;
            }
            Self::Label { layout, .. } => {
                layout.origin[0] += x;
                layout.origin[1] += y;
                if let Some(clip) = &mut layout.clip {
                    clip[0] += x;
                    clip[2] += x;
                    clip[1] += y;
                    clip[3] += y;
                }
            }
            Self::Progress { bar, .. } => {
                bar.x += x;
                bar.y += y;
            }
        }
    }
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;

/// Retained font discovery for consecutive scenes on one preparation worker.
pub struct SceneResources {
    fonts: FontOwner,
}

impl SceneResources {
    /// Discover platform fonts outside the UI thread.
    /// # Errors
    /// Preserves locale validation and font-discovery failures.
    pub fn new(locale: &str) -> io::Result<Self> {
        Ok(Self { fonts: FontOwner::new(locale)? })
    }

    /// Measure labels using the same font owner and layout that paint the scene.
    /// Production callers pass their preparation lease; offline qualification may omit it.
    /// # Errors
    /// Preserves input, admission, font discovery and native layout failures.
    pub fn measure(
        &mut self,
        text: &str,
        spec: LabelPaintSpec,
        lease: Option<&MemoryLease>,
    ) -> io::Result<crate::text::TextSize> {
        self.fonts.measure(text, spec, lease)
    }
}

enum Active {
    Decoration(DecorationCursor),
    Label(GlyphPaint),
    Progress(ProgressCursor),
    Vector(VectorPixels),
}

/// Resumable scene composition into the caller's admitted, unpublished target.
/// Scratch requires two target-sized byte masks plus seventeen MiB for text/vector
/// output. Callers additionally admit their scene metadata and destination bytes.
pub struct SceneCursor {
    surface: PaintSpec,
    accent: AccentPalette,
    elements: std::vec::IntoIter<Element>,
    active: Option<Active>,
    lease: Option<MemoryLease>,
    _scratch: Option<MemoryClaim>,
}

impl SceneCursor {
    /// Validate scene admission and command geometry before modifying the target.
    /// Every decoration and progress element paints its accent roles from `accent`.
    /// A missing lease is reserved for synchronous off-screen qualification.
    /// # Errors
    /// Rejects excessive scenes, malformed labels and invalid widget geometry.
    pub fn new(
        surface: PaintSpec,
        accent: AccentPalette,
        elements: Vec<Element>,
        lease: Option<MemoryLease>,
    ) -> io::Result<Self> {
        Self::validate(surface, &elements)?;
        let scratch = lease
            .as_ref()
            .map(|lease| {
                lease
                    .claim(u64::from(surface.width) * u64::from(surface.height) * 2)
                    .map_err(io::Error::other)
            })
            .transpose()?;
        Ok(Self {
            surface,
            accent,
            elements: elements.into_iter(),
            active: None,
            lease,
            _scratch: scratch,
        })
    }

    /// Validate all commands before an owning multi-layer surface admits any raster work.
    fn validate(surface: PaintSpec, elements: &[Element]) -> io::Result<()> {
        if surface.width == 0
            || surface.height == 0
            || surface.width > 4096
            || surface.height > 4096
            || !surface.scale.is_finite()
            || !(0.5..=4.0).contains(&surface.scale)
            || elements.len() > 256
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid native scene"));
        }
        for element in elements {
            match element {
                Element::Label { text, layout } => {
                    LabelPaintSpec { surface, layout: *layout }.validate(text)?
                }
                Element::Decoration { bounds, .. } => {
                    bounds.validate(surface.scale).map_err(io::Error::other)?
                }
                Element::Icon { bounds: b, .. } | Element::Artwork { bounds: b, .. } => {
                    VectorViewport {
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height,
                        scale: surface.scale,
                    }
                    .validate()?;
                }
                Element::Progress { bar, frame }
                    if !bar.valid(surface.scale) || !frame.is_valid() =>
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Invalid scene progress",
                    ));
                }
                Element::Progress { .. } => {}
            }
        }
        Ok(())
    }

    /// Advance one preparation slice or one bounded vector-library operation.
    /// The target must retain its original shape for the entire cursor lifetime.
    /// # Errors
    /// Preserves font, asset, allocation and memory-admission failures.
    pub fn step(
        &mut self,
        target: &mut Bitmap,
        resources: &mut SceneResources,
    ) -> io::Result<bool> {
        if target.width() != self.surface.width || target.height() != self.surface.height {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Native scene target changed"));
        }
        if let Some(active) = &mut self.active {
            let done = match active {
                Active::Decoration(cursor) => cursor.step(target).map_err(io::Error::other)?,
                Active::Label(cursor) => {
                    cursor.step(target, NonZeroUsize::MIN.saturating_add(32767))?
                }
                Active::Progress(cursor) => cursor.step(target).map_err(io::Error::other)?,
                Active::Vector(pixels) => {
                    let [x, y] = pixels.origin();
                    target.draw_bitmap(
                        x,
                        y,
                        pixels.pixels(),
                        &PixmapPaint::default(),
                        Transform::identity(),
                        None,
                    );
                    true
                }
            };
            if done {
                self.active = None;
            }
            return Ok(false);
        }
        let Some(element) = self.elements.next() else {
            return Ok(true);
        };
        let s = self.surface;
        self.active = Some(match element {
            Element::Decoration { bounds, appearance } => Active::Decoration(
                DecorationCursor::new(
                    s.width,
                    s.height,
                    bounds,
                    appearance,
                    s.theme,
                    self.accent,
                    s.scale,
                )
                .map_err(io::Error::other)?,
            ),
            Element::Label { text, layout } => {
                let spec = LabelPaintSpec { surface: s, layout };
                Active::Label(match &self.lease {
                    Some(lease) => resources.fonts.prepare(&text, spec, lease.clone())?,
                    None => resources.fonts.prepare_offline(&text, spec)?,
                })
            }
            Element::Progress { bar, frame } => Active::Progress(
                ProgressCursor::new(s.width, s.height, bar, frame, self.accent, s.scale)
                    .map_err(io::Error::other)?,
            ),
            Element::Icon { bounds, icon, on_accent } => {
                let foreground = on_accent.then(|| {
                    crate::accent::foreground(
                        s.theme == sugarsubstitute_contracts::configuration::Theme::Dark,
                        crate::widgets::ControlState::Rest,
                    )
                });
                self.vector(bounds, icon.source(), foreground)?
            }
            Element::Artwork { bounds, source } => self.vector(bounds, source, None)?,
        });
        Ok(false)
    }

    /// Retain exact fractional artwork placement with the output memory claim.
    fn vector(
        &self,
        b: Bounds,
        source: &'static str,
        foreground: Option<[u8; 4]>,
    ) -> io::Result<Active> {
        let asset = VectorAsset::parse(source)?;
        let mut pixels = asset.rasterize(
            VectorViewport {
                x: b.x,
                y: b.y,
                width: b.width,
                height: b.height,
                scale: self.surface.scale,
            },
            self.lease.clone(),
        )?;
        if let Some(foreground) = foreground {
            pixels.recolor(foreground);
        }
        Ok(Active::Vector(pixels))
    }
}
