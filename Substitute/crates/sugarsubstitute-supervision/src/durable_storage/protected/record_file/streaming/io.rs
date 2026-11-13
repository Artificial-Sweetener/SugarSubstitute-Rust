// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt actual file outputs before checking protection and bounded streaming facts.
use super::*;
use windows::Win32::Storage::FileSystem::*;
impl StreamingPort {
    pub(super) fn open_owned(
        &mut self,
        path: NativeName,
        bound: u64,
        stage: bool,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.check(context)?;
        if !stage {
            let mut retained = None;
            for file in &self.files {
                let fields = file.fields()?;
                if fields.path.path()? == path.path()?
                    && file.writable()?
                    && !fields.stage
                    && !fields.deleted
                    && file.has_output()
                {
                    if retained.is_some() {
                        return Err(self.reject());
                    }
                    retained = Some(fields.id);
                }
            }
            if let Some(id) = retained {
                let actual = self.observe_file(id, context)?;
                if actual.bytes > bound {
                    return Err(self.invalid("diagnostic_size_bound"));
                }
                let index = self.index(id)?;
                self.files[index].fields_mut()?.facts = Some(actual);
                return Ok((id, actual));
            }
        }
        self.open_file(path, bound, stage, stage, context)
    }
    pub(super) fn open_file(
        &mut self,
        path: NativeName,
        bound: u64,
        stage: bool,
        writable: bool,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.check(context)?;
        if bound == 0 {
            return Err(self.invalid("diagnostic_file_capacity"));
        }
        let grant = match (stage, writable) {
            (true, true) => FileGrant::WritableStage,
            (false, true) => FileGrant::WritableRecovery,
            (false, false) => FileGrant::WriterReadDelete,
            (true, false) => return Err(self.invalid("diagnostic_stage_grant")),
        };
        let mut session = session::Session::path(&self.original, context)?;
        let index = self.reserve_file(path, bound, grant, context)?;
        let original = self.original_context.as_ref().ok_or_else(|| self.reject())?;
        let sid = self.sid.as_ref().ok_or_else(|| self.reject())?.bytes()?;
        let mut phase = session.phase(context);
        self.files[index].open(Some(sid), original, context, &mut phase)?;

        drop(session);
        let facts = self.observe(index, context)?;
        self.files[index].fields_mut()?.facts = Some(facts);
        self.check(context)?;
        Ok((self.files[index].fields()?.id, facts))
    }
    pub(super) fn observe(
        &mut self,
        index: usize,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.check(context)?;
        let mut session = session::Session::path(&self.original, context)?;
        let sid = self.sid.as_ref().ok_or_else(|| self.reject())?.bytes()?;
        let facts = self.files[index].observe(Some(sid), context, &mut session.phase(context))?;
        self.check(context)?;
        Ok(facts)
    }
    pub(super) fn read_owned(
        &mut self,
        id: NativeId,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let mut session = session::Session::path(&self.original, context)?;
        let actual =
            self.files[index].read(offset, output, context, &mut session.phase(context))?;
        self.check(context)?;
        Ok(actual)
    }
    pub(super) fn write_owned(
        &mut self,
        id: NativeId,
        offset: u64,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        let file = &mut self.files[index];
        let fields = file.fields()?;
        if !fields.stage
            || fields.deleted
            || input.len() > DIAGNOSTIC_IO_BYTES
            || offset.checked_add(input.len() as u64).is_none_or(|end| end > fields.bound)
        {
            return Err(phase.invalid("diagnostic_write_range"));
        }
        let original = fields.facts.ok_or_else(|| phase.invalid("diagnostic_unvalidated_stage"))?;
        if paths::file_identity(&fields.handle, &mut phase)? != original.identity {
            return Err(phase.invalid("diagnostic_changed_stage"));
        }
        seek(&fields.handle, offset, &mut phase)?;
        phase.context(context)?;
        let mut written = 0;
        // SAFETY: The caller's exact bounded initialized input and retained synchronous
        // file remain live through the call and unconditional post-control observation.
        let result =
            unsafe { WriteFile(fields.handle.raw(), Some(input), Some(&mut written), None) }
                .map_err(|cause| {
                    phase.error("diagnostic_write", BoundedNativeCause::Windows(cause))
                });
        let result = result.and_then(|()| {
            if written as usize == input.len() {
                Ok(())
            } else {
                Err(phase.invalid("diagnostic_short_write"))
            }
        });
        phase.post_context(result, context)?;
        self.check(context)
    }
    pub(super) fn flush_owned(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let mut session = session::Session::path(&self.original, context)?;
        let mut phase = session.phase(context);
        phase.context(context)?;
        // SAFETY: This actual retained synchronous writable file remains owned
        // through both the flush result and unconditional post-control observation.
        let result = unsafe { FlushFileBuffers(self.files[index].fields()?.handle.raw()) }
            .map_err(|cause| phase.error("diagnostic_flush", BoundedNativeCause::Windows(cause)));
        phase.post_context(result, context)?;

        drop(session);
        let actual = self.observe(index, context)?;
        if self.files[index].fields()?.facts.is_some_and(|prior| prior.identity != actual.identity)
        {
            return Err(self.invalid("diagnostic_changed_stage"));
        }
        self.files[index].fields_mut()?.facts = Some(actual);
        Ok(actual)
    }
}
