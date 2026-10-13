// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain interrupted schema generation through actual finite cleanup and release.
use super::*;
pub(super) fn interrupted(
    origin: ElectionRecordOrigin,
    stop: Stop,
    fault: ReleaseFault,
    epoch: u8,
) -> Result<RecordRead<128>, Box<dyn Error>> {
    let mut fixture = PublicationFixture::new(origin)?;
    let proof = fixture.retained_proof()?;
    let record = generation(proof.namespace, epoch)?;
    let encoded = record.encode();
    let expected = Arc::new(Mutex::new(Some(record.clone())));
    let observed = Arc::new(Mutex::new(None));
    let original = match origin {
        ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing) => RecordRead::Missing,
        ElectionRecordOrigin::Fresh(FreshRecordOrigin::Empty) => RecordRead::Empty,
        ElectionRecordOrigin::Released(bytes)
        | ElectionRecordOrigin::RecoveredHeld { record: bytes, .. } => RecordRead::Present(bytes),
    };
    let fake = Fake::new(original);
    let external = fake.clone();
    let gate = Arc::new(Gate::default());
    {
        let mut disk = lock(&fake.0);
        match stop {
            Stop::DeadlineExpiry => {
                disk.expiry = Some((
                    Effect::Activate,
                    fixture.entry.clock.clone(),
                    Moment::from_nanos(15_000_000_000),
                ));
            }
            _ => {
                disk.gate = Some((
                    if matches!(stop, Stop::CancelBeforeRename) {
                        Effect::Create
                    } else {
                        Effect::Activate
                    },
                    gate.clone(),
                ));
            }
        }
    }
    let retained = Arc::new(Mutex::new(None));
    let owned = retained.clone();
    let coordinator = fixture.entry.fake.clone();
    let admission_gate = gate.clone();
    let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
    let mut job = fixture.entry.client.submit_blocking(
        &fixture.entry.scope,
        spec,
        move |context| -> Result<(), ClaimError> {
            let mut fake = fake;
            let inode = adopt(&owned, &mut fake, &context)?;
            coordinator.observe_coordinator(inode);
            let mut slot = lock(&owned);
            let file = slot.as_mut().ok_or_else(|| ClaimError::invalid("held_missing"))?;
            match origin {
                ElectionRecordOrigin::Fresh(_) | ElectionRecordOrigin::RecoveredHeld { .. } => {
                    with_elected_publication(
                        &proof,
                        file,
                        inode,
                        &context,
                        &resolved(),
                        |authority, file| {
                            publication_core::publish_with_boundary(
                                file,
                                &encoded,
                                PublicationAuthority::ElectedCoordinator(authority),
                                &context,
                                |old| authority.validate(file, old),
                                &mut fake,
                            )
                        },
                    )
                }
                ElectionRecordOrigin::Released(_) => {
                    assert!(
                        with_elected_publication(
                            &proof,
                            file,
                            inode,
                            &context,
                            &resolved(),
                            |_, _| Ok(())
                        )
                        .is_err()
                    );
                    if matches!(stop, Stop::CancelBeforeRename) {
                        admission_gate.enter()?;
                        if context.cancellation_requested() {
                            return Err(ClaimError::invalid(
                                "fixture_original_admission_cancelled",
                            ));
                        }
                    }
                    ordinary(file, &record, &mut fake, &context)?;
                    if context.cancellation_requested()
                        || context.deadline().is_none_or(|until| context.now() >= until)
                    {
                        return Err(ClaimError::invalid("fixture_original_admission_closed"));
                    }
                    Ok(())
                }
            }
        },
    )?;
    match stop {
        Stop::CancelBeforeRename => {
            gate.wait()?;
            job.cancel();
            gate.release();
        }
        Stop::AdmissionClosed => {
            gate.wait()?;
            fixture.close_claim_admission();
            gate.release();
        }
        Stop::DeadlineExpiry => {}
    }
    let completion = fixture.entry.consume_job(&mut job)?;
    match stop {
        Stop::CancelBeforeRename => {
            assert!(matches!(completion.outcome, JobOutcome::Cancelled))
        }
        Stop::DeadlineExpiry => assert!(matches!(completion.outcome, JobOutcome::TimedOut)),
        Stop::AdmissionClosed => assert!(matches!(completion.outcome, JobOutcome::Failure(_))),
    }
    drop(completion);
    drop(job);
    let effects = lock(&external.0).effects.len();
    {
        let mut disk = lock(&external.0);
        disk.gate = None;
        disk.expiry = None;
    }
    let owned = retained.clone();
    let output = observed.clone();
    let cleanup_fake = external.clone();
    let coordinator = fixture.entry.fake.clone();
    let close_gate = Arc::new(Gate::default());
    if matches!(fault, ReleaseFault::Close) {
        lock(&external.0).gate = Some((Effect::Activate, close_gate.clone()));
    }
    let now = fixture.entry.clock.now();
    let spec = fixture.entry.spec(Moment::from_nanos(now.as_nanos() + 15_000_000_000))?;
    let mut cleanup = fixture.entry.client.submit_blocking(
        &fixture.entry.scope,
        spec,
        move |context| -> Result<(), ClaimError> {
            let mut fake = cleanup_fake;
            let mut slot = lock(&owned);
            let file = slot.as_mut().ok_or_else(|| ClaimError::invalid("cleanup_owner_missing"))?;
            if matches!(fault, ReleaseFault::ChangedInode) {
                fake.replace_target_identity([77; 24]);
                assert!(retirement::retire_with_boundary(file, Some(&encoded), &context, &mut fake).is_err());
                assert!(lock(&file.stage).is_some());
                assert!(fake.live() > 1);
                assert_eq!(lock(&fake.0).effects.len(), effects);
                assert_eq!(lock(&expected).as_ref().map(Record::encode), Some(encoded));
                fake.replace_target_identity([12; 24]);
            }
            let actual = retirement::retire_with_boundary(file, Some(&encoded), &context, &mut fake)?;
            assert_eq!(lock(&fake.0).effects.len(), effects);
            let original_released = match origin {
                ElectionRecordOrigin::Released(ref bytes) => Some(crate::native_claims::CoordinatorPredecessor::Released(bytes)),
                ElectionRecordOrigin::RecoveredHeld {ref record,..} => Some(crate::native_claims::CoordinatorPredecessor::RecoveredHeld(record)),
                _ => None,
            };
            let mut expected = lock(&expected);
            if prepare_coordinator_release(actual, &mut expected, original_released)? {
                let released = expected.as_ref().ok_or_else(|| ClaimError::invalid("released_missing"))?;
                assert_eq!(released.state, RecordState::Released);
                assert_eq!(released.owner.epoch.bytes(), &[epoch; 16]);
                if matches!(fault, ReleaseFault::Flush) {
                    lock(&fake.0).faults.flush = true;
                }
                let result = ordinary(file, released, &mut fake, &context);
                if matches!(fault, ReleaseFault::Flush | ReleaseFault::Close) {
                    assert!(result.is_err());
                    assert!(lock(&file.stage).is_some());
                    assert_eq!(fake.live(), 2);
                    let activated = lock(&fake.0).effects.len();
                    let encoded_release = released.encode();
                    lock(&fake.0).faults = Default::default();
                    let actual = retirement::retire_with_boundary(file, Some(&encoded_release), &context, &mut fake)?;
                    assert!(matches!(actual, PublicationRetirement::Activated(bytes) if bytes == encoded_release));
                    assert!(!prepare_coordinator_release(actual, &mut expected, original_released)?);
                    assert_eq!(lock(&fake.0).effects.len(), activated);
                } else {
                    result?;
                }
            }
            *lock(&output) = Some(fake.read_shape(file, None, &mut || Ok(()))?);
            file.retire_fixture([11; 24], &mut fake, &context)?;
            coordinator.close_coordinator();
            *slot = None;
            Ok(())
        },
    )?;
    if matches!(fault, ReleaseFault::Close) {
        close_gate.wait()?;
        lock(&external.0).faults.close = true;
        close_gate.release();
    }
    let completion = fixture.entry.consume_job(&mut cleanup)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
    drop(completion);
    drop(cleanup);
    assert_eq!(external.live(), 0);
    finish(&mut fixture)?;
    let result = lock(&observed).take().ok_or("actual result missing")?;
    Ok(result)
}
