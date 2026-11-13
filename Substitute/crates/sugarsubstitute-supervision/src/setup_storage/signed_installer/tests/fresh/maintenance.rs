// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Repair completed first-install candidates through signed maintenance without rewriting settings.
use super::*;

#[test]
fn signed_maintenance_resumes_fresh_configuration_and_repairs_missing_executable() -> TestResult {
    repair(false, false)?;
    repair(true, false)
}

#[test]
fn signed_maintenance_preserves_original_descriptor_whitespace_when_resuming() -> TestResult {
    repair(true, true)
}

/// Keep actual signature verification, native publication and byte preservation in one campaign.
fn repair(missing_executable: bool, formatted: bool) -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let runtime = runtime(path)?;
        let candidate =
            import(path, context)?.prepare(|| NOW, context)?.complete(&runtime, || NOW, context)?;
        let directory = candidate.directory().to_owned();
        let executable = directory.join("sugarsubstitute.exe");
        let payload = std::fs::read(&executable)?;
        drop(candidate);
        let descriptor = directory.join("sugarsubstitute-launch.local.json");
        if formatted {
            let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&descriptor)?)?;
            std::fs::write(&descriptor, serde_json::to_vec_pretty(&value)?)?;
        }
        let files =
            ["startup.local.toml", "sugarsubstitute-launch.local.json", "installation-ready.json"];
        let original = files
            .map(|name| std::fs::read(directory.join(name)))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let authored = path.join("native/user/desktop-data/authored-workflow.json");
        std::fs::write(&authored, b"preserve authored data")?;
        if missing_executable {
            std::fs::remove_file(&executable)?;
        }
        let session = InstallationSession::maintain(&path.join("native"), context)?;
        let prepared = import_session(path, session, PreparationIntent::Repair, context)?
            .prepare(|| NOW, context)?;
        assert_eq!(prepared.maintenance_runtime(), Some(&runtime));
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        assert_eq!(candidate.directory(), directory);
        assert_eq!(std::fs::read(executable)?, payload);
        for (name, bytes) in files.into_iter().zip(original) {
            assert_eq!(std::fs::read(directory.join(name))?, bytes);
        }
        assert_eq!(std::fs::read(authored)?, b"preserve authored data");
        let record = SelectionStore::open(&path.join("native"), context)?
            .load(context)?
            .ok_or("Repaired selection absent")?;
        assert!(record.accepted().is_none());
        assert!(matches!(record.candidate(), Some(Candidate::Prepared { .. })));
        Ok(())
    })
}
