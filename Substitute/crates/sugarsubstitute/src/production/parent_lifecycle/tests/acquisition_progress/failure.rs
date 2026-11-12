// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve real policy cancellation/failure and original progress custody without readiness.
use super::*;
#[test]
fn released_pending_policy_cancels_original_acquire_and_keeps_progress_until_retirement()
-> Result<()> {
    let (mut parent, h, lease, mut coordinator) = pending()?;
    complete_progress(&mut parent, h)?;
    let transition = coordinator.request(
        copied_binding(lease),
        ResourceRequest {
            run: lease.run,
            connection: lease.connection,
            id: RequestId::new(4)?,
            command: ResourceCommand::Release(lease),
        },
    )?;
    let original = *transition
        .replies
        .iter()
        .find(|reply| Some(reply.request) == h.request)
        .ok_or("Original cancellation missing")?;
    assert_eq!(original.outcome, RequestOutcome::Cancelled);
    let acq = parent.acquisition.as_mut().ok_or("Original missing")?;
    acq.observe_backend(h, original.outcome)?;
    acq.reply(h, Reply::Cancelled)?;
    assert!(acq.progress().is_none());
    assert_eq!(
        acq.output_lease(copied_binding(lease), super::super::super::replies::reference(lease))?,
        lease
    );
    assert_eq!(acq.original, Moment::from_nanos(180_000_000_000));
    assert!(parent.lease(super::super::super::replies::reference(lease)).is_err());
    assert_eq!(parent.main.stage(), MainAuthorizationStage::Retired);
    Ok(())
}
#[test]
fn genuine_preparation_cause_survives_nonfinal_output_reservation_failure() -> Result<()> {
    let (mut parent, h, lease, mut coordinator) = pending()?;
    complete_progress(&mut parent, h)?;
    let transition = coordinator
        .observe(ResourceObservation::Failed {
            resource: lease.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::other("original typed preparation boundary")),
            ),
        })
        .map_err(|rejected| {
            format!("Original failure unexpectedly rejected: {:?}", rejected.reason)
        })?;
    let original = *transition
        .replies
        .iter()
        .find(|reply| Some(reply.request) == h.request)
        .ok_or("Original failure missing")?;
    assert_eq!(original.outcome, RequestOutcome::Failed);
    let acq = parent.acquisition.as_mut().ok_or("Original missing")?;
    acq.observe_backend(h, original.outcome)?;
    acq.reply(h, Reply::Failed(LifecycleFailureCode::Preparation))?;
    assert!(acq.progress().is_none());
    assert_eq!(acq.original, Moment::from_nanos(180_000_000_000));
    let cause = coordinator
        .failure(lease.resource)
        .and_then(Error::source)
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("Original cause absent")?;
    assert_eq!(cause.to_string(), "original typed preparation boundary");
    assert!(parent.lease(super::super::super::replies::reference(lease)).is_err());
    assert_eq!(parent.main.stage(), MainAuthorizationStage::Retired);
    Ok(())
}
