// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify exact original bytes, mutable activation, ownership rejection and durable-phase restart.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
mod association;
mod bounds;
mod cancellation;
mod links;
mod selection;

struct Fixture {
    root: Directory,
    source: Directory,
    target: PathBuf,
    identity: InstallationId,
    temporary: Option<tempfile::TempDir>,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = test_worker::installation(0xaa, context).map_err(io::Error::other)?;
        let root = Directory::open(temporary.path(), false)?;
        protection::with(context, |owner| {
            owner.directory(
                &root.path().join("user"),
                super::super::protected::directories::Mode::CreateNew,
                context,
            )?;
            Ok(())
        })?;
        let source = Directory::open(&root.path().join("legacy"), true)?;
        let authored = source.child("authored").or_else(|cause| {
            if cause.kind() != io::ErrorKind::NotFound {
                return Err(cause);
            }
            std::fs::create_dir(source.path().join("authored"))?;
            source.child("authored")
        })?;
        std::fs::write(source.path().join("document"), b"original document")?;
        std::fs::write(source.path().join(".lease"), b"old lifecycle record")?;
        std::fs::write(authored.path().join(".lease"), b"authored nested data")?;
        drop(authored);
        let target = root.path().join("user").join("imported");
        Ok(Self {
            root,
            source,
            target,
            identity: InstallationId::new(format!("{:032x}", 0xaa)).map_err(io::Error::other)?,
            temporary: Some(temporary),
        })
    }
    fn reopen(path: &Path) -> io::Result<Self> {
        let root = Directory::open(path, false)?;
        let source = Directory::open(&root.path().join("legacy"), false)?;
        let target = root.path().join("user").join("imported");
        Ok(Self {
            root,
            source,
            target,
            identity: InstallationId::new(format!("{:032x}", 0xaa)).map_err(io::Error::other)?,
            temporary: None,
        })
    }
    fn copy(&self, context: &JobContext) -> io::Result<()> {
        copy_tree(
            &self.root,
            ImportAssociation::Installed(&self.identity),
            &self.source,
            &self.target,
            RootEntries::Except(&[".lease"]),
            context,
        )
    }
    fn receipt(&self) -> io::Result<PathBuf> {
        let parent = Directory::open(
            self.target.parent().ok_or_else(|| invalid("Fixture parent missing"))?,
            false,
        )?
        .resolved()?;
        Ok(self
            .root
            .path()
            .join("user/migration-originals/import-records")
            .join(receipt_name(&parent.path().join("imported"))?))
    }
}

#[test]
fn copy_preserves_originals_and_nested_names_without_replaying_lifecycle_or_user_edits()
-> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        fixture.copy(context)?;
        assert_eq!(std::fs::read(fixture.source.path().join("document"))?, b"original document");
        assert_eq!(std::fs::read(fixture.source.path().join(".lease"))?, b"old lifecycle record");
        assert_eq!(std::fs::read(fixture.target.join("document"))?, b"original document");
        assert_eq!(std::fs::read(fixture.target.join("authored/.lease"))?, b"authored nested data");
        assert!(!fixture.target.join(".lease").exists());
        std::fs::write(fixture.target.join("document"), b"new user edits")?;
        fixture.copy(context)?;
        verify_completed(
            &fixture.root,
            ImportAssociation::Installed(&fixture.identity),
            &fixture.target,
            context,
        )?;
        assert_eq!(std::fs::read(fixture.target.join("document"))?, b"new user edits");
        assert_eq!(std::fs::read(fixture.source.path().join("document"))?, b"original document");
        Ok(())
    })
}

#[test]
fn unknown_destination_hardlinks_and_foreign_installation_keep_every_original() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        std::fs::create_dir(&fixture.target)?;
        std::fs::write(fixture.target.join("authored"), b"unrelated existing data")?;
        assert!(fixture.copy(context).is_err());
        assert_eq!(std::fs::read(fixture.target.join("authored"))?, b"unrelated existing data");
        let other = fixture.root.path().join("user/other");
        let alias = fixture.root.path().join("source-alias");
        std::fs::hard_link(fixture.source.path().join("document"), &alias)?;
        assert!(
            copy_tree(
                &fixture.root,
                ImportAssociation::Installed(&fixture.identity),
                &fixture.source,
                &other,
                RootEntries::Except(&[]),
                context
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&alias)?, b"original document");
        std::fs::remove_file(&alias)?;
        copy_tree(
            &fixture.root,
            ImportAssociation::Installed(&fixture.identity),
            &fixture.source,
            &other,
            RootEntries::Except(&[]),
            context,
        )?;
        let changed = InstallationId::new("b".repeat(32))?;
        assert!(
            copy_tree(
                &fixture.root,
                ImportAssociation::Installed(&changed),
                &fixture.source,
                &other,
                RootEntries::Except(&[]),
                context
            )
            .is_err()
        );
        assert_eq!(std::fs::read(other.join("document"))?, b"original document");
        Ok(())
    })
}

#[test]
fn future_or_corrupt_receipts_are_preserved_and_cannot_fall_back_to_copying() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        fixture.copy(context)?;
        let receipt = fixture.receipt()?;
        let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&receipt)?)?;
        value["journal"]["schema_version"] = serde_json::json!(2);
        let bytes = serde_json::to_vec(&value)?;
        std::fs::write(&receipt, &bytes)?;
        assert!(fixture.copy(context).is_err());
        assert_eq!(std::fs::read(&receipt)?, bytes);
        std::fs::write(&receipt, b"{partial")?;
        assert!(fixture.copy(context).is_err());
        assert_eq!(std::fs::read(&receipt)?, b"{partial");
        assert_eq!(std::fs::read(fixture.target.join("document"))?, b"original document");
        Ok(())
    })
}

#[test]
fn completed_data_survives_source_relocation_and_unconfined_destinations_are_rejected() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        assert!(
            copy_tree(
                &fixture.root,
                ImportAssociation::Installed(&fixture.identity),
                &fixture.source,
                &fixture.root.path().join("outside-user"),
                RootEntries::Except(&[]),
                context
            )
            .is_err()
        );
        assert!(!fixture.root.path().join("outside-user").exists());
        fixture.copy(context)?;
        let Fixture { root, source, target, identity, temporary } = fixture;
        let original = source.path().to_owned();
        drop(source);
        let retained_original = root.path().join("retained-original");
        std::fs::rename(&original, &retained_original)?;
        verify_completed(&root, ImportAssociation::Installed(&identity), &target, context)?;
        assert_eq!(std::fs::read(target.join("document"))?, b"original document");
        assert_eq!(std::fs::read(retained_original.join("document"))?, b"original document");
        drop(root);
        drop(temporary);
        Ok(())
    })
}

#[derive(Clone, Copy)]
enum Boundary {
    Journal,
    Data,
    Receipt,
    Activated,
}

fn any_entry(
    directory: &Path,
    mut predicate: impl FnMut(std::fs::DirEntry) -> io::Result<bool>,
) -> io::Result<bool> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(cause) => return Err(cause),
    };
    for entry in entries {
        if predicate(entry?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn present(path: &Path, directory: bool) -> io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(if directory { metadata.is_dir() } else { metadata.is_file() }),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        // A data writer still holds its share-exclusive new file; observe only after it closes.
        Err(cause) if !directory && cause.raw_os_error() == Some(32) => Ok(false),
        Err(cause) => Err(cause),
    }
}

#[test]
fn expiration_at_each_durable_phase_reopens_under_a_fresh_owner_and_preserves_original_bytes()
-> TestResult {
    for boundary in [Boundary::Journal, Boundary::Data, Boundary::Receipt, Boundary::Activated] {
        let retained = Arc::new(Mutex::new(None::<tempfile::TempDir>));
        let saved = retained.clone();
        test_worker::run_reserved_expiring(16 * 1024 * 1024, move |context, clock| {
            let mut fixture = Fixture::new(context)?;
            let receipt = fixture.receipt()?;
            let target = fixture.target.clone();
            let parent = target.parent().ok_or("Fixture parent missing")?.to_owned();
            let journals = fixture.root.path().join("user/migration-originals/import-records");
            let observation_failure = Arc::new(Mutex::new(None::<io::Error>));
            let observed_failure = observation_failure.clone();
            clock.observe_checks(move || {
                let reached = match boundary {
                    Boundary::Journal => any_entry(&journals, |entry| {
                        Ok(entry.file_name().to_string_lossy().starts_with(".user-import-"))
                    }),
                    Boundary::Data => any_entry(&parent, |entry| {
                        Ok(entry.file_name().to_string_lossy().starts_with(".user-import-")
                            && present(&entry.path().join("document"), false)?)
                    }),
                    Boundary::Receipt => present(&receipt, false),
                    Boundary::Activated => present(&target, true),
                };
                match reached {
                    Ok(reached) => reached,
                    Err(cause) => {
                        *crate::registry::lock(&observed_failure) = Some(cause);
                        true
                    }
                }
            })?;
            assert!(fixture.copy(context).is_err());
            if let Some(cause) = crate::registry::lock(&observation_failure).take() {
                return Err(cause.into());
            }
            assert!(context.deadline().is_some_and(|until| context.now() >= until));
            assert_eq!(
                std::fs::read(fixture.source.path().join("document"))?,
                b"original document"
            );
            *saved.lock().map_err(|_| "Fixture preservation lock poisoned")? =
                fixture.temporary.take();
            Ok(())
        })?;
        let path = retained
            .lock()
            .map_err(|_| "Fixture preservation lock poisoned")?
            .as_ref()
            .ok_or("Original fixture not retained")?
            .path()
            .to_owned();
        test_worker::run(move |context, _| {
            let fixture = Fixture::reopen(&path)?;
            fixture.copy(context)?;
            assert_eq!(std::fs::read(fixture.target.join("document"))?, b"original document");
            assert_eq!(
                std::fs::read(fixture.source.path().join("document"))?,
                b"original document"
            );
            fixture.copy(context)?;
            Ok(())
        })?;
    }
    Ok(())
}
