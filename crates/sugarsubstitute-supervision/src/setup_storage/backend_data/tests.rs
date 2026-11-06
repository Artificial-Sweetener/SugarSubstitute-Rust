// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual persisted/native ownership and environment selection without ambient mutation.
use super::*;
use crate::LaunchEnvironment;
use crate::setup_storage::{
    RuntimePaths, Selection, ownership,
    test_worker::{self, TestResult},
};
use sugarsubstitute_contracts::installation::{GenerationId, PackageDigest};

struct Fixture {
    root: Directory,
    generation: Directory,
    identity: Generation,
    source: PathBuf,
    managed: PathBuf,
    attempt: PathBuf,
    _temporary: tempfile::TempDir,
}
impl Fixture {
    fn new(context: &JobContext, mode: &str) -> io::Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        ownership::establish(&root)?;
        let identity = Generation {
            id: GenerationId::new("a".repeat(32)).map_err(io::Error::other)?,
            package: PackageDigest::new("b".repeat(64)).map_err(io::Error::other)?,
        };
        let generation =
            Directory::open(&root.path().join("releases").join(identity.id.as_str()), true)?;
        let managed = root.path().join("selected-managed-environment");
        let owner = "01234567-89ab-cdef-0123-456789abcdef";
        let attempt = managed.join(owner);
        let source = attempt.join("ComfyUI");
        Directory::open(&source, true)?;
        std::fs::write(
            managed.join(".substitute-managed.json"),
            br#"{"schema_version":1,"product":"SugarSubstitute","kind":"managed-comfy"}"#,
        )?;
        std::fs::write(
            attempt.join(".substitute-provisioning.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema_version":1,"owner":owner,"state":"ready",
                "installation":{"id":owner,"sourceId":"standalone","installPath":attempt}
            }))
            .map_err(io::Error::other)?,
        )?;
        let selection = Selection {
            locale: "en".into(),
            install_root: root.path().to_owned(),
            mode: mode.into(),
            comfy_root: managed.clone(),
            python: String::new(),
            models: root.path().join("models"),
            outputs: root.path().join("outputs"),
            selection_token: (mode == "managed").then(|| "selected-plan".into()),
        };
        let runtime = RuntimePaths {
            directory: source.clone(),
            python: attempt.join("python.exe"),
            frontend: root.path().join("frontend"),
            metadata: root.path().join("METADATA"),
        };
        std::fs::write(generation.path().join("installation-ready.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"bundle_sha256":identity.package.as_str(),"selection":selection,"runtime":runtime
        })).map_err(io::Error::other)?)?;
        check(context)?;
        Ok(Self { root, generation, identity, source, managed, attempt, _temporary: temporary })
    }
    fn read(&self, context: &JobContext) -> io::Result<BackendDataAuthority> {
        BackendDataAuthority::read(&self.root, &self.generation, &self.identity, context)
    }
}

#[test]
fn verified_owned_native_source_confines_backend_data_and_preserves_original_records() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "managed")?;
        let paths = [
            fixture.generation.path().join("installation-ready.json"),
            fixture.managed.join(".substitute-managed.json"),
            fixture.attempt.join(".substitute-provisioning.json"),
        ];
        let original = paths.iter().map(std::fs::read).collect::<io::Result<Vec<_>>>()?;
        let authority = fixture.read(context)?;
        let policy = authority.select(&fixture.source, context)?;
        assert!(matches!(policy, BackendStorage::InstallationOwned(_)));
        let spelling =
            PathBuf::from(fixture.source.to_str().ok_or("Unicode source")?.to_ascii_uppercase());
        assert!(matches!(
            authority.select(&spelling, context)?,
            BackendStorage::InstallationOwned(_)
        ));
        let snapshot = "HOME=C:\\outside\0APPDATA=C:\\outside\0HF_HOME=C:\\outside\0\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        let launch = sugarsubstitute_contracts::configuration::LaunchConfiguration::new(
            fixture.source.clone(),
            fixture.attempt.join("python.exe"),
            vec![],
            8188,
        )?;
        let environment = LaunchEnvironment::for_managed_backend(&snapshot, &launch, policy)?;
        let text = String::from_utf16(environment.as_utf16())?;
        let storage = fixture.root.path().join("user").join("comfy-data").join("runtime");
        for name in ["HOME", "APPDATA", "HF_HOME"] {
            let prefix = format!("{name}=");
            let actual = text.split('\0').find_map(|entry| entry.strip_prefix(&prefix));
            assert_eq!(actual, storage.to_str(), "{name}");
        }
        assert_eq!(paths.iter().map(std::fs::read).collect::<io::Result<Vec<_>>>()?, original);
        assert!(std::fs::write(&paths[0], b"changed").is_err());
        assert!(std::fs::rename(&fixture.source, fixture.attempt.join("changed-source")).is_err());
        let other = fixture.root.path().join("explicit-external");
        std::fs::create_dir(&other)?;
        assert!(matches!(authority.select(&other, context)?, BackendStorage::External));
        Ok(())
    })
}

#[test]
fn attached_selection_stays_external_even_when_the_source_has_managed_receipts() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "attached")?;
        let authority = fixture.read(context)?;
        assert!(matches!(authority.select(&fixture.source, context)?, BackendStorage::External));
        assert!(!fixture.root.path().join("user/comfy-data/runtime").exists());
        Ok(())
    })
}

#[test]
fn missing_unknown_or_contradictory_installed_provenance_never_grants_owned_storage() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "managed")?;
        let ready = fixture.generation.path().join("installation-ready.json");
        let bytes = std::fs::read(&ready)?;
        std::fs::remove_file(&ready)?;
        assert!(fixture.read(context).is_err());
        std::fs::write(&ready, &bytes)?;
        for (path, changed) in [
            (
                fixture.managed.join(".substitute-managed.json"),
                serde_json::json!({"schema_version":2,"product":"SugarSubstitute","kind":"managed-comfy"}),
            ),
            (
                fixture.attempt.join(".substitute-provisioning.json"),
                serde_json::json!({"schema_version":1,"owner":"01234567-89ab-cdef-0123-456789abcdef","state":"provisioning"}),
            ),
        ] {
            let original = std::fs::read(&path)?;
            let rejected = serde_json::to_vec(&changed)?;
            std::fs::write(&path, &rejected)?;
            assert!(fixture.read(context).is_err());
            assert_eq!(std::fs::read(&path)?, rejected);
            std::fs::write(&path, original)?;
        }
        let mut changed: serde_json::Value = serde_json::from_slice(&bytes)?;
        changed["bundle_sha256"] = serde_json::json!("c".repeat(64));
        std::fs::write(&ready, serde_json::to_vec(&changed)?)?;
        assert!(fixture.read(context).is_err());
        assert!(!fixture.root.path().join("user/comfy-data/runtime").exists());
        Ok(())
    })
}

#[test]
fn hardlinked_ready_records_and_foreign_native_attempts_are_preserved_and_rejected() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "managed")?;
        let ready = fixture.generation.path().join("installation-ready.json");
        let original = std::fs::read(&ready)?;
        let alias = fixture.root.path().join("ready-alias.json");
        std::fs::hard_link(&ready, &alias)?;
        assert!(fixture.read(context).is_err());
        assert_eq!(std::fs::read(&ready)?, original);
        std::fs::remove_file(&alias)?;
        let receipt = fixture.attempt.join(".substitute-provisioning.json");
        let mut changed: serde_json::Value = serde_json::from_slice(&std::fs::read(&receipt)?)?;
        changed["installation"]["installPath"] = serde_json::json!(fixture.root.path());
        let bytes = serde_json::to_vec(&changed)?;
        std::fs::write(&receipt, &bytes)?;
        assert!(fixture.read(context).is_err());
        assert_eq!(std::fs::read(&receipt)?, bytes);
        assert_eq!(std::fs::read(&ready)?, original);
        assert!(!fixture.root.path().join("user/comfy-data/runtime").exists());
        Ok(())
    })
}

#[test]
fn foreign_sources_and_unsupported_receipt_fields_remain_unchanged() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context, "managed")?;
        let path = fixture.attempt.join(".substitute-provisioning.json");
        let original = std::fs::read(&path)?;
        for (pointer, value) in [
            ("/installation/sourceId", serde_json::json!("external")),
            ("/installation/id", serde_json::json!("fedcba98-7654-3210-fedc-ba9876543210")),
            ("/owner", serde_json::json!("01234567-89AB-CDEF-0123-456789ABCDEF")),
            ("/schema_version", serde_json::json!(2)),
        ] {
            let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
            *changed.pointer_mut(pointer).ok_or("Receipt field missing")? = value;
            let bytes = serde_json::to_vec(&changed)?;
            std::fs::write(&path, &bytes)?;
            assert!(fixture.read(context).is_err());
            assert_eq!(std::fs::read(&path)?, bytes);
        }
        let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
        changed["unknown"] = serde_json::json!(true);
        let bytes = serde_json::to_vec(&changed)?;
        std::fs::write(&path, &bytes)?;
        assert!(fixture.read(context).is_err());
        assert_eq!(std::fs::read(&path)?, bytes);
        std::fs::write(&path, vec![b' '; 65537])?;
        assert!(fixture.read(context).is_err());
        assert_eq!(std::fs::read(&path)?, vec![b' '; 65537]);
        assert!(!fixture.root.path().join("user/comfy-data/runtime").exists());
        Ok(())
    })
}

#[test]
fn expired_provenance_and_selection_reject_without_changing_original_records() -> TestResult {
    test_worker::run_expiring(|context, clock| {
        let fixture = Fixture::new(context, "managed")?;
        let ready = fixture.generation.path().join("installation-ready.json");
        let bytes = std::fs::read(&ready)?;
        let authority = fixture.read(context)?;
        clock.expire();
        assert!(authority.select(&fixture.source, context).is_err());
        assert!(fixture.read(context).is_err());
        assert_eq!(std::fs::read(&ready)?, bytes);
        Ok(())
    })
}
