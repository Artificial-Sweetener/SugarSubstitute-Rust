// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enumerate bounded native directory pages and preserve every unknown candidate.
mod names;
use super::*;
use crate::diagnostic_storage::inventory::{ObservedEntry, Scan};
use names::classify;
use windows::Win32::{Foundation::ERROR_NO_MORE_FILES, Storage::FileSystem::*};
impl StreamingPort {
    pub(super) fn scan_owned(&mut self, context: &JobContext) -> Result<Scan, ClaimError> {
        self.check(context)?;
        self.stage_scan_complete = false;
        let retention =
            self.original.scan_retention(context).map_err(ClaimError::diagnostic_allocation)?;
        let mut session = session::Session::scan(&self.original, context)?;
        let mut stages = NativeStageScan { candidates: Vec::new(), _retention: retention.clone() };
        let mut scan =
            Scan { entries: Vec::new(), complete: false, _native_retention: Some(retention) };
        stages.candidates.try_reserve_exact(DIAGNOSTIC_SCAN_BOUND).map_err(|cause| {
            session
                .phase(context)
                .error("diagnostic_stage_scan_reservation", BoundedNativeCause::Reserve(cause))
        })?;
        scan.entries.try_reserve_exact(DIAGNOSTIC_SCAN_BOUND).map_err(|cause| {
            session
                .phase(context)
                .error("diagnostic_scan_reservation", BoundedNativeCause::Reserve(cause))
        })?;
        if stages.candidates.capacity() != DIAGNOSTIC_SCAN_BOUND
            || scan.entries.capacity() != DIAGNOSTIC_SCAN_BOUND
        {
            return Err(session.phase(context).invalid("diagnostic_scan_capacity"));
        }
        let directory = self.directory.as_ref().ok_or_else(|| self.reject())?.path.clone();
        let index = self.reserve_file(directory.clone(), 0, FileGrant::DirectoryScan, context)?;
        let id = self.files[index].fields()?.id;
        let original = self.original_context.as_ref().ok_or_else(|| self.reject())?;
        let sid = self.sid.as_ref().ok_or_else(|| self.reject())?.bytes()?;
        self.files[index].open(Some(sid), original, context, &mut session.phase(context))?;
        let mut buffer = Vec::<u64>::new();
        buffer.try_reserve_exact(DIAGNOSTIC_IO_BYTES / size_of::<u64>()).map_err(|cause| {
            session
                .phase(context)
                .error("diagnostic_scan_page_reservation", BoundedNativeCause::Reserve(cause))
        })?;
        if buffer.capacity() != DIAGNOSTIC_IO_BYTES / size_of::<u64>() {
            return Err(session.phase(context).invalid("diagnostic_scan_page_capacity"));
        }
        buffer.resize(DIAGNOSTIC_IO_BYTES / size_of::<u64>(), 0);
        let mut first = true;
        let mut complete = true;
        let mut scanned = 0_usize;
        'pages: loop {
            self.check(context)?;
            session.phase(context).context(context)?;
            let index = self.index(id)?;
            // SAFETY: Aligned initialized64KiB output is advertised exactly. The
            // retained synchronous directory handle alone owns enumeration state.
            let page = unsafe {
                GetFileInformationByHandleEx(
                    self.files[index].fields()?.handle.raw(),
                    if first { FileIdBothDirectoryRestartInfo } else { FileIdBothDirectoryInfo },
                    buffer.as_mut_ptr().cast(),
                    DIAGNOSTIC_IO_BYTES as u32,
                )
            };
            first = false;
            let page = match page {
                Err(cause) if cause.code() == ERROR_NO_MORE_FILES.to_hresult() => Ok(false),
                Err(cause) => Err(session
                    .phase(context)
                    .error("diagnostic_scan", BoundedNativeCause::Windows(cause))),
                Ok(()) => Ok(true),
            };
            if !session.phase(context).post_context(page, context)? {
                break;
            }
            let mut offset = 0;
            loop {
                let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
                if offset + header > DIAGNOSTIC_IO_BYTES || !offset.is_multiple_of(8) {
                    return Err(session.phase(context).invalid("diagnostic_scan_record"));
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
                    .ok_or_else(|| session.phase(context).invalid("diagnostic_scan_name"))?;
                if !name_bytes.is_multiple_of(2)
                    || (next != 0 && (next < header + name_bytes || !next.is_multiple_of(8)))
                {
                    return Err(session.phase(context).invalid("diagnostic_scan_record"));
                }
                // SAFETY: Exact UTF16 byte range and alignment are bounded above.
                let name_units = unsafe {
                    std::slice::from_raw_parts(
                        buffer.as_ptr().cast::<u8>().add(offset + header).cast::<u16>(),
                        (end - offset - header) / 2,
                    )
                };
                let name = String::from_utf16(name_units).map_err(|cause| {
                    session
                        .phase(context)
                        .error("diagnostic_scan_unicode", BoundedNativeCause::Unicode(cause))
                })?;
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
                    if scan.entries.len() == DIAGNOSTIC_SCAN_BOUND {
                        complete = false;
                        break 'pages;
                    }
                    let classified = classify(&name);
                    let (digest, class) = (classified.digest, classified.class);
                    let regular = info.FileAttributes
                        & (FILE_ATTRIBUTE_DIRECTORY.0 | FILE_ATTRIBUTE_REPARSE_POINT.0)
                        == 0;
                    let mut observed = ObservedEntry {
                        digest,
                        class,
                        identity: [0; 24],
                        bytes: u64::try_from(info.EndOfFile).unwrap_or(u64::MAX),
                        protected_regular: false,
                        payload_group: classified.payload_group,
                    };
                    if regular && class != DiagnosticEntryClass::Unknown {
                        let path = session.name(&directory, &name, context)?;
                        let inspected = self.inspect_entry(path, context, &mut session);
                        match inspected {
                            Ok((identity, bytes)) => {
                                observed.identity = identity;
                                observed.bytes = bytes;
                                observed.protected_regular = true;
                                if class == DiagnosticEntryClass::Stage {
                                    let mut retained_leaf = String::new();
                                    if name.len() > leaf::LEAF_BYTES {
                                        return Err(session
                                            .phase(context)
                                            .invalid("diagnostic_stage_leaf_bound"));
                                    }
                                    retained_leaf.try_reserve_exact(name.len()).map_err(
                                        |cause| {
                                            session.phase(context).error(
                                                "diagnostic_stage_leaf_reservation",
                                                BoundedNativeCause::Reserve(cause),
                                            )
                                        },
                                    )?;
                                    retained_leaf.push_str(&name);
                                    if retained_leaf.capacity() > leaf::LEAF_BYTES {
                                        return Err(session
                                            .phase(context)
                                            .invalid("diagnostic_stage_leaf_capacity"));
                                    }
                                    stages.candidates.push(StageCandidate {
                                        leaf: retained_leaf,
                                        identity,
                                        bytes,
                                    });
                                }
                            }
                            Err(cause) => return Err(cause),
                        }
                    }
                    scan.entries.push(observed);
                }
                if next == 0 {
                    break;
                }
                offset = offset
                    .checked_add(next)
                    .filter(|value| *value < DIAGNOSTIC_IO_BYTES)
                    .ok_or_else(|| session.phase(context).invalid("diagnostic_scan_next"))?;
            }
        }
        self.close(id, context)?;
        self.stage_candidates = Some(stages);
        self.stage_scan_complete = complete;
        scan.complete = complete;
        Ok(scan)
    }
    fn inspect_entry(
        &mut self,
        path: NativeName,
        context: &JobContext,
        session: &mut session::Session,
    ) -> Result<([u8; 24], u64), ClaimError> {
        self.check(context)?;
        let index = self.reserve_file(path, 0, FileGrant::InventoryMetadata, context)?;
        let id = self.files[index].fields()?.id;
        let original = self.original_context.as_ref().ok_or_else(|| self.reject())?;
        let sid = self.sid.as_ref().ok_or_else(|| self.reject())?.bytes()?;
        let mut phase = session.phase(context);
        self.files[index].open(Some(sid), original, context, &mut phase)?;
        let file = self.files[index].fields()?;
        paths::validate_regular(&file.handle, None, &mut phase)?;
        security::verify(&file.handle, sid, &mut phase)?;
        let destination = phase.name_destination()?;
        let actual = paths::final_path(&file.handle, &mut phase, destination)?;
        if actual.path()? != file.path.path()? {
            return Err(phase.invalid("diagnostic_changed_scan_entry"));
        }
        drop(actual);
        let identity = paths::file_identity(&file.handle, &mut phase)?;
        let mut bytes = 0_i64;
        phase.context(context)?;
        // SAFETY: This initialized exact size output and actual retained file
        // remain live through the native result and unconditional post-control.
        let result = unsafe { GetFileSizeEx(file.handle.raw(), &mut bytes) }.map_err(|cause| {
            phase.error("diagnostic_scan_size", BoundedNativeCause::Windows(cause))
        });
        phase.post_context(result, context)?;
        let bytes = u64::try_from(bytes).map_err(|cause| {
            phase.error("diagnostic_scan_size", BoundedNativeCause::Integer(cause))
        })?;

        self.close(id, context)?;
        Ok((identity, bytes))
    }
}
