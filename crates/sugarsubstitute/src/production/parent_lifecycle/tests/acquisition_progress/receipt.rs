// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inject native readiness/write/retirement facts only at pure policy boundaries; no capabilities.
use super::*;
use sugarsubstitute_application::supervision::MainAuthorizationObservation;
fn unwritten_grant() -> Result<(ParentLifecycle, WireHeader, LeaseKey)> {
    let (mut parent, h, lease, mut coordinator) = pending()?;
    // These are external adapter-boundary facts for the pure coordinator. No kernel
    // handle, authenticated peer, readiness/source proof, frame or write admission exists.
    for observation in [
        ResourceObservation::ClaimAcquired(lease.resource),
        ResourceObservation::OwnedStarted {
            resource: lease.resource,
            process: ProcessKey { owner: lease.run.owner, id: ProcessId::new(1)? },
        },
    ] {
        coordinator
            .observe(observation)
            .map_err(|rejected| format!("Boundary ordering rejected: {:?}", rejected.reason))?;
    }
    let transition = coordinator
        .observe(ResourceObservation::Ready {
            resource: lease.resource,
            origin: sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
                "http://127.0.0.1:8188",
                8188,
            )?,
        })
        .map_err(|rejected| format!("Boundary readiness rejected: {:?}", rejected.reason))?;
    let reply = *transition.replies.first().ok_or("Policy Grant absent")?;
    assert_eq!(reply.outcome, RequestOutcome::Granted(lease));
    let acq = parent.acquisition.as_mut().ok_or("Original absent")?;
    acq.observe_backend(h, reply.outcome)?;
    acq.reply(h, Reply::Granted { lease, identity: identity()? })?;
    parent.lease = Some(lease);
    parent.main.begin(identity()?, lease, Moment::from_nanos(30_000_000_000))?;
    Ok((parent, h, lease))
}
#[test]
fn immediate_grant_holds_receive_routing_until_checked_original_write_without_progress()
-> Result<()> {
    let (mut parent, h, lease) = unwritten_grant()?;
    assert!(parent.hold_acquisition_receive());
    assert!(
        parent
            .acquisition
            .as_ref()
            .ok_or("Missing")?
            .output_lease(copied_binding(lease), super::super::super::replies::reference(lease))
            .is_err()
    );
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(8, 104);
    assert!(receipt.complete(8, 104).is_err());
    receipt.issue(8)?;
    receipt.complete(8, 104)?;
    parent.acquisition.as_mut().ok_or("Missing")?.written(h)?;
    assert!(!parent.hold_acquisition_receive());
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Missing")?.original,
        Moment::from_nanos(180_000_000_000)
    );
    Ok(())
}
#[test]
fn late_grant_cancellation_intention_waits_native_retirement_and_its_exact_checked_ack()
-> Result<()> {
    let (mut parent, h, lease) = unwritten_grant()?;
    parent.acquisition.as_mut().ok_or("Missing")?.written(h)?;
    let until = Moment::from_nanos(20_000_000_000);
    let cleanup =
        WireHeader { request: Some(RequestId::new(4)?), deadline_qpc: 20_000_000_000, ..h };
    let (index, _) = parent.requests.admit(cleanup, b"original cleanup intention")?;
    parent
        .prepare_attempt_cleanup(
            index,
            identity()?,
            h.request.ok_or("Original request absent")?,
            until,
            Moment::from_nanos(6_000_000_000),
            false,
        )
        .map_err(|cause| cause as Box<dyn Error>)?;
    let attempt = parent.attempt_cleanup.as_ref().ok_or("Original cleanup absent")?;
    assert!(attempt.main_retirement_required);
    assert!(!attempt.main_retired(parent.main.stage()));
    assert!(parent.requests.get(index)?.reply.is_none());
    parent.main.retire(identity()?)?;
    assert!(!parent.attempt_cleanup.as_ref().ok_or("Missing")?.main_retired(parent.main.stage()));
    let retirement = WireHeader { request: Some(RequestId::new(5)?), ..cleanup };
    parent
        .attempt_cleanup
        .as_mut()
        .ok_or("Missing")?
        .observe_retirement(identity()?, retirement)?;
    parent.main.accept(
        copied_binding(lease),
        retirement.request.ok_or("Request absent")?,
        MainAuthorizationObservation::MainRetired { identity: identity()? },
        Moment::from_nanos(7_000_000_000),
    )?;
    parent.main.confirm_retired(identity()?)?;
    let attempt = parent.attempt_cleanup.as_mut().ok_or("Missing")?;
    assert!(!attempt.main_retired(parent.main.stage()));
    attempt.retirement_written(cleanup);
    assert!(!attempt.main_retired(parent.main.stage()));
    attempt.retirement_written(retirement);
    assert!(attempt.main_retired(parent.main.stage()));
    assert_eq!(attempt.until, until);
    assert_eq!(attempt.origin, h);
    assert!(parent.requests.get(index)?.reply.is_none());
    assert!(!parent.native_retired());
    Ok(())
}
