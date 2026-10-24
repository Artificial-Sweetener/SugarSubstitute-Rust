// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual trusted capture registration and typed child pin admissions.
use crate::job_sequence::JobSequence;
use crate::production::entry_runner::EntryResources;
use std::{error::Error, path::Path};
use sugarsubstitute_contracts::{
    crash::{CaptureBudget, CaptureState, CaptureTarget},
    execution::{CompletionBatch, JobClass, JobSpec, Moment, Operation, OwnedJob, Scope},
    incident::{BuildIdentity, ReportRestartAvailability},
    lifecycle::{CoordinatorKey, ShutdownBudget},
};
use sugarsubstitute_crash::{CaptureStartInput, StartRejected, WindowsCrashCapture};
use sugarsubstitute_execution::{
    ExecutionClient,
    windows::{NativeCorrelation, NativeObservationError, ProcessExit},
    windows_capture_attach::{
        CaptureAttachExpectation, CaptureAttachFailure, CaptureAttachLaunchPin, CaptureProcessFacts,
    },
};
use sugarsubstitute_supervision::InstalledReleaseOwner;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Own one actual process-lifetime registration; unavailable trust remains a real failure.
pub(in crate::production) struct CaptureRegistration {
    pub(super) capture: WindowsCrashCapture,
    root_storage: storage::CaptureRootStorage,
    rejected: Option<StartRejected>,
    unbound_watch: Option<OwnedJob<ProcessExit, NativeObservationError>>,
    attempted: bool,
    failure_reported: bool,
}
impl CaptureRegistration {
    pub(super) fn new(owner: CoordinatorKey) -> Self {
        Self {
            capture: WindowsCrashCapture::new(owner),
            root_storage: storage::CaptureRootStorage::new(),
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
        let budget = CaptureBudget::registration(now, source_until)?;
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
        if self.root_storage.source().is_none() && self.registration_ready() {
            self.root_storage.retain(self.capture.retain_root_source()?);
        }
        self.root_storage.observe()?;
        report_failure(&mut self.failure_reported, self.capture.errors().next())
    }
    /// Borrow unresolved registration/failure custody without closing the live helper.
    pub(super) fn normal_close_pending(&self, owner: CoordinatorKey) -> bool {
        let actual = self.capture.snapshot();
        self.rejected.is_some()
            || self.unbound_watch.is_some()
            || self.capture.errors().next().is_some()
            || !close_request_ready(actual, owner, self.capture.handler_lost())
    }
    /// Inspect the retained registration before attempting real attachment admission.
    pub(in crate::production) fn registration_ready(&self) -> bool {
        self.capture.snapshot().state == CaptureState::Registered
            && self.capture.snapshot().handler_exit.is_none()
    }
    /// Current Root decision for presentation/admission; it grants no attachment or restart capability.
    pub(super) fn restart_availability(&self) -> ReportRestartAvailability {
        if self.registration_ready() || self.fresh_supervisor_required() {
            ReportRestartAvailability::Available
        } else {
            ReportRestartAvailability::Unavailable
        }
    }
    /// Known lost one-shot registration requires a new subordinate; old clients remain degraded.
    pub(super) fn fresh_supervisor_required(&self) -> bool {
        self.capture.handler_lost() && self.root_source().is_some()
    }
    /// Mint an attachment expectation exclusively from actual retained registration facts.
    pub(in crate::production) fn attach(
        &self,
        target: CaptureTarget,
        source_until: Moment,
        client: &ExecutionClient,
        native: &Scope,
        jobs: &JobSequence,
    ) -> std::result::Result<
        OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
        Box<dyn Error + Send + Sync>,
    > {
        if self.capture.snapshot().owner != target.owner()
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
        let budget = CaptureBudget::registration(client.now()?, source_until)?;
        let expected = CaptureAttachExpectation::new(
            target.owner(),
            target,
            &build,
            creator,
            handler,
            budget,
        )?;
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 16384,
            replacement: None,
        };
        Ok(self.capture.request_attach_bootstrap(client, native, spec, expected)?)
    }
    /// Close the independently retained protected Root only after all consumers retire.
    /// This uses the same original Root shutdown budget through central finite execution.
    pub(super) fn close(
        &mut self,
        budget: ShutdownBudget,
        resources: &EntryResources<'_>,
    ) -> Result<bool> {
        let storage = self.root_storage.close(budget, resources)?;
        let registration = self.close_registration(budget, resources)?;
        Ok(registration && storage)
    }
    /// Borrow actual retained protected-root opening custody independently of handler health.
    pub(super) fn root_source(&self) -> Option<&sugarsubstitute_crash::CaptureRootSource> {
        self.root_storage.source()
    }
    /// Retire only the original handler registration, preserving genuine Root opening custody.
    /// Recovery can replace its own fresh family while unrelated live clients retain Root policy.
    pub(super) fn close_registration(
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

/// A deliberate close request still enters every original native/role/claim retirement owner.
/// Known registered loss is not a registration retry, healthy source or settlement proof.
fn close_request_ready(
    actual: sugarsubstitute_crash::CaptureSnapshot,
    expected: CoordinatorKey,
    handler_lost: bool,
) -> bool {
    actual.owner == expected
        && actual.handler.is_some()
        && match actual.state {
            CaptureState::Registered => actual.handler_exit.is_none(),
            CaptureState::Degraded => handler_lost && actual.handler_exit.is_some(),
            _ => false,
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
    fn known_matching_handler_loss_permits_only_a_deliberate_close_request() -> Result<()> {
        use sugarsubstitute_contracts::{crash::CaptureHandlerIdentity, lifecycle::*};
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let mut actual = sugarsubstitute_crash::CaptureSnapshot {
            owner,
            state: CaptureState::Degraded,
            handler: Some(CaptureHandlerIdentity { process_id: 17, creation_time_100ns: 23 }),
            handler_exit: Some(259),
            callback_pin_bytes: 65536,
            callback_pin_reservation_bytes: 2 * 1024 * 1024,
        };
        assert!(!close_request_ready(actual, owner, false));
        assert!(close_request_ready(actual, owner, true));
        let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..owner };
        assert!(!close_request_ready(actual, foreign, true));
        actual.handler_exit = None;
        assert!(!close_request_ready(actual, owner, true));
        actual.state = CaptureState::Registered;
        assert!(close_request_ready(actual, owner, false));
        actual.state = CaptureState::Closing;
        assert!(!close_request_ready(actual, owner, true));
        actual.state = CaptureState::Closed;
        assert!(!close_request_ready(actual, owner, true));
        actual.state = CaptureState::Degraded;
        actual.handler = None;
        assert!(!close_request_ready(actual, owner, true));
        Ok(())
    }
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

#[path = "capture/storage.rs"]
mod storage;
