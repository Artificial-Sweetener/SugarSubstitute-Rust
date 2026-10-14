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
    pub fn submit_custodied_process<I, F>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        parent: &super::super::BackendCustodyParentStorage,
        input: &mut Option<I>,
        factory: F,
    ) -> Result<OwnedJob<CustodiedProcessReport, CustodiedProcessFailure>, AdmissionError>
    where
        I: Send + 'static,
        F: FnOnce(
                I,
                &JobContext,
                &CustodiedProcessMemory,
            )
                -> Result<Box<dyn CustodiedProcessDriver>, crate::native::NativeObservationError>
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
        if spec.reservation_bytes < CUSTODIED_PROCESS_STORAGE_BYTES
            || size_of::<I>()
                + size_of::<F>()
                + worker::storage_bytes()
                + CustodiedProcessMemory::control_storage_bytes()
                > CUSTODIED_PROCESS_STORAGE_BYTES as usize
        {
            return Err(AdmissionError::NativeReservationTooSmall {
                requested_bytes: spec.reservation_bytes,
                minimum_bytes: CUSTODIED_PROCESS_STORAGE_BYTES,
            });
        }
        self.core
            .admit_owned(
                scope,
                spec,
                RequestKind::CustodiedProcess(parent),
                input,
                |input, context, cancel, timing| {
                    let claim = match context.memory().claim(CUSTODIED_PROCESS_STORAGE_BYTES) {
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
                    let memory = CustodiedProcessMemory::new(
                        parent.clone(),
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
                    let factory =
                        Box::new(move |context: &JobContext, memory: &CustodiedProcessMemory| {
                            factory(original, context, memory)
                        });
                    let work =
                        Box::new(worker::Work { factory: Some(factory), job: job.clone(), until });
                    Ok((
                        QueuedJob {
                            context,
                            job: job.clone(),
                            work: Work::Native(crate::native::NativeWork::CustodiedProcess(
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
