// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise current-owner disposal through admitted jobs and actual record custody.
use super::*;
use crate::{
    coordinator_entry::fixture::{Harness, resolved},
    protected_storage::record_file::{
        publication::publish_with_boundary, retirement::retire_with_boundary, tests::boundary::Fake,
    },
};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobOutcome},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[cfg(test)]
mod native_root;
fn held() -> Result<record::Record, ClaimError> {
    Ok(record::Record {
        account: [3; 32],
        folder: [4; 24],
        state: record::State::Held,
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])
                    .map_err(|cause| ClaimError::new("disposal_epoch", cause))?,
            },
            instance: InstanceId::new(1)
                .map_err(|cause| ClaimError::new("disposal_instance", cause))?,
            generation: RunGeneration::new(1)
                .map_err(|cause| ClaimError::new("disposal_generation", cause))?,
        },
    })
}
fn exercise(case: u8) -> TestResult {
    let mut harness = Harness::new()?;
    let spec = harness.spec(Moment::from_nanos(15_000_000_000))?;
    let original = held()?;
    let fake = Fake::with_bytes(Some(&original.encode()));
    let outside = fake.clone();
    let mut job = harness.client.submit_blocking(&harness.scope, spec, move |context| {
        let mut fake = fake;
        let mut state = NativeState::new();
        state.retired = false;
        state.failure = Some(ClaimError::invalid("profile_unresolved_predecessor"));
        let memory = Arc::new(
            context
                .memory()
                .claim(1024)
                .map_err(|cause| ClaimError::new("disposal_fixture_memory", cause))?,
        );
        let native = resolved();
        let inode = HeldFile::adopt_fixture_into(
            &mut state.file,
            native.directory.join("profile.record"),
            &native.account_sid,
            memory,
            &mut fake,
            &context,
        )?;
        let target = fake.target_identity();
        if case == 1 {
            fake.fail_close(true);
            let result = dispose_unpublished(
                &mut state,
                &context,
                |file| file.retire_fixture(inode, &mut fake, &context),
                |_| Err(ClaimError::invalid("unexpected_profile_root")),
            );
            assert!(result.is_err());
            assert!(state.file.as_ref().is_some_and(HeldFile::is_locked));
            assert!(!state.retired);
            assert_eq!(fake.live(), 1);
            fake.fail_close(false);
        } else if case == 2 {
            let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("disposal_file"))?;
            fake.fail_write(true);
            assert!(
                publish_with_boundary(
                    file,
                    &original.encode(),
                    record_file::PublicationAuthority::Ordinary,
                    &context,
                    |_| Ok(()),
                    &mut fake,
                )
                .is_err()
            );
            let effects = fake.effect_count();
            assert!(
                dispose_unpublished(
                    &mut state,
                    &context,
                    |_| Err(ClaimError::invalid("must_not_close_partial_write")),
                    |_| Err(ClaimError::invalid("must_not_close_profile_root")),
                )
                .is_err()
            );
            assert_eq!(fake.effect_count(), effects);
            let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("disposal_file"))?;
            assert!(lock(&file.stage).is_some());
            fake.fail_write(false);
            retire_with_boundary(file, Some(&original.encode()), &context, &mut fake)?;
        } else if case == 3 {
            state.publication_attempted = true;
            assert!(!dispose_unpublished(
                &mut state,
                &context,
                |_| Err(ClaimError::invalid("must_not_close_issued_publication")),
                |_| Err(ClaimError::invalid("must_not_close_profile_root")),
            )?);
            state.publication_attempted = false;
        }
        let effects = fake.effect_count();
        assert!(dispose_unpublished(
            &mut state,
            &context,
            |file| file.retire_fixture(inode, &mut fake, &context),
            |_| Err(ClaimError::invalid("unexpected_profile_root")),
        )?);
        assert!(state.file.is_none());
        assert!(state.record.is_none());
        assert!(state.retired);
        assert!(!state.release_published);
        assert_eq!(
            state.failure.as_ref().map(ClaimError::operation),
            Some("profile_unresolved_predecessor")
        );
        assert_eq!(fake.target_identity(), target);
        assert_eq!(fake.effect_count(), effects);
        assert_eq!(fake.live(), 0);
        assert!(size_of::<NativeState>() < 512 * 1024);
        Ok::<_, ClaimError>(())
    })?;
    let done = harness.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    assert_eq!(outside.live(), 0);
    harness.close(Moment::from_nanos(15_000_000_000))?;
    harness.join_execution_after_retirement()?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn unpublished_acquisition_disposes_only_current_guards_without_predecessor_release() -> TestResult
{
    exercise(0)
}
#[test]
fn unpublished_close_failure_retains_actual_output_until_checked_same_cutoff_close() -> TestResult {
    exercise(1)
}
#[test]
fn unknown_partial_publication_is_not_disposed_as_never_issued() -> TestResult {
    exercise(2)
}
#[test]
fn issued_publication_remains_with_the_original_record_release_owner() -> TestResult {
    exercise(3)
}
