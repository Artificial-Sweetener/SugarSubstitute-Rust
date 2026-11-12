// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real native file custody can outlive the application without retaining its installation locks.
use super::*;
use crate::setup_storage::{
    InstallationSession, LAUNCH_RESERVATION_BYTES, test_worker::pipeline::Pipeline,
};

#[test]
fn helper_image_pins_payloads_but_releases_run_and_allows_selection_and_setup() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (temporary, generation) =
        pipeline.run(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context| {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&root, &context)?;
            Ok((temporary, generation))
        })?;
    let path = temporary.path().join("releases").join(generation.id.as_str());
    let running = pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        Ok(retain_running_generation(&path, None, &context)?.ok_or("Run guard absent")?)
    })?;
    let image = pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        let image = running.pin_preparation(&context)?;
        assert_eq!(image.directory(), running.directory());
        assert_eq!(image.generation(), running.generation());
        drop(running);
        Ok(image)
    })?;
    pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        assert_eq!(image.installation(), temporary.path());
        assert_eq!(image.generation(), &generation);
        let store = Arc::new(SelectionStore::open(temporary.path(), &context)?);
        let before = store.load(&context)?.ok_or("Selection absent")?;
        let mut run = InstallationRun::acquire(store.clone(), &context)?;
        let InstallationLaunch::Application(next) = run.select_launch(None, &context)? else {
            return Err("Helper blocked the next application launch".into());
        };
        assert_eq!(next.executable(), image.executable());
        drop(next);
        drop(run);
        let setup = InstallationSession::maintain(temporary.path(), &context)?;
        let prepared = before.prepare(
            Generation {
                id: GenerationId::new("2".repeat(32))?,
                package: PackageDigest::new("c".repeat(64))?,
                release: sugarsubstitute_contracts::installation::SignedRelease::new(
                    "1.0.0".to_owned(),
                    1,
                )?,
            },
            PreparationIntent::Automatic,
        )?;
        store.compare_exchange(&before, &prepared, &context)?;
        drop(setup);
        let executable = image.executable();
        assert!(std::fs::OpenOptions::new().write(true).open(&executable).is_err());
        assert!(std::fs::remove_file(&executable).is_err());
        assert!(std::fs::rename(image.directory(), temporary.path().join("moved")).is_err());
        let settings = image.directory().join("startup.local.toml");
        let user_data = b"# user settings remain writable\n";
        std::fs::write(&settings, user_data)?;
        drop(image);
        drop(std::fs::OpenOptions::new().write(true).open(executable)?);
        assert_eq!(std::fs::read(settings)?, user_data);
        assert_eq!(store.load(&context)?, Some(prepared));
        Ok(())
    })
}

#[test]
fn expired_image_acquisition_does_not_release_running_ownership_or_publish_state() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&root, context)?;
        let path = root.path().join("releases").join(generation.id.as_str());
        let running = retain_running_generation(&path, None, context)?.ok_or("Run guard absent")?;
        let record = root.path().join("state/selection/.substitute-selection.json");
        let before = std::fs::read(&record)?;
        clock.expire();
        let error = match running.pin_preparation(context) {
            Ok(_) => return Err("Expired job retained a helper image".into()),
            Err(error) => error,
        };
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            std::fs::OpenOptions::new().write(true).open(path.join("sugarsubstitute.exe")).is_err()
        );
        assert_eq!(std::fs::read(record)?, before);
        Ok(())
    })
}
