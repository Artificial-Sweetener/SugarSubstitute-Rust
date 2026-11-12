// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Invoke the same pinned CPP token only under the protected three-file capture fence.
use super::{SettlementFailure, Token};
use crate::incident::CaptureDispositionPlan;
use sugarsubstitute_contracts::incident::IncidentRevision;
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticCaptureFence, DiagnosticCaptureNativeSource,
};
pub(crate) struct JournalSource<'a> {
    token: &'a mut Token,
    plan: &'a CaptureDispositionPlan,
    payload_revision: IncidentRevision,
}
impl<'a> JournalSource<'a> {
    pub(crate) fn new(
        token: &'a mut Token,
        plan: &'a CaptureDispositionPlan,
        payload_revision: IncidentRevision,
    ) -> Self {
        Self { token, plan, payload_revision }
    }
}
// SAFETY: Only the repository's actual pinned/decoded Journal and schema2 Summary construct
// this private adapter. Token compares the same real generation and all actual pinned file
// facts before FFI, checks the genuine fence before every effect, adopts outputs before
// validating them, and returns Ok only on complete native source/metadata/stage settlement.
unsafe impl DiagnosticCaptureNativeSource for JournalSource<'_> {
    type Error = SettlementFailure;
    fn settle(&mut self, fence: &mut DiagnosticCaptureFence<'_>) -> Result<(), Self::Error> {
        self.token
            .dispose(self.plan, self.payload_revision, fence)
            .map_err(|issue| self.token.settlement_failure(issue))
    }
}
