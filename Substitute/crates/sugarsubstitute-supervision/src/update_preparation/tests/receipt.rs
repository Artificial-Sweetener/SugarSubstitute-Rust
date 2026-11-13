// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A prepared update starts offline on its receipt once cached metadata expired, and only then.
use super::restart::prepared;
use super::*;
use sugarsubstitute_contracts::installation::Candidate;
use sugarsubstitute_update::{RECEIPT_LIFETIME_SECONDS, ReceiptRefusal, fixture::EXPIRES};

const DAY: i64 = 24 * 60 * 60;

/// Run a cached startup at `now` until it reports a typed failure; nothing may be published.
fn refused(fixture: &Fixture, now: i64) -> Result<Arc<crate::update_repository::Failure>, Failure> {
    let selection = fixture.installation.path().join("state/selection/.substitute-selection.json");
    let before = std::fs::read(&selection)?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(cached_configuration(fixture))?;
    for expected in [Progress::Restoring, Progress::Reopening] {
        assert_eq!(step_at(&mut pipeline, &mut worker, now)?.progress, expected);
    }
    submit_at(&mut pipeline, &mut worker, now)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(worker.take_candidate().is_err());
    assert_eq!(std::fs::read(&selection)?, before);
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, fixture)?;
    observed.failure.ok_or_else(|| "Refusal cause absent".into())
}

/// The failure chain names exactly this receipt condition.
fn names(failure: &crate::update_repository::Failure, expected: ReceiptRefusal) -> bool {
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(failure);
    while let Some(current) = cause {
        if current.downcast_ref::<ReceiptRefusal>() == Some(&expected) {
            return true;
        }
        cause = current.source();
    }
    false
}

#[test]
fn offline_restart_starts_the_prepared_update_on_its_receipt_after_metadata_expired() -> TestResult
{
    let (fixture, original) = prepared()?;
    assert!(original.join(crate::setup_storage::UPDATE_RECEIPT_FILE).is_file());
    let later = EXPIRES + 30 * DAY;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(cached_configuration(&fixture))?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step_at(&mut pipeline, &mut worker, later)?.progress, expected);
    }
    let candidate = worker.take_candidate()?;
    assert_eq!(candidate.directory(), original);
    let root = fixture.installation.path().to_owned();
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        let transfer = candidate.transfer(move || later, &context, &mut None)?;
        assert_eq!(transfer.directory(), original);
        let store = SelectionStore::open(&root, &context)?;
        assert!(matches!(
            store.load(&context)?.ok_or("Selection absent")?.candidate(),
            Some(Candidate::Trial { .. })
        ));
        drop(transfer);
        drop(candidate);
        Ok(())
    })?;
    fixture.preserved()
}

#[test]
fn a_receipt_older_than_ninety_days_starts_the_accepted_version() -> TestResult {
    let (fixture, _) = prepared()?;
    let failure = refused(&fixture, NOW + RECEIPT_LIFETIME_SECONDS + 1)?;
    assert!(names(&failure, ReceiptRefusal::Aged));
    fixture.preserved()
}

#[test]
fn a_root_rotation_after_preparation_starts_the_accepted_version() -> TestResult {
    let (fixture, _) = prepared()?;
    let root = fixture.installation.path().to_owned();
    let mut pipeline = Pipeline::new()?;
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let trust = TrustStore::open(&root, [92; 32], &context)?;
        let signing = sugarsubstitute_update::fixture::key(92)?;
        trust.rotate(
            &sugarsubstitute_update::fixture::root(&signing, 2, Some(&signing))?,
            &context,
        )?;
        Ok(())
    })?;
    drop(pipeline);
    let failure = refused(&fixture, EXPIRES + DAY)?;
    assert!(names(&failure, ReceiptRefusal::RootAdvanced));
    fixture.preserved()
}

#[test]
fn a_damaged_receipt_is_refused_rather_than_ignored() -> TestResult {
    let (fixture, original) = prepared()?;
    std::fs::write(original.join(crate::setup_storage::UPDATE_RECEIPT_FILE), b"damaged")?;
    let failure = refused(&fixture, EXPIRES + DAY)?;
    assert!(!names(&failure, ReceiptRefusal::Aged));
    fixture.preserved()
}
