// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real child pull preparation and source custody with deterministic external time.
use super::*;
use crate::production::child::{
    configuration::tests::Rig,
    test_support::{Frozen, child},
};
use std::sync::{Arc, atomic::Ordering};

/// Real output constructor and request ledger; only native transport remains unavailable.
fn fixture() -> Result<(ProductionChild, Rig, Arc<Frozen>)> {
    let (mut child, previous, clock) = child()?;
    let rig = Rig::with_clock(clock.clone()).map_err(|cause| cause.to_string())?;
    child.client = Some(rig.client.clone());
    drop(previous);
    let resource = ResourceKey {
        owner: child.run.owner,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let binding = OutputBinding {
        run: child.run,
        resource,
        connection: ConnectionId::new(1)?,
        lease: LeaseReference {
            resource: resource.id,
            generation: resource.generation,
            lease: LeaseId::new(1)?,
        },
    };
    child.bootstrap.channel_mut().output_polling_fixture(&rig.client, binding.connection)?;
    child.output.begin(
        &rig.client,
        &rig.scope,
        JobId::new(1)?,
        binding,
        Moment::from_nanos(30_000_000_000),
    )?;
    let completion = rig
        .completion(child.output.job.as_mut().ok_or("Constructor absent")?)
        .map_err(|cause| cause.to_string())?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    child.output.terminal = Some(completion);
    child.output.job = None;
    child.output.provisional = false;
    Ok((child, rig, clock))
}
/// Reaching the missing native boundary proves real ledger and output-owner preparation happened.
fn prepared(child: &mut ProductionChild) -> Result<(WireHeader, Moment)> {
    let error = child.advance_output().err().ok_or("Native transport unexpectedly present")?;
    let mut boundary: &(dyn Error + 'static) = error.as_ref();
    while let Some(cause) = boundary.source() {
        boundary = cause;
    }
    assert_eq!(boundary.to_string(), "Lifecycle application session missing");
    let pending = child.output.pending.ok_or("Real output pull not prepared")?;
    assert!(child.bootstrap.channel_mut().request_prepared(pending.0)?);
    let (message, original) = child
        .output
        .owner_mut()
        .ok_or("Source absent")?
        .pending_message()
        .ok_or("Original owner message absent")?;
    assert_eq!(message.header, pending.0);
    assert_eq!(original, pending.1);
    Ok(pending)
}

#[test]
fn idle_reply_stops_early_native_wakes_and_admits_one_pull_at_the_exact_deadline() -> Result<()> {
    let (mut child, _rig, clock) = fixture()?;
    child.output.pull_after = polling::after_reply(Moment::from_nanos(0), true)?;
    let reserved = child.output.owner_mut().ok_or("Source absent")?.worker_pending();
    for now in [0, 1, 99_999_999] {
        clock.0.store(now, Ordering::SeqCst);
        child.advance_output()?;
        assert!(child.output.pending.is_none());
        assert_eq!(child.output.deadline(), Some(Moment::from_nanos(100_000_000)));
        assert_eq!(child.next_deadline(), Some(Moment::from_nanos(100_000_000)));
        assert_eq!(child.output.owner_mut().ok_or("Source absent")?.worker_pending(), reserved);
    }
    clock.0.store(100_000_000, Ordering::SeqCst);
    let original = prepared(&mut child)?;
    assert_eq!(original.1, Moment::from_nanos(30_100_000_000));
    assert_eq!(child.output.pull_after, None);
    clock.0.store(200_000_000, Ordering::SeqCst);
    assert_eq!(prepared(&mut child)?, original);
    assert_eq!(child.output.deadline(), Some(original.1));
    Ok(())
}

#[test]
fn delivery_disposition_keeps_next_pull_immediate_with_its_original_request_deadline() -> Result<()>
{
    let (mut child, _rig, _clock) = fixture()?;
    child.output.pull_after = polling::after_reply(Moment::from_nanos(0), false)?;
    assert_eq!(prepared(&mut child)?.1, Moment::from_nanos(30_000_000_000));
    Ok(())
}

#[test]
fn repeated_outage_reply_is_paced_and_current_user_retry_bypasses_the_interval() -> Result<()> {
    let (mut child, _rig, _clock) = fixture()?;
    let binding = child.output.binding.ok_or("Current binding absent")?;
    let mut running = backend::RunningBackendBinding::new(child.identity, binding)?;
    child.output.owner_mut().ok_or("Source absent")?.running_main(child.identity)?;
    assert!(running.unavailable(binding.lease, true)?.is_some());
    assert!(running.unavailable(binding.lease, true)?.is_none());
    let failed = running.key(binding.lease);
    child.running_backend = Some(running);
    child.output.pull_after = polling::after_reply(Moment::from_nanos(0), true)?;
    child.advance_output()?;
    assert!(child.output.pending.is_none());
    child.backend_retry_requested(child.identity, failed)?;
    assert_eq!(child.output.pull_after, None);
    let original = prepared(&mut child)?;
    assert_eq!(original.1, Moment::from_nanos(180_000_000_000));
    assert!(child.output.retry_pending());
    assert_eq!(
        child.running_backend.as_ref().and_then(|running| running.retry_deadline()),
        Some(original.1)
    );
    Ok(())
}

#[test]
fn close_clears_idle_wake_and_prevents_preparing_a_new_request() -> Result<()> {
    let (mut child, _rig, _clock) = fixture()?;
    child.output.pull_after = polling::after_reply(Moment::from_nanos(0), true)?;
    child.output.close();
    child.advance_output()?;
    assert!(child.output.pending.is_none());
    assert_eq!(child.output.pull_after, None);
    assert_eq!(child.output.deadline(), None);
    Ok(())
}

#[test]
fn provisional_managed_output_after_three_minutes_keeps_finite_pulls_and_close_custody()
-> Result<()> {
    let (mut child, _rig, clock) = fixture()?;
    child.output.provisional = true;
    clock.0.store(360_000_000_000, Ordering::SeqCst);
    let original = prepared(&mut child)?;
    assert_eq!(original.1, Moment::from_nanos(390_000_000_000));
    clock.0.store(361_000_000_000, Ordering::SeqCst);
    assert_eq!(prepared(&mut child)?, original);
    child.output.close();
    assert!(child.output.pending());
    assert_eq!(child.output.pending, Some(original));
    assert_eq!(child.output.deadline(), Some(original.1));
    Ok(())
}
