// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A retained candidate must revalidate authority when retrying a deferred launch.
use super::*;

#[test]
fn retry_after_busy_launch_rechecks_expiry_and_current_signing_keys() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        for rotate in [false, true] {
            let fixture = Fixture::new(context)?;
            let prepared = fixture
                .import(SignedPackage::new(&fixture.archive, 1)?, context)?
                .prepare(|| NOW, context)?;
            let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
            let candidate = prepared.complete(&runtime, || NOW, context)?;
            let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
            let running = InstallationRun::acquire(store.clone(), context)?;
            let before = store.load(context)?;
            assert!(candidate.transfer(|| NOW, context, &mut None).is_err());
            drop(running);
            if rotate {
                let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
                trust.rotate(&root(&key(93)?, 2, Some(&key(92)?))?, context)?;
            }
            assert!(
                candidate
                    .transfer(|| if rotate { NOW } else { EXPIRES }, context, &mut None)
                    .is_err()
            );
            assert_eq!(store.load(context)?, before);
            let _available = InstallationRun::acquire(store, context)?;
            fixture.preserved()?;
        }
        Ok(())
    })
}
