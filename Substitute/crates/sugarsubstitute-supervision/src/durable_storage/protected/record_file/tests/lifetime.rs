// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stop subsequent publication effects at actual proof closure or original expiry.
use super::*;
use crate::coordinator_entry::fixture::Gate;
#[test]
fn fixed_record_closed_claim_admission_fences_each_external_effect() -> TestResult {
    interrupted_publication(Interruption::AdmissionClosed)
}
#[test]
fn fixed_record_original_expiry_fences_each_effect_and_cleanup_never_republishes() -> TestResult {
    interrupted_publication(Interruption::Expired)
}
#[test]
fn fixed_record_cancelled_admitted_job_fences_each_external_effect() -> TestResult {
    interrupted_publication(Interruption::Cancelled)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Interruption {
    AdmissionClosed,
    Expired,
    Cancelled,
}
fn interrupted_publication(interruption: Interruption) -> TestResult {
    for effect in [Effect::Create, Effect::Write, Effect::Activate] {
        let mut fixture =
            PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
        let proof = fixture.retained_proof()?;
        let coordinator = fixture.entry.fake.clone();
        let fake = Fake::new(RecordRead::Missing);
        let external = fake.clone();
        let gate = Arc::new(Gate::default());
        {
            let mut disk = lock(&fake.0);
            if interruption == Interruption::Expired {
                disk.expiry =
                    Some((effect, fixture.entry.clock.clone(), Moment::from_nanos(15_000_000_000)));
            } else {
                disk.gate = Some((effect, gate.clone()));
            }
        }
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
                let file = slot.as_mut().ok_or_else(|| ClaimError::invalid("missing_held"))?;
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
                )
            },
        )?;
        if interruption != Interruption::Expired {
            gate.wait()?;
            if interruption == Interruption::Cancelled {
                job.cancel();
            } else {
                fixture.close_claim_admission();
            }
            gate.release();
        }
        let completion = fixture.entry.consume_job(&mut job)?;
        if interruption == Interruption::Expired {
            assert!(matches!(completion.outcome, JobOutcome::TimedOut), "{completion:?}");
        } else if interruption == Interruption::Cancelled {
            assert!(matches!(completion.outcome, JobOutcome::Cancelled), "{completion:?}");
        } else {
            assert!(matches!(completion.outcome, JobOutcome::Failure(_)), "{completion:?}");
        }
        drop(completion);
        drop(job);
        assert_eq!(external.live(), 2);
        assert!(lock(&retained).as_ref().is_some_and(|file| lock(&file.stage).is_some()));
        let before = lock(&external.0).effects.len();
        assert_eq!(
            before,
            match effect {
                Effect::Create => 1,
                Effect::Write => 2,
                Effect::Activate => 3,
            }
        );
        let coordinator = fixture.entry.fake.clone();
        let owned = retained.clone();
        let cleanup_fake = external.clone();
        let now = fixture.entry.clock.now();
        let spec = fixture.entry.spec(Moment::from_nanos(now.as_nanos() + 15_000_000_000))?;
        let mut cleanup = fixture.entry.client.submit_blocking(
            &fixture.entry.scope,
            spec,
            move |context| -> Result<(), ClaimError> {
                let mut fake = cleanup_fake;
                let mut slot = lock(&owned);
                let file =
                    slot.as_mut().ok_or_else(|| ClaimError::invalid("cleanup_owner_missing"))?;
                let outcome =
                    retirement::retire_with_boundary(file, Some(&[42; 128]), &context, &mut fake)?;
                match effect {
                    Effect::Activate => assert!(matches!(outcome,
                        PublicationRetirement::Activated(bytes) if bytes==[42;128])),
                    _ => assert!(matches!(
                        outcome,
                        PublicationRetirement::Preserved(RecordRead::Missing)
                    )),
                }
                assert_eq!(lock(&fake.0).effects.len(), before);
                assert_eq!(fake.live(), 1);
                file.retire_fixture([11; 24], &mut fake, &context)?;
                coordinator.close_coordinator();
                *slot = None;
                Ok(())
            },
        )?;
        let completion = fixture.entry.consume_job(&mut cleanup)?;
        assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
        drop(completion);
        drop(cleanup);
        assert_eq!(external.live(), 0);
        finish(&mut fixture)?;
    }
    Ok(())
}
