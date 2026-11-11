// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Represent supported panel appearance without carrying web or window handles.

use super::{PanelError, wire};

/// The shared SugarCubes presentation styles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelStyle {
    /// ComfyUI's presentation.
    Comfy,
    /// Fluent-style presentation.
    Fluent,
    /// Spectrum-style presentation.
    Spectrum,
    /// PhotoPro-style presentation.
    PhotoPro,
    /// GIMP-style presentation.
    Gimp,
}

impl PanelStyle {
    /// Select the exact schema spelling for a known style.
    fn wire(self) -> &'static str {
        match self {
            Self::Comfy => "comfy",
            Self::Fluent => "fluent",
            Self::Spectrum => "spectrum",
            Self::PhotoPro => "photopro",
            Self::Gimp => "gimp",
        }
    }

    /// Translate a schema-valid style into its owned variant.
    fn parse(value: &str) -> Result<Self, PanelError> {
        match value {
            "comfy" => Ok(Self::Comfy),
            "fluent" => Ok(Self::Fluent),
            "spectrum" => Ok(Self::Spectrum),
            "photopro" => Ok(Self::PhotoPro),
            "gimp" => Ok(Self::Gimp),
            _ => Err(PanelError::Invalid),
        }
    }
}

/// PhotoPro palette selection retained even when another style is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelPalette {
    /// Black palette.
    Black,
    /// Dark gray palette.
    DarkGray,
    /// Medium gray palette.
    MediumGray,
    /// Light gray palette.
    LightGray,
}

impl PanelPalette {
    /// Select the exact schema spelling for a known palette.
    fn wire(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::DarkGray => "dark-gray",
            Self::MediumGray => "medium-gray",
            Self::LightGray => "light-gray",
        }
    }

    /// Translate a schema-valid palette into its owned variant.
    fn parse(value: &str) -> Result<Self, PanelError> {
        match value {
            "black" => Ok(Self::Black),
            "dark-gray" => Ok(Self::DarkGray),
            "medium-gray" => Ok(Self::MediumGray),
            "light-gray" => Ok(Self::LightGray),
            _ => Err(PanelError::Invalid),
        }
    }
}

/// Validated presentation values that confer no native privileges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanelPresentation {
    plugin_mode: bool,
    style: PanelStyle,
    palette: PanelPalette,
    accent_color: Option<String>,
    resize_corner_size: u8,
}

impl PanelPresentation {
    /// Validate the shared color and resize bounds before retaining presentation.
    ///
    /// `accent_color` is either absent or an RGB string such as `#aF0123`.
    /// `resize_corner_size` is measured in CSS pixels.
    ///
    /// # Errors
    /// Rejects a color, style or size outside the generated shared schema.
    pub fn new(
        plugin_mode: bool,
        style: PanelStyle,
        palette: PanelPalette,
        accent_color: Option<String>,
        resize_corner_size: u8,
    ) -> Result<Self, PanelError> {
        let mut result = Self { plugin_mode, style, palette, accent_color, resize_corner_size };
        if !result.wire().valid() {
            return Err(PanelError::Invalid);
        }
        result.accent_color = result.accent_color.map(|value| value.into_boxed_str().into_string());
        Ok(result)
    }

    /// Return whether embedded-plugin presentation is requested.
    pub const fn plugin_mode(&self) -> bool {
        self.plugin_mode
    }

    /// Return the selected SugarCubes style.
    pub const fn style(&self) -> PanelStyle {
        self.style
    }

    /// Return the retained PhotoPro palette.
    pub const fn palette(&self) -> PanelPalette {
        self.palette
    }

    /// Borrow the optional RGB accent string.
    pub fn accent_color(&self) -> Option<&str> {
        self.accent_color.as_deref()
    }

    /// Return the resize corner extent in CSS pixels.
    pub const fn resize_corner_size(&self) -> u8 {
        self.resize_corner_size
    }

    /// Form the generated value without carrying any host authority.
    pub(super) fn wire(&self) -> wire::PanelPresentation {
        wire::PanelPresentation {
            plugin_mode: self.plugin_mode,
            visual_style: self.style.wire().into(),
            photo_pro_palette: self.palette.wire().into(),
            accent_color: self.accent_color.clone(),
            resize_corner_size: u64::from(self.resize_corner_size),
        }
    }

    /// Retain only a fully schema-valid generated presentation.
    pub(super) fn from_wire(value: wire::PanelPresentation) -> Result<Self, PanelError> {
        if !value.valid() {
            return Err(PanelError::Invalid);
        }
        Self::new(
            value.plugin_mode,
            PanelStyle::parse(&value.visual_style)?,
            PanelPalette::parse(&value.photo_pro_palette)?,
            value.accent_color,
            u8::try_from(value.resize_corner_size).map_err(|_| PanelError::Invalid)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_color_does_not_retain_unrelated_caller_capacity() -> Result<(), PanelError> {
        let mut value = String::with_capacity(8192);
        value.push_str("#aF0123");
        let presentation =
            PanelPresentation::new(false, PanelStyle::Comfy, PanelPalette::Black, Some(value), 0)?;
        let color = presentation.accent_color.as_ref().ok_or(PanelError::Invalid)?;
        assert_eq!(color.capacity(), color.len());
        Ok(())
    }
}
