// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit real claims and registry construction under the original entry cutoff.
use super::*;

impl CoordinatorAuthority {
    /// Bind actual election first, then admit its original finite claims initialization once.
    /// Errors retain all actual adapter custody and close this owner's ordinary admission.
    pub(crate) fn prepare(
        &mut self,
        entry: &CoordinatorEntryOwner,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        installation: PathBuf,
    ) -> Result<(), AuthorityIssue> {
        if self.association.is_some() {
            return Err(AuthorityIssue::Started);
        }
        if self.closing {
            return Err(AuthorityIssue::Closed);
        }
        if entry.snapshot().phase != EntryPhase::ElectionHeld
            || entry.original_setup_budget() != self.original
            || entry.original_execution_owner() != Some(scope.owner_id())
        {
            return Err(AuthorityIssue::Identity);
        }
        self.association = Some((scope.owner_id(), scope.id()));
        if let Err(cause) = entry.prepare_claims(&mut self.claims) {
            self.fail(Failure::Entry(cause));
            return Err(AuthorityIssue::Adapter);
        }
        let spec = self.spec(
            client,
            scope,
            jobs,
            sugarsubstitute_supervision::setup_storage::LAUNCH_RESERVATION_BYTES,
            self.original.until(),
        )?;
        if let Err(cause) = self.installation.start(client, scope, spec, installation.clone()) {
            self.fail(Failure::InstallationAdmission(cause));
            return Err(AuthorityIssue::Adapter);
        }
        let spec = self.spec(client, scope, jobs, CLAIM_POOL_BYTES, self.original.until())?;
        match self.claims.submit_initialize(client, scope, spec, installation) {
            Ok(id) => self.initialization = Some(id),
            Err(cause) => {
                self.fail(Failure::ClaimsAdmission(cause));
                return Err(AuthorityIssue::Adapter);
            }
        }
        Ok(())
    }
    pub(super) fn spec(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        bytes: u64,
        until: Moment,
    ) -> Result<JobSpec, AuthorityIssue> {
        let now = match client.now() {
            Ok(now) => now,
            Err(cause) => {
                self.fail(Failure::Clock(cause));
                return Err(AuthorityIssue::Clock);
            }
        };
        if self.association.is_none_or(|(owner, _)| owner != scope.owner_id())
            || scope.snapshot().closed
            || scope.snapshot().cancellation_requested
            || now >= until
        {
            return Err(AuthorityIssue::Closed);
        }
        let id = match jobs.allocate() {
            Ok(id) => id,
            Err(cause) => {
                self.fail(Failure::Identity(cause));
                return Err(AuthorityIssue::Identity);
            }
        };
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    pub(super) fn construct_registry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), AuthorityIssue> {
        if self.registry_job.is_some() || self.registry_completion.is_some() {
            return Ok(());
        }
        let key = self.key.ok_or(AuthorityIssue::Identity)?;
        let spec = self.spec(
            client,
            scope,
            jobs,
            WindowsProcessRegistry::required_reservation_bytes(),
            until,
        )?;
        match WindowsProcessRegistry::submit_new(client, scope, spec, key) {
            Ok(job) => self.registry_job = Some(job),
            Err(cause) => {
                self.fail(Failure::RegistryAdmission(cause));
                return Err(AuthorityIssue::Adapter);
            }
        }
        Ok(())
    }
}
