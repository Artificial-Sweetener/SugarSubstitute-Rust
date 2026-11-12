// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise durable preferences through real protected storage and the shared execution owner.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
use sugarsubstitute_update::fixture::{key, root};

/// Create an installation-bound trust owner on the real storage worker.
fn fixture(
    context: &JobContext,
) -> Result<(tempfile::TempDir, TrustStore), Box<dyn std::error::Error + Send + Sync>> {
    let installation = installation(81, context)?;
    let store =
        TrustStore::initialize(installation.path(), [81; 32], &root(&key(81)?, 1, None)?, context)?;
    Ok((installation, store))
}

#[test]
fn subscription_survives_restart_and_stale_writes_including_round_trips_fail() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let (installation, store) = fixture(context)?;
        let initial = store.subscription(context)?;
        assert_eq!(initial, Subscription::default());
        let canary = store.select_subscription(initial, Channel::Canary, context)?;
        assert_eq!(store.select_subscription(canary, Channel::Canary, context)?, canary);
        assert!(store.select_subscription(initial, Channel::Release, context).is_err());
        drop(store);
        let reopened = TrustStore::open(installation.path(), [81; 32], context)?;
        assert_eq!(reopened.subscription(context)?, canary);
        let release = reopened.select_subscription(canary, Channel::Release, context)?;
        assert_eq!(release.revision(), 2);
        assert!(reopened.select_subscription(initial, Channel::Canary, context).is_err());
        assert_eq!(reopened.subscription(context)?, release);
        Ok(())
    })
}

#[test]
fn missing_established_preference_and_corrupt_or_foreign_bytes_are_preserved() -> TestResult {
    for mode in 0..4 {
        worker::run_reserved(TRUST_RESERVATION_BYTES, move |context, _| {
            let (_installation, store) = fixture(context)?;
            store.subscription(context)?;
            let path = store.root.path().join(RECORD);
            let bytes = match mode {
                0 => None,
                1 => Some(b"damaged preference".to_vec()),
                2 => Some(envelope::encode(
                    envelope::Kind::Subscription,
                    &store.installation,
                    &[82; 32],
                    &Subscription::default().encode()?,
                )?),
                _ => Some(envelope::encode(
                    envelope::Kind::Subscription,
                    &store.installation,
                    &store.repository,
                    br#"{"schema_version":2,"revision":0,"channel":"release"}"#,
                )?),
            };
            match &bytes {
                Some(bytes) => std::fs::write(&path, bytes)?,
                None => std::fs::remove_file(&path)?,
            }
            assert!(store.subscription(context).is_err());
            assert!(
                store
                    .select_subscription(Subscription::default(), Channel::Canary, context)
                    .is_err()
            );
            match bytes {
                Some(bytes) => assert_eq!(std::fs::read(path)?, bytes),
                None => assert!(!path.exists()),
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn interrupted_initialization_preserves_completed_preference_and_unknown_staging() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let (_installation, store) = fixture(context)?;
        let initial = store.subscription(context)?;
        let canary = store.select_subscription(initial, Channel::Canary, context)?;
        std::fs::remove_file(store.root.path().join(MARKER))?;
        let orphan = store.root.path().join("subscription-pending-interrupted");
        std::fs::write(&orphan, b"partial")?;
        assert_eq!(store.subscription(context)?, canary);
        assert_eq!(std::fs::read(orphan)?, b"partial");
        assert!(store.root.path().join(MARKER).exists());
        Ok(())
    })
}

/// A held trust writer makes a preference write wait in the kernel: it times out at the bound,
/// and once the holder releases the same write succeeds. The application run stays available.
#[test]
fn preference_writes_wait_for_the_trust_writer_but_leave_application_run_available() -> TestResult {
    use crate::setup_storage::{short_lock::WAIT, test_worker::driven::run_driven};
    use std::os::windows::fs::OpenOptionsExt;
    use sugarsubstitute_contracts::execution::Moment;
    let lock = std::sync::Arc::new(std::sync::Mutex::new(None));
    let worker_lock = lock.clone();
    run_driven(
        TRUST_RESERVATION_BYTES,
        move |context, handoff| {
            let (installation, store) = fixture(context)?;
            let initial = store.subscription(context)?;
            *crate::registry::lock(&worker_lock) = Some(store.root.path().join(LOCK));
            handoff.reach(1);
            handoff.wait(2);
            assert!(matches!(
                store.select_subscription(initial, Channel::Canary, context),
                Err(TrustFailure::LockTimeout)
            ));
            handoff.reach(3);
            handoff.wait(4);
            store.select_subscription(initial, Channel::Canary, context)?;
            let selection =
                std::sync::Arc::new(SelectionStore::open(installation.path(), context)?);
            let run = crate::setup_storage::InstallationRun::acquire(selection, context)?;
            assert_eq!(store.subscription(context)?.channel(), Channel::Canary);
            drop(run);
            Ok(())
        },
        |driver| {
            driver.handoff.wait(1);
            let path = crate::registry::lock(&lock).take().ok_or("Writer path absent")?;
            let holder = std::fs::OpenOptions::new().read(true).share_mode(0x1).open(path)?;
            holder.try_lock()?;
            driver.handoff.reach(2);
            let bound = Moment::from_nanos(0).checked_add(WAIT)?;
            driver.clock.wait_for_sleeper(bound);
            driver.clock.advance_to(bound)?;
            driver.handoff.wait(3);
            drop(holder);
            driver.handoff.reach(4);
            Ok(())
        },
    )
}
