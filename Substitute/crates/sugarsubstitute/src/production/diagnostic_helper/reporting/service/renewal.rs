// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace an actually adopted passive generation under its unchanged original source claim.
use super::*;
use crate::production::incident_host::AdoptedPassiveReporting;

struct PassiveFailureData {
    marker: PassiveReportingRestoration,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    completion: Option<JobCompletion<bool, ReportingIssue>>,
}
/// Retain original no-write delivery and causes until actual receiving-family disposal.
/// The complete metadata allocation remains covered by the same original source claim.
pub(in crate::production) struct RetainedPassiveReporting {
    data: Box<PassiveFailureData>,
    _source: Source,
}
impl RetainedPassiveReporting {
    /// Borrow the exact original first cause; absence does not create a fault classification.
    pub(in crate::production) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.data
            .admission
            .as_ref()
            .map(|cause| cause as _)
            .or_else(|| self.data.identity.as_ref().map(|cause| cause as _))
    }
    /// Observe only the original actual completion, independently from pure policy adoption.
    pub(in crate::production) fn outcome(&self) -> Option<TerminalKind> {
        self.data.completion.as_ref().map(|done| done.outcome.kind())
    }
    /// Read the immutable old generation marker; it grants no authority over the fresh source.
    pub(in crate::production) fn marker(&self) -> PassiveReportingRestoration {
        self.data.marker
    }
}
impl ReportingService {
    /// Authenticate only this same current closed pristine generation and its actual disposition.
    /// Aliases, views, admitted work, presentation and cleanup prevent replacement.
    pub(in crate::production) fn validate_passive_restoration(
        &self,
        marker: PassiveReportingRestoration,
    ) -> bool {
        self.restored == Some(marker)
            && self.requested == Some(marker.original())
            && self.installed
            && self.work.job.is_none()
            && !self.presentation.occupied()
            && !self.cleanup.occupied()
            && self.view_slot.is_none()
            && self.view_binding.is_none()
            && self.outcome() == marker.terminal()
            && self.source.passive_replacement_ready()
    }
    /// Move the same fixed claim into a distinct generation after actual sealed Host adoption.
    /// Rejection preserves the identical caller witness and all original service data.
    /// # Errors
    /// Rejects missing/foreign/repeated adoption, wrong work kind, aliases or nonpristine custody.
    pub(in crate::production) fn renew_passive_adopted(
        &mut self,
        adopted: &mut Option<AdoptedPassiveReporting>,
    ) -> Result<RetainedPassiveReporting, ReportingIssue> {
        let marker = self.restored.ok_or(ReportingIssue::Started)?;
        if !marker.is_symbols()
            || !self.validate_passive_restoration(marker)
            || adopted.as_ref().is_none_or(|witness| !witness.matches(marker))
        {
            return Err(ReportingIssue::Busy);
        }
        self.source.replace_passive_generation()?;
        let data = Box::new(PassiveFailureData {
            marker,
            admission: self.admission.take(),
            identity: self.identity.take(),
            completion: self.work.completion.take(),
        });
        let retained = RetainedPassiveReporting { data, _source: self.source.alias() };
        let _adopted = adopted.take();
        self.work = Delivery::default();
        self.cleanup = Delivery::default();
        self.presentation = Delivery::default();
        self.installed = false;
        self.requested = None;
        self.cleanup_binding = None;
        self.restored = None;
        Ok(retained)
    }
    /// Account the bounded single retained capsule within the existing source role.
    pub(in crate::production::diagnostic_helper::reporting) const fn passive_retention_bytes()
    -> usize {
        size_of::<PassiveFailureData>()
            + size_of::<RetainedPassiveReporting>()
            + 2 * size_of::<usize>()
    }
}
