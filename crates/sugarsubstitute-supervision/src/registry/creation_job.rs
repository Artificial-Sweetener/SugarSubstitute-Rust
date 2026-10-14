// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume either original finite creation owner through its genuine typed completion.
use super::*;
use sugarsubstitute_contracts::execution::{JobCompletion, JobOutcome, ScopeId};
use sugarsubstitute_execution::backend_custody::process::{
    CustodiedProcessFailure, CustodiedProcessReport,
};
pub(super) enum CreationJob {
    Blocking(OwnedJob<(), RegistryError>),
    Custodied(OwnedJob<CustodiedProcessReport, CustodiedProcessFailure>),
}
impl CreationJob {
    pub(super) fn cancel(&self) {
        match self {
            Self::Blocking(job) => job.cancel(),
            Self::Custodied(job) => job.cancel(),
        }
    }
    pub(super) fn scope_id(&self) -> ScopeId {
        match self {
            Self::Blocking(job) => job.scope_id(),
            Self::Custodied(job) => job.scope_id(),
        }
    }
    pub(super) fn spec(&self) -> JobSpec {
        match self {
            Self::Blocking(job) => job.spec(),
            Self::Custodied(job) => job.spec(),
        }
    }
    pub(super) fn try_take_completion(&mut self) -> Option<JobCompletion<(), RegistryError>> {
        match self {
            Self::Blocking(job) => job.try_take_completion(),
            Self::Custodied(job) => job.try_take_completion().map(|completion| JobCompletion {
                scope_id: completion.scope_id,
                spec: completion.spec,
                queue_time: completion.queue_time,
                execution_time: completion.execution_time,
                memory: completion.memory,
                native_handles: completion.native_handles,
                outcome: match completion.outcome {
                    JobOutcome::Success(report) => {
                        drop(report);
                        JobOutcome::Success(())
                    }
                    JobOutcome::Failure(cause) => {
                        JobOutcome::Failure(RegistryError::new("custodied_creation", cause))
                    }
                    JobOutcome::Cancelled => JobOutcome::Cancelled,
                    JobOutcome::TimedOut => JobOutcome::TimedOut,
                    JobOutcome::Coalesced { replacement } => JobOutcome::Coalesced { replacement },
                    JobOutcome::Panicked(cause) => JobOutcome::Panicked(cause),
                },
            }),
        }
    }
}
