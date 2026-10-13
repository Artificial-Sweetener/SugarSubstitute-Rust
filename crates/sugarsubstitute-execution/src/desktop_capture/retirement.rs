// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the exact accepted cleanup call and returned cell through the original cutoff.
use super::{
    NativeCaptureDriver,
    call::{self, CallOutputs},
    control::DesktopCapturePhase,
    metadata::DesktopCaptureCustodyRef,
    native_abi::NativeCaptureProgress,
};
use crate::{
    admission::Core,
    capture_attach::control::CaptureCleanupSnapshot,
    capture_bundle::{CaptureBundleOrigin, CaptureBundleRetirement},
    native::NativeObservationError,
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;

/// Private synchronous loan binds one real output cell and its original sealed target.
/// No caller can supply a returned status or reuse this nonclone accepted call.
pub(super) struct AcceptedCleanupCall<'a> {
    core: &'a Core,
    custody: &'a DesktopCaptureCustodyRef,
    context: &'a JobContext,
    driver: &'a dyn NativeCaptureDriver,
    outputs: &'a mut CallOutputs,
    attach: usize,
    nonce: u64,
    invoked: bool,
    _origin: &'a CaptureBundleOrigin,
    _proof: &'a CaptureBundleRetirement,
}

impl<'a> AcceptedCleanupCall<'a> {
    /// Check original cleanup authority outside locks, then claim this exact cell.
    /// The same Core->cleanup->stage fence revalidates actual held ownership.
    pub(super) fn accept(
        core: &'a Core,
        custody: &'a DesktopCaptureCustodyRef,
        context: &'a JobContext,
        origin: &'a CaptureBundleOrigin,
        proof: &'a CaptureBundleRetirement,
        driver: &'a dyn NativeCaptureDriver,
        outputs: &'a mut CallOutputs,
    ) -> Result<Self, NativeObservationError> {
        proof.check().map_err(|cause| error("desktop_cleanup_proof", cause))?;
        if !proof.matches_origin(origin) || outputs.attach == 0 || outputs.foreign_attach {
            return Err(invariant("desktop_cleanup_target"));
        }
        let observed =
            core.clock.metadata().map_err(|cause| error("desktop_cleanup_clock", cause))?;
        let state = lock(&core.state);
        let record = state
            .jobs
            .get(&custody.cleanup.job)
            .ok_or_else(|| invariant("desktop_cleanup_record"))?;
        if context.owner_id() != core.id
            || custody.cleanup.execution != core.id
            || record.scope_id != context.scope_id()
            || record.spec != context.spec()
            || record.deadline != context.deadline()
            || !record.capture_bundle_issued
            || record.memory.as_ref().is_none_or(|actual| !actual.matches_lease(context.memory()))
            || record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(custody))
            || record
                .native_custody
                .as_ref()
                .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
        {
            return Err(invariant("desktop_cleanup_association"));
        }
        let cleanup = lock(&custody.cleanup.state);
        let mut stage = lock(&custody.state);
        let cutoff = match &cleanup.snapshot {
            CaptureCleanupSnapshot::Latched { budget, .. } => budget.settle_until,
            _ => return Err(invariant("desktop_cleanup_original_budget")),
        };
        if cutoff != proof.cleanup_cutoff()
            || observed >= cutoff
            || core.clock.failed()
            || !cleanup.stopped
            || !cleanup.held()
            || stage.phase != DesktopCapturePhase::Cleaning
            || stage.accepted_stage.is_some()
            || stage.accepted_cleanup.is_some()
        {
            return Err(invariant("desktop_cleanup_not_current"));
        }
        let nonce = stage
            .last_cleanup
            .checked_add(1)
            .ok_or_else(|| invariant("desktop_cleanup_sequence"))?;
        stage.last_cleanup = nonce;
        stage.accepted_cleanup = Some(nonce);
        let attach = outputs.attach;
        drop(stage);
        drop(cleanup);
        drop(state);
        Ok(Self {
            core,
            custody,
            context,
            driver,
            outputs,
            attach,
            nonce,
            invoked: false,
            _origin: origin,
            _proof: proof,
        })
    }

    /// Run finite native cleanup on the same worker after acceptance locks leave.
    /// Outer/nested cells are adopted before status or unwind; no postreturn clock
    /// or proof is needed to interpret those exact correlated returned facts.
    pub(super) fn run(&mut self) -> Result<(), NativeObservationError> {
        self.invoked = true;
        let attach = self.attach;
        let driver = self.driver;
        call::invoke(
            self.core,
            self.custody,
            self.context,
            driver,
            self.outputs,
            |cleanup, issue, output| {
                driver.retire(
                    &NativeCaptureProgress { version: 2, size: 48, attach, cleanup, issue },
                    output,
                )
            },
        )
    }
}

impl Drop for AcceptedCleanupCall<'_> {
    /// End only this exact synchronous acceptance after its cell was adopted.
    /// Native uncertainty/output ownership remains in Active on return and unwind.
    fn drop(&mut self) {
        let state = lock(&self.core.state);
        let cleanup = lock(&self.custody.cleanup.state);
        let mut stage = lock(&self.custody.state);
        if stage.accepted_cleanup == Some(self.nonce) {
            if self.invoked {
                self.outputs.returned_cleanup =
                    Some(call::ReturnedCleanup { attach: self.attach, nonce: self.nonce });
            }
            stage.accepted_cleanup = None;
        }
        drop(stage);
        drop(cleanup);
        drop(state);
    }
}

/// Preserve original bounded causes across the typed child retirement seam.
fn error(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> NativeObservationError {
    NativeObservationError::new(stage, cause)
}
/// A private association rejection changes no native cell or accounting claim.
fn invariant(stage: &'static str) -> NativeObservationError {
    error(stage, std::io::Error::other("Actual child cleanup rejected"))
}
