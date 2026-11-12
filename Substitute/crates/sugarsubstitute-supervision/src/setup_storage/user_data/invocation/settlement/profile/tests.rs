// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise source-host copy/profile scope through actual released native profile records.
use super::*;
use crate::{
    durable_storage::files,
    profiles::{locations, record},
    setup_storage::test_worker::{self, TestResult},
};
use std::io::Write;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, RunGeneration, RunKey,
};

struct Fixture {
    _temporary: tempfile::TempDir,
    host: VerifiedHost,
    paths: StartupPaths,
    row: record::Record,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path();
        let image = root.join("SugarSubstitute.exe");
        std::fs::write(&image, b"inert actual native image fixture")?;
        let host = VerifiedHost::fixture(&image, context)?;
        let profile = root.join("desktop-data/profile");
        let row = protection::with(context, |owner| {
            owner.directory(&root.join("desktop-data"), Mode::CreateNew, context)?;
            owner.directory(&profile, Mode::CreateNew, context)?;
            let directory = crate::durable_storage::files::Directory::open(&profile, false)?;
            let row = record::Record {
                account: record::account_identity(owner.account_sid()),
                folder: directory.native_identity()?,
                run: RunKey {
                    owner: CoordinatorKey {
                        namespace: host.producer.namespace(),
                        epoch: CoordinatorEpoch::new([3; 16]).map_err(io::Error::other)?,
                    },
                    instance: InstanceId::new(1).map_err(io::Error::other)?,
                    generation: RunGeneration::new(1).map_err(io::Error::other)?,
                },
                state: record::State::Released,
            };
            for name in [locations::CLAIM, locations::CUSTODY] {
                let file = owner.file(&profile.join(name), context)?;
                files::sync(&file, context)?;
                owner.close_file(file)?;
            }
            let mut file = owner.file(&profile.join(locations::RECORD), context)?;
            file.write_all(&row.fixture_bytes())?;
            files::sync(&file, context)?;
            owner.close_file(file)?;
            Ok(row)
        })?;
        std::fs::write(profile.join("Preferences"), b"original exact Unicode profile preferences")?;
        let paths = StartupPaths::new([
            root.join("startup.toml"),
            root.join("frontend"),
            root.join("METADATA"),
            root.join("SugarCubes"),
            root.join("main.py"),
            profile,
        ])
        .map_err(io::Error::other)?;
        Ok(Self { _temporary: temporary, host, paths, row })
    }
    fn settle(&self, profile: &ProfileId, context: &JobContext) -> io::Result<PathBuf> {
        require_source_host(&self.host.root, context)?;
        let correlation = InstallationId::new("a".repeat(32)).map_err(io::Error::other)?;
        settle(
            &self.host,
            &self.paths,
            &correlation,
            profile,
            self.host.producer.namespace(),
            &mut |_, _| Ok(HistoricalArchiveDisposition::PreservedOriginal),
            context,
        )
    }
}

#[test]
fn source_profile_copy_is_restart_safe_preserves_originals_and_does_not_create_installation_selection()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let profile = ProfileId::new("real".into())?;
        let selected = fixture.settle(&profile, context)?;
        assert_eq!(selected, InstallationLayout::new(fixture.host.root.path()).webview_profile());
        assert_eq!(
            std::fs::read(selected.join("Preferences"))?,
            std::fs::read(fixture.paths.as_paths()[5].join("Preferences"))?
        );
        assert!(!selected.join(locations::RECORD).exists());
        assert_eq!(
            std::fs::read(fixture.paths.as_paths()[5].join(locations::RECORD))?,
            fixture.row.fixture_bytes()
        );
        std::fs::write(selected.join("Preferences"), b"later legitimate confined profile edit")?;
        let legacy =
            crate::durable_storage::files::Directory::open(fixture.paths.as_paths()[5], false)?;
        let held = legacy.open_existing(
            locations::CLAIM,
            files::ExistingAccess::ExclusiveRead,
            context,
        )?;
        assert_eq!(fixture.settle(&profile, context)?, selected);
        assert_eq!(
            std::fs::read(selected.join("Preferences"))?,
            b"later legitimate confined profile edit"
        );
        drop(held);
        assert!(fixture.settle(&ProfileId::new("Real".into())?, context).is_err());
        require_source_host(&fixture.host.root, context)?;
        assert!(!fixture.host.root.path().join("state").exists());
        Ok(())
    })
}

#[test]
fn fresh_source_admission_refuses_damaged_or_real_installed_selection_without_fallback()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        require_source_host(&fixture.host.root, context)?;
        assert!(!fixture.host.root.path().join("state").exists());
        std::fs::create_dir_all(fixture.host.root.path().join("state/selection"))?;
        std::fs::write(
            fixture.host.root.path().join("state/selection/unknown-original"),
            b"damaged namespace original",
        )?;
        assert!(require_source_host(&fixture.host.root, context).is_err());
        assert_eq!(
            std::fs::read(fixture.host.root.path().join("state/selection/unknown-original"))?,
            b"damaged namespace original"
        );
        let installed = test_worker::installation(0xa7, context)?;
        let installed = crate::durable_storage::files::Directory::open(installed.path(), false)?;
        assert!(require_source_host(&installed, context).is_err());
        Ok(())
    })
}

#[test]
fn foreign_released_source_namespace_refuses_before_copy_and_keeps_original_profile() -> TestResult
{
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let mut row = fixture.row;
        row.run.owner.namespace = NamespaceId::new([9; 32]);
        std::fs::write(fixture.paths.as_paths()[5].join(locations::RECORD), row.fixture_bytes())?;
        assert!(fixture.settle(&ProfileId::new("real".into())?, context).is_err());
        assert!(!InstallationLayout::new(fixture.host.root.path()).webview_profile().exists());
        assert_eq!(
            std::fs::read(fixture.paths.as_paths()[5].join(locations::RECORD))?,
            row.fixture_bytes()
        );
        Ok(())
    })
}

#[test]
fn held_original_profile_claim_cannot_be_imported_by_source_host() -> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let directory =
            crate::durable_storage::files::Directory::open(fixture.paths.as_paths()[5], false)?;
        let held = directory.open_existing(
            locations::CLAIM,
            files::ExistingAccess::ExclusiveRead,
            context,
        )?;
        assert!(fixture.settle(&ProfileId::new("real".into())?, context).is_err());
        assert!(!InstallationLayout::new(fixture.host.root.path()).webview_profile().exists());
        drop(held);
        fixture.settle(&ProfileId::new("real".into())?, context)?;
        Ok(())
    })
}
