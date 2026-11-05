// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report parked input documents' backings to the memory budget and give them back under
//! pressure.
//!
//! Input documents are authoritative and never leave under pressure; only their prepared
//! backings, which are rebuilt from the document when it is shown again, are released. Detail
//! backings go first, then the remaining fallback and navigation backings. The output parked
//! behind a shown input document follows the same rule.
//!
//! Whenever the input documents change, or their disposal settles, the session logs what the
//! canvas memory budget charges: the claimed bytes of the canvas's resident lease, which every
//! input document's coverage, history and edits are charged to, and the parked backing bytes
//! the application memory budget governs.
use super::*;

/// What the last input memory report described: loaded documents, their combined revision,
/// workflows, and whether disposal was still pending.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(in crate::native_canvas::session) struct ReportedMemory {
    documents: usize,
    revisions: u64,
    workflows: usize,
    disposing: bool,
}

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
    /// Log the canvas memory budget's charge once the input documents changed or their
    /// disposal settled since the last report.
    pub(in crate::native_canvas::session) fn report_input_memory(&mut self) {
        let Some(resources) = self.resources.as_ref() else { return };
        let retirement_idle = self.retirement.is_none()
            && self.pending_retirement.is_none()
            && self.deferred_retirement.is_none();
        self.inputs.disposing &= !retirement_idle;
        let revisions = self.input_revisions();
        let reported = ReportedMemory {
            documents: revisions.len(),
            // Published revisions only: a stroke's staged steps change no charge.
            revisions: revisions.iter().fold(0, |sum, (_, shown)| sum.wrapping_add(shown.revision)),
            workflows: self.inputs.workflows.len(),
            disposing: self.inputs.disposing || !self.inputs.retiring.is_empty(),
        };
        if self.inputs.reported.replace(reported) == Some(reported) {
            return;
        }
        tracing::info!(
            operation = "canvas_input_memory",
            documents = reported.documents,
            workflows = reported.workflows,
            disposing = reported.disposing,
            resident_claimed_bytes = resources.memory.claimed_bytes(),
            resident_bytes = resources.memory.bytes(),
            parked_input_backing_bytes = self.inputs.bytes(),
            retained_output_bytes = self.retained.bytes(),
            "Input documents changed what the canvas memory budget charges"
        );
    }

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
