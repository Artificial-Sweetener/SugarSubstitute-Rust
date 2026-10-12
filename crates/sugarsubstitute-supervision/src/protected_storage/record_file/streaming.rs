// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Extend the authoritative record-file owner with retained bounded streaming custody.
mod inventory;
mod io;
pub(crate) mod layout;
mod lifecycle;
mod recovery;
use super::super::{ProtectedStorageRoot, exclusion::HeldFile, paths, security};
use crate::{
    ClaimError,
    diagnostic_storage::{self as diagnostic, *},
    native_claims::files::{Budget, Handle},
};
use diagnostic::{NativeId, Port};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    incident::LogicalStoreOperationKey,
    lifecycle::NamespaceId,
};

const FILE_SLOTS: usize = DIAGNOSTIC_STAGE_BOUND + DIAGNOSTIC_READ_BOUND + 4;

struct StageCandidate {
    leaf: String,
    identity: [u8; 24],
    bytes: u64,
}
struct NativeFile {
    id: NativeId,
    handle: Handle,
    path: PathBuf,
    facts: Option<DiagnosticReadFacts>,
    bound: u64,
    stage: bool,
    writable: bool,
    deleted: bool,
}
pub(crate) struct StreamingPort {
    root: ProtectedStorageRoot,
    memory: Arc<MemoryClaim>,
    directory: Option<PathBuf>,
    namespace: Option<NamespaceId>,
    sid: Vec<u8>,
    writer: Option<HeldFile>,
    files: Vec<NativeFile>,
    next: u64,
    stage_candidates: Vec<StageCandidate>,
    stage_scan_complete: bool,
    original_context: Option<JobContext>,
}
impl StreamingPort {
    pub(crate) fn new(memory: Arc<MemoryClaim>) -> Result<Self, ClaimError> {
        let root = ProtectedStorageRoot::from_shared_claim(memory.clone())?;
        let mut files = Vec::new();
        files
            .try_reserve_exact(FILE_SLOTS)
            .map_err(|cause| ClaimError::new("diagnostic_native_reservation", cause))?;
        Ok(Self {
            root,
            memory,
            directory: None,
            namespace: None,
            sid: Vec::new(),
            writer: None,
            files,
            next: 1,
            stage_candidates: Vec::new(),
            stage_scan_complete: false,
            original_context: None,
        })
    }
    fn check(&self, context: &JobContext) -> Result<(), ClaimError> {
        check_context(&self.original_context, context)?;
        if self.live_handles() >= DIAGNOSTIC_HANDLE_BOUND {
            return Err(ClaimError::invalid("diagnostic_handle_bound"));
        }
        Ok(())
    }
    fn path(&self, object: DiagnosticObject) -> Result<PathBuf, ClaimError> {
        let namespace =
            self.namespace.ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?;
        if object.namespace().is_some_and(|actual| actual != namespace) {
            return Err(ClaimError::invalid("diagnostic_foreign_object"));
        }
        Ok(self
            .directory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?
            .join(layout::leaf(object, namespace)))
    }
    fn index(&self, id: NativeId) -> Result<usize, ClaimError> {
        self.files
            .iter()
            .position(|file| file.id == id)
            .ok_or_else(|| ClaimError::invalid("diagnostic_retired_lease"))
    }
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
    ) -> Result<(), ClaimError> {
        self.check(context)?;
        self.root.resolve(root, context)?;
        let (directory, sid) = self.root.diagnostic_directory(namespace, context)?;
        self.directory = Some(directory);
        self.sid = sid;
        self.namespace = Some(namespace);
        self.check(context)
    }
    fn acquire_writer(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self.check(context)?;
        let directory =
            self.directory.as_ref().ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?;
        HeldFile::acquire_into(
            &mut self.writer,
            &directory.join("writer.lock"),
            directory.join("unused.record"),
            &self.sid,
            Budget::Protected(self.memory.clone()),
        )?;
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
        let path = self.path(object)?;
        self.open_owned(path, bound, false, context)
    }
    fn open_inventory(
        &mut self,
        observed: &diagnostic::inventory::ObservedEntry,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        if !observed.readable() {
            return Err(ClaimError::invalid("diagnostic_inventory_read"));
        }
        let path = self
            .directory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?
            .join(format!("{}.{}", super::hex(&observed.digest), observed.class.suffix()));
        let (id, facts) = self.open_owned(path, bound, false, context)?;
        if facts.identity != observed.identity || facts.bytes != observed.bytes {
            return Err(ClaimError::invalid("diagnostic_changed_inventory"));
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
        let mut path = self.path(object)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ClaimError::invalid("diagnostic_leaf"))?;
        path.set_file_name(format!(
            "{name}.{}.{}.stage",
            super::hex(&layout::operation(operation)),
            super::hex(&super::random()?)
        ));
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
    fn recover_migration_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        access: diagnostic::MigrationStageAccess,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        let access = match access {
            diagnostic::MigrationStageAccess::PreservedOriginal => {
                recovery::StageRecoveryAccess::PreservedOriginal
            }
            diagnostic::MigrationStageAccess::Candidate => recovery::StageRecoveryAccess::Candidate,
        };
        self.recover_stage_owned(object, historical, expected, access, context)
    }
    fn observe_file(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        let index = self.index(id)?;
        if paths::final_path(&self.files[index].handle)? != self.files[index].path {
            return Err(ClaimError::invalid("diagnostic_changed_observed_path"));
        }
        self.observe(index, context)
    }
    fn migration_stage_target(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<Option<DiagnosticReadFacts>, ClaimError> {
        let target = self.path(object)?;
        let index = self.index(id)?;
        let is_target = self.files[index].path == target;
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
        self.files[index].handle.close()?;
        if !self.files[index].deleted {
            self.files.remove(index);
        }
        self.check(context)
    }
    fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        for file in self.files.iter_mut().rev() {
            super::super::check(context)?;
            file.handle.close()?;
        }
        self.files.clear();
        if let Some(writer) = self.writer.as_mut() {
            super::super::check(context)?;
            writer.close()?;
        }
        self.writer = None;
        self.root.retire(context)
    }
    fn live_handles(&self) -> usize {
        self.root.live_handles()
            + self.files.iter().filter(|file| !file.handle.raw().0.is_null()).count()
            + usize::from(
                self.writer.as_ref().is_some_and(|writer| !writer.handle.raw().0.is_null()),
            )
    }
}
fn check_context(original: &Option<JobContext>, current: &JobContext) -> Result<(), ClaimError> {
    let first =
        original.as_ref().ok_or_else(|| ClaimError::invalid("diagnostic_unbound_context"))?;
    if first.owner_id() != current.owner_id()
        || first.scope_id() != current.scope_id()
        || first.deadline().is_none()
        || current.deadline().is_none()
        || current.deadline() > first.deadline()
    {
        return Err(ClaimError::invalid("diagnostic_changed_context"));
    }
    super::super::check(first)?;
    super::super::check(current)
}
