// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A downloaded installer never expires: it refreshes online, or installs its exact included
//! release offline after the user's explicit choice, and refuses tampered bytes either way.
use super::*;
use crate::update_repository::InstallerMetadata;
use sugarsubstitute_update::fixture::{Set, key};

const DAY: i64 = 24 * 60 * 60;
/// Long after every compiled-in role expired.
const LATER: i64 = EXPIRES + 30 * DAY;

/// Freshly signed roles for `sequence`, valid at `LATER`, naming `package` as the release.
fn fresh(sequence: u64, package: &[u8]) -> Result<InstallerMetadata, Failure> {
    let digest = format!("{:x}", Sha256::digest(package));
    let descriptor = serde_json::to_vec(&serde_json::json!({
        "schema_version":1, "product":"sugarsubstitute", "channel":"release",
        "platform":"windows-x86_64", "component":"application", "sequence":sequence,
        "version":format!("{sequence}.0.0"), "package_sha256":digest, "package_bytes":package.len(),
        "package_schema":1, "updater_protocol":1, "minimum_updater_release":1,
    }))?;
    let name = SignedPackage::new(package, 1)?.request.target_name();
    let set = Set::targets(
        &key(92)?,
        [2; 3],
        &[(name.as_str(), descriptor.as_slice()), (&format!("{digest}.ssupdate"), package)],
        LATER + DAY,
    )?;
    Ok(InstallerMetadata {
        roots: Vec::new(),
        timestamp: set.timestamp,
        snapshot: set.snapshot,
        targets: set.targets,
        descriptor,
    })
}

impl Fixture {
    fn import_choosing(
        &self,
        choice: InstallerMetadataChoice,
        context: &JobContext,
    ) -> Result<SignedInstallation, TrustFailure> {
        let distribution = InstallerDistribution::decode(
            &serde_json::to_vec(&self.configuration).map_err(io::Error::other)?,
            context,
        )?;
        distribution.import_with(
            InstallationSession::maintain(self.root.path(), context)?,
            &self.executable,
            BundleSource::Extract(self.workspace.path()),
            choice,
            || LATER,
            context,
        )
    }

    /// Complete preparation at `LATER` and return the prepared release's version label.
    fn complete(
        &self,
        imported: SignedInstallation,
        context: &JobContext,
    ) -> Result<String, Failure> {
        let prepared = imported.prepare(|| LATER, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        let candidate = prepared.complete(&runtime, || LATER, context)?;
        Ok(candidate.prepared().generation().release.version().to_owned())
    }

    /// No candidate was reserved and no generation allocated.
    fn untouched(&self, context: &JobContext) -> TestResult {
        let state =
            SelectionStore::open(self.root.path(), context)?.load(context)?.ok_or("No state")?;
        assert!(state.candidate().is_none());
        assert_eq!(std::fs::read_dir(self.root.path().join("releases"))?.count(), 1);
        Ok(())
    }
}

#[test]
fn expired_compiled_in_metadata_is_reported_as_expiry_and_reserves_nothing() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let distribution =
            InstallerDistribution::decode(&serde_json::to_vec(&fixture.configuration)?, context)?;
        assert!(!distribution.expired(NOW)?);
        assert!(distribution.expired(EXPIRES)?);
        let failure = fixture
            .import_choosing(InstallerMetadataChoice::Embedded, context)
            .err()
            .ok_or("Expired installer metadata was accepted")?;
        assert!(failure.is_expiry(), "{failure}");
        fixture.untouched(context)
    })
}

#[test]
fn expired_installer_with_a_connection_installs_under_refreshed_metadata() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let imported = fixture.import_choosing(
            InstallerMetadataChoice::Refreshed(fresh(1, &fixture.archive)?),
            context,
        )?;
        assert_eq!(fixture.complete(imported, context)?, "1.0.0");
        Ok(())
    })
}

#[test]
fn refreshed_metadata_naming_a_newer_release_installs_the_included_one() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let imported = fixture.import_choosing(
            InstallerMetadataChoice::Refreshed(fresh(2, b"a newer published package")?),
            context,
        )?;
        assert_eq!(fixture.complete(imported, context)?, "1.0.0");
        Ok(())
    })
}

#[test]
fn refreshed_metadata_that_withdraws_the_release_is_refused() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let mut withdrawn = fresh(1, b"a different package at the same sequence")?;
        assert!(
            fixture
                .import_choosing(InstallerMetadataChoice::Refreshed(withdrawn), context)
                .is_err()
        );
        withdrawn = fresh(2, &fixture.archive)?;
        withdrawn.targets[10] ^= 1;
        assert!(
            fixture
                .import_choosing(InstallerMetadataChoice::Refreshed(withdrawn), context)
                .is_err()
        );
        fixture.untouched(context)
    })
}

#[test]
fn expired_installer_offline_installs_its_included_release_after_the_users_choice() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let imported = fixture.import_choosing(
            InstallerMetadataChoice::Included(IncludedInstallChoice::confirmed_by_user()),
            context,
        )?;
        assert_eq!(fixture.complete(imported, context)?, "1.0.0");
        Ok(())
    })
}

#[test]
fn tampered_included_bytes_are_refused_offline_and_online() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let mut bytes = std::fs::read(&fixture.executable)?;
        // Flip one package byte inside the signed archive interval, after the inert prefix.
        let inside = b"inert installer prefix".len() + fixture.archive.len() / 2;
        bytes[inside] ^= 0x40;
        std::fs::write(&fixture.executable, &bytes)?;
        assert!(
            fixture
                .import_choosing(
                    InstallerMetadataChoice::Included(IncludedInstallChoice::confirmed_by_user()),
                    context,
                )
                .is_err()
        );
        assert!(
            fixture
                .import_choosing(
                    InstallerMetadataChoice::Refreshed(fresh(1, &fixture.archive)?),
                    context,
                )
                .is_err()
        );
        fixture.untouched(context)
    })
}
