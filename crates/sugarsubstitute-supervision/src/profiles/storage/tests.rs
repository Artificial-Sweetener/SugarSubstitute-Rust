// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive profile release facts from actual shared publication and retirement.
use super::*;
use crate::durable_storage::protected::record_file::{
    publication::publish_with_boundary,
    retirement::retire_with_boundary,
    tests::{boundary::Fake, role_fixture::with_record},
};
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn profile_custody_inline_metadata_remains_inside_original_funded_partition() {
    let metadata = size_of::<NativeState>() + size_of::<super::super::custody::Release>();
    let family = crate::native_process::Family::storage_bytes();
    let prepared = crate::registry::profile::prepared::PreparedDesktop::storage_bytes();
    assert!(metadata + prepared + 2 * 65532 + 2 * 8192 < 512 * 1024);
    assert!(
        crate::registry::profile::prepared::PreparedDesktopCarrier::metadata_bytes() < 512 * 1024
    );
    assert!(family < crate::registry_input::FAMILY_STORAGE_BYTES as usize);
    eprintln!(
        "profile_layout NativeState={} PreparedDesktop={} Family={} profile_operation={}",
        size_of::<NativeState>(),
        prepared,
        family,
        crate::profiles::PROFILE_OPERATION_BYTES
    );
}
pub(super) fn generation(value: u64) -> Result<record::Record, ClaimError> {
    Ok(record::Record {
        account: [3; 32],
        folder: [4; 24],
        state: record::State::Held,
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])
                    .map_err(|cause| ClaimError::new("profile_fixture_epoch", cause))?,
            },
            instance: InstanceId::new(1)
                .map_err(|cause| ClaimError::new("profile_fixture_instance", cause))?,
            generation: RunGeneration::new(value)
                .map_err(|cause| ClaimError::new("profile_fixture_generation", cause))?,
        },
    })
}
fn publish(
    file: &HeldFile<160>,
    record: record::Record,
    fake: &mut Fake,
    context: &JobContext,
) -> Result<(), ClaimError> {
    publish_with_boundary(
        file,
        &record.encode(),
        record_file::PublicationAuthority::Ordinary,
        context,
        |old| match old {
            record_file::RecordRead::Present(bytes)
                if record::Record::decode(bytes)?.permits_replacement(record) =>
            {
                Ok(())
            }
            _ => Err(ClaimError::invalid("profile_fixture_predecessor")),
        },
        fake,
    )
}
#[test]
fn fixed_record_profile_release_preserves_absence_empty_original_or_unknown_bytes() -> TestResult {
    for case in 0..5 {
        let mut old = generation(1)?;
        old.state = record::State::Released;
        let held = generation(2)?;
        let mut source = match case {
            0 => None,
            1 => Some(Vec::new()),
            _ => Some(old.encode().to_vec()),
        };
        if case == 3 {
            source = Some(vec![0; 160]);
        }
        if case == 4 {
            source = Some(generation(3)?.encode().to_vec());
        }
        let original = old.encode();
        with_record::<160>(source, move |file, fake, context| {
            let mut expected = Some(held);
            if case == 2 {
                fake.fail_write(true);
                assert!(publish(file, held, fake, context).is_err());
                fake.fail_write(false);
            }
            let effects = fake.effect_count();
            let observed = retire_with_boundary(file, Some(&held.encode()), context, fake)?;
            let decision = prepare_release(observed, &mut expected, Some(&original));
            if case < 3 {
                assert!(!decision?);
            } else {
                assert!(decision.is_err());
            }
            assert_eq!(expected, Some(held));
            assert_eq!(fake.effect_count(), effects);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn fixed_record_profile_partial_released_activation_settles_exact_inode_once() -> TestResult {
    for close in [false, true] {
        let mut old = generation(1)?;
        old.state = record::State::Released;
        let held = generation(2)?;
        let original = old.encode();
        with_record::<160>(Some(original.to_vec()), move |file, fake, context| {
            publish(file, held, fake, context)?;
            let held_inode = fake.target_identity();
            let mut expected = Some(held);
            let observed = retire_with_boundary(file, Some(&held.encode()), context, fake)?;
            assert!(prepare_release(observed, &mut expected, Some(&original))?);
            let released =
                expected.ok_or_else(|| ClaimError::invalid("profile_expected_release"))?;
            assert_eq!(released.state, record::State::Released);
            fake.fail_flush(!close);
            fake.fail_activated_close(close);
            assert!(publish(file, released, fake, context).is_err());
            let actual_inode = fake.target_identity();
            assert_ne!(actual_inode, held_inode);
            let effects = fake.effect_count();
            fake.fail_flush(false);
            fake.fail_close(false);
            fake.fail_activated_close(false);
            fake.replace_target_identity([77; 24]);
            assert!(retire_with_boundary(file, Some(&released.encode()), context, fake).is_err());
            assert!(lock(&file.stage).is_some());
            assert_eq!(expected, Some(released));
            assert_eq!(fake.effect_count(), effects);
            fake.replace_target_identity(actual_inode);
            let observed = retire_with_boundary(file, Some(&released.encode()), context, fake)?;
            assert!(
                matches!(&observed,record_file::PublicationRetirement::Activated(bytes) if *bytes==released.encode())
            );
            assert!(!prepare_release(observed, &mut expected, Some(&original))?);
            assert_eq!(fake.target_identity(), actual_inode);
            assert_eq!(fake.effect_count(), effects);
            Ok(())
        })?;
    }
    Ok(())
}
