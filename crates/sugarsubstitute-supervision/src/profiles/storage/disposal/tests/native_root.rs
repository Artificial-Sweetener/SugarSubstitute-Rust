// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise rejected acquisition's actual protected root and checked close ordering.
use super::*;
use crate::profiles::storage::{ProfilePort, WindowsProfilePort};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn rejected_native_profile_closes_file_once_and_retains_root_until_same_cutoff_retry() -> TestResult
{
    let mut harness = Harness::new()?;
    let until = Moment::from_nanos(15_000_000_000);
    let mut spec = harness.spec(until)?;
    spec.reservation_bytes = crate::profiles::PROFILE_OPERATION_BYTES;
    let directory = std::env::temp_dir().join(format!(
        "sugarsubstitute-profile-disposal-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut job = harness.client.submit_blocking(&harness.scope, spec, move |context| {
        let directory =
            crate::durable_storage::protected::profile_fixture::create(&directory, &context)?;
        let profile = directory.join("profile");
        let marker = profile.join(".sugarsubstitute-profile-v1.record");
        let claim = profile.join(".sugarsubstitute-profile-v1.claim");
        let port = WindowsProfilePort;
        let run = held()?.run;
        let mut predecessor = NativeState::new();
        port.acquire(&mut predecessor, &profile, run, &context)?;
        let original = std::fs::read(&marker)
            .map_err(|cause| ClaimError::new("disposal_original_record", cause))?;
        assert!(predecessor.held);
        let file = predecessor
            .file
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("disposal_predecessor_file"))?;
        file.close()?;
        predecessor.file = None;
        predecessor
            .root
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("disposal_predecessor_root"))?
            .retire(&context)?;
        predecessor.root = None;
        drop(predecessor);

        let mut current = NativeState::new();
        let failure = port
            .acquire(
                &mut current,
                &profile,
                RunKey {
                    generation: RunGeneration::new(2)
                        .map_err(|cause| ClaimError::new("disposal_next_generation", cause))?,
                    ..run
                },
                &context,
            )
            .err()
            .ok_or_else(|| ClaimError::invalid("disposal_expected_predecessor_refusal"))?;
        assert_eq!(failure.operation(), "profile_unresolved_predecessor");
        current.failure = Some(failure);
        assert!(!current.publication_attempted && current.record.is_none());
        let handles = current
            .root
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("disposal_current_root"))?
            .live_handles();
        assert!(handles > 0);
        let mut file_closes = 0;
        let mut root_closes = 0;
        let failed = dispose_unpublished(
            &mut current,
            &context,
            |file| {
                file_closes += 1;
                file.close()
            },
            |root| {
                root_closes += 1;
                assert_eq!(root.live_handles(), handles);
                assert_eq!(context.deadline(), Some(until));
                Err(ClaimError::invalid("disposal_native_root_close_failure"))
            },
        );
        assert_eq!(
            failed.err().map(|cause| cause.operation()),
            Some("disposal_native_root_close_failure")
        );
        assert!(current.file.is_none());
        assert!(!current.retired && !current.release_published);
        assert_eq!(current.root.as_ref().map(ProtectedStorageRoot::live_handles), Some(handles));
        assert_eq!(
            std::fs::read(&marker)
                .map_err(|cause| ClaimError::new("disposal_preserved_record", cause))?,
            original
        );
        assert!(dispose_unpublished(
            &mut current,
            &context,
            |_| {
                file_closes += 1;
                Err(ClaimError::invalid("disposal_file_closed_twice"))
            },
            |root| {
                root_closes += 1;
                assert_eq!(context.deadline(), Some(until));
                root.retire(&context)
            }
        )?);
        assert_eq!((file_closes, root_closes), (1, 2));
        assert!(current.root.is_none() && current.file.is_none() && current.retired);
        assert!(!current.release_published);
        assert_eq!(
            current.failure.as_ref().map(ClaimError::operation),
            Some("profile_unresolved_predecessor")
        );
        assert_eq!(
            std::fs::read(&marker)
                .map_err(|cause| ClaimError::new("disposal_final_record", cause))?,
            original
        );
        drop(current);
        for path in [marker, claim] {
            std::fs::remove_file(path)
                .map_err(|cause| ClaimError::new("disposal_owned_file_cleanup", cause))?;
        }
        std::fs::remove_dir(&profile)
            .map_err(|cause| ClaimError::new("disposal_owned_profile_cleanup", cause))?;
        std::fs::remove_dir(&directory)
            .map_err(|cause| ClaimError::new("disposal_owned_directory_cleanup", cause))?;
        Ok::<_, ClaimError>(())
    })?;
    let done = harness.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    harness.close(until)?;
    harness.join_execution_after_retirement()?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
