// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update storage another program made untrustworthy is set aside and set up again by the
//! next check, without ever accepting a release older than the one that runs.
use super::*;
use crate::durable_storage::protected::{
    directories::Mode,
    security::rewrite::{Rewrite, rewrite},
};

/// Accept release `sequence` through the real installer pipeline and native run owner.
fn accept(pipeline: &mut Pipeline, fixture: Fixture, sequence: u32) -> Result<Fixture, Failure> {
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, sequence)?, &context)?
            .prepare(|| NOW, &context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime missing")?.clone();
        drop(prepared.complete(&runtime, || NOW, &context)?);
        crate::setup_storage::test_package::maintenance::accept_pending(
            fixture.installation.path(),
            &context,
        )?;
        Ok(fixture)
    })
}

#[test]
fn a_check_sets_untrusted_update_storage_aside_and_keeps_refusing_older_releases() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (mut fixture, signed) = initialized(&mut pipeline)?;
    for sequence in [1, 2] {
        fixture = accept(&mut pipeline, fixture, sequence)?;
    }
    let root = fixture.installation.path().to_owned();
    // Another program lets every user of the machine change the trust checkpoint.
    rewrite(&root.join("state/update-trust/checkpoint"), Rewrite::UsersMayWrite)?;
    let (mut worker, server) =
        campaign(&mut pipeline, &fixture, SignedPackage::new(&fixture.archive, 1)?, false)?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Checking);
    assert!(worker.storage_set_aside());
    let mut failure = None;
    while !matches!(
        worker.progress(),
        Progress::Decision(_) | Progress::Failed | Progress::Ready | Progress::Unavailable
    ) {
        submit(&mut pipeline, &mut worker)?;
        failure = pipeline.observe(|| worker.poll())?.failure;
    }
    assert_eq!(worker.progress(), Progress::Failed, "an older release was prepared");
    // The fresh trust carries the set-aside trust's metadata floors, so the older release's
    // metadata is refused before anything else is requested.
    let failure = failure.ok_or("The refusal has no cause")?;
    assert!(format!("{failure:?}").contains("Rollback"), "refused for another cause: {failure:?}");
    server.stop()?;
    retire(&mut pipeline, &mut worker)?;
    // The old storage is kept, renamed, under user\ for diagnosis.
    let set_aside: Vec<_> = std::fs::read_dir(root.join("user").join(SET_ASIDE_FOLDER))?
        .collect::<std::io::Result<_>>()?;
    assert_eq!(set_aside.len(), 1);
    assert!(set_aside[0].path().join("update-trust").join("checkpoint").is_file());
    assert!(set_aside[0].path().join("update-trust-enabled").is_file());
    // The version floors kept outside the trust folder stay for the fresh trust.
    assert!(root.join("state").join("update-floors").is_file(), "the floors were not kept");
    let installation = fixture;
    let set_aside = set_aside[0].path();
    // Fresh storage this account owns opens, and its floor is the running release.
    let installation = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        // Set-aside storage keeps the protection of the trust storage it holds.
        crate::durable_storage::protection::with(&context, |protection| {
            for folder in
                [set_aside.parent().ok_or_else(|| std::io::Error::other("parent"))?, &set_aside]
            {
                protection.directory(folder, Mode::Existing, &context)?;
            }
            Ok(())
        })?;
        drop(TrustStore::open(installation.installation.path(), [92; 32], &context)?);
        let accepted = SelectionStore::open(installation.installation.path(), &context)?
            .load(&context)?
            .ok_or("Selection missing")?;
        assert_eq!(accepted.accepted().ok_or("Nothing accepted")?.release.sequence(), 2);
        let older = installation.import(SignedPackage::new(&installation.archive, 1)?, &context);
        assert!(older.is_err(), "a release older than the running one was accepted");
        // The running release's own archive is still admitted at its sequence.
        drop(installation.import(SignedPackage::new(&installation.archive, 2)?, &context)?);
        drop(installation.import(SignedPackage::new(&installation.archive, 3)?, &context)?);
        Ok(installation)
    })?;
    drop(signed);
    installation.preserved()
}

#[test]
fn a_check_protects_storage_that_only_inherits_again_instead_of_setting_it_aside() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (mut fixture, signed) = initialized(&mut pipeline)?;
    fixture = accept(&mut pipeline, fixture, 1)?;
    let root = fixture.installation.path().to_owned();
    // A tool keeps the account's and SYSTEM's grants but lets the checkpoint inherit again.
    rewrite(&root.join("state/update-trust/checkpoint"), Rewrite::InheritsAgain)?;
    let (mut worker, server) =
        campaign(&mut pipeline, &fixture, SignedPackage::new(&fixture.archive, 2)?, false)?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Checking);
    assert!(!worker.storage_set_aside(), "harmless storage was set aside");
    while !matches!(
        worker.progress(),
        Progress::Decision(_) | Progress::Failed | Progress::Ready | Progress::Unavailable
    ) {
        submit(&mut pipeline, &mut worker)?;
        pipeline.observe(|| worker.poll())?;
    }
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    assert!(!root.join("user").join(SET_ASIDE_FOLDER).exists());
    let installation = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        // The same storage opens again, protected as before.
        drop(TrustStore::open(fixture.installation.path(), [92; 32], &context)?);
        Ok(fixture)
    })?;
    drop(signed);
    installation.preserved()
}

#[test]
fn a_reset_that_finds_the_storage_trusted_under_the_writer_keeps_it() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let fixture = accept(&mut pipeline, fixture, 1)?;
    let root = signed.root.clone();
    let installation = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        // Another check set the storage up again while this one waited for the writer.
        let (trust, set_aside) =
            TrustStore::reset_untrusted(fixture.installation.path(), [92; 32], &root, &context)?;
        assert!(!set_aside, "trusted storage was set aside");
        drop(trust);
        Ok(fixture)
    })?;
    assert!(!installation.installation.path().join("user").join(SET_ASIDE_FOLDER).exists());
    drop(signed);
    installation.preserved()
}

#[test]
fn a_check_finishes_a_reset_interrupted_after_it_set_the_storage_aside() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let fixture = accept(&mut pipeline, fixture, 1)?;
    let root = fixture.installation.path().to_owned();
    rewrite(&root.join("state/update-trust/checkpoint"), Rewrite::UsersMayWrite)?;
    let trusted = signed.root.clone();
    let fixture = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        INTERRUPT_AFTER_SET_ASIDE.set(true);
        let interrupted =
            TrustStore::reset_untrusted(fixture.installation.path(), [92; 32], &trusted, &context);
        INTERRUPT_AFTER_SET_ASIDE.set(false);
        assert!(interrupted.is_err(), "the reset was not interrupted");
        Ok(fixture)
    })?;
    assert!(!root.join("state/update-trust").exists(), "the storage was not set aside");
    // The next check finishes the reset instead of finding no trust at all.
    let (mut worker, server) =
        campaign(&mut pipeline, &fixture, SignedPackage::new(&fixture.archive, 2)?, false)?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Checking);
    assert!(worker.storage_set_aside(), "the check did not finish the reset");
    while !matches!(
        worker.progress(),
        Progress::Decision(_) | Progress::Failed | Progress::Ready | Progress::Unavailable
    ) {
        submit(&mut pipeline, &mut worker)?;
        pipeline.observe(|| worker.poll())?;
    }
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    let installation = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        drop(TrustStore::open(fixture.installation.path(), [92; 32], &context)?);
        Ok(fixture)
    })?;
    drop(signed);
    installation.preserved()
}
