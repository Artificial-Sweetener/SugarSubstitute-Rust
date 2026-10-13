// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose the replaceable CPU splash view from immutable presentation inputs.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryClaimError, MemoryLease};
use sugarsubstitute_contracts::presentation::FrameError;
#[path = "render_task.rs"]
mod preparation;
#[path = "render_stages.rs"]
mod stages;
pub use preparation::{
    PreparationError, SPLASH_CACHE_RESERVATION_BYTES, SplashInitializationTask,
    SplashPreparationResult, SplashRenderRequest, SplashRenderTask, TranscriptView,
};
use sugarsubstitute_contracts::localization::messages;
use tiny_skia::Pixmap;

use crate::{
    AnimationFrame, Appearance, CharacterError, CharacterLibrary, IconError, ProgressFrame,
    assets::POSE_ASSETS, caption_icons::CaptionIcons, text::FontOwner,
};

/// Preserve errors at the rendering boundary without turning them into startup policy.
#[derive(Debug)]
pub enum RenderError {
    /// An external frame size, scale or progress value violates presentation bounds.
    InvalidFrame,
    /// Resumable preparation requires an admitted persistent cache reservation.
    UnadmittedCache,
    /// CPU raster allocation failed.
    Allocation,
    /// Bounded paint coverage allocation failed, retaining its allocator cause.
    RasterAllocation(std::io::Error),
    /// Packaged character validation or rendering failed.
    Character(CharacterError),
    /// Platform font discovery or loading failed.
    Font(std::io::Error),
    /// Packaged caption glyph validation or decoding failed.
    Icon(IconError),
    /// A bounded CPU allocation failed, retaining its allocator cause.
    Buffer(std::collections::TryReserveError),
    /// The admitted memory budget could not cover a live allocation.
    Memory(MemoryClaimError),
    /// Immutable output shape or premultiplied storage validation failed.
    Frame(FrameError),
}

impl fmt::Display for RenderError {
    /// Describe the rendering operation for structured host diagnostics.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrame => {
                formatter.write_str("Invalid splash frame dimensions, scale or progress")
            }
            Self::UnadmittedCache => {
                formatter.write_str("Splash preparation requires an admitted cache owner")
            }
            Self::Allocation => formatter.write_str("Cannot allocate splash CPU frame"),
            Self::RasterAllocation(error) => {
                write!(formatter, "Cannot prepare splash coverage: {error}")
            }
            Self::Character(error) => error.fmt(formatter),
            Self::Font(error) => write!(formatter, "Cannot prepare splash status fonts: {error}"),
            Self::Icon(error) => error.fmt(formatter),
            Self::Buffer(error) => write!(formatter, "Cannot allocate splash buffer: {error}"),
            Self::Memory(error) => error.fmt(formatter),
            Self::Frame(error) => error.fmt(formatter),
        }
    }
}

impl Error for RenderError {
    /// Retain decoder failures for diagnostics.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Character(error) => Some(error),
            Self::Font(error) => Some(error),
            Self::RasterAllocation(error) => Some(error),
            Self::Icon(error) => Some(error),
            Self::Buffer(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Frame(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::shapes::AllocationError> for RenderError {
    /// Retain paint-owner allocation causes across the public render boundary.
    fn from(error: crate::shapes::AllocationError) -> Self {
        Self::RasterAllocation(std::io::Error::other(error))
    }
}

/// Own process-lifetime decoded images and text resources for this presentation only.
pub struct SplashRenderer {
    characters: CharacterLibrary,
    icons: CaptionIcons,
    text: Option<FontOwner>,
    locale: String,
    // Cache claims release after all decoded rasters and native font resources.
    cache_claim: Option<MemoryClaim>,
}

/// Keep a validated frame's composition parameters together through both startup stages.
struct RenderFrame {
    width: u32,
    height: u32,
    scale: f64,
    appearance: Appearance,
    character: AnimationFrame,
    feedback: ProgressFrame,
    status: bool,
}

impl SplashRenderer {
    /// Validate the compiled asset inventory without decoding all characters or scanning fonts.
    pub fn new(locale: &str) -> Result<Self, RenderError> {
        Ok(Self {
            characters: CharacterLibrary::new(POSE_ASSETS).map_err(RenderError::Character)?,
            icons: CaptionIcons::new(),
            text: None,
            locale: messages(locale).id.to_owned(),
            cache_claim: None,
        })
    }

    /// Validate lazy cache metadata on an admitted CPU lane and retain its full cache budget.
    ///
    /// Native font objects are free-threaded; all drawing remains serialized on execution.
    /// Idle destruction may occur on the owning caller thread. No PNG or font work occurs here.
    ///
    /// # Errors
    /// Preserve metadata and aggregate memory-claim failures before returning an owner.
    pub fn new_admitted(locale: &str, lease: MemoryLease) -> Result<Self, RenderError> {
        let claim = lease.claim(SPLASH_CACHE_RESERVATION_BYTES).map_err(RenderError::Memory)?;
        let mut renderer = Self::new(locale)?;
        renderer.cache_claim = Some(claim);
        Ok(renderer)
    }
    /// Capture initialization inputs cheaply; admitted execution establishes lazy caches.
    pub fn initialize(locale: String, lease: MemoryLease) -> SplashInitializationTask {
        SplashInitializationTask::new(locale, lease)
    }
    /// Move this exclusive renderer into a resumable preparation job without doing raster work.
    pub fn prepare<T: crate::TranscriptView>(
        self,
        request: SplashRenderRequest<T>,
        lease: MemoryLease,
    ) -> SplashRenderTask<T> {
        SplashRenderTask::new(self, request, lease)
    }

    /// Rasterize the current view in device pixels, retaining transparent native-material areas.
    ///
    /// # Errors
    /// Reject invalid/oversized input, allocation failures and invalid packaged images.
    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        scale: f64,
        appearance: Appearance,
        character: AnimationFrame,
        feedback: ProgressFrame,
    ) -> Result<Pixmap, RenderError> {
        self.render_frame(RenderFrame {
            width,
            height,
            scale,
            appearance,
            character,
            feedback,
            status: true,
        })
    }

    /// Paint the first packaged pose and controls before discovering status fonts.
    ///
    /// Use this for the initial submission, then call `render` to enrich the same view.
    /// The collapsed status remains transparent until enrichment. No font discovery or
    /// other pose decoding occurs here; native material, control and progress paint match
    /// a normal frame with the same inputs.
    ///
    /// # Errors
    /// Reject invalid/oversized input, allocation failures and invalid packaged images.
    pub fn render_bootstrap(
        &mut self,
        width: u32,
        height: u32,
        scale: f64,
        appearance: Appearance,
        feedback: ProgressFrame,
    ) -> Result<Pixmap, RenderError> {
        self.render_frame(RenderFrame {
            width,
            height,
            scale,
            appearance,
            character: AnimationFrame::bootstrap(),
            feedback,
            status: false,
        })
    }

    /// Validate and compose one frame while retaining this view's prepared resources.
    fn render_frame(&mut self, frame: RenderFrame) -> Result<Pixmap, RenderError> {
        let mut stages = stages::RenderStages::new(frame)?;
        while !stages.step(self)? {}
        stages.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::SplashRenderer;
    use crate::{AnimationFrame, Appearance, ProgressFrame};

    /// Preserve a real rejected coverage reservation through the public render error.
    #[test]
    fn coverage_allocator_cause_survives_the_render_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = crate::shapes::rounded(0.0, 0.0, 1.0, 4.0, 2.0, 1.0)
            .ok_or("Cannot construct coverage failure fixture")?;
        let failure = crate::shapes::coverage(&path, u32::MAX, u32::MAX)
            .err()
            .ok_or("Oversized coverage allocation unexpectedly succeeded")?;
        let error = super::RenderError::from(failure);
        let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&error);
        while let Some(current) = cause {
            if current.downcast_ref::<std::collections::TryReserveError>().is_some() {
                return Ok(());
            }
            cause = current.source();
        }
        Err("Render boundary discarded the coverage allocator cause".into())
    }

    /// Submit a collapsed mascot without font work, then enrich that same retained owner.
    #[test]
    fn bootstrap_defers_status_fonts_and_other_pose_decoding()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut renderer = SplashRenderer::new("en")?;
        let appearance = Appearance { expanded: false, ..Appearance::demo() };
        let feedback = ProgressFrame { completed: 0.4, activity: None };
        let bootstrap = renderer.render_bootstrap(558, 558, 1.0, appearance, feedback)?;
        assert!(renderer.text.is_none());
        assert_eq!(renderer.characters.decoded_count(), 1);
        assert!(
            (486..530)
                .all(|y| (24..534).all(|x| bootstrap.pixel(x, y).is_some_and(|p| p.alpha() == 0)))
        );
        let enriched =
            renderer.render(558, 558, 1.0, appearance, AnimationFrame::bootstrap(), feedback)?;
        assert!(renderer.text.is_some());
        assert_eq!(renderer.characters.decoded_count(), 1);
        assert!(
            (486..530)
                .any(|y| (24..534).any(|x| enriched.pixel(x, y).is_some_and(|p| p.alpha() > 0)))
        );
        for y in 0..558 {
            for x in 0..558 {
                if !(24..534).contains(&x) || !(486..530).contains(&y) {
                    assert_eq!(bootstrap.pixel(x, y), enriched.pixel(x, y));
                }
            }
        }
        Ok(())
    }

    /// Keep material pixels transparent and blank output confined to its owned rectangle.
    #[test]
    fn blank_console_preserves_native_material_and_rejects_invalid_frames()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut renderer = SplashRenderer::new("en")?;
        let frame = renderer.render(
            558,
            558,
            1.0,
            Appearance::demo(),
            AnimationFrame::bootstrap(),
            ProgressFrame { completed: 0.4, activity: None },
        )?;
        assert_eq!(frame.pixel(0, 0).map(|pixel| pixel.alpha()), Some(0));
        assert_eq!(frame.pixel(100, 400).map(|pixel| pixel.alpha()), Some(247));
        assert!(
            renderer
                .render(
                    0,
                    558,
                    1.0,
                    Appearance::demo(),
                    AnimationFrame::bootstrap(),
                    ProgressFrame { completed: 0.4, activity: None }
                )
                .is_err()
        );
        Ok(())
    }
}
