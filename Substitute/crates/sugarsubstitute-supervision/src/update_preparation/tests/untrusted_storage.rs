// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update storage another program made untrustworthy is set aside and set up again by the
//! next check, without ever accepting a release older than the one that runs.
use super::*;
use crate::durable_storage::protected::security::rewrite::{Rewrite, rewrite};

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
    while !matches!(
        worker.progress(),
        Progress::Decision(_) | Progress::Failed | Progress::Ready | Progress::Unavailable
    ) {
        submit(&mut pipeline, &mut worker)?;
        pipeline.observe(|| worker.poll())?;
    }
    assert_ne!(worker.progress(), Progress::Ready, "an older release was prepared");
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    // The old storage is kept, renamed, under user\ for diagnosis.
    let set_aside: Vec<_> = std::fs::read_dir(root.join("user").join(SET_ASIDE_FOLDER))?
        .collect::<std::io::Result<_>>()?;
    assert_eq!(set_aside.len(), 1);
    assert!(set_aside[0].path().join("update-trust").join("checkpoint").is_file());
    assert!(set_aside[0].path().join("update-trust-enabled").is_file());
    let installation = fixture;
    // Fresh storage this account owns opens, and its floor is the running release.
    let installation = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        drop(TrustStore::open(installation.installation.path(), [92; 32], &context)?);
        let accepted = SelectionStore::open(installation.installation.path(), &context)?
            .load(&context)?
            .ok_or("Selection missing")?;
        assert_eq!(accepted.accepted().ok_or("Nothing accepted")?.release.sequence(), 2);
        let older = installation.import(SignedPackage::new(&installation.archive, 1)?, &context);
        assert!(older.is_err(), "a release older than the running one was accepted");
        drop(installation.import(SignedPackage::new(&installation.archive, 3)?, &context)?);
        Ok(installation)
    })?;
    drop(signed);
    installation.preserved()
}
