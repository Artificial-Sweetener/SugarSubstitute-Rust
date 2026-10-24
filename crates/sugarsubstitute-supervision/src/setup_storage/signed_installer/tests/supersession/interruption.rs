// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A persisted replacement survives interruption before its canonical directory exists.
use super::*;
use crate::setup_storage::test_worker::pipeline::Pipeline;

#[test]
fn interrupted_supersession_reopens_its_reserved_generation_and_preserves_the_predecessor()
-> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, before) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Arc::new(Fixture::new(&context)?);
        drop(complete(&fixture, 1, &context)?);
        accept_pending(fixture.installation.path(), &context)?;
        drop(complete(&fixture, 2, &context)?);
        let before = SelectionStore::open(fixture.installation.path(), &context)?
            .load(&context)?
            .ok_or("Selection absent")?;
        Ok((fixture, before))
    })?;
    drop(pipeline);
    let previous = before.candidate().ok_or("Candidate absent")?.generation().clone();
    let previous_path = fixture.installation.path().join("releases").join(previous.id.as_str());
    let receipt = std::fs::read(previous_path.join("application-update-v1.record"))?;
    let expiring = fixture.clone();
    let old_id = previous.id.as_str().to_owned();
    worker::run_reserved_expiring(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, clock| {
        let imported = expiring.import(SignedPackage::new(&expiring.archive, 3)?, context)?;
        let selection =
            expiring.installation.path().join(".selection-state/.substitute-selection.json");
        clock.observe_checks(move || {
            let Ok(bytes) = std::fs::read(&selection) else {
                return false;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return false;
            };
            value["candidate"]["phase"] == "staging"
                && value["candidate"]["generation"]["id"].as_str().is_some_and(|id| id != old_id)
        })?;
        assert!(imported.prepare(|| NOW, context).is_err());
        Ok(())
    })?;
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let store = SelectionStore::open(fixture.installation.path(), context)?;
        let reserved = store.load(context)?.ok_or("Reservation absent")?;
        assert_eq!(reserved.revision(), before.revision() + 1);
        assert_eq!(reserved.accepted(), before.accepted());
        assert_eq!(reserved.retained(), before.retained());
        assert_eq!(reserved.rejected(), before.rejected());
        let replacement = reserved.candidate().ok_or("Candidate absent")?.generation();
        assert_ne!(replacement.id, previous.id);
        let path = fixture.installation.path().join("releases").join(replacement.id.as_str());
        assert!(!path.exists());
        let recovered = complete(&fixture, 3, context)?;
        assert_eq!(recovered.directory(), path);
        assert_eq!(std::fs::read(previous_path.join("application-update-v1.record"))?, receipt);
        assert!(previous_path.join("sugarsubstitute.exe").is_file());
        fixture.preserved()
    })
}
