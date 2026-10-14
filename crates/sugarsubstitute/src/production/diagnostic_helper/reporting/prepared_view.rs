// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep canonical view text, exact stored facts and metadata credit inseparable through disposal.
use super::*;
/// Original immutable facts are copied only from the genuine committed load after credit admission.
/// The snapshot grants no filesystem or activation authority.
pub(super) struct PreparedViewContent {
    view: ReportView,
    stored: StoredIncidentSummary,
    evictions: SummaryEvictions,
    _metadata: MemoryClaim,
}
impl PreparedViewContent {
    /// Borrow bounded canonical text without releasing the same snapshot or metadata claim.
    pub(super) fn view(&self) -> &ReportView {
        &self.view
    }
}
const SNAPSHOT_HEAP_BYTES: usize = 2 * BuildIdentity::MAX_BYTES
    + 2 * FileLabel::MAX_BYTES
    + SymbolName::MAX_BYTES
    + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>();
const _: () = assert!(
    size_of::<PreparedReportView>()
        + size_of::<PreparedViewContent>()
        + SNAPSHOT_HEAP_BYTES
        + 2 * size_of::<usize>()
        <= VIEW_METADATA_BYTES as usize
);
/// Same charged view, exact snapshot, central delivery and one of two source-owned slots.
/// The composing parent retains this owner until matched stream/native disposition.
pub(in crate::production) struct PreparedReportView {
    content: Box<PreparedViewContent>,
    _completion: JobCompletion<bool, ReportingIssue>,
    _slot: source::ViewSlot,
}
impl PreparedReportView {
    /// Couple only the internally prepared content with its validated completion and unique slot.
    pub(super) fn new(
        content: Box<PreparedViewContent>,
        completion: JobCompletion<bool, ReportingIssue>,
        slot: source::ViewSlot,
    ) -> Self {
        Self { content, _completion: completion, _slot: slot }
    }
    /// Borrow canonical bounded LF/Unicode without escaping its slot or original320KiB charge.
    pub(in crate::production) fn view(&self) -> &ReportView {
        self.content.view()
    }
    /// Compare whole original summary, immutable payload generation and namespace eviction ledger.
    /// The caller supplies independently observed current facts under actual writer/read custody.
    pub(in crate::production) fn matches_current(
        &self,
        current: &StoredIncidentSummary,
        evictions: SummaryEvictions,
    ) -> bool {
        self.content.stored == *current
            && self.content.evictions == evictions
            && self.view().binding().incident() == current.summary().seed().key()
            && self.view().binding().revision() == current.summary().revision()
    }
    /// Inspect retained metadata partition in owner tests without exporting its claim.
    #[cfg(test)]
    pub(super) fn metadata_claimed_bytes(&self) -> u64 {
        self.content._metadata.bytes()
    }
}
impl std::borrow::Borrow<ReportView> for PreparedReportView {
    fn borrow(&self) -> &ReportView {
        self.view()
    }
}
impl ReportingOperation {
    /// Format exact immutable committed facts solely from this finite presentation reservation.
    pub(super) fn prepare_view(&mut self, context: &JobContext) -> Result<(), ReportingCause> {
        if context.spec().reservation_bytes != REPORT_VIEW_JOB_BYTES
            || self.view.is_some()
            || self.failed()
            || self.retiring_authentication
            || context
                .deadline()
                .is_none_or(|until| self.original_deadline.is_none_or(|original| until > original))
        {
            return Err(ReportingCause::Identity);
        }
        self.check_view_controls(context)?;
        let (binding, locale) = self.view_request.ok_or(ReportingCause::Identity)?;
        let current = self.loaded.as_ref().ok_or(ReportingCause::Identity)?;
        let metadata =
            context.memory().claim(VIEW_METADATA_BYTES).map_err(ReportingCause::Memory)?;
        let stored =
            StoredIncidentSummary::new(current.summary().clone(), current.payload_revision())?;
        let evictions = current.evictions();
        let view =
            ReportView::format(stored.summary(), binding, evictions, &[], locale, context.memory())
                .map_err(ReportingCause::View)?;
        self.view =
            Some(Box::new(PreparedViewContent { view, stored, evictions, _metadata: metadata }));
        self.check_view_controls(context)
    }
}
