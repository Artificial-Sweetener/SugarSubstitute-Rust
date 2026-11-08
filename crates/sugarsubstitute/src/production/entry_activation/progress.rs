// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe each real owner once per batch before routing authenticated parent work.
use super::*;
impl ProductionActivation {
    pub(super) fn activity(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        let mut delivery_failure = self.incidents.capture(batch).err();
        for record in self.records.iter_mut().flatten() {
            for result in
                [record.record.seed.capture(batch), record.record.collection.capture(batch)]
            {
                if let Err(cause) = result {
                    delivery_failure.get_or_insert(cause);
                }
            }
        }
        let first_delivery_failure = delivery_failure.is_some();
        if let Some(cause) = delivery_failure {
            self.preserve(cause);
            self.closing = true;
        }
        let capture_failure =
            self.capture.as_mut().and_then(|capture| capture.observe(batch).err());
        let first_capture_failure = capture_failure.is_some();
        if let Some(cause) = capture_failure {
            self.preserve(cause);
            self.closing = true;
        }
        let result = self.advance(batch, resources);
        // The selected recovery owns its independent finite work and cleanup even
        // when another owner failed this turn; no pending outcome is inferred.
        let recovery = self.advance_fresh_restart(batch, resources);
        let result = match (result, recovery) {
            (Ok(()), next) => next,
            (Err(cause), _) => Err(cause),
        };
        // Failed adapters cannot prevent deadline consumption or keep a past native
        // wake spinning. Actual late completions are still observed on every batch.
        if let Err(cause) = self.arm_next_wake(resources.client, Instant::now()) {
            if result.is_ok() {
                return Err(cause);
            }
            self.preserve(cause);
        }
        self.complete_observation(first_capture_failure || first_delivery_failure, result)
    }
    /// Re-arm future original boundaries after this turn attempted owner progress.
    /// A retained past boundary stays with its owner; it cannot suppress another owner's
    /// future force/settle wake or manufacture admission, progress or physical retirement.
    /// Due owners still owe their finite progress turn before any further admission.
    pub(super) fn arm_next_wake(
        &mut self,
        client: &sugarsubstitute_execution::ExecutionClient,
        anchor: Instant,
    ) -> Result<()> {
        self.next_wake = None;
        let deadline = {
            let mut deadlines = self.wake_deadlines().peekable();
            if deadlines.peek().is_some() {
                let now = client.now()?;
                deadlines
                    .filter(|until| *until > now)
                    .min()
                    .map(|until| {
                        Ok::<_, Box<dyn Error>>(
                            anchor.checked_add(until.checked_duration_since(now)?),
                        )
                    })
                    .transpose()?
                    .flatten()
            } else {
                None
            }
        };
        self.next_wake =
            if self.incidents.reporting.take_continuation() { Some(anchor) } else { deadline };
        Ok(())
    }
    /// Enumerate bounded original owner candidates before selecting one native wake.
    fn wake_deadlines(
        &self,
    ) -> impl Iterator<Item = sugarsubstitute_contracts::execution::Moment> + '_ {
        self.records
            .iter()
            .flatten()
            .flat_map(|held| held.record.parent.iter().flat_map(|parent| parent.deadlines()))
            .chain(self.backend.iter().flat_map(|held| held.backend.deadlines()))
            .chain(
                self.records
                    .iter()
                    .flatten()
                    .filter_map(|held| held.record.collection.next_deadline()),
            )
            .chain(self.records.iter().flatten().filter_map(|held| {
                held.record
                    .pending_desktop_failure
                    .as_ref()
                    .map(incident_reporting::PendingDesktopFailure::until)
            }))
            .chain(self.records.iter().flatten().flat_map(|held| {
                held.record
                    .failed_shutdown
                    .into_iter()
                    .flat_map(|budget| [budget.graceful_until, budget.settle_until])
            }))
            .chain(self.recovery_pending.iter().flat_map(|pending| pending.deadlines()))
            .chain(self.incidents.reporting.deadlines())
            .chain(self.incidents.recovery_deadline())
            .chain(self.records.iter().flatten().filter_map(|held| held.record.restart.deadline()))
            .chain(
                self.records
                    .iter()
                    .flatten()
                    .filter_map(|held| held.record.profile_wait_deadline()),
            )
    }
    /// Report this turn's first capture failure only after owned completion and
    /// deadline adoption; retained historical errors do not fail later turns again.
    pub(super) fn complete_observation(
        &self,
        first_capture_failure: bool,
        result: Result<()>,
    ) -> Result<()> {
        match result {
            Ok(()) if first_capture_failure => {
                Err("Actual capture failure retained after completion adoption".into())
            }
            other => other,
        }
    }
    fn advance(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        self.receive_projections(resources)?;
        if !self.closing {
            // The current Root's genuine retained source authorizes historical opening;
            // historical path/build/origin facts cannot stand in for this registration.
            if let Some(value) = self.records.iter().flatten().find_map(|held| {
                match &held.record.projected.as_ref()?.outcome {
                    JobOutcome::Success(value) => Some(value),
                    _ => None,
                }
            }) {
                let capture = self.capture.as_mut().ok_or("Actual Root capture owner missing")?;
                capture.start(&value.capture_root, value.until, resources)?;
                if capture.root_source().is_none() {
                    return Ok(());
                }
            }
            let source = self.capture.as_ref().and_then(CaptureRegistration::root_source);
            if !self.incidents.recover_pending(&self.records, source, resources)? {
                return Ok(());
            }
        }
        self.adopt_installed_restarts(resources)?;
        let Some(retained_backend) = &mut self.backend else {
            return Ok(());
        };
        let backend = retained_backend.backend.as_mut();
        let delivery = self.delivery.as_mut().ok_or("Actual backend delivery storage missing")?;
        let mut failure =
            relay::route(&mut self.records, backend, self.capture.as_ref(), resources).err();
        if let Err(cause) = backend.maintain_profile_custody_without_resources(
            &mut backend_resources(resources, self.capture.as_ref()),
        ) {
            failure.get_or_insert_with(|| outer(cause));
        }
        delivery.capture_pending(|| backend.take_pending_transition());
        if let Err(cause) =
            delivery.route(&mut self.records, backend, self.capture.as_ref(), resources)
        {
            failure.get_or_insert(cause);
        }
        if failure.is_none() && !delivery.pending() {
            for _ in 0..32 {
                let Some(observation) = resources.authority.take_observation() else {
                    break;
                };
                let mut unrelated_observation = false;
                let produced = delivery.produce(|| {
                    let (transition, unrelated) = backend
                        .claim_observation(
                            observation,
                            &mut backend_resources(resources, self.capture.as_ref()),
                        )
                        .map_err(outer)?;
                    unrelated_observation = unrelated.is_some();
                    Ok(transition)
                });
                delivery.capture_pending(|| backend.take_pending_transition());
                if unrelated_observation {
                    failure.get_or_insert("Unrouted actual coordinator claim observation".into());
                }
                if let Err(cause) = produced {
                    failure.get_or_insert(cause);
                }
                if let Err(cause) =
                    delivery.route(&mut self.records, backend, self.capture.as_ref(), resources)
                {
                    failure.get_or_insert(cause);
                }
                if failure.is_some() || delivery.pending() {
                    break;
                }
            }
            if failure.is_none() && !delivery.pending() {
                for key in backend.resource_keys().into_iter().flatten() {
                    let produced = delivery.produce(|| {
                        backend
                            .activity(
                                key,
                                batch,
                                &mut backend_resources(resources, self.capture.as_ref()),
                            )
                            .map_err(outer)
                    });
                    delivery.capture_pending(|| backend.take_pending_transition());
                    if let Err(cause) = produced {
                        if self.failure.is_none() {
                            self.failure_resource.get_or_insert(key);
                        }
                        failure.get_or_insert(Box::new(backend_failure::OperationFailure::new(
                            key, cause,
                        )));
                    }
                    if let Err(cause) =
                        delivery.route(&mut self.records, backend, self.capture.as_ref(), resources)
                    {
                        failure.get_or_insert(cause);
                    }
                    match backend_failure::classify(backend, key) {
                        Ok(backend_failure::Disposition::FatalCleanup) => {
                            if self.failure.is_none() {
                                self.failure_resource.get_or_insert(key);
                            }
                            failure.get_or_insert(Box::new(backend_failure::CleanupFailure {
                                resource: key,
                            }));
                        }
                        Ok(_) => {}
                        Err(cause) => {
                            failure.get_or_insert(cause);
                        }
                    }
                    if failure.is_some() || delivery.pending() {
                        break;
                    }
                }
            }
        }
        let capture = self.capture.as_mut().ok_or("Actual capture owner missing")?;
        for at in 0..self.records.len() {
            let profile_waiting = Self::profile_admission_waiting(&mut self.records, at, backend)?;
            let backend_commands_open = !self.closing
                && failure.is_none()
                && commands_open(&self.records, delivery, backend);
            let Some(record) = self.records[at].as_mut().map(|held| held.record.as_mut()) else {
                continue;
            };
            if let Err(cause) = Self::adopt_failed_family(record, &mut self.incidents) {
                failure.get_or_insert(cause);
            }
            let retired = record.retired.as_ref();
            match progress_retired_record(&mut record.closed, retired.is_some(), || {
                let retired = retired.ok_or("Original desktop retirement receipt missing")?;
                backend.settle_run(retired).map_err(outer)
            }) {
                Ok(true) => {}
                Ok(false) => {
                    if let Err(cause) = super::normal_retirement::retire(record) {
                        failure.get_or_insert(cause);
                    }
                    continue;
                }
                Err(cause) => {
                    failure.get_or_insert(cause);
                    continue;
                }
            }
            if let Err(cause) = Self::progress_record(
                record,
                &mut self.incidents,
                capture,
                backend,
                &self.executable,
                &self.working_directory,
                batch,
                resources,
                self.closing || failure.is_some() || delivery.pending(),
                backend_commands_open,
                profile_waiting,
            ) {
                if self.failure.is_none() {
                    self.failure_run.get_or_insert(record.run);
                    if let Some(key) =
                        record.parent.as_ref().and_then(|parent| parent.backend_failure_resource())
                    {
                        self.failure_resource.get_or_insert(key);
                    }
                }
                failure.get_or_insert(cause);
            }
            if !self.closing && failure.is_none() && !delivery.pending() {
                let budget = match Self::normal_role_budget(record) {
                    Some(budget) => Some(budget),
                    None if record
                        .parent
                        .as_ref()
                        .and_then(|parent| parent.close_progress_request())
                        .is_some() =>
                    {
                        Self::overdue_role_budget(record, resources.client.now()?)
                    }
                    None => None,
                };
                if let Some(budget) = budget
                    && let Err(cause) =
                        Self::close_record(record, backend, Some(capture), resources, budget)
                {
                    failure.get_or_insert(cause);
                }
            }
        }
        let report_restart = capture.restart_availability();
        let fresh_supervisor_required = capture.fresh_supervisor_required();
        if let Err(cause) =
            relay::route(&mut self.records, backend, self.capture.as_ref(), resources)
        {
            failure.get_or_insert(cause);
        }
        delivery.capture_pending(|| backend.take_pending_transition());
        if let Err(cause) =
            delivery.route(&mut self.records, backend, self.capture.as_ref(), resources)
        {
            failure.get_or_insert(cause);
        }
        if let Err(cause) = self.incidents.reporting.progress(
            &mut self.incidents.host,
            &mut self.records,
            &self.executable,
            &self.working_directory,
            resources,
            incident_reporting::ReportAdmission::new(
                self.closing || failure.is_some(),
                report_restart,
                fresh_supervisor_required,
            ),
            &mut self.incidents.transactions,
        ) {
            failure.get_or_insert(cause);
        }
        if let Some(cause) = failure {
            return Err(cause);
        }
        if self.incidents.reporting.deferred_fresh_restart().is_none()
            && self.recovery_pending.is_none()
        {
            self.advance_restart(resources)?;
        }
        Ok(())
    }
}

pub(super) fn backend_resources<'a>(
    resources: &'a mut EntryResources<'_>,
    capture: Option<&'a CaptureRegistration>,
) -> BackendResources<'a> {
    BackendResources {
        client: resources.client,
        native: resources.native,
        cleanup: resources.cleanup,
        jobs: resources.jobs,
        scopes: resources.scopes,
        authority: resources.authority,
        capture,
    }
}
pub(super) fn with_parent<T>(
    record: &mut Record,
    backend: &mut CoordinatorBackend,
    capture: Option<&CaptureRegistration>,
    resources: &mut EntryResources<'_>,
    backend_commands_open: bool,
    call: impl FnOnce(&mut ParentLifecycle, &mut ParentResources<'_, '_>) -> Result<T>,
) -> Result<T> {
    let start = record.start.as_mut().ok_or("Actual desktop start missing")?;
    let parent = record.parent.as_mut().ok_or("Actual parent lifecycle missing")?;
    let process = start.ticket.ok_or("Actual desktop process ticket missing")?.process();
    let value = match &start.projection.outcome {
        JobOutcome::Success(value) => value,
        _ => return Err("Original projection missing".into()),
    };
    let session = start.pipe.as_mut().ok_or("Actual captured lifecycle missing")?;
    let finite = resources.finite;
    let mut execution = backend_resources(resources, capture);
    let mut borrowed = ParentResources {
        session,
        process,
        profile: &mut start.profile,
        requested_profile: &value.profile_units,
        installation: &value.session_installation,
        backend_data_disposition:
            sugarsubstitute_contracts::configuration::BackendDataDisposition::External,
        target: &value.target,
        finite,
        backend_commands_open,
        backend,
        preparation: &mut record.preparation,
        execution: &mut execution,
    };
    call(parent, &mut borrowed)
}

/// Recompute across EVERY source before each Parent turn; a newly returned foreign result closes admission for later roles.
/// Cleanup, current-binding pulls and real completion adoption do not use this command gate.
fn commands_open(
    records: &[Option<RetainedRecord>; 64],
    delivery: &delivery::TransitionDelivery,
    backend: &CoordinatorBackend,
) -> bool {
    !delivery.pending()
        && !backend.delivery_pending()
        && !records.iter().flatten().any(|held| {
            held.record.forwarded.is_some()
                || held.record.parent.as_ref().is_some_and(|parent| {
                    parent.pending_backend_delivery() || parent.pending_backend_handoff()
                })
        })
}

/// Continue native record progress only while its retained owner remains live.
/// The caller supplies the same backend settlement boundary and keeps the sealed
/// native receipt. A pending policy settlement retains the record for later turns;
/// a failed boundary returns its exact cause without marking the record closed.
fn progress_retired_record(
    closed: &mut bool,
    retired: bool,
    settle: impl FnOnce() -> Result<bool>,
) -> Result<bool> {
    if *closed {
        return Ok(false);
    }
    if retired {
        *closed = settle()?;
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
#[path = "progress_retired_tests.rs"]
mod retired_tests;
