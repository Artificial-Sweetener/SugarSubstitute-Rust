// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interrupt real native replacement at each durable boundary without executing fixture bytes.
use super::*;
use crate::setup_storage::{
    protection,
    test_worker::{self as worker, TestResult},
};
use sugarsubstitute_contracts::{
    application_launch::CURRENT_LAUNCHER_CONTRACT,
    installation::{Generation, GenerationId, PackageDigest},
};
mod supersession;

/// Trusted inert sources isolate the transition owner; signed admission is tested by its caller.
fn fixture(
    root: &Directory,
    context: &JobContext,
    protection: &mut Protection,
) -> io::Result<(Directory, record::Record, record::Record)> {
    let state = Directory::open(&root.path.join(".selection-state"), true)?;
    let mut records = Vec::new();
    for (id, bytes) in [("a", b"old launcher".as_slice()), ("b", b"new launcher".as_slice())] {
        let generation = Generation {
            id: GenerationId::new(id.repeat(32)).map_err(io::Error::other)?,
            package: PackageDigest::new(id.repeat(64)).map_err(io::Error::other)?,
        };
        let source =
            Directory::open(&root.path.join("releases").join(generation.id.as_str()), true)?;
        publication::write_immutable(&source, EXECUTABLE, bytes, protection, context)?;
        publication::write_immutable(
            &source,
            "sugarsubstitute-launcher.json",
            CURRENT_LAUNCHER_CONTRACT,
            protection,
            context,
        )?;
        records.push(record::Record::from_source(&generation, &source, context)?);
    }
    let previous = records.remove(0);
    let next = records.remove(0);
    record::publish(&state, &previous, protection, context)?;
    drop(payload::ensure(root, &previous, protection, context)?);
    Ok((state, previous, next))
}

/// Native readers stop rename at the image, ownership or journal retirement boundary.
fn interrupted(boundary: u8) -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, move |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, previous, next) =
            protection::with(context, |protection| fixture(&root, context, protection))?;
        if boundary < 2 {
            let pin = if boundary == 0 { root.read(EXECUTABLE)? } else { state.read(RECORD)? };
            protection::with(context, |protection| {
                assert!(
                    begin(&root, &state, previous.clone(), next.clone(), protection, context)
                        .is_err()
                );
                Ok(())
            })?;
            assert_eq!(
                std::fs::read(root.path.join(EXECUTABLE))?,
                if boundary == 0 { b"old launcher" } else { b"new launcher" }
            );
            drop(pin);
        } else {
            let pin = root.read(EXECUTABLE)?;
            protection::with(context, |protection| {
                assert!(
                    begin(&root, &state, previous.clone(), next.clone(), protection, context)
                        .is_err()
                );
                Ok(())
            })?;
            drop(pin);
            let journal = state.read(JOURNAL)?;
            protection::with(context, |protection| {
                assert!(recover(&root, &state, protection, context).is_err());
                assert!(record::read(&state, protection)?.as_ref() == Some(&next));
                Ok(())
            })?;
            assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"new launcher");
            drop(journal);
        }
        assert!(state.path.join(JOURNAL).is_file());
        protection::with(context, |protection| {
            super::super::recover(&root, &state, protection, context)?;
            assert!(record::read(&state, protection)?.as_ref() == Some(&next));
            assert!(recover(&root, &state, protection, context)?.is_none());
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"new launcher");
        assert!(!state.path.join(JOURNAL).exists());
        assert_eq!(
            std::fs::read(
                root.path.join("releases").join(previous.source.id.as_str()).join(EXECUTABLE)
            )?,
            b"old launcher"
        );
        Ok(())
    })
}

#[test]
fn interruption_before_image_replacement_recovers() -> TestResult {
    interrupted(0)
}

#[test]
fn interruption_after_image_before_ownership_recovers() -> TestResult {
    interrupted(1)
}

#[test]
fn interruption_after_ownership_before_journal_retirement_recovers() -> TestResult {
    interrupted(2)
}

#[test]
fn interrupted_replacement_preserves_unknown_root_bytes() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, previous, next) =
            protection::with(context, |protection| fixture(&root, context, protection))?;
        let pin = root.read(EXECUTABLE)?;
        protection::with(context, |protection| {
            assert!(begin(&root, &state, previous.clone(), next, protection, context).is_err());
            Ok(())
        })?;
        drop(pin);
        std::fs::write(root.path.join(EXECUTABLE), b"unknown owner")?;
        protection::with(context, |protection| {
            assert!(recover(&root, &state, protection, context).is_err());
            assert!(record::read(&state, protection)?.as_ref() == Some(&previous));
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"unknown owner");
        assert!(state.path.join(JOURNAL).is_file());
        Ok(())
    })
}

#[test]
fn foreign_or_unsupported_transition_preserves_both_owned_files() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, previous, next) =
            protection::with(context, |protection| fixture(&root, context, protection))?;
        let pin = root.read(EXECUTABLE)?;
        protection::with(context, |protection| {
            assert!(begin(&root, &state, previous.clone(), next, protection, context).is_err());
            Ok(())
        })?;
        drop(pin);
        let original = std::fs::read(state.path.join(JOURNAL))?;
        for (field, value) in [
            ("schema_version", serde_json::json!(2)),
            ("installation_directory", serde_json::json!([0, 0, 0])),
        ] {
            let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
            changed[field] = value;
            let bytes = serde_json::to_vec(&changed)?;
            std::fs::write(state.path.join(JOURNAL), &bytes)?;
            protection::with(context, |protection| {
                assert!(recover(&root, &state, protection, context).is_err());
                assert!(record::read(&state, protection)?.as_ref() == Some(&previous));
                Ok(())
            })?;
            assert_eq!(std::fs::read(state.path.join(JOURNAL))?, bytes);
            assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"old launcher");
        }
        Ok(())
    })
}
