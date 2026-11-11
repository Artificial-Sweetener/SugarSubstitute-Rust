// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authored Fluent viewports shared by native surface compositions.

/// Select a bundled Fluent icon without reconstructing or recentering its paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FluentIcon {
    /// Language selection.
    Language,
    /// Folder selection.
    Folder,
    /// Managed local installation.
    Home,
    /// Attached installation.
    Link,
    /// Installation or update work.
    Sync,
    /// Successful completion.
    Accept,
    /// Close a native surface.
    Close,
    /// Expand a selection control.
    ChevronDown,
}

impl FluentIcon {
    /// Borrow the single embedded source used by native vector preparation.
    pub const fn source(self) -> &'static str {
        match self {
            Self::Language => include_str!("../assets/fluent/Language.svg"),
            Self::Folder => include_str!("../assets/fluent/Folder.svg"),
            Self::Home => include_str!("../assets/fluent/Home.svg"),
            Self::Link => include_str!("../assets/fluent/Link.svg"),
            Self::Sync => include_str!("../assets/fluent/Sync.svg"),
            Self::Accept => include_str!("../assets/fluent/Accept.svg"),
            Self::Close => include_str!("../assets/fluent/Close.svg"),
            Self::ChevronDown => include_str!("../assets/fluent/ChevronDown.svg"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::{VectorAsset, VectorViewport};

    /// Exercise every real bundled source through the actual native vector renderer.
    #[test]
    fn fluent_sources_render_at_supported_icon_scales() -> Result<(), Box<dyn std::error::Error>> {
        for icon in [
            FluentIcon::Language,
            FluentIcon::Folder,
            FluentIcon::Home,
            FluentIcon::Link,
            FluentIcon::Sync,
            FluentIcon::Accept,
            FluentIcon::Close,
            FluentIcon::ChevronDown,
        ] {
            let asset = VectorAsset::parse(icon.source())?;
            for size in [10, 12, 16, 22, 24, 30, 36, 44, 48] {
                let pixels = asset.rasterize(
                    VectorViewport {
                        x: 0.0,
                        y: 0.0,
                        width: size as f32,
                        height: size as f32,
                        scale: 1.0,
                    },
                    None,
                )?;
                let view = pixels.pixels();
                assert!(view.data().chunks_exact(4).any(|p| p[3] > 0), "{icon:?}, {size}");
                assert!(view.data().chunks_exact(4).all(|p| p[..3].iter().all(|v| *v <= p[3])));
            }
        }
        Ok(())
    }
}
