// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real original writer/witness exclusions and unchanged native claim codecs.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};
use std::io::Write;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, ResourceGeneration, ResourceId, ResourceKey,
};

struct Fixture {
    _temporary: tempfile::TempDir,
    root: Directory,
    coordinator: Record,
    backend: Record,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = protection::with(context, |owner| {
            let path = temporary.path().join("claims");
            owner.directory(&path, Mode::CreateNew, context)?;
            for leaf in ["coordinators", "backends"] {
                owner.directory(&path.join(leaf), Mode::CreateNew, context)?;
            }
            Directory::open(&path, false)
        })?;
        let key = CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([3; 16]).map_err(io::Error::other)?,
        };
        let mut coordinator = Record::coordinator(key);
        coordinator.state = RecordState::Released;
        let mut backend = Record::backend(
            ResourceKey {
                owner: key,
                id: ResourceId::new(1).map_err(io::Error::other)?,
                generation: ResourceGeneration::new(1).map_err(io::Error::other)?,
            },
            [9; 32],
        );
        backend.version = 2;
        backend.custodian = 1;
        backend.state = RecordState::Released;
        let fixture = Self { _temporary: temporary, root, coordinator, backend };
        fixture.write("coordinators", &fixture.coordinator, context)?;
        fixture.write("backends", &fixture.backend, context)?;
        Ok(fixture)
    }
    fn write(&self, directory: &str, record: &Record, context: &JobContext) -> io::Result<()> {
        let stem = crate::durable_storage::protected::record_file::hex(&record.digest);
        protection::with(context, |owner| {
            for extension in ["claim", "custody"] {
                let file = owner.file(
                    &self.root.path().join(directory).join(format!("{stem}.{extension}")),
                    context,
                )?;
                owner.close_file(file)?;
            }
            let mut file = owner
                .file(&self.root.path().join(directory).join(format!("{stem}.record")), context)?;
            file.write_all(&record.encode())?;
            files::sync(&file, context)?;
            owner.close_file(file)
        })
    }
    fn open(&self, context: &JobContext) -> io::Result<HistoricalClaims> {
        released(self.root.retained()?, &[self.coordinator.owner.namespace], context)
    }
    fn backend_path(&self, extension: &str) -> std::path::PathBuf {
        self.root
            .path()
            .join("backends")
            .join(crate::durable_storage::protected::record_file::hex(&self.backend.digest))
            .with_extension(extension)
    }
}
#[test]
fn released_originals_remain_pinned_without_publishing_or_activating_claims() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let original = std::fs::read(fixture.backend_path("record"))?;
        let source = fixture.open(context)?;
        assert!(std::fs::write(fixture.backend_path("record"), b"changed").is_err());
        assert!(fixture.open(context).is_err());
        drop(source);
        assert_eq!(std::fs::read(fixture.backend_path("record"))?, original);
        drop(fixture.open(context)?);
        Ok(())
    })
}
#[test]
fn current_backend_requires_independent_custody_and_rejects_both_live_exclusions() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let backend = fixture.root.child("backends")?;
        for extension in ["claim", "custody"] {
            let name = fixture
                .backend_path(extension)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Fixture leaf")?
                .to_owned();
            let live = backend.open_existing(&name, ExistingAccess::ExclusiveRead, context)?;
            assert!(fixture.open(context).is_err());
            drop(live);
        }
        std::fs::remove_file(fixture.backend_path("custody"))?;
        assert!(fixture.open(context).is_err());
        assert!(!fixture.backend_path("custody").exists());
        assert_eq!(std::fs::read(fixture.backend_path("record"))?, fixture.backend.encode());
        Ok(())
    })
}
#[test]
fn held_foreign_future_and_incomplete_records_never_supply_terminal_authority() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let original = fixture.backend.encode();
        let mut held = fixture.backend.clone();
        held.state = RecordState::Held;
        for bytes in [
            held.encode().to_vec(),
            {
                let mut future = original;
                future[8] = 3;
                future.to_vec()
            },
            original[..127].to_vec(),
        ] {
            std::fs::write(fixture.backend_path("record"), &bytes)?;
            assert!(fixture.open(context).is_err());
            assert_eq!(std::fs::read(fixture.backend_path("record"))?, bytes);
        }
        std::fs::write(fixture.backend_path("record"), original)?;
        let source = released(fixture.root.retained()?, &[NamespaceId::new([8; 32])], context)?;
        // No record from another installation namespace was locked or activated.
        assert!(fixture.open(context).is_ok());
        drop(source);
        Ok(())
    })
}
