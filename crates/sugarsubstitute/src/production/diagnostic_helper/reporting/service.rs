// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit finite report work and checked cleanup; retain actual central deliveries separately.
use super::{source::Source, *};
use crate::job_sequence::JobSequence;
use sugarsubstitute_execution::ExecutionClient;
#[path = "service/renewal.rs"]
mod renewal;
#[path = "service/restoration.rs"]
mod restoration;
#[path = "service/symbols.rs"]
mod symbols;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[path = "service/view.rs"]
mod view;
pub(in crate::production) use renewal::RetainedPassiveReporting;
pub(in crate::production) use restoration::PassiveReportingRestoration;
/// Categorical service failures preserve their original causes in the retained service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum ReportingIssue {
    Started,
    Identity,
    Busy,
    Admission,
}
impl std::fmt::Display for ReportingIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incident report service rejected: {self:?}")
    }
}
impl std::error::Error for ReportingIssue {}
#[derive(Default)]
struct Delivery {
    job: Option<OwnedJob<bool, ReportingIssue>>,
    completion: Option<JobCompletion<bool, ReportingIssue>>,
    terminal: Option<TerminalKind>,
}
impl Delivery {
    fn occupied(&self) -> bool {
        self.job.is_some() || self.completion.is_some()
    }
    fn capture(&mut self, batch: &CompletionBatch) -> Result<bool, ReportingIssue> {
        let Some(job) = &mut self.job else {
            return Ok(false);
        };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err(ReportingIssue::Identity);
            }
            self.terminal = Some(notice.kind);
            matched = true;
        }
        if self.terminal.is_some()
            && let Some(completion) = job.try_take_completion()
        {
            let valid = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && self.terminal == Some(completion.outcome.kind());
            self.completion = Some(completion);
            self.job = None;
            if !valid {
                return Err(ReportingIssue::Identity);
            }
        }
        Ok(matched)
    }
    fn succeeded(&self) -> bool {
        matches!(
            self.completion.as_ref().map(|value| &value.outcome),
            Some(JobOutcome::Success(true))
        )
    }
}
/// Original controller source, original Core completion, and independent cleanup delivery.
/// This parent owner never performs storage effects or waits for execution.
pub(in crate::production) struct ReportingService {
    source: Source,
    work: Delivery,
    cleanup: Delivery,
    presentation: Delivery,
    view_slot: Option<super::source::ViewSlot>,
    view_binding: Option<(ScopeId, Moment)>,
    installed: bool,
    requested: Option<(ExecutionId, ScopeId, Moment)>,
    cleanup_binding: Option<(ScopeId, Moment)>,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    restored: Option<PassiveReportingRestoration>,
}
impl ReportingService {
    /// Admit fixed16KiB source metadata inside the already admitted controller2MiB.
    /// Its two-slot metadata remains funded here; text belongs to finite presentation jobs.
    pub(in crate::production) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            source: Source::new(context)?,
            work: Delivery::default(),
            cleanup: Delivery::default(),
            presentation: Delivery::default(),
            view_slot: None,
            view_binding: None,
            installed: false,
            requested: None,
            cleanup_binding: None,
            admission: None,
            identity: None,
            restored: None,
        })
    }
    /// Install the genuine write loan and command once; admission rejection retains them.
    pub(in crate::production) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        input: &mut Option<ReportingInput>,
    ) -> Result<(), ReportingIssue> {
        if self.installed {
            return Err(ReportingIssue::Started);
        }
        if scope.owner_id() != self.source.owner() {
            return Err(ReportingIssue::Identity);
        }
        self.source.install(input)?;
        self.installed = true;
        self.retry(client, scope, jobs, until)
    }
    /// Retry only the original unexecuted input at the same execution/scope/deadline binding.
    pub(in crate::production) fn retry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), ReportingIssue> {
        if self.restored.is_some() || !self.installed || self.work.occupied() {
            return Err(ReportingIssue::Started);
        }
        let requested = (scope.owner_id(), scope.id(), until);
        if requested.0 != self.source.owner() || self.requested.is_some_and(|old| old != requested)
        {
            return Err(ReportingIssue::Identity);
        }
        self.requested = Some(requested);
        let id = self.allocate(jobs)?;
        let source = self.source.alias();
        let spec = Self::spec(id, until, DIAGNOSTIC_STORAGE_BYTES);
        match client.submit_blocking(scope, spec, move |context| {
            let (mut loan, input) = source.begin()?;
            let operation = loan.operation()?;
            operation.execute(input, &context);
            Ok(!operation.failed() && !operation.retiring_authentication)
        }) {
            Ok(job) => {
                self.work.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(ReportingIssue::Admission)
            }
        }
    }
    fn allocate(&mut self, jobs: &JobSequence) -> Result<JobId, ReportingIssue> {
        jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            ReportingIssue::Identity
        })
    }
    fn spec(id: JobId, until: Moment, reservation_bytes: u64) -> JobSpec {
        JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes,
            replacement: None,
        }
    }
    /// Consume only genuine matching work terminal notices and their exact completion once.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, ReportingIssue> {
        self.work.capture(batch)
    }
    /// Observe the actual delivered Core terminal; Success(false) remains the original result.
    /// The operation retains its distinct domain cause and constructor-private commit.
    pub(in crate::production) fn outcome(&self) -> Option<TerminalKind> {
        self.work.completion.as_ref().map(|done| done.outcome.kind())
    }
    /// Observe an accepted cleanup job or its retained completion, excluding rejected admission.
    pub(in crate::production) fn cleanup_started(&self) -> bool {
        self.cleanup.occupied()
    }
    /// Observe the actual delivered cleanup terminal independently of checked file retirement.
    pub(in crate::production) fn cleanup_outcome(&self) -> Option<TerminalKind> {
        self.cleanup.completion.as_ref().map(|done| done.outcome.kind())
    }
    /// Cancel the actual admitted job without releasing source or repository custody.
    pub(in crate::production) fn cancel(&self) {
        if let Some(job) = &self.work.job {
            job.cancel();
        }
        if let Some(job) = &self.presentation.job {
            job.cancel();
        }
    }
    /// Submit one finite32KiB close-only job after the original work completion is delivered.
    /// Its first cleanup cutoff and scope are immutable across central admission retries.
    pub(in crate::production) fn submit_cleanup(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), ReportingIssue> {
        if self.restored.is_some()
            || self.work.job.is_some()
            || self.work.completion.is_none()
            || self.cleanup.occupied()
            || self.presentation.job.is_some()
        {
            return Err(ReportingIssue::Busy);
        }
        if self.requested.is_none_or(|old| old.0 != scope.owner_id())
            || self.cleanup_binding.is_some_and(|old| old != (scope.id(), until))
        {
            return Err(ReportingIssue::Identity);
        }
        self.cleanup_binding = Some((scope.id(), until));
        let id = self.allocate(jobs)?;
        let source = self.source.alias();
        match client.submit_blocking(scope, Self::spec(id, until, 32 * 1024), move |context| {
            let mut loan = source.loan()?;
            Ok(loan.operation()?.retire(&context))
        }) {
            Ok(job) => {
                self.cleanup.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(ReportingIssue::Admission)
            }
        }
    }
    /// Consume cleanup delivery independently from work and native file retirement.
    pub(in crate::production) fn capture_cleanup(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, ReportingIssue> {
        self.cleanup.capture(batch)
    }
    /// Observe checked file cleanup with its actual delivered successful cleanup result.
    pub(in crate::production) fn files_retired(&self) -> bool {
        self.cleanup.succeeded()
            && self.source.inspect(ReportingOperation::files_retired) == Some(true)
    }
    /// Inspect passive retained products or causes only; no provider runs under the source lock.
    pub(in crate::production) fn inspect<T>(
        &self,
        observe: impl FnOnce(&ReportingOperation) -> T,
    ) -> Option<T> {
        self.source.inspect(observe)
    }
    /// Transfer the same charged view after genuine successful completion and checked cleanup.
    pub(in crate::production) fn take_view(&mut self) -> Option<PreparedReportView> {
        if !self.files_retired()
            || !self.work.succeeded()
            || !self.presentation.succeeded()
            || self.view_slot.is_none()
        {
            return None;
        }
        let view = self.source.take_view()?;
        let completion = self.presentation.completion.take()?;
        let slot = self.view_slot.take()?;
        Some(PreparedReportView::new(view, completion, slot))
    }
    /// Return unchanged received/issued tokens once after successful actual mutation and cleanup.
    /// Original Core and cleanup completions remain retained until exact product adoption.
    pub(in crate::production) fn take_authentication(&mut self) -> Option<ReportingAuthentication> {
        if !self.files_retired() || !self.work.succeeded() {
            return None;
        }
        self.source.take_authentication()
    }
    /// Return the original guard for close-only disposal after genuine Core/cleanup delivery.
    /// The caller must first observe the actual same Reporter process settled or never launched.
    /// This grants no successful mutation, acknowledgement, adoption, or subsequent source reuse.
    pub(in crate::production) fn take_retiring_authentication(
        &mut self,
    ) -> Option<ReportingAuthentication> {
        if !self.files_retired()
            || self.work.job.is_some()
            || self.work.completion.is_none()
            || self.cleanup.job.is_some()
            || self.presentation.job.is_some()
        {
            return None;
        }
        self.source.take_retiring_authentication()
    }
    /// Move the exact original first cause after genuine Core/cleanup delivery and checked close.
    /// Its original Core lease and immutable failed source fence survive this one-time transfer.
    pub(in crate::production) fn take_failure(&mut self) -> Option<OwnedReportingFailure> {
        if !self.files_retired() || self.work.job.is_some() || self.cleanup.job.is_some() {
            return None;
        }
        let memory = self.work.completion.as_ref()?.memory.clone();
        self.source.take_failure(memory)
    }
    /// Release original Core data/causes only after the caller adopts the exact real product.
    /// The fixed controller metadata stays funded for the next operation.
    pub(in crate::production) fn release_adopted(&mut self) -> Result<(), ReportingIssue> {
        if !self.files_retired()
            || !self.work.succeeded()
            || self.presentation.job.is_some()
            || self.view_slot.is_some()
        {
            return Err(ReportingIssue::Busy);
        }
        self.source.release_adopted()?;
        self.work = Delivery::default();
        self.cleanup = Delivery::default();
        self.presentation = Delivery::default();
        self.view_binding = None;
        self.installed = false;
        self.requested = None;
        self.cleanup_binding = None;
        Ok(())
    }
}
