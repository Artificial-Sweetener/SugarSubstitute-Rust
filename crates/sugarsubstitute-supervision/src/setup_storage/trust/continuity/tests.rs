// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native continuity survives missing mutable state and never grants authority to foreign markers.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
use sugarsubstitute_update::fixture::{key, root};

#[test]
fn opened_trust_retains_its_exact_bound_marker_against_replacement() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(60, context)?;
        let store = TrustStore::initialize(
            installation.path(),
            [60; 32],
            &root(&key(60)?, 1, None)?,
            context,
        )?;
        let marker = installation.path().join("state").join(MARKER);
        let bytes = std::fs::read(&marker)?;
        assert_eq!(
            envelope::decode(envelope::Kind::Continuity, &store.installation, &[60; 32], &bytes)?,
            CONTENT
        );
        assert!(std::fs::write(&marker, b"replace").is_err());
        assert!(std::fs::remove_file(&marker).is_err());
        drop(store);
        let reopened = TrustStore::open(installation.path(), [60; 32], context)?;
        assert_eq!(reopened.next_root_version(context)?, 2);
        assert!(std::fs::remove_file(&marker).is_err());
        assert_eq!(std::fs::read(marker)?, bytes);
        Ok(())
    })
}

#[test]
fn earlier_complete_trust_enables_continuity_without_rewriting_accepted_floors() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(61, context)?;
        let store = TrustStore::initialize(
            installation.path(),
            [60; 32],
            &root(&key(60)?, 1, None)?,
            context,
        )?;
        store.rotate(&root(&key(61)?, 2, Some(&key(60)?))?, context)?;
        let checkpoint = std::fs::read(store.root.path().join(RECORD))?;
        let history = std::fs::read(store.root.path().join(releases::RECORD))?;
        drop(store);
        std::fs::remove_file(installation.path().join("state").join(MARKER))?;
        let reopened = TrustStore::open(installation.path(), [60; 32], context)?;
        assert_eq!(reopened.next_root_version(context)?, 3);
        assert_eq!(std::fs::read(reopened.root.path().join(RECORD))?, checkpoint);
        assert_eq!(std::fs::read(reopened.root.path().join(releases::RECORD))?, history);
        assert!(installation.path().join("state").join(MARKER).exists());
        Ok(())
    })
}

#[test]
fn unprotected_corrupt_or_foreign_markers_are_preserved_and_never_replaced() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        for variant in 0..5 {
            let installation = installation(62 + variant, context)?;
            let initial_root = root(&key(60)?, 1, None)?;
            let store =
                TrustStore::initialize(installation.path(), [60; 32], &initial_root, context)?;
            let checkpoint = std::fs::read(store.root.path().join(RECORD))?;
            let identity = if variant == 4 {
                InstallationId::new("f".repeat(32))?
            } else {
                store.installation.clone()
            };
            let mut bytes = envelope::encode(
                envelope::Kind::Continuity,
                &identity,
                &if variant == 1 { [61; 32] } else { [60; 32] },
                if variant == 3 { b"unknown-namespace" } else { CONTENT },
            )?;
            if variant == 2 {
                bytes[7] = b'9';
            }
            drop(store);
            let marker = installation.path().join("state").join(MARKER);
            std::fs::remove_file(&marker)?;
            if variant == 0 {
                std::fs::write(&marker, &bytes)?;
            } else {
                protection::with(context, |owner| {
                    let mut file = owner.file(&marker, context)?;
                    file.write_all(&bytes)?;
                    file.sync_all()
                })?;
            }
            assert!(TrustStore::open(installation.path(), [60; 32], context).is_err());
            assert!(
                TrustStore::initialize(installation.path(), [60; 32], &initial_root, context)
                    .is_err()
            );
            assert_eq!(std::fs::read(marker)?, bytes);
            assert_eq!(
                std::fs::read(installation.path().join("state").join(DIRECTORY).join(RECORD))?,
                checkpoint
            );
        }
        Ok(())
    })
}

#[test]
fn incomplete_earlier_history_cannot_enable_continuity() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(67, context)?;
        let store = TrustStore::initialize(
            installation.path(),
            [60; 32],
            &root(&key(60)?, 1, None)?,
            context,
        )?;
        let checkpoint = std::fs::read(store.root.path().join(RECORD))?;
        drop(store);
        let marker = installation.path().join("state").join(MARKER);
        std::fs::remove_file(&marker)?;
        std::fs::remove_file(
            installation.path().join("state").join(DIRECTORY).join(releases::RECORD),
        )?;
        assert!(TrustStore::open(installation.path(), [60; 32], context).is_err());
        assert!(!marker.exists());
        assert_eq!(
            std::fs::read(installation.path().join("state").join(DIRECTORY).join(RECORD))?,
            checkpoint
        );
        Ok(())
    })
}

#[test]
fn interruption_before_marker_publication_reopens_complete_trust() -> TestResult {
    use sugarsubstitute_contracts::installation::InstallationRecord;
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().to_owned();
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES, move |context, clock| {
        ownership::establish(&crate::durable_storage::files::Directory::open(&path, false)?)?;
        SelectionStore::initialize(
            &path,
            &InstallationRecord::empty(InstallationId::new("6".repeat(32))?),
            context,
        )?;
        let observed = path.clone();
        clock.observe_checks(move || {
            observed.join("state").join(DIRECTORY).join(RECORD).exists()
                && !observed.join("state").join(MARKER).exists()
        })?;
        assert!(
            TrustStore::initialize(&path, [60; 32], &root(&key(60)?, 1, None)?, context).is_err()
        );
        assert!(path.join("state").join(DIRECTORY).join(RECORD).exists());
        assert!(path.join("state").join(DIRECTORY).join(releases::RECORD).exists());
        assert!(!path.join("state").join(MARKER).exists());
        Ok(())
    })?;
    let path = temporary.path().to_owned();
    worker::run_reserved(TRUST_RESERVATION_BYTES, move |context, _| {
        let checkpoint = std::fs::read(path.join("state").join(DIRECTORY).join(RECORD))?;
        let store = TrustStore::open(&path, [60; 32], context)?;
        assert_eq!(store.next_root_version(context)?, 2);
        assert!(path.join("state").join(MARKER).exists());
        assert_eq!(std::fs::read(path.join("state").join(DIRECTORY).join(RECORD))?, checkpoint);
        Ok(())
    })
}

#[test]
fn occupied_continuity_marker_alone_blocks_bootstrap_without_changing_unknown_bytes() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(68, context)?;
        let marker = installation.path().join("state").join(MARKER);
        std::fs::write(&marker, b"unknown preserved state")?;
        assert!(
            TrustStore::initialize(
                installation.path(),
                [60; 32],
                &root(&key(60)?, 1, None)?,
                context
            )
            .is_err()
        );
        assert!(!installation.path().join("state").join(DIRECTORY).exists());
        assert_eq!(std::fs::read(marker)?, b"unknown preserved state");
        Ok(())
    })
}
