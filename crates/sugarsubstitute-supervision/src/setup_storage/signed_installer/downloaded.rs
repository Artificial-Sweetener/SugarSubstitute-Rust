// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join retained online delivery to the shared installer preparation lifetime.
use super::*;

impl InstallationSession {
    /// Import retained authenticated bytes into the shared installation/update preparation owner.
    /// Obtain this session after downloading so network work holds no setup exclusion. Existing
    /// application run custody is independent and may remain held throughout preparation.
    /// The repository identity and local subscription remain those used for discovery. Current
    /// trust, release floors and expiry are checked before extraction and again before returning.
    /// This starts no process, publishes no candidate and grants no activation or cleanup rights.
    /// `now` supplies current Unix UTC seconds. Reserve `SIGNED_PREPARATION_RESERVATION_BYTES`
    /// for this worker in addition to the trust, offer and download custody already retained.
    /// # Errors
    /// Rejects foreign installation trust, changed authorization, mismatched archives, clock
    /// rollback, extraction failures and cancellation. Owned partial files remain recoverable.
    /// Execute on an admitted storage worker; never call on the UI thread.
    pub fn import_download(
        self,
        trust: TrustStore,
        offer: AuthenticatedOffer,
        download: VerifiedDownload,
        workspace: &Path,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<SignedInstallation, TrustFailure> {
        check(context)?;
        let started = now();
        offer.authorization(&trust).verify_delivery(
            self.directory(),
            download.identity(),
            sugarsubstitute_contracts::update::Component::Application,
            started,
            context,
        )?;
        let cache = trust.workspace_cache(workspace, context)?;
        let bundle = Arc::new(VerifiedBundle::from_download(download, &cache, context)?);
        let finished = now();
        if finished < started {
            return Err(sugarsubstitute_update::Error::ClockRollback.into());
        }
        offer.authorization(&trust).verify_delivery(
            self.directory(),
            bundle.download_identity().ok_or_else(|| invalid("Downloaded archive absent"))?,
            sugarsubstitute_contracts::update::Component::Application,
            finished,
            context,
        )?;
        check(context)?;
        Ok(SignedInstallation { session: self, bundle, trust, offer })
    }
}
