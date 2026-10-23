// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! First installation survives interruption after replacement reservation and before allocation.
use super::*;

#[test]
fn first_install_retry_recovers_missing_replacement_directory_with_older_incomplete_files()
-> TestResult {
    let temporary = tempfile::tempdir()?;
    let runtime = runtime(temporary.path())?;
    let path = temporary.path().to_owned();
    worker::run_reserved_expiring(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, clock| {
        drop(version(&path, 1, context)?.prepare(|| NOW, context)?);
        let store = SelectionStore::open(&path.join("native"), context)?;
        let before = store.load(context)?.ok_or("Selection absent")?;
        let old = before.candidate().ok_or("Candidate absent")?.generation().id.as_str().to_owned();
        std::fs::write(path.join("native/releases").join(&old).join("user-note"), b"keep")?;
        let imported = version(&path, 2, context)?;
        let selection = path.join("native/.selection-state/.substitute-selection.json");
        clock.observe_checks(move || {
            let Ok(bytes) = std::fs::read(&selection) else {
                return false;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return false;
            };
            value["candidate"]["phase"] == "staging"
                && value["candidate"]["generation"]["id"].as_str().is_some_and(|id| id != old)
        })?;
        assert!(imported.prepare(|| NOW, context).is_err());
        Ok(())
    })?;
    let path = temporary.path().to_owned();
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let root = path.join("native");
        let store = SelectionStore::open(&root, context)?;
        let before = store.load(context)?.ok_or("Selection absent")?;
        let generation = before.candidate().ok_or("Candidate absent")?.generation();
        let destination = root.join("releases").join(generation.id.as_str());
        assert!(!destination.exists());
        let previous =
            std::fs::read_dir(root.join("releases"))?.next().ok_or("Predecessor missing")??.path();
        assert!(inspect_installation(&root, context)?.is_none());
        let candidate = version(&path, 2, context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?;
        assert_eq!(candidate.directory(), destination);
        let after = store.load(context)?.ok_or("Selection absent")?;
        assert_eq!(after.revision(), before.revision() + 1);
        assert!(after.accepted().is_none());
        assert_eq!(std::fs::read(previous.join("user-note"))?, b"keep");
        assert_eq!(std::fs::read(runtime.python)?, b"inert, never executed");
        Ok(())
    })
}
