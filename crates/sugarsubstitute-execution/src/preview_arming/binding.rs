// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mint only from actual retained source and consume original inputs after cumulative preclaim.
use super::{PreviewArmingError, PreviewArmingSeal, PreviewSourceLoan, source};
use crate::{DesktopCaptureSession, ExecutionClient, scope::lock};
use std::path::PathBuf;
use sugarsubstitute_contracts::{execution::*, startup::MainIdentity};

impl ExecutionClient {
    /// Borrow the same authenticated capture source before submitting a preview worker.
    /// The actual native producer stamps its Core allocation and persistent native Scope.
    /// The counter's frequency must match the authenticated original30s enrollment
    /// counter. Its cutoff may be at most that immutable enrollment cutoff+180s,
    /// preserving any original narrowing. The sole conversion samples the original
    /// Core clock before QPC; revealing Main and worker binding never resample it.
    /// Include PreviewSourceLoan::storage_bytes() in the existing controller preflight.
    /// This inert loan grants no filesystem effect before genuine Running admission.
    /// # Errors
    /// Rejects foreign/unauthenticated/stopped/retired source, wrong native scope,
    /// changed counter epoch, oversized campaign and original clock/counter failures.
    pub fn borrow_preview_source(
        &self,
        capture: &DesktopCaptureSession,
        native_scope: &Scope,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
        counter: CounterDeadline,
    ) -> Result<PreviewSourceLoan, PreviewArmingError> {
        self.borrow_preview_sampled(capture, native_scope, run, counter, counter_sample)
    }
    fn borrow_preview_sampled(
        &self,
        capture: &DesktopCaptureSession,
        native_scope: &Scope,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
        counter: CounterDeadline,
        sample: impl FnOnce() -> Result<(u64, u64), crate::native::NativeObservationError>,
    ) -> Result<PreviewSourceLoan, PreviewArmingError> {
        let custody = capture.qualification_custody();
        let native_owner = native_scope.owner_id();
        let (source_scope, control_bytes) = {
            let state = lock(&self.core.state);
            self.core.owned_scope(&state, native_scope, native_owner)?;
            let stage = lock(&custody.state);
            if custody.cleanup.full_target.desktop() != Some(run)
                || !std::ptr::eq(custody.cleanup.core.as_ptr(), self.core.as_ref())
                || stage.authenticated_creator.is_none_or(|creator| {
                    !creator.matches(run)
                        || !creator.matches_native(&self.core, native_scope.id())
                        || !creator.permits_campaign(counter)
                })
                || stage.preview_control_bytes < PreviewSourceLoan::storage_bytes()
                || stage.preview_loan_issued
                || stage.preview_job.is_some()
            {
                return Err(AdmissionError::InvalidCaptureAdmission.into());
            }
            (
                state
                    .jobs
                    .get(&custody.cleanup.job)
                    .ok_or(AdmissionError::InvalidCaptureAdmission)?
                    .scope_id,
                stage.preview_control_bytes,
            )
        };
        let now = source::now(&self.core)?;
        let until = crate::counter_deadline::decode_sampled(|| Ok(now), sample, counter)
            .map_err(PreviewArmingError::Native)?;
        let loan = PreviewSourceLoan {
            custody,
            run,
            native_scope: native_scope.id(),
            source_scope,
            control_bytes,
            counter,
            until,
        };
        // Sampling may reenter cancellation: the final source fence observes it.
        let observed = source::now(&self.core)?;
        let state = lock(&self.core.state);
        source::current(&self.core, &state, &loan, observed)?;
        {
            let mut stage = lock(&loan.custody.state);
            if stage.preview_loan_issued {
                return Err(AdmissionError::InvalidCaptureAdmission.into());
            }
            stage.preview_loan_issued = true;
        }
        drop(state);
        Ok(loan)
    }
    /// Test only: replace the external QPC sample, retaining every real source fence.
    #[cfg(test)]
    pub(crate) fn borrow_preview_owner_fixture(
        &self,
        capture: &DesktopCaptureSession,
        native_scope: &Scope,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
        counter: CounterDeadline,
    ) -> Result<PreviewSourceLoan, PreviewArmingError> {
        self.borrow_preview_sampled(capture, native_scope, run, counter, || Ok((100, 10)))
    }
    /// Bind an inert source/path to this actual Running ordinary64KiB worker once.
    /// Every validation or cumulative claim rejection leaves both original inputs
    /// untouched. The purpose flag commits only after the final actual source fence.
    /// This creates no queue, thread, shared capsule, native output or capacity.
    /// # Errors
    /// Rejects counterfeit metadata/ledger, foreign scope/job/spec/current source,
    /// repeated issuance, cancellation/deadline, destination bounds and exhausted credit.
    pub fn bind_preview_source(
        &self,
        context: &JobContext,
        loan: &mut Option<PreviewSourceLoan>,
        destination: &mut Option<PathBuf>,
        identity: MainIdentity,
    ) -> Result<PreviewArmingSeal, PreviewArmingError> {
        let original = loan.as_ref().ok_or(PreviewArmingError::Destination)?;
        let path = destination.as_ref().ok_or(PreviewArmingError::Destination)?;
        if identity.run != original.run || !bounded_destination(path) {
            return Err(PreviewArmingError::Destination);
        }
        {
            let state = lock(&self.core.state);
            crate::capture_bundle::validate::association(&self.core, &state, context)?;
        }
        let now = source::now(&self.core)?;
        {
            let state = lock(&self.core.state);
            source::worker(&self.core, &state, original, context, now)?;
            if lock(&original.custody.state).preview_job.is_some() {
                return Err(AdmissionError::InvalidCaptureAdmission.into());
            }
        }
        let bytes = size_of::<PreviewArmingSeal>()
            .checked_add(path.capacity())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(PreviewArmingError::Destination)?;
        let storage = context
            .memory()
            .claim(bytes)
            .map_err(|cause| PreviewArmingError::Admission(AdmissionError::Memory(cause)))?;
        // Keep physical input and claim outside Core locks on every rejection/drop.
        let acceptance = {
            let state = lock(&self.core.state);
            source::worker(&self.core, &state, original, context, now).and_then(|()| {
                let record =
                    crate::capture_bundle::validate::association(&self.core, &state, context)?;
                let actual =
                    crate::capture_bundle::validate::reconstruct(&self.core, record, context);
                let mut stage = lock(&original.custody.state);
                if stage.preview_job.is_some() {
                    return Err(AdmissionError::InvalidCaptureAdmission);
                }
                stage.preview_job = Some(context.job_id());
                Ok(actual)
            })
        };
        let actual = acceptance?;
        // Exclusive mutable options were verified before claiming and cannot change
        // during this synchronous owner call. No fallible work follows the commit.
        match (loan.take(), destination.take()) {
            (Some(loan), Some(destination)) => Ok(PreviewArmingSeal {
                loan,
                destination,
                original: actual,
                identity,
                _storage: storage,
            }),
            (original_loan, original_destination) => {
                {
                    let _state = lock(&self.core.state);
                    if let Some(original) = &original_loan {
                        let mut stage = lock(&original.custody.state);
                        if stage.preview_job == Some(context.job_id()) {
                            stage.preview_job = None;
                        }
                    }
                }
                *loan = original_loan;
                *destination = original_destination;
                Err(PreviewArmingError::Destination)
            }
        }
    }
}
fn bounded_destination(path: &PathBuf) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    let drive = bytes.len() >= 3
        && matches!(bytes[0], b'D' | b'd')
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    if !drive || path.capacity() > 4096 || bytes.contains(&0) || bytes[2..].contains(&b':') {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().take(1025).count() <= 1024
    }
    #[cfg(not(windows))]
    {
        path.as_os_str()
            .to_str()
            .is_some_and(|value| value.encode_utf16().take(1025).count() <= 1024)
    }
}
#[cfg(windows)]
fn counter_sample() -> Result<(u64, u64), crate::native::NativeObservationError> {
    crate::windows_counter::sample()
}
#[cfg(not(windows))]
fn counter_sample() -> Result<(u64, u64), crate::native::NativeObservationError> {
    Err(crate::native::NativeObservationError::new(
        "preview_platform",
        std::io::Error::other("Preview qualification requires Windows"),
    ))
}
