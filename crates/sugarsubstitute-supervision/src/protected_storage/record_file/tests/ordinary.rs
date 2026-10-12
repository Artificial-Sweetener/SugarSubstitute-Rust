// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence ordinary publication through actual admitted cancellation and deadline.
use super::release::{generation, ordinary};
use super::*;
use crate::{coordinator_entry::fixture::Gate, native_claims::Record};
#[test]
fn fixed_record_ordinary_cancellation_fences_each_adopted_effect() -> TestResult {
    interrupt(false)
}
#[test]
fn fixed_record_ordinary_expiry_fences_each_adopted_effect() -> TestResult {
    interrupt(true)
}
fn interrupt(expired: bool) -> TestResult {
    for effect in [Effect::Create, Effect::Write, Effect::Activate] {
        let mut predecessor = generation(resolved().namespace, 1)?;
        predecessor.state = crate::native_claims::RecordState::Released;
        let original = predecessor.encode();
        let desired = generation(resolved().namespace, 2)?;
        let encoded = desired.encode();
        let mut fixture = PublicationFixture::new(ElectionRecordOrigin::Released(original))?;
        let fake = Fake::new(RecordRead::Present(original));
        let external = fake.clone();
        let gate = Arc::new(Gate::default());
        {
            let mut disk = lock(&fake.0);
            if expired {
                disk.expiry =
                    Some((effect, fixture.entry.clock.clone(), Moment::from_nanos(15_000_000_000)));
            } else {
                disk.gate = Some((effect, gate.clone()));
            }
        }
        let retained = Arc::new(Mutex::new(None));
        let owned = retained.clone();
        let coordinator = fixture.entry.fake.clone();
        let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
        let mut job =
            fixture.entry.client.submit_blocking(&fixture.entry.scope, spec, move |context| {
                let mut fake = fake;
                let inode = adopt(&owned, &mut fake, &context)?;
                coordinator.observe_coordinator(inode);
                let slot = lock(&owned);
                ordinary(
                    slot.as_ref().ok_or_else(|| ClaimError::invalid("ordinary_owner_missing"))?,
                    &desired,
                    &mut fake,
                    &context,
                )
            })?;
        if !expired {
            gate.wait()?;
            job.cancel();
            gate.release();
        }
        let done = fixture.entry.consume_job(&mut job)?;
        assert!(
            matches!(
                (expired, &done.outcome),
                (true, JobOutcome::TimedOut) | (false, JobOutcome::Cancelled)
            ),
            "{done:?}"
        );
        drop((done, job));
        let expected_effects = match effect {
            Effect::Create => 1,
            Effect::Write => 2,
            Effect::Activate => 3,
        };
        assert_eq!(lock(&external.0).effects.len(), expected_effects);
        assert_eq!(external.live(), 2);
        assert!(fixture.entry.scope.snapshot().reserved_bytes > 0);
        {
            let mut disk = lock(&external.0);
            disk.gate = None;
            disk.expiry = None;
        }
        let cleanup_fake = external.clone();
        let owned = retained.clone();
        let coordinator = fixture.entry.fake.clone();
        let now = fixture.entry.clock.now().as_nanos();
        let spec = fixture.entry.spec(Moment::from_nanos(now + 15_000_000_000))?;
        let mut cleanup = fixture.entry.client.submit_blocking(
            &fixture.entry.scope,
            spec,
            move |context| -> Result<(), ClaimError> {
                let mut fake = cleanup_fake;
                let mut slot = lock(&owned);
                let file =
                    slot.as_mut().ok_or_else(|| ClaimError::invalid("ordinary_cleanup_missing"))?;
                assert!(ordinary(file, &Record::decode(&encoded)?, &mut fake, &context).is_err());
                assert_eq!(lock(&fake.0).effects.len(), expected_effects);
                let actual =
                    retirement::retire_with_boundary(file, Some(&encoded), &context, &mut fake)?;
                match actual {
                    PublicationRetirement::Activated(bytes) => {
                        assert_eq!(bytes, encoded);
                        assert_eq!(effect, Effect::Activate);
                    }
                    PublicationRetirement::Preserved(RecordRead::Present(bytes)) => {
                        assert_eq!(bytes, original);
                        assert_ne!(effect, Effect::Activate);
                    }
                    _ => return Err(ClaimError::invalid("ordinary_observation_lost")),
                }
                assert!(lock(&file.stage).is_none());
                assert!(lock(&file.publication).is_none());
                assert_eq!(lock(&fake.0).effects.len(), expected_effects);
                file.retire_fixture([11; 24], &mut fake, &context)?;
                coordinator.close_coordinator();
                *slot = None;
                Ok(())
            },
        )?;
        let done = fixture.entry.consume_job(&mut cleanup)?;
        assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
        drop((done, cleanup));
        assert_eq!(external.live(), 0);
        finish(&mut fixture)?;
    }
    Ok(())
}
