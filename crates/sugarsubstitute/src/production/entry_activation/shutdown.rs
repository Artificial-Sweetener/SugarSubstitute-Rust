// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire original child/profile/pipe ownership before coordinator claim closure.
use super::*;
use sugarsubstitute_contracts::lifecycle::ShutdownId;
use sugarsubstitute_supervision::{RegistryAdmissionError, profiles::PROFILE_OPERATION_BYTES};
impl ProductionActivation {
    pub(super) fn close_records(&mut self, resources: &mut EntryResources<'_>) -> Result<()> {
        let budget = self.shutdown.ok_or("Original activation shutdown budget missing")?;
        // An unlaunched source has no active captured consumer. Return its passive
        // transport loan before waiting for the very transport job it keeps alive.
        for record in self.records.iter_mut().flatten().map(|held| held.record.as_mut()) {
            if record.start.is_none()
                && let Some(completed) = &mut record.projected
            {
                projection::return_unused_source(&mut completed.outcome);
            }
        }
        let Some(retained_backend) = &mut self.backend else {
            for record in self.records.iter_mut().flatten().map(|retained| retained.record.as_mut())
            {
                record.closed = true;
            }
            return Ok(());
        };
        let backend = retained_backend.backend.as_mut();
        for record in self.records.iter_mut().flatten().map(|retained| retained.record.as_mut()) {
            Self::close_record(record, backend, self.capture.as_ref(), resources, budget)?;
        }
        backend
            .close_custody(
                budget,
                &mut progress::backend_resources(resources, self.capture.as_ref()),
            )
            .map_err(outer)?;
        Ok(())
    }
    /// Advance the same actual native owners for explicit coordinator close or one failed desktop.
    /// The caller retains original collection/source custody until checked retirement permits this work.
    pub(super) fn close_record(
        record: &mut Record,
        backend: &mut CoordinatorBackend,
        capture: Option<&CaptureRegistration>,
        resources: &mut EntryResources<'_>,
        budget: ShutdownBudget,
    ) -> Result<()> {
        let budget = record.parent.as_ref().map_or(budget, |parent| parent.cleanup_budget(budget));
        if record.closed {
            return Ok(());
        }
        if record.collection.pending() {
            return Ok(());
        }
        if record.attach.is_some() {
            return Ok(());
        }
        if let Some(released) = &mut record.released_attach {
            if record.attach_retirement.is_none()
                && let JobOutcome::Success(observer) = &mut released.outcome
            {
                record.attach_retirement = observer.try_take_report();
            }
            match &record.attach_retirement {
                Some(Ok(actual))
                    if actual.target()
                        == sugarsubstitute_contracts::crash::CaptureTarget::Desktop(record.run)
                        && actual.execution_id() == resources.native.owner_id()
                        && actual.job_id() == released.spec.id => {}
                Some(Err(cause)) => return Err(Box::new(cause.clone())),
                Some(Ok(_)) => {
                    return Err("Actual released capture retirement identity mismatch".into());
                }
                None => return Ok(()),
            }
        }
        if record.retired.is_some() {
            progress::with_parent(record, backend, capture, resources, |parent, borrowed| {
                Ok(parent.request_cleanup(parent.cleanup_budget(budget), borrowed)?)
            })?;
            let retired = record.retired.as_ref().ok_or("Actual retirement receipt missing")?;
            backend.settle_run(retired).map_err(outer)?;
            record.closed = true;
            return Ok(());
        }
        if record.start.is_none() {
            record.closed = true;
            return Ok(());
        }
        if record.parent.is_some() {
            progress::with_parent(record, backend, capture, resources, |parent, borrowed| {
                Ok(parent.request_cleanup(parent.cleanup_budget(budget), borrowed)?)
            })?;
        }
        let start = record.start.as_mut().ok_or("Original desktop start missing")?;
        if let Some(output) = &mut start.output {
            output.request_close();
            output.observe(
                #[cfg(feature = "k1-native-qualification")]
                record.parent.as_deref().and_then(ParentLifecycle::qualification_identity),
            )?;
        }
        if record.parent.is_none() && !start.close_unlaunched()? {
            return Ok(());
        }
        if let Some(ticket) = start.ticket {
            let registry =
                resources.authority.registry_mut().ok_or("Actual process registry missing")?;
            let actual = registry.snapshot(ticket.process())?;
            if !actual.settled {
                registry.begin_stop(
                    ticket.process(),
                    ShutdownId::new(record.run.instance.get())?,
                    budget,
                )?;
                if registry.snapshot(ticket.process())?.cleanup_step_due(resources.client.now()?) {
                    let member = if actual.member_watch_required {
                        Some(JobSpec {
                            id: resources.jobs.allocate()?,
                            operation: Operation::NativeLifecycle,
                            class: JobClass::NativeObservation,
                            deadline: None,
                            reservation_bytes: 4096,
                            replacement: None,
                        })
                    } else {
                        None
                    };
                    let spec = JobSpec {
                        id: resources.jobs.allocate()?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(budget.settle_until),
                        reservation_bytes: 4096,
                        replacement: None,
                    };
                    match registry.submit_cleanup_step(
                        resources.client,
                        ticket.process(),
                        resources.cleanup,
                        spec,
                        member,
                    ) {
                        Ok(_) => {}
                        Err(RegistryAdmissionError::Busy) => {}
                        Err(cause) => return Err(Box::new(cause)),
                    }
                }
                return Ok(());
            }
            while registry.take_observation(ticket.process())?.is_some() {}
        }
        if !start.profile.snapshot().retirement_ready() {
            if start.ticket.is_none() {
                let registry = resources
                    .authority
                    .registry_mut()
                    .ok_or("Actual prepared profile registry missing")?;
                if !start.profile.retire_prepared_family(registry)? {
                    return Ok(());
                }
            }
            if !start.profile.snapshot().pending && !start.profile.custody_release_pending() {
                let registry =
                    resources.authority.registry().ok_or("Actual process registry missing")?;
                let spec = JobSpec {
                    id: resources.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(budget.settle_until),
                    reservation_bytes: PROFILE_OPERATION_BYTES,
                    replacement: None,
                };
                start.profile.submit_release(
                    resources.client,
                    resources.cleanup,
                    spec,
                    registry,
                )?;
            }
            return Ok(());
        }
        if let Some(parent) = &record.parent {
            let registry =
                resources.authority.registry_mut().ok_or("Actual process registry missing")?;
            if let Some(retired) = retirement::retire(start, parent, registry)? {
                record.retired = Some(retired);
                backend
                    .settle_run(record.retired.as_ref().ok_or("Actual retirement receipt missing")?)
                    .map_err(outer)?;
                record.closed = true;
            }
        } else {
            record.closed = true;
        }
        Ok(())
    }
}
