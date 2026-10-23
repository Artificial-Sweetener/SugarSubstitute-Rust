// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reopen completed native preparation without extracting its provisioning distribution again.
use super::*;
use crate::setup_storage::{TrustFailure, TrustStore, UpdateAuthorization};

impl PreparedCandidate {
    /// Retain destination and configuration pins before reauthorizing the exact prepared snapshot.
    /// Missing preparation returns `None`; invalid evidence remains an error and is preserved.
    pub(in crate::setup_storage) fn reopen(
        session: &InstallationSession,
        trust: &TrustStore,
        authorization: &UpdateAuthorization<'_>,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<Option<(Arc<Self>, ExistingInstallation)>, TrustFailure> {
        let store = crate::setup_storage::SelectionStore::open(session.directory(), context)
            .map_err(io::Error::other)?;
        let snapshot = store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Installation selection is absent"))?;
        let generation = match snapshot.candidate() {
            Some(Candidate::Prepared { generation }) => generation.clone(),
            Some(Candidate::Staging { .. }) | None => return Ok(None),
            _ => return Err(invalid("Candidate is not available for prepared startup").into()),
        };
        let Some(publication) = trust.published_release(&generation, context)? else {
            return Ok(None);
        };
        let path = session.directory().join("releases").join(generation.id.as_str());
        let (_, marker) =
            super::super::generation::open(&path, generation.package.as_str(), context)?;
        let existing = session
            .existing
            .as_ref()
            .ok_or_else(|| invalid("Prepared startup requires saved installation choices"))?;
        let configuration =
            existing.reopen_candidate(&session.admission.root, &generation, context)?;
        let application = crate::setup_storage::application::VerifiedApplication::open(
            &session.admission.root,
            generation,
            context,
        )?;
        application.require_installed_launch()?;
        application.payload.compatibility.require_trial_launch()?;
        let installation = Directory::open(session.directory(), false)?;
        authorization.publish_prepared(
            &installation,
            &publication,
            now,
            context,
            |_, recheck| {
                store
                    .require_launcher(&application.payload.compatibility, context)
                    .map_err(io::Error::other)?;
                recheck()?;
                if store.load(context).map_err(io::Error::other)?.as_ref() != Some(&snapshot) {
                    return Err(io::Error::other(crate::setup_storage::SelectionFailure::Changed));
                }
                recheck()
            },
        )?;
        let candidate = Self {
            generation: application.generation,
            directory: application.directory,
            installation,
            snapshot: std::sync::Mutex::new(snapshot),
            _payload: application.payload,
            evidence: Evidence::Published {
                publication: Box::new(publication),
                _inventory: application.inventory,
                _marker: marker,
            },
        };
        Ok(Some((Arc::new(candidate), configuration)))
    }
}
