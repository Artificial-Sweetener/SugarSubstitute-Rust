// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose the replaceable CPU splash view from immutable presentation inputs.

use std::{error::Error, fmt};
use tiny_skia::Pixmap;

use crate::{
    AnimationFrame, Appearance, CharacterError, CharacterLibrary, Control, IconError,
    ProgressFrame, Theme,
    assets::POSE_ASSETS,
    caption_icons::{CaptionIcon, CaptionIcons},
    chrome,
    locale::messages,
    progress,
    text::StatusText,
};

/// Preserve errors at the rendering boundary without turning them into startup policy.
#[derive(Debug)]
pub enum RenderError {
    /// An external frame size, scale or progress value violates presentation bounds.
    InvalidFrame,
    /// CPU raster allocation failed.
    Allocation,
    /// Packaged character validation or rendering failed.
    Character(CharacterError),
    /// Platform font discovery or loading failed.
    Font(std::io::Error),
    /// Packaged caption glyph validation or decoding failed.
    Icon(IconError),
}

impl fmt::Display for RenderError {
    /// Describe the rendering operation for structured host diagnostics.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrame => {
                formatter.write_str("Invalid splash frame dimensions, scale or progress")
            }
            Self::Allocation => formatter.write_str("Cannot allocate splash CPU frame"),
            Self::Character(error) => error.fmt(formatter),
            Self::Font(error) => write!(formatter, "Cannot prepare splash status fonts: {error}"),
            Self::Icon(error) => error.fmt(formatter),
        }
    }
}

impl Error for RenderError {
    /// Retain decoder failures for diagnostics.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Character(error) => Some(error),
            Self::Font(error) => Some(error),
            Self::Icon(error) => Some(error),
            _ => None,
        }
    }
}

/// Own process-lifetime decoded images and text resources for this presentation only.
pub struct SplashRenderer {
    characters: CharacterLibrary,
    icons: CaptionIcons,
    text: Option<StatusText>,
    locale: String,
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
        })
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
        let RenderFrame { width, height, scale, appearance, character, feedback, status } = frame;
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || !scale.is_finite()
            || !(0.5..=4.0).contains(&scale)
            || !feedback.is_valid()
        {
            return Err(RenderError::InvalidFrame);
        }
        let mut target = Pixmap::new(width, height).ok_or(RenderError::Allocation)?;
        let mut bounds = appearance.mascot();
        bounds.x *= scale;
        bounds.y *= scale;
        bounds.width *= scale;
        bounds.height *= scale;
        self.characters.render(&mut target, bounds, character).map_err(RenderError::Character)?;
        if appearance.expanded {
            chrome::console(&mut target, appearance.theme, scale as f32);
        } else if status {
            if self.text.is_none() {
                self.text = Some(StatusText::new(&self.locale).map_err(RenderError::Font)?);
            }
            if let Some(text) = &mut self.text {
                text.paint(
                    &mut target,
                    messages(&self.locale).loading,
                    appearance.theme,
                    scale as f32,
                )
                .map_err(RenderError::Font)?;
            }
        }
        progress::paint(&mut target, appearance, feedback, scale as f32);
        chrome::controls(&mut target, appearance, scale as f32);
        let foreground = if appearance.theme == Theme::Dark { [255; 3] } else { [0; 3] };
        self.icons
            .draw(&mut target, scale, CaptionIcon::Console, foreground)
            .map_err(RenderError::Icon)?;
        let close_foreground = if appearance.hovered == Some(Control::Close)
            || appearance.pressed == Some(Control::Close)
        {
            [255; 3]
        } else {
            foreground
        };
        self.icons
            .draw(&mut target, scale, CaptionIcon::Close, close_foreground)
            .map_err(RenderError::Icon)?;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::SplashRenderer;
    use crate::{AnimationFrame, Appearance, ProgressFrame};

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
