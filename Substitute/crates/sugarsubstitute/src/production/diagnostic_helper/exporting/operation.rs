// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read genuine current records and retain all native custody across sequential active cleanup.
use super::*;
use sugarsubstitute_execution::{ExecutionClient, ReportExportAdmission, ReportExportOrigin};
/// Metadata belongs to the original fixed controller source; decode belongs to source Main.
pub(super) struct Operation {
    pub(super) input: Option<ExportInput>,
    pub(super) native: Option<CommittedDiagnosticReader>,
    pub(super) construction_failure: Option<CommittedReaderConstructionFailure>,
    pub(super) dump_reader: Option<IncidentDumpReader>,
    pub(super) export: Option<ReportExportOperation<PreparedReportView>>,
    receiving_admission: Option<ReportExportAdmission>,
    receiving_origin: Option<ReportExportOrigin>,
    pub(super) rejected_dump: Option<IncidentDumpReadLease>,
    pub(super) correlation: Option<(LogicalStoreOperationKey, ReportBinding)>,
    pub(super) current: Option<StoredIncidentSummary>,
    pub(super) ledger: Option<SummaryEvictions>,
    pub(super) slot: Option<CommittedReadSlot>,
    pub(super) source: Vec<u8>,
    pub(super) decode: Option<MemoryClaim>,
    pub(super) first: Option<ExportCause>,
    pub(super) receiving_failure: Option<ExportCause>,
    pub(super) source_failure: Option<ExportCause>,
    destination_returned: bool,
    pub(super) source_closed: bool,
    pub(super) receiving_closed: bool,
}
impl Operation {
    pub(super) fn new() -> Self {
        Self {
            input: None,
            native: None,
            construction_failure: None,
            dump_reader: None,
            export: None,
            receiving_admission: None,
            receiving_origin: None,
            rejected_dump: None,
            correlation: None,
            current: None,
            ledger: None,
            slot: None,
            source: Vec::new(),
            decode: None,
            first: None,
            receiving_failure: None,
            source_failure: None,
            destination_returned: false,
            source_closed: false,
            receiving_closed: false,
        }
    }
    /// Preserve this same authenticated empty rejection while returning original terminal loans.
    /// The service first requires all four actual deliveries and both checked closes; this
    /// disposition grants no native effect, read, write, activation or retirement authority.
    pub(super) fn take_terminal_input(&mut self) -> Option<ExportInput> {
        let rejected_destination = self.empty_destination_rejection();
        let input = self.input.take()?;
        self.destination_returned = rejected_destination;
        Some(input)
    }
    /// Both contexts must remain genuinely Running on one central coordinated worker.
    /// Rejections preserve the same authenticated input and every opened native owner.
    pub(super) fn execute(
        &mut self,
        client: &ExecutionClient,
        source: &JobContext,
        receiving: &JobContext,
    ) {
        if let Err(cause) = self.execute_inner(client, source, receiving) {
            self.first.get_or_insert(cause);
        }
    }
    #[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
    fn execute_inner(
        &mut self,
        client: &ExecutionClient,
        source: &JobContext,
        receiving: &JobContext,
    ) -> Result<(), ExportCause> {
        let input = self.input.as_ref().ok_or(ExportCause::State)?;
        let protected = input.protected.as_ref().ok_or(ExportCause::State)?;
        if protected.execution_owner() != source.owner_id() {
            return Err(ExportCause::State);
        }
        let intent = match authentication::validate(input) {
            Ok(intent) => intent,
            Err(cause) if authentication::destination_rejection(&cause) => {
                self.correlation = Some((
                    protected.operation(),
                    input.view.as_ref().ok_or(ExportCause::State)?.view().binding(),
                ));
                return Err(cause);
            }
            Err(cause) => return Err(cause),
        };
        let binding = intent.binding();
        self.correlation =
            Some((input.protected.as_ref().ok_or(ExportCause::State)?.operation(), binding));
        self.receiving_admission =
            Some(client.report_export_admission(receiving).map_err(ExportCause::Admission)?);
        let admission =
            client.committed_reader_admission(source).map_err(ExportCause::Admission)?;
        let request = self
            .input
            .as_mut()
            .and_then(|input| input.protected.take())
            .ok_or(ExportCause::State)?;
        match CommittedDiagnosticReader::new(request, admission) {
            Ok(native) => self.native = Some(native),
            Err(failure) => {
                let cause = failure.cause.clone();
                self.construction_failure = Some(failure);
                return Err(ExportCause::Native(cause));
            }
        }
        self.decode = Some(source.memory().claim(786_432).map_err(ExportCause::Memory)?);
        let slot = self.read(CommittedReadObject::Summary(binding.incident()))?;
        self.slot = Some(slot);
        self.current = Some(decode_summary(&self.source).map_err(ExportCause::Record)?);
        let current = self.current.as_ref().ok_or(ExportCause::State)?;
        if current.summary().seed().key() != binding.incident()
            || current.summary().revision() != binding.revision()
        {
            return Err(ExportCause::State);
        }
        let utc = SystemIncidentUtc
            .sample(self.native.as_ref().ok_or(ExportCause::State)?.original_origin())
            .map_err(ExportCause::Utc)?;
        let facts = current.summary().facts();
        if current.summary().seed().created() > utc
            || facts.retention().occurred() > utc
            || facts.retention().acknowledged().is_some_and(|at| at > utc)
        {
            return Err(ExportCause::State);
        }
        self.read(CommittedReadObject::StoreState)?;
        let IncidentStoreState::Ledger(ledger) =
            decode_store_state(&self.source).map_err(ExportCause::Record)?
        else {
            return Err(ExportCause::State);
        };
        if ledger.namespace() != binding.incident().subject().coordinator().namespace
            || ledger.last().is_some_and(|last| last.at() > utc)
        {
            return Err(ExportCause::State);
        }
        if !self
            .input
            .as_ref()
            .and_then(|input| input.view.as_ref())
            .ok_or(ExportCause::State)?
            .matches_current(self.current.as_ref().ok_or(ExportCause::State)?, ledger)
        {
            return Err(ExportCause::State);
        }
        self.ledger = Some(ledger);
        let mut dump = None;
        if intent.selection() == ExportSelection::TextAndNativeDump {
            let CaptureEvidence::Finished(id) =
                self.current.as_ref().ok_or(ExportCause::State)?.summary().facts().capture()
            else {
                return Err(ExportCause::State);
            };
            let native = self.native.take().ok_or(ExportCause::State)?;
            match IncidentDumpReader::new(native) {
                Ok(reader) => self.dump_reader = Some(reader),
                Err(failure) => {
                    self.native = Some(failure.reader);
                    return Err(match failure.cause {
                        DumpReaderConstructionCause::Control(cause) => {
                            ExportCause::Admission(cause)
                        }
                        DumpReaderConstructionCause::Allocation(cause) => {
                            ExportCause::Memory(cause)
                        }
                    });
                }
            }
            let reader = self.dump_reader.as_mut().ok_or(ExportCause::State)?;
            IncidentRepository::open_dump(reader, binding.incident(), binding.revision(), id, utc)
                .map_err(|_| ExportCause::State)?;
            match self.dump_reader.take().ok_or(ExportCause::State)?.take_verified() {
                Ok(lease) => dump = Some(lease),
                Err(reader) => {
                    self.dump_reader = Some(reader);
                    return Err(ExportCause::State);
                }
            }
        } else {
            self.check_current()?;
        }
        self.rejected_dump = dump;
        let admission = self.receiving_admission.take().ok_or(ExportCause::State)?;
        let view =
            self.input.as_mut().and_then(|input| input.view.take()).ok_or(ExportCause::State)?;
        match ReportExportOperation::new(admission, intent, view, self.rejected_dump.take()) {
            Ok(export) => self.export = Some(export),
            Err((cause, admission, _intent, view, dump)) => {
                self.input.as_mut().ok_or(ExportCause::State)?.view = Some(view);
                // The checked source owner must remain here even on receiving admission failure.
                self.rejected_dump = dump;
                self.receiving_admission = Some(admission);
                return Err(ExportCause::Export(cause));
            }
        }
        self.export
            .as_mut()
            .ok_or(ExportCause::State)?
            .execute(self.current.as_ref().ok_or(ExportCause::State)?)
            .map_err(|_| ExportCause::State)?;
        if self.native.is_some() {
            self.check_current()?;
        }
        Ok(())
    }
    /// A lexical rejection retains its actual authenticated inputs without constructing an owner.
    pub(super) fn empty_destination_rejection(&self) -> bool {
        self.first.as_ref().is_some_and(authentication::destination_rejection)
            && self.correlation.is_some()
            && self.input.as_ref().map_or(self.destination_returned, |input| {
                input.protected.is_some() && input.view.is_some()
            })
            && self.native.is_none()
            && self.construction_failure.is_none()
            && self.dump_reader.is_none()
            && self.export.is_none()
            && self.receiving_admission.is_none()
            && self.receiving_origin.is_none()
            && self.rejected_dump.is_none()
            && self.current.is_none()
            && self.ledger.is_none()
            && self.slot.is_none()
            && self.decode.is_none()
            && self.source.capacity() == 0
    }
    #[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
    fn read(&mut self, object: CommittedReadObject) -> Result<CommittedReadSlot, ExportCause> {
        let native = self.native.as_mut().ok_or(ExportCause::State)?;
        let slot = native.open(object).map_err(ExportCause::Native)?;
        let count = usize::try_from(native.facts(&slot).map_err(ExportCause::Native)?.bytes())
            .map_err(|_| ExportCause::State)?;
        if count == 0 || count > 65_536 {
            return Err(ExportCause::State);
        }
        self.source.clear();
        if self.source.capacity() < count {
            self.source.try_reserve_exact(count).map_err(ExportCause::Reserve)?;
        }
        if self.source.capacity() > 65_536 {
            return Err(ExportCause::State);
        }
        self.source.resize(count, 0);
        if native.read_chunk(&slot, 0, &mut self.source).map_err(ExportCause::Native)? != count {
            return Err(ExportCause::State);
        }
        Ok(slot)
    }
    #[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
    fn check_current(&mut self) -> Result<(), ExportCause> {
        self.native
            .as_mut()
            .ok_or(ExportCause::State)?
            .check_current(self.slot.as_ref().ok_or(ExportCause::State)?)
            .map_err(ExportCause::Native)
    }
    /// Create and use receiving retirement only inside its own still-Running factory.
    #[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
    pub(super) fn retire_receiving(
        &mut self,
        client: &ExecutionClient,
        context: &JobContext,
    ) -> Result<(), ExportCause> {
        if let Some(export) = self.export.as_mut() {
            let proof = client
                .report_export_retirement(
                    export.original_origin().map_err(ExportCause::Export)?,
                    context,
                )
                .map_err(ExportCause::Admission)?;
            export.retire_receiving(&proof).map_err(|_| ExportCause::State)?;
        }
        if let Some(admission) = self.receiving_admission.take() {
            self.receiving_origin = Some(admission.into_origin());
        }
        if let Some(origin) = self.receiving_origin.as_ref() {
            client
                .report_export_retirement(origin, context)
                .map_err(ExportCause::Admission)?
                .check()
                .map_err(ExportCause::Admission)?;
        }
        self.receiving_closed = true;
        Ok(())
    }
    /// Create and use source retirement only inside its own still-Running factory.
    #[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
    pub(super) fn retire_source(
        &mut self,
        client: &ExecutionClient,
        context: &JobContext,
    ) -> Result<(), ExportCause> {
        if let Some(export) = self.export.as_mut()
            && let Some(origin) = export.source_origin()
        {
            let proof = client
                .committed_reader_retirement(origin, context)
                .map_err(ExportCause::Admission)?;
            export.retire_source(&proof).map_err(|_| ExportCause::State)?;
        }
        if let Some(dump) = self.rejected_dump.as_mut()
            && let Some(origin) = dump.original_origin()
        {
            let proof = client
                .committed_reader_retirement(origin, context)
                .map_err(ExportCause::Admission)?;
            dump.retire_in_place(&proof).map_err(|_| ExportCause::State)?;
        }
        if let Some(reader) = self.dump_reader.as_mut()
            && let Some(origin) = reader.original_origin()
        {
            let proof = client
                .committed_reader_retirement(origin, context)
                .map_err(ExportCause::Admission)?;
            reader.retire_in_place(&proof).map_err(|_| ExportCause::State)?;
        }
        if let Some(native) = self.native.take() {
            let proof = match client.committed_reader_retirement(native.original_origin(), context)
            {
                Ok(proof) => proof,
                Err(cause) => {
                    self.native = Some(native);
                    return Err(ExportCause::Admission(cause));
                }
            };
            if let Err(failure) = native.retire(&proof) {
                self.native = Some(failure.owner);
                return Err(ExportCause::Native(failure.cause));
            }
        }
        if let Some(failure) = self.construction_failure.as_ref() {
            client
                .committed_reader_retirement(failure.original_origin(), context)
                .map_err(ExportCause::Admission)?
                .check()
                .map_err(ExportCause::Admission)?;
        }
        self.source_closed = true;
        Ok(())
    }
}
