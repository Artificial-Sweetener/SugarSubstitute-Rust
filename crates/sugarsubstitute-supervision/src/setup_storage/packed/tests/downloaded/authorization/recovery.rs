// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drop actual preparation custody and reopen durable candidates with the same authenticated cache.
use super::*;
use crate::setup_storage::{PreparedCandidate, SelectionStore};
use sugarsubstitute_contracts::installation::Candidate;

struct Recovery<'a> {
    directory: &'a Directory,
    original: &'a Path,
    bundle: Arc<VerifiedBundle>,
    signed: &'a SignedPackage,
    trust: &'a TrustStore,
    context: &'a JobContext,
}
impl Recovery<'_> {
    fn resume(&self) -> io::Result<PreparedInstallation> {
        crate::setup_storage::InstallationSession::maintain(&self.directory.path, self.context)?
            .resume_update(self.bundle.clone(), self.context)
            .map_err(|cause| {
                let chain =
                    std::iter::successors(Some(&cause as &dyn std::error::Error), |error| {
                        error.source()
                    })
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; caused by: ");
                io::Error::other(chain)
            })
    }
    fn finish(
        &self,
        preparation: &PreparedInstallation,
    ) -> Result<Arc<PreparedCandidate>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(preparation.prepare_update(
            preparation.maintenance_runtime().ok_or("Missing runtime")?,
            &self.signed.authorization(self.trust),
            || NOW,
            self.context,
        )?)
    }
    fn preserved(&self) -> TestResult {
        assert_eq!(entrypoint::selected(self.directory)?.ok_or("Shell lost")?.path, self.original);
        assert!(!self.directory.path.join("external-comfy").exists());
        Ok(())
    }
}

fn recover(body: impl FnOnce(Recovery<'_>, PathBuf) -> TestResult + Send + 'static) -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, move |context, _| {
        let temporary = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let original = fixture::installed(&directory, context)?;
        let (manifest, compressed) = test_package::distribution()?;
        let archive = test_package::seal(&manifest, &compressed);
        let signed = SignedPackage::new(&archive, 1)?;
        let trust = signed.initialize(&directory.path, context)?;
        let target = trust
            .offer(signed.metadata.metadata(), &signed.request, &signed.descriptor, NOW, context)?
            .into_package()?;
        let bundle = Arc::new(VerifiedBundle::from_download(
            receive(&archive, &directory.path, target, context)?,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            context,
        )?);
        let prepared =
            crate::setup_storage::InstallationSession::maintain(&directory.path, context)?
                .prepare(bundle.clone(), context)?;
        let mut checks = 0;
        assert!(
            prepared
                .prepare_update(
                    prepared.maintenance_runtime().ok_or("Missing runtime")?,
                    &signed.authorization(&trust),
                    || {
                        checks += 1;
                        if checks == 3 { EXPIRES } else { NOW }
                    },
                    context,
                )
                .is_err()
        );
        let state = SelectionStore::open(&directory.path, context)?
            .load(context)?
            .ok_or("Missing staging")?;
        let Some(Candidate::Staging { generation }) = state.candidate() else {
            return Err("No interrupted candidate".into());
        };
        let path = directory.path.join("releases").join(generation.id.as_str());
        assert!(
            crate::setup_storage::InstallationSession::maintain(&directory.path, context).is_err()
        );
        drop(prepared);
        body(
            Recovery {
                directory: &directory,
                original: &original,
                bundle,
                signed: &signed,
                trust: &trust,
                context,
            },
            path,
        )
    })
}

#[test]
fn interrupted_complete_candidate_reopens_and_preparation_is_idempotent() -> TestResult {
    recover(|case, path| {
        let store = SelectionStore::open(&case.directory.path, case.context)?;
        let before = store.load(case.context)?.ok_or("Missing staging")?;
        let resumed = case.resume()?;
        let candidate = case.finish(&resumed)?;
        assert_eq!(candidate.directory(), path);
        let prepared = store.load(case.context)?.ok_or("Missing prepared candidate")?;
        assert_eq!(prepared.revision(), before.revision() + 1);
        drop(candidate);
        drop(resumed);
        let again = case.resume()?;
        let repeated = case.finish(&again)?;
        assert_eq!(repeated.directory(), path);
        assert_eq!(store.load(case.context)?, Some(prepared));
        assert_eq!(
            std::fs::read(path.join("startup.local.toml"))?,
            std::fs::read(case.original.join("startup.local.toml"))?
        );
        case.preserved()
    })
}

#[test]
fn missing_final_files_resume_and_abandoned_temporary_bytes_are_preserved() -> TestResult {
    recover(|case, path| {
        for leaf in ["sugarsubstitute.exe", "startup.local.toml", "installation-ready.json"] {
            std::fs::remove_file(path.join(leaf))?;
        }
        let orphan = path.join(".payload-stage-interrupted");
        std::fs::write(&orphan, b"partial output before shutdown")?;
        let resumed = case.resume()?;
        let candidate = case.finish(&resumed)?;
        assert_eq!(candidate.directory(), path);
        assert!(path.join("sugarsubstitute.exe").is_file());
        assert!(path.join("installation-ready.json").is_file());
        assert_eq!(std::fs::read(orphan)?, b"partial output before shutdown");
        case.preserved()
    })
}

#[test]
fn edited_candidate_configuration_is_preserved_and_cannot_be_completed() -> TestResult {
    recover(|case, path| {
        let config = path.join("startup.local.toml");
        std::fs::write(&config, b"user edited candidate")?;
        let store = SelectionStore::open(&case.directory.path, case.context)?;
        let before = store.load(case.context)?;
        let resumed = case.resume()?;
        assert!(case.finish(&resumed).is_err());
        assert_eq!(std::fs::read(config)?, b"user edited candidate");
        assert_eq!(store.load(case.context)?, before);
        case.preserved()
    })
}

#[test]
fn damaged_generation_record_is_preserved_without_directory_fallback() -> TestResult {
    recover(|case, path| {
        let marker = path.join("setup-generation.json");
        std::fs::write(&marker, b"unrecognized generation")?;
        assert!(case.resume().is_err());
        assert_eq!(std::fs::read(marker)?, b"unrecognized generation");
        case.preserved()
    })
}
