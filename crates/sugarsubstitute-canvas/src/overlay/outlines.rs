// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep traced selection outlines per coverage fingerprint, so pans, zooms, previews and undo
//! steps reuse them and only new selections are traced, off the input thread.
use super::CoverageContours;
use crate::CanvasError;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::Coverage;

/// Outlines kept for recently shown selections, such as the selection before and after an undo.
const RETAINED: usize = 4;

/// Tracing the host runs as an owned job for a selection whose outline is not cached.
#[derive(Debug)]
pub struct ContourWork {
    coverage: Coverage,
}
impl ContourWork {
    /// Content fingerprint of the coverage this work traces.
    pub fn fingerprint(&self) -> [u8; 32] {
        self.coverage.fingerprint()
    }
    /// Trace the outlines inside an owned job.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn run(self, context: &JobContext) -> Result<CoverageContours, CanvasError> {
        CoverageContours::trace(&self.coverage, context)
    }
}

/// Recently traced outlines, most recent first, plus the fingerprint being traced.
#[derive(Debug, Default)]
pub struct SelectionOutlines {
    retained: Vec<Arc<CoverageContours>>,
    tracing: Option<[u8; 32]>,
}
impl SelectionOutlines {
    /// Work for `selection` when its outline is neither cached nor already being traced. Hosts
    /// run it and hand the result to [`Self::complete`].
    pub fn request(&mut self, selection: Option<&Coverage>) -> Option<ContourWork> {
        let selection = selection?;
        let fingerprint = selection.fingerprint();
        if self.tracing == Some(fingerprint) || self.position(fingerprint).is_some() {
            return None;
        }
        self.tracing = Some(fingerprint);
        Some(ContourWork { coverage: selection.clone() })
    }
    /// Retain finished outlines, evicting the least recently shown beyond the bound.
    pub fn complete(&mut self, contours: CoverageContours) {
        if self.tracing == Some(contours.fingerprint()) {
            self.tracing = None;
        }
        if let Some(index) = self.position(contours.fingerprint()) {
            self.retained.remove(index);
        }
        self.retained.insert(0, Arc::new(contours));
        self.retained.truncate(RETAINED);
    }
    /// Forget an unfinished trace, such as one that failed or was cancelled, so a later request
    /// retries it.
    pub fn abandon(&mut self, fingerprint: [u8; 32]) {
        if self.tracing == Some(fingerprint) {
            self.tracing = None;
        }
    }
    /// Cached outlines of exactly `selection`, marking them most recently shown.
    pub fn get(&mut self, selection: Option<&Coverage>) -> Option<Arc<CoverageContours>> {
        // Fingerprints cover the extent as well as every value.
        let index = self.position(selection?.fingerprint())?;
        let contours = self.retained.remove(index);
        self.retained.insert(0, Arc::clone(&contours));
        Some(contours)
    }
    fn position(&self, fingerprint: [u8; 32]) -> Option<usize> {
        self.retained.iter().position(|contours| contours.fingerprint() == fingerprint)
    }
}
