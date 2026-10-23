// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate an independent updater package without acquiring application run or setup custody.
use super::{
    AuthenticatedOffer, TrustFailure, TrustStore, VerifiedDownload, bundle::Payload, check, invalid,
};
use std::path::Path;
use sugarsubstitute_contracts::{execution::JobContext, update::Component};
mod stage;
pub use stage::{
    PreparedUpdater, RejectedUpdaterProbe, UPDATER_STAGING_RESERVATION_BYTES,
    UnstartedUpdaterTrial, UpdaterProbeTransfer,
};

/// Retained updater archive, verified files and signed evidence for subsequent publication.
/// Import does not select or execute an updater, and dropping this owner never deletes files.
pub struct UpdaterPackage {
    payload: Payload,
    trust: TrustStore,
    offer: AuthenticatedOffer,
}
impl UpdaterPackage {
    /// Authenticate and extract an updater-only download on an admitted storage worker.
    /// Reserve `SIGNED_PREPARATION_RESERVATION_BYTES` alongside retained trust/offer/download
    /// custody. `now` supplies Unix UTC seconds. No application lease or setup exclusion is taken.
    /// The installation must be the exact native installation owning the retained trust.
    /// # Errors
    /// Rejects foreign components, changed trust/subscriptions, expired metadata, clock rollback,
    /// altered bytes, incompatible entry declarations, unsafe paths and cancellation.
    /// Failed extraction preserves owned partials for diagnosis and a fresh retry.
    pub fn import(
        installation: &Path,
        trust: TrustStore,
        offer: AuthenticatedOffer,
        download: VerifiedDownload,
        workspace: &Path,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        check(context)?;
        let started = now();
        offer.authorization(&trust).verify_delivery(
            installation,
            download.identity(),
            Component::Updater,
            started,
            context,
        )?;
        let cache = trust.workspace_cache(workspace, context)?;
        let payload = Payload::from_download(download, &cache, Component::Updater, context)?;
        let finished = now();
        if finished < started {
            return Err(sugarsubstitute_update::Error::ClockRollback.into());
        }
        let package = Self { payload, trust, offer };
        package.verify_current(installation, finished, context)?;
        check(context)?;
        Ok(package)
    }

    /// Recheck retained evidence against current durable trust without granting activation rights.
    /// Call on an admitted worker with trust-operation capacity; `now` is Unix UTC seconds.
    /// # Errors
    /// Rejects stale authority, changed subscriptions, foreign installation identity or cancellation.
    pub fn verify_current(
        &self,
        installation: &Path,
        now: i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        self.offer.authorization(&self.trust).verify_delivery(
            installation,
            self.payload
                .download_identity()
                .ok_or_else(|| invalid("Updater archive custody absent"))?,
            Component::Updater,
            now,
            context,
        )
    }

    /// Exact inventory identity of the retained updater payload; not execution authority.
    pub fn digest(&self) -> &str {
        self.payload.digest()
    }
}

#[cfg(test)]
mod tests;
