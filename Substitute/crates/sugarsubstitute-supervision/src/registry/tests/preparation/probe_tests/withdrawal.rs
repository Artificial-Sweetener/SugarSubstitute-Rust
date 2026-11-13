// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Before admission, cancellation closes creation permanently while retaining retryable journal custody.
use super::*;

fn attempt(
    h: &mut Harness,
    input: &mut Option<WindowsLaunchInput>,
) -> Result<LaunchTicket, Box<dyn Error>> {
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let create = h.spec(
        JobClass::InfrastructureLifecycle,
        input.as_ref().ok_or("Input absent")?.required_reservation_bytes(),
    )?;
    let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
    h.next_scope += 1;
    Ok(h.registry.submit_preparation_launch(&h.client, &h.scope, native, create, watch, input)?)
}

fn withdrawal(reject_before: bool, fail_write: bool) -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let mut input = Some(input);
    if reject_before {
        h.registry.close_launch_admission();
        let error = attempt(&mut h, &mut input).err().ok_or("Launch unexpectedly admitted")?;
        assert!(matches!(error.downcast_ref(), Some(RegistryAdmissionError::Closed)));
        assert!(input.is_some());
        assert!(h.registry.is_empty());
    }
    let (fixture, mut input) =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            let before = journal(&fixture)?;
            let pin =
                if fail_write { Some(updater::pin_journal(fixture.root.path())?) } else { None };
            let result = input.as_mut().ok_or("Input absent")?.withdraw_updater_probe(&context);
            if fail_write {
                assert!(result.is_err());
                assert_eq!(journal(&fixture)?, before);
            } else {
                result?;
                assert_ne!(journal(&fixture)?, before);
            }
            let probe = input
                .as_ref()
                .and_then(|input| input.preparation.as_ref())
                .and_then(|launch| launch.probe())
                .ok_or("Probe absent")?;
            assert!(!probe.can_launch());
            assert!(probe.arguments().is_err());
            excluded(&fixture)?;
            assert!(!writable(&fixture.executable));
            drop(pin);
            Ok((fixture, input))
        })?;
    // A failed storage write must not reopen native creation, even on an open registry.
    let error =
        attempt(&mut h, &mut input).err().ok_or("Withdrawn launch unexpectedly admitted")?;
    assert!(matches!(
        error.downcast_ref(),
        Some(RegistryAdmissionError::Closed | RegistryAdmissionError::InvalidSpecification)
    ));
    assert!(input.is_some());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert!(h.registry.is_empty());
    let fixture =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            let launch = input.as_mut().ok_or("Input absent")?;
            launch.withdraw_updater_probe(&context)?;
            let after = journal(&fixture)?;
            launch.withdraw_updater_probe(&context)?;
            assert_eq!(journal(&fixture)?, after);
            excluded(&fixture)?;
            drop(input);
            Ok(fixture)
        })?;
    assert!(writable(&fixture.executable));
    let (_fixture, state) = selection(&mut h, fixture)?;
    assert!(matches!(state.candidate(), Some(Candidate::Prepared { .. })));
    assert!(state.accepted().is_none());
    assert!(state.strikes().is_empty());
    Ok(())
}

#[test]
fn rejected_admission_can_restore_prepared_engine_without_quarantining_it() -> TestResult {
    withdrawal(true, false)
}

#[test]
fn withdrawn_input_cannot_be_admitted_again() -> TestResult {
    withdrawal(false, false)
}

#[test]
fn failed_withdrawal_keeps_original_custody_and_blocks_creation_until_retry() -> TestResult {
    withdrawal(false, true)
}
