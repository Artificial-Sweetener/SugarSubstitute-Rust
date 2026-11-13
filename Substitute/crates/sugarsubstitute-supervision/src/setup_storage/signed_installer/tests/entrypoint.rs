// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Signed maintenance restores established launcher ownership before an optional application run.
use super::*;
use crate::setup_storage::entrypoint::native;
use sugarsubstitute_contracts::{
    application_launch::{CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT},
    execution::JobContext,
};

/// Stable root entry users launch.
const ENTRY: &str = "SugarSubstitute.exe";
/// Launcher image inside each application generation.
const SOURCE: &str = "sugarsubstitute-launcher.exe";
const RECORD: &str = "state/selection/.launcher-generation.json";

/// Establish accepted inert payloads and their real protected shell ownership.
/// Health is supplied at the storage boundary; these tests execute no application payload.
fn installed(context: &JobContext) -> Result<Fixture, Failure> {
    let fixture = Fixture::with_distribution(
        test_package::distribution_with_launcher(
            Some(CURRENT_CONTRACT),
            Some(CURRENT_LAUNCHER_CONTRACT),
        )?,
        context,
    )?;
    drop(prepare(&fixture, 1, context)?);
    test_package::maintenance::accept_pending(fixture.installation.path(), context)?;
    let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
    let mut run = InstallationRun::acquire(store, context)?;
    let InstallationLaunch::Application(application) = run.select_launch(None, context)? else {
        return Err("Accepted fixture missing".into());
    };
    application.repair_launch_entry(context)?;
    Ok(fixture)
}

/// Exercise signed completion without requesting trial transfer or opening the application.
fn prepare(
    fixture: &Fixture,
    sequence: u32,
    context: &JobContext,
) -> Result<SignedCandidate, Failure> {
    let prepared = fixture
        .import(SignedPackage::new(&fixture.archive, sequence)?, context)?
        .prepare(|| NOW, context)?;
    let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
    Ok(prepared.complete(&runtime, || NOW, context)?)
}

/// The root holds exactly the stable entry file; everything else is a folder.
fn require_self_contained_root(root: &std::path::Path) -> Result<(), Failure> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() && entry.file_name() != ENTRY {
            return Err(format!("Unexpected root file {:?}", entry.file_name()).into());
        }
    }
    Ok(())
}

#[test]
fn signed_rescue_restores_launcher_while_application_run_is_held() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        require_self_contained_root(root)?;
        let image = std::fs::read(root.join(ENTRY))?;
        let store = Arc::new(SelectionStore::open(root, context)?);
        let before = store.load(context)?.ok_or("Selection missing")?;
        let run = InstallationRun::acquire(store.clone(), context)?;
        std::fs::remove_file(root.join(ENTRY))?;
        let candidate = prepare(&fixture, 2, context)?;
        assert_eq!(std::fs::read(root.join(ENTRY))?, image);
        require_self_contained_root(root)?;
        let ownership: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(RECORD))?)?;
        assert_eq!(ownership["source"]["id"], candidate.candidate.generation().id.as_str());
        let after = store.load(context)?.ok_or("Selection missing")?;
        assert_eq!(after.accepted(), before.accepted());
        assert!(matches!(after.candidate(), Some(Candidate::Prepared { .. })));
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(candidate);
        drop(run);
        let _next = InstallationRun::acquire(store, context)?;
        fixture.preserved()
    })
}

#[test]
fn signed_rescue_retires_the_earlier_root_launcher_and_its_owned_shortcut() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        let image = std::fs::read(root.join(ENTRY))?;
        let earlier = root.join(SOURCE);
        let shortcut = native::encode(&earlier)?;
        crate::durable_storage::protection::with(context, |protection| {
            use std::io::Write as _;
            protection.file(&earlier, context)?.write_all(&image)?;
            protection
                .file(&root.join("state/selection/SugarSubstitute.lnk"), context)?
                .write_all(&shortcut)?;
            Ok(())
        })?;
        std::fs::write(root.join("SugarSubstitute.lnk"), &shortcut)?;
        let _candidate = prepare(&fixture, 2, context)?;
        assert!(!earlier.exists());
        assert!(!root.join("SugarSubstitute.lnk").exists());
        assert_eq!(std::fs::read(root.join(ENTRY))?, image);
        require_self_contained_root(root)?;
        fixture.preserved()
    })
}

#[test]
fn edited_earlier_shortcut_and_foreign_root_launcher_are_preserved() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        let mut shortcut = native::encode(&root.join(SOURCE))?;
        crate::durable_storage::protection::with(context, |protection| {
            use std::io::Write as _;
            protection
                .file(&root.join("state/selection/SugarSubstitute.lnk"), context)?
                .write_all(&shortcut)?;
            Ok(())
        })?;
        shortcut.extend_from_slice(b"user edit");
        std::fs::write(root.join("SugarSubstitute.lnk"), &shortcut)?;
        std::fs::write(root.join(SOURCE), b"user launcher")?;
        let _candidate = prepare(&fixture, 2, context)?;
        assert_eq!(std::fs::read(root.join("SugarSubstitute.lnk"))?, shortcut);
        assert_eq!(std::fs::read(root.join(SOURCE))?, b"user launcher");
        fixture.preserved()
    })
}

#[test]
fn signed_rescue_restores_launcher_when_accepted_application_is_missing() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        let store = SelectionStore::open(root, context)?;
        let before = store.load(context)?.ok_or("Selection missing")?;
        let accepted = before.accepted().ok_or("Accepted generation missing")?;
        let application =
            root.join("releases").join(accepted.id.as_str()).join("sugarsubstitute.exe");
        std::fs::remove_file(&application)?;
        let image = std::fs::read(root.join(ENTRY))?;
        std::fs::remove_file(root.join(ENTRY))?;
        let _candidate = prepare(&fixture, 2, context)?;
        assert_eq!(std::fs::read(root.join(ENTRY))?, image);
        assert!(!application.exists());
        assert_eq!(store.load(context)?.ok_or("Selection missing")?.accepted(), Some(accepted));
        fixture.preserved()
    })
}

#[test]
fn reopening_cached_candidate_restores_launcher_without_changing_selection() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        let SignedCandidate { candidate, trust, offer, _custody } = prepare(&fixture, 2, context)?;
        let directory = candidate.directory().to_owned();
        drop(candidate);
        drop(_custody);
        let image = std::fs::read(root.join(ENTRY))?;
        std::fs::remove_file(root.join(ENTRY))?;
        let store = Arc::new(SelectionStore::open(root, context)?);
        let before = store.load(context)?;
        let _run = InstallationRun::acquire(store.clone(), context)?;
        let CachedCandidate::Ready(reopened) =
            SignedCandidate::reopen_cached(root, trust, offer, || NOW, context)?
        else {
            return Err("Cached candidate not restored".into());
        };
        assert_eq!(reopened.directory(), directory);
        assert_eq!(std::fs::read(root.join(ENTRY))?, image);
        assert_eq!(store.load(context)?, before);
        assert!(InstallationRun::acquire(store, context).is_err());
        fixture.preserved()
    })
}

/// Every preservation case has its own native operation and observation budget.
fn modified_owned_entry(leaf: &'static str) -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        let store = SelectionStore::open(root, context)?;
        let before = store.load(context)?.ok_or("Selection missing")?;
        let mut changed = std::fs::read(root.join(leaf))?;
        changed.extend_from_slice(b"user modification");
        std::fs::write(root.join(leaf), &changed)?;
        assert!(prepare(&fixture, 2, context).is_err());
        assert_eq!(std::fs::read(root.join(leaf))?, changed);
        assert_eq!(store.load(context)?.ok_or("Selection missing")?.accepted(), before.accepted());
        fixture.preserved()
    })
}

#[test]
fn signed_rescue_preserves_modified_root_executable() -> TestResult {
    modified_owned_entry(ENTRY)
}

#[test]
fn signed_rescue_preserves_corrupt_ownership() -> TestResult {
    modified_owned_entry(RECORD)
}

#[test]
fn signed_rescue_uses_authenticated_candidate_when_original_launcher_source_is_missing()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = installed(context)?;
        let root = fixture.installation.path();
        let record = std::fs::read(root.join(RECORD))?;
        let ownership: serde_json::Value = serde_json::from_slice(&record)?;
        let source = ownership["source"]["id"].as_str().ok_or("Source missing")?;
        std::fs::remove_file(root.join("releases").join(source).join(SOURCE))?;
        std::fs::remove_file(root.join(ENTRY))?;
        let candidate = prepare(&fixture, 2, context)?;
        assert_eq!(
            std::fs::read(root.join(ENTRY))?,
            std::fs::read(candidate.directory().join(SOURCE))?
        );
        let next: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(RECORD))?)?;
        assert_eq!(next["source"]["id"], candidate.candidate.generation().id.as_str());
        assert_ne!(std::fs::read(root.join(RECORD))?, record);
        fixture.preserved()
    })
}

#[test]
fn launcher_rescue_fits_the_production_preparation_job_reservation() -> TestResult {
    use crate::setup_storage::test_worker::pipeline::Pipeline;
    let mut pipeline = Pipeline::new()?;
    let fixture =
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| installed(&context))?;
    let image = std::fs::read(fixture.installation.path().join(ENTRY))?;
    std::fs::remove_file(fixture.installation.path().join(ENTRY))?;
    let (fixture, imported) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let imported = fixture.import(SignedPackage::new(&fixture.archive, 2)?, &context)?;
        Ok((fixture, imported))
    })?;
    let prepared = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        Ok(imported.prepare(|| NOW, &context)?)
    })?;
    let candidate = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        Ok(prepared.complete(&runtime, || NOW, &context)?)
    })?;
    assert_eq!(std::fs::read(fixture.installation.path().join(ENTRY))?, image);
    fixture.preserved()?;
    drop(candidate);
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}
