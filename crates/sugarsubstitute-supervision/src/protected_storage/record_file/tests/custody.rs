// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove actual external claim custody survives returned and partial owner drops.
use super::*;
use crate::protected_storage::exclusion::fixture::ExclusionFixtureBoundary;
#[test]
fn fixed_record_owner_drop_keeps_actual_external_claim_until_exact_checked_close() -> TestResult {
    for partial in [false, true] {
        let mut fixture =
            PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
        let fake = Fake::new(RecordRead::Missing);
        let external = fake.clone();
        let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
        let mut job = fixture.entry.client.submit_blocking(
            &fixture.entry.scope,
            spec,
            move |context| -> Result<(), ClaimError> {
                let mut fake = fake;
                let memory = Arc::new(
                    context
                        .memory()
                        .claim(1024)
                        .map_err(|cause| ClaimError::new("fixture_claim", cause))?,
                );
                let actual = Arc::downgrade(&memory);
                lock(&fake.0).faults.adopt = partial;
                let mut retained: Option<HeldFile<128>> = None;
                let result = HeldFile::adopt_fixture_into(
                    &mut retained,
                    path(),
                    &resolved().account_sid,
                    memory,
                    &mut fake,
                    &context,
                );
                if partial {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result?, [11; 24]);
                    assert!(
                        retained
                            .as_mut()
                            .ok_or_else(|| ClaimError::invalid("held_missing"))?
                            .close()
                            .is_err()
                    );
                }
                drop(retained);
                assert_eq!(fake.live(), 1);
                assert_eq!(actual.upgrade().map(|claim| claim.bytes()), Some(1024));
                assert!(fake.close([99; 24], &context).is_err());
                lock(&fake.0).faults.close = true;
                let failed = if partial {
                    fake.retire_unreturned(&context)
                } else {
                    fake.close([11; 24], &context)
                };
                assert!(failed.is_err());
                assert_eq!(fake.live(), 1);
                assert!(actual.upgrade().is_some());
                lock(&fake.0).faults.close = false;
                if partial {
                    fake.retire_unreturned(&context)?;
                } else {
                    fake.close([11; 24], &context)?;
                }
                assert_eq!(fake.live(), 0);
                assert!(actual.upgrade().is_none());
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
