// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold viewport-anchored wipe intent for a loaded comparison and derive its document divider.
use super::*;

impl Session {
    /// Move the wipe divider; the next prepared frame reveals the comparison at this split.
    /// # Errors
    /// Rejects a non-finite split or one outside `0.0..=1.0`.
    #[cfg(feature = "native-composition")]
    pub(crate) fn set_wipe(
        &mut self,
        orientation: WipeOrientation,
        split: f64,
    ) -> Result<(), Box<dyn Error>> {
        if !split.is_finite() || !(0.0..=1.0).contains(&split) {
            return Err("Comparison split must be within 0 and 1".into());
        }
        self.wipe = Wipe { orientation, split };
        Ok(())
    }

    /// Physical extent of the drawable surface the session currently prepares frames for.
    #[cfg(feature = "native-composition")]
    pub(crate) fn surface_extent(&self) -> sugarsubstitute_image_ops::Extent {
        self.document.tools.navigation().viewport().surface()
    }

    /// Return the selected comparison original's own extent, if one is loaded.
    #[cfg(feature = "native-composition")]
    pub(crate) fn compared_extent(&self) -> Option<sugarsubstitute_image_ops::Extent> {
        let scene = self.document.scene.as_ref()?;
        if scene.source_revision != self.selection {
            return None;
        }
        scene.compared.as_ref().map(|compared| compared.source.descriptor().layout().extent())
    }

    /// Place the divider in document coordinates for this screen camera, so every backing and its
    /// projection reveals the comparison at the viewport-anchored split once this camera settles.
    pub(super) fn comparison_for(
        &self,
        screen: Viewport,
    ) -> Result<Option<Comparison>, Box<dyn Error>> {
        let Some(compared) = self.document.scene.as_ref().and_then(|scene| scene.compared.as_ref())
        else {
            return Ok(None);
        };
        let surface = screen.surface();
        let Wipe { orientation, split } = self.wipe;
        let boundary = match orientation {
            WipeOrientation::Vertical => {
                screen.document_at([f64::from(surface.width()) * split, 0.0])?[0]
            }
            WipeOrientation::Horizontal => {
                screen.document_at([0.0, f64::from(surface.height()) * split])?[1]
            }
        };
        Ok(Some(compared.comparison(orientation, boundary)?))
    }
}
