// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Corrupt acknowledgement custody on purpose, so tests can prove each classified outcome.

use super::*;

impl Session {
    /// Make the evidence of the pending CPU frame disappear while the frame awaits its acknowledgement.
    pub(crate) fn lose_pending_cpu_evidence(&mut self) {
        self.document.pending_cpu_evidence = None;
    }

    /// Make the pending CPU frame's evidence name a source its projection custody does not hold.
    ///
    /// # Errors
    /// Fails when no CPU frame awaits acknowledgement or its generation is exhausted.
    pub(crate) fn skew_pending_cpu_evidence(&mut self) -> Result<(), Box<dyn Error>> {
        let evidence =
            self.document.pending_cpu_evidence.as_mut().ok_or("No CPU frame awaits evidence")?;
        evidence.source = FrameStamp {
            surface: evidence.source.surface,
            generation: evidence.source.generation.next()?,
        };
        Ok(())
    }

    /// Move the backdrop generation on without withdrawing the frame drawn for the old one.
    pub(crate) fn advance_backdrop_generation(&mut self) {
        self.backdrops.generation += 1;
    }

    /// Let pacing forget that `target` is in flight, as lost frame accounting would.
    ///
    /// # Errors
    /// Fails when pacing never held `target`.
    pub(crate) fn forget_frame_in_pacing(
        &mut self,
        target: FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        self.pacing.abandoned(target, self.navigation_time())?;
        Ok(())
    }
}
