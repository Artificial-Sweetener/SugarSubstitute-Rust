// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one physical native file and its immutable grant across every read consumer.
mod admission;
mod causes;
mod inspect;
mod layout;
#[cfg(test)]
mod observations;
mod open;
mod reader;
use super::native_name::NativeName;
use super::{paths, security};
use crate::claim_error::{BoundedNativeCause, NativeCauseDestination};
use crate::diagnostic_storage::{DiagnosticNativeGrant, DiagnosticNativeOwnerLoan};
use crate::{
    ClaimError,
    diagnostic_storage::{DIAGNOSTIC_IO_BYTES, DiagnosticReadFacts},
    native_claims::files::{Budget, Handle},
};
pub(crate) use causes::NativeCauseDestinations;
pub(crate) use inspect::seek;
pub(crate) use reader::NativeReadCore;
use sugarsubstitute_contracts::execution::JobContext;
#[cfg(test)]
use sugarsubstitute_contracts::execution::NativeHandleOutput;
use windows::Win32::Foundation::HANDLE;

/// Select immutable actual OS access; only stage creation may create an object.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum FileGrant {
    MetadataReadOnly,
    InventoryMetadata,
    DirectoryScan,
    ReadOnly,
    WriterReadDelete,
    WritableStage,
    WritableRecovery,
}
impl FileGrant {
    pub(crate) fn writable(self) -> bool {
        matches!(self, Self::WritableStage | Self::WritableRecovery)
    }
}
/// Keep physical output, pinned metadata and original context in one durable capsule.
pub(crate) struct Fields {
    pub(crate) id: u64,
    pub(crate) handle: Handle,
    pub(crate) path: NativeName,
    pub(crate) facts: Option<DiagnosticReadFacts>,
    pub(crate) bound: u64,
    pub(crate) stage: bool,
    pub(crate) deleted: bool,
    grant: FileGrant,
    counted: bool,
    attempted: bool,
    causes: NativeCauseDestinations,
    original: Option<JobContext>,
    first_failure: Option<ClaimError>,
    secondary_failure: Option<ClaimError>,
}
pub(crate) struct NativeFile {
    fields: Option<Box<Fields>>,
    allocation: Option<NativeFileAllocation>,
}
/// Keep the physical fields storage alive before releasing its outside credit.
enum NativeFileAllocation {
    #[cfg(test)]
    Ordinary {
        _budget: Budget,
    },
    Diagnostic {
        original: DiagnosticNativeGrant,
        _allocation: DiagnosticNativeOwnerLoan,
    },
    Selected {
        _original: crate::installed_release::AllocationOrigin,
        _allocation: crate::installed_release::SharedCaptureLoan,
    },
}
/// Consume exactly the caller's original typed admission before allocating fields.
pub(crate) enum NativeFileAdmission {
    #[cfg(test)]
    Ordinary { budget: Budget },
    #[cfg(test)]
    Symbol { budget: Budget, original: JobContext, output: NativeHandleOutput },
    Selected {
        original: crate::installed_release::AllocationOrigin,
        allocation: crate::installed_release::SharedCaptureLoan,
        output: crate::installed_release::CaptureOutputGuard,
        causes: NativeCauseDestinations,
    },
    Diagnostic {
        original: JobContext,
        grant: DiagnosticNativeGrant,
        allocation: DiagnosticNativeOwnerLoan,
        causes: NativeCauseDestinations,
    },
}
impl NativeFile {
    fn absent_cause(&self) -> ClaimError {
        match &self.allocation {
            Some(NativeFileAllocation::Diagnostic { original, .. }) => {
                ClaimError::diagnostic_allocation(original.reject_native_state())
            }
            Some(NativeFileAllocation::Selected { .. }) => ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::missing(),
            ),
            _ => ClaimError::invalid("native_file_owner_absent"),
        }
    }
    /// Borrow the actual physical owner without projecting an absent slot.
    /// # Errors
    /// Rejects access after this cell's complete owner has been consumed.
    pub(crate) fn fields(&self) -> Result<&Fields, ClaimError> {
        self.fields.as_deref().ok_or_else(|| self.absent_cause())
    }
    /// Mutably borrow the same physical owner; no replacement owner is created.
    /// # Errors
    /// Rejects an already consumed owner before any native operation.
    pub(crate) fn fields_mut(&mut self) -> Result<&mut Fields, ClaimError> {
        if self.fields.is_none() {
            return Err(self.absent_cause());
        }
        self.fields.as_deref_mut().ok_or_else(|| ClaimError::invalid("native_file_owner_absent"))
    }
    /// Observe retained output custody; an absent slot remains conservatively live.
    pub(crate) fn has_output(&self) -> bool {
        self.fields.as_ref().is_none_or(|fields| fields.handle.is_armed())
    }
    /// Classify a failed existing-only open as absence only without physical custody.
    /// Late validation/query failures keep their adopted output and original cause.
    pub(crate) fn missing_without_output(&self, cause: &ClaimError) -> bool {
        use std::error::Error;
        self.fields.as_ref().is_some_and(|fields| !fields.handle.is_armed())
            && cause
                .source()
                .and_then(|source| source.downcast_ref::<windows::core::Error>())
                .is_some_and(|error| {
                    error.code() == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.to_hresult()
                })
    }
    /// Observe the immutable writable grant, independently of logical stage state.
    /// # Errors
    /// Rejects an absent physical owner rather than treating it as read-only.
    pub(crate) fn writable(&self) -> Result<bool, ClaimError> {
        Ok(self.fields()?.grant.writable())
    }
    /// Retire the actual output; errors retain the complete capsule and first cause.
    #[cfg(test)]
    pub(crate) fn close(&mut self) -> Result<(), ClaimError> {
        let result = self.fields_mut()?.handle.close();
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    /// Close this normal-operation cell with its original preheld cause pair.
    pub(crate) fn close_in_phase(
        &mut self,
        phase: &mut super::native_phase::NativePhase<'_>,
        current: &JobContext,
    ) -> Result<(), ClaimError> {
        let fields = self.fields_mut()?;
        let result = {
            let mut phase = phase.for_causes(&mut fields.causes)?;
            fields.handle.close_in_phase(&mut phase)
        };
        let result = phase.post_context_for(result, current, &mut fields.causes);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    /// Retire existing custody after normal admission closes; no new loan is issued.
    pub(crate) fn retire_diagnostic(
        &mut self,
        retirement: &mut crate::diagnostic_storage::DiagnosticNativeRetirement<'_>,
    ) -> Result<(), ClaimError> {
        let fields = self.fields_mut()?;
        let result = fields.handle.close_diagnostic(retirement, &mut fields.causes);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    /// Borrow the real close proof and adopt only this file's checked returned output.
    pub(crate) fn retire_selected(
        &mut self,
        proof: &sugarsubstitute_execution::CaptureBundleRetirement,
    ) -> Result<(), ClaimError> {
        let fields = self.fields_mut()?;
        let result = fields.handle.close_selected(proof, &mut fields.causes);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    fn remember(&mut self, cause: ClaimError) {
        if let Some(fields) = self.fields.as_mut() {
            if fields.first_failure.as_ref().is_some_and(|first| first.same_observation(&cause))
                || fields
                    .secondary_failure
                    .as_ref()
                    .is_some_and(|secondary| secondary.same_observation(&cause))
            {
                return;
            }
            if fields.first_failure.is_none() {
                fields.first_failure = Some(cause);
                return;
            }
            if fields.secondary_failure.is_none() {
                fields.secondary_failure = Some(cause);
                return;
            }
        }
        let operation = cause.operation();
        std::mem::forget(cause);
        tracing::error!(operation, "Additional native file failure custody retained");
    }
    fn check(&self, context: &JobContext) -> Result<(), ClaimError> {
        if let Budget::SelectedRelease(original) = self.fields()?.handle.budget() {
            // The actual Selected phase performs Core checks before every effect.
            // This immutable lookup checks only the retained cell association and
            // never invokes or authorizes work through caller-supplied ports.
            let expected = original.original().original_context().map_err(|_| {
                ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::missing(),
                )
            })?;
            let held = self.fields()?.original.as_ref().ok_or_else(|| {
                ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::missing(),
                )
            })?;
            if held.owner_id() != expected.owner_id()
                || held.scope_id() != expected.scope_id()
                || held.job_id() != expected.job_id()
            {
                return Err(ClaimError::capture_allocation(
                    crate::installed_release::CaptureCapacityFailure::identity(),
                ));
            }
            return Ok(());
        }
        if let Budget::Diagnostic(grant) = self.fields()?.handle.budget() {
            grant.check(context).map_err(ClaimError::diagnostic_allocation)?;
            let first = self.fields()?.original.as_ref();
            if first.is_none_or(|first| {
                first.owner_id() != context.owner_id()
                    || first.scope_id() != context.scope_id()
                    || first.cancellation_requested()
                    || first.deadline().is_none_or(|limit| first.now() >= limit)
                    || context.deadline().is_none()
                    || context.deadline() > first.deadline()
            }) {
                return Err(ClaimError::diagnostic_allocation(grant.reject_native_state()));
            }
            return Ok(());
        }
        let first = self
            .fields()?
            .original
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("diagnostic_unbound_context"))?;
        if first.owner_id() != context.owner_id()
            || first.scope_id() != context.scope_id()
            || first.deadline().is_none()
            || context.deadline().is_none()
            || context.deadline() > first.deadline()
        {
            return Err(ClaimError::invalid("diagnostic_changed_context"));
        }
        super::check(first)?;
        super::check(context)
    }
}
impl Drop for NativeFile {
    fn drop(&mut self) {
        if let Some(fields) = self.fields.take() {
            if fields.handle.is_armed()
                && (fields.counted
                    || matches!(self.allocation, Some(NativeFileAllocation::Diagnostic { .. })))
            {
                std::mem::forget(fields);
                if let Some(allocation) = self.allocation.take() {
                    std::mem::forget(allocation);
                }
                tracing::error!(
                    operation = "native_file_abandoned",
                    "Native file custody retained"
                );
            } else {
                drop(fields);
            }
        }
    }
}
