// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit a fresh captured family under the unchanged Root before acknowledging Restart.
//! The selected Reporter, old retired family, new grant and all finite work remain owned
//! until their exact checked transitions complete; unrelated records keep progressing.
#[path = "recovery_supervisor/admission.rs"]
mod admission;
#[path = "recovery_supervisor/closing.rs"]
mod closing;
#[path = "recovery_supervisor/origin.rs"]
mod origin;
#[path = "recovery_supervisor/preparation.rs"]
mod preparation;
#[path = "recovery_supervisor/projecting.rs"]
mod projecting;
#[path = "recovery_supervisor/session.rs"]
mod session;
#[path = "recovery_supervisor/start.rs"]
pub(super) mod start;
use super::incident_flow::transactions::{DiagnosticTransaction, DiagnosticTransactionRole};
use super::*;
use origin::OriginPublication;
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::Moment,
    recovery_supervisor::{RecoveryGrantId, RecoveryGrantKey},
    startup::MainIdentity,
};
use sugarsubstitute_execution::RegisteredRecoveryGrant;
/// Own one selected actual request and its original finite preparation lifetime.
pub(super) struct RetainedPreparation {
    pending: Box<RecoveryPreparation>,
    _outside: MemoryLease,
}
impl std::ops::Deref for RetainedPreparation {
    type Target = RecoveryPreparation;
    fn deref(&self) -> &Self::Target {
        &self.pending
    }
}
impl std::ops::DerefMut for RetainedPreparation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pending
    }
}
/// Keep one origin operation's application admission through actual native publication retirement.
struct OriginAdmission {
    scope: sugarsubstitute_application::incident::IncidentOperationScope,
    transaction: Option<DiagnosticTransaction>,
}
/// Keep the selected original source and its native preparation state together.
pub(super) struct RecoveryPreparation {
    at: usize,
    selected: incident_reporting::DeferredFreshRestart,
    previous: RunKey,
    identity: MainIdentity,
    connection: ConnectionId,
    original: EntrySetupBudget,
    job: Option<OwnedJob<ProjectedEntry, ProjectionFailure>>,
    completion: Option<JobCompletion<ProjectedEntry, ProjectionFailure>>,
    grant: Option<RegisteredRecoveryGrant>,
    record: Option<RetainedRecord>,
    origin: Option<OriginPublication>,
    origin_rejected: Option<origin::OriginPublicationRejection>,
    origin_scope: Option<OriginAdmission>,
    retired_seed: Option<incident_flow::RetiredRecoverySeed>,
    seed_settled: bool,
    old_shutdown: Option<ShutdownBudget>,
    cleanup: Option<ShutdownBudget>,
    ack_requested: bool,
    failed: Option<Box<dyn Error>>,
    cleanup_failure: Option<Box<dyn Error>>,
    rejected: bool,
    installed: bool,
    _storage: MemoryClaim,
}
impl RecoveryPreparation {
    /// Keep the original startup/cleanup candidates; overdue work remains independently owned.
    pub(super) fn deadlines(&self) -> impl Iterator<Item = Moment> + '_ {
        (!self.installed && self.failed.is_none())
            .then_some(self.original.until())
            .into_iter()
            .chain(
                self.old_shutdown
                    .into_iter()
                    .flat_map(|value| [value.graceful_until, value.settle_until]),
            )
            .chain(
                self.cleanup
                    .into_iter()
                    .flat_map(|value| [value.graceful_until, value.settle_until]),
            )
    }
    /// Actual worker/native failure classification remains distinct from local rejection.
    pub(super) fn owner_failed(&self) -> bool {
        self.retired_seed.as_ref().is_some_and(incident_flow::RetiredRecoverySeed::owner_failed)
            || self
                .record
                .as_ref()
                .and_then(|held| held.record.start.as_ref())
                .is_some_and(DesktopStart::recovery_owner_failed)
    }
    /// Borrow the independently retained native/control cause without moving its custody.
    pub(super) fn owner_failure(&self) -> Option<&(dyn Error + 'static)> {
        self.retired_seed
            .as_ref()
            .and_then(incident_flow::RetiredRecoverySeed::failure)
            .or_else(|| self.record.as_ref()?.record.start.as_ref()?.recovery_failure())
    }
    /// Borrow actual retained origin/rejection and native/control causes before compact issue codes.
    pub(super) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        self.origin_rejected
            .as_ref()
            .map(|cause| cause as &dyn Error)
            .or_else(|| self.origin.as_ref().and_then(OriginPublication::failure))
            .or_else(|| self.owner_failure())
            .or(self.failed.as_deref())
            .or(self.cleanup_failure.as_deref())
    }
    pub(super) fn preserve(&mut self, cause: Box<dyn Error>) {
        if self.failed.is_none() {
            tracing::error!(
                operation = "fresh_recovery_preparation_failed",
                run_instance = self.identity.run.instance.get(),
                run_generation = self.identity.run.generation.get(),
                "Fresh recovery preparation retained its original cause"
            );
            self.failed = Some(cause);
        }
    }
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        if let Some(completion) = self.job.as_mut().and_then(OwnedJob::try_take_completion) {
            self.completion = Some(completion);
            self.job = None;
        }
        let mut first = None;
        if let Some(origin) = &mut self.origin
            && let Err(cause) = origin.capture(batch)
        {
            first = Some(Box::<dyn Error>::from(cause));
        }
        if let Some(record) = &mut self.record {
            if let Err(cause) = record.record.seed.capture(batch) {
                first.get_or_insert(cause);
            }
            if let Err(cause) = record.record.collection.capture(batch) {
                first.get_or_insert(cause);
            }
            if let Some(start) = &mut record.record.start {
                match start.profile.consume(batch) {
                    Err(cause) => {
                        first.get_or_insert(Box::new(cause));
                    }
                    Ok(Some(completion)) => {
                        if let Some(cause) = completion.failure {
                            first.get_or_insert(Box::new(cause));
                        } else if completion.kind
                            != sugarsubstitute_contracts::execution::TerminalKind::Success
                            && self.failed.is_none()
                            && self.cleanup.is_none()
                        {
                            first.get_or_insert("Actual pending recovery profile operation stopped with original completion retained".into());
                        }
                    }
                    Ok(None) => {}
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
}
