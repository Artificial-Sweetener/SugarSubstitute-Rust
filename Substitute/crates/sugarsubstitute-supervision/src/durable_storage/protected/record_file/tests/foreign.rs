// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject another real executor before any retained publication or cleanup effect.
use super::release::{generation, ordinary};
use super::*;
#[test]
fn fixed_record_foreign_executor_cannot_publish_or_retire_original_stage() -> TestResult {
    let mut old = generation(resolved().namespace, 1)?;
    old.state = crate::native_claims::RecordState::Released;
    let original = old.encode();
    let desired = generation(resolved().namespace, 2)?;
    let encoded = desired.encode();
    let mut fixture = PublicationFixture::new(ElectionRecordOrigin::Released(original))?;
    let mut foreign =
        PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
    let retained = Arc::new(Mutex::new(None));
    let fake = Fake::new(RecordRead::Present(original));
    let output = retained.clone();
    let external = fake.clone();
    let coordinator = fixture.entry.fake.clone();
    let record = desired.clone();
    let spec = fixture.entry.spec(Moment::from_nanos(100))?;
    let mut job = fixture.entry.client.submit_blocking(
        &fixture.entry.scope,
        spec,
        move |context| -> Result<(), ClaimError> {
            let mut fake = external;
            let inode = adopt(&output, &mut fake, &context)?;
            coordinator.observe_coordinator(inode);
            fake.fail_write(true);
            let slot = lock(&output);
            assert!(
                ordinary(
                    slot.as_ref().ok_or_else(|| ClaimError::invalid("foreign_fixture_owner"))?,
                    &record,
                    &mut fake,
                    &context
                )
                .is_err()
            );
            Ok(())
        },
    )?;
    let done = fixture.entry.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    let effects = fake.effect_count();
    let output = retained.clone();
    let external = fake.clone();
    let record = desired.clone();
    let spec = foreign.entry.spec(Moment::from_nanos(90))?;
    let mut job = foreign.entry.client.submit_blocking(
        &foreign.entry.scope,
        spec,
        move |context| -> Result<(), ClaimError> {
            let mut fake = external;
            let slot = lock(&output);
            let file = slot.as_ref().ok_or_else(|| ClaimError::invalid("foreign_fixture_owner"))?;
            assert!(ordinary(file, &record, &mut fake, &context).is_err());
            assert!(
                retirement::retire_with_boundary(file, Some(&encoded), &context, &mut fake)
                    .is_err()
            );
            assert_eq!(fake.effect_count(), effects);
            assert_eq!(fake.live(), 2);
            assert!(lock(&file.stage).is_some());
            assert!(lock(&file.publication).is_some());
            Ok(())
        },
    )?;
    let done = foreign.entry.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    finish(&mut foreign)?;
    let output = retained.clone();
    let external = fake.clone();
    let coordinator = fixture.entry.fake.clone();
    let spec = fixture.entry.spec(Moment::from_nanos(100))?;
    let mut job=fixture.entry.client.submit_blocking(&fixture.entry.scope,spec,move|context|->Result<(),ClaimError>{
        let mut fake=external;
        fake.fail_write(false);
        let mut slot=lock(&output);
        let file=slot.as_mut().ok_or_else(||ClaimError::invalid("foreign_cleanup_owner"))?;
        let actual=retirement::retire_with_boundary(file,Some(&encoded),&context,&mut fake)?;
        assert!(matches!(actual,PublicationRetirement::Preserved(RecordRead::Present(bytes)) if bytes==original));
        assert_eq!(fake.effect_count(),effects);
        file.retire_fixture([11;24],&mut fake,&context)?;
        coordinator.close_coordinator();
        *slot=None;
        Ok(())
    })?;
    let done = fixture.entry.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    assert_eq!(fake.live(), 0);
    finish(&mut fixture)?;
    Ok(())
}
