// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit one development qualification trap from actual registered Desktop custody.
//! The default-off feature exposes no trap, seal constructor, or supplied callback.
pub(crate) mod admission;

mod effect;
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod windows_trap;

use crate::{
    DesktopCaptureSession, ExecutionClient,
    admission::RequestKind,
    native::NativeObservationError,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::{execution::*, lifecycle::RunKey};

/// Preserve the actual owner/native cause if the registered trap cannot run or returns.
#[derive(Debug)]
pub enum RegisteredDesktopFaultError {
    /// The original clock, cancellation, record or capture custody vetoed this effect.
    Stopped(AdmissionError),
    /// The original Windows mode operation or restoration failed.
    Native(NativeObservationError),
    /// Preserve both the original stop/returned-trap failure and mode restoration cause.
    Restoration {
        /// Original qualification failure, without replacing its cause.
        first: Box<RegisteredDesktopFaultError>,
        /// Actual failure restoring this same worker's previous thread error mode.
        cause: NativeObservationError,
    },
    /// A debugger or other handler resumed after the authored breakpoint.
    Returned,
    /// The development trap requires the Windows x64 native owner.
    Platform,
}
impl fmt::Display for RegisteredDesktopFaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped(cause) => cause.fmt(f),
            Self::Native(cause) => cause.fmt(f),
            Self::Restoration { first, cause } => {
                write!(f, "{first}; thread mode restoration failed: {cause}")
            }
            Self::Returned => f.write_str("Registered Desktop qualification breakpoint returned"),
            Self::Platform => f.write_str("Registered Desktop qualification requires Windows x64"),
        }
    }
}
impl Error for RegisteredDesktopFaultError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Stopped(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Restoration { first, .. } => Some(first.as_ref()),
            _ => None,
        }
    }
}
impl ExecutionClient {
    /// Admit one finite qualification effect from the actual authenticated registered Desktop.
    /// `spec` must be NativeLifecycle/InfrastructureLifecycle, exactly 4096 bytes, no
    /// replacement, and the unchanged original capture cutoff in the same source scope.
    /// Admission uses the existing central queue, memory, delivery and worker limits.
    /// No UI-thread wait occurs. Cancellation/shutdown and native closure remain observed.
    /// # Errors
    /// Foreign, unauthenticated, stale, retired, cancelled, expired or repeated sources,
    /// and actual central saturation reject before consuming the once-only source flag.
    /// A debugger-resumed breakpoint produces an explicit terminal failure without retry.
    pub fn submit_registered_desktop_fault(
        &self,
        scope: &Scope,
        spec: JobSpec,
        capture: &DesktopCaptureSession,
        run: RunKey,
    ) -> Result<OwnedJob<(), RegisteredDesktopFaultError>, AdmissionError> {
        let custody = capture.qualification_custody();
        let source = custody.clone();
        let core = self.core.clone();
        self.core
            .admit_owned(
                scope,
                spec,
                RequestKind::RegisteredDesktopFault { custody: &custody, run },
                (source, core),
                |(source, core), context, cancel, timing| {
                    let storage = match context.memory().claim(effect::WORK_BYTES) {
                        Ok(storage) => storage,
                        Err(cause) => return Err((AdmissionError::Memory(cause), (source, core))),
                    };
                    let worker = effect::Worker { source, core, run, _storage: storage };
                    let job = Job::new(
                        scope.id(),
                        spec,
                        timing.admitted,
                        cancel,
                        Arc::downgrade(&self.core),
                    );
                    let factory_job = job.clone();
                    let queued = QueuedJob {
                        context,
                        job: job.clone(),
                        work: Work::Blocking(Box::new(move |context, started| {
                            Box::pin(crate::blocking::run_source_first(
                                factory_job,
                                context,
                                started,
                                move |context| worker.run(context),
                            ))
                        })),
                    };
                    Ok((queued, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job)))))
                },
            )
            .map_err(|(cause, _original)| cause)
    }
}
