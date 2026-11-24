// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Icons shared with the page: one SVG file each in the monorepo's UI element record, which the
//! page's own controls draw too, so a native and a web copy of an element show the same artwork.

/// A shared icon, named by what activating it does: a toggle shows where it goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedIcon {
    /// Open the ComfyUI console: the console window with its bar solid.
    ConsoleOpen,
    /// Close the ComfyUI console: the console window with its bar hollow.
    ConsoleClose,
    /// Show the output canvas: the canvas frame with its side bar solid.
    CanvasOpen,
    /// Hide the output canvas: the canvas frame with its side bar hollow.
    CanvasClose,
    /// Close the window: the caption close cross.
    WindowClose,
}

impl SharedIcon {
    /// Borrow the embedded SVG source.
    pub const fn source(self) -> &'static str {
        match self {
            Self::ConsoleOpen => {
                include_str!("../../../../Sugar/integrations/contracts/ui/icons/console-open.svg")
            }
            Self::ConsoleClose => {
                include_str!("../../../../Sugar/integrations/contracts/ui/icons/console-close.svg")
            }
            Self::CanvasOpen => {
                include_str!("../../../../Sugar/integrations/contracts/ui/icons/canvas-open.svg")
            }
            Self::CanvasClose => {
                include_str!("../../../../Sugar/integrations/contracts/ui/icons/canvas-close.svg")
            }
            Self::WindowClose => {
                include_str!("../../../../Sugar/integrations/contracts/ui/icons/window-close.svg")
            }
        }
    }

    /// The console toggle's icon: where activating it goes from the console's current state.
    pub const fn console_toggle(open: bool) -> Self {
        if open { Self::ConsoleClose } else { Self::ConsoleOpen }
    }
}

#[cfg(test)]
mod tests {
    use super::SharedIcon;
    use crate::vector::{VectorAsset, VectorViewport};

    const ALL: [SharedIcon; 5] = [
        SharedIcon::ConsoleOpen,
        SharedIcon::ConsoleClose,
        SharedIcon::CanvasOpen,
        SharedIcon::CanvasClose,
        SharedIcon::WindowClose,
    ];

    /// Every shared icon renders through the native vector renderer at caption glyph sizes.
    #[test]
    fn shared_icons_render_at_caption_scales() -> Result<(), Box<dyn std::error::Error>> {
        for icon in ALL {
            let asset = VectorAsset::parse(icon.source())?;
            for scale in [1.0_f32, 1.25, 1.5, 2.0, 3.0, 4.0] {
                let pixels = asset.rasterize(
                    VectorViewport { x: 0.0, y: 0.0, width: 16.0, height: 16.0, scale },
                    None,
                )?;
                assert!(pixels.pixels().data().chunks_exact(4).any(|p| p[3] > 0), "{icon:?}");
            }
        }
        Ok(())
    }

    /// The console toggle shows where it goes: closed shows the solid bar, open the hollow one.
    #[test]
    fn the_console_toggle_shows_its_destination() {
        assert_eq!(SharedIcon::console_toggle(false), SharedIcon::ConsoleOpen);
        assert_eq!(SharedIcon::console_toggle(true), SharedIcon::ConsoleClose);
    }

    /// The record names exactly these files, so the page and the host cannot drift apart.
    #[test]
    fn the_shared_record_names_these_icons() -> Result<(), Box<dyn std::error::Error>> {
        let record: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../Sugar/integrations/contracts/ui/elements.json"
        ))?;
        let toggles = &record["toggles"];
        let named = [
            (&toggles["console"]["toOpen"], SharedIcon::ConsoleOpen, "console-open.svg"),
            (&toggles["console"]["toClose"], SharedIcon::ConsoleClose, "console-close.svg"),
            (&toggles["outputCanvas"]["toOpen"], SharedIcon::CanvasOpen, "canvas-open.svg"),
            (&toggles["outputCanvas"]["toClose"], SharedIcon::CanvasClose, "canvas-close.svg"),
            (&record["windowClose"], SharedIcon::WindowClose, "window-close.svg"),
        ];
        for (entry, icon, file) in named {
            assert_eq!(entry.as_str(), Some(format!("icons/{file}").as_str()), "{icon:?}");
        }
        Ok(())
    }
}
