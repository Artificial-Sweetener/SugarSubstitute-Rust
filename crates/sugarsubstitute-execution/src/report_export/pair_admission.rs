// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit both fixed export roles in the same Core transaction before factory transfer.
use super::*;
use crate::{
    report_export::pair::{Association, Work as PairWork},
    task::{Job, JobHandle, Work},
};
use sugarsubstitute_contracts::execution::{AdmittedJobResources, OwnedJob};

fn descendant(state: &State, mut child: ScopeId, ancestor: ScopeId) -> bool {
    loop {
        if child == ancestor {
            return true;
        }
        match state.scopes.get(&child).and_then(|scope| scope.parent) {
            Some(parent) => child = parent,
            None => return false,
        }
    }
}

impl Core {
    pub(crate) fn admit_report_export_pair<E, M>(
        self: &Arc<Self>,
        source: crate::report_export::pair::MainRequest<'_>,
        receiving: crate::report_export::pair::MainRequest<'_>,
        factory: M,
    ) -> Result<crate::ReportExportPairJobs<E>, (AdmissionError, M)>
    where
        E: Copy + Send + 'static,
        M: FnOnce(JobContext, JobContext) -> Result<bool, E> + Send + 'static,
    {
        let crate::report_export::pair::MainRequest {
            scope: source_scope,
            spec: source_spec,
            cleanup: source_cleanup,
        } = source;
        let crate::report_export::pair::MainRequest {
            scope: receiving_scope,
            spec: receiving_spec,
            cleanup: receiving_cleanup,
        } = receiving;
        let required = crate::native_resources::symbol_storage_bytes()
            + 2 * size_of::<Record>()
            + 2 * size_of::<Job<bool, E>>()
            + size_of::<PairWork>()
            + 2 * size_of::<JobContext>()
            + size_of::<M>()
            + 256;
        if required > crate::native_resources::SYMBOL_METADATA_BYTES as usize {
            return Err((AdmissionError::InvalidReportExportAdmission, factory));
        }
        let now = match self.clock.metadata() {
            Ok(now) => now,
            Err(cause) => return Err((AdmissionError::Unexpected(cause), factory)),
        };
        let source_guard;
        let receiving_guard;
        let mut state = lock(&self.state);
        let source_kind = RequestKind::SymbolHelper { cleanup: source_cleanup };
        let receiving_kind = RequestKind::ReportExportMain { cleanup: receiving_cleanup };
        let validated = (|| {
            if state.stopping || self.clock.failed() {
                return Err(AdmissionError::ShuttingDown);
            }
            let source = self.owned_scope(&state, source_scope, source_scope.owner_id())?;
            let receiving =
                self.owned_scope(&state, receiving_scope, receiving_scope.owner_id())?;
            for (stored, scope) in [(source, source_scope), (receiving, receiving_scope)] {
                if stored.closed || stored.cancel.is_requested() {
                    return Err(AdmissionError::ScopeClosed { scope_id: scope.id() });
                }
                if stored.native || stored.deadline.is_some() {
                    return Err(AdmissionError::InvalidReportExportAdmission);
                }
            }
            if !crate::symbol_admission::valid_main(source_spec)
                || !crate::report_export::admission::valid_main(receiving_spec)
                || source_spec.deadline != receiving_spec.deadline
                || source_spec.id >= receiving_spec.id
                || descendant(&state, source_scope.id(), receiving_scope.id())
                || descendant(&state, receiving_scope.id(), source_scope.id())
            {
                return Err(AdmissionError::InvalidReportExportAdmission);
            }
            if let Some(deadline) = source_spec.deadline.filter(|deadline| *deadline <= now) {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
            for spec in [source_spec, receiving_spec] {
                if state.jobs.contains_key(&spec.id) {
                    return Err(AdmissionError::DuplicateJobId { job_id: spec.id });
                }
                if let Some(latest) = state.latest_job.filter(|latest| spec.id <= *latest) {
                    return Err(AdmissionError::JobIdentityOrder { latest });
                }
            }
            crate::symbol_admission::validate(
                self,
                &state,
                source_scope,
                source_spec,
                &source_kind,
                now,
            )?;
            crate::report_export::admission::validate(
                self,
                &state,
                receiving_scope,
                receiving_spec,
                &receiving_kind,
                now,
            )?;
            let parents = (source.cancel.clone(), receiving.cancel.clone());
            let mut combined = source_spec;
            combined.reservation_bytes += receiving_spec.reservation_bytes;
            Self::check_capacity_slots(&mut state, combined, false, 2)?;
            Ok(parents)
        })();
        let (source_parent, receiving_parent) = match validated {
            Ok(parents) => parents,
            Err(error) => return Err((error, factory)),
        };
        state.bytes += source_spec.reservation_bytes + receiving_spec.reservation_bytes;
        for (scope, spec) in [(source_scope, source_spec), (receiving_scope, receiving_spec)] {
            if let Some(stored) = state.scopes.get_mut(&scope.id()) {
                stored.bytes += spec.reservation_bytes;
            }
        }
        let source_memory = MemoryLease::new(
            source_spec.reservation_bytes,
            Box::new(Release {
                core: self.clone(),
                scope_id: source_scope.id(),
                job_id: source_spec.id,
            }),
        );
        let receiving_memory = MemoryLease::new(
            receiving_spec.reservation_bytes,
            Box::new(Release {
                core: self.clone(),
                scope_id: receiving_scope.id(),
                job_id: receiving_spec.id,
            }),
        );
        let release =
            match crate::native_resources::release(self.clone(), source_spec.id, &source_memory) {
                Ok(release) => release,
                Err(cause) => {
                    drop(state);
                    drop(source_memory);
                    drop(receiving_memory);
                    return Err((
                        AdmissionError::Unexpected(crate::runtime::error(cause)),
                        factory,
                    ));
                }
            };
        let source_cancel = Cancellation::new(Some(source_parent), Arc::downgrade(self));
        let receiving_cancel = Cancellation::new(Some(receiving_parent), Arc::downgrade(self));
        source_guard = JobContext::new(
            self.id,
            source_scope.id(),
            source_spec,
            source_spec.deadline,
            source_cancel.clone(),
            self.clock.clone(),
            AdmittedJobResources::new(source_memory, Some(release)),
        );
        receiving_guard = JobContext::new(
            self.id,
            receiving_scope.id(),
            receiving_spec,
            receiving_spec.deadline,
            receiving_cancel.clone(),
            self.clock.clone(),
            AdmittedJobResources::new(receiving_memory, None),
        );
        let source_job =
            Job::new(source_scope.id(), source_spec, now, source_cancel, Arc::downgrade(self));
        let receiving_job = Job::new(
            receiving_scope.id(),
            receiving_spec,
            now,
            receiving_cancel,
            Arc::downgrade(self),
        );
        let mut source_record =
            Record::admitted(&source_guard, source_job.clone(), &source_kind, None);
        let mut receiving_record =
            Record::admitted(&receiving_guard, receiving_job.clone(), &receiving_kind, None);
        source_record.report_pair =
            Some(Association { root: source_spec.id, partner: receiving_spec.id, worker: None });
        receiving_record.report_pair =
            Some(Association { root: source_spec.id, partner: source_spec.id, worker: None });
        if let Some(lease) = source_guard.native_handles() {
            state.symbol_resources.insert(
                source_spec.id,
                crate::native_resources::ResourceRecord {
                    bundle_issued: false,
                    reader_issued: false,
                    provider_issued: false,
                    scope_id: source_scope.id(),
                    observation: lease.observation(),
                },
            );
        }
        if let Some(binding) =
            state.jobs.get_mut(&source_cleanup).and_then(|record| record.symbol_cleanup.as_mut())
        {
            binding.paired = true;
            binding.report_export_partner = Some(receiving_spec.id);
        }
        if let Some(binding) =
            state.jobs.get_mut(&receiving_cleanup).and_then(|record| record.report_cleanup.as_mut())
        {
            binding.paired = true;
        }
        state.jobs.insert(source_spec.id, source_record);
        state.jobs.insert(receiving_spec.id, receiving_record);
        state.latest_job = Some(receiving_spec.id);
        let task_source = source_job.clone();
        let task_receiving = receiving_job.clone();
        let task_context = receiving_guard.clone();
        let core = self.clone();
        state.queue.push_back(QueuedJob {
            context: source_guard.clone(),
            job: source_job.clone(),
            work: Work::ReportExportPair(Box::new(PairWork {
                context: receiving_guard.clone(),
                job: receiving_job.clone(),
                factory: Box::new(move |source, dispatched| {
                    Box::pin(crate::report_export::pair_run::run(
                        core,
                        task_source,
                        source,
                        task_receiving,
                        task_context,
                        dispatched,
                        factory,
                    ))
                }),
            })),
        });
        drop(state);
        drop(source_guard);
        drop(receiving_guard);
        self.changed();
        Ok((
            OwnedJob::new(source_scope.id(), source_spec, Box::new(JobHandle(source_job))),
            OwnedJob::new(receiving_scope.id(), receiving_spec, Box::new(JobHandle(receiving_job))),
        ))
    }
}
