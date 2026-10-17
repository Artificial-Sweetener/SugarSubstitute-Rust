// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the mounted binding owner and real request ledger with deterministic boundary data.
use super::*;
use crate::production_output::OutputBinding;
use std::time::Duration;
fn fixture() -> Result<(MainIdentity, OutputBinding)> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([6; 32]),
        epoch: CoordinatorEpoch::new([7; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let identity = MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(1)? };
    let resource =
        ResourceKey { owner, id: ResourceId::new(1)?, generation: ResourceGeneration::new(1)? };
    Ok((
        identity,
        OutputBinding {
            run,
            resource,
            connection: ConnectionId::new(19)?,
            lease: LeaseReference {
                resource: resource.id,
                generation: resource.generation,
                lease: LeaseId::new(1)?,
            },
        },
    ))
}
fn successor(id: u64) -> Result<LeaseReference> {
    Ok(LeaseReference {
        resource: ResourceId::new(id)?,
        generation: ResourceGeneration::new(1)?,
        lease: LeaseId::new(id)?,
    })
}
#[test]
fn historical_handoff_keeps_outage_until_positive_current_reply_and_keeps_main_identity()
-> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    assert_eq!(
        running.unavailable(binding.lease, true)?,
        Some((MainBackendState::RetryReady, None))
    );
    let failed = running.key(binding.lease);
    running.retry(identity, failed, Moment::from_nanos(10))?;
    let next = successor(2)?;
    assert_eq!(running.replacing(identity, binding.lease, next)?, failed);
    running.replaced(identity, binding.lease, next)?;
    assert_eq!(running.identity, identity);
    assert_eq!(running.current, next);
    assert_eq!(
        running.outage.ok_or("Outage disappeared at historical handoff")?.displayed,
        MainBackendState::Retrying
    );
    assert!(running.positive(binding.lease).is_err());
    assert_eq!(running.positive(next)?, Some(failed));
    assert_eq!(running.positive(next)?, None);
    assert_eq!(
        running.retry(identity, failed, Moment::from_nanos(11))?,
        RetryRequestDisposition::Obsolete
    );
    Ok(())
}
#[test]
fn immediate_successor_death_replaces_old_outage_without_claiming_recovery() -> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    running.unavailable(binding.lease, false)?;
    let next = successor(2)?;
    let old = running.replacing(identity, binding.lease, next)?;
    running.replaced(identity, binding.lease, next)?;
    assert_eq!(running.unavailable(next, false)?, Some((MainBackendState::Retiring, Some(old))));
    assert_eq!(running.awaiting_positive, None);
    assert_eq!(running.unavailable(next, true)?, Some((MainBackendState::RetryReady, None)));
    assert_eq!(
        running.retry(identity, old, Moment::from_nanos(20))?,
        RetryRequestDisposition::Obsolete
    );
    let next_key = running.key(next);
    running.retry(identity, next_key, Moment::from_nanos(20))?;
    assert_eq!(running.retry_request().ok_or("New actual outage retry absent")?.1, next);
    let third = successor(3)?;
    running.replacing(identity, next, third)?;
    running.replaced(identity, next, third)?;
    assert_eq!(running.positive(third)?, Some(next_key));
    assert_eq!(running.identity, identity);
    Ok(())
}
#[test]
fn repeated_notice_does_not_rearm_native_action_or_renew_original_managed_deadline() -> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    running.unavailable(binding.lease, true)?;
    let failed = running.key(binding.lease);
    let started = Moment::from_nanos(40);
    running.retry(identity, failed, started)?;
    let deadline = started.checked_add(Duration::from_secs(180))?;
    assert_eq!(running.retry_deadline(), Some(deadline));
    assert_eq!(running.unavailable(binding.lease, true)?, None);
    assert_eq!(
        running.retry(identity, failed, Moment::from_nanos(41))?,
        RetryRequestDisposition::Obsolete
    );
    assert_eq!(running.retry_deadline(), Some(deadline));
    assert_eq!(running.expire_retry(deadline), Some(failed));
    assert_eq!(running.expire_retry(deadline), None);
    assert_eq!(running.retry_deadline(), None);
    assert_eq!(running.retry_request(), None);
    assert_eq!(running.unavailable(binding.lease, true)?, None);
    assert_eq!(running.retry(identity, failed, deadline)?, RetryRequestDisposition::Obsolete);
    Ok(())
}
#[test]
fn foreign_callback_and_handoff_preserve_current_binding_and_original_retry_pair() -> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    running.unavailable(binding.lease, true)?;
    let failed = running.key(binding.lease);
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(running.retry(foreign, failed, Moment::from_nanos(50)).is_err());
    assert!(
        running
            .retry(
                identity,
                LeaseKey { connection: ConnectionId::new(20)?, ..failed },
                Moment::from_nanos(50)
            )
            .is_err()
    );
    assert_eq!(running.retry_request(), None);
    running.retry(identity, failed, Moment::from_nanos(50))?;
    let original = running.retry_request();
    assert!(running.replacing(foreign, binding.lease, successor(2)?).is_err());
    assert!(running.replacing(identity, successor(3)?, successor(2)?).is_err());
    assert!(running.replacing(identity, binding.lease, binding.lease).is_err());
    assert_eq!(running.current, binding.lease);
    assert_eq!(running.retry_request(), original);
    Ok(())
}
#[test]
fn actual_retry_failure_disables_until_new_authenticated_settlement_notice() -> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    running.unavailable(binding.lease, true)?;
    let failed = running.key(binding.lease);
    running.retry(identity, failed, Moment::from_nanos(60))?;
    assert_eq!(running.retry_failed()?, failed);
    assert!(running.retry_failed().is_err());
    assert_eq!(
        running.retry(identity, failed, Moment::from_nanos(61))?,
        RetryRequestDisposition::Obsolete
    );
    assert_eq!(
        running.unavailable(binding.lease, true)?,
        Some((MainBackendState::RetryReady, None))
    );
    running.retry(identity, failed, Moment::from_nanos(61))?;
    Ok(())
}
#[test]
fn real_ledger_consumes_one_reply_per_distinct_pull_and_retry_header() -> Result<()> {
    let (_, binding) = fixture()?;
    let mut ledger = requests::tests::requests_for_run(binding.run)?;
    let until = Moment::from_nanos(500);
    let encoded = CounterDeadline::new(1000, 100)?;
    let old_pull = ledger.prepare(until, encoded)?;
    ledger.output_pull(old_pull)?;
    ledger.admitted(old_pull)?;
    ledger.received(old_pull, Moment::from_nanos(100))?;
    ledger.consumed(old_pull)?;
    assert!(ledger.received(old_pull, Moment::from_nanos(101)).is_err());
    let retry = ledger.prepare(until, encoded)?;
    ledger.backend_retry(retry)?;
    assert_ne!(retry.request, old_pull.request);
    assert!(ledger.output_pull(retry).is_err());
    ledger.admitted(retry)?;
    ledger.received(retry, Moment::from_nanos(102))?;
    ledger.consumed(retry)?;
    assert!(ledger.consumed(retry).is_err());
    assert!(ledger.received(retry, Moment::from_nanos(103)).is_err());
    let current = ledger.prepare(until, encoded)?;
    assert_ne!(current.request, retry.request);
    ledger.output_pull(current)?;
    Ok(())
}

#[test]
fn queued_old_retry_after_actual_binding_consumption_is_obsolete_without_new_wire_intent()
-> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    running.unavailable(binding.lease, true)?;
    let queued_old_button = running.key(binding.lease);
    let next = successor(2)?;
    running.replacing(identity, binding.lease, next)?;
    assert_eq!(
        running.retry(identity, queued_old_button, Moment::from_nanos(70))?,
        RetryRequestDisposition::Obsolete
    );
    running.replaced(identity, binding.lease, next)?;
    assert_eq!(
        running.retry(identity, queued_old_button, Moment::from_nanos(71))?,
        RetryRequestDisposition::Obsolete
    );
    assert_eq!(running.retry_request(), None);
    assert_eq!(running.current, next);
    assert_eq!(running.identity, identity);
    assert_eq!(running.outage.ok_or("Mounted outage lost")?.displayed, MainBackendState::Retrying);
    assert_eq!(running.positive(next)?, Some(queued_old_button));
    assert_eq!(
        running.retry(identity, queued_old_button, Moment::from_nanos(72))?,
        RetryRequestDisposition::Obsolete
    );
    assert_eq!(running.retry_request(), None);
    assert!(running.retry(identity, running.key(successor(9)?), Moment::from_nanos(73)).is_err());
    Ok(())
}

#[test]
fn admitted_retry_waits_beyond_three_minutes_without_renewing_finite_preparation() -> Result<()> {
    let (identity, binding) = fixture()?;
    let mut running = RunningBackendBinding::new(identity, binding)?;
    running.unavailable(binding.lease, true)?;
    let failed = running.key(binding.lease);
    running.retry(identity, failed, Moment::from_nanos(0))?;
    let original = running.retry_request().ok_or("Original retry absent")?.2;
    running.retry_admitted()?;
    assert_eq!(running.retry_deadline(), None);
    assert_eq!(running.expire_retry(Moment::from_nanos(360_000_000_000)), None);
    assert_eq!(running.retry_request().ok_or("Original retry lost")?.2, original);
    assert!(running.retry_admitted().is_err());
    assert_eq!(running.retry_failed()?, failed);
    assert_eq!(running.retry_request(), None);
    Ok(())
}

#[test]
fn frontend_reload_rebinds_only_current_main_and_preserves_exact_backend_interest() -> Result<()> {
    let (old, binding) = fixture()?;
    let mut owner = RunningBackendBinding::new(old, binding)?;
    let next = MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..old };
    owner.reload_main(old, next)?;
    assert_eq!(owner.reload_lease(next)?, binding.lease);
    assert_eq!(owner.connection, binding.connection);
    assert_eq!(owner.key(binding.lease).run, old.run);
    assert!(owner.reload_main(old, next).is_err());
    owner.unavailable(binding.lease, true)?;
    assert!(owner.reload_lease(next).is_err());
    assert_eq!(owner.current, binding.lease);
    Ok(())
}
