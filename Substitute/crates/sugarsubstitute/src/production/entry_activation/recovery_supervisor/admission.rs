// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind durable origin, exact old retirement and held native launch before policy ACK.
use super::*;
impl RecoveryPreparation {
    pub(super) fn prepare_admission(
        &mut self,
        root: &mut ProductionActivation,
        resources: &mut EntryResources<'_>,
    ) -> Result<bool> {
        let old = root.records[self.at]
            .as_ref()
            .ok_or("Actual original recovery slot missing")?
            .record
            .as_ref();
        let previous =
            &old.start.as_ref().ok_or("Actual original recovery profile missing")?.profile;
        let backend =
            root.backend.as_mut().ok_or("Actual recovery backend missing")?.backend.as_mut();
        let parent = backend
            .profile_custody_parent()
            .ok_or("Actual retained profile custody parent missing")?;
        let record =
            self.record.as_mut().ok_or("Actual prepared recovery record missing")?.record.as_mut();
        let start = record.start.as_mut().ok_or("Actual fresh recovery launch missing")?;
        if !start.prepare_recovery(
            previous,
            &parent,
            &root.executable,
            &root.working_directory,
            resources,
        )? {
            return Ok(false);
        }
        let capture = root.capture.as_ref().ok_or("Actual Root capture owner missing")?;
        let grant = self.grant.as_ref().ok_or("Actual recovery grant missing")?;
        if !root.incidents.seed_ready(record, capture, backend, resources, Some(grant))? {
            return Ok(false);
        }
        if self.origin.is_none() {
            if self.origin_rejected.is_some() {
                return Err("Original recovery origin constructor rejection retained".into());
            }
            if self.origin_scope.is_none() {
                self.origin_scope = Some(OriginAdmission {
                    scope: root.incidents.host.admit_recovery_capture_origin(
                        self.selected.binding(),
                        self.selected.context(),
                    )?,
                    transaction: None,
                });
            }
            let held =
                self.origin_scope.as_mut().ok_or("Actual recovery origin operation missing")?;
            let scope = held.scope;
            if resources.client.now()? >= scope.original().until() {
                return Err("Original recovery origin namespace admission cutoff elapsed".into());
            }
            if !root.incidents.transactions.admit(
                DiagnosticTransactionRole::RecoveryOrigin,
                scope.operation(),
                &mut held.transaction,
            )? {
                return Ok(false);
            }
            let request =
                root.incidents.host.namespace()?.diagnostic_write_request(scope.operation())?;
            let start = record.start.as_ref().ok_or("Actual recovery projection missing")?;
            let claim =
                start.projection.memory.claim(OriginPublication::required_storage_bytes())?;
            let source =
                capture.root_source().ok_or("Actual retained Root storage source missing")?.clone();
            match OriginPublication::new(
                claim,
                grant.clone(),
                source,
                request,
                scope.operation(),
                self.selected.context().at(),
            ) {
                Ok(service) => self.origin = Some(service),
                Err(rejected) => {
                    self.origin_rejected = Some(rejected);
                    return Err(
                        "Actual recovery origin source rejected with original inputs retained"
                            .into(),
                    );
                }
            }
            self.origin.as_mut().ok_or("Actual recovery origin service missing")?.submit(
                resources.client,
                resources.finite,
                resources.jobs,
            )?;
        }
        let origin = self.origin.as_mut().ok_or("Actual recovery origin owner missing")?;
        let progress = origin.poll(resources.client, resources.cleanup, resources.jobs);
        if origin.files_retired() {
            let held =
                self.origin_scope.as_mut().ok_or("Actual recovery origin admission missing")?;
            root.incidents.transactions.retired(held.scope.operation(), &mut held.transaction)?;
        }
        progress?;
        if !origin.ready(grant)? {
            return Ok(false);
        }
        let old = root.records[self.at]
            .as_ref()
            .ok_or("Actual old recovery profile missing")?
            .record
            .as_ref();
        self.record
            .as_ref()
            .ok_or("Actual recovery record missing")?
            .record
            .start
            .as_ref()
            .ok_or("Actual recovery start missing")?
            .recovery_acknowledgement_allowed(
                &old.start.as_ref().ok_or("Actual retired profile missing")?.profile,
                resources,
            )
    }
    pub(super) fn request_ack(
        &mut self,
        root: &mut ProductionActivation,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        let grant = self.grant.as_ref().ok_or("Actual recovery grant missing")?;
        let old =
            &root.records[self.at].as_ref().ok_or("Actual old recovery record missing")?.record;
        let next = &self.record.as_ref().ok_or("Actual admitted recovery record missing")?.record;
        let registry = resources.authority.registry().ok_or("Actual Root registry missing")?;
        self.ack_requested = root.incidents.reporting.acknowledge_prepared_fresh_restart(
            &mut root.incidents.host,
            grant,
            &next.start.as_ref().ok_or("Actual new profile missing")?.profile,
            &old.start.as_ref().ok_or("Actual retired profile missing")?.profile,
            registry,
        )?;
        Ok(())
    }
    pub(super) fn install(
        &mut self,
        root: &mut ProductionActivation,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        let grant = self.grant.as_ref().ok_or("Actual recovery grant missing")?;
        let Some(ack) = root
            .incidents
            .reporting
            .fresh_restart_acknowledgement(&mut root.incidents.host, grant)?
        else {
            return Ok(());
        };
        let old = root.records[self.at]
            .as_mut()
            .ok_or("Actual old recovery family missing")?
            .record
            .as_mut();
        let next =
            self.record.as_mut().ok_or("Actual held recovery record missing")?.record.as_mut();
        let source = root
            .capture
            .as_ref()
            .and_then(CaptureRegistration::root_source)
            .ok_or("Actual Root storage custody missing")?
            .clone();
        let start = next.start.as_mut().ok_or("Actual admitted recovery launch missing")?;
        if !start.recovery_committed() {
            start.commit_recovery(
                &old.start.as_ref().ok_or("Actual old profile missing")?.profile,
                ack,
                source,
                resources,
            )?;
        }
        start.progress_recovery(resources)?;
        let enrolled = start.with_recovery_desktop(|_| true)?.unwrap_or(false);
        if start.recovery_failed() || (start.recovery_handler_lost() && !enrolled) {
            return Err(
                "Actual fresh recovery startup lost its admitted capture/control owner".into()
            );
        }
        if !root.incidents.reporting.accept_fresh_restart(grant)? {
            return Ok(());
        }
        cause_transfer::before_replace(old, next, true)?;
        root.records[self.at] = self.record.take();
        self.installed = true;
        tracing::info!(
            operation = "fresh_recovery_family_committed",
            run_instance = self.identity.run.instance.get(),
            run_generation = self.identity.run.generation.get(),
            "Root admitted the fresh captured family; prior registrations retain degraded coverage"
        );
        Ok(())
    }
}
