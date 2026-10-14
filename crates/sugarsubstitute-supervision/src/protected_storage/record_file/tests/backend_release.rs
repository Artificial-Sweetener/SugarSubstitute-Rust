// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive backend release from actual staging and checked retirement observations.
use super::*;
use super::{release::ordinary, role_fixture::with_record};
use crate::native_claims::{Record, RecordState, prepare_backend_release};
use sugarsubstitute_contracts::lifecycle::{ResourceGeneration, ResourceId, ResourceKey};
fn generation(value: u64) -> Result<Record, ClaimError> {
    let owner = super::release::generation(resolved().namespace, 2)?.owner;
    Ok(Record::backend(
        ResourceKey {
            owner,
            id: ResourceId::new(1)
                .map_err(|cause| ClaimError::new("backend_fixture_resource", cause))?,
            generation: ResourceGeneration::new(value)
                .map_err(|cause| ClaimError::new("backend_fixture_generation", cause))?,
        },
        [3; 32],
    ))
}
#[test]
fn fixed_record_backend_release_preserves_absence_empty_original_or_unknown_bytes() -> TestResult {
    for case in 0..5 {
        let mut old = generation(1)?;
        old.state = RecordState::Released;
        let held = generation(2)?;
        let original = old.encode();
        let source = match case {
            0 => None,
            1 => Some(Vec::new()),
            3 => Some(vec![0; 128]),
            4 => Some(generation(3)?.encode().to_vec()),
            _ => Some(original.to_vec()),
        };
        with_record::<128>(source, move |file, fake, context| {
            let mut expected = Some(held.clone());
            if case == 2 {
                fake.fail_write(true);
                assert!(ordinary(file, &held, fake, context).is_err());
                fake.fail_write(false);
            }
            let effects = fake.effect_count();
            let actual =
                retirement::retire_with_boundary(file, Some(&held.encode()), context, fake)?;
            let decision = prepare_backend_release(actual, &mut expected, Some(&original));
            if case < 3 {
                assert!(!decision?);
            } else {
                assert!(decision.is_err());
            }
            assert_eq!(expected.as_ref().map(Record::encode), Some(held.encode()));
            assert_eq!(fake.effect_count(), effects);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn fixed_record_backend_held_pending_owned_release_uses_new_inode_then_settles_once() -> TestResult
{
    for state in [RecordState::Held, RecordState::LaunchPending, RecordState::Owned] {
        for close in [false, true] {
            let mut old = generation(1)?;
            old.state = RecordState::Released;
            let original = old.encode();
            let mut held = generation(2)?;
            held.state = state;
            held.process = if state == RecordState::Owned { 42 } else { 0 };
            with_record::<128>(Some(original.to_vec()), move |file, fake, context| {
                ordinary(file, &held, fake, context)?;
                let held_inode = fake.target_identity();
                let mut expected = Some(held.clone());
                let actual =
                    retirement::retire_with_boundary(file, Some(&held.encode()), context, fake)?;
                assert!(prepare_backend_release(actual, &mut expected, Some(&original))?);
                let released = expected
                    .as_ref()
                    .ok_or_else(|| ClaimError::invalid("backend_expected_release"))?
                    .clone();
                assert_eq!(released.state, RecordState::Released);
                assert_eq!(released.process, 0);
                fake.fail_flush(!close);
                fake.fail_activated_close(close);
                assert!(ordinary(file, &released, fake, context).is_err());
                let released_inode = fake.target_identity();
                assert_ne!(released_inode, held_inode);
                let effects = fake.effect_count();
                fake.fail_flush(false);
                fake.fail_close(false);
                fake.fail_activated_close(false);
                let actual = retirement::retire_with_boundary(
                    file,
                    Some(&released.encode()),
                    context,
                    fake,
                )?;
                assert!(
                    matches!(&actual,PublicationRetirement::Activated(bytes) if *bytes==released.encode())
                );
                assert!(!prepare_backend_release(actual, &mut expected, Some(&original))?);
                assert_eq!(fake.target_identity(), released_inode);
                assert_eq!(fake.effect_count(), effects);
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[test]
fn rejected_backend_acquisition_retires_own_outputs_without_rewriting_predecessor() -> TestResult {
    for state in
        [Some(RecordState::Held), Some(RecordState::LaunchPending), Some(RecordState::Owned), None]
    {
        let mut predecessor = generation(1)?;
        if let Some(state) = state {
            predecessor.state = state;
            predecessor.process = if state == RecordState::Owned { 42 } else { 0 };
        }
        let original = state.map_or([0; 128], |_| predecessor.encode());
        with_record::<128>(Some(original.to_vec()), move |file, fake, context| {
            let mut expected = None;
            let inode = fake.target_identity();
            let effects = fake.effect_count();
            let retired = retirement::retire_with_boundary(file, None, context, fake)?;
            assert!(!prepare_backend_release(retired, &mut expected, None)?);
            assert!(expected.is_none());
            assert_eq!(fake.target_identity(), inode);
            assert_eq!(fake.effect_count(), effects);
            let operation = crate::protected_storage::exclusion::operation::Operation::begin(file)?;
            let observed = fake.read_shape(file, None, &mut || Ok(()), &operation)?;
            assert!(matches!(observed, RecordRead::Present(bytes) if bytes == original));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn activated_backend_generation_requires_its_original_expected_owner_during_retirement()
-> TestResult {
    let held = generation(2)?;
    let mut predecessor = generation(1)?;
    predecessor.state = RecordState::Released;
    with_record::<128>(Some(predecessor.encode().to_vec()), move |file, fake, context| {
        fake.fail_activated_close(true);
        assert!(ordinary(file, &held, fake, context).is_err());
        fake.fail_activated_close(false);
        fake.fail_close(false);
        let inode = fake.target_identity();
        let effects = fake.effect_count();
        assert!(retirement::retire_with_boundary(file, None, context, fake).is_err());
        assert_eq!(fake.target_identity(), inode);
        assert_eq!(fake.effect_count(), effects);
        let actual = retirement::retire_with_boundary(file, Some(&held.encode()), context, fake)?;
        assert!(
            matches!(&actual, PublicationRetirement::Activated(bytes) if *bytes == held.encode())
        );
        let mut missing_owner = None;
        assert!(prepare_backend_release(actual, &mut missing_owner, None).is_err());
        assert!(missing_owner.is_none());
        let operation = crate::protected_storage::exclusion::operation::Operation::begin(file)?;
        let observed = fake.read_shape(file, None, &mut || Ok(()), &operation)?;
        assert!(matches!(observed, RecordRead::Present(bytes) if bytes == held.encode()));
        Ok(())
    })
}
