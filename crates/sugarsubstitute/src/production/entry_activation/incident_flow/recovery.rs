// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover before new seeds while retaining current startup as the only fresh-launch authority.
use super::*;
use crate::production::diagnostic_helper::recovery::{RecoveryInput, RecoveryIssue, RecoveryPhase};
use std::time::{SystemTime, UNIX_EPOCH};
use sugarsubstitute_application::incident::{IncidentOperationBudget, IncidentRecoveryScope};
use sugarsubstitute_contracts::incident::IncidentUtc;
pub(super) struct RecoveryFlow {
    scope: Option<IncidentRecoveryScope>,
    input: Option<RecoveryInput>,
    submitted: bool,
    cleanup_until: Option<Moment>,
    done: bool,
    scan_ready: bool,
    cursor: usize,
    active: Option<sugarsubstitute_contracts::incident::IncidentKey>,
    at: Option<IncidentUtc>,
}
impl RecoveryFlow {
    pub(super) const fn new() -> Self {
        Self {
            scope: None,
            input: None,
            submitted: false,
            cleanup_until: None,
            done: false,
            scan_ready: false,
            cursor: 0,
            active: None,
            at: None,
        }
    }
    pub(super) fn pending(&self) -> bool {
        self.scope.is_some() && !self.done
    }
}
impl IncidentFlow {
    /// The incoming authenticated projection supplies current environment/locale/run, never old ownership.
    pub(in crate::production::entry_activation) fn recover_pending(
        &mut self,
        records: &[Option<RetainedRecord>; 64],
        capture: Option<&sugarsubstitute_crash::CaptureRootSource>,
        resources: &mut EntryResources<'_>,
    ) -> Result<bool> {
        if self.recovery.done {
            return Ok(true);
        }
        if self.recovery.scan_ready {
            self.advance_pending()?;
            return Ok(true);
        }
        if !self.host.prepared() {
            return Ok(false);
        }
        if self.recovery.scope.is_none() {
            let Some(record) = records.iter().flatten().find(|held| {
                held.record
                    .projected
                    .as_ref()
                    .is_some_and(|done| matches!(done.outcome, JobOutcome::Success(_)))
            }) else {
                return Ok(false);
            };
            let completed = record.record.projected.as_ref().ok_or("Actual startup missing")?;
            let JobOutcome::Success(value) = &completed.outcome else {
                return Err("Actual startup failed".into());
            };
            let owner = resources.authority.key().ok_or("Actual current coordinator missing")?;
            if record.record.run.owner != owner {
                return Err("Current recovery startup owner changed".into());
            }
            self.reporting.configure_recovery_source(
                record.record.run,
                value.registered.clone(),
                value.locale,
            )?;
            let original = IncidentOperationBudget::new(resources.client.now()?)?;
            let scope = self.host.begin_recovery(original)?;
            let at = IncidentUtc::from_unix_seconds(i64::try_from(
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            )?)?;
            let storage_operation = self.host.next_recovery_storage_operation(scope)?;
            self.recovery.input = Some(RecoveryInput {
                request: self.host.namespace()?.diagnostic_write_request(storage_operation)?,
                scope,
                at,
                storage_operation,
                capture: capture.cloned(),
                phase: RecoveryPhase::Migrate,
            });
            self.recovery.at = Some(at);
            self.recovery.scope = Some(scope);
        }
        let scope = self.recovery.scope.ok_or("Original recovery missing")?;
        if !self.recovery.submitted {
            let service = self.host.recovery_service()?;
            let submitted = service.submit(
                resources.client,
                resources.finite,
                resources.jobs,
                scope.original().until(),
                &mut self.recovery.input,
            );
            if self.recovery.input.is_none()
                && matches!(self.host.next_effect()?, Some(IncidentEffect::Recover { scope: actual }) if *actual == scope)
            {
                let _owned = self.host.take_effect()?;
            }
            match submitted {
                Ok(()) => self.recovery.submitted = true,
                Err(RecoveryIssue::Admission) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        let service = self.host.recovery_service()?;
        if !service.delivered() {
            return Ok(false);
        }
        let until = match self.recovery.cleanup_until {
            Some(until) => until,
            None => {
                let until = ShutdownBudget::new(resources.client.now()?)?.settle_until;
                self.recovery.cleanup_until = Some(until);
                until
            }
        };
        if !service.cleanup_started() {
            match service.submit_cleanup(resources.client, resources.cleanup, resources.jobs, until)
            {
                Ok(()) => {}
                Err(RecoveryIssue::Admission) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        if service.cleanup_stopped() {
            return Err("Original recovery cleanup terminal retained".into());
        }
        if !service.files_retired() {
            return Ok(false);
        }
        if let Some(phase) = service.next_step()? {
            let storage_operation = if phase == RecoveryPhase::Scan {
                scope.operation()
            } else {
                self.host.next_recovery_storage_operation(scope)?
            };
            self.recovery.input = Some(RecoveryInput {
                request: self.host.namespace()?.diagnostic_write_request(storage_operation)?,
                scope,
                at: self.recovery.at.ok_or("Original recovery UTC observation missing")?,
                storage_operation,
                capture: capture.cloned(),
                phase,
            });
            self.host.recovery_service()?.advance(&mut self.recovery.input)?;
            self.recovery.submitted = false;
            self.recovery.cleanup_until = None;
            return Ok(false);
        }
        if !service.final_scan_ready()? {
            return Err("Actual final recovery scan missing after checked retirement".into());
        }
        self.recovery.scan_ready = true;
        self.advance_pending()?;
        Ok(true)
    }
    fn advance_pending(&mut self) -> Result<()> {
        use crate::production::incident_host::RecoveryProgress;
        let scope = self.recovery.scope.ok_or("Original recovery missing")?;
        match self.host.adopt_recovery(scope, self.recovery.cursor, self.recovery.active)? {
            RecoveryProgress::Pending => {}
            RecoveryProgress::Adopted(key) => {
                self.recovery.cursor =
                    self.recovery.cursor.checked_add(1).ok_or("Pending cursor exhausted")?;
                self.recovery.active = Some(key);
            }
            RecoveryProgress::Finished => self.recovery.done = true,
        }
        Ok(())
    }
    pub(super) fn close_recovery(
        &mut self,
        resources: &EntryResources<'_>,
        budget: ShutdownBudget,
    ) -> Result<bool> {
        if !self.recovery.pending() {
            return Ok(true);
        }
        let service = if self.host.closing() {
            self.host.retiring_recovery_service()?
        } else {
            self.host.recovery_service()?
        };
        service.cancel();
        if !service.delivered() {
            if service.restore_unsubmitted(&mut self.recovery.input)? {
                self.recovery.done = true;
                return Ok(true);
            }
            return Ok(false);
        }
        let until = *self.recovery.cleanup_until.get_or_insert(budget.settle_until);
        if !service.cleanup_started() {
            match service.submit_cleanup(resources.client, resources.cleanup, resources.jobs, until)
            {
                Ok(()) => {}
                Err(RecoveryIssue::Admission) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        if service.cleanup_stopped() {
            return Err("Original recovery cleanup terminal retained".into());
        }
        Ok(service.files_retired())
    }
    pub(in crate::production::entry_activation) fn recovery_deadline(&self) -> Option<Moment> {
        if self.recovery.done || self.recovery.scan_ready {
            None
        } else {
            self.recovery
                .cleanup_until
                .or_else(|| self.recovery.scope.map(|scope| scope.original().until()))
        }
    }
}
