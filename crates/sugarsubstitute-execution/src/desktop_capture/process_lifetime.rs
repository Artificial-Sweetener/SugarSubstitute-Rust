// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the original permanent capture owner without claiming process-exit retirement.

use super::{
    NativeCaptureDriver,
    call::CallOutputs,
    metadata::{ChildMetadata, DesktopCaptureCustodyRef},
};
use crate::{capture_bundle::CaptureBundleOrigin, native::Signal};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Exact retained fields are preallocated in the original source/control partition.
/// Driver keeps the original bundle/PIN owner; these fields keep its whole reservation.
/// A Job/OwnedJob/strong Core never enters this capsule.
pub(super) struct ProcessCaptureFields {
    pub(super) custody: DesktopCaptureCustodyRef,
    pub(super) driver: Box<dyn NativeCaptureDriver>,
    pub(super) original: CaptureBundleOrigin,
    pub(super) context: JobContext,
    /// Keep the exact callback userdata allocation alive through permanent custody.
    pub(super) _callback_signal: Arc<Signal>,
    pub(super) outputs: CallOutputs,
}

/// Exact passive borrowed settlement, validated while Core->cleanup->stage are held.
/// It correlates the real capsule/native cell and adopted Event/Wait closure. It
/// grants no fresh native effects, changes no deadline and cannot be cloned.
pub(crate) struct TemporaryCaptureSettlement<'a> {
    capsule: &'a ProcessCaptureCapsule,
    fields: &'a ProcessCaptureFields,
}

impl<'a> TemporaryCaptureSettlement<'a> {
    /// Bind returned facts to their original accepted cleanup and actual empty event.
    /// Caller supplies the actual guards; this observes no clock or native provider.
    pub(crate) fn validate_locked(
        capsule: &'a ProcessCaptureCapsule,
        event: &super::event::CompletionEvent,
        event_identity: usize,
        cleanup: &crate::capture_attach::control::CleanupState,
        stage: &super::control::DesktopCaptureState,
    ) -> Result<Self, sugarsubstitute_contracts::execution::AdmissionError> {
        use sugarsubstitute_contracts::execution::AdmissionError;
        let fields =
            capsule.storage.fields.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
        let returned = fields
            .outputs
            .returned_cleanup
            .as_ref()
            .ok_or(AdmissionError::InvalidCaptureAdmission)?;
        let facts = super::native_reply::decode(&fields.outputs.outer)
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        let original_cleanup = match &cleanup.snapshot {
            crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
                counter: Ok(original),
                ..
            } => original,
            _ => return Err(AdmissionError::InvalidCaptureAdmission),
        };
        let returned_budget =
            facts.cleanup.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
        if !stage.registered_published
            || !cleanup.stopped
            || !cleanup.held()
            || stage.phase != super::control::DesktopCapturePhase::Cleaning
            || stage.accepted_stage.is_some()
            || stage.accepted_cleanup.is_some()
            || returned.nonce != stage.last_cleanup
            || returned.attach == 0
            || returned.attach != fields.outputs.attach
            || fields.outputs.outer.attach != returned.attach
            || fields.outputs.foreign_attach
            || !facts.registered
            || !facts.retired
            || facts.pending
            || !event.settled_for(&fields.original, event_identity)
            || fields.context.owner_id() != fields.custody.cleanup.execution
            || fields.context.job_id() != fields.custody.cleanup.job
            || returned_budget.started != original_cleanup.started_counter()
            || returned_budget.grace != original_cleanup.graceful_counter()
            || returned_budget.total != original_cleanup.total_counter()
            || returned_budget.frequency != original_cleanup.frequency()
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(Self { capsule, fields })
    }

    /// Borrow sealed original provenance; the Core checks its exact record association.
    pub(crate) fn origin(&self) -> &CaptureBundleOrigin {
        &self.fields.original
    }

    /// Compare the actual capsule custody allocation, never just numeric run/job facts.
    pub(crate) fn custody(&self) -> &DesktopCaptureCustodyRef {
        &self.fields.custody
    }

    /// Observe the exact occupied Box identity for the Core's inert conflict record.
    /// This address never grants dereference/close capability and cannot be supplied
    /// through a public constructor or reused with a different retained capsule.
    pub(crate) fn capsule_identity(&self) -> usize {
        self.capsule.storage.as_ref() as *const ProcessCaptureStorage as usize
    }
}

/// Vacancy remains inert; an occupied capsule is never overwritten on rejection.
pub(super) struct ProcessCaptureStorage {
    pub(super) fields: Option<Box<ProcessCaptureFields>>,
    /// Hold the original child partition through every retained storage field.
    pub(super) _retained_child_partition: Option<MemoryClaim>,
    pub(super) failures: super::failures::RetainedFailures,
}

/// Keep the original metadata claim outside the complete Box/control lifetime.
/// Inline storage adds no separately allocated wrapper; storage drops before ledger.
pub(crate) struct ProcessCaptureCapsule {
    pub(super) storage: Box<ProcessCaptureStorage>,
    /// Drop the complete Box before its final original metadata charge alias.
    pub(super) _metadata: ChildMetadata,
}

impl ProcessCaptureCapsule {
    /// Borrow sealed original provenance from this actual occupied capsule only.
    /// No claim/driver/raw Box or Arc projection escapes this passive getter.
    pub(crate) fn original(
        &self,
    ) -> Result<&CaptureBundleOrigin, sugarsubstitute_contracts::execution::AdmissionError> {
        self.storage
            .fields
            .as_ref()
            .map(|fields| &fields.original)
            .ok_or(sugarsubstitute_contracts::execution::AdmissionError::InvalidCaptureAdmission)
    }
    /// Borrow the actual guarded custody before Core takes cleanup/stage guards.
    pub(crate) fn custody(
        &self,
    ) -> Result<&DesktopCaptureCustodyRef, sugarsubstitute_contracts::execution::AdmissionError>
    {
        self.storage
            .fields
            .as_ref()
            .map(|fields| &fields.custody)
            .ok_or(sugarsubstitute_contracts::execution::AdmissionError::InvalidCaptureAdmission)
    }
}

impl ProcessCaptureStorage {
    /// Prepare only inert slots before driver/native effects and terminal publication.
    pub(super) fn vacant(
        ledger: ChildMetadata,
        child_reserved_storage: MemoryClaim,
    ) -> ProcessCaptureCapsule {
        let storage = Box::new(Self {
            fields: None,
            _retained_child_partition: Some(child_reserved_storage),
            failures: Default::default(),
        });
        ProcessCaptureCapsule { storage, _metadata: ledger }
    }

    /// Moving into an already allocated capsule performs no native call or allocation.
    /// Caller keeps actual original Active fields on rejection before this handoff.
    pub(super) fn install(
        &mut self,
        fields: Box<ProcessCaptureFields>,
    ) -> Result<(), Box<ProcessCaptureFields>> {
        if self.fields.is_some() {
            return Err(fields);
        }
        self.fields = Some(fields);
        Ok(())
    }
}

/// Retain the same successful process capture owner until the actual process exits.
/// Temporary I/O/event/wait settlement is separately proven by the worker before
/// this value can be constructed. It never grants fresh Ready or native effects.
#[must_use = "Retain the permanent capture owner for the desktop process lifetime"]
pub struct ProcessLifetimeCapture {
    capsule: Option<ProcessCaptureCapsule>,
}

impl ProcessLifetimeCapture {
    /// Consume the exact preallocated capsule after the actual Core transfer fence.
    /// Construction is private; byte observations cannot manufacture this transfer.
    pub(super) fn transferred(capsule: ProcessCaptureCapsule) -> Self {
        Self { capsule: Some(capsule) }
    }

    /// Observe retained original reservation bytes without querying native state.
    /// This remains charged after the original job/temporary worker entry completes.
    pub fn reservation_bytes(&self) -> u64 {
        self.capsule
            .as_ref()
            .and_then(|capsule| capsule.storage.fields.as_ref())
            .map_or(0, |fields| fields.context.memory().bytes())
    }
}

impl Drop for ProcessLifetimeCapture {
    /// Abandoned host delivery keeps the identical complete owner before any field
    /// destruction. No FFI, unload, module close, source release or renewed cleanup
    /// deadline occurs. Only registry-observed process exit ends this native lifetime.
    fn drop(&mut self) {
        if let Some(capsule) = self.capsule.take() {
            std::mem::forget(capsule);
        }
    }
}
