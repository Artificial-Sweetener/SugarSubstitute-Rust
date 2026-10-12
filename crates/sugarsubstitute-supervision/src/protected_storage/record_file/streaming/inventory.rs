// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enumerate bounded native directory pages and preserve every unknown candidate.
mod names;
use super::*;
use crate::diagnostic_storage::inventory::{ObservedEntry, Scan};
use names::classify;
use windows::{
    Win32::{Foundation::ERROR_NO_MORE_FILES, Storage::FileSystem::*},
    core::PCWSTR,
};
impl StreamingPort {
    pub(super) fn scan_owned(&mut self, context: &JobContext) -> Result<Scan, ClaimError> {
        self.check(context)?;
        self.stage_scan_complete = false;
        self.stage_candidates.clear();
        let mut stages = Vec::new();
        stages
            .try_reserve_exact(DIAGNOSTIC_SCAN_BOUND)
            .map_err(|cause| ClaimError::new("diagnostic_stage_scan_reservation", cause))?;
        let directory =
            self.directory.clone().ok_or_else(|| ClaimError::invalid("diagnostic_unresolved"))?;
        let units = paths::wide(&directory)?;
        let id = self.next;
        self.next =
            id.checked_add(1).ok_or_else(|| ClaimError::invalid("diagnostic_lease_exhausted"))?;
        if self.files.len() >= FILE_SLOTS {
            return Err(ClaimError::invalid("diagnostic_file_capacity"));
        }
        // SAFETY: The bounded pinned directory name stays live; the noninheritable
        // synchronous scan handle is immediately retained before metadata checks.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(units.as_ptr()),
                FILE_LIST_DIRECTORY.0 | FILE_READ_ATTRIBUTES.0 | READ_CONTROL.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|cause| ClaimError::new("diagnostic_scan_open", cause))?;
        let index = self.files.len();
        self.files.push(NativeFile {
            id,
            handle: Handle::new(raw, Budget::Protected(self.memory.clone())),
            path: directory.clone(),
            facts: None,
            bound: 0,
            stage: false,
            writable: false,
            deleted: false,
        });
        paths::validate_object(&self.files[index].handle, true, true)?;
        security::verify(&self.files[index].handle, &self.sid)?;
        if paths::final_path(&self.files[index].handle)? != directory {
            return Err(ClaimError::invalid("diagnostic_changed_scan_root"));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(DIAGNOSTIC_SCAN_BOUND)
            .map_err(|cause| ClaimError::new("diagnostic_scan_reservation", cause))?;
        let mut buffer = vec![0_u64; DIAGNOSTIC_IO_BYTES / 8];
        let mut first = true;
        let mut complete = true;
        let mut scanned = 0_usize;
        'pages: loop {
            self.check(context)?;
            let index = self.index(id)?;
            // SAFETY: Aligned initialized64KiB output is advertised exactly. The
            // retained synchronous directory handle alone owns enumeration state.
            let page = unsafe {
                GetFileInformationByHandleEx(
                    self.files[index].handle.raw(),
                    if first { FileIdBothDirectoryRestartInfo } else { FileIdBothDirectoryInfo },
                    buffer.as_mut_ptr().cast(),
                    DIAGNOSTIC_IO_BYTES as u32,
                )
            };
            first = false;
            match page {
                Err(cause) if cause.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(cause) => return Err(ClaimError::new("diagnostic_scan", cause)),
                Ok(()) => (),
            }
            let mut offset = 0;
            loop {
                let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
                if offset + header > DIAGNOSTIC_IO_BYTES || !offset.is_multiple_of(8) {
                    return Err(ClaimError::invalid("diagnostic_scan_record"));
                }
                // SAFETY: Offset alignment and header length were checked above;
                // Windows wrote this FILE_ID_BOTH_DIR_INFO into our owned page.
                let info = unsafe {
                    &*buffer.as_ptr().cast::<u8>().add(offset).cast::<FILE_ID_BOTH_DIR_INFO>()
                };
                let name_bytes = info.FileNameLength as usize;
                let next = info.NextEntryOffset as usize;
                let end = offset
                    .checked_add(header)
                    .and_then(|start| start.checked_add(name_bytes))
                    .filter(|end| *end <= DIAGNOSTIC_IO_BYTES)
                    .ok_or_else(|| ClaimError::invalid("diagnostic_scan_name"))?;
                if !name_bytes.is_multiple_of(2)
                    || (next != 0 && (next < header + name_bytes || !next.is_multiple_of(8)))
                {
                    return Err(ClaimError::invalid("diagnostic_scan_record"));
                }
                // SAFETY: Exact UTF16 byte range and alignment are bounded above.
                let name_units = unsafe {
                    std::slice::from_raw_parts(
                        buffer.as_ptr().cast::<u8>().add(offset + header).cast::<u16>(),
                        (end - offset - header) / 2,
                    )
                };
                let name = String::from_utf16(name_units)
                    .map_err(|cause| ClaimError::new("diagnostic_scan_unicode", cause))?;
                if name != "." && name != ".." {
                    scanned += 1;
                    if scanned > DIAGNOSTIC_SCAN_BOUND {
                        complete = false;
                        break 'pages;
                    }
                }
                let owned_lock =
                    name == "writer.lock" && self.writer.as_ref().is_some_and(HeldFile::is_locked);
                if name != "." && name != ".." && !owned_lock {
                    if entries.len() == DIAGNOSTIC_SCAN_BOUND {
                        complete = false;
                        break 'pages;
                    }
                    let (digest, class) = classify(&name);
                    let regular = info.FileAttributes
                        & (FILE_ATTRIBUTE_DIRECTORY.0 | FILE_ATTRIBUTE_REPARSE_POINT.0)
                        == 0;
                    let payload_group = (class == DiagnosticEntryClass::Payload
                        || (class == DiagnosticEntryClass::Stage
                            && name
                                .split_once('.')
                                .is_some_and(|(_, rest)| rest.starts_with("p-"))))
                    .then_some(digest);
                    let mut observed = ObservedEntry {
                        digest,
                        class,
                        identity: [0; 24],
                        bytes: u64::try_from(info.EndOfFile).unwrap_or(u64::MAX),
                        protected_regular: false,
                        payload_group,
                    };
                    if regular && class != DiagnosticEntryClass::Unknown {
                        match self.inspect_entry(directory.join(&name), context) {
                            Ok((identity, bytes)) => {
                                observed.identity = identity;
                                observed.bytes = bytes;
                                observed.protected_regular = true;
                                if class == DiagnosticEntryClass::Stage {
                                    stages.push(StageCandidate {
                                        leaf: name.clone(),
                                        identity,
                                        bytes,
                                    });
                                }
                            }
                            Err(cause) => return Err(cause),
                        }
                    }
                    entries.push(observed);
                }
                if next == 0 {
                    break;
                }
                offset = offset
                    .checked_add(next)
                    .filter(|value| *value < DIAGNOSTIC_IO_BYTES)
                    .ok_or_else(|| ClaimError::invalid("diagnostic_scan_next"))?;
            }
        }
        self.close(id, context)?;
        self.stage_candidates = stages;
        self.stage_scan_complete = complete;
        Ok(Scan { entries, complete })
    }
    fn inspect_entry(
        &mut self,
        path: PathBuf,
        context: &JobContext,
    ) -> Result<([u8; 24], u64), ClaimError> {
        self.check(context)?;
        let units = paths::wide(&path)?;
        let id = self.next;
        self.next =
            id.checked_add(1).ok_or_else(|| ClaimError::invalid("diagnostic_lease_exhausted"))?;
        if self.files.len() >= FILE_SLOTS {
            return Err(ClaimError::invalid("diagnostic_file_capacity"));
        }
        // SAFETY: OPEN_EXISTING is read-only and its bounded output is retained
        // before security/identity queries; reparse data never authorizes access.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(units.as_ptr()),
                FILE_READ_ATTRIBUTES.0 | READ_CONTROL.0,
                FILE_SHARE_READ | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|cause| ClaimError::new("diagnostic_scan_entry", cause))?;
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
        let file = &self.files[index];
        paths::validate_regular(&file.handle, None)?;
        security::verify(&file.handle, &self.sid)?;
        if paths::final_path(&file.handle)? != file.path {
            return Err(ClaimError::invalid("diagnostic_changed_scan_entry"));
        }
        let identity = paths::file_identity(&file.handle)?;
        let mut bytes = 0_i64;
        // SAFETY: Exact initialized size output and actual retained file remain live.
        unsafe { GetFileSizeEx(file.handle.raw(), &mut bytes) }
            .map_err(|cause| ClaimError::new("diagnostic_scan_size", cause))?;
        let bytes =
            u64::try_from(bytes).map_err(|cause| ClaimError::new("diagnostic_scan_size", cause))?;
        self.close(id, context)?;
        Ok((identity, bytes))
    }
}
