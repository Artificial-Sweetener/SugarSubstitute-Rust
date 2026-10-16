// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive one authentic pre-ACK selection without replacing any live Root authority.
use super::*;
impl ProductionActivation {
    pub(in crate::production::entry_activation) fn advance_fresh_restart(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if self.recovery_pending.is_none()
            && !self.closing
            && let Some(selected) = self.incidents.reporting.deferred_fresh_restart()
            && let Err(cause) = self.begin_fresh_restart(selected, resources)
        {
            self.incidents.reporting.reject_deferred_fresh_restart()?;
            // No native/preparation owner transferred on this constructor rejection.
            // Retain the first actual cause in the existing Root error owner.
            self.recovery_denial.get_or_insert(cause);
            return Ok(());
        }
        let Some(mut pending) = self.recovery_pending.take() else {
            return Ok(());
        };
        if let Err(cause) = pending.capture(batch) {
            pending.preserve(cause);
        }
        if self.closing && pending.cleanup.is_none() {
            pending.cleanup = self.shutdown;
        }
        if !self.closing
            && pending.failed.is_none()
            && let Err(cause) = pending.advance(self, resources)
        {
            pending.preserve(cause);
        }
        // A first failed turn must enter its real close transaction before rearming
        // or archival; waiting for another unrelated event would strand native work.
        if (self.closing || pending.failed.is_some())
            && let Err(cause) = pending.close(self, resources)
        {
            pending.cleanup_failure.get_or_insert(cause);
        }
        let owner_failed = pending.owner_failed();
        if owner_failed {
            self.failure_run.get_or_insert(pending.identity.run);
            self.preserve(
                "Actual temporary recovery execution owner failed; original cause retained".into(),
            );
        }
        if pending.installed {
            return Ok(());
        }
        if pending.failed.is_some() && pending.settled() {
            let at = pending.at;
            if self.failed_recoveries[at].is_some() {
                self.recovery_pending = Some(pending);
                return Err("Actual failed recovery custody slot occupied".into());
            }
            self.failed_recoveries[at] = Some(pending);
        } else {
            self.recovery_pending = Some(pending);
        }
        if owner_failed {
            Err("Actual temporary recovery execution owner failed; original cause retained".into())
        } else {
            Ok(())
        }
    }
    fn begin_fresh_restart(
        &mut self,
        selected: incident_reporting::DeferredFreshRestart,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        let owner = resources.authority.key().ok_or("Actual recovery Root missing")?;
        let previous = self.incidents.reporting.fresh_restart_previous(owner)?;
        let at = self
            .records
            .iter()
            .position(|held| held.as_ref().is_some_and(|held| held.record.run == previous))
            .ok_or("Actual recovery registered source missing")?;
        if self.failed_recoveries[at].is_some() {
            return Err("Actual failed recovery grant is closed for this run".into());
        }
        if self.pending[at].is_some() {
            return Err("Actual recovery slot has another projection".into());
        }
        let record =
            self.records[at].as_ref().ok_or("Actual recovery record missing")?.record.as_ref();
        let start = record.start.as_ref().ok_or("Actual failed family/profile source missing")?;
        let storage = start.projection.memory.claim(size_of::<RecoveryPreparation>() as u64)?;
        let outside = start.projection.memory.clone();
        let original = EntrySetupBudget::new(resources.client.now()?)?;
        if original.started() >= selected.original_until() {
            return Err("Original Restart request expired before recovery admission".into());
        }
        let identity = MainIdentity {
            run: RunKey { generation: previous.generation.next()?, ..previous },
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        };
        let connection =
            self.next_connection.checked_add(1).ok_or("Connection identity exhausted")?;
        self.recovery_pending = Some(RetainedPreparation {
            pending: Box::new(RecoveryPreparation {
                at,
                selected,
                previous,
                identity,
                connection: ConnectionId::new(connection)?,
                original,
                job: None,
                completion: None,
                grant: None,
                record: None,
                origin: None,
                origin_rejected: None,
                origin_scope: None,
                retired_seed: None,
                seed_settled: false,
                old_shutdown: None,
                cleanup: None,
                ack_requested: false,
                failed: None,
                cleanup_failure: None,
                rejected: false,
                installed: false,
                _storage: storage,
            }),
            _outside: outside,
        });
        self.next_connection = connection;
        Ok(())
    }
}
impl RecoveryPreparation {
    fn advance(
        &mut self,
        root: &mut ProductionActivation,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if self.installed {
            return Ok(());
        }
        let now = resources.client.now()?;
        if now < self.original.started() || now >= self.original.until() {
            return Err("Original fresh recovery admission expired".into());
        }
        if self.record.is_none() {
            self.prepare_projection(root, resources)?;
        }
        if self.record.is_none() {
            return Ok(());
        }
        if self.ack_requested {
            // Committed creation is no longer a pre-ACK admission candidate.
            // Keep its actual session progressing while Reporter disposal completes.
            return self.install(root, resources);
        }
        if !self.prepare_old(root, resources)? {
            return Ok(());
        }
        if !self.prepare_admission(root, resources)? {
            return Ok(());
        }
        if !self.ack_requested {
            self.request_ack(root, resources)?;
        }
        if self.ack_requested {
            self.install(root, resources)?;
        }
        Ok(())
    }
    fn prepare_old(
        &mut self,
        root: &mut ProductionActivation,
        resources: &mut EntryResources<'_>,
    ) -> Result<bool> {
        let old = root.records[self.at]
            .as_mut()
            .ok_or("Actual original recovery family missing")?
            .record
            .as_mut();
        if old.run != self.previous {
            return Err("Actual recovery source run changed".into());
        }
        if !old.closed {
            let budget = match self.old_shutdown {
                Some(value) => value,
                None => {
                    let value = ShutdownBudget::new(resources.client.now()?)?;
                    self.old_shutdown = Some(value);
                    value
                }
            };
            let backend =
                root.backend.as_mut().ok_or("Actual recovery backend missing")?.backend.as_mut();
            ProductionActivation::close_record(
                old,
                backend,
                root.capture.as_ref(),
                resources,
                budget,
            )?;
        }
        Ok(old.closed
            && old.retired.as_ref().is_some_and(|retired| retired.run() == old.run)
            && old.seed.delivered()
            && old.collection.delivered()
            && old.attach.is_none()
            && old.start.as_ref().is_some_and(|start| start.profile.snapshot().retirement_ready()))
    }
}
