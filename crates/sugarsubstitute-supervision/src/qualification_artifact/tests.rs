// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual create-only files and checked owner retirement in ordinary finite jobs.

use super::*;
use crate::native_claims::files::test_support::{FixedClock, TestResult, Wake, completion};
use std::{error::Error, fs};
use sugarsubstitute_contracts::{execution::*, lifecycle::*, startup::LaunchId};
use sugarsubstitute_execution::ExecutionOwner;
use windows::core::GUID;

fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

/// Use the actual ordinary reservation, never the unrelated symbol/capture admission classes.
fn admitted(
    work: impl FnOnce(JobContext, MainIdentity) -> Result<(), ClaimError> + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let main = identity()?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 64 * 1024,
            replacement: None,
        },
        move |context| work(context, main),
    )?;
    match completion(&client, &wake, &mut job)?.outcome {
        JobOutcome::Success(()) => Ok(()),
        other => Err(format!("Actual preview fixture failed: {other:?}").into()),
    }
}

fn directory() -> Result<PathBuf, ClaimError> {
    let id = GUID::new().map_err(|cause| ClaimError::new("fixture_id", cause))?;
    let path = std::env::temp_dir().join(format!("preview-{id:?}"));
    fs::create_dir(&path).map_err(|cause| ClaimError::new("fixture_directory", cause))?;
    Ok(path)
}

fn png() -> [u8; 33] {
    let mut bytes = [0; 33];
    bytes[..16].copy_from_slice(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    bytes[16..20].copy_from_slice(&3_u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&2_u32.to_be_bytes());
    bytes
}

#[test]
fn malformed_namespace_geometry_and_png_are_rejected_before_native_effects() -> TestResult {
    for path in [
        "relative.png",
        "\\\\server\\share\\a.png",
        "D:\\a.png:stream",
        "D:\\alias.\\a.png",
        "D:\\alias \\a.png",
        "D:\\a\\..\\b.png",
        "D:\\a.jpg",
    ] {
        assert!(validate(Path::new(path), (3, 2)).is_err(), "{path}");
    }
    assert!(validate(Path::new("D:\\a.png"), (0, 1)).is_err());
    assert!(validate(Path::new("D:\\a.png"), (u32::MAX, u32::MAX)).is_err());
    assert!(validate_png(&png(), (3, 3)).is_err());
    assert!(validate_png(&png()[..32], (3, 2)).is_err());
    let mut forged = png();
    forged[8] = 1;
    assert!(validate_png(&forged, (3, 2)).is_err());
    Ok(())
}

#[test]
fn absent_sealed_source_rejects_before_artifact_allocation_or_native_effect() -> TestResult {
    admitted(|context, identity| {
        let authority = Authority { seal: None, filesystem_boundary: None };
        assert!(
            PreviewArtifact::prepare(
                authority,
                identity,
                (3, 2),
                Moment::from_nanos(100),
                &context
            )
            .is_err()
        );
        let all = context
            .memory()
            .claim(64 * 1024)
            .map_err(|cause| ClaimError::new("fixture_untouched_credit", cause))?;
        assert_eq!(all.bytes(), 64 * 1024);
        Ok(())
    })
}

#[test]
fn actual_create_flush_retire_preserves_exact_borrowed_bytes_and_rejects_overwrite() -> TestResult {
    admitted(|context, identity| {
        let directory = directory()?;
        let destination = directory.join("actual.png");
        let bytes = png();
        let pointer = bytes.as_ptr();
        let mut artifact = PreviewArtifact::fixture_new(
            &destination,
            identity,
            (3, 2),
            Moment::from_nanos(100),
            &context,
        )?;
        artifact.initialize()?;
        assert!(!artifact.retired());
        assert!(artifact.activate().is_err());
        artifact.write_png(&bytes)?;
        assert_eq!(bytes.as_ptr(), pointer);
        assert!(artifact.write_png(&bytes).is_err());
        artifact.activate()?;
        assert!(artifact.activated());
        artifact.retire()?;
        assert!(artifact.retired());
        assert_eq!(fs::read(&destination).map_err(|e| ClaimError::new("fixture_read", e))?, bytes);
        assert!(context.memory().claim(STORAGE_BYTES).is_err());
        drop(artifact);
        let released = context
            .memory()
            .claim(64 * 1024)
            .map_err(|cause| ClaimError::new("observe_retired_fixture_credit", cause))?;
        drop(released);
        let mut duplicate = PreviewArtifact::fixture_new(
            &destination,
            identity,
            (3, 2),
            Moment::from_nanos(100),
            &context,
        )?;
        assert!(duplicate.initialize().is_err());
        duplicate.retire()?;
        assert!(duplicate.retired());
        assert_eq!(fs::read(&destination).map_err(|e| ClaimError::new("fixture_read", e))?, bytes);
        fs::remove_file(&destination).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
        fs::remove_dir(&directory).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
        Ok(())
    })
}

#[test]
fn native_close_boundary_failure_retains_one_cell_and_attempts_all_other_outputs() -> TestResult {
    admitted(|context, identity| {
        let directory = directory()?;
        let destination = directory.join("pending.png");
        let mut artifact = PreviewArtifact::fixture_new(
            &destination,
            identity,
            (3, 2),
            Moment::from_nanos(100),
            &context,
        )?;
        artifact.initialize()?;
        let expected = artifact.handles.len();
        let mut calls = 0;
        assert!(
            artifact
                .retire_with(|handle| {
                    calls += 1;
                    if calls == 1 {
                        Err(ClaimError::invalid("fixture_close_rejected"))
                    } else {
                        handle.close()
                    }
                })
                .is_err()
        );
        assert_eq!(calls, expected);
        assert!(!artifact.retired());
        assert_eq!(artifact.handles.iter().filter(|h| h.is_armed()).count(), 1);
        assert_eq!(artifact.memory.bytes(), STORAGE_BYTES);
        artifact.retire()?;
        assert!(artifact.retired());
        fs::remove_file(&destination).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
        fs::remove_dir(&directory).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
        Ok(())
    })
}

#[test]
fn foreign_resolved_name_and_expired_original_cutoff_do_not_activate() -> TestResult {
    admitted(|context, identity| {
        let directory = directory()?;
        let destination = directory.join("original.png");
        assert!(
            PreviewArtifact::fixture_new(
                &destination,
                identity,
                (3, 2),
                Moment::from_nanos(16),
                &context
            )
            .is_err()
        );
        let mut artifact = PreviewArtifact::fixture_new(
            &destination,
            identity,
            (3, 2),
            Moment::from_nanos(100),
            &context,
        )?;
        artifact.initialize()?;
        let index = artifact.output.ok_or_else(|| ClaimError::invalid("fixture_output"))?;
        assert!(
            native::validate_output(&mut artifact.handles[index], &directory.join("foreign.png"))
                .is_err()
        );
        assert!(!artifact.activated());
        artifact.retire()?;
        assert!(artifact.retired());
        fs::remove_file(&destination).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
        fs::remove_dir(&directory).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
        Ok(())
    })
}

#[test]
fn actual_scope_cancellation_prevents_write_and_still_requires_checked_retirement() -> TestResult {
    use std::sync::atomic::{AtomicBool, Ordering};
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    let cancelled = scope.clone();
    let retired = Arc::new(AtomicBool::new(false));
    let observed = retired.clone();
    let identity = identity()?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 64 * 1024,
            replacement: None,
        },
        move |context| {
            let directory = directory()?;
            let destination = directory.join("cancelled.png");
            let mut artifact = PreviewArtifact::fixture_new(
                &destination,
                identity,
                (3, 2),
                Moment::from_nanos(100),
                &context,
            )?;
            artifact.initialize()?;
            cancelled.cancel();
            assert!(context.cancellation_requested());
            assert!(artifact.write_png(&png()).is_err());
            assert!(artifact.activate().is_err());
            assert!(!artifact.retired());
            artifact.retire()?;
            assert!(artifact.retired());
            assert!(!artifact.activated());
            observed.store(true, Ordering::SeqCst);
            fs::remove_file(&destination).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
            fs::remove_dir(&directory).map_err(|e| ClaimError::new("fixture_cleanup", e))?;
            Ok::<_, ClaimError>(())
        },
    )?;
    assert!(matches!(completion(&client, &wake, &mut job)?.outcome, JobOutcome::Cancelled));
    assert!(retired.load(Ordering::SeqCst));
    Ok(())
}
