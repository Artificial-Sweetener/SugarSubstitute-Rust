// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native handle duplicates preserve updater exclusion without retaining application ownership.
use super::*;
use crate::setup_storage::{
    self as storage,
    test_worker::{self as worker, TestResult},
};
use std::{io::Write, sync::Arc};
use sugarsubstitute_update::fixture;

/// Fund trust initialization and the independent application/setup ownership checks together.
fn run(body: impl FnOnce(&JobContext) -> TestResult + Send + 'static) -> TestResult {
    worker::run_reserved(
        TRUST_RESERVATION_BYTES + 2 * TRUST_OPERATION_RESERVATION_BYTES,
        move |context, _| body(context),
    )
}

/// Establish real protected trust; the rendezvous is opened by the production claim operation.
fn initialize(
    root: &Path,
    context: &JobContext,
) -> Result<TrustStore, Box<dyn std::error::Error + Send + Sync>> {
    let key = fixture::key(140)?;
    let signed_root = fixture::root(&key, 1, None)?;
    Ok(TrustStore::initialize(root, [140; 32], &signed_root, context)?)
}

#[test]
fn witness_duplicates_never_release_exclusion_and_dropping_the_owner_does() -> TestResult {
    run(|context| {
        let root = worker::installation(140, context)?;
        let trust = initialize(root.path(), context)?;
        let original = trust.claim_updater_preparation(context)?;
        let identity = crate::durable_storage::files::file_identity(original.witness())?;
        let witness = original.witness().try_clone()?;
        drop(original.witness().try_clone()?);
        drop(trust);
        let reopened = TrustStore::open(root.path(), [140; 32], context)?;
        assert!(reopened.claim_updater_preparation(context).is_err());
        let path = reopened.root.path().join(RENDEZVOUS);
        reopened.protected(context, |protection| {
            protection.verify_exclusive_rendezvous(&witness, &path, context)?;
            Ok(())
        })?;
        let selection = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let _application = storage::InstallationRun::acquire(selection, context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false, context)?;
        assert!(reopened.claim_updater_preparation(context).is_err());
        drop(original);
        assert!(
            reopened
                .protected(context, |protection| {
                    Ok(protection.verify_exclusive_rendezvous(&witness, &path, context)?)
                })
                .is_err()
        );
        let successor = reopened.claim_updater_preparation(context)?;
        assert_eq!(crate::durable_storage::files::file_identity(successor.witness())?, identity);
        assert_eq!(successor.witness().metadata()?.len(), 0);
        Ok(())
    })
}

#[test]
fn foreign_readers_never_block_preparation_and_whole_file_locks_still_conflict() -> TestResult {
    run(|context| {
        let root = worker::installation(141, context)?;
        let trust = initialize(root.path(), context)?;
        let path = trust.root.path().join(RENDEZVOUS);
        let legacy =
            trust.protected(context, |protection| Ok(protection.rendezvous(&path, context)?))?;
        legacy.try_lock().map_err(io::Error::from)?;
        assert!(trust.claim_updater_preparation(context).is_err());
        drop(legacy);
        // An indexer or antivirus scanner reading the file does not make preparation busy.
        let reader = std::fs::File::open(&path)?;
        let current = trust.claim_updater_preparation(context)?;
        assert!(trust.claim_updater_preparation(context).is_err());
        let whole =
            trust.protected(context, |protection| Ok(protection.rendezvous(&path, context)?))?;
        assert!(whole.try_lock().is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(reader);
        drop(current);
        whole.try_lock().map_err(io::Error::from)?;
        assert_eq!(whole.metadata()?.len(), 0);
        Ok(())
    })
}

#[test]
fn unknown_rendezvous_contents_are_preserved_and_do_not_claim_application_ownership() -> TestResult
{
    run(|context| {
        let root = worker::installation(142, context)?;
        let trust = initialize(root.path(), context)?;
        let path = trust.root.path().join(RENDEZVOUS);
        trust.protected(context, |protection| {
            let mut file = protection.file(&path, context)?;
            file.write_all(b"unknown retained data")?;
            file.sync_all()?;
            Ok(())
        })?;
        let checkpoint = std::fs::read(trust.root.path().join("checkpoint"))?;
        assert!(trust.claim_updater_preparation(context).is_err());
        assert_eq!(std::fs::read(&path)?, b"unknown retained data");
        assert_eq!(std::fs::read(trust.root.path().join("checkpoint"))?, checkpoint);
        let selection = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let _application = storage::InstallationRun::acquire(selection, context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false, context)?;
        Ok(())
    })
}
