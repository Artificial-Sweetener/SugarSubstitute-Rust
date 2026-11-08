// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Upload viewport-anchored effects in full rather than translating their baked pixels.
use super::*;
impl Session {
    /// Repair camera-only pixels only when neither the old nor new frame contains fixed effects.
    pub(in crate::native_canvas::session) fn cpu_display_damage(
        &self,
        current: DisplayEvidence,
    ) -> Result<(DamagePlan, Option<FrameStamp>), Box<dyn Error>> {
        let anchored = current.overlay != 0
            || current.backdrop_active
            || self
                .document
                .display_evidence
                .is_some_and(|shown| shown.overlay != 0 || shown.backdrop_active);
        let same_sources = !anchored
            && self.document.display_source == Some(current.source)
            && self
                .document
                .display_evidence
                .and_then(|shown| shown.underlay)
                .map(|underlay| underlay.source)
                == current.underlay.map(|underlay| underlay.source);
        if same_sources
            && let (Some(previous), Some(base)) =
                (self.document.display_view, self.document.native_presented)
        {
            let damage = current.view.repair_from(previous)?;
            let base = (damage.blit().is_some() || damage.dirty().is_empty()).then_some(base);
            return Ok((damage, base));
        }
        Ok((DamagePlan::full(current.view.surface()), None))
    }
}
