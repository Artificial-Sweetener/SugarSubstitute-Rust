// SPDX-License-Identifier: GPL-3.0-or-later
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

    /// Check while the caller retains setup or run custody, excluding launcher installation.
    pub(in crate::setup_storage) fn require_launcher(
        &self,
        compatibility: &Compatibility,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            entrypoint::stable::require_compatible(&self.root, compatibility, protection, context)?;
            Ok(())
        })
    }
}
