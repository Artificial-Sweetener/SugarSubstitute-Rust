// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Admit and retire bounded scope ancestry independently of job queue transactions.

use super::*;
use crate::scope::ScopeHandle;
use sugarsubstitute_contracts::execution::{ScopeSnapshot, ScopeSpec};

impl Core {
    pub(crate) fn owned_scope<'a>(
        &self,
        state: &'a State,
        scope: &Scope,
        owner_id: ExecutionId,
    ) -> Result<&'a ScopeState, AdmissionError> {
        if owner_id != self.id {
            return Err(AdmissionError::ForeignScope {
                expected_owner: self.id,
                actual_owner: owner_id,
            });
        }
        state
            .scopes
            .get(&scope.id())
            .filter(|stored| stored.spec == scope.spec())
            .ok_or(AdmissionError::ScopeClosed { scope_id: scope.id() })
    }

    pub(crate) fn create_scope(
        self: &Arc<Self>,
        spec: ScopeSpec,
        parent: Option<&Scope>,
    ) -> Result<Scope, AdmissionError> {
        self.create_scope_kind(spec, parent, false)
    }

    pub(crate) fn create_scope_kind(
        self: &Arc<Self>,
        spec: ScopeSpec,
        parent: Option<&Scope>,
        native: bool,
    ) -> Result<Scope, AdmissionError> {
        let now = self.clock.retirement_now();
        let parent_owner = parent.map(Scope::owner_id);
        let mut state = lock(&self.state);
        if state.stopping || self.clock.failed() {
            return Err(AdmissionError::ShuttingDown);
        }
        if state.scopes.contains_key(&spec.id) {
            return Err(AdmissionError::DuplicateScopeId { scope_id: spec.id });
        }
        if let Some(latest) = state.latest_scope.filter(|latest| spec.id <= *latest) {
            return Err(AdmissionError::ScopeIdentityOrder { latest });
        }
        if state.scopes.values().filter(|scope| scope.native == native).count()
            >= if native { 132 } else { 64 }
        {
            return Err(AdmissionError::ScopeCapacity);
        }
        let (parent_id, parent_cancel, deadline) = match parent {
            Some(parent) => {
                let stored = self.owned_scope(&state, parent, parent_owner.unwrap_or(self.id))?;
                if stored.closed || stored.cancel.is_requested() {
                    return Err(AdmissionError::ScopeClosed { scope_id: parent.id() });
                }
                (
                    Some(parent.id()),
                    Some(stored.cancel.clone()),
                    earliest(spec.deadline, stored.deadline),
                )
            }
            None => (None, None, spec.deadline),
        };
        if let Some(deadline) = deadline.filter(|deadline| *deadline <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        let cancel = Cancellation::new(parent_cancel, Arc::downgrade(self));
        state.scopes.insert(
            spec.id,
            ScopeState {
                native,
                spec,
                parent: parent_id,
                deadline,
                cancel: cancel.clone(),
                closed: false,
                abandoned: false,
                bytes: 0,
            },
        );
        state.latest_scope = Some(spec.id);
        Ok(Scope::new(
            spec,
            Box::new(ScopeHandle { owner_id: self.id, id: spec.id, core: Arc::downgrade(self) }),
        ))
    }

    pub(crate) fn close_scope(&self, id: ScopeId) {
        let mut state = lock(&self.state);
        if let Some(scope) = state.scopes.get_mut(&id) {
            scope.closed = true;
            scope.abandoned = true;
        }
        Self::retire_scopes(&mut state);
        drop(state);
        #[cfg(any(windows, test))]
        self.stop_cancelled_capture(crate::capture_attach::control::CleanupTrigger::ScopeShutdown);
        self.changed();
    }

    pub(crate) fn scope_snapshot(&self, id: ScopeId) -> ScopeSnapshot {
        let state = lock(&self.state);
        let stored = state.scopes.get(&id);
        let count = |predicate: fn(JobPhase) -> bool| {
            state
                .jobs
                .values()
                .filter(|record| record.scope_id == id && predicate(record.phase))
                .count()
        };
        ScopeSnapshot {
            id,
            closed: stored.is_none_or(|scope| scope.closed),
            cancellation_requested: stored.is_none_or(|scope| scope.cancel.is_requested()),
            queued: count(|phase| phase == JobPhase::Queued),
            running: count(|phase| phase == JobPhase::Running),
            terminal_unobserved: state
                .jobs
                .values()
                .filter(|record| {
                    record.scope_id == id
                        && !record.acknowledged
                        && matches!(record.phase, JobPhase::Terminal(_))
                })
                .count(),
            children_live: state.scopes.values().filter(|scope| scope.parent == Some(id)).count(),
            reserved_bytes: stored.map_or(0, |scope| scope.bytes),
            native_reservations: state
                .symbol_resources
                .values()
                .filter(|resource| resource.scope_id == id)
                .count(),
            native_outputs_in_custody: state
                .symbol_resources
                .values()
                .filter(|resource| resource.scope_id == id)
                .map(|resource| usize::from(resource.observation.snapshot().0))
                .sum(),
        }
    }

    pub(crate) fn retire_scopes(state: &mut State) {
        loop {
            let candidate = state
                .scopes
                .iter()
                .find(|(id, scope)| {
                    scope.abandoned
                        && !scope.cancel.pending()
                        && scope.bytes == 0
                        && !state
                            .symbol_resources
                            .values()
                            .any(|resource| resource.scope_id == **id)
                        && !state.jobs.values().any(|job| job.scope_id == **id)
                        && !state.scopes.values().any(|child| child.parent == Some(**id))
                })
                .map(|(id, _)| *id);
            match candidate {
                Some(id) => {
                    state.scopes.remove(&id);
                }
                None => break,
            }
        }
    }
}
