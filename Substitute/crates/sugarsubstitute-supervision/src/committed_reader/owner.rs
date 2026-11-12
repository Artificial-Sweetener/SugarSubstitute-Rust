// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preadmit the complete fixed reader graph before allocation or native effects.
use super::*;
use crate::installed_release::{ReleaseAuthority, allocation::CaptureRegion};
use sugarsubstitute_execution::CommittedReaderAdmission;
/// Own an existing-only transaction with one writer fence and three immutable data pins.
pub struct CommittedDiagnosticReader {
    pub(super) fields: Option<Box<Fields>>,
    pub(super) allocation: Option<SharedCaptureLoan>,
    pub(super) original: CommittedReaderOrigin,
}
fn shared(original: &AllocationOrigin, bytes: usize) -> Result<SharedCaptureLoan, ClaimError> {
    SharedCaptureLoan::new(
        original
            .reserve(CaptureRegion::Metadata, bytes + SharedCaptureLoan::control_bytes())
            .map_err(ClaimError::capture_allocation)?,
    )
    .map_err(ClaimError::capture_allocation)
}
pub(super) fn causes(
    original: &AllocationOrigin,
    index: u8,
) -> Result<NativeCauseDestinations, ClaimError> {
    Ok(NativeCauseDestinations::selected(
        original,
        original.cause_destination(index * 2).map_err(ClaimError::capture_allocation)?,
        original.cause_destination(index * 2 + 1).map_err(ClaimError::capture_allocation)?,
    ))
}
fn output(
    original: &AllocationOrigin,
    quota: &CaptureOutputQuota,
    index: u8,
) -> Result<PreparedFile, ClaimError> {
    let mut causes = causes(original, index)?;
    let output = quota.reserve_selected(&mut causes)?;
    Ok(PreparedFile { output, causes })
}
impl CommittedDiagnosticReader {
    /// Consume an actual namespace borrowing request and genuine Core3MiB/140 admission.
    /// # Errors
    /// Rejects source layouts and unavailable namespaces before effects; preserves admission.
    pub fn new(
        request: ProtectedCommittedReadRequest,
        admission: CommittedReaderAdmission,
    ) -> Result<Self, CommittedReaderConstructionFailure> {
        let original = admission.into_origin();
        Self::construct(request, &original)
            .map(|(fields, allocation)| Self {
                fields: Some(fields),
                allocation: Some(allocation),
                original: original.clone(),
            })
            .map_err(|cause| CommittedReaderConstructionFailure { cause, original })
    }
    fn construct(
        request: ProtectedCommittedReadRequest,
        core: &CommittedReaderOrigin,
    ) -> Result<(Box<Fields>, SharedCaptureLoan), ClaimError> {
        // Source-owned fixed regions include every retained data/control allocation.
        // The Core wrapper and this namespace wrapper are included before either Arc.
        let controls = core
            .control_storage_bytes()
            .checked_add(authority::ReaderAuthority::additional_control_bytes())
            .and_then(|n| n.checked_add(AllocationOrigin::control_bytes()))
            .and_then(|n| {
                n.checked_add(size_of::<Fields>() + DIRECTORY_PINS * size_of::<Handle>())
            });
        if controls.is_none_or(|n| n > 64 * 1024) {
            return Err(missing());
        }
        let namespace =
            crate::coordinator_entry::read_namespace::NamespaceReadLoan::take(&request, core)?;
        let original = AllocationOrigin::from_authority(ReleaseAuthority::CommittedReader(
            authority::ReaderAuthority::new(&request, core.clone()),
        ))
        .map_err(|failure| ClaimError::capture_allocation(failure.cause))?;
        let allocation =
            shared(&original, size_of::<Fields>() + DIRECTORY_PINS * size_of::<Handle>())?;
        let quota = CaptureOutputQuota::new(shared(
            &original,
            CaptureOutputQuota::control_storage_bytes(),
        )?);
        let scratch = original.scratch().map_err(ClaimError::capture_allocation)?;
        let mut main_causes = causes(&original, 0)?;
        let token = output(&original, &quota, 1)?;
        let token_handle = Handle::reserve_capture(&mut Some((original.clone(), token.output)))?;
        let directory_causes = [
            causes(&original, 2)?,
            causes(&original, 3)?,
            causes(&original, 4)?,
            causes(&original, 5)?,
            causes(&original, 10)?,
        ];
        let mut directories = Vec::new();
        directories.try_reserve_exact(DIRECTORY_PINS).map_err(|cause| {
            main_causes.error(
                "reader_directory_reserve",
                crate::claim_error::BoundedNativeCause::Reserve(cause),
                false,
            )
        })?;
        if directories.capacity() != DIRECTORY_PINS {
            return Err(missing());
        }
        let writer_output = Some(output(&original, &quota, 6)?);
        let prepared = [
            Some(output(&original, &quota, 7)?),
            Some(output(&original, &quota, 8)?),
            Some(output(&original, &quota, 9)?),
        ];
        Ok((
            Box::new(Fields {
                request,
                namespace,
                root: None,
                directory: None,
                sid: None,
                directories,
                directory_causes,
                token: token_handle,
                token_causes: token.causes,
                writer: None,
                writer_output,
                files: std::array::from_fn(|_| None),
                prepared,
                objects: [None; 3],
                quota,
                scratch,
                causes: main_causes,
                attempted: false,
                ready: false,
                failure: None,
            }),
            allocation,
        ))
    }
    /// Borrow only this actual target for its independently activated cleanup proof.
    pub fn original_origin(&self) -> &CommittedReaderOrigin {
        &self.original
    }
    /// Check genuine Core controls and the actual namespace fence before parser work.
    /// # Errors
    /// Terminal, cancelled, closed or expired work cannot authorize a new effect.
    pub fn check_effect(&self) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        self.allocation
            .as_ref()
            .ok_or(sugarsubstitute_contracts::execution::AdmissionError::InvalidSymbolAdmission)?
            .original()
            .original()
            .check_effect()
    }
    pub(super) fn parts(&mut self) -> Result<(&mut Fields, &AllocationOrigin), ClaimError> {
        Ok((
            self.fields.as_deref_mut().ok_or_else(missing)?,
            self.allocation.as_ref().ok_or_else(missing)?.original(),
        ))
    }
}
impl Drop for CommittedDiagnosticReader {
    fn drop(&mut self) {
        if let Some(fields) = self.fields.take() {
            if fields.has_output() {
                std::mem::forget(fields);
                if let Some(allocation) = self.allocation.take() {
                    std::mem::forget(allocation);
                }
                tracing::error!(
                    operation = "committed_reader_retained",
                    "Complete reader custody retained"
                );
            } else {
                drop(fields);
            }
        }
    }
}
