// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe fresh policy, external partial effects and exact checked cleanup causally.
use super::*;
#[test]
fn fixed_record_missing_and_empty_require_actual_sealed_fresh_publication() -> TestResult {
    for origin in [FreshRecordOrigin::Missing, FreshRecordOrigin::Empty] {
        let mut fixture = PublicationFixture::new(ElectionRecordOrigin::Fresh(origin))?;
        let proof = fixture.retained_proof()?;
        let coordinator = fixture.entry.fake.clone();
        let fake = Fake::new(match origin {
            FreshRecordOrigin::Missing => RecordRead::Missing,
            FreshRecordOrigin::Empty => RecordRead::Empty,
        });
        let external = fake.clone();
        let retained = Arc::new(Mutex::new(None));
        let owned = retained.clone();
        let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
        let mut job = fixture.entry.client.submit_blocking(
            &fixture.entry.scope,
            spec,
            move |context| -> Result<(), ClaimError> {
                let mut fake = fake;
                let inode = adopt(&owned, &mut fake, &context)?;
                coordinator.observe_coordinator(inode);
                let mut slot = lock(&owned);
                let file =
                    slot.as_mut().ok_or_else(|| ClaimError::invalid("fixture_held_missing"))?;
                assert!(
                    publication_core::publish_with_boundary(
                        file,
                        &[42; 128],
                        PublicationAuthority::Ordinary,
                        &context,
                        |_| Ok(()),
                        &mut fake
                    )
                    .is_err()
                );
                assert!(lock(&file.stage).is_none());
                assert!(lock(&fake.0).effects.is_empty());
                with_elected_publication(
                    &proof,
                    file,
                    inode,
                    &context,
                    &resolved(),
                    |authority, file| {
                        publication_core::publish_with_boundary(
                            file,
                            &[42; 128],
                            PublicationAuthority::ElectedCoordinator(authority),
                            &context,
                            |_| Ok(()),
                            &mut fake,
                        )
                    },
                )?;
                assert!(lock(&file.stage).is_none());
                assert_eq!(fake.live(), 1);
                file.retire_fixture(inode, &mut fake, &context)?;
                coordinator.close_coordinator();
                *slot = None;
                Ok(())
            },
        )?;
        let completion = fixture.entry.consume_job(&mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
        drop(completion.memory);
        drop(job);
        assert_eq!(external.live(), 0);
        assert!(lock(&retained).is_none());
        finish(&mut fixture)?;
    }
    Ok(())
}
#[test]
fn fixed_record_partial_outputs_and_rename_failure_keep_real_stage_and_generation() -> TestResult {
    for fault in 0..5 {
        let mut fixture =
            PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
        let proof = fixture.retained_proof()?;
        let coordinator = fixture.entry.fake.clone();
        let fake = Fake::new(RecordRead::Missing);
        let external = fake.clone();
        let retained = Arc::new(Mutex::new(None));
        let owned = retained.clone();
        let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
        let mut job = fixture.entry.client.submit_blocking(
            &fixture.entry.scope,
            spec,
            move |context| -> Result<(), ClaimError> {
                let mut fake = fake;
                let inode = adopt(&owned, &mut fake, &context)?;
                coordinator.observe_coordinator(inode);
                {
                    let mut disk = lock(&fake.0);
                    match fault {
                        0 => disk.faults.create = true,
                        1 => disk.faults.write = true,
                        2 => disk.faults.activate = true,
                        3 => disk.faults.flush = true,
                        4 => disk.faults.close = true,
                        _ => return Err(ClaimError::invalid("unknown_fault")),
                    };
                }
                let mut slot = lock(&owned);
                let file =
                    slot.as_mut().ok_or_else(|| ClaimError::invalid("fixture_held_missing"))?;
                let result = with_elected_publication(
                    &proof,
                    file,
                    inode,
                    &context,
                    &resolved(),
                    |authority, file| {
                        publication_core::publish_with_boundary(
                            file,
                            &[42; 128],
                            PublicationAuthority::ElectedCoordinator(authority),
                            &context,
                            |_| Ok(()),
                            &mut fake,
                        )
                    },
                );
                assert!(result.is_err());
                assert!(lock(&file.stage).is_some());
                assert!(fake.live() > 1);
                let before = lock(&fake.0).effects.len();
                assert!(
                    with_elected_publication(
                        &proof,
                        file,
                        inode,
                        &context,
                        &resolved(),
                        |authority, file| publication_core::publish_with_boundary(
                            file,
                            &[43; 128],
                            PublicationAuthority::ElectedCoordinator(authority),
                            &context,
                            |_| Ok(()),
                            &mut fake
                        )
                    )
                    .is_err()
                );
                assert_eq!(lock(&fake.0).effects.len(), before);
                lock(&fake.0).faults = Default::default();
                let outcome =
                    retirement::retire_with_boundary(file, Some(&[42; 128]), &context, &mut fake)?;
                if fault < 2 {
                    assert!(matches!(
                        outcome,
                        PublicationRetirement::Preserved(RecordRead::Missing)
                    ));
                } else {
                    assert!(matches!(outcome, PublicationRetirement::Activated(bytes) if bytes == [42; 128]));
                }
                assert!(lock(&file.stage).is_none());
                assert_eq!(fake.live(), 1);
                file.retire_fixture(inode, &mut fake, &context)?;
                coordinator.close_coordinator();
                *slot = None;
                Ok(())
            },
        )?;
        let completion = fixture.entry.consume_job(&mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
        drop(completion.memory);
        drop(job);
        assert_eq!(external.live(), 0);
        finish(&mut fixture)?;
    }
    Ok(())
}
