// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native producer custody selects actual source or confined owned base before worker effects.
use super::*;
use crate::setup_storage::{
    ownership,
    test_worker::{self, TestResult},
};

struct Fixture {
    _temporary: tempfile::TempDir,
    root: Directory,
    selection: Selection,
    runtime: RuntimePaths,
}
impl Fixture {
    fn new(context: &JobContext, mode: &str) -> io::Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        ownership::establish(&root)?;
        let managed = root.path().join("selected standalone");
        let attempt = managed.join("01234567-89ab-cdef-0123-456789abcdef");
        let source = attempt.join("ComfyUI");
        Directory::open(&source, true)?;
        std::fs::write(
            managed.join(".substitute-managed.json"),
            br#"{"schema_version":1,"product":"SugarSubstitute","kind":"managed-comfy"}"#,
        )?;
        std::fs::write(attempt.join(".substitute-provisioning.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"owner":"01234567-89ab-cdef-0123-456789abcdef","state":"ready",
            "installation":{"id":"01234567-89ab-cdef-0123-456789abcdef","sourceId":"standalone","installPath":attempt}
        })).map_err(io::Error::other)?)?;
        let runtime = RuntimePaths {
            directory: source.clone(),
            python: root.path().join("python.exe"),
            frontend: root.path().join("frontend"),
            metadata: root.path().join("METADATA"),
        };
        let selection = Selection {
            locale: "en".into(),
            install_root: root.path().to_owned(),
            mode: mode.into(),
            comfy_root: if mode == "managed" { managed } else { source },
            python: String::new(),
            models: root.path().join("unchanged models"),
            outputs: root.path().join("unchanged outputs"),
            selection_token: (mode == "managed").then(|| "native-plan".into()),
        };
        check(context)?;
        Ok(Self { _temporary: temporary, root, selection, runtime })
    }
}
#[test]
fn verified_owned_producer_serializes_v2_actual_storage_and_keeps_provisioning_bytes_pinned()
-> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "managed")?;
        let receipt = fixture
            .runtime
            .directory
            .parent()
            .ok_or("Attempt parent")?
            .join(".substitute-provisioning.json");
        let original = std::fs::read(&receipt)?;
        let storage = admit(&fixture.root, &fixture.selection, fixture.runtime.clone(), context)?;
        let wire = serde_json::to_value(storage.request())?;
        assert_eq!(wire["version"], 2);
        assert_eq!(wire["directory"], serde_json::to_value(&fixture.runtime.directory)?);
        assert_eq!(
            wire["storageBase"],
            serde_json::to_value(fixture.root.path().join("user").join("comfy-data"))?
        );
        assert_eq!(wire["models"], serde_json::to_value(&fixture.selection.models)?);
        assert_eq!(wire.as_object().ok_or("Integration object")?.len(), 7);
        storage.validate_result(&fixture.runtime)?;
        let mut changed = fixture.runtime.clone();
        changed.directory = fixture.root.path().join("different source");
        assert!(storage.validate_result(&changed).is_err());
        assert!(std::fs::write(&receipt, b"changed").is_err());
        drop(storage);
        assert_eq!(std::fs::read(receipt)?, original);
        assert!(!fixture.selection.models.exists());
        Ok(())
    })
}
#[test]
fn attached_producer_preserves_selected_normal_base_even_for_a_managed_looking_source() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "attached")?;
        let storage = admit(&fixture.root, &fixture.selection, fixture.runtime.clone(), context)?;
        let wire = serde_json::to_value(storage.request())?;
        assert_eq!(wire["storageBase"], serde_json::to_value(&fixture.runtime.directory)?);
        assert!(!fixture.root.path().join("user").exists());
        let other = Directory::open(&fixture.root.path().join("other source"), true)?;
        let mut changed = fixture.runtime.clone();
        changed.directory = other.path().to_owned();
        assert!(admit(&fixture.root, &fixture.selection, changed, context).is_err());
        Ok(())
    })
}
#[test]
fn missing_or_contradictory_owned_receipts_reject_before_creating_storage_or_model_outputs()
-> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "managed")?;
        let receipt = fixture
            .runtime
            .directory
            .parent()
            .ok_or("Attempt parent")?
            .join(".substitute-provisioning.json");
        let original = std::fs::read(&receipt)?;
        let mut value: serde_json::Value = serde_json::from_slice(&original)?;
        value["installation"]["sourceId"] = "foreign".into();
        let foreign = serde_json::to_vec(&value)?;
        std::fs::write(&receipt, &foreign)?;
        assert!(
            admit(&fixture.root, &fixture.selection, fixture.runtime.clone(), context).is_err()
        );
        assert_eq!(std::fs::read(&receipt)?, foreign);
        assert!(!fixture.root.path().join("user").exists());
        assert!(!fixture.selection.models.exists());
        std::fs::remove_file(&receipt)?;
        assert!(
            admit(&fixture.root, &fixture.selection, fixture.runtime.clone(), context).is_err()
        );
        assert!(!receipt.exists());
        assert!(!fixture.root.path().join("user").exists());
        Ok(())
    })
}
