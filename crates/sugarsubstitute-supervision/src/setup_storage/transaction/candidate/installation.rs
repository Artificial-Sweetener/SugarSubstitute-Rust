// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind durable explicit installation authorization to verified runtime and saved configuration.
use super::*;
use crate::setup_storage::{
    SelectionStore, TrustFailure, UpdateAuthorization, selection::InstallationAuthorization,
};

impl PreparedCandidate {
    /// Keep the exact prepared revision pinned while publishing this installation's shell entries.
    pub(in crate::setup_storage) fn publish_installation(
        &self,
        authorization: &InstallationAuthorization,
        context: &JobContext,
    ) -> io::Result<()> {
        let store =
            SelectionStore::open(self.installation_root(), context).map_err(io::Error::other)?;
        let snapshot = crate::native_claims::lock(&self.snapshot);
        store.install_launcher(self, authorization, &snapshot, context).map_err(io::Error::other)
    }

    /// Publish an explicit installation choice under fresh trust and exact prepared selection.
    pub(in crate::setup_storage) fn authorize_installation(
        &self,
        authorization: &UpdateAuthorization<'_>,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<InstallationAuthorization, TrustFailure> {
        self.compatibility().require_installed_launch()?;
        self.compatibility().require_trial_launch()?;
        let store =
            SelectionStore::open(&self.installation.path, context).map_err(io::Error::other)?;
        let snapshot = crate::native_claims::lock(&self.snapshot);
        self.authorize(authorization, now, context, |offer, recheck| {
            self.compatibility()
                .require_installation_activation(offer.offer().minimum_updater_release())?;
            store
                .authorize_installation(
                    &snapshot,
                    &self.directory,
                    offer.offer().minimum_updater_release(),
                    recheck,
                    context,
                )
                .map_err(io::Error::other)
        })
    }

    /// Require the retained local choice again before exact-revision trial admission.
    pub(in crate::setup_storage) fn require_installation_authorization(
        &self,
        authorization: &InstallationAuthorization,
        snapshot: &InstallationRecord,
    ) -> io::Result<()> {
        authorization.require(&self.installation, &self.directory, snapshot)
    }

    /// Reopen one authorized prepared generation without discovery, archive or repository clock.
    /// Configuration, manifest and runtime verification share the ordinary prepared-launch owners.
    pub(in crate::setup_storage) fn reopen_installed(
        session: &InstallationSession,
        context: &JobContext,
    ) -> io::Result<Option<(Arc<Self>, ExistingInstallation, InstallationAuthorization)>> {
        let store = SelectionStore::open(session.directory(), context).map_err(io::Error::other)?;
        let snapshot = store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Installation selection is absent"))?;
        let Some(Candidate::Prepared { generation }) = snapshot.candidate() else {
            return Ok(None);
        };
        let path = session.directory().join("releases").join(generation.id.as_str());
        let directory = Directory::open(&path, false)?;
        let Some(authorization) = store
            .installation_authorization(&snapshot, &directory, context)
            .map_err(io::Error::other)?
        else {
            return Ok(None);
        };
        let (_, marker) =
            super::super::generation::open(&path, generation.package.as_str(), context)?;
        let existing = session
            .existing
            .as_ref()
            .ok_or_else(|| invalid("Installed startup requires saved installation choices"))?;
        let configuration =
            existing.reopen_candidate(&session.admission.root, generation, context)?;
        let application = crate::setup_storage::application::VerifiedApplication::open(
            &session.admission.root,
            generation.clone(),
            context,
        )?;
        application.require_installed_launch()?;
        application.payload.compatibility.require_trial_launch()?;
        authorization.require_payload(&application.payload.compatibility)?;
        let candidate = Arc::new(Self {
            generation: application.generation,
            directory: application.directory,
            installation: Directory::open(session.directory(), false)?,
            snapshot: std::sync::Mutex::new(snapshot.clone()),
            _payload: application.payload,
            evidence: Evidence::Installed { _inventory: application.inventory, _marker: marker },
        });
        if store.load(context).map_err(io::Error::other)?.as_ref() != Some(&snapshot) {
            return Err(io::Error::other(crate::setup_storage::SelectionFailure::Changed));
        }
        candidate.require_installation_authorization(&authorization, &snapshot)?;
        Ok(Some((candidate, configuration, authorization)))
    }
}
