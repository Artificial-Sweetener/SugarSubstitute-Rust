// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join current package authority, pinned candidate bytes and exact installation run custody.
use super::*;
use crate::setup_storage::{InstallationRun, OwnedTrial, TrustFailure, UpdateAuthorization};

/// One durably admitted trial with its exact executable bytes and live run exclusion retained.
/// The host keeps this owner through authenticated readiness and complete process-family teardown.
/// Admission is not proof of health or process settlement.
pub struct AdmittedCandidate {
    prepared: Arc<PreparedCandidate>,
    trial: OwnedTrial,
}
impl AdmittedCandidate {
    /// Require trial entry before a native adapter can prepare inherited launch handles.
    pub(in crate::setup_storage) fn require_trial_launch(&self) -> io::Result<()> {
        self.prepared.compatibility().require_trial_launch()
    }
    /// Exact candidate directory whose application files remain held against modification.
    pub fn directory(&self) -> &Path {
        self.prepared.directory()
    }

    /// Owned attempt for the host's health-acknowledgement or settled-failure decision.
    pub fn trial(&self) -> &OwnedTrial {
        &self.trial
    }
}

impl PreparedCandidate {
    /// Admit a durable explicit installation choice; application health remains a separate step.
    pub(in crate::setup_storage) fn begin_installed_trial(
        self: Arc<Self>,
        run: &mut InstallationRun,
        authorization: &crate::setup_storage::selection::InstallationAuthorization,
        context: &JobContext,
    ) -> Result<AdmittedCandidate, crate::setup_storage::SelectionFailure> {
        self.compatibility().require_installed_launch()?;
        self.compatibility().require_trial_launch()?;
        let trial = {
            let snapshot = crate::native_claims::lock(&self.snapshot);
            self.require_installation_authorization(authorization, &snapshot)?;
            run.validate_installation(&self.installation)?;
            run.require_launcher(self.compatibility(), context)?;
            run.begin_trial(&snapshot, context)?
        };
        Ok(AdmittedCandidate { prepared: self, trial })
    }
    /// Admit a candidate reopened on its update receipt (D22): the receipt must still name this
    /// exact prepared revision and directories, and every receipt condition must hold at `now`
    /// (Unix UTC seconds, injected clock) while run custody is held. Nothing is published on
    /// any failure; the caller starts the accepted version.
    pub(in crate::setup_storage) fn begin_received_trial(
        self: Arc<Self>,
        run: &mut InstallationRun,
        trust: &crate::setup_storage::TrustStore,
        receipt: &crate::setup_storage::UpdateReceipt,
        request: &sugarsubstitute_contracts::update::Request,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<AdmittedCandidate, TrustFailure> {
        self.compatibility().require_installed_launch()?;
        self.compatibility().require_trial_launch()?;
        let trial = {
            let snapshot = crate::native_claims::lock(&self.snapshot);
            receipt.require(&self.installation, &self.directory, &snapshot)?;
            run.validate_installation(&self.installation).map_err(io::Error::other)?;
            run.require_launcher(self.compatibility(), context).map_err(io::Error::other)?;
            trust.admit_receipt(receipt, request, now(), context)?;
            run.begin_trial(&snapshot, context).map_err(io::Error::other)?
        };
        Ok(AdmittedCandidate { prepared: self, trial })
    }

    /// Retain the prepared snapshot and original run before trial publication can occur.
    pub(in crate::setup_storage) fn reserve_unstarted(
        &self,
        run: &InstallationRun,
        context: &JobContext,
    ) -> Result<crate::setup_storage::UnstartedTrial, crate::setup_storage::SelectionFailure> {
        run.validate_installation(&self.installation)?;
        run.reserve_unstarted(&crate::native_claims::lock(&self.snapshot), context)
    }

    /// Restore the exact unstarted attempt and bind subsequent retries to its durable revision.
    pub(crate) fn restore_unstarted(
        &self,
        recovery: &crate::setup_storage::UnstartedTrial,
        context: &JobContext,
    ) -> Result<(), crate::setup_storage::SelectionFailure> {
        recovery.restore(
            &self.installation,
            &mut crate::native_claims::lock(&self.snapshot),
            context,
        )
    }

    /// Admit this exact candidate after rechecking current signed authority and installation custody.
    /// Shares pinned preparation with the trial owner so failed admission can be retried.
    /// Retained preparation grants no launch authority: every call revalidates the exact snapshot,
    /// and a recorded trial cannot be admitted again even after its run custody is released.
    /// The caller must launch only after success and retain the result through native teardown.
    /// `now` supplies Unix UTC seconds. Reserve `SIGNED_TRANSFER_RESERVATION_BYTES` alongside
    /// this candidate; its trust owner retains the previously admitted parsing workspace.
    /// # Errors
    /// Rejects expired/withdrawn packages, foreign run owners and changed selection. On failure,
    /// persisted candidate state remains available for reinspection; no launch is attempted.
    pub fn begin_trial(
        self: Arc<Self>,
        run: &mut InstallationRun,
        authorization: &UpdateAuthorization<'_>,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<AdmittedCandidate, TrustFailure> {
        self.compatibility().require_installed_launch()?;
        self.compatibility().require_trial_launch()?;
        let trial = {
            let snapshot = crate::native_claims::lock(&self.snapshot);
            self.authorize(authorization, now, context, |_, recheck| {
                run.validate_installation(&self.installation).map_err(io::Error::other)?;
                run.require_launcher(self.compatibility(), context).map_err(io::Error::other)?;
                recheck()?;
                run.begin_trial(&snapshot, context).map_err(io::Error::other)
            })?
        };
        Ok(AdmittedCandidate { prepared: self, trial })
    }
}
