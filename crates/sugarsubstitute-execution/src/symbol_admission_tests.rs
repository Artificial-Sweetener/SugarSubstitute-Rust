// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject reconstructed accounting and reentrant stale authority at the actual Core boundary.
use super::*;
use crate::{runtime_test_support::TestResult, symbol_test_support::LiveSymbol};
use sugarsubstitute_contracts::execution::{AdmittedJobResources, NativeHandleRelease};
struct ForeignRelease;
impl NativeHandleRelease for ForeignRelease {
    fn release_symbol_reservation(&self) {}
    fn output_custody_changed(&self) {}
}
#[test]
fn same_numeric_public_context_and_claim_cannot_alias_actual_core_ledger() -> TestResult {
    let live = LiveSymbol::new()?;
    let core = live.rig.client.core_for_test();
    let cancellation = lock(&core.state)
        .jobs
        .get(&live.context.job_id())
        .ok_or("Original record absent")?
        .job
        .cancellation();
    let forged = JobContext::new(
        live.context.owner_id(),
        live.context.scope_id(),
        live.context.spec(),
        live.context.deadline(),
        cancellation,
        core.clock.clone(),
        AdmittedJobResources::new(live.context.memory().clone(), Some(Box::new(ForeignRelease))),
    );
    let lease = forged.native_handles().ok_or("Reconstructed ledger absent")?;
    let claim = lease.take_partition(SymbolNativePartition::Transport)?;
    assert!(claim.matches_lease(lease));
    assert_eq!(lease.owner_id(), live.context.owner_id());
    assert_eq!(lease.original_scope_id(), live.context.scope_id());
    assert_eq!(lease.original_job_id(), live.context.job_id());
    let rejected = match live.rig.client.symbol_transport_reservation(&forged, claim) {
        Err(rejected) => rejected,
        Ok(_) => return Err("Reconstructed same-number ledger acquired native transport".into()),
    };
    assert!(matches!(
        rejected.error,
        AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired)
    ));
    assert!(rejected.handles.matches_lease(lease));
    assert_eq!(rejected.handles.in_custody(), 0);
    assert_eq!(live.rig.scope.snapshot().native_reservations, 1);
    let actual = live.transport()?;
    assert!(
        actual.handles.matches_lease(live.context.native_handles().ok_or("Actual ledger absent")?)
    );
    Ok(())
}
#[test]
fn external_clock_reentrant_main_cancel_vetoes_handoff_with_exact_claim() -> TestResult {
    let live = LiveSymbol::new()?;
    let lease = live.context.native_handles().ok_or("Original ledger absent")?;
    let claim = lease.take_partition(SymbolNativePartition::Transport)?;
    let client = live.rig.client.clone();
    let scope_id = live.rig.scope.id();
    live.rig.clock.on_next_current_thread_observation(Box::new(move || {
        client.core_for_test().cancel_scope(scope_id)
    }));
    let rejected = match live.rig.client.symbol_transport_reservation(&live.context, claim) {
        Err(rejected) => rejected,
        Ok(_) => return Err("Reentrant cancellation admitted transport".into()),
    };
    assert!(rejected.handles.matches_lease(lease));
    assert_eq!(rejected.handles.in_custody(), 0);
    assert!(live.context.cancellation_requested());
    Ok(())
}
#[test]
fn original_main_expiry_vetoes_child_effect_and_preserves_existing_custody() -> TestResult {
    let live = LiveSymbol::new()?;
    let actual = live.transport()?;
    let effect = crate::native_resources::EffectContext::new(&actual);
    let output = actual.handles.reserve_output()?;
    assert!(effect.check().is_ok());
    live.rig.clock.advance(10_000_000_000);
    assert!(matches!(effect.check(), Err(AdmissionError::DeadlineExpired { .. })));
    assert_eq!(actual.handles.in_custody(), 1);
    assert_eq!(live.rig.scope.snapshot().native_outputs_in_custody, 1);
    drop(output);
    assert_eq!(actual.handles.in_custody(), 0);
    Ok(())
}
#[test]
fn terminal_ack_vetoes_fresh_effect_but_does_not_release_original_ledger() -> TestResult {
    let mut live = LiveSymbol::new()?;
    let actual = live.transport()?;
    let effect = crate::native_resources::EffectContext::new(&actual);
    let completion = live.complete()?;
    assert!(matches!(
        completion.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    drop(completion);
    assert!(matches!(
        effect.check(),
        Err(AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired))
    ));
    assert_eq!(live.rig.scope.snapshot().native_reservations, 1);
    Ok(())
}
#[test]
fn output_counter_notification_is_passive_inside_actual_core_fence() -> TestResult {
    let live = LiveSymbol::new()?;
    let actual = live.transport()?;
    let core = live.rig.client.core_for_test();
    let state = lock(&core.state);
    let output = actual.handles.reserve_output()?;
    assert_eq!(
        state
            .symbol_resources
            .get(&live.context.job_id())
            .ok_or("Actual counter absent")?
            .observation
            .snapshot(),
        (1, 1)
    );
    drop(output);
    assert_eq!(actual.handles.in_custody(), 0);
    drop(state);
    Ok(())
}

#[test]
fn shared_original_context_clones_stay_inside_one_fixed_metadata_claim() -> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let original = crate::native_resources::EffectContext::new(&reservation);
    let retained = original.clone();
    drop(original);
    drop(reservation);
    retained.check()?;
    assert!(
        crate::native_resources::symbol_storage_bytes() as u64
            <= crate::native_resources::SYMBOL_METADATA_BYTES
    );
    let remainder =
        live.context.memory().claim(MAIN_BYTES - crate::native_resources::SYMBOL_METADATA_BYTES)?;
    assert!(live.context.memory().claim(1).is_err());
    assert_eq!(live.rig.scope.snapshot().native_reservations, 1);
    drop(retained);
    drop(remainder);
    Ok(())
}
