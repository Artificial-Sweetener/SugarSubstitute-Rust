// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Extend the authoritative record-file owner with retained bounded streaming custody.
mod allocation;
mod inventory;
mod io;
pub(crate) mod layout;
pub(crate) mod leaf;
mod lifecycle;
mod recovery;
mod session;
use super::super::{ProtectedStorageRoot, exclusion::HeldFile, paths, security};
use crate::protected_storage::{
    native_file::{FileGrant, NativeCauseDestinations, NativeFile, NativeFileAdmission, seek},
    native_name::{NativeName, NativeSid},
};
use crate::{
    ClaimError,
    claim_error::BoundedNativeCause,
    diagnostic_storage::{self as diagnostic, *},
};
use diagnostic::{NativeId, OwnedPort, Port};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use sugarsubstitute_contracts::{
    execution::JobContext, incident::LogicalStoreOperationKey, lifecycle::NamespaceId,
};

const FILE_SLOTS: usize = DIAGNOSTIC_STAGE_BOUND + DIAGNOSTIC_READ_BOUND + 4;

struct StageCandidate {
    leaf: String,
    identity: [u8; 24],
    bytes: u64,
}
struct NativeStageScan {
    candidates: Vec<StageCandidate>,
    _retention: DiagnosticNativeScanRetention,
}
pub(crate) struct StreamingPort {
    root: ProtectedStorageRoot,
    directory: Option<NativeName>,
    namespace: Option<NamespaceId>,
    sid: Option<NativeSid>,
    writer: Option<HeldFile>,
    files: Vec<NativeFile>,
    next: u64,
    stage_candidates: Option<NativeStageScan>,
    stage_scan_complete: bool,
    original_context: Option<JobContext>,
    causes: NativeCauseDestinations,
    original: DiagnosticNativeGrant,
}
impl Port for StreamingPort {
    fn bind_operation(&mut self, context: &JobContext) {
        self.original_context = Some(context.clone());
    }
    fn resolve(
        &mut self,
        root: &Path,
        namespace: NamespaceId,
        context: &JobContext,
    ) -> Result<diagnostic::genesis::NativeOpening, ClaimError> {
        self.check(context)?;
        self.root.resolve(root, context)?;
        let (directory, sid, disposition, identity) =
            self.root.diagnostic_directory(namespace, context)?;
        self.directory = Some(directory);
        self.sid = Some(sid);
        self.namespace = Some(namespace);
        self.check(context)?;
        Ok(match disposition {
            crate::protected_storage::directories::DirectoryDisposition::Created => {
                diagnostic::genesis::NativeOpening::Created(identity)
            }
            crate::protected_storage::directories::DirectoryDisposition::Existing => {
                diagnostic::genesis::NativeOpening::Existing
            }
        })
    }
    fn resolve_namespace(
        &mut self,
        request: &ProtectedDiagnosticWriteRequest,
        context: &JobContext,
    ) -> Result<diagnostic::genesis::NativeOpening, ClaimError> {
        self.check(context)?;
        self.original
            .bind_namespace(request, context)
            .map_err(ClaimError::diagnostic_allocation)?;
        let loan = crate::coordinator_entry::read_namespace::NamespaceReadLoan::take_writer(
            request,
            &self.original,
            context,
        )?;
        let resolved = loan.resolved()?;
        self.root.resolve(resolved.diagnostic_directory().0, context)?;
        self.root.validate_diagnostic_namespace(resolved, loan.handles()?, context)?;
        let (directory, sid, disposition, identity) =
            self.root.diagnostic_directory(request.operation().owner().namespace, context)?;
        self.directory = Some(directory);
        self.sid = Some(sid);
        self.namespace = Some(request.operation().owner().namespace);
        self.check(context)?;
        Ok(match disposition {
            crate::protected_storage::directories::DirectoryDisposition::Existing => {
                diagnostic::genesis::NativeOpening::Existing
            }
            crate::protected_storage::directories::DirectoryDisposition::Created => {
                diagnostic::genesis::NativeOpening::Created(identity)
            }
        })
    }
    fn acquire_writer(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self.check(context)?;
        let path = self.name("writer.lock", context)?;
        let record_path = self.name("unused.record", context)?;
        let sid = self.sid.as_ref().ok_or_else(|| self.reject())?.clone();
        let mut session = session::Session::path(&self.original, context)?;
        let result = HeldFile::acquire_into(
            crate::protected_storage::exclusion::ExclusionOpen::CreateOrOpen,
            &mut self.writer,
            path,
            record_path,
            sid,
            crate::native_claims::files::Budget::Diagnostic(self.original.clone()),
            &mut session.phase(context),
        );
        result?;
        self.check(context)
    }
    fn scan(&mut self, context: &JobContext) -> Result<diagnostic::inventory::Scan, ClaimError> {
        self.scan_owned(context)
    }
    fn open(
        &mut self,
        object: DiagnosticObject,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        let path = self.path(object, context)?;
        self.open_owned(path, bound, false, context)
    }
    fn open_inventory(
        &mut self,
        observed: &diagnostic::inventory::ObservedEntry,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        if !observed.readable() {
            return Err(self.invalid("diagnostic_inventory_read"));
        }
        let leaf = leaf::Leaf::inventory(observed).map_err(|_| self.reject())?;
        let path = self.name(leaf.text().map_err(|_| self.reject())?, context)?;
        let (id, facts) = self.open_owned(path, bound, false, context)?;
        if facts.identity != observed.identity || facts.bytes != observed.bytes {
            return Err(self.invalid("diagnostic_changed_inventory"));
        }
        Ok((id, facts))
    }
    fn create_stage(
        &mut self,
        object: DiagnosticObject,
        operation: LogicalStoreOperationKey,
        reserved: u64,
        context: &JobContext,
    ) -> Result<NativeId, ClaimError> {
        self.check(context)?;
        let namespace = self.namespace.ok_or_else(|| self.reject())?;
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        let result = super::random(&mut phase);
        let nonce = phase.post_context(result, context)?;

        drop(session);
        let leaf =
            leaf::Leaf::stage(object, namespace, operation, &nonce).map_err(|_| self.reject())?;
        let path = self.name(leaf.text().map_err(|_| self.reject())?, context)?;
        self.open_owned(path, reserved, true, context).map(|(id, _)| id)
    }
    fn recover_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.recover_stage_owned(
            object,
            historical,
            expected,
            recovery::StageRecoveryAccess::Ledger,
            context,
        )
    }
    fn recover_summary_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        access: diagnostic::SummaryStageAccess,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        let access = match access {
            diagnostic::SummaryStageAccess::PreservedOriginal => {
                recovery::StageRecoveryAccess::PreservedOriginal
            }
            diagnostic::SummaryStageAccess::Candidate => recovery::StageRecoveryAccess::Candidate,
        };
        self.recover_stage_owned(object, historical, expected, access, context)
    }
    fn observe_file(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        let destination = phase.name_destination()?;
        let actual =
            paths::final_path(&self.files[index].fields()?.handle, &mut phase, destination)?;
        if actual.path()? != self.files[index].fields()?.path.path()? {
            return Err(phase.invalid("diagnostic_changed_observed_path"));
        }
        drop(actual);
        self.observe(index, context)
    }
    fn stage_target(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<Option<DiagnosticReadFacts>, ClaimError> {
        let target = self.path(object, context)?;
        let index = self.index(id)?;
        let is_target = self.files[index].fields()?.path.path()? == target.path()?;
        let actual = self.observe_file(id, context)?;
        Ok(is_target.then_some(actual))
    }
    fn read(
        &mut self,
        id: NativeId,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, ClaimError> {
        self.read_owned(id, offset, output, context)
    }
    fn write(
        &mut self,
        id: NativeId,
        offset: u64,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.write_owned(id, offset, input, context)
    }
    fn flush(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.flush_owned(id, context)
    }
    fn activate(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.stage_scan_complete = false;
        self.activate_owned(id, object, expected, context)
    }
    fn reconcile_activation(
        &mut self,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.reconcile_owned(object, expected, context)
    }
    fn delete(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticReadFacts,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.stage_scan_complete = false;
        self.delete_owned(id, object, expected, context)
    }
    fn absent(
        &mut self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<bool, ClaimError> {
        self.absent_owned(object, context)
    }
    fn close(&mut self, id: NativeId, context: &JobContext) -> Result<(), ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let mut session = session::Session::path(&self.original, context)?;
        self.files[index].close_in_phase(&mut session.phase(context), context)?;
        if !self.files[index].fields()?.deleted {
            self.files.remove(index);
        }
        self.check(context)
    }
    fn retire(
        &mut self,
        _context: &JobContext,
        control: &mut diagnostic::DiagnosticNativeRetirement<'_>,
    ) -> Result<(), ClaimError> {
        for file in self.files.iter_mut().rev() {
            file.retire_diagnostic(control)?;
        }
        self.files.clear();
        if let Some(writer) = self.writer.as_mut() {
            writer.retire_diagnostic(control)?;
        }
        self.writer = None;
        self.root.retire_diagnostic(control)
    }
    fn retained_files(&self) -> usize {
        self.files.len()
    }
    fn live_handles(&self) -> usize {
        self.root.live_handles()
            + self.files.iter().filter(|file| file.has_output()).count()
            + usize::from(self.writer.as_ref().is_some_and(HeldFile::has_output))
    }
}
