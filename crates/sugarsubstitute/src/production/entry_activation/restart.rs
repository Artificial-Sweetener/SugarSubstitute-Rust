// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the policy owner's once-forwarded intent through the surviving coordinator.
use super::*;
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{AdmissionError, Moment},
};
/// The first validated interval stays with the retired slot through saturation.
pub(super) struct RestartState {
    original: Option<EntrySetupBudget>,
    admitted: bool,
    fresh_shutdown: Option<ShutdownBudget>,
    pub(super) installation_pending: bool,
}
impl RestartState {
    pub(super) const fn new() -> Self {
        Self { original: None, admitted: false, fresh_shutdown: None, installation_pending: false }
    }
    pub(super) fn for_projection(restart: bool) -> Self {
        Self { installation_pending: restart, ..Self::new() }
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        if self.admitted {
            None
        } else {
            self.original
                .map(EntrySetupBudget::until)
                .or_else(|| self.fresh_shutdown.map(|budget| budget.settle_until))
        }
    }
    /// The existing original cleanup interval closes admission for this current source.
    pub(super) fn closing_current(&self) -> bool {
        self.fresh_shutdown.is_some() && !self.admitted
    }
}
impl ProductionActivation {
    /// Reuse only a genuinely retired existing slot; the same Root custody stays alive.
    pub(super) fn advance_restart(&mut self, resources: &mut EntryResources<'_>) -> Result<()> {
        if self.closing || !resources.authority.serving() {
            return Ok(());
        }
        let Some(target) = self.incidents.reporting.restart_run(&mut self.incidents.host)? else {
            return Ok(());
        };
        let previous = target.run();
        let Some(at) = self
            .records
            .iter()
            .position(|held| held.as_ref().is_some_and(|held| held.record.run == previous))
        else {
            return Err("Actual restart run missing".into());
        };
        if self.pending[at].is_some() {
            return Ok(());
        }
        let held = self.records[at].as_mut().ok_or("Actual restart record missing")?;
        let record = held.record.as_mut();
        if record.completed_normal.is_some() {
            return Err(sugarsubstitute_contracts::lifecycle::LifecycleRejection::WrongPhase.into());
        }
        if record.restart.admitted {
            return Err("Actual restart intent repeated".into());
        }
        if resources.authority.key() != Some(previous.owner)
            || resources.authority.registry().is_none()
        {
            return Err("Actual restart coordinator changed".into());
        }
        let current_recovery =
            matches!(target, incident_reporting::RestartTarget::CurrentRecovery(_));
        // A seed already admitted for this current startup must complete its genuine
        // launch/exit path. It cannot be reclassified as a never-issued record.
        if current_recovery
            && record.seed.started()
            && record.start.as_ref().is_none_or(|start| start.ticket.is_none())
            && record.retired.is_none()
        {
            return Ok(());
        }
        if current_recovery && !record.closed {
            let budget = match record.restart.fresh_shutdown {
                Some(budget) => budget,
                None => {
                    let budget = ShutdownBudget::new(resources.client.now()?)?;
                    record.restart.fresh_shutdown = Some(budget);
                    budget
                }
            };
            let backend = self.backend.as_mut().ok_or("Actual current restart backend missing")?;
            Self::close_record(
                record,
                backend.backend.as_mut(),
                self.capture.as_ref(),
                resources,
                budget,
            )?;
        }
        let never_issued = current_recovery
            && record.closed
            && record.parent.is_none()
            && !record.seed.started()
            && !record.seeded
            && record.failed.is_none()
            && record.start.as_ref().is_none_or(|start| start.never_launched());
        if never_issued && let Some(completed) = &mut record.projected {
            projection::return_unused_source(&mut completed.outcome);
        }
        if !record.closed
            || (!never_issued
                && record.retired.as_ref().is_none_or(|actual| actual.run() != previous))
            || !record.seed.delivered()
            || !record.collection.delivered()
            || record.attach.is_some()
        {
            return Ok(());
        }
        let now = resources.client.now()?;
        let original = match record.restart.original {
            Some(original) => original,
            None => {
                let original = EntrySetupBudget::new(now)?;
                record.restart.original = Some(original);
                original
            }
        };
        if now < original.started() || now >= original.until() {
            return Err("Original coordinator restart deadline expired".into());
        }
        if resources.finite.owner_id() != resources.native.owner_id() {
            return Err("Actual restart execution owners changed".into());
        }
        let completed = if let Some(start) = &record.start {
            &start.projection
        } else {
            record.projected.as_ref().ok_or("Actual registered current projection missing")?
        };
        let JobOutcome::Success(value) = &completed.outcome else {
            return Err("Actual registered startup missing".into());
        };
        let run = RunKey { generation: previous.generation.next()?, ..previous };
        let next_connection =
            self.next_connection.checked_add(1).ok_or("Connection identity exhausted")?;
        let connection = ConnectionId::new(next_connection)?;
        let mut input = Some(ProjectionInput::restart(
            resources.client.clone(),
            self.capture_root.clone(),
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                &self.working_directory,
            )
            .user(),
            projection::RestartProjection {
                source_custody: value.source.clone(),
                source: value.registered.clone(),
                previous,
                run,
                original,
                connection: value.entry_connection,
                request: value.entry_request,
            },
            resources.authority.running_installation(),
        ));
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(original.until()),
            reservation_bytes: projection::PROJECTION_BYTES,
            replacement: None,
        };
        let job = match resources.client.submit_blocking_owned_box(
            resources.finite,
            spec,
            &mut input,
            projection::project,
        ) {
            Ok(job) => job,
            Err(sugarsubstitute_execution::OwnedInputError::Admission(
                AdmissionError::QueueSaturated { .. } | AdmissionError::MemorySaturated { .. },
            )) => {
                return Ok(());
            }
            Err(cause) => return Err(cause.into()),
        };
        record.restart.admitted = true;
        self.next_connection = next_connection;
        self.pending[at] = Some(PendingProjection {
            qualification: Default::default(),
            run,
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
            connection,
            job: Some(job),
            source: None,
            completion: None,
            restart: true,
            probe: Default::default(),
        });
        Ok(())
    }
}

/// Borrow real registered admission only after its successful projection is installed in this Record.
/// This grants no fresh launch, cleanup or frontend readiness authority.
pub(super) struct InstalledRestartIngress<'a> {
    admission: sugarsubstitute_execution::RegisteredRestartAdmission<'a>,
    installed: &'a Record,
}
impl InstalledRestartIngress<'_> {
    pub(super) fn previous(&self) -> RunKey {
        self.admission.previous()
    }
    pub(super) fn run(&self) -> RunKey {
        self.admission.run()
    }
    pub(super) fn source(&self) -> &sugarsubstitute_execution::RegisteredStartupSource {
        self.admission.source()
    }
    pub(super) fn check(&self) -> Result<()> {
        if !self.installed.restart.installation_pending
            || self.installed.run != self.admission.run()
        {
            return Err("Actual installed restart custody changed".into());
        }
        self.admission.check()?;
        Ok(())
    }
}
impl ProductionActivation {
    /// Keep the whole successful installed projection and snapshot through deferred receipt adoption.
    /// No native attachment or new Desktop may begin until the original Reporter receipt is accepted.
    pub(super) fn adopt_installed_restarts(
        &mut self,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        for held in self.records.iter_mut().flatten() {
            let record = held.record.as_mut();
            if !record.restart.installation_pending {
                continue;
            }
            let completed =
                record.projected.as_ref().ok_or("Actual installed projection missing")?;
            let JobOutcome::Success(value) = &completed.outcome else {
                return Err(
                    "Actual restart projection failed with original completion retained".into()
                );
            };
            let snapshot =
                value.snapshot.as_ref().ok_or("Actual installed restart snapshot missing")?;
            let admission = resources.client.borrow_registered_restart_admission(
                snapshot,
                &value.registered,
                completed,
            )?;
            if record.run != admission.run() || record.start.is_some() || record.attach.is_some() {
                return Err("Actual installed restart is already transferred or foreign".into());
            }
            let original = admission.original_budget();
            if record.restart.original.is_some_and(|held| held != original) {
                return Err("Original installed restart cutoff changed".into());
            }
            record.restart.original = Some(original);
            let witness = InstalledRestartIngress { admission, installed: record };
            match self.incidents.reporting.restart_admitted(&witness)? {
                incident_reporting::RestartAcceptance::Pending => {}
                incident_reporting::RestartAcceptance::Accepted => {
                    record.restart.installation_pending = false;
                    record.restart.original = None;
                }
            }
        }
        Ok(())
    }
}
