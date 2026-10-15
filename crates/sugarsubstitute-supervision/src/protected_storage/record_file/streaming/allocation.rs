// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive the closed Stream roles from their actual owning allocation types.
use super::*;
use crate::native_claims::files::{NativeLayoutFailure, NativeRoleLayout};
use std::alloc::Layout;

impl StreamingPort {
    /// Describe its real Box and sole fixed native-file cells before construction.
    /// Inline native-file wrappers are counted here; each file owns its Fields Box.
    pub(crate) fn diagnostic_owner_layout() -> Result<NativeRoleLayout, NativeLayoutFailure> {
        let cells = Layout::array::<NativeFile>(FILE_SLOTS)
            .map_err(|_| NativeLayoutFailure::Overflow)?
            .size();
        let bytes =
            Layout::new::<Self>().size().checked_add(cells).ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: bytes, peak: bytes })
    }
    /// Describe one complete bounded native stage-candidate generation.
    /// A successor is admitted while its predecessor's actual allocations remain.
    pub(crate) fn diagnostic_scan_state_layout() -> Result<NativeRoleLayout, NativeLayoutFailure> {
        let cells = Layout::array::<StageCandidate>(DIAGNOSTIC_SCAN_BOUND)
            .map_err(|_| NativeLayoutFailure::Overflow)?
            .size();
        let leaves = DIAGNOSTIC_SCAN_BOUND
            .checked_mul(leaf::LEAF_BYTES)
            .ok_or(NativeLayoutFailure::Overflow)?;
        let observations =
            Layout::array::<diagnostic::inventory::ObservedEntry>(DIAGNOSTIC_SCAN_BOUND)
                .map_err(|_| NativeLayoutFailure::Overflow)?
                .size();
        let bytes = cells
            .checked_add(leaves)
            .and_then(|bytes| bytes.checked_add(observations))
            .and_then(|bytes| bytes.checked_add(DiagnosticNativeScanRetention::control_bytes()))
            .and_then(|bytes| bytes.checked_add(size_of::<DiagnosticNativeScanRetention>()))
            .ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: bytes, peak: bytes })
    }
    /// Describe the same page session through recognized-leaf native inspection.
    /// Its page remains alive during canonical decoding and protection checks.
    pub(crate) fn diagnostic_scan_page_layout() -> Result<NativeRoleLayout, NativeLayoutFailure> {
        let peak = 327_863_usize
            .checked_add(size_of::<session::Session>())
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<u64>>()))
            .and_then(|bytes| bytes.checked_add(size_of::<String>()))
            .ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: 0, peak })
    }
}

impl StreamingPort {
    pub(crate) fn admit(original: DiagnosticNativeGrant) -> Result<OwnedPort, ClaimError> {
        let allocation = original.streaming_owner().map_err(ClaimError::diagnostic_allocation)?;
        let mut causes = NativeCauseDestinations::diagnostic(&original)?;
        let root = ProtectedStorageRoot::from_diagnostic_grant(original.clone())
            .map_err(ClaimError::diagnostic_allocation)?;
        let mut files = Vec::new();
        files.try_reserve_exact(FILE_SLOTS).map_err(|cause| {
            causes.error("diagnostic_native_reservation", BoundedNativeCause::Reserve(cause), false)
        })?;
        if files.capacity() != FILE_SLOTS {
            return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
        }
        let port = Self {
            root,
            directory: None,
            namespace: None,
            sid: None,
            writer: None,
            files,
            next: 1,
            stage_candidates: None,
            stage_scan_complete: false,
            original_context: None,
            causes,
            original,
        };
        Ok(OwnedPort::new(Box::new(port), Some(allocation)))
    }
    pub(super) fn reject(&self) -> ClaimError {
        ClaimError::diagnostic_allocation(self.original.reject_native_state())
    }
    pub(super) fn invalid(&mut self, operation: &'static str) -> ClaimError {
        self.causes.error(
            operation,
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::InvalidData)),
            false,
        )
    }
    pub(super) fn check(&self, context: &JobContext) -> Result<(), ClaimError> {
        self.original.check(context).map_err(ClaimError::diagnostic_allocation)?;
        self.causes.check(false)?;
        if self.live_handles() >= DIAGNOSTIC_HANDLE_BOUND {
            return Err(self.reject());
        }
        Ok(())
    }
    pub(super) fn name(&self, leaf: &str, context: &JobContext) -> Result<NativeName, ClaimError> {
        let directory = self.directory.as_ref().ok_or_else(|| self.reject())?;
        let mut session = session::Session::path(&self.original, context)?;
        session.name(&directory.path, leaf, context)
    }
    pub(super) fn path(
        &self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<NativeName, ClaimError> {
        let namespace = self.namespace.ok_or_else(|| self.reject())?;
        if object.namespace().is_some_and(|actual| actual != namespace) {
            return Err(self.reject());
        }
        let leaf = leaf::Leaf::object(object, namespace).map_err(|_| self.reject())?;
        self.name(leaf.text().map_err(|_| self.reject())?, context)
    }
    pub(super) fn index(&self, id: NativeId) -> Result<usize, ClaimError> {
        for (index, file) in self.files.iter().enumerate() {
            if file.fields()?.id == id {
                return Ok(index);
            }
        }
        Err(self.reject())
    }
    pub(super) fn reserve_file(
        &mut self,
        name: NativeName,
        bound: u64,
        grant: FileGrant,
        current: &JobContext,
    ) -> Result<usize, ClaimError> {
        self.check(current)?;
        if self.files.len() >= FILE_SLOTS {
            return Err(self.reject());
        }
        let id = self.next;
        let next = id.checked_add(1).ok_or_else(|| self.reject())?;
        let original = self.original_context.as_ref().ok_or_else(|| self.reject())?.clone();
        let allocation =
            self.original.native_file_owner().map_err(ClaimError::diagnostic_allocation)?;
        let causes = NativeCauseDestinations::diagnostic(&self.original)?;
        let admission = NativeFileAdmission::Diagnostic {
            original,
            grant: self.original.clone(),
            allocation,
            causes,
        };
        let file = NativeFile::reserve(id, bound, grant, &mut Some((name, admission)))?;
        let index = self.files.len();
        self.files.push(file);
        self.next = next;
        Ok(index)
    }
}
