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
        // Failed adapters cannot prevent deadline consumption or keep a past native
        // wake spinning. Actual late completions are still observed on every batch.
        let deadline = self
            .records
            .iter()
            .flatten()
            .filter_map(|held| {
                held.record.parent.as_ref().and_then(|parent| parent.next_deadline())
            })
            .chain(self.backend.as_ref().and_then(|held| held.backend.next_deadline()))
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
            .chain(self.incidents.reporting.next_deadline())
            .chain(self.records.iter().flatten().filter_map(|held| held.record.restart.deadline()))
            .min();
        self.next_wake = None;
        if let Some(until) = deadline {
            let anchor = Instant::now();
            match resources.client.now() {
                Ok(now) if until > now => {
                    self.next_wake = anchor.checked_add(until.checked_duration_since(now)?)
                }
                Ok(_) => {}
                Err(cause) if result.is_ok() => return Err(Box::new(cause)),
                Err(cause) => self.preserve(Box::new(cause)),
            }
        }
        self.complete_observation(first_capture_failure || first_delivery_failure, result)
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
        self.adopt_installed_restarts(resources)?;
        let Some(retained_backend) = &mut self.backend else {
            if let Some(record) =
                self.records.iter().flatten().map(|held| held.record.as_ref()).find(|record| {
                    record.projected.as_ref().is_some_and(|completed| {
                        !matches!(completed.outcome, JobOutcome::Success(_))
                    })
                })
            {
                self.failure_run.get_or_insert(record.run);
                return Err("Actual entry projection failed with retained original outcome".into());
            }
            return Ok(());
        };
        let backend = retained_backend.backend.as_mut();
        let delivery = self.delivery.as_mut().ok_or("Actual backend delivery storage missing")?;
        let mut failure =
            relay::route(&mut self.records, backend, self.capture.as_ref(), resources).err();
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
            }
        }
        let capture = self.capture.as_mut().ok_or("Actual capture owner missing")?;
        for record in self.records.iter_mut().flatten().map(|held| held.record.as_mut()) {
            if let Err(cause) = Self::adopt_failed_family(record, &mut self.incidents) {
                failure.get_or_insert(cause);
            }
            if record.closed || record.retired.is_some() {
                continue;
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
            if !self.closing
                && failure.is_none()
                && !delivery.pending()
                && let Some(budget) = Self::normal_role_budget(record)
                && let Err(cause) =
                    Self::close_record(record, backend, Some(capture), resources, budget)
            {
                failure.get_or_insert(cause);
            }
        }
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
            self.closing || failure.is_some(),
        ) {
            failure.get_or_insert(cause);
        }
        if let Some(cause) = failure {
            return Err(cause);
        }
        self.advance_restart(resources)?;
        Ok(())
    }
    fn receive_projections(&mut self, resources: &EntryResources<'_>) -> Result<()> {
        for pending in self.pending.iter_mut().flatten() {
            if let Some(job) = &mut pending.job
                && let Some(completion) = job.try_take_completion()
            {
                pending.completion = Some(completion);
                pending.job = None;
            }
        }
        if self.closing {
            for pending in self.pending.iter_mut().flatten() {
                if let Some(completed) = &mut pending.completion {
                    projection::return_unused_source(&mut completed.outcome);
                }
            }
        }
        for at in 0..self.pending.len() {
            let Some(pending) = &mut self.pending[at] else {
                continue;
            };
            let Some(completion) = &pending.completion else {
                continue;
            };
            if self.backend.is_none() && matches!(completion.outcome, JobOutcome::Success(_)) {
                let storage = completion.memory.claim(RetainedBackend::required_storage_bytes())?;
                self.backend =
                    Some(RetainedBackend::new(resources.authority, storage).map_err(outer)?);
                self.backend
                    .as_mut()
                    .ok_or("Backend owner absent")?
                    .backend
                    .configure_custody(self.executable.clone(), self.working_directory.clone())
                    .map_err(outer)?;
            }
            if self.delivery.is_none() && matches!(completion.outcome, JobOutcome::Success(_)) {
                let storage =
                    completion.memory.claim(delivery::TransitionDelivery::storage_bytes())?;
                self.delivery = Some(delivery::TransitionDelivery::new(storage)?);
            }
            let storage = completion.memory.claim(size_of::<Record>() as u64)?;
            let projected =
                pending.completion.take().ok_or("Actual projection completion missing")?;
            let record = Record {
                qualification: std::mem::take(&mut pending.qualification),
                run: pending.run,
                launch: pending.launch,
                navigation: pending.navigation,
                connection: pending.connection,
                projected: Some(projected),
                attach: None,
                attached_memory: None,
                attach_failure: None,
                released_attach: None,
                attach_retirement: None,
                start: None,
                parent: None,
                preparation: None,
                retired: None,
                forwarded: None,
                ui_suspected: false,
                replied: pending.restart,
                closed: false,
                seed: incident_seed::SeedPreparation::new(),
                seeded: false,
                collection: incident_collection::IncidentCollection::new(),
                failed: None,
                failed_cause: None,
                failed_shutdown: None,
                failed_family_adopted: false,
                pending_desktop_failure: None,
                restart: restart::RestartState::for_projection(pending.restart),
            };
            self.records[at] = Some(RetainedRecord { record: Box::new(record), _storage: storage });
            self.pending[at] = None;
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
        target: &value.target,
        finite,
        backend,
        preparation: &mut record.preparation,
        execution: &mut execution,
    };
    call(parent, &mut borrowed)
}
