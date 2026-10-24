// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual execution capacity remains shared across signed installer phase transitions.
use super::*;
use crate::setup_storage::test_worker::pipeline::Pipeline;

#[test]
fn successive_signed_phases_reuse_funded_trust_workspace() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, imported) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        let imported = fixture.import(SignedPackage::new(&fixture.archive, 1)?, &context)?;
        Ok((fixture, imported))
    })?;
    // The actual ledger must reject a second peak reservation while the first result owns it.
    let rejected = pipeline
        .run_once(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))
        .err()
        .ok_or("Duplicate peak reservation was admitted")?;
    assert!(matches!(
        rejected.downcast_ref::<sugarsubstitute_contracts::execution::AdmissionError>(),
        Some(sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated { .. })
    ));
    let prepared = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        Ok(imported.prepare(|| NOW, &context)?)
    })?;
    let prepared = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        prepared.stage(&context)?;
        Ok(prepared)
    })?;
    let candidate = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        Ok(prepared.complete(&runtime, || NOW, &context)?)
    })?;
    let transfer = pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        Ok(candidate.transfer(|| NOW, &context, &mut None)?)
    })?;
    assert!(transfer.executable().is_file());
    fixture.preserved()?;
    drop(transfer);
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}

#[test]
fn retained_workspace_does_not_reuse_the_import_jobs_cancellation_or_deadline() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, imported) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        let imported = fixture.import(SignedPackage::new(&fixture.archive, 1)?, &context)?;
        Ok((fixture, imported))
    })?;
    let selection = fixture.installation.path().join(".selection-state/.substitute-selection.json");
    let before = std::fs::read(&selection)?;
    worker::run_reserved_expiring(SIGNED_PREPARATION_RESERVATION_BYTES, move |context, clock| {
        // The first execution owner's clock remains live at zero; only this new job expires.
        clock.expire();
        assert!(imported.prepare(|| NOW, context).is_err());
        Ok(())
    })?;
    assert_eq!(std::fs::read(selection)?, before);
    fixture.preserved()?;
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}

#[test]
fn rejected_execution_admission_retains_prepared_custody_for_a_later_launch() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, candidate) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
            .prepare(|| NOW, &context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        let candidate = Arc::new(prepared.complete(&runtime, || NOW, &context)?);
        Ok((fixture, candidate))
    })?;
    let selection = fixture.installation.path().join(".selection-state/.substitute-selection.json");
    let before = std::fs::read(&selection)?;
    let pending = Arc::clone(&candidate);
    let rejected = pipeline
        .run_once(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
            Ok(pending.transfer(|| NOW, &context, &mut None)?)
        })
        .err()
        .ok_or("Duplicate peak reservation was admitted")?;
    assert!(matches!(
        rejected.downcast_ref::<sugarsubstitute_contracts::execution::AdmissionError>(),
        Some(sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated { .. })
    ));
    assert_eq!(std::fs::read(&selection)?, before);
    assert!(std::fs::write(candidate.directory().join("sugarsubstitute.exe"), b"changed").is_err());
    let pending = Arc::clone(&candidate);
    let transfer = pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        Ok(pending.transfer(|| NOW, &context, &mut None)?)
    })?;
    assert_eq!(transfer.directory(), candidate.directory());
    assert_ne!(std::fs::read(selection)?, before);
    fixture.preserved()?;
    drop(candidate);
    assert!(std::fs::write(transfer.executable(), b"changed").is_err());
    drop(transfer);
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}
