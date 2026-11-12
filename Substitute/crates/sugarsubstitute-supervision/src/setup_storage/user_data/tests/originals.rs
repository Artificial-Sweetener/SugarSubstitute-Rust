// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact original controls survive valid edits, rejected recovery bytes and interrupted siblings.
use super::*;
use crate::durable_storage::files;
use std::io::Write;

fn directory(installation: &InstallationData) -> io::Result<PathBuf> {
    let parent = installation
        .root
        .path()
        .join("user/migration-originals")
        .join(installation.identity.id.as_str());
    let mut entries = std::fs::read_dir(parent)?;
    let first = entries
        .next()
        .transpose()?
        .ok_or_else(|| io::Error::other("Original fixture directory absent"))?;
    if entries.next().is_some() {
        return Err(io::Error::other("Original fixture directory ambiguous"));
    }
    Ok(first.path())
}
#[test]
fn recovery_keeps_exact_originals_and_first_complete_snapshot_after_valid_user_edits() -> TestResult
{
    test_worker::run_reserved(LAUNCH_DATA_PREPARATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let installation = fixture.installation(context)?;
        let controls = fixture.controls(context)?;
        super::super::originals::preserve(&installation, &controls, context)?;
        let directory = directory(&installation)?;
        let manifest = std::fs::read(directory.join("originals-v1.json"))?;
        let record: serde_json::Value = serde_json::from_slice(&manifest)?;
        for ((kind, original), row) in
            controls.files().zip(record["originals"].as_array().ok_or("Original inventory")?)
        {
            assert_eq!(row["kind"], kind.name());
            assert_eq!(
                std::fs::read(directory.join(row["leaf"].as_str().ok_or("Original leaf")?))?,
                original.bytes
            );
        }
        let startup = controls.startup.as_ref().ok_or("Original startup")?.bytes.clone();
        drop(controls);
        let mut changed = b"# later legitimate edit\r\n".to_vec();
        changed.extend_from_slice(&startup);
        std::fs::write(fixture.image.join("startup.local.toml"), &changed)?;
        let controls = fixture.controls(context)?;
        super::super::originals::preserve(&installation, &controls, context)?;
        assert_eq!(std::fs::read(directory.join("originals-v1.json"))?, manifest);
        assert_eq!(std::fs::read(fixture.image.join("startup.local.toml"))?, changed);
        Ok(())
    })
}
#[test]
fn future_manifest_and_changed_recovery_copies_fail_without_overwriting_either_original()
-> TestResult {
    test_worker::run_reserved(LAUNCH_DATA_PREPARATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let installation = fixture.installation(context)?;
        let controls = fixture.controls(context)?;
        super::super::originals::preserve(&installation, &controls, context)?;
        let directory = directory(&installation)?;
        let manifest = std::fs::read(directory.join("originals-v1.json"))?;
        let mut record: serde_json::Value = serde_json::from_slice(&manifest)?;
        record["schema_version"] = 2.into();
        let future = serde_json::to_vec(&record)?;
        std::fs::write(directory.join("originals-v1.json"), &future)?;
        assert!(super::super::originals::preserve(&installation, &controls, context).is_err());
        assert_eq!(std::fs::read(directory.join("originals-v1.json"))?, future);
        std::fs::write(directory.join("originals-v1.json"), &manifest)?;
        let leaf = record["originals"][0]["leaf"].as_str().ok_or("Original leaf")?;
        std::fs::write(directory.join(leaf), b"conflicting original")?;
        assert!(super::super::originals::preserve(&installation, &controls, context).is_err());
        assert_eq!(std::fs::read(directory.join(leaf))?, b"conflicting original");
        assert_eq!(std::fs::read(directory.join("originals-v1.json"))?, manifest);
        for (_, original) in controls.files() {
            assert_eq!(std::fs::read(&original.path)?, original.bytes);
        }
        Ok(())
    })
}
#[test]
fn interrupted_known_publication_siblings_are_retained_on_reopen() -> TestResult {
    test_worker::run_reserved(LAUNCH_DATA_PREPARATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let installation = fixture.installation(context)?;
        let controls = fixture.controls(context)?;
        super::super::originals::preserve(&installation, &controls, context)?;
        let directory = directory(&installation)?;
        let stage = directory.join(".import-record-0123456789abcdef0123456789abcdef");
        protection::with(context, |owner| {
            let mut file = owner.file(&stage, context)?;
            file.write_all(b"{interrupted original")?;
            files::sync(&file, context)?;
            owner.close_file(file)
        })?;
        drop(controls);
        drop(installation);
        let installation = fixture.installation(context)?;
        let controls = fixture.controls(context)?;
        super::super::originals::preserve(&installation, &controls, context)?;
        assert_eq!(std::fs::read(stage)?, b"{interrupted original");
        for (_, original) in controls.files() {
            assert_eq!(std::fs::read(&original.path)?, original.bytes);
        }
        Ok(())
    })
}
