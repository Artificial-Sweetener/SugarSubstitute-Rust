// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence contained desktop creation behind the exact genuine durable seed transaction.
use super::*;
use crate::production::{
    diagnostic_helper::{SeedInput, SeedServiceIssue},
    incident_host::*,
};
use sugarsubstitute_application::incident::{IncidentEffect, IncidentOperationScope};
use sugarsubstitute_contracts::{
    execution::{Moment, TerminalKind},
    lifecycle::RunEnrollment,
};

struct SeedTransaction {
    transaction: Option<transactions::DiagnosticTransaction>,
    run: RunKey,
    scope: IncidentOperationScope,
    submitted: bool,
    cleanup_until: Option<Moment>,
    cleanup_submitted: bool,
}
/// Retain one genuine namespace/controller and serialize its bounded seed source role.
pub(super) struct IncidentFlow {
    pub(super) host: IncidentHost,
    pub(super) reporting: super::incident_reporting::IncidentReporting,
    preparation: Option<PreparationInput>,
    setup_until: Option<Moment>,
    preparing: bool,
    seed: Option<SeedTransaction>,
    pub(super) transactions: transactions::DiagnosticTransactions,
    pub(super) collection_run: Option<RunKey>,
    recovery: recovery::RecoveryFlow,
}
impl IncidentFlow {
    /// Close original jobs before releasing the namespace or capture registration.
    pub(super) fn request_close(&mut self, budget: ShutdownBudget) {
        self.reporting.request_close(budget);
        self.host.request_close(budget);
    }
    /// Observe actual constructor and close-only seed delivery independently of expiry.
    pub(super) fn close_seed(
        &mut self,
        resources: &EntryResources<'_>,
        budget: ShutdownBudget,
    ) -> Result<bool> {
        if !(if self.host.closing() {
            self.host.construction_settled()
        } else {
            self.host.construction_delivered()
        }) {
            return Ok(false);
        }
        if !self.close_recovery(resources, budget)? {
            return Ok(false);
        }
        let Some(active) = &mut self.seed else {
            return Ok(true);
        };
        let service = if self.host.closing() {
            self.host.retiring_seed_service()?
        } else {
            self.host.seed_service()?
        };
        if service.outcome().is_none() && active.submitted {
            return Ok(false);
        }
        let until = *active.cleanup_until.get_or_insert(budget.settle_until);
        if !service.cleanup_started() {
            match service.submit_cleanup(resources.client, resources.cleanup, resources.jobs, until)
            {
                Ok(()) => active.cleanup_submitted = true,
                Err(SeedServiceIssue::Admission) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        let retired = service.files_retired();
        if retired {
            self.transactions.retired(active.scope.operation(), &mut active.transaction)?;
            self.seed = None;
        }
        Ok(retired)
    }
    pub(super) const fn new() -> Self {
        Self {
            host: IncidentHost::new(),
            reporting: super::incident_reporting::IncidentReporting::new(),
            preparation: None,
            setup_until: None,
            preparing: false,
            seed: None,
            transactions: transactions::DiagnosticTransactions::new(),
            collection_run: None,
            recovery: recovery::RecoveryFlow::new(),
        }
    }
    /// Transfer the native entry namespace once before actual capture or child effects.
    pub(super) fn prepare(&mut self, resources: &mut EntryResources<'_>) -> Result<()> {
        if self.preparing {
            if self
                .host
                .preparation_outcome()
                .is_some_and(|outcome| outcome != TerminalKind::Success)
            {
                tracing::error!(
                    operation = "incident_constructor_stopped",
                    panic_retained = self.host.panic_cause().is_some(),
                    original_cause_retained = self.host.failure().is_some(),
                    "Original incident constructor terminal retained"
                );
                return Err(
                    "Actual incident constructor stopped with original custody retained".into()
                );
            }
            return Ok(());
        }
        if self.preparation.is_none() {
            let owner = resources.authority.key().ok_or("Actual incident coordinator missing")?;
            let namespace =
                resources.entry.take_namespace().ok_or("Actual incident namespace missing")?;
            self.transactions.bind(owner)?;
            self.setup_until = Some(namespace.original_preparation().until());
            self.preparation = Some(PreparationInput::new(namespace, owner));
        }
        let until = self.setup_until.ok_or("Original incident setup deadline missing")?;
        self.host.submit_prepare(
            resources.client,
            resources.finite,
            resources.jobs,
            until,
            &mut self.preparation,
        )?;
        self.preparing = true;
        Ok(())
    }
    /// Consume actual constructor/helper terminals during both normal progress and retirement.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        let results: [Result<()>; 8] = [
            self.reporting.capture(batch),
            self.host.capture_recovery(batch).map_err(Into::into),
            self.host.capture(batch).map(|_| ()).map_err(Into::into),
            self.host.capture_seed_jobs(batch).map(|_| ()).map_err(Into::into),
            self.host.capture_collection(batch).map(|_| ()).map_err(Into::into),
            self.host.capture_collection_cleanup(batch).map(|_| ()).map_err(Into::into),
            self.host.capture_reporting(batch).map(|_| ()).map_err(Into::into),
            self.host.capture_export(batch).map(|_| ()).map_err(Into::into),
        ];
        let mut first = None;
        for result in results {
            if let Err(cause) = result {
                first.get_or_insert(cause);
            }
        }
        first.map_or(Ok(()), Err)
    }
    /// Seed only the same already held profile and enrolled run before native child creation.
    /// Every denial preserves original data and source custody; retries reuse one cutoff.
    /// Ordinary startup waits pending historical reports. A genuine same-run recovery grant
    /// admits its pre-ACK seed because that report's ACK depends on this checked fresh preparation.
    pub(super) fn seed_ready(
        &mut self,
        record: &mut Record,
        capture: &CaptureRegistration,
        backend: &mut CoordinatorBackend,
        resources: &mut EntryResources<'_>,
        recovery: Option<&sugarsubstitute_execution::RegisteredRecoveryGrant>,
    ) -> Result<bool> {
        if record.seeded {
            return Ok(true);
        }
        if let Some(grant) = recovery {
            grant.check()?;
            if grant.key().run() != record.run {
                return Err("Actual recovery seed grant changed run".into());
            }
        }
        if !self.seed_busy()
            && (recovery.is_none() && self.recovery.pending()
                || !self.transactions.available()
                || self.host.next_effect()?.is_some())
        {
            return Ok(false);
        }
        if self.collection_run.is_some() {
            return Ok(false);
        }
        if !self.host.prepared() {
            return Ok(false);
        }
        let start = record.start.as_ref().ok_or("Actual desktop start missing for seed")?;
        if !start.profile.snapshot().held || start.profile.snapshot().pending {
            return Ok(false);
        }
        let selected =
            capture.capture.registration_build().ok_or("Actual selected build missing")?;
        record.seed.submit(record.run, selected, resources, start.value()?.until)?;
        let Some(material) = record.seed.material()? else {
            return Ok(false);
        };
        if let Some(active) = &self.seed {
            if active.run != record.run {
                return Ok(false);
            }
        } else {
            backend
                .preflight_policy(
                    sugarsubstitute_application::supervision::ResourceStorageInput::Current,
                )
                .map_err(outer)?;
            let enrollment = RunEnrollment {
                run: record.run,
                launch: record.launch,
                profile: start.value()?.profile.clone(),
            };
            backend
                .incident_policy(
                    sugarsubstitute_application::supervision::ResourceStorageInput::Enrollment(
                        &enrollment,
                    ),
                )
                .map_err(outer)?
                .enroll_run(enrollment)?;
            self.host.seed(
                backend
                    .incident_policy(
                        sugarsubstitute_application::supervision::ResourceStorageInput::Current,
                    )
                    .map_err(outer)?,
                material.seed.clone(),
                material.context,
            )?;
            let Some(IncidentEffect::PublishSeed { scope, seed }) = self.host.next_effect()? else {
                return Err("Actual seed command missing".into());
            };
            let scope = *scope;
            let seed = seed.clone();
            let request = self.host.namespace()?.diagnostic_write_request(scope.operation())?;
            let mut input = Some(SeedInput { request, operation: scope.operation(), seed });
            let mut transaction = None;
            if !self.transactions.admit(
                transactions::DiagnosticTransactionRole::Seed,
                scope.operation(),
                &mut transaction,
            )? {
                return Err("Actual seed admission changed within the same native turn".into());
            }
            self.seed = Some(SeedTransaction {
                transaction,
                run: record.run,
                scope,
                submitted: false,
                cleanup_until: None,
                cleanup_submitted: false,
            });
            let submitted = self.host.seed_service()?.submit(
                resources.client,
                resources.finite,
                resources.jobs,
                scope.original().until(),
                &mut input,
            );
            if input.is_some() {
                let active = self.seed.as_mut().ok_or("Actual seed transaction missing")?;
                self.transactions.retired(active.scope.operation(), &mut active.transaction)?;
                self.seed = None;
                return Err("Actual seed source did not retain original input".into());
            }
            match self.host.take_effect()? {
                Some(IncidentEffect::PublishSeed { scope: actual, .. }) if actual == scope => {}
                _ => return Err("Actual retained seed command identity changed".into()),
            }
            self.seed.as_mut().ok_or("Actual seed transaction missing")?.submitted =
                submitted.is_ok();
            if let Err(cause) = submitted
                && cause != SeedServiceIssue::Admission
            {
                return Err(cause.into());
            }
        }
        let active = self.seed.as_mut().ok_or("Actual seed transaction missing")?;
        let service = self.host.seed_service()?;
        if !active.submitted {
            match service.retry(
                resources.client,
                resources.finite,
                resources.jobs,
                active.scope.original().until(),
            ) {
                Ok(()) => active.submitted = true,
                Err(SeedServiceIssue::Admission) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        if service.outcome().is_none() {
            return Ok(false);
        }
        if active.cleanup_until.is_none() {
            active.cleanup_until =
                Some(resources.client.now()?.checked_add(std::time::Duration::from_secs(15))?);
        }
        if !active.cleanup_submitted {
            match service.submit_cleanup(
                resources.client,
                resources.cleanup,
                resources.jobs,
                active.cleanup_until.ok_or("Original seed cleanup cutoff missing")?,
            ) {
                Ok(()) => active.cleanup_submitted = true,
                Err(SeedServiceIssue::Admission) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        if !service.files_retired() {
            return Ok(false);
        }
        let scope = active.scope;
        self.transactions.retired(active.scope.operation(), &mut active.transaction)?;
        self.host.adopt_seed_service(scope)?;
        record.seeded = true;
        self.seed = None;
        Ok(true)
    }
}

impl IncidentFlow {
    /// Observe cached application transaction custody after independently pumping every close owner.
    pub(super) fn diagnostic_transactions_settled(&self) -> bool {
        self.transactions.settled()
    }
    pub(super) fn seed_busy(&self) -> bool {
        self.seed.is_some()
    }
    /// Classify retained cached duties for qualification without advancing any owner.
    pub(super) fn idle_block(&self) -> Option<&'static str> {
        if self.recovery.pending() {
            return Some("IncidentRecovery");
        }
        if self.preparation.is_some() {
            return Some("IncidentPreparation");
        }
        if self.seed.is_some() {
            return Some("IncidentSeed");
        }
        if self.collection_run.is_some() {
            return Some("IncidentCollection");
        }
        if self.host.diagnostic_custody_pending() {
            return Some("IncidentHost");
        }
        self.reporting.idle_block()
    }
}

#[path = "incident_flow/transactions.rs"]
pub(super) mod transactions;

#[path = "incident_flow/recovery.rs"]
mod recovery;

#[path = "incident_flow/recovery_seed.rs"]
mod recovery_seed;
pub(super) use crate::production::diagnostic_helper::RetiredRecoverySeed;
