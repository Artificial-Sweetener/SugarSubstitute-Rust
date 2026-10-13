// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual trusted capture registration and typed child pin admissions.
use crate::production::entry_runner::EntryResources;
use std::{error::Error, path::Path};
use sugarsubstitute_contracts::{
    crash::{CaptureBudget, CaptureState},
    execution::{CompletionBatch, JobClass, JobSpec, Moment, Operation, OwnedJob},
    incident::BuildIdentity,
    lifecycle::{CoordinatorKey, RunKey, ShutdownBudget},
};
use sugarsubstitute_crash::{CaptureStartInput, StartRejected, WindowsCrashCapture};
use sugarsubstitute_execution::{
    windows::{NativeCorrelation, NativeObservationError, ProcessExit},
    windows_capture_attach::{
        CaptureAttachExpectation, CaptureAttachFailure, CaptureAttachLaunchPin, CaptureProcessFacts,
    },
};
use sugarsubstitute_supervision::InstalledReleaseOwner;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Own one actual process-lifetime registration; unavailable trust remains a real failure.
pub(super) struct CaptureRegistration {
    pub(super) capture: WindowsCrashCapture,
    rejected: Option<StartRejected>,
    unbound_watch: Option<OwnedJob<ProcessExit, NativeObservationError>>,
    attempted: bool,
    failure_reported: bool,
}
impl CaptureRegistration {
    pub(super) fn new(owner: CoordinatorKey) -> Self {
        Self {
            capture: WindowsCrashCapture::new(owner),
            rejected: None,
            unbound_watch: None,
            attempted: false,
            failure_reported: false,
        }
    }
    /// Admit real installed-release selection before loader/registration effects.
    /// The inert configured root still requires actual protected-root verification.
    pub(super) fn start(
        &mut self,
        root: &Path,
        source_until: Moment,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        if self.attempted {
            return Ok(());
        }
        let input = CaptureStartInput::new(
            root.to_owned(),
            InstalledReleaseOwner::new().capture_selection(),
        )?;
        let bytes = input.required_reservation_bytes() as u64;
        let now = resources.client.now()?;
        if now >= source_until {
            return Err("Original entry expired before capture registration".into());
        }
        let budget = CaptureBudget::new(now)?;
        let watch_id = resources.jobs.allocate()?;
        let creation_id = resources.jobs.allocate()?;
        let watch = resources.client.reserve_process_watch(
            resources.native,
            JobSpec {
                id: watch_id,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes: 4096,
                replacement: None,
            },
            NativeCorrelation::new(watch_id.get())?,
        )?;
        let creation = JobSpec {
            id: creation_id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(source_until.min(budget.until())),
            reservation_bytes: bytes,
            replacement: None,
        };
        self.attempted = true;
        if let Err(rejected) = self.capture.submit_start(
            resources.client,
            resources.finite,
            creation,
            budget,
            watch,
            input,
        ) {
            self.rejected = Some(rejected);
            return Err("Actual capture start admission rejected with retained input/watch".into());
        }
        Ok(())
    }
    /// Consume actual registration/watch/retirement facts without synthesizing success.
    pub(super) fn observe(&mut self, batch: &CompletionBatch) -> Result<()> {
        self.capture.observe(batch);
        report_failure(&mut self.failure_reported, self.capture.errors().next())
    }
    /// Mint an attachment expectation exclusively from actual retained registration facts.
    pub(super) fn attach(
        &self,
        run: RunKey,
        resources: &EntryResources<'_>,
    ) -> Result<OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>> {
        if self.capture.snapshot().owner != run.owner
            || self.capture.snapshot().state != CaptureState::Registered
        {
            return Err("Actual capture registration is not available".into());
        }
        let build =
            self.capture.registration_build().ok_or("Actual selected capture build missing")?;
        let build = BuildIdentity::new(build.as_str()?)?;
        let creator =
            self.capture.creator_process().ok_or("Actual retained creator identity missing")?;
        let handler = self.capture.snapshot().handler.ok_or("Actual retained handler missing")?;
        let handler = CaptureProcessFacts::new(handler.process_id, handler.creation_time_100ns)?;
        let budget = CaptureBudget::new(resources.client.now()?)?;
        let expected =
            CaptureAttachExpectation::new(run.owner, run, &build, creator, handler, budget)?;
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 16384,
            replacement: None,
        };
        Ok(self.capture.request_attach_bootstrap(
            resources.client,
            resources.native,
            spec,
            expected,
        )?)
    }
    /// Close only after all actual desktop source pins/handlers may retire.
    /// Keep the same original shutdown budget and every uncertain owner.
    pub(super) fn close(
        &mut self,
        budget: ShutdownBudget,
        resources: &EntryResources<'_>,
    ) -> Result<bool> {
        if let Some(rejected) = self.rejected.take() {
            match rejected.into_parts() {
                Ok((_cause, watch, _input)) => self.unbound_watch = Some(watch.cancel_unbound()),
                Err(cause) => return Err(Box::new(cause)),
            }
        }
        if let Some(watch) = &mut self.unbound_watch {
            if watch.try_take_completion().is_none() {
                return Ok(false);
            }
            self.unbound_watch = None;
        }
        if !self.attempted || self.capture.snapshot().state == CaptureState::Closed {
            return Ok(true);
        }
        if matches!(
            self.capture.snapshot().state,
            CaptureState::Registered | CaptureState::Degraded | CaptureState::Closing
        ) {
            let spec = JobSpec {
                id: resources.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(budget.settle_until),
                reservation_bytes: 4096,
                replacement: None,
            };
            match self.capture.submit_close(resources.client, resources.cleanup, spec) {
                Ok(()) => {}
                Err(sugarsubstitute_crash::CrashAdmissionError::Busy) => {}
                Err(cause) => return Err(Box::new(cause)),
            }
        }
        Ok(self.capture.snapshot().state == CaptureState::Closed)
    }
}

fn report_failure(
    reported: &mut bool,
    cause: Option<&sugarsubstitute_crash::CrashError>,
) -> Result<()> {
    if !*reported && let Some(cause) = cause {
        *reported = true;
        return Err(Box::new(cause.clone()));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_failure_reports_original_once_without_reopening_registration() -> Result<()> {
        let request = InstalledReleaseOwner::new().capture_selection();
        let cause = match CaptureStartInput::new("relative".into(), request) {
            Err(cause) => cause,
            Ok(_) => return Err("Relative capture location was admitted".into()),
        };
        let original = cause.source().ok_or("Original capture cause absent")?;
        let mut reported = false;
        let observed =
            report_failure(&mut reported, Some(&cause)).err().ok_or("Failure not reported")?;
        assert!(std::ptr::eq(original, observed.source().ok_or("Source lost")?));
        let mut owner = super::super::ProductionActivation::new(
            r"E:\application\SugarSubstitute.exe".into(),
            r"E:\application".into(),
            r"D:\captures".into(),
        )?;
        owner.preserve(observed);
        owner.closing = true;
        // Completion/deadline adoption has returned successfully. That success
        // cannot hide the newly observed capture failure from the actual Runner.
        assert!(owner.complete_observation(true, Ok(())).is_err());
        assert!(std::ptr::eq(
            original,
            owner.failure_source().and_then(Error::source).ok_or("Owner source lost")?
        ));
        for _ in 0..4 {
            report_failure(&mut reported, Some(&cause))?;
            owner.complete_observation(false, Ok(()))?;
        }
        assert!(reported);
        assert!(owner.closing);
        Ok(())
    }
}
