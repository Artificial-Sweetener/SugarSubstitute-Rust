// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report parked input documents' backings to the memory budget and give them back under
//! pressure.
//!
//! Input documents are authoritative and never leave under pressure; only their prepared
//! backings, which are rebuilt from the document when it is shown again, are released. Detail
//! backings go first, then the remaining fallback and navigation backings. The output parked
//! behind a shown input document follows the same rule.
use super::*;

impl InputDocuments {
    fn parked(&self) -> impl Iterator<Item = &document::DocumentState> {
        self.workflows
            .iter()
            .flat_map(|(_, surfaces)| surfaces)
            .filter_map(|surface| surface.document.as_deref())
            .chain(self.parked_output.as_deref())
    }

    /// Backing bytes parked documents hold, as the retained-document ledger counts them.
    pub(in crate::native_canvas::session) fn bytes(&self) -> u64 {
        self.parked().map(document::DocumentState::retained_bytes).fold(0, u64::saturating_add)
    }

    /// The part of [`Self::bytes`] held in video memory.
    pub(in crate::native_canvas::session) fn gpu_bytes(&self) -> u64 {
        self.parked().map(document::DocumentState::retained_gpu_bytes).fold(0, u64::saturating_add)
    }

    /// Release parked backings until [`Self::bytes`], or with `video` [`Self::gpu_bytes`],
    /// fits `allowance`: every detail backing first, then whole documents' backings.
    pub(in crate::native_canvas::session) fn shed(&mut self, allowance: u64, video: bool) {
        let measure = |inputs: &Self| if video { inputs.gpu_bytes() } else { inputs.bytes() };
        for complete in [false, true] {
            let documents = self
                .workflows
                .iter_mut()
                .flat_map(|(_, surfaces)| surfaces)
                .filter_map(|surface| surface.document.as_deref_mut())
                .chain(self.parked_output.as_deref_mut());
            for document in documents {
                release(document, complete, &mut self.retiring);
            }
            if measure(self) <= allowance {
                return;
            }
        }
    }
}

/// Release one document's detail backings, or with `complete` every backing it holds.
fn release(document: &mut document::DocumentState, complete: bool, retiring: &mut Retiring) {
    retiring.surfaces.extend(document.front.take());
    retiring.gpu_surfaces.extend(document.gpu_front.take());
    document.presented = None;
    if complete {
        retiring.surfaces.extend(document.fallback_front.take());
        retiring.surfaces.extend(document.navigation_front.take());
        retiring.gpu_surfaces.extend(document.fallback_gpu_front.take());
        retiring.gpu_surfaces.extend(document.navigation_gpu_front.take());
        document.fallback_presented = None;
        document.navigation_presented = None;
        document.presented_documents = [None, None, None];
    }
}

impl Session {
    /// Bring parked input documents' backings within what the retained-document `limit`
    /// leaves after retained outputs.
    pub(in crate::native_canvas::session) fn shed_input_backings(&mut self, limit: u64) {
        let allowance = limit.saturating_sub(self.retained.bytes());
        if self.inputs.bytes() > allowance {
            tracing::debug!(
                operation = "canvas_input_backings_shed",
                parked_bytes = self.inputs.bytes(),
                allowance_bytes = allowance,
                "Parked input documents gave backing memory back"
            );
            self.inputs.shed(allowance, false);
        }
    }
}
