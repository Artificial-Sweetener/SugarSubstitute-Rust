// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Choose what an installer import rests on once its compiled-in metadata has expired (C4).
//!
//! A downloaded installer never expires. While its compiled-in metadata is current it is used
//! as is. Once expired, the installer fetches current metadata online. Without a connection it
//! tells the user, in one plain sentence, that retrying installs the version included in this
//! installer; only a retry after that sentence installs the included version, whose signatures
//! and compiled-in digest are still verified in full. The updater then brings it current.
use std::{error::Error, fmt, io, time::Duration};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::{
    setup_storage::{IncludedInstallChoice, InstallerDistribution, InstallerMetadataChoice},
    update_repository::InstallerMetadata,
};

/// Idle bound of each metadata request while refreshing an expired installer.
const REFRESH_IDLE: Duration = Duration::from_secs(30);

/// Current metadata could not be fetched; the user has now been told that retrying installs
/// the included version. Retains the original cause for diagnostics.
#[derive(Debug)]
pub(in crate::setup) struct IncludedVersionOffered {
    cause: Box<dyn Error + Send + Sync>,
}
impl fmt::Display for IncludedVersionOffered {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Installer metadata expired and current metadata could not be fetched")
    }
}
impl Error for IncludedVersionOffered {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Decide the metadata for one import. `offered` is true only when the user retried after
/// being shown the included-version sentence; that retry is the explicit choice.
/// `refresh` fetches current metadata and runs only when the compiled-in set has expired.
/// # Errors
/// Returns `IncludedVersionOffered` when the set expired, the refresh failed and the user has
/// not yet been offered the included version.
pub(super) fn choose(
    expired: bool,
    offered: bool,
    refresh: impl FnOnce() -> Result<InstallerMetadata, Box<dyn Error + Send + Sync>>,
) -> io::Result<InstallerMetadataChoice> {
    if !expired {
        return Ok(InstallerMetadataChoice::Embedded);
    }
    match refresh() {
        Ok(current) => Ok(InstallerMetadataChoice::Refreshed(current)),
        Err(cause) if offered => {
            tracing::warn!(operation = "installer_included_version_chosen",
                cause = %crate::failure_details::Details(cause.as_ref()),
                "Installing the included version after the user's choice; current metadata is unavailable");
            Ok(InstallerMetadataChoice::Included(IncludedInstallChoice::confirmed_by_user()))
        }
        Err(cause) => Err(io::Error::other(IncludedVersionOffered { cause })),
    }
}

/// [`choose`] for this installer's compiled-in distribution at `now` (Unix UTC seconds).
/// # Errors
/// As [`choose`], and preserves unreadable compiled-in metadata.
pub(super) fn select(
    distribution: &InstallerDistribution,
    offered: bool,
    now: i64,
    context: &JobContext,
) -> io::Result<InstallerMetadataChoice> {
    choose(distribution.expired(now)?, offered, || {
        let mut repository = crate::updates::installer_repository(REFRESH_IDLE)?
            .ok_or("This build has no update repository")?;
        let fetched = repository.installer_metadata(
            distribution.next_root()?,
            &distribution.request()?,
            context,
        );
        repository.retire()?;
        Ok(fetched?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> InstallerMetadata {
        InstallerMetadata {
            roots: Vec::new(),
            timestamp: b"t".to_vec(),
            snapshot: b"s".to_vec(),
            targets: b"g".to_vec(),
            descriptor: b"d".to_vec(),
        }
    }

    #[test]
    fn current_compiled_in_metadata_is_used_without_a_network_check() -> io::Result<()> {
        let choice = choose(false, false, || Err("refresh must not run".into()))?;
        assert!(matches!(choice, InstallerMetadataChoice::Embedded));
        Ok(())
    }

    #[test]
    fn expired_metadata_with_a_connection_is_refreshed_whether_or_not_offered() -> io::Result<()> {
        for offered in [false, true] {
            let choice = choose(true, offered, || Ok(current()))?;
            assert!(matches!(choice, InstallerMetadataChoice::Refreshed(_)));
        }
        Ok(())
    }

    #[test]
    fn offline_offers_the_included_version_and_installs_it_only_on_the_retry() -> io::Result<()> {
        let first = choose(true, false, || Err("offline".into())).err().ok_or_else(|| {
            io::Error::other("Offline import proceeded without the user's choice")
        })?;
        let offer =
            first.get_ref().and_then(|cause| cause.downcast_ref::<IncludedVersionOffered>());
        assert!(offer.is_some_and(|offer| offer.source().is_some()));
        let retry = choose(true, true, || Err("still offline".into()))?;
        assert!(matches!(retry, InstallerMetadataChoice::Included(_)));
        Ok(())
    }

    #[test]
    fn the_offer_is_one_localized_sentence_in_every_locale() -> io::Result<()> {
        let offer = choose(true, false, || Err("offline".into())).err().ok_or_else(|| {
            io::Error::other("Offline import proceeded without the user's choice")
        })?;
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            let text = crate::setup::failure::storage_message(&offer, locale.id);
            assert_eq!(text, locale.installer.included_version_offer);
            assert_ne!(text, locale.installer.setup_failed);
        }
        Ok(())
    }
}
