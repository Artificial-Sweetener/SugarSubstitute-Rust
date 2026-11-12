// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish typed terminal delivery for the single retained native pipe worker.
use super::*;
use crate::task::ErasedJob;

use crate::purpose_pipe::PurposePipeReport;
use crate::{backend_custody::BackendCustodyPipeReport, recovery_supervisor::RecoveryPipeReport};
use sugarsubstitute_contracts::execution::{CancelControl, JobOutcome};
/// Copy only immutable retirement facts, releasing Shared before actual terminal delivery.
pub(crate) struct RetiredFacts {
    pub(crate) binding: Option<PeerBinding>,
    pub(crate) received: u64,
    pub(crate) written: u64,
}
#[derive(Clone)]
pub(crate) enum Owner {
    Desktop(Arc<Job<LifecyclePipeReport, LifecyclePipeFailure>>),
    Purpose(Arc<Job<PurposePipeReport, LifecyclePipeFailure>>),
    BackendCustody(Arc<Job<BackendCustodyPipeReport, LifecyclePipeFailure>>),
    Recovery(Arc<Job<RecoveryPipeReport, LifecyclePipeFailure>>),
}
impl Owner {
    /// Match the actual retained job allocation; IDs alone cannot associate a control.
    pub(crate) fn matches_record(&self, record: &crate::admission::Record) -> bool {
        let (scope, spec, erased): (_, _, Arc<dyn ErasedJob>) = match self {
            Self::Desktop(job) => (job.scope_id, job.spec, job.clone()),
            Self::Purpose(job) => (job.scope_id, job.spec, job.clone()),
            Self::BackendCustody(job) => (job.scope_id, job.spec, job.clone()),
            Self::Recovery(job) => (job.scope_id, job.spec, job.clone()),
        };
        record.scope_id == scope && record.spec == spec && Arc::ptr_eq(&record.job, &erased)
    }
    pub(crate) fn cancel(&self) {
        match self {
            Self::Desktop(job) => job.cancel(),
            Self::Purpose(job) => job.cancel(),
            Self::BackendCustody(job) => job.cancel(),
            Self::Recovery(job) => job.cancel(),
        }
    }
    pub(crate) fn cancellation_requested(&self) -> bool {
        match self {
            Self::Desktop(job) => job.cancel.is_requested(),
            Self::Purpose(job) => job.cancel.is_requested(),
            Self::BackendCustody(job) => job.cancel.is_requested(),
            Self::Recovery(job) => job.cancel.is_requested(),
        }
    }
    pub(crate) fn finish(
        &self,
        context: &JobContext,
        started: Moment,
        ended: Moment,
        outcome: JobOutcome<(), LifecyclePipeFailure>,
        facts: RetiredFacts,
    ) {
        match self {
            Self::Desktop(job) => job.finish_in_custody_at(
                context,
                Some(started),
                ended,
                project(
                    outcome,
                    LifecyclePipeReport {
                        run: facts.binding.and_then(PeerBinding::desktop),
                        received_messages: facts.received,
                        completed_writes: facts.written,
                    },
                ),
            ),
            Self::Purpose(job) => job.finish_in_custody_at(
                context,
                Some(started),
                ended,
                project(
                    outcome,
                    PurposePipeReport {
                        key: facts.binding.and_then(PeerBinding::purpose),
                        received_messages: facts.received,
                        completed_writes: facts.written,
                    },
                ),
            ),
            Self::BackendCustody(job) => job.finish_in_custody_at(
                context,
                Some(started),
                ended,
                project(
                    outcome,
                    BackendCustodyPipeReport {
                        key: facts.binding.and_then(PeerBinding::backend_custody),
                        received_messages: facts.received,
                        completed_writes: facts.written,
                    },
                ),
            ),
            Self::Recovery(job) => job.finish_in_custody_at(
                context,
                Some(started),
                ended,
                project(
                    outcome,
                    RecoveryPipeReport {
                        key: facts.binding.and_then(PeerBinding::recovery),
                        received_messages: facts.received,
                        completed_writes: facts.written,
                    },
                ),
            ),
        }
    }
}
fn project<T>(
    outcome: JobOutcome<(), LifecyclePipeFailure>,
    report: T,
) -> JobOutcome<T, LifecyclePipeFailure> {
    match outcome {
        JobOutcome::Success(()) => JobOutcome::Success(report),
        JobOutcome::Failure(error) => JobOutcome::Failure(error),
        JobOutcome::Panicked(cause) => JobOutcome::Panicked(cause),
        JobOutcome::Cancelled => JobOutcome::Cancelled,
        JobOutcome::TimedOut => JobOutcome::TimedOut,
        JobOutcome::Coalesced { replacement } => JobOutcome::Coalesced { replacement },
    }
}
