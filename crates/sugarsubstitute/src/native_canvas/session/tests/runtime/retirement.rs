// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise cleanup admission, retained result custody and nonrenewable retirement cutoffs.

use super::*;

#[test]
fn retirement_admits_while_payload_retains_its_entire_original_reservation()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::with_memory_capacity(
        clock,
        wake.clone(),
        std::num::NonZeroU64::new(64 * 1024).ok_or("Capacity absent")?,
    )?;
    let client = owner.client();
    let runtime =
        Runtime::shared(client.clone(), ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        64 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let mut producer = client.submit_blocking(&runtime.scope, spec, |context| {
        Ok::<_, WorkError>(context.memory().clone())
    })?;
    let completion = wake.completion(&client, &mut producer)?;
    let JobOutcome::Success(retained) = completion.outcome else {
        return Err("Retained payload producer failed".into());
    };
    drop(completion.memory);
    drop(producer);
    assert_eq!(runtime.scope.snapshot().reserved_bytes, 64 * 1024);

    session.runtime = Some(runtime);
    session.pending_retirement =
        Some(Retired { _fixture_memory: Some(retained), ..Default::default() });
    session.retirement_deadline = Some(Moment::from_nanos(5_000_000_000));
    session.advance()?;
    assert!(session.pending_retirement.is_none());
    assert!(session.retirement.is_some());

    while session.retirement.is_some() {
        let observed = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.advance()?;
        if session.retirement.is_some() {
            wake.wait_after(observed)?;
        }
    }
    assert_eq!(
        session.runtime.as_ref().ok_or("Canvas runtime absent")?.scope.snapshot().reserved_bytes,
        0
    );
    settle(&mut session, &client, &wake)?;
    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    while !owner.poll_shutdown()?.settled {
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
    }
    owner.finish_shutdown()?;
    Ok(())
}

#[test]
fn pending_retirement_consumes_terminal_work_that_releases_its_admission()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::with_memory_capacity(
        clock,
        wake.clone(),
        std::num::NonZeroU64::new(64 * 1024).ok_or("Capacity absent")?,
    )?;
    let client = owner.client();
    let runtime =
        Runtime::shared(client.clone(), ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        64 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    session.initializing = Some(client.submit_blocking(&runtime.scope, spec, |_| {
        Err::<(Resources, Scene), _>(WorkError::invalid("terminal initialization"))
    })?);
    session.runtime = Some(runtime);
    session.pending_retirement = Some(Retired::default());
    session.retirement_deadline = Some(Moment::from_nanos(5_000_000_000));

    loop {
        let observed = wake.revision();
        if matches!(
            session.initializing.as_ref().ok_or("Initialization job absent")?.status().phase,
            JobPhase::Terminal(_)
        ) {
            break;
        }
        wake.wait_after(observed)?;
    }
    client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
    session.advance()?;
    assert!(session.initializing.is_none());
    assert!(session.pending_retirement.is_none());
    assert!(session.retirement.is_some());
    assert_eq!(
        session.failed.take().ok_or("Expected initialization failure")?.to_string(),
        "terminal initialization"
    );

    while session.retirement.is_some() {
        let observed = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.advance()?;
        if session.retirement.is_some() {
            wake.wait_after(observed)?;
        }
    }
    settle(&mut session, &client, &wake)?;
    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    while !owner.poll_shutdown()?.settled {
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
    }
    owner.finish_shutdown()?;
    Ok(())
}

/// Observe each cleanup completion before finishing the application-owned execution scope.
fn settle(
    session: &mut Session,
    client: &ExecutionClient,
    wake: &Wake,
) -> Result<(), Box<dyn Error>> {
    session.close_until(Moment::from_nanos(10_000_000_000))?;
    loop {
        let observed = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if session.settled()? {
            break;
        }
        wake.wait_after(observed)?;
    }
    session.finish()?;
    Ok(())
}

#[test]
fn inherited_shutdown_cutoff_never_renews_pending_canvas_retirement() -> Result<(), Box<dyn Error>>
{
    let (mut session, clock, _directory) = session()?;
    session.retire(Retired::default())?;
    let original = Moment::from_nanos(2_000_000_000);
    session.close_until(original)?;
    assert_eq!(session.shutdown_deadline, Some(original));
    assert_eq!(session.retirement_deadline, Some(original));

    clock.0.store(3_000_000_000, Ordering::SeqCst);
    session.close_until(Moment::from_nanos(20_000_000_000))?;
    session.close()?;
    assert_eq!(session.shutdown_deadline, Some(original));
    assert_eq!(session.retirement_deadline, Some(original));

    let tightened = Moment::from_nanos(1_000_000_000);
    session.close_until(tightened)?;
    assert_eq!(session.shutdown_deadline, Some(tightened));
    assert_eq!(session.retirement_deadline, Some(tightened));
    Ok(())
}

#[test]
fn completed_payload_retirement_is_deferred_behind_retained_cleanup() -> Result<(), Box<dyn Error>>
{
    let (mut session, clock, _directory) = session()?;
    session.retire(Retired::default())?;
    assert_eq!(session.retirement_deadline, Some(Moment::from_nanos(5_000_000_000)));

    clock.0.store(1_000_000_000, Ordering::SeqCst);
    session.retire(Retired::default())?;
    assert!(session.pending_retirement.is_some());
    assert!(session.deferred_retirement.is_some());
    assert_eq!(session.deferred_retirement_deadline, Some(Moment::from_nanos(6_000_000_000)));

    let cutoff = Moment::from_nanos(2_000_000_000);
    session.close_until(cutoff)?;
    assert_eq!(session.retirement_deadline, Some(cutoff));
    assert_eq!(session.deferred_retirement_deadline, Some(cutoff));
    Ok(())
}
