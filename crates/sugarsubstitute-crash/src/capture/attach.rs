// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reserve the actual registration-lifetime source before central capture admission.
use super::*;
use crate::capture_endpoint::{RegisteredSource, SourceFailure};
use sugarsubstitute_contracts::execution::{JobSpec, Scope};
use sugarsubstitute_execution::{
    ExecutionClient,
    windows_capture_attach::{
        CaptureAttachAdmissionRejection, CaptureAttachExpectation, CaptureAttachFailure,
        CaptureAttachLaunchPin, CaptureEndpointSource, CaptureEndpointSourceError,
    },
};

/// Preserve source rejection, unchanged unadmitted authority or an already admitted job.
pub enum CaptureBootstrapRejection {
    /// Registration/source validation failed before creating native attachment work.
    Source(CaptureEndpointSourceError),
    /// Central rejection retains the exact expectation and source for explicit retry.
    Admission(Box<CaptureAttachAdmissionRejection>),
    /// Unexpected association failure retains the actual admitted job for observation.
    /// The original job is cancelled and its native owner still performs checked retirement.
    Association {
        /// Original exact source-association cause.
        cause: CaptureEndpointSourceError,
        /// Original admitted lifetime; result drop cannot erase actual native custody.
        job: OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
    },
}
impl std::fmt::Debug for CaptureBootstrapRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "CaptureBootstrapSourceRejected { redacted }",
            Self::Admission(_) => "CaptureBootstrapAdmissionRejected { redacted }",
            Self::Association { .. } => "CaptureBootstrapAssociationRejected { redacted }",
        })
    }
}
impl std::fmt::Display for CaptureBootstrapRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Capture bootstrap source admission rejected")
    }
}
impl std::error::Error for CaptureBootstrapRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(cause) | Self::Association { cause, .. } => Some(cause),
            Self::Admission(rejection) => Some(&rejection.error),
        }
    }
}
fn rejected(cause: SourceFailure) -> Box<CaptureBootstrapRejection> {
    Box::new(CaptureBootstrapRejection::Source(CaptureEndpointSourceError::new(
        "capture_bootstrap_source",
        cause,
    )))
}
impl WindowsCrashCapture {
    /// Reserve the actual source ledger and submit one unchanged capture expectation.
    ///
    /// The registered supervisor, original executor, build and handler watch remain
    /// authoritative. Source creation performs no getter, loader or native query.
    /// The producer and native worker commit one identical association; a worker-first
    /// retirement cannot be reversed by a late successful producer return.
    ///
    /// # Errors
    /// Source validation/replay/overload rejects before native work. Central rejection
    /// returns the exact original source/expectation for retry through the execution
    /// capture boundary. Unexpected admitted association failure retains the original
    /// cancelled job and cause; its actual worker owns cleanup and terminal observation.
    pub fn request_attach_bootstrap(
        &self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        expected: CaptureAttachExpectation,
    ) -> Result<
        OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
        Box<CaptureBootstrapRejection>,
    > {
        let registered = self.reserve_attach_source(scope, expected)?;
        let source: Arc<dyn CaptureEndpointSource> = registered.clone();
        let job = execution
            .reserve_capture_attach_pin(scope, spec, expected, source)
            .map_err(|rejection| Box::new(CaptureBootstrapRejection::Admission(rejection)))?;
        commit_admitted(registered, scope.owner_id(), job)
    }
    /// Validate actual registration and reserve one unchanged private source view.
    pub(super) fn reserve_attach_source(
        &self,
        scope: &Scope,
        expected: CaptureAttachExpectation,
    ) -> Result<Arc<RegisteredSource>, Box<CaptureBootstrapRejection>> {
        if self.snapshot.state != CaptureState::Registered
            || self.watch.is_none()
            || self.snapshot.handler_exit.is_some()
        {
            return Err(rejected(SourceFailure::Closing));
        }
        let shared = self.shared.as_ref().ok_or_else(|| rejected(SourceFailure::Closing))?;
        if expected.owner() != self.owner
            || expected.run().owner != self.owner
            || scope.owner_id() != shared.execution_id
            || expected.build_bytes() != &shared.build[..shared.build_length]
            || self.snapshot.handler.is_none_or(|handler| {
                expected.handler().process_id() != handler.process_id
                    || expected.handler().creation_time_100ns() != handler.creation_time_100ns
            })
        {
            return Err(rejected(SourceFailure::ForeignAssociation));
        }
        {
            let native = lock(&shared.native);
            if !native.registered
                || native.retired
                || native.lease.is_none()
                || native.handler != self.snapshot.handler
            {
                return Err(rejected(SourceFailure::Closing));
            }
        }
        RegisteredSource::reserve(shared, expected.run(), expected.attach_budget())
            .map_err(rejected)
    }
}

/// Finish one actual admitted association without reversing worker-first retirement.
pub(super) fn commit_admitted(
    registered: Arc<RegisteredSource>,
    execution_id: sugarsubstitute_contracts::execution::ExecutionId,
    job: OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
) -> Result<OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>, Box<CaptureBootstrapRejection>>
{
    if let Err(cause) = registered.commit(execution_id, job.spec().id) {
        job.cancel();
        return Err(Box::new(CaptureBootstrapRejection::Association {
            cause: CaptureEndpointSourceError::new("capture_bootstrap_association", cause),
            job,
        }));
    }
    Ok(job)
}
