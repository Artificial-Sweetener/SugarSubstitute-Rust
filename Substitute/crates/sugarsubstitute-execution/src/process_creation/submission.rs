// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the original finite creation once, before transferring the complete input.
use super::*;
use crate::{
    admission::RequestKind,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobSpec, Operation, OwnedJob, Scope,
};

impl crate::ExecutionClient {
    /// Admit managed creation on the existing native worker under finite class credits.
    /// The source is genuine parent transport storage. Input stays identical on every
    /// pre-admission or claim rejection; the factory runs later outside Core locks.
    /// Original deadline/reservation and ordinary scope remain unchanged across ACKs.
    /// # Errors
    /// Rejects closed/foreign sources, wrong finite jobs, capacity or original memory.
    pub fn submit_custody_creation<I, F>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        parent: &crate::backend_custody::BackendCustodyParentStorage,
        input: &mut Option<I>,
        factory: F,
    ) -> Result<OwnedJob<ProcessCreationReport, ProcessCreationFailure>, AdmissionError>
    where
        I: Send + 'static,
        F: FnOnce(
                I,
                &JobContext,
                &ProcessCreationMemory,
            )
                -> Result<Box<dyn ProcessCreationDriver>, crate::native::NativeObservationError>
            + Send
            + 'static,
    {
        self.admit_creation(
            scope,
            spec,
            super::source::Source::Custody { parent: parent.clone(), grant: None },
            input,
            factory,
        )
    }

    /// Reserve the same real profile-custody creation without allowing its factory to run.
    /// Queue, completion and original byte credits are charged before this returns.
    /// The Root must independently settle the old family/profile and durably acknowledge
    /// the exact restart before activating this once-only permit. No worker waits for ACK.
    /// # Errors
    /// Leaves input unchanged on invalid original Root grant, repeated creation,
    /// custody loss, scope/class/deadline or actual queue/memory rejection.
    pub fn reserve_recovery_custody_creation<I, F>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        parent: &crate::backend_custody::BackendCustodyParentStorage,
        grant: &crate::RegisteredRecoveryGrant,
        input: &mut Option<I>,
        factory: F,
    ) -> Result<super::RecoveryCreationPermit, AdmissionError>
    where
        I: Send + 'static,
        F: FnOnce(
                I,
                &JobContext,
                &ProcessCreationMemory,
            )
                -> Result<Box<dyn ProcessCreationDriver>, crate::native::NativeObservationError>
            + Send
            + 'static,
    {
        let job = self.admit_creation(
            scope,
            spec,
            super::source::Source::Custody { parent: parent.clone(), grant: Some(grant.clone()) },
            input,
            factory,
        )?;
        Ok(super::RecoveryCreationPermit::new(job, &self.core, grant.clone()))
    }

    /// Admit the fresh Desktop under the genuine authenticated Root delegation.
    /// This does not create a profile/backend authority. The adapter keeps the child
    /// suspended until Root independently enrolls its exact birth and family membership.
    /// Original deadline, fixed claim and the same native progress owner are retained.
    /// # Errors
    /// Leaves input unchanged on foreign/closed/replayed source, queue or memory rejection.
    pub fn submit_recovery_desktop_creation<I, F>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        origin: &crate::recovery_supervisor::AuthenticatedRecoveryOrigin,
        input: &mut Option<I>,
        factory: F,
    ) -> Result<OwnedJob<ProcessCreationReport, ProcessCreationFailure>, AdmissionError>
    where
        I: Send + 'static,
        F: FnOnce(
                I,
                &JobContext,
                &ProcessCreationMemory,
            )
                -> Result<Box<dyn ProcessCreationDriver>, crate::native::NativeObservationError>
            + Send
            + 'static,
    {
        self.admit_creation(
            scope,
            spec,
            super::source::Source::Recovery(origin.0.clone()),
            input,
            factory,
        )
    }

    fn admit_creation<I, F>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        source: super::source::Source,
        input: &mut Option<I>,
        factory: F,
    ) -> Result<OwnedJob<ProcessCreationReport, ProcessCreationFailure>, AdmissionError>
    where
        I: Send + 'static,
        F: FnOnce(
                I,
                &JobContext,
                &ProcessCreationMemory,
            )
                -> Result<Box<dyn ProcessCreationDriver>, crate::native::NativeObservationError>
            + Send
            + 'static,
    {
        if spec.class != JobClass::InfrastructureLifecycle
            || spec.operation != Operation::NativeLifecycle
        {
            return Err(AdmissionError::WrongClass {
                operation: spec.operation,
                class: spec.class,
            });
        }
        if spec.deadline.is_none() {
            return Err(AdmissionError::DeadlineRequired);
        }
        if spec.replacement.is_some() {
            return Err(AdmissionError::InvalidReplacement {
                class: spec.class,
                replacement: spec.replacement,
            });
        }
        if input.is_none() {
            return Err(AdmissionError::NativeResources(
                sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired,
            ));
        }
        if spec.reservation_bytes < PROCESS_CREATION_STORAGE_BYTES
            || size_of::<I>()
                + size_of::<F>()
                + worker::storage_bytes()
                + ProcessCreationMemory::control_storage_bytes()
                > PROCESS_CREATION_STORAGE_BYTES as usize
        {
            return Err(AdmissionError::NativeReservationTooSmall {
                requested_bytes: spec.reservation_bytes,
                minimum_bytes: PROCESS_CREATION_STORAGE_BYTES,
            });
        }
        self.core
            .admit_owned(
                scope,
                spec,
                RequestKind::ProcessCreation(&source),
                input,
                |input, context, cancel, timing| {
                    let claim = match context.memory().claim(PROCESS_CREATION_STORAGE_BYTES) {
                        Ok(claim) => claim,
                        Err(cause) => return Err((AdmissionError::Memory(cause), input)),
                    };
                    let Some(original) = input.take() else {
                        return Err((AdmissionError::ShuttingDown, input));
                    };
                    let until = context.deadline().ok_or(AdmissionError::ShuttingDown);
                    let until = match until {
                        Ok(until) => until,
                        Err(cause) => {
                            *input = Some(original);
                            return Err((cause, input));
                        }
                    };
                    let memory = ProcessCreationMemory::new(
                        source.clone(),
                        context.clone(),
                        Arc::downgrade(&self.core),
                        claim,
                    );
                    let job = Job::new(
                        scope.id(),
                        spec,
                        timing.admitted,
                        cancel,
                        Arc::downgrade(&self.core),
                    );
                    if let Err(cause) = source.admitted(spec.id) {
                        *input = Some(original);
                        return Err((cause, input));
                    }
                    let factory =
                        Box::new(move |context: &JobContext, memory: &ProcessCreationMemory| {
                            factory(original, context, memory)
                        });
                    let work = Box::new(worker::Work {
                        factory: Some(factory),
                        job: job.clone(),
                        until,
                        held: source.held(),
                    });
                    Ok((
                        QueuedJob {
                            context,
                            job: job.clone(),
                            work: Work::Native(crate::native::NativeWork::ProcessCreation(
                                work, memory,
                            )),
                        },
                        OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))),
                    ))
                },
            )
            .map_err(|(cause, _)| cause)
    }
}
