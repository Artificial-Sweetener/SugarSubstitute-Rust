// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish typed terminal delivery for the single retained native pipe worker.
use super::*;
use crate::task::ErasedJob;

use crate::diagnostic_pipe::DiagnosticPipeReport;
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
    Diagnostic(Arc<Job<DiagnosticPipeReport, LifecyclePipeFailure>>),
}
impl Owner {
    pub(crate) fn cancel(&self) {
        match self {
            Self::Desktop(job) => job.cancel(),
            Self::Diagnostic(job) => job.cancel(),
        }
    }
    pub(crate) fn cancellation_requested(&self) -> bool {
        match self {
            Self::Desktop(job) => job.cancel.is_requested(),
            Self::Diagnostic(job) => job.cancel.is_requested(),
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
            Self::Desktop(job) => job.finish_at(
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
            Self::Diagnostic(job) => job.finish_at(
                context,
                Some(started),
                ended,
                project(
                    outcome,
                    DiagnosticPipeReport {
                        key: facts.binding.and_then(PeerBinding::diagnostic),
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
