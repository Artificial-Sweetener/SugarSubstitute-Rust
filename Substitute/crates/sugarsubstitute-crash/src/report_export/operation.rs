// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep receiving outputs, immutable report and genuine source alive through cleanup.
use super::retention::Retained;
use super::{ReportExportCause as E, ReportExportCode as C, ReportExportIntent, stream};
use crate::{
    incident::{IncidentDumpReadLease, StoredIncidentSummary},
    report_view::ReportView,
};
use std::borrow::Borrow;
use sugarsubstitute_contracts::incident::{ExportSelection, ReportBinding};
use sugarsubstitute_execution::{
    CommittedReaderOrigin, CommittedReaderRetirement, ReportExportAdmission, ReportExportOrigin,
    ReportExportRetirement,
};
use sugarsubstitute_supervision::ReportFile;
/// Actual selected export facts emitted only after flush, activation and checked close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportExportReceipt {
    /// Exact canonical report incident/revision/session.
    pub binding: ReportBinding,
    /// Original authenticated selected request number.
    pub request: u64,
    /// Explicit sensitive-memory selection actually written.
    pub selection: ExportSelection,
    /// Complete resulting file length, including ZIP metadata when selected.
    pub bytes: u64,
    /// Complete resulting file digest observed while writing the held stage.
    pub sha256: [u8; 32],
}
/// Passive actual receiving-handle observations; these grant no effect or close right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportExportCustody {
    /// Actual physical parent/file outputs still retained by this owner.
    pub native_outputs: u16,
    /// Actual receiving-stage inode still has its held kernel handle.
    pub stage: bool,
    /// Actual selected predecessor inode still has its held kernel handle.
    pub predecessor: bool,
    /// Actual successful same-stage rename has been observed.
    pub activated: bool,
}
/// Owning finite operation; dropping it does not prove file/source retirement.
#[must_use = "Retain export outputs and original source until checked cleanup"]
pub struct ReportExportOperation<V: Borrow<ReportView> = ReportView> {
    fields: Retained<Fields<V>>,
    destination: ReportFile,
}
/// Retain original charges, canonical report, optional source and bounded failure slots.
struct Fields<V> {
    _original: ReportExportOrigin,
    intent: ReportExportIntent,
    view: Option<V>,
    dump: Option<IncidentDumpReadLease>,
    buffer: [u8; stream::CHUNK],
    attempted: bool,
    written: Option<stream::Written>,
    first: Option<E>,
    secondary: Option<E>,
    retirement: Option<E>,
    source_retirement: Option<E>,
    receiving_retired: bool,
}
impl<V: Borrow<ReportView>> ReportExportOperation<V> {
    /// Consume genuine receiving-write admission and exact already charged report/source.
    /// This performs no native receiving effect. The immutable view's existing charge
    /// and source's original reader claim are retained, never charged to this admission.
    /// `V` keeps the presentation owner and its slot inseparable from the report text.
    /// # Errors
    /// Invalid admission returns the same inputs in the rejection tuple.
    // Rejection returns the complete fixed stack selection before any new allocation.
    #[allow(clippy::type_complexity, clippy::result_large_err)]
    pub fn new(
        admission: ReportExportAdmission,
        intent: ReportExportIntent,
        view: V,
        dump: Option<IncidentDumpReadLease>,
    ) -> Result<
        Self,
        (E, ReportExportAdmission, ReportExportIntent, V, Option<IncidentDumpReadLease>),
    > {
        if let Err(cause) = admission.check_effect() {
            return Err((E::Control(cause), admission, intent, view, dump));
        }
        if size_of::<Fields<V>>() + 32_768 > 131_072 {
            return Err((E::Rejected(C::Bounds), admission, intent, view, dump));
        }
        let original = admission.into_origin();
        Ok(Self {
            fields: Retained::new(Fields {
                _original: original.clone(),
                intent,
                view: Some(view),
                dump,
                buffer: [0; stream::CHUNK],
                attempted: false,
                written: None,
                first: None,
                secondary: None,
                retirement: None,
                source_retirement: None,
                receiving_retired: false,
            }),
            destination: ReportFile::new(original),
        })
    }
    /// Borrow genuine receiving lifetime for the central paired cleanup owner.
    /// # Errors
    /// Returns Identity if the native destination owner is unavailable.
    pub fn original_origin(&self) -> Result<&ReportExportOrigin, E> {
        self.destination.origin().map_err(E::from)
    }
    /// Observe physical receiving custody independently of finite result delivery.
    pub fn custody(&self) -> ReportExportCustody {
        let custody = self.destination.custody();
        ReportExportCustody {
            native_outputs: custody.native_outputs,
            stage: custody.stage,
            predecessor: custody.predecessor,
            activated: custody.activated,
        }
    }
    /// Borrow genuine original reader custody for its own active cleanup factory.
    /// Returns None for text-only or after all source pins have checked closure.
    pub fn source_origin(&self) -> Option<&CommittedReaderOrigin> {
        self.fields.dump.as_ref().and_then(IncidentDumpReadLease::original_origin)
    }
    /// Export the current record on the receiving owner's finite worker.
    /// The source caller retains the actual committed-reader fence throughout this
    /// method, after the preceding repository writer has checked retirement.
    /// For text+dump, the genuine dump reader holds that same writer fence and
    /// independently validates exact current Summary and immutable payload generation.
    /// Its original reader Main remains Running until streaming settles. The reader
    /// re-observes Summary before/after copying and immediately before activation.
    /// # Errors
    /// Failure keeps original error, same view, native files and genuine reader here.
    pub fn execute(&mut self, current: &StoredIncidentSummary) -> Result<(), C> {
        if self.fields.attempted {
            return Err(C::Identity);
        }
        self.fields.attempted = true;
        let result = self.execute_inner(current);
        match result {
            Ok(()) => Ok(()),
            Err(cause) => {
                let code = cause.code();
                self.fields.first = Some(cause);
                if let Err(cause) = self
                    .destination
                    .origin()
                    .map_err(E::from)
                    .and_then(|origin| origin.check_effect().map_err(E::Control))
                {
                    self.fields.secondary = Some(cause);
                }
                Err(code)
            }
        }
    }
    /// Validate exact current facts before receiving effects and native activation.
    fn execute_inner(&mut self, current: &StoredIncidentSummary) -> Result<(), E> {
        let fields: &mut Fields<V> = &mut self.fields;
        let binding = fields.intent.binding();
        if binding != fields.view.as_ref().ok_or(E::Rejected(C::Identity))?.borrow().binding()
            || binding.incident() != current.summary().seed().key()
            || binding.revision() != current.summary().revision()
            || (fields.intent.selection() == ExportSelection::TextAndNativeDump)
                != fields.dump.is_some()
        {
            return Err(E::Rejected(C::Identity));
        }
        if let Some(dump) = &mut fields.dump {
            if dump.summary().map_err(E::Dump)? != current || current.payload_revision().is_none() {
                return Err(E::Rejected(C::Identity));
            }
            dump.check_current().map_err(E::Dump)?;
        }
        self.destination.open(&fields.intent.destination).map_err(E::from)?;
        let written = stream::write(
            &mut self.destination,
            fields.view.as_ref().ok_or(E::Rejected(C::Identity))?.borrow(),
            fields.dump.as_mut(),
            &mut fields.buffer,
        )?;
        self.destination.flush()?;
        if let Some(dump) = &mut fields.dump {
            dump.check_current().map_err(E::Dump)?;
        }
        self.destination.activate()?;
        fields.written = Some(written);
        Ok(())
    }
    /// Checked-close receiving files within their actual active cleanup factory.
    /// The genuine source closes separately under its own original cleanup job.
    /// # Errors
    /// Uncertain close retains identical receiving custody and its original cause.
    pub fn retire_receiving(&mut self, receiving: &ReportExportRetirement) -> Result<(), C> {
        let result = self.destination.retire(receiving);
        if let Err(cause) = result {
            if self.fields.retirement.is_none() {
                self.fields.retirement = Some(cause.into());
            }
            return Err(C::Retirement);
        }
        self.fields.receiving_retired = true;
        Ok(())
    }
    /// Checked-close the genuine source within its own actual active cleanup factory.
    /// This creates no receiving-file effect and cannot renew either original cutoff.
    /// # Errors
    /// Foreign proof or uncertain source closure retains the same reader and cause.
    pub fn retire_source(&mut self, source: &CommittedReaderRetirement) -> Result<(), C> {
        let result = self
            .fields
            .dump
            .as_mut()
            .map_or(Ok(()), |dump| dump.retire_in_place(source).map_err(E::Dump));
        if let Err(cause) = result {
            if self.fields.source_retirement.is_none() {
                self.fields.source_retirement = Some(cause);
            }
            return Err(C::Retirement);
        }
        Ok(())
    }
    /// Observe complete success only after all receiving outputs have checked closure.
    /// No attempt/acknowledgement/restart or storage-disposition mutation is implied.
    pub fn receipt(&self) -> Option<ReportExportReceipt> {
        if !self.fields.receiving_retired
            || !self.destination.closed()
            || self.fields.first.is_some()
            || self.fields.retirement.is_some()
            || self.fields.source_retirement.is_some()
            || self.fields.dump.as_ref().is_some_and(|dump| dump.original_origin().is_some())
        {
            return None;
        }
        let written = self.fields.written.as_ref()?;
        Some(ReportExportReceipt {
            binding: self.fields.intent.binding(),
            request: self.fields.intent.request().get(),
            selection: self.fields.intent.selection(),
            bytes: written.bytes,
            sha256: written.sha256,
        })
    }
    /// Transfer the complete presentation owner after actual receiving and source closure.
    /// Its native slot, original view charge and disposal guard remain inseparable in `V`.
    /// Original export errors remain with this operation after transfer.
    pub fn take_view(&mut self) -> Option<V> {
        if !self.fields.receiving_retired
            || !self.destination.closed()
            || self.fields.dump.as_ref().is_some_and(|dump| dump.original_origin().is_some())
        {
            return None;
        }
        self.fields.view.take()
    }
    /// Borrow original native/execution/source category without formatting sensitive input.
    pub fn original_failure(&self) -> Option<&E> {
        self.fields
            .first
            .as_ref()
            .or(self.fields.retirement.as_ref())
            .or(self.fields.source_retirement.as_ref())
    }
}
impl<V: Borrow<ReportView>> std::fmt::Debug for ReportExportOperation<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReportExportOperation")
    }
}
impl<V: Borrow<ReportView>> std::fmt::Display for ReportExportOperation<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Report export retains original failure and custody")
    }
}
impl<V: Borrow<ReportView>> std::error::Error for ReportExportOperation<V> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let cause = self.original_failure()?;
        if matches!(cause, E::Dump(_)) {
            return self
                .fields
                .dump
                .as_ref()
                .map(|dump| dump as &(dyn std::error::Error + 'static));
        }
        Some(cause)
    }
}
impl<V: Borrow<ReportView>> Drop for ReportExportOperation<V> {
    fn drop(&mut self) {
        if self.destination.closed()
            && self.fields.dump.as_ref().is_none_or(|dump| dump.original_origin().is_none())
        {
            self.fields.release();
        }
    }
}
const _: () = assert!(size_of::<Fields<ReportView>>() + 32_768 < 131_072);
#[cfg(test)]
mod tests;
