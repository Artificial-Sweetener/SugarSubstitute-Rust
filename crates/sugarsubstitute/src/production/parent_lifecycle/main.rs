// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authorize environment/browser replies from the actual retained Windows profile proofs.
use super::types::ProfilePending;
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_application::supervision::{
    MainAuthorizationEffect as Effect, MainAuthorizationObservation as Observation,
};
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, JobClass, JobSpec, Moment, Operation, TerminalKind},
    lifecycle::{LifecycleFailure, LifecycleFailureCode},
};
use sugarsubstitute_supervision::{
    lifecycle_protocol::MainStageFailure, profiles::PROFILE_OPERATION_BYTES,
};

impl ParentLifecycle {
    pub(super) fn main_deadline(&self) -> Option<Moment> {
        use sugarsubstitute_application::supervision::MainAuthorizationStage;
        match self.main.stage() {
            MainAuthorizationStage::RequestedFolder
            | MainAuthorizationStage::Environment
            | MainAuthorizationStage::Browser => self.main.snapshot().frontend_until,
            _ => None,
        }
    }

    pub(super) fn check_main_deadline(&self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if let Some(until) = self.main_deadline()
            && resources.execution.client.now()? >= until
        {
            return Err(sugarsubstitute_execution::windows_lifecycle_pipe::
                LifecyclePipeError::DeadlineExpired.into());
        }
        Ok(())
    }

    pub(super) fn profile_effect(
        &mut self,
        index: usize,
        effect: Option<Effect>,
        until: Moment,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.profile_pending.is_some() {
            return Err(ParentError::Overloaded.into());
        }
        resources.session.control.validate_current_admission()?;
        let spec = JobSpec {
            id: resources.execution.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: PROFILE_OPERATION_BYTES,
            replacement: None,
        };
        let (identity, pid) = match effect.ok_or(ParentError::WrongPhase)? {
            Effect::ValidateEnvironment(identity) => {
                resources.profile.submit_environment_validation(
                    resources.execution.client,
                    resources.finite,
                    spec,
                    identity,
                    self.folders.units(),
                )?;
                (identity, None)
            }
            Effect::ValidateBrowser { identity, browser_process_id } => {
                let registry =
                    resources.execution.authority.registry().ok_or(ParentError::WrongPhase)?;
                resources.profile.submit_browser_validation(
                    resources.execution.client,
                    resources.finite,
                    spec,
                    registry,
                    identity,
                    browser_process_id.get(),
                )?;
                (identity, Some(browser_process_id))
            }
            _ => return Err(ParentError::WrongPhase.into()),
        };
        self.profile_pending =
            Some(ProfilePending { job: spec.id, index, identity, pid, original: until });
        Ok(())
    }

    pub(super) fn consume_profile(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let Some(pending) = &self.profile_pending else {
            return Ok(());
        };
        let Some(completion) = resources.profile.consume(batch)? else {
            return Ok(());
        };
        if completion.job != pending.job {
            return Err(ParentError::WrongPhase.into());
        }
        let pending = self.profile_pending.take().ok_or(ParentError::WrongPhase)?;
        if let Some(cause) = completion.failure {
            return Err(cause.into());
        }
        if self.closed || self.main_retirement.is_some() {
            self.environment = resources.profile.take_environment_proof();
            self.browser = resources.profile.take_browser_proof();
            self.environment = None;
            self.browser = None;
            return Ok(());
        }
        if completion.kind != TerminalKind::Success
            || resources.execution.client.now()? >= pending.original
        {
            return Err(ParentError::OwnerFailed.into());
        }
        let header = self.requests.get(pending.index)?.header;
        let binding = self.request_binding(header)?;
        let request = header.request.ok_or(ParentError::WrongPhase)?;
        resources.session.control.validate_current_admission()?;
        if let Some(pid) = pending.pid {
            let proof = resources.profile.take_browser_proof().ok_or(ParentError::WrongPhase)?;
            self.browser = Some(proof);
            let proof = self.browser.as_ref().ok_or(ParentError::WrongPhase)?;
            if proof.job_id() != pending.job
                || !proof.validates(resources.profile, pending.identity, pid.get())
            {
                return Err(ParentError::ForeignPeer.into());
            }
            self.main.accept(
                binding,
                request,
                Observation::BrowserVerified {
                    identity: pending.identity,
                    browser_process_id: pid,
                },
                resources.execution.client.now()?,
            )?;
            self.requests.get_mut(pending.index)?.reply =
                Some(Reply::Browser { identity: pending.identity, pid });
        } else {
            let proof =
                resources.profile.take_environment_proof().ok_or(ParentError::WrongPhase)?;
            self.environment = Some(proof);
            let proof = self.environment.as_ref().ok_or(ParentError::WrongPhase)?;
            if proof.job_id() != pending.job
                || !proof.validates(resources.profile, pending.identity)
            {
                return Err(ParentError::ForeignPeer.into());
            }
            self.main.accept(
                binding,
                request,
                Observation::EnvironmentVerified { identity: pending.identity },
                resources.execution.client.now()?,
            )?;
            self.requests.get_mut(pending.index)?.reply =
                Some(Reply::Environment(pending.identity));
        }
        Ok(())
    }

    pub(super) fn authorization_issued(
        &mut self,
        index: usize,
        resources: &ParentResources<'_, '_>,
    ) -> Result<()> {
        let record = self.requests.get(index)?;
        resources.session.control.validate_current_admission()?;
        let observation =
            match record.reply.ok_or(ParentError::WrongPhase)? {
                Reply::Environment(identity) => {
                    if self
                        .environment
                        .as_ref()
                        .is_none_or(|proof| !proof.validates(resources.profile, identity))
                    {
                        return Err(ParentError::ForeignPeer.into());
                    }
                    Observation::EnvironmentAuthorizationIssued { identity }
                }
                Reply::Browser { identity, pid } => {
                    if self.browser.as_ref().is_none_or(|proof| {
                        !proof.validates(resources.profile, identity, pid.get())
                    }) {
                        return Err(ParentError::ForeignPeer.into());
                    }
                    Observation::BrowserAuthorizationIssued { identity, browser_process_id: pid }
                }
                _ => return Ok(()),
            };
        self.main.accept(
            self.request_binding(record.header)?,
            record.header.request.ok_or(ParentError::WrongPhase)?,
            observation,
            resources.execution.client.now()?,
        )?;
        Ok(())
    }

    pub(super) fn finish_main_retirement(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let Some((index, identity)) = self.main_retirement else {
            return Ok(());
        };
        if self.profile_pending.is_some() || resources.profile.snapshot().pending {
            return Ok(());
        }
        resources.profile.confirm_main_retired(identity)?;
        self.main.confirm_retired(identity)?;
        self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
        self.main_retirement = None;
        Ok(())
    }

    pub(super) fn main_failed(
        &mut self,
        index: usize,
        failure: MainStageFailure,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let header = self.requests.get(index)?.header;
        self.main.accept(self.binding()?, header.request.ok_or(ParentError::WrongPhase)?,
            Observation::Failed { identity: failure.identity,
                failure: LifecycleFailure::new(match failure.failure {
                    sugarsubstitute_supervision::lifecycle_protocol::MainFailureCode::Deadline =>
                        LifecycleFailureCode::Deadline,
                    sugarsubstitute_supervision::lifecycle_protocol::MainFailureCode::NativeCleanupUncertain =>
                        LifecycleFailureCode::Cleanup,
                    _ => LifecycleFailureCode::Preparation,
                }, Box::new(RemoteMainFailure(failure))) }, resources.execution.client.now()?)?;
        resources.profile.retire_main_attempt(failure.identity)?;
        self.requests.get_mut(index)?.reply = Some(Reply::MainFailed(failure));
        Ok(())
    }
}

#[derive(Debug)]
struct RemoteMainFailure(MainStageFailure);
impl std::fmt::Display for RemoteMainFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authenticated desktop main failure: {:?}/{:?}", self.0.stage, self.0.failure)
    }
}
impl std::error::Error for RemoteMainFailure {}
