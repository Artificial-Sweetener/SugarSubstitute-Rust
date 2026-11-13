// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep updater generations and release identity independent of application selection and run custody.
use super::receipt::Receipt;
use super::*;
use std::path::PathBuf;
use sugarsubstitute_contracts::{
    installation::{Candidate, Generation, InstallationRecord, PackageDigest},
    update::Component,
};

const SELECTION: &str = "updater-selection-v1";
const MARKER: &str = "updater-established-v1";
const ESTABLISHED: &[u8] = b"established";
mod custody;
mod probe;
mod recovery;
mod request;
mod running;
mod trial;

/// Only preparation may initialize a missing engine journal; probes and trials require continuity.
#[derive(Clone, Copy)]
enum SelectionRead {
    Initialize,
    Existing,
}

impl TrustStore {
    /// Installation ancestry retained by the engine's authenticated trust owner.
    pub(in crate::setup_storage) fn updater_installation_path(&self) -> &Path {
        self._installation_root.path()
    }

    /// Read or initialize empty updater selection under independent update ownership.
    /// A candidate is never healthy merely because it appears here. Execute on an admitted
    /// storage worker with ordinary trust-operation capacity beside this retained store.
    /// # Errors
    /// Preserves corrupt, foreign or missing established state; rejects busy writers and cancellation.
    pub fn updater_selection(
        &self,
        context: &JobContext,
    ) -> Result<InstallationRecord, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            self.updater_locked(protection, SelectionRead::Initialize, context)
        })
    }

    /// Read complete state while the caller holds the trust writer; initialization never claims health.
    fn updater_locked(
        &self,
        protection: &Protection,
        mode: SelectionRead,
        context: &JobContext,
    ) -> Result<InstallationRecord, TrustFailure> {
        let marker = self.read_bound_record(MARKER, envelope::Kind::UpdaterMarker, protection)?;
        if marker.is_none() && matches!(mode, SelectionRead::Existing) {
            return Err(invalid("Established updater continuity is required").into());
        }
        if marker.as_deref().is_some_and(|bytes| bytes != ESTABLISHED) {
            return Err(invalid("Updater continuity marker is invalid").into());
        }
        let state = match self.read_bound_record(
            SELECTION,
            envelope::Kind::UpdaterSelection,
            protection,
        )? {
            Some(bytes) => InstallationRecord::decode(&bytes).map_err(io::Error::other)?,
            None if marker.is_some() => {
                return Err(invalid("Established updater selection is missing").into());
            }
            None => {
                let empty = InstallationRecord::empty(self.installation.clone());
                self.write_updater(&empty, protection, context)?;
                empty
            }
        };
        if state.installation() != &self.installation || (marker.is_none() && state.revision() != 0)
        {
            return Err(invalid("Updater selection identity or continuity differs").into());
        }
        if marker.is_none() {
            self.retain_bound_record(
                MARKER,
                envelope::Kind::UpdaterMarker,
                ESTABLISHED,
                protection,
                context,
            )?;
        }
        check(context)?;
        Ok(state)
    }

    /// Reserve or resume the exact authenticated release. Called only inside `publish_updater`.
    /// Its caller freshly acquires engine exclusion and retains it and trust-writer custody
    /// throughout this operation. A retained trial owner must never call this reservation path.
    pub(in crate::setup_storage) fn reserve_updater(
        &self,
        request: &Request,
        offer: &VerifiedOffer,
        digest: &str,
        preparation: &crate::durable_storage::protection::SlotExclusion,
        context: &JobContext,
    ) -> Result<Generation, TrustFailure> {
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            self.verify_updater_preparation(preparation.witness(), protection, context)?;
            let state = self.updater_locked(protection, SelectionRead::Initialize, context)?;
            let state = self.recover_updater_locked(state, protection, context)?;
            if let Some(candidate) = state.candidate()
                && request.intent
                    == sugarsubstitute_contracts::installation::PreparationIntent::Automatic
            {
                let generation = candidate.generation();
                let receipt = self.updater_receipt(generation, protection)?;
                if generation.package.as_str() == digest
                    && receipt.channel == request.channel
                    && receipt.platform == request.platform
                    && receipt.sequence == offer.offer().sequence()
                    && receipt.archive_sha256 == offer.offer().package_digest()
                {
                    return Ok(generation.clone());
                }
            }
            let generation = Generation {
                id: crate::setup_storage::identity::generation()?,
                package: PackageDigest::new(digest.to_owned()).map_err(io::Error::other)?,
                release: crate::setup_storage::signed_release(offer)?,
            };
            let now = self.strike_clock.now().map_err(io::Error::other)?;
            let next = match state.candidate() {
                Some(candidate) => state.supersede_preparation(
                    candidate.generation(),
                    generation.clone(),
                    request.intent,
                    now,
                ),
                None => state.prepare(generation.clone(), request.intent, now),
            }
            .map_err(io::Error::other)?;
            let receipt = Receipt {
                schema_version: 1,
                generation: generation.clone(),
                channel: request.channel,
                platform: request.platform,
                component: Component::Updater,
                sequence: offer.offer().sequence(),
                archive_sha256: offer.offer().package_digest(),
            };
            self.retain_bound_record(
                &Self::updater_receipt_name(&generation),
                envelope::Kind::UpdaterRelease,
                &serde_json::to_vec(&receipt).map_err(io::Error::other)?,
                protection,
                context,
            )?;
            self.write_updater(&next, protection, context)?;
            Ok(generation)
        })
        .map_err(TrustFailure::from)
    }

    /// Publish completion after exact destination verification inside `publish_updater`.
    /// Never overwrites a newer candidate or a trial, and never changes accepted selection.
    pub(in crate::setup_storage) fn complete_updater(
        &self,
        generation: &Generation,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        protection::with_domain(context, |protection| -> Result<_, TrustFailure> {
            let state = self.updater_locked(protection, SelectionRead::Existing, context)?;
            self.updater_receipt(generation, protection)?;
            match state.candidate() {
                Some(Candidate::Prepared { generation: selected }) if selected == generation => {
                    Ok(())
                }
                _ => {
                    let next = state.prepared(generation).map_err(io::Error::other)?;
                    self.write_updater(&next, protection, context)
                }
            }
        })
        .map_err(TrustFailure::from)
    }

    /// Generation payloads are protected siblings of their journal, outside application releases.
    pub(in crate::setup_storage) fn updater_path(&self, generation: &Generation) -> PathBuf {
        self.root.path().join(format!("engine-{}", generation.id.as_str()))
    }

    /// Validate immutable identity before using an existing reservation as resumption authority.
    fn updater_receipt(
        &self,
        generation: &Generation,
        protection: &Protection,
    ) -> Result<Receipt, TrustFailure> {
        let bytes = self
            .read_bound_record(
                &Self::updater_receipt_name(generation),
                envelope::Kind::UpdaterRelease,
                protection,
            )?
            .ok_or_else(|| invalid("Updater release receipt is missing"))?;
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if receipt.schema_version != 1
            || receipt.sequence == 0
            || &receipt.generation != generation
            || receipt.component != Component::Updater
            || receipt.platform != UpdateAuthorization::local_request(receipt.channel)?.platform
        {
            return Err(invalid("Updater release receipt differs from selection").into());
        }
        Ok(receipt)
    }

    /// Canonical generation IDs cannot escape or alias another record domain.
    fn updater_receipt_name(generation: &Generation) -> String {
        format!("engine-{}.release", generation.id.as_str())
    }

    /// Commit only complete, validated records while the caller retains the trust writer.
    fn write_updater(
        &self,
        state: &InstallationRecord,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        self.replace_bound_record(
            SELECTION,
            envelope::Kind::UpdaterSelection,
            &state.encode().map_err(io::Error::other)?,
            protection,
            context,
        )
    }
}

#[cfg(test)]
mod tests;
