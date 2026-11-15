// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Candidate publication checks the installed launcher's retained protocol authority.
use super::*;
use crate::setup_storage::{
    compatibility::Compatibility, entrypoint, transaction::admission::Admission,
};

impl SelectionStore {
    /// Publish stable entries for the exact durable installation choice while setup custody is held.
    /// The writer protects selection and shell ownership; publication never declares application health.
    pub(in crate::setup_storage) fn install_launcher(
        &self,
        candidate: &crate::setup_storage::PreparedCandidate,
        authorization: &super::InstallationAuthorization,
        expected: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            let current = self.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if &current != expected {
                return Err(SelectionFailure::Changed);
            }
            candidate.require_installation_authorization(authorization, &current)?;
            let directory = Directory::open(candidate.directory(), false)?;
            authorization.require(&self._installation, &directory, &current)?;
            authorization.require_payload(candidate.compatibility())?;
            entrypoint::stable::install(
                &self._installation,
                &self.root,
                candidate,
                protection,
                context,
            )?;
            Ok(())
        })
    }

    /// Prepare established shell ownership from the exact authenticated completed candidate.
    /// Selection is checked under its writer but never changed; no run lease is acquired.
    /// The accepted application and previous launcher source may be unavailable.
    pub(in crate::setup_storage) fn prepare_launcher(
        &self,
        admission: &Admission,
        candidate: &crate::setup_storage::PreparedCandidate,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        check(context)?;
        if admission.root.identity()? != self._installation.identity()? {
            return Err(SelectionFailure::RunOwnership);
        }
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            let current = self.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if !matches!(current.candidate(), Some(sugarsubstitute_contracts::installation::Candidate::Prepared { generation }) if generation == candidate.generation()) {
                return Err(SelectionFailure::Changed);
            }
            if current.accepted().is_some() {
                entrypoint::stable::prepare(
                    &admission.root,
                    &self.root,
                    candidate,
                    protection,
                    context,
                )?;
            }
            Ok(())
        })
    }

    /// Finish a launcher replacement a running copy had deferred, at a start that found no copy
    /// running. A launch never fails for it: a replacement that still cannot be published stays
    /// journaled for a later start, and the published launcher keeps starting every release.
    pub(in crate::setup_storage) fn resume_launcher(&self, context: &JobContext) {
        let resumed = (|| -> Result<bool, SelectionFailure> {
            if !entrypoint::stable::replacement_pending(&self.root)? {
                return Ok(false);
            }
            self.protected(context, |protection| {
                let _writer = writer(&self.root, protection, context)?;
                match entrypoint::stable::resume(
                    &self._installation,
                    &self.root,
                    protection,
                    context,
                ) {
                    Err(cause) if crate::durable_storage::protection::held(&cause) => {
                        tracing::info!(operation = "stable_launcher_replacement_deferred", cause = %cause,
                            "The installation is still in use; the launcher is replaced at a later start");
                        Ok(false)
                    }
                    resumed => Ok(resumed?),
                }
            })
        })();
        if let Err(cause) = resumed {
            tracing::warn!(operation = "stable_launcher_replacement_failed", cause = %cause,
                "The journaled launcher replacement could not be finished at this start");
        }
    }

    /// Check while the caller retains setup or run custody; the shared lock excludes launcher
    /// installation, which writes stable entries under the exclusive lock.
    pub(in crate::setup_storage) fn require_launcher(
        &self,
        compatibility: &Compatibility,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        self.protected(context, |protection| {
            let _reader = reader(&self.root, protection, context)?;
            entrypoint::stable::require_compatible(&self.root, compatibility, protection, context)?;
            Ok(())
        })
    }
}
