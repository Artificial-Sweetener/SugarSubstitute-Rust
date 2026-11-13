// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual protected Released rows and independent read-only predecessor exclusions.
use super::*;
mod recovery;
use crate::setup_storage::test_worker::{self, TestResult};
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, RunGeneration, RunKey,
};

struct Fixture {
    source: Directory,
    row: record::Record,
    temporary: tempfile::TempDir,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("profile");
        let row = protection::with(context, |owner| {
            owner.directory(
                &path,
                crate::durable_storage::protected::directories::Mode::CreateNew,
                context,
            )?;
            let source = Directory::open(&path, false)?;
            let row = record::Record {
                account: record::account_identity(owner.account_sid()),
                folder: source.native_identity()?,
                run: RunKey {
                    owner: CoordinatorKey {
                        namespace: NamespaceId::new([7; 32]),
                        epoch: CoordinatorEpoch::new([3; 16]).map_err(io::Error::other)?,
                    },
                    instance: InstanceId::new(1).map_err(io::Error::other)?,
                    generation: RunGeneration::new(1).map_err(io::Error::other)?,
                },
                state: record::State::Released,
            };
            for name in [locations::CLAIM, locations::CUSTODY] {
                let file = owner.file(&source.path().join(name), context)?;
                files::sync(&file, context)?;
                owner.close_file(file)?;
            }
            let mut file = owner.file(&source.path().join(locations::RECORD), context)?;
            use std::io::Write;
            file.write_all(&row.encode())?;
            files::sync(&file, context)?;
            owner.close_file(file)?;
            Ok(row)
        })?;
        Ok(Self { source: Directory::open(&path, false)?, row, temporary })
    }
    fn admit(&self, context: &JobContext) -> io::Result<()> {
        with_released(&self.source, &[self.row.run.owner.namespace], context, |_| Ok(()))
    }
}

#[test]
fn released_source_retains_actual_exclusions_and_original_bytes_only_with_verified_namespace()
-> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let original = std::fs::read(fixture.source.path().join(locations::RECORD))?;
        with_released(&fixture.source, &[fixture.row.run.owner.namespace], context, |source| {
            assert_eq!(source.namespace(), fixture.row.run.owner.namespace);
            assert_eq!(source.directory().native_identity()?, fixture.row.folder);
            assert!(source.excluded_root_controls().contains(&locations::CLAIM));
            assert!(source.excluded_root_controls().contains(&locations::CUSTODY));
            assert!(
                std::fs::write(fixture.source.path().join(locations::RECORD), b"changed").is_err()
            );
            assert!(
                fixture
                    .source
                    .open_existing(locations::CLAIM, ExistingAccess::ExclusiveRead, context)
                    .is_err()
            );
            Ok(())
        })?;
        assert_eq!(std::fs::read(fixture.source.path().join(locations::RECORD))?, original);
        assert!(
            with_released(&fixture.source, &[NamespaceId::new([8; 32])], context, |_| Ok(()))
                .is_err()
        );
        fixture.admit(context)?;
        Ok(())
    })
}

#[test]
fn held_claim_or_independent_witness_cannot_become_a_copyable_released_source() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        for name in [locations::CLAIM, locations::CUSTODY] {
            let held =
                fixture.source.open_existing(name, ExistingAccess::ExclusiveRead, context)?;
            assert!(fixture.admit(context).is_err());
            drop(held);
            fixture.admit(context)?;
        }
        let mut row = fixture.row;
        row.state = record::State::Held;
        let bytes = row.encode();
        std::fs::write(fixture.source.path().join(locations::RECORD), bytes)?;
        assert!(fixture.admit(context).is_err());
        assert_eq!(std::fs::read(fixture.source.path().join(locations::RECORD))?, bytes);
        Ok(())
    })
}

#[test]
fn foreign_account_folder_and_future_profile_rows_are_preserved_and_rejected() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        for bytes in [
            record::Record { account: [1; 32], ..fixture.row }.encode(),
            record::Record { folder: [2; 24], ..fixture.row }.encode(),
            {
                let mut bytes = fixture.row.encode();
                bytes[8] = 3;
                bytes
            },
        ] {
            std::fs::write(fixture.source.path().join(locations::RECORD), bytes)?;
            assert!(fixture.admit(context).is_err());
            assert_eq!(std::fs::read(fixture.source.path().join(locations::RECORD))?, bytes);
        }
        Ok(())
    })
}

#[test]
fn hardlinked_controls_and_unrecognized_reserved_lifecycle_bytes_never_get_activated() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let alias = fixture.temporary.path().join("record-alias");
        let record = fixture.source.path().join(locations::RECORD);
        std::fs::hard_link(&record, &alias)?;
        assert!(fixture.admit(context).is_err());
        assert_eq!(std::fs::read(&alias)?, fixture.row.encode());
        std::fs::remove_file(&alias)?;
        protection::with(context, |owner| {
            let mut file = owner.file(
                &fixture.source.path().join(".sugarsubstitute-profile-unrecognized.record"),
                context,
            )?;
            use std::io::Write;
            file.write_all(b"unknown original")?;
            files::sync(&file, context)?;
            owner.close_file(file)
        })?;
        assert!(fixture.admit(context).is_err());
        assert_eq!(
            std::fs::read(
                fixture.source.path().join(".sugarsubstitute-profile-unrecognized.record")
            )?,
            b"unknown original"
        );
        Ok(())
    })
}
