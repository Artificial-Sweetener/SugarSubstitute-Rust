// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe schema-one release and restart through sealed election and actual record custody.
mod lifecycle;
use super::*;
use crate::{
    coordinator_entry::fixture::Gate,
    native_claims::{Record, RecordState, permits_replacement, prepare_coordinator_release},
};
use lifecycle::interrupted;
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};

#[derive(Clone, Copy)]
enum Stop {
    CancelBeforeRename,
    AdmissionClosed,
    DeadlineExpiry,
}
#[derive(Clone, Copy)]
enum ReleaseFault {
    None,
    Flush,
    Close,
    ChangedInode,
}
pub(super) fn generation(namespace: NamespaceId, epoch: u8) -> Result<Record, ClaimError> {
    Ok(Record::coordinator(CoordinatorKey {
        namespace,
        epoch: CoordinatorEpoch::new([epoch; 16])
            .map_err(|cause| ClaimError::new("fixture_epoch", cause))?,
    }))
}
pub(super) fn ordinary(
    file: &HeldFile<128>,
    record: &Record,
    fake: &mut Fake,
    context: &JobContext,
) -> Result<(), ClaimError> {
    publication_core::publish_with_boundary(
        file,
        &record.encode(),
        PublicationAuthority::Ordinary,
        context,
        |old| {
            if let RecordRead::Present(bytes) = old
                && !permits_replacement(&Record::decode(bytes)?, record)
            {
                return Err(ClaimError::invalid("changed_claim_generation"));
            }
            Ok(())
        },
        fake,
    )
}
#[test]
fn fixed_record_release_cancelled_missing_and_empty_preserve_actual_origin() -> TestResult {
    for origin in [FreshRecordOrigin::Missing, FreshRecordOrigin::Empty] {
        let actual = interrupted(
            ElectionRecordOrigin::Fresh(origin),
            Stop::CancelBeforeRename,
            ReleaseFault::None,
            2,
        )?;
        assert!(matches!(
            (origin, actual),
            (FreshRecordOrigin::Missing, RecordRead::Missing)
                | (FreshRecordOrigin::Empty, RecordRead::Empty)
        ));
    }
    Ok(())
}
#[test]
fn fixed_record_release_closed_or_expired_fresh_activation_restarts_ordinary() -> TestResult {
    for stop in [Stop::AdmissionClosed, Stop::DeadlineExpiry] {
        let actual = interrupted(
            ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing),
            stop,
            ReleaseFault::None,
            2,
        )?;
        let RecordRead::Present(bytes) = actual else {
            return Err("released record missing".into());
        };
        assert_eq!(Record::decode(&bytes)?.state, RecordState::Released);
        let restarted = interrupted(
            ElectionRecordOrigin::Released(bytes),
            Stop::DeadlineExpiry,
            ReleaseFault::None,
            3,
        )?;
        let RecordRead::Present(actual) = restarted else {
            return Err("restart release missing".into());
        };
        assert_eq!(Record::decode(&actual)?.owner.epoch.bytes(), &[3; 16]);
        assert_ne!(actual, bytes);
    }
    Ok(())
}
#[test]
fn fixed_record_release_old_epoch_is_preserved_until_new_actual_activation() -> TestResult {
    let mut original = generation(resolved().namespace, 1)?;
    original.state = RecordState::Released;
    let bytes = original.encode();
    let actual = interrupted(
        ElectionRecordOrigin::Released(bytes),
        Stop::CancelBeforeRename,
        ReleaseFault::None,
        2,
    )?;
    assert!(matches!(actual, RecordRead::Present(actual) if actual == bytes));
    let actual = interrupted(
        ElectionRecordOrigin::Released(bytes),
        Stop::DeadlineExpiry,
        ReleaseFault::None,
        2,
    )?;
    let RecordRead::Present(actual) = actual else {
        return Err("new release missing".into());
    };
    assert_eq!(Record::decode(&actual)?.owner.epoch.bytes(), &[2; 16]);
    assert_ne!(actual, bytes);
    Ok(())
}
#[test]
fn fixed_record_release_partial_flush_and_close_settle_same_activation() -> TestResult {
    for fault in [ReleaseFault::Flush, ReleaseFault::Close] {
        let actual = interrupted(
            ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing),
            Stop::AdmissionClosed,
            fault,
            2,
        )?;
        let RecordRead::Present(bytes) = actual else {
            return Err("release missing".into());
        };
        assert_eq!(Record::decode(&bytes)?.state, RecordState::Released);
    }
    Ok(())
}
#[test]
fn fixed_record_release_same_bytes_changed_inode_preserves_stage_before_proof() -> TestResult {
    let actual = interrupted(
        ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing),
        Stop::AdmissionClosed,
        ReleaseFault::ChangedInode,
        2,
    )?;
    let RecordRead::Present(bytes) = actual else {
        return Err("release missing".into());
    };
    assert_eq!(Record::decode(&bytes)?.state, RecordState::Released);
    Ok(())
}

#[test]
fn fixed_record_release_corrupt_or_foreign_observation_never_publishes_released() -> TestResult {
    let own = generation(resolved().namespace, 2)?;
    for bytes in [[42; 128], generation(resolved().namespace, 3)?.encode()] {
        let mut fixture =
            PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
        let fake = Fake::new(RecordRead::Present(bytes));
        let external = fake.clone();
        let owned = Arc::new(Mutex::new(None));
        let retained = owned.clone();
        let coordinator = fixture.entry.fake.clone();
        let expected_record = own.clone();
        let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
        let mut job = fixture.entry.client.submit_blocking(
            &fixture.entry.scope, spec,
            move |context| -> Result<(), ClaimError> {
                let mut fake = fake;
                let inode = adopt(&owned, &mut fake, &context)?;
                coordinator.observe_coordinator(inode);
                let mut slot = lock(&owned);
                let file = slot.as_mut().ok_or_else(|| ClaimError::invalid("held_missing"))?;
                let actual = retirement::retire_with_boundary(file, Some(&expected_record.encode()), &context, &mut fake)?;
                let mut expected = Some(expected_record.clone());
                assert!(prepare_coordinator_release(actual, &mut expected, None).is_err());
                assert_eq!(expected.as_ref().map(Record::encode), Some(expected_record.encode()));
                assert!(lock(&fake.0).effects.is_empty());
                assert!(matches!(fake.read_shape(file, None, &mut || Ok(()), &crate::protected_storage::exclusion::operation::Operation::begin(file)?)?, RecordRead::Present(actual) if actual == bytes));
                file.retire_fixture(inode, &mut fake, &context)?;
                coordinator.close_coordinator();
                *slot = None;
                Ok(())
            },
        )?;
        let completion = fixture.entry.consume_job(&mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
        drop(completion);
        drop(job);
        assert!(lock(&retained).is_none());
        assert_eq!(external.live(), 0);
        finish(&mut fixture)?;
    }
    Ok(())
}
