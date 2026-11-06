// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate packaged setup inputs and retain filesystem custody during installation.
mod application;
mod backend_data;
pub use backend_data::{BackendStorage, OwnedBackendStorage};
mod bundle;
pub mod cache;
#[allow(unsafe_code)]
pub(crate) mod cache_custody;
mod compatibility;
pub(crate) mod provisioning_cache;
pub use application::LAUNCH_RESERVATION_BYTES;
pub use provisioning_cache::{
    DownloadRecord, MetadataChunk, MetadataRead, MetadataRecord, MetadataWrite,
    ProvisioningCachePressure, ProvisioningDownloads, ProvisioningMetadata,
    recover_provisioning_cache,
};
#[allow(unsafe_code)]
mod disk_inheritance;
mod download;
#[allow(unsafe_code)]
mod updater_probe;
pub use download::{DOWNLOAD_RESERVATION_BYTES, Download, DownloadFailure, VerifiedDownload};
pub(crate) use updater_probe::UpdaterProbeDisposition;
pub use updater_probe::{InheritedUpdaterProbe, capture_updater_probe};
mod entrypoint;
mod existing;
pub use existing::{InstallationSummary, RecoveryFailure, inspect_installation};
mod identity;
mod image;
mod inventory;
pub use image::PreparationImage;
mod running;
mod runtime_record;
pub use running::{RunningApplication, retain_running_generation};
mod ownership;
mod packed;
mod publication;
pub use packed::{MATERIALIZATION_RESERVATION_BYTES, materialize};
mod selection;
pub(crate) use selection::UnstartedTrial;
mod transaction;
mod trust;
mod updater_package;
pub use bundle::{BundleSource, ProvisioningWorker, VerifiedBundle};
pub use selection::{
    AcceptedApplication, InheritedSelection, InstallationLaunch, InstallationRun,
    LaunchInheritance, OwnedTrial, ReservedApplication, ReservedLaunch, SelectionFailure,
    SelectionStore, capture_launch_selection, reserve_installed_launch,
};
pub use selection::{InheritedLaunch, capture_launch};
pub use selection::{
    InheritedTrial, TRIAL_WITNESS_RESERVATION_BYTES, TrialApplication, TrialTransfer, TrialWitness,
    TrialWitnessTransfer, capture_trial_custody,
};
use std::io;
use sugarsubstitute_contracts::execution::JobContext;
pub use transaction::{
    AdmittedCandidate, InstallationSession, InstalledApplication, PreparedCandidate,
    PreparedInstallation, RuntimePaths, Selection,
};
pub(crate) use trust::offered::OfferInputs;
pub use trust::{
    AuthenticatedOffer, OFFER_LOOKUP_RESERVATION_BYTES, OFFER_PUBLICATION_RESERVATION_BYTES,
    OFFER_RESERVATION_BYTES,
};
pub use trust::{
    TRUST_OPERATION_RESERVATION_BYTES, TRUST_RESERVATION_BYTES, TrustFailure, TrustStore,
    UpdateAuthorization,
};
pub use updater_package::{
    PreparedUpdater, RejectedUpdaterProbe, UPDATER_STAGING_RESERVATION_BYTES,
    UnstartedUpdaterTrial, UpdaterPackage, UpdaterProbeTransfer,
};
mod signed_installer;
pub(crate) use signed_installer::CachedCandidate;
pub use signed_installer::{
    CompletedInstallation, InstalledCandidate, InstallerDistribution, InstallerSource,
    MAX_DISTRIBUTION_BYTES, SIGNED_INSTALLER_RESERVATION_BYTES,
    SIGNED_PREPARATION_RESERVATION_BYTES, SIGNED_TRANSFER_RESERVATION_BYTES, SignedCandidate,
    SignedInstallation, SignedPreparation,
};

/// Memory reserved for bounded manifests, streaming copies and retained filesystem custody.
pub const RESERVATION_BYTES: u64 = 16 * 1024 * 1024;

/// Selection publication against an already retained application: one MiB for native
/// protection plus bounded 16 KiB records and their encoded copies. Payload admission
/// and shell repair retain the separate `LAUNCH_RESERVATION_BYTES` allowance.
pub const SELECTION_PUBLICATION_RESERVATION_BYTES: u64 = 2 * 1024 * 1024;

fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Installation cancelled"));
    }
    if context.deadline().is_some_and(|until| context.now() >= until) {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Installation operation expired"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
pub(crate) mod test_package;
#[cfg(test)]
pub(crate) mod test_worker;
