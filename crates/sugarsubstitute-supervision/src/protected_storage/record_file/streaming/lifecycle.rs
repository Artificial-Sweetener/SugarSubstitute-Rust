// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle exact activation and by-handle disposition under retained exclusion.
use super::*;
use windows::{
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, Storage::FileSystem::*},
    core::PCWSTR,
};
impl StreamingPort {
    fn writer_check(&self, context: &JobContext) -> Result<(), ClaimError> {
        self.check(context)?;
        if !self.writer.as_ref().is_some_and(HeldFile::is_locked) {
            return Err(ClaimError::invalid("diagnostic_no_exclusion"));
        }
        Ok(())
    }
    pub(super) fn activate_owned(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.writer_check(context)?;
        let target = self.path(object)?;
        match expected {
            DiagnosticPredecessor::Absent => {
                if !self.absent_owned(object, context)? {
                    return Err(ClaimError::invalid("diagnostic_existing_predecessor"));
                }
            }
            DiagnosticPredecessor::Exact { file_identity, sha256 } => {
                let (prior, facts) =
                    self.open_owned(target.clone(), object.bound(), false, context)?;
                if facts.identity != file_identity || facts.sha256 != sha256 {
                    return Err(ClaimError::invalid("diagnostic_changed_predecessor"));
                }
                self.close(prior, context)?;
            }
        }
        let index = self.index(id)?;
        if !self.files[index].stage || self.files[index].deleted {
            return Err(ClaimError::invalid("diagnostic_nonstage_activation"));
        }
        let prior = self.files[index]
            .facts
            .ok_or_else(|| ClaimError::invalid("diagnostic_unflushed_stage"))?;
        if self.observe(index, context)? != prior {
            return Err(ClaimError::invalid("diagnostic_changed_stage"));
        }
        let units = paths::wide(&target)?;
        self.check(context)?;
        super::super::activate(
            &self.files[index].handle,
            &units,
            matches!(expected, DiagnosticPredecessor::Exact { .. }),
        )?;
        self.files[index].path = target;
        let actual = self.flush_owned(id, context)?;
        if actual != prior
            || paths::final_path(&self.files[index].handle)? != self.files[index].path
        {
            return Err(ClaimError::invalid("diagnostic_changed_activation"));
        }
        self.close(id, context)?;
        Ok(actual)
    }
    pub(super) fn reconcile_owned(
        &mut self,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.writer_check(context)?;
        let DiagnosticPredecessor::Exact { file_identity, sha256 } = expected else {
            return Err(ClaimError::invalid("diagnostic_unchecked_reconcile"));
        };
        let path = self.path(object)?;
        let mut retained = self.files.iter().filter(|file| {
            file.path == path && file.writable && !file.deleted && !file.handle.raw().0.is_null()
        });
        let id = retained.next().map(|file| file.id);
        if retained.next().is_some() {
            return Err(ClaimError::invalid("diagnostic_ambiguous_reconcile"));
        }
        let (id, facts) = if let Some(id) = id {
            let index = self.index(id)?;
            if paths::final_path(&self.files[index].handle)? != path {
                return Err(ClaimError::invalid("diagnostic_changed_reconcile_path"));
            }
            let facts = self.observe(index, context)?;
            self.files[index].facts = Some(facts);
            (id, facts)
        } else {
            self.open_file(path, object.bound(), false, true, context)?
        };
        if facts.identity != file_identity || facts.sha256 != sha256 {
            return Err(ClaimError::invalid("diagnostic_changed_reconcile"));
        }
        let actual = self.flush_owned(id, context)?;
        if actual != facts {
            return Err(ClaimError::invalid("diagnostic_changed_reconcile"));
        }
        self.close(id, context)?;
        Ok(actual)
    }
    pub(super) fn delete_owned(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticReadFacts,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.writer_check(context)?;
        let index = self.index(id)?;
        if self.files[index].path != self.path(object)? || self.files[index].facts != Some(expected)
        {
            return Err(ClaimError::invalid("diagnostic_changed_deletion"));
        }
        if self.files[index].deleted {
            return Ok(());
        }
        if self.observe(index, context)? != expected {
            return Err(ClaimError::invalid("diagnostic_changed_deletion"));
        }
        let information = FILE_DISPOSITION_INFO { DeleteFile: true };
        self.check(context)?;
        // SAFETY: The exact retained DELETE-capable file is validated immediately
        // above. Initialized native disposition remains live through the call.
        unsafe {
            SetFileInformationByHandle(
                self.files[index].handle.raw(),
                FileDispositionInfo,
                (&information as *const FILE_DISPOSITION_INFO).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        }
        .map_err(|cause| ClaimError::new("diagnostic_disposition", cause))?;
        self.files[index].deleted = true;
        self.check(context)
    }
    pub(super) fn absent_owned(
        &mut self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<bool, ClaimError> {
        self.check(context)?;
        let path = self.path(object)?;
        let units = paths::wide(&path)?;
        let id = self.next;
        self.next =
            id.checked_add(1).ok_or_else(|| ClaimError::invalid("diagnostic_lease_exhausted"))?;
        if self.files.len() >= FILE_SLOTS {
            return Err(ClaimError::invalid("diagnostic_file_capacity"));
        }
        // SAFETY: The pinned protected directory and bounded canonical name stay
        // retained; OPEN_EXISTING cannot create or mutate a missing target.
        let result = unsafe {
            CreateFileW(
                PCWSTR(units.as_ptr()),
                FILE_READ_ATTRIBUTES.0 | READ_CONTROL.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        };
        let raw = match result {
            Err(cause) if cause.code() == ERROR_FILE_NOT_FOUND.to_hresult() => {
                self.check(context)?;
                self.files.retain(|file| {
                    file.path != path || !file.deleted || !file.handle.raw().0.is_null()
                });
                return Ok(true);
            }
            Err(cause) => return Err(ClaimError::new("diagnostic_absence", cause)),
            Ok(raw) => raw,
        };
        let index = self.files.len();
        self.files.push(NativeFile {
            id,
            handle: Handle::new(raw, Budget::Protected(self.memory.clone())),
            path,
            facts: None,
            bound: 0,
            stage: false,
            writable: false,
            deleted: false,
        });
        paths::validate_regular(&self.files[index].handle, None)?;
        security::verify(&self.files[index].handle, &self.sid)?;
        self.close(id, context)?;
        Ok(false)
    }
}
