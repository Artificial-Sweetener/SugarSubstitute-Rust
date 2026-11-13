// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match observed facts to one genuine admitted child job; facts grant no custody.
use super::ChildCaptureError;
use sugarsubstitute_contracts::{
    execution::{
        CompletionNotification, ExecutionId, JobCompletion, JobId, JobSpec, ScopeId, TerminalKind,
    },
    lifecycle::RunKey,
};
/// Keep original admission facts independently of actual result and source ownership.
pub(super) struct CaptureCorrelation {
    run: RunKey,
    execution: ExecutionId,
    scope: ScopeId,
    spec: JobSpec,
    notice: Option<TerminalKind>,
}
impl CaptureCorrelation {
    /// Retain facts from the successful original reservation and composing Scope.
    pub(super) const fn new(
        run: RunKey,
        execution: ExecutionId,
        scope: ScopeId,
        spec: JobSpec,
    ) -> Self {
        Self { run, execution, scope, spec, notice: None }
    }
    /// Ignore foreign jobs; preserve the first exact terminal kind on conflict.
    pub(super) fn notice(
        &mut self,
        notice: CompletionNotification,
    ) -> Result<(), ChildCaptureError> {
        if (notice.scope_id, notice.job_id) != (self.scope, self.spec.id) {
            return Ok(());
        }
        if self.notice.is_some_and(|first| first != notice.kind) || notice.replacement.is_some() {
            return Err(ChildCaptureError::Identity);
        }
        self.notice = Some(notice.kind);
        Ok(())
    }
    /// Permit polling of the real typed handle only after matching metadata.
    pub(super) const fn noticed(&self) -> bool {
        self.notice.is_some()
    }
    /// Compare the original owned completion, including its complete memory reservation.
    /// Desktop capture has its own two-output owner, so SymbolHelper's optional
    /// handle ledger is absent. This observation grants no native closure or Ready.
    pub(super) fn completion<T, E>(&self, completion: &JobCompletion<T, E>) -> bool {
        self.terminal(completion.scope_id, completion.spec, completion.outcome.kind())
            && completion.memory.bytes() == self.spec.reservation_bytes
            && completion.native_handles.is_none()
    }
    /// Compare immutable terminal metadata before inspecting any owned result.
    pub(super) fn terminal(&self, scope: ScopeId, spec: JobSpec, kind: TerminalKind) -> bool {
        scope == self.scope && spec == self.spec && self.notice == Some(kind)
    }
    /// Match facts borrowed from the worker-constructed successful registration.
    pub(super) fn attached(&self, run: RunKey, execution: ExecutionId, job: JobId) -> bool {
        run == self.run && execution == self.execution && job == self.spec.id
    }
}
