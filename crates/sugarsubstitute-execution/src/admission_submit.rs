// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Admit factories and native permits atomically before ownership or external effects transfer.

use super::*;

impl Core {
    pub(crate) fn admit<R>(
        self: &Arc<Self>,
        scope: &Scope,
        spec: JobSpec,
        kind: RequestKind,
        build: impl FnOnce(JobContext, Arc<Cancellation>, Moment) -> (QueuedJob, R),
    ) -> Result<R, AdmissionError> {
        let now = self.clock.now();
        let owner_id = scope.owner_id();
        let mut state = lock(&self.state);
        if state.stopping {
            return Err(AdmissionError::ShuttingDown);
        }
        let stored = self.owned_scope(&state, scope, owner_id)?;
        if stored.closed || stored.cancel.is_requested() {
            return Err(AdmissionError::ScopeClosed { scope_id: scope.id() });
        }
        let deadline = earliest(stored.deadline, spec.deadline);
        let parent_cancel = stored.cancel.clone();
        if let Some(deadline) = deadline.filter(|deadline| *deadline <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        let valid_method = matches!(
            (kind, spec.class),
            (RequestKind::Async, JobClass::IoOrdinary | JobClass::IoLifecycle)
                | (
                    RequestKind::Blocking,
                    JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle
                )
                | (RequestKind::Cpu, JobClass::CpuInteractive | JobClass::CpuDecode)
        );
        #[cfg(any(windows, test))]
        let valid_method = valid_method
            || matches!((kind, spec.class), (RequestKind::Native, JobClass::NativeObservation));
        if !valid_method || !spec.validates_class() {
            return Err(AdmissionError::WrongClass {
                operation: spec.operation,
                class: spec.class,
            });
        }
        if !spec.validates_replacement() {
            return Err(AdmissionError::InvalidReplacement {
                class: spec.class,
                replacement: spec.replacement,
            });
        }
        if spec.class == JobClass::NativeObservation {
            if !stored.native {
                return Err(AdmissionError::NativeScopeRequired);
            }
            if spec.reservation_bytes < 4096 {
                return Err(AdmissionError::NativeReservationTooSmall {
                    requested_bytes: spec.reservation_bytes,
                    minimum_bytes: 4096,
                });
            }
        } else if stored.native {
            return Err(AdmissionError::WrongClass {
                operation: spec.operation,
                class: spec.class,
            });
        }
        if state.jobs.contains_key(&spec.id) {
            return Err(AdmissionError::DuplicateJobId { job_id: spec.id });
        }
        if let Some(latest) = state.latest_job.filter(|latest| spec.id <= *latest) {
            return Err(AdmissionError::JobIdentityOrder { latest });
        }
        if let Some(key) = spec.replacement
            && let Some(current_revision) = state
                .jobs
                .values()
                .filter_map(|record| record.spec.replacement)
                .filter(|current| current.surface == key.surface)
                .map(|current| current.revision)
                .max()
                .filter(|current| *current >= key.revision)
        {
            return Err(AdmissionError::StaleReplacement { replacement: key, current_revision });
        }
        let replacement_index = spec.replacement.and_then(|key| {
            state.queue.iter().position(|queued| {
                queued.context.spec().class == JobClass::CpuInteractive
                    && queued
                        .context
                        .spec()
                        .replacement
                        .is_some_and(|pending| pending.surface == key.surface)
            })
        });
        Self::check_capacity(&mut state, spec, replacement_index.is_some())?;
        state.bytes += spec.reservation_bytes;
        if let Some(stored) = state.scopes.get_mut(&scope.id()) {
            stored.bytes += spec.reservation_bytes;
        }
        let memory = MemoryLease::new(
            spec.reservation_bytes,
            Box::new(Release { core: self.clone(), scope_id: scope.id() }),
        );
        let cancel = Cancellation::new(Some(parent_cancel), Arc::downgrade(self));
        let context = JobContext::new(
            self.id,
            scope.id(),
            spec,
            deadline,
            cancel.clone(),
            self.clock.clone(),
            memory,
        );
        let (queued, result) = build(context, cancel, now);
        state.jobs.insert(
            spec.id,
            Record {
                deadline,
                scope_id: scope.id(),
                spec,
                phase: JobPhase::Queued,
                job: queued.job.clone(),
                retiring: false,
                acknowledged: false,
            },
        );
        state.latest_job = Some(spec.id);
        let replaced = replacement_index.and_then(|index| state.queue.remove(index));
        if let Some(replaced) = replaced {
            if let Some(record) = state.jobs.get_mut(&replaced.context.job_id()) {
                record.retiring = true;
            }
            state.retired.push_back(Retirement::Factory(replaced, Stop::Coalesced(spec.id)));
        }
        state.queue.push_back(queued);
        drop(state);
        self.changed();
        Ok(result)
    }
}
