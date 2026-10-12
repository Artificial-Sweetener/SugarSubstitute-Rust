// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve activated custody when actual path or target inode changes during cleanup.
use super::*;
#[test]
fn fixed_record_changed_native_path_or_same_bytes_new_inode_cannot_prove_activation() -> TestResult
{
    for changed_path in [false, true] {
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
                let mut slot = lock(&owned);
                let file = slot.as_mut().ok_or_else(|| ClaimError::invalid("held_missing"))?;
                lock(&fake.0).faults.activate = true;
                assert!(
                    with_fresh_publication(
                        &proof,
                        file,
                        inode,
                        &context,
                        &resolved(),
                        |authority, file| publication_core::publish_with_boundary(
                            file,
                            &[42; 128],
                            PublicationAuthority::FreshCoordinator(authority),
                            &context,
                            |_| Ok(()),
                            &mut fake
                        )
                    )
                    .is_err()
                );
                lock(&fake.0).faults.activate = false;
                if changed_path {
                    lock(&fake.0).faults.changed_path = true;
                } else {
                    fake.replace_target_identity([77; 24]);
                }
                let effects = lock(&fake.0).effects.len();
                assert!(
                    retirement::retire_with_boundary(file, Some(&[42; 128]), &context, &mut fake)
                        .is_err()
                );
                assert!(lock(&file.stage).is_some());
                assert!(fake.live() > 1);
                assert!(file.retire_fixture(inode, &mut fake, &context).is_err());
                assert_eq!(lock(&fake.0).effects.len(), effects);
                lock(&fake.0).faults.changed_path = false;
                fake.replace_target_identity([12; 24]);
                let settled =
                    retirement::retire_with_boundary(file, Some(&[42; 128]), &context, &mut fake)?;
                assert!(
                    matches!(settled,PublicationRetirement::Activated(bytes) if bytes==[42;128])
                );
                assert_eq!(fake.live(), 1);
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
        assert_eq!(external.live(), 0);
        finish(&mut fixture)?;
    }
    Ok(())
}
