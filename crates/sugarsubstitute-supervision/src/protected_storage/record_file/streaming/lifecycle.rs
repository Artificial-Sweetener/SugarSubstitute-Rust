// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle exact activation and by-handle disposition under retained exclusion.
use super::*;
use windows::Win32::Storage::FileSystem::*;
impl StreamingPort {
    fn writer_check(&self, context: &JobContext) -> Result<(), ClaimError> {
        self.check(context)?;
        if !self.writer.as_ref().is_some_and(HeldFile::is_locked) {
            return Err(self.reject());
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
        let target = self.path(object, context)?;
        match expected {
            DiagnosticPredecessor::Absent => {
                if !self.absent_owned(object, context)? {
                    return Err(self.invalid("diagnostic_existing_predecessor"));
                }
            }
            DiagnosticPredecessor::Exact { file_identity, sha256 } => {
                let (prior, facts) =
                    self.open_owned(target.clone(), object.bound(), false, context)?;
                if facts.identity != file_identity || facts.sha256 != sha256 {
                    return Err(self.invalid("diagnostic_changed_predecessor"));
                }
                self.close(prior, context)?;
            }
        }
        let index = self.index(id)?;
        let fields = self.files[index].fields()?;
        if !fields.stage || fields.deleted {
            return Err(self.reject());
        }
        let prior = fields.facts.ok_or_else(|| self.reject())?;
        if self.observe(index, context)? != prior {
            return Err(self.invalid("diagnostic_changed_stage"));
        }
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        let fields = self.files[index].fields_mut()?;
        let directory =
            self.directory.as_ref().ok_or_else(|| phase.invalid("diagnostic_rename_directory"))?;
        let result = self.root.rename_diagnostic_file(
            directory,
            &fields.handle,
            target.path()?,
            matches!(expected, DiagnosticPredecessor::Exact { .. }),
            &mut phase,
        );
        if result.is_ok() {
            fields.path = target;
        }
        phase.post_context(result, context)?;

        drop(session);
        let actual = self.flush_owned(id, context)?;
        if actual != prior {
            return Err(self.invalid("diagnostic_changed_activation"));
        }
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        let destination = phase.name_destination()?;
        let actual_path =
            paths::final_path(&self.files[index].fields()?.handle, &mut phase, destination)?;
        if actual_path.path()? != self.files[index].fields()?.path.path()? {
            return Err(phase.invalid("diagnostic_changed_activation_path"));
        }
        drop(actual_path);

        drop(session);
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
            return Err(self.reject());
        };
        let path = self.path(object, context)?;
        let mut retained = None;
        for file in &self.files {
            let fields = file.fields()?;
            if fields.path.path()? == path.path()?
                && file.writable()?
                && !fields.deleted
                && file.has_output()
            {
                if retained.is_some() {
                    return Err(self.reject());
                }
                retained = Some(fields.id);
            }
        }
        let (id, facts) = if let Some(id) = retained {
            let index = self.index(id)?;
            let mut session = session::Session::path(&self.original, context)?;
            let mut phase = session.phase(context);
            let destination = phase.name_destination()?;
            let actual_path =
                paths::final_path(&self.files[index].fields()?.handle, &mut phase, destination)?;
            if actual_path.path()? != path.path()? {
                return Err(phase.invalid("diagnostic_changed_reconcile_path"));
            }
            drop(actual_path);

            drop(session);
            let facts = self.observe(index, context)?;
            self.files[index].fields_mut()?.facts = Some(facts);
            (id, facts)
        } else {
            self.open_file(path, object.bound(), false, true, context)?
        };
        if facts.identity != file_identity || facts.sha256 != sha256 {
            return Err(self.invalid("diagnostic_changed_reconcile"));
        }
        let actual = self.flush_owned(id, context)?;
        if actual != facts {
            return Err(self.invalid("diagnostic_changed_reconcile"));
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
        let target = self.path(object, context)?;
        let fields = self.files[index].fields()?;
        if fields.path.path()? != target.path()? || fields.facts != Some(expected) {
            return Err(self.reject());
        }
        if fields.deleted {
            return Ok(());
        }
        if self.observe(index, context)? != expected {
            return Err(self.invalid("diagnostic_changed_deletion"));
        }
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        let information = FILE_DISPOSITION_INFO { DeleteFile: true };
        phase.context(context)?;
        let fields = self.files[index].fields_mut()?;
        // SAFETY: The exact retained DELETE-capable file and initialized disposition
        // remain alive through the synchronous native result and its adoption.
        let result = unsafe {
            SetFileInformationByHandle(
                fields.handle.raw(),
                FileDispositionInfo,
                (&information as *const FILE_DISPOSITION_INFO).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        }
        .map_err(|cause| phase.error("diagnostic_disposition", BoundedNativeCause::Windows(cause)));
        if result.is_ok() {
            fields.deleted = true;
        }
        phase.post_context(result, context)
    }
    pub(super) fn absent_owned(
        &mut self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<bool, ClaimError> {
        self.check(context)?;
        let path = self.path(object, context)?;
        let mut session = session::Session::path(&self.original, context)?;
        let index = self.reserve_file(path.clone(), 0, FileGrant::MetadataReadOnly, context)?;
        let original = self.original_context.as_ref().ok_or_else(|| self.reject())?;
        let sid = self.sid.as_ref().ok_or_else(|| self.reject())?.bytes()?;
        let opened =
            self.files[index].open(Some(sid), original, context, &mut session.phase(context));
        if let Err(cause) = opened {
            if self.files[index].missing_without_output(&cause) {
                self.files.remove(index);
                self.check(context)?;
                let mut index = 0;
                while index < self.files.len() {
                    let fields = self.files[index].fields()?;
                    if fields.path.path()? == path.path()?
                        && fields.deleted
                        && !self.files[index].has_output()
                    {
                        self.files.remove(index);
                    } else {
                        index += 1;
                    }
                }
                return Ok(true);
            }
            return Err(cause);
        }
        let id = self.files[index].fields()?.id;
        drop(session);
        self.close(id, context)?;
        Ok(false)
    }
}
