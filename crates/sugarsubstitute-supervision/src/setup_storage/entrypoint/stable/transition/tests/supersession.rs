// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rescue interrupted launcher replacement without depending on either obsolete repair source.
use super::*;

/// Supply a third pinned, authenticated-at-the-caller source with distinguishable executable bytes.
fn replacement(root: &Directory, context: &JobContext) -> io::Result<record::Record> {
    protection::with(context, |protection| {
        let generation = Generation {
            id: GenerationId::new("c".repeat(32)).map_err(io::Error::other)?,
            package: PackageDigest::new("c".repeat(64)).map_err(io::Error::other)?,
        };
        let source =
            Directory::open(&root.path.join("releases").join(generation.id.as_str()), true)?;
        publication::write_immutable(&source, EXECUTABLE, b"rescue launcher", protection, context)?;
        publication::write_immutable(
            &source,
            "sugarsubstitute-launcher.json",
            CURRENT_LAUNCHER_CONTRACT,
            protection,
            context,
        )?;
        record::Record::from_source(&generation, &source, context)
    })
}

/// A real conflicting native handle interrupts publication before or after the root image rename.
fn pending_fixture(
    root: &Directory,
    after_image: bool,
    context: &JobContext,
) -> io::Result<(Directory, record::Record, record::Record)> {
    let (state, previous, next) =
        protection::with(context, |protection| fixture(root, context, protection))?;
    let pin = if after_image { state.read(RECORD)? } else { root.read(EXECUTABLE)? };
    protection::with(context, |protection| {
        assert!(begin(root, &state, previous.clone(), next.clone(), protection, context).is_err());
        Ok(())
    })?;
    drop(pin);
    Ok((state, previous, next))
}

/// Remove only fixture-owned source files, then rescue both surviving and missing root images.
fn rescue(after_image: bool, missing_image: bool, ownership_advanced: bool) -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, move |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, previous, next) = pending_fixture(&root, after_image, context)?;
        if ownership_advanced {
            let pin = state.read(JOURNAL)?;
            protection::with(context, |protection| {
                assert!(recover(&root, &state, protection, context).is_err());
                assert!(record::read(&state, protection)?.as_ref() == Some(&next));
                Ok(())
            })?;
            drop(pin);
        }
        for source in [&previous, &next] {
            std::fs::remove_file(
                root.path.join("releases").join(source.source.id.as_str()).join(EXECUTABLE),
            )?;
        }
        if missing_image {
            std::fs::remove_file(root.path.join(EXECUTABLE))?;
        }
        let replacement = replacement(&root, context)?;
        protection::with(context, |protection| {
            super::super::super::recover_publications(&root, &state, protection, context)?;
            let installed = supersede(&root, &state, replacement.clone(), protection, context)?;
            assert!(installed == replacement);
            assert!(record::read(&state, protection)?.as_ref() == Some(&replacement));
            assert!(recover(&root, &state, protection, context)?.is_none());
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"rescue launcher");
        assert!(!state.path.join(JOURNAL).exists());
        Ok(())
    })
}

#[test]
fn rescue_supersedes_unpublished_image_without_old_sources() -> TestResult {
    rescue(false, false, false)
}

#[test]
fn rescue_supersedes_published_image_before_ownership_without_old_sources() -> TestResult {
    rescue(true, false, false)
}

#[test]
fn rescue_supersedes_published_ownership_without_old_sources() -> TestResult {
    rescue(true, false, true)
}

#[test]
fn rescue_supersedes_missing_image_before_ownership_without_old_sources() -> TestResult {
    rescue(false, true, false)
}

#[test]
fn rescue_supersedes_missing_image_after_ownership_without_old_sources() -> TestResult {
    rescue(true, true, true)
}

#[test]
fn interrupted_rescue_reconciles_ownership_before_retiring_old_intent() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, _, next) = pending_fixture(&root, true, context)?;
        let replacement = replacement(&root, context)?;
        let original = std::fs::read(state.path.join(JOURNAL))?;
        let pin = state.read(JOURNAL)?;
        protection::with(context, |protection| {
            super::super::super::recover_publications(&root, &state, protection, context)?;
            assert!(supersede(&root, &state, replacement.clone(), protection, context).is_err());
            assert!(record::read(&state, protection)?.as_ref() == Some(&next));
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"new launcher");
        assert_eq!(std::fs::read(state.path.join(JOURNAL))?, original);
        drop(pin);
        protection::with(context, |protection| {
            supersede(&root, &state, replacement.clone(), protection, context)?;
            assert!(record::read(&state, protection)?.as_ref() == Some(&replacement));
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"rescue launcher");
        Ok(())
    })
}

#[test]
fn rescue_rejects_incompatible_replacement_without_discarding_old_intent() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, previous, _) = pending_fixture(&root, false, context)?;
        let original = std::fs::read(state.path.join(JOURNAL))?;
        let mut replacement = replacement(&root, context)?;
        replacement.contract =
            r#"{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":2}"#
                .into();
        protection::with(context, |protection| {
            assert!(supersede(&root, &state, replacement, protection, context).is_err());
            assert!(record::read(&state, protection)?.as_ref() == Some(&previous));
            Ok(())
        })?;
        assert_eq!(std::fs::read(state.path.join(JOURNAL))?, original);
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"old launcher");
        Ok(())
    })
}

#[test]
fn rescue_preserves_foreign_intent_and_unknown_root_bytes() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (state, previous, _) = pending_fixture(&root, false, context)?;
        let replacement = replacement(&root, context)?;
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
                assert!(
                    supersede(&root, &state, replacement.clone(), protection, context).is_err()
                );
                assert!(record::read(&state, protection)?.as_ref() == Some(&previous));
                Ok(())
            })?;
            assert_eq!(std::fs::read(state.path.join(JOURNAL))?, bytes);
        }
        std::fs::write(state.path.join(JOURNAL), &original)?;
        std::fs::write(root.path.join(EXECUTABLE), b"unknown owner")?;
        protection::with(context, |protection| {
            assert!(supersede(&root, &state, replacement, protection, context).is_err());
            assert!(record::read(&state, protection)?.as_ref() == Some(&previous));
            Ok(())
        })?;
        assert_eq!(std::fs::read(state.path.join(JOURNAL))?, original);
        assert_eq!(std::fs::read(root.path.join(EXECUTABLE))?, b"unknown owner");
        Ok(())
    })
}
