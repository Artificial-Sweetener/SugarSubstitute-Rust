// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the producing file output in its pre-reserved durable capsule.
use super::*;
use crate::durable_storage::protected::native_phase::NativePhase;
use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
impl NativeFile {
    /// Open only the immutable grant and adopt before inspecting object facts.
    /// # Errors
    /// Every native/validation failure retains this same cell and original context.
    pub(crate) fn open(
        &mut self,
        sid: Option<&[u8]>,
        original: &JobContext,
        context: &JobContext,
        phase: &mut NativePhase<'_>,
    ) -> Result<(), ClaimError> {
        if self.fields()?.attempted || self.fields()?.handle.is_armed() {
            return Err(phase.invalid("diagnostic_already_opened"));
        }
        self.fields_mut()?.attempted = true;
        if self.fields()?.original.is_none() {
            self.fields_mut()?.original = Some(original.clone());
        }
        self.check(original)?;
        phase.check_budget(&self.fields()?.handle.budget())?;
        let result = self.open_once(sid, context, phase);
        let result = phase.post_context_for(result, context, &mut self.fields_mut()?.causes);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    fn open_once(
        &mut self,
        sid: Option<&[u8]>,
        context: &JobContext,
        phase: &mut NativePhase<'_>,
    ) -> Result<(), ClaimError> {
        phase.context(context)?;
        let Fields { handle, path, grant, causes, .. } = self.fields_mut()?;
        let mut phase = phase.for_causes(causes)?;
        let stage = *grant == FileGrant::WritableStage;
        if stage && sid.is_none() {
            return Err(phase.invalid("diagnostic_stage_account"));
        }
        let mut producing = phase.open_buffers(path.path()?, if stage { sid } else { None })?;
        let attributes = producing.attributes();
        let access = match *grant {
            FileGrant::MetadataReadOnly | FileGrant::InventoryMetadata => {
                FILE_READ_ATTRIBUTES.0 | READ_CONTROL.0
            }
            FileGrant::DirectoryScan => {
                FILE_LIST_DIRECTORY.0 | FILE_READ_ATTRIBUTES.0 | READ_CONTROL.0
            }
            FileGrant::ReadOnly => FILE_GENERIC_READ.0,
            FileGrant::WriterReadDelete => FILE_GENERIC_READ.0 | DELETE.0,
            FileGrant::WritableStage | FileGrant::WritableRecovery => {
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | DELETE.0
            }
        };
        let share = FILE_SHARE_READ
            | match *grant {
                FileGrant::DirectoryScan => FILE_SHARE_WRITE,
                FileGrant::InventoryMetadata => FILE_SHARE_DELETE,
                _ => FILE_SHARE_MODE(0),
            };
        let flags = FILE_FLAG_OPEN_REPARSE_POINT
            | if matches!(*grant, FileGrant::DirectoryScan) {
                FILE_FLAG_BACKUP_SEMANTICS
            } else {
                FILE_FLAGS_AND_ATTRIBUTES(0)
            };
        producing.context(context)?;
        producing.check()?;
        // SAFETY: Bounded canonical name and optional protected descriptor remain
        // live. The SDK wrapper returns Err only for INVALID_HANDLE_VALUE. CREATE_NEW
        // never replaces. Content reads exclude writes/deletion; metadata scan grants
        // preserve the original writer's explicit directory/delete sharing.
        let opened = unsafe {
            CreateFileW(
                PCWSTR(producing.units().as_ptr()),
                access,
                share,
                attributes.as_ref().map(|value| value as *const _),
                if stage { CREATE_NEW } else { OPEN_EXISTING },
                flags,
                None,
            )
        };
        match opened {
            Ok(raw) => handle.adopt_output(raw),
            Err(cause) => {
                handle.adopt_output(HANDLE::default());
                handle.release_unused_output()?;
                drop(producing);
                let operation = match *grant {
                    FileGrant::MetadataReadOnly => "diagnostic_absence",
                    FileGrant::InventoryMetadata => "diagnostic_scan_entry",
                    FileGrant::DirectoryScan => "diagnostic_scan_open",
                    _ => "diagnostic_open",
                };
                return Err(phase.error(operation, BoundedNativeCause::Windows(cause)));
            }
        }
        drop(producing);
        if matches!(*grant, FileGrant::DirectoryScan) {
            paths::validate_object(handle, true, true, &mut phase)?;
        } else {
            paths::validate_regular(handle, None, &mut phase)?;
        }
        if let Some(sid) = sid {
            security::verify(handle, sid, &mut phase)?;
        }
        let destination = phase.name_destination()?;
        if paths::final_path(handle, &mut phase, destination)?.path()? != path.path()? {
            return Err(phase.invalid("diagnostic_changed_path"));
        }
        phase.context(context)
    }
}
