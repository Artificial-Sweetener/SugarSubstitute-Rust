// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original context retention across actual independent admitted jobs.
use super::release::{generation, ordinary};
use super::*;
use sugarsubstitute_contracts::execution::{ScopeId, ScopeSpec};
#[test]
fn fixed_record_normal_retry_narrows_without_renewal_or_authority_transfer() -> TestResult {
    let mut old = generation(resolved().namespace, 1)?;
    old.state = crate::native_claims::RecordState::Released;
    let original = old.encode();
    let record = generation(resolved().namespace, 2)?;
    let encoded = record.encode();
    let mut fixture = PublicationFixture::new(ElectionRecordOrigin::Released(original))?;
    let other = fixture
        .entry
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let fake = Fake::new(RecordRead::Present(original));
    let retained = Arc::new(Mutex::new(None));
    for phase in 1..=11 {
        let deadline = match phase {
            1 => 100,
            3 => 95,
            6 => 16_000_000_000,
            9 => 91,
            _ => 90,
        };
        let spec = fixture.entry.spec(Moment::from_nanos(deadline))?;
        let scope = if phase == 4 || phase == 10 { &other } else { &fixture.entry.scope };
        let output = retained.clone();
        let external = fake.clone();
        let record = record.clone();
        let coordinator = fixture.entry.fake.clone();
        let mut job=fixture.entry.client.submit_blocking(scope,spec,move |context| -> Result<(),ClaimError> {
            let mut fake=external;
            if phase==1 {
                let inode=adopt(&output,&mut fake,&context)?;
                coordinator.observe_coordinator(inode);
            }
            let mut slot=lock(&output);
            let file=slot.as_mut().ok_or_else(||ClaimError::invalid("retry_owner_missing"))?;
            match phase {
                1=>{lock(&fake.0).faults.create=true;assert!(ordinary(file,&record,&mut fake,&context).is_err());}
                2=>{
                    lock(&fake.0).faults=Default::default();
                    lock(&fake.0).faults.write=true;
                    assert!(ordinary(file,&record,&mut fake,&context).is_err());
                    assert_eq!(lock(&fake.0).effects,[Effect::Create,Effect::Write]);
                }
                3|4|8=>{
                    let result=ordinary(file,&record,&mut fake,&context);
                    assert!(result.is_err());
                    assert_eq!(lock(&fake.0).effects.len(),2);
                    assert!(lock(&file.stage).is_some());
                }
                5=>{
                    let next=generation(resolved().namespace,3)?;
                    assert!(ordinary(file,&next,&mut fake,&context).is_err());
                    assert_eq!(lock(&file.stage).as_ref().map(|stage|stage.encoded),Some(encoded));
                    assert_eq!(lock(&fake.0).effects.len(),2);
                }
                6|7|9|10=>{
                    lock(&fake.0).faults.close=phase==7;
                    let result=retirement::retire_with_boundary(file,Some(&encoded),&context,&mut fake);
                    assert!(result.is_err());
                    assert_eq!(fake.live(),2);
                    assert!(lock(&file.publication).is_some());
                    assert!(lock(&file.stage).is_some());
                    assert_eq!(lock(&fake.0).effects.len(),2);
                }
                11=>{
                    lock(&fake.0).faults=Default::default();
                    let result=retirement::retire_with_boundary(file,Some(&encoded),&context,&mut fake)?;
                    assert!(matches!(result,PublicationRetirement::Preserved(RecordRead::Present(bytes)) if bytes==original));
                    assert_eq!(lock(&fake.0).effects.len(),2);
                    file.retire_fixture([11;24],&mut fake,&context)?;
                    coordinator.close_coordinator();
                    *slot=None;
                }
                _=>return Err(ClaimError::invalid("retry_phase")),
            }
            Ok(())
        })?;
        let done = fixture.entry.consume_job(&mut job)?;
        assert!(matches!(done.outcome, JobOutcome::Success(())), "phase{phase}: {done:?}");
        drop((done, job));
        if phase < 11 {
            assert!(fixture.entry.scope.snapshot().reserved_bytes > 0);
        }
    }
    assert_eq!(fake.live(), 0);
    finish(&mut fixture)?;
    Ok(())
}
#[test]
fn fixed_record_stage_identities_never_alias_or_overflow_into_output() -> TestResult {
    let mut old = generation(resolved().namespace, 1)?;
    old.state = crate::native_claims::RecordState::Released;
    let original = old.encode();
    let record = generation(resolved().namespace, 2)?;
    let encoded = record.encode();
    let mut fixture = PublicationFixture::new(ElectionRecordOrigin::Released(original))?;
    let retained = Arc::new(Mutex::new(None));
    let owned = retained.clone();
    let fake = Fake::new(RecordRead::Present(original));
    let external = fake.clone();
    let coordinator = fixture.entry.fake.clone();
    let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
    let mut job=fixture.entry.client.submit_blocking(&fixture.entry.scope,spec,move |context| -> Result<(),ClaimError> {
        let mut fake=fake;
        adopt(&owned,&mut fake,&context)?;
        coordinator.observe_coordinator([11;24]);
        let mut slot=lock(&owned);
        let file=slot.as_mut().ok_or_else(||ClaimError::invalid("identity_owner_missing"))?;
        let original_identity=fake.target_identity();
        assert_eq!(original_identity,[10;24]);
        for identity in 12..=254 {
            ordinary(file,&record,&mut fake,&context)?;
            assert_eq!(fake.target_identity(),[identity;24]);
            assert_ne!(fake.target_identity(),original_identity);
            assert!(lock(&file.stage).is_none());
        }
        let effects=lock(&fake.0).effects.len();
        assert!(ordinary(file,&record,&mut fake,&context).is_err());
        assert_eq!(lock(&fake.0).effects.len(),effects);
        assert_eq!(fake.target_identity(),[254;24]);
        assert_eq!(fake.live(),1);
        let result=retirement::retire_with_boundary(file,Some(&encoded),&context,&mut fake)?;
        assert!(matches!(result,PublicationRetirement::Preserved(RecordRead::Present(bytes)) if bytes==encoded));
        file.retire_fixture([11;24],&mut fake,&context)?;
        coordinator.close_coordinator();
        *slot=None;
        Ok(())
    })?;
    let done = fixture.entry.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    assert_eq!(external.live(), 0);
    finish(&mut fixture)?;
    Ok(())
}
