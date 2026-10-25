// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Local signed packages enter the same preservation, release-history and trial transaction.
use super::*;
use crate::setup_storage::{InstallationRun, SelectionStore};

/// Delivery changes the external bytes only; every route uses repository authentication.
pub(super) enum Delivery {
    Network,
    File,
    Installer,
}
impl Delivery {
    /// Resolve fixture delivery while retaining the real package cache and publication owners.
    pub(super) fn receive(
        self,
        workspace: &Path,
        archive: &[u8],
        installation: &Path,
        target: sugarsubstitute_update::Target,
        context: &JobContext,
    ) -> Result<VerifiedDownload, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Self::File => {
                let source = workspace.join("offline.ssupdate");
                std::fs::write(&source, archive)?;
                Ok(Download::import_file(
                    crate::setup_storage::cache::RuntimeCache::open(
                        installation,
                        crate::setup_storage::cache::CachePolicy::default(),
                        context,
                    )?,
                    target,
                    &source,
                    || NOW,
                    context,
                )?)
            }
            Self::Installer => {
                let source = workspace.join("setup.exe");
                let bytes = [b"inert installer prefix".as_slice(), archive].concat();
                std::fs::write(&source, &bytes)?;
                let download = Download::import_installer(
                    crate::setup_storage::cache::RuntimeCache::open(
                        installation,
                        crate::setup_storage::cache::CachePolicy::default(),
                        context,
                    )?,
                    target,
                    &source,
                    || NOW,
                    context,
                )?;
                assert_eq!(std::fs::read(source)?, bytes);
                Ok(download)
            }
            Self::Network => super::receive(archive, installation, target, context),
        }
    }
}

#[test]
fn offline_distribution_prepares_and_accepts_through_shared_trial_ownership() -> TestResult {
    accepts(Delivery::File)
}

#[test]
fn embedded_distribution_prepares_and_accepts_through_shared_trial_ownership() -> TestResult {
    accepts(Delivery::Installer)
}

fn accepts(delivery: Delivery) -> TestResult {
    run_source(2, true, test_package::distribution()?, delivery, |case| {
        let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
        let before = store.load(case.context)?.ok_or("Selection missing")?;
        let document = case.directory.path().join("user-document");
        std::fs::write(&document, b"user authored")?;
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_eq!(
            store.load(case.context)?.ok_or("Prepared state missing")?.accepted(),
            before.accepted()
        );
        case.preserved()?;
        let mut run = InstallationRun::acquire(store.clone(), case.context)?;
        let admitted = candidate.begin_trial(
            &mut run,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_eq!(store.load(case.context)?.ok_or("Trial missing")?.accepted(), before.accepted());
        // This supplies health at the storage boundary; the inert fixture is never executed.
        let accepted = run.accept(admitted.trial(), case.context)?;
        assert_eq!(accepted.accepted(), Some(&admitted.trial().attempt().generation));
        assert!(accepted.candidate().is_none());
        assert_eq!(std::fs::read(document)?, b"user authored");
        case.preserved()
    })
}

#[test]
fn offline_source_cannot_bypass_current_signing_keys_at_trial_admission() -> TestResult {
    revokes(Delivery::File)
}

#[test]
fn embedded_source_cannot_bypass_current_signing_keys_at_trial_admission() -> TestResult {
    revokes(Delivery::Installer)
}

fn revokes(delivery: Delivery) -> TestResult {
    run_source(2, true, test_package::distribution()?, delivery, |case| {
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
        let before = store.load(case.context)?;
        case.trust.rotate(&root(&key(94)?, 2, Some(&key(92)?))?, case.context)?;
        let mut run = InstallationRun::acquire(store.clone(), case.context)?;
        assert!(candidate.begin_trial(
            &mut run, &case.signed.authorization(case.trust), || NOW, case.context,
        ).is_err());
        assert_eq!(store.load(case.context)?, before);
        case.preserved()
    })
}
