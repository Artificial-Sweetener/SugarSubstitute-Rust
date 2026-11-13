// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The pointer stays readable across releases, and only exact protected engine bytes are pinned.
use super::*;
use crate::{
    durable_storage::protected::directories::Mode,
    setup_storage::test_worker::{self as worker, TestResult},
};
use serde_json::json;

fn target(number: u8) -> serde_json::Value {
    json!({"generation": format!("{number:032x}"), "sha256": "a".repeat(64), "bytes": 4096})
}

#[test]
fn readers_ignore_fields_added_by_later_releases() -> Result<(), PointerFailure> {
    let mut primary = target(1);
    primary["signature"] = json!("added later");
    let bytes = json!({
        "schema_version": 1,
        "primary": primary,
        "fallback": target(2),
        "channel": "beta",
    })
    .to_string();
    let pointer = LaunchPointer::decode(bytes.as_bytes())?;
    assert_eq!(pointer.primary().generation().as_str(), format!("{:032x}", 1));
    assert_eq!(pointer.fallback().map(|target| target.bytes), Some(4096));
    let without_fallback = json!({"schema_version": 1, "primary": target(1)}).to_string();
    assert!(LaunchPointer::decode(without_fallback.as_bytes())?.fallback().is_none());
    Ok(())
}

#[test]
fn incompatible_damaged_and_ambiguous_pointers_are_typed_refusals() {
    let incompatible = json!({"schema_version": 2, "primary": target(1)}).to_string();
    assert!(matches!(
        LaunchPointer::decode(incompatible.as_bytes()),
        Err(PointerFailure::Incompatible)
    ));
    let mut short = target(1);
    short["sha256"] = json!("abc");
    let mut empty = target(1);
    empty["bytes"] = json!(0);
    let mut upper = target(1);
    upper["sha256"] = json!("A".repeat(64));
    for bytes in [
        b"not json".to_vec(),
        json!({"schema_version": 1}).to_string().into_bytes(),
        json!({"schema_version": 1, "primary": short}).to_string().into_bytes(),
        json!({"schema_version": 1, "primary": empty}).to_string().into_bytes(),
        json!({"schema_version": 1, "primary": upper}).to_string().into_bytes(),
        json!({"schema_version": 1, "primary": target(1), "fallback": target(1)})
            .to_string()
            .into_bytes(),
        json!({"schema_version": 1, "primary": {"generation": "../x", "sha256": "a".repeat(64), "bytes": 1}})
            .to_string()
            .into_bytes(),
        vec![b' '; MAX_BYTES + 1],
    ] {
        assert!(matches!(LaunchPointer::decode(&bytes), Err(PointerFailure::Damaged)));
    }
}

/// Write `bytes` as a protected generation engine and return its target.
pub(crate) fn engine(
    installation: &Path,
    generation: &GenerationId,
    bytes: &[u8],
    context: &JobContext,
) -> io::Result<EngineTarget> {
    protection::with(context, |protection| {
        let releases = installation.join("releases");
        protection.directory(&releases, Mode::OpenOrCreate, context)?;
        let directory = releases.join(generation.as_str());
        protection.directory(&directory, Mode::OpenOrCreate, context)?;
        let mut file = protection.file(&directory.join(LAUNCH_ENGINE), context)?;
        std::io::Write::write_all(&mut file, bytes)?;
        files::sync(&file, context)?;
        drop(file);
        EngineTarget::of(generation, &Directory::open(&directory, false)?, context)
    })
}

/// A pointer naming `primary` then `fallback`.
pub(crate) fn pointer(primary: EngineTarget, fallback: Option<EngineTarget>) -> LaunchPointer {
    LaunchPointer::new(primary, fallback)
}

/// The same target with bytes that no longer match its engine.
pub(crate) fn changed(target: &EngineTarget) -> EngineTarget {
    EngineTarget { sha256: "b".repeat(64), ..target.clone() }
}

/// A target naming a generation folder that does not exist.
pub(crate) fn absent(number: u8) -> io::Result<EngineTarget> {
    Ok(EngineTarget {
        generation: GenerationId::new(format!("{number:032x}")).map_err(io::Error::other)?,
        sha256: "c".repeat(64),
        bytes: 1,
    })
}

/// Replace the pointer with raw protected bytes, as a damaged disk or foreign tool might leave it.
pub(crate) fn overwrite(installation: &Path, bytes: &[u8], context: &JobContext) -> io::Result<()> {
    protection::with(context, |protection| {
        let root = Directory::open(installation, false)?;
        let state = Directory::open(&super::super::ownership::state_path(&root), false)?;
        let mut stage = publication::Stage::create(&state, protection, context)?;
        std::io::Write::write_all(stage.file(), bytes)?;
        stage.publish(&state.path().join(LAUNCH_POINTER_FILE), true, protection, context)
    })
}

/// Publish `pointer` into the fixture's protected state folder.
pub(crate) fn point(
    installation: &Path,
    pointer: &LaunchPointer,
    context: &JobContext,
) -> io::Result<()> {
    protection::with(context, |protection| {
        let root = Directory::open(installation, false)?;
        let state = Directory::open(&super::super::ownership::state_path(&root), false)?;
        publish(&state, pointer, protection, context)
    })
}

#[test]
fn published_pointers_read_back_and_a_missing_pointer_needs_setup() -> TestResult {
    worker::run_reserved(4 * POINTER_RESERVATION_BYTES, |context, _| {
        let root = worker::installation(170, context)?;
        assert!(matches!(read_launch_pointer(root.path(), context), Err(PointerFailure::Missing)));
        assert!(pointed_generations(root.path(), context)?.is_empty());
        let first = GenerationId::new("1".repeat(32))?;
        let second = GenerationId::new("2".repeat(32))?;
        let primary = engine(root.path(), &first, b"first engine", context)?;
        let fallback = engine(root.path(), &second, b"second engine", context)?;
        let pointer = LaunchPointer::new(primary.clone(), Some(fallback));
        point(root.path(), &pointer, context)?;
        assert_eq!(read_launch_pointer(root.path(), context)?, pointer);
        assert_eq!(pointed_generations(root.path(), context)?, [first.clone(), second.clone()]);
        let replaced = LaunchPointer::new(primary.clone(), Some(primary));
        point(root.path(), &replaced, context)?;
        assert!(read_launch_pointer(root.path(), context)?.fallback().is_none());
        Ok(())
    })
}

#[test]
fn only_exact_protected_engine_bytes_are_pinned() -> TestResult {
    worker::run_reserved(4 * POINTER_RESERVATION_BYTES, |context, _| {
        let root = worker::installation(171, context)?;
        let generation = GenerationId::new("3".repeat(32))?;
        let target = engine(root.path(), &generation, b"engine bytes", context)?;
        let pinned = pin_engine(root.path(), &target, context)?;
        // The pin denies writes and deletion while the creator holds it.
        assert!(std::fs::OpenOptions::new().write(true).open(pinned.executable()).is_err());
        assert!(std::fs::remove_file(pinned.executable()).is_err());
        drop(pinned);
        let mut changed = target.clone();
        changed.sha256 = "b".repeat(64);
        assert!(matches!(
            pin_engine(root.path(), &changed, context),
            Err(PointerFailure::EngineMismatch)
        ));
        let mut resized = target.clone();
        resized.bytes += 1;
        assert!(matches!(
            pin_engine(root.path(), &resized, context),
            Err(PointerFailure::EngineMismatch)
        ));
        let missing = EngineTarget { generation: GenerationId::new("4".repeat(32))?, ..target };
        assert!(matches!(
            pin_engine(root.path(), &missing, context),
            Err(PointerFailure::EngineMismatch)
        ));
        Ok(())
    })
}
