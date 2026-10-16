// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create once inside the genuine inherited Job; resume only after actual Root enrollment.
use super::*;
use crate::native_claims::files::{Budget, Handle};
use crate::native_process::FamilyStorage;
use sugarsubstitute_contracts::execution::MemoryClaim;
use sugarsubstitute_execution::process_creation::{ProcessCreationDriver, ProcessCreationProgress};
pub(super) struct Driver {
    port: Arc<dyn NativePort>,
    family: Option<FamilyStorage>,
    section: Option<RecoveryDesktopBootstrap>,
    captured: Option<CapturedLifecycleStart>,
    factory: Option<Factory>,
    input: Option<WindowsLaunchInput>,
    creation_attempted: bool,
    failed_without_output: bool,
    shared: Arc<Shared>,
    origin: AuthenticatedRecoveryOrigin,
    memory: ProcessCreationMemory,
    account: Vec<Handle>,
    account_memory: Arc<MemoryClaim>,
    _transient: MemoryClaim,
}
impl Driver {
    /// Fund every retained allocation before constructing the native driver; no native effects occur.
    pub(super) fn construct(
        shared: Arc<Shared>,
        input: RecoveryChildInput,
        origin: AuthenticatedRecoveryOrigin,
        context: &JobContext,
        memory: &ProcessCreationMemory,
    ) -> Result<Box<dyn ProcessCreationDriver>, NativeObservationError> {
        if Family::storage_bytes() + size_of::<Self>() + size_of::<Shared>() + 4096
            > memory.bytes() as usize
        {
            return Err(fail(RegistryError::invalid("recovery_child_layout")));
        }
        let transient = context
            .memory()
            .claim(TRANSIENT_BYTES)
            .map_err(|cause| fail(RegistryError::new("recovery_child_transient", cause)))?;
        let account_memory =
            Arc::new(context.memory().claim(16384).map_err(|cause| {
                fail(RegistryError::new("recovery_child_account_memory", cause))
            })?);
        let mut account = Vec::new();
        account
            .try_reserve_exact(1)
            .map_err(|cause| fail(RegistryError::new("recovery_child_account_capacity", cause)))?;
        let family = Family::custodied(memory, size_of::<Self>() + size_of::<Shared>() + 4096)
            .map_err(fail)?;
        Ok(Box::new(Self {
            port: native_process::production(),
            family: Some(family),
            section: None,
            captured: Some(input.captured),
            factory: Some(input.factory),
            input: None,
            creation_attempted: false,
            failed_without_output: false,
            shared,
            origin,
            memory: memory.clone(),
            account,
            account_memory,
            _transient: transient,
        }))
    }
    /// Fence each fresh effect with both original creation memory and genuine Root source.
    fn check(&self) -> Result<(), RegistryError> {
        self.memory
            .check_effect()
            .map_err(|cause| RegistryError::new("recovery_child_original", cause))?;
        self.origin.check().map_err(|cause| RegistryError::new("recovery_child_source", cause))
    }
}
impl ProcessCreationDriver for Driver {
    fn initialize(
        &mut self,
        context: &JobContext,
        memory: &ProcessCreationMemory,
        wake: ProcessCreationWake,
    ) -> Result<(), NativeObservationError> {
        let result = (|| {
            if !memory.same_allocation(&self.memory) {
                return Err(RegistryError::invalid("recovery_child_memory_changed"));
            }
            self.check()?;
            let family = self
                .family
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("recovery_child_family"))?;
            let raw = {
                let mut state = lock(&self.shared.state);
                state.memory = Some(memory.clone());
                state.wake = Some(wake);
                state
                    .pending_job
                    .take()
                    .ok_or_else(|| RegistryError::invalid("recovery_child_inherited_job"))?
            };
            family.adopt_recovery_child_job(
                raw,
                &self.memory,
                &mut self.account,
                Budget::Protected(self.account_memory.clone()),
            )?;
            let captured = self
                .captured
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("recovery_child_capture"))?;
            self.section = Some(
                RecoveryDesktopBootstrap::new(captured, memory)
                    .map_err(|cause| RegistryError::new("recovery_child_section", cause))?,
            );
            let section = self
                .section
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("recovery_child_section"))?;
            section
                .prepare()
                .map_err(|cause| RegistryError::new("recovery_child_section_prepare", cause))?;
            let factory = self
                .factory
                .take()
                .ok_or_else(|| RegistryError::invalid("recovery_child_command_once"))?;
            let input = factory(section, context)?;
            if input.required_reservation_bytes() > TRANSIENT_BYTES
                || input.lifecycle.is_some()
                || input.recovery_carrier.is_some()
                || input.profile_custody.is_some()
                || input.managed_custody.is_some()
                || input.diagnostic.is_some()
                || input.backend_custody.is_some()
                || input.fixture_events.is_some()
                || input.output.is_some()
            {
                return Err(RegistryError::invalid("recovery_child_command_roles"));
            }
            self.input = Some(input);
            family.prepare_recovery_child_inheritance(section)?;
            Ok(())
        })();
        result.map_err(fail)
    }
    fn progress(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<ProcessCreationProgress, NativeObservationError> {
        let result = (|| {
            self.check()?;
            let port = self.port.clone();
            let family = self
                .family
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("recovery_child_family"))?;
            if family.root.is_none() {
                let input = self
                    .input
                    .as_mut()
                    .ok_or_else(|| RegistryError::invalid("recovery_child_command"))?;
                self.creation_attempted = true;
                let created = port.create(family, input);
                self.failed_without_output =
                    created.is_err() && family.count == 0 && family.thread.is_none();
                let root = created?;
                let actual = RecoveryProcessIdentity::new(root.pid, root.birth)
                    .map_err(|cause| RegistryError::new("recovery_child_identity", cause))?;
                lock(&self.shared.state).created = Some(actual);
                let duplicate = port.duplicate(family, 0)?;
                let permit = lock(&self.shared.state)
                    .permit
                    .take()
                    .ok_or_else(|| RegistryError::invalid("recovery_child_watch_permit"))?;
                let wake = {
                    let mut state = lock(&self.shared.state);
                    state.watch = submission::bind(permit, duplicate);
                    state.wake.clone()
                };
                if let Some(wake) = wake {
                    wake.application_observed().map_err(|cause| {
                        RegistryError::new("recovery_child_created_wake", cause)
                    })?;
                }
            }
            let token = lock(&self.shared.state).enrollment.take();
            let Some(token) = token else { return Ok(ProcessCreationProgress::Pending) };
            let actual = lock(&self.shared.state)
                .created
                .ok_or_else(|| RegistryError::invalid("recovery_child_identity"))?;
            token
                .check(&self.origin, actual, &self.memory)
                .map_err(|cause| RegistryError::new("recovery_child_resume_enrollment", cause))?;
            let resumed = port.resume(family);
            lock(&self.shared.state).resumed = family.resumed;
            resumed?;
            Ok(ProcessCreationProgress::Started)
        })();
        result.map_err(fail)
    }
    fn retire(&mut self, _: &JobContext, _: Moment) -> Result<bool, NativeObservationError> {
        let result = (|| {
            let proof = self
                .memory
                .begin_retirement()
                .map_err(|cause| RegistryError::new("recovery_child_retirement", cause))?;
            proof
                .check()
                .map_err(|cause| RegistryError::new("recovery_child_retirement", cause))?;
            if let Some(family) = &mut self.family {
                family.creation_transferred();
                family.close_recovery_child(&proof)?;
            }
            self.family = None;
            if let Some(section) = &mut self.section
                && !section
                    .retire(&proof)
                    .map_err(|cause| RegistryError::new("recovery_child_section_retire", cause))?
            {
                return Ok(false);
            }
            self.section = None;
            for handle in self.account.iter_mut().rev() {
                proof
                    .check()
                    .map_err(|cause| RegistryError::new("recovery_child_account_retire", cause))?;
                handle
                    .close()
                    .map_err(|cause| RegistryError::new("recovery_child_account_close", cause))?;
            }
            self.account.clear();
            drop(self.input.take());
            drop(self.factory.take());
            drop(self.captured.take());
            let mut state = lock(&self.shared.state);
            state.native_retired = state.pending_job.is_none();
            state.process_absent_proven = !self.creation_attempted || self.failed_without_output;
            state.wake = None;
            state.enrollment = None;
            if let Some(permit) = state.permit.take() {
                state.watch = Some(permit.cancel_unbound());
            }
            Ok(true)
        })();
        result.map_err(fail)
    }
}
/// Check-close only the original unadopted slot under a same-Core finite retirement job.
pub(super) fn close_unstarted(
    shared: &Shared,
    origin: &AuthenticatedRecoveryOrigin,
    context: &JobContext,
) -> Result<(), RegistryError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err(RegistryError::invalid("recovery_child_unstarted_expired"));
    }
    origin
        .check_retirement_context(context)
        .map_err(|cause| RegistryError::new("recovery_child_input_original", cause))?;
    let mut state = lock(&shared.state);
    if state.cleanup_memory.is_none() {
        state.cleanup_memory =
            Some(context.memory().claim(4096).map_err(|cause| {
                RegistryError::new("recovery_child_input_cleanup_memory", cause)
            })?);
    }
    if state.memory.is_some() {
        return Err(RegistryError::invalid("recovery_child_unstarted_adopted"));
    }
    crate::native_process::close_recovery_input(&mut state.pending_job, context)?;
    state.native_retired = true;
    state.process_absent_proven = true;
    Ok(())
}
/// Preserve the actual registry/native cause inside the central typed failure.
fn fail(cause: RegistryError) -> NativeObservationError {
    NativeObservationError::new("recovery_child_creation", cause)
}
