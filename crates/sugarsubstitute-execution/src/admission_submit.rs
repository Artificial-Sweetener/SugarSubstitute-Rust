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
        self.admit_owned(scope, spec, kind, build, |build, context, cancel, now| {
            build(context, cancel, now)
        })
        .map_err(|(error, _build)| error)
    }

    pub(crate) fn admit_owned<M, R>(
        self: &Arc<Self>,
        scope: &Scope,
        spec: JobSpec,
        kind: RequestKind,
        input: M,
        build: impl FnOnce(M, JobContext, Arc<Cancellation>, Moment) -> (QueuedJob, R),
    ) -> Result<R, (AdmissionError, M)> {
        let symbol =
            matches!(&kind, RequestKind::SymbolCleanup { .. } | RequestKind::SymbolHelper { .. });
        #[cfg(any(windows, test))]
        let symbol = symbol || matches!(&kind, RequestKind::SymbolNative { .. });
        let now = if symbol {
            match self.clock.metadata() {
                Ok(now) => now,
                Err(cause) => return Err((AdmissionError::Unexpected(cause), input)),
            }
        } else {
            self.clock.retirement_now()
        };
        let owner_id = scope.owner_id();
        let mut state = lock(&self.state);
        let validated = (|| {
            if state.stopping || self.clock.failed() {
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
                (&kind, spec.class),
                (RequestKind::Async, JobClass::IoOrdinary | JobClass::IoLifecycle)
                    | (
                        RequestKind::Blocking,
                        JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle
                    )
                    | (RequestKind::Cpu, JobClass::CpuInteractive | JobClass::CpuDecode)
            );
            #[cfg(any(windows, test))]
            let valid_method = valid_method
                || matches!(
                    (&kind, spec.class),
                    (
                        RequestKind::Native
                            | RequestKind::CaptureAttach(_)
                            | RequestKind::SymbolNative { .. },
                        JobClass::NativeObservation
                    )
                );
            let valid_method = valid_method
                || matches!(
                    (&kind, spec.class),
                    (RequestKind::SymbolCleanup { .. }, JobClass::InfrastructureLifecycle)
                        | (RequestKind::SymbolHelper { .. }, JobClass::InfrastructureOrdinary)
                );
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
            #[cfg(any(windows, test))]
            if let RequestKind::CaptureAttach(run) = &kind
                && state.jobs.values().any(|record| {
                    record
                        .native_custody
                        .as_ref()
                        .is_some_and(|custody| custody.full_run == *run && custody.held())
                })
            {
                return Err(AdmissionError::ConflictingNativeRun(*run));
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
                return Err(AdmissionError::StaleReplacement {
                    replacement: key,
                    current_revision,
                });
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
            crate::symbol_admission::validate(self, &state, scope, spec, &kind, now)?;
            Self::check_capacity(&mut state, spec, replacement_index.is_some())?;
            Ok((deadline, parent_cancel, replacement_index))
        })();
        let (deadline, parent_cancel, replacement_index) = match validated {
            Ok(validated) => validated,
            Err(error) => return Err((error, input)),
        };
        state.bytes += spec.reservation_bytes;
        if let Some(stored) = state.scopes.get_mut(&scope.id()) {
            stored.bytes += spec.reservation_bytes;
        }
        let memory = MemoryLease::new(
            spec.reservation_bytes,
            Box::new(Release { core: self.clone(), scope_id: scope.id() }),
        );
        let symbol_release = if matches!(&kind, RequestKind::SymbolHelper { .. }) {
            match crate::native_resources::release(self.clone(), spec.id, &memory) {
                Ok(release) => Some(release),
                Err(cause) => {
                    drop(state);
                    drop(memory);
                    return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input));
                }
            }
        } else {
            None
        };
        let cancel = Cancellation::new(Some(parent_cancel), Arc::downgrade(self));
        let context = JobContext::new(
            self.id,
            scope.id(),
            spec,
            deadline,
            cancel.clone(),
            self.clock.clone(),
            sugarsubstitute_contracts::execution::AdmittedJobResources::new(memory, symbol_release),
        );
        if let Some(lease) = context.native_handles() {
            state.symbol_resources.insert(
                spec.id,
                crate::native_resources::ResourceRecord {
                    scope_id: scope.id(),
                    observation: lease.observation(),
                },
            );
        }
        if let RequestKind::SymbolHelper { cleanup } = &kind
            && let Some(binding) =
                state.jobs.get_mut(cleanup).and_then(|record| record.symbol_cleanup.as_mut())
        {
            binding.paired = true;
        }
        let (queued, result) = build(input, context, cancel, now);
        state.jobs.insert(
            spec.id,
            Record {
                symbol_original: match &kind {
                    RequestKind::SymbolNative { original, .. } => Some(original.job_id()),
                    _ => None,
                },
                symbol_cleanup: match &kind {
                    RequestKind::SymbolCleanup { main_scope, main_spec } => {
                        Some(crate::symbol_cleanup::Binding {
                            main_scope: *main_scope,
                            main_spec: *main_spec,
                            paired: false,
                            original: None,
                        })
                    }
                    _ => None,
                },
                #[cfg(any(windows, test))]
                native_custody: match &queued.work {
                    crate::task::Work::Native(work) => work.initial_custody(),
                    _ => None,
                },
                #[cfg(any(windows, test))]
                native_deadlines: match &queued.work {
                    crate::task::Work::Native(work) => work.initial_deadlines(),
                    _ => None,
                },
                deadline,
                scope_id: scope.id(),
                spec,
                phase: JobPhase::Queued,
                job: queued.job.clone(),
                retiring: false,
                acknowledged: false,
                terminal_published: false,
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
