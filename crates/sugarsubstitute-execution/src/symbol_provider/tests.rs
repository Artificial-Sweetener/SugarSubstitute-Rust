// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe genuine Provider4/1MiB issuance, reentrant controls and actual paired cleanup.
use crate::symbol_bundle::test_support::*;
use crate::{runtime_test_support::TestResult, symbol_test_support::LiveSymbol};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn provider_selected_and_reader_shares_are_disjoint_and_last_alias_reclaims_only_provider()
-> TestResult {
    let symbol = LiveSymbol::new()?;
    let selected = symbol.rig.client.symbol_bundle_admission(&symbol.context)?.into_origin();
    let provider = symbol.rig.client.symbol_provider_admission(&symbol.context)?.into_origin();
    assert_eq!(provider.retained_bytes(), 1048576);
    assert!(provider.control_storage_bytes() <= 65536);
    assert!(symbol.rig.client.symbol_provider_admission(&symbol.context).is_err());
    let ledger = symbol.context.native_handles().ok_or("Actual ledger absent")?;
    let committed = symbol.rig.client.committed_reader_admission(&symbol.context)?.into_origin();
    assert_eq!(committed.retained_bytes(), 3145728);
    assert_eq!(ledger.take_partition(SymbolNativePartition::Transport)?.capacity(), 20);
    let mut outputs = Vec::new();
    for _ in 0..4 {
        outputs.push(provider.reserve_output()?);
    }
    assert!(matches!(provider.reserve_output(), Err(AdmissionError::NativeResources(_))));
    let selected_output = selected.reserve_output()?;
    assert_eq!(ledger.observation().snapshot(), (5, 5));
    drop(outputs);
    drop(selected_output);
    let remainder = symbol.context.memory().claim(8159232 - 4096 - 2097152 - 3145728 - 1048576)?;
    assert!(symbol.context.memory().claim(1).is_err());
    let last = provider.clone();
    assert!(provider.same_origin(&last));
    drop(provider);
    assert!(symbol.context.memory().claim(1).is_err());
    drop(last);
    let reclaimed = symbol.context.memory().claim(1048576)?;
    assert!(selected.check_effect().is_ok());
    assert!(symbol.rig.client.symbol_provider_admission(&symbol.context).is_err());
    drop(reclaimed);
    drop(remainder);
    Ok(())
}

#[test]
fn provider_counterfeit_memory_and_numeric_native_ledger_reject_before_actual_clock() -> TestResult
{
    let symbol = LiveSymbol::new()?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let reads = Arc::new(AtomicUsize::new(0));
    let observed = reads.clone();
    symbol.rig.clock.on_next_current_thread_observation(Box::new(move || {
        observed.fetch_add(1, Ordering::AcqRel);
    }));
    let foreign = supplied(
        &symbol.context,
        MemoryLease::new(8159232, Box::new(ForeignRelease)),
        symbol.context.native_handles().cloned(),
        ports.clone(),
    );
    assert!(symbol.rig.client.symbol_provider_admission(&foreign).is_err());
    let foreign = supplied(
        &symbol.context,
        symbol.context.memory().clone(),
        symbol.context.native_handles().cloned(),
        ports.clone(),
    );
    assert!(symbol.rig.client.symbol_provider_admission(&foreign).is_err());
    assert_eq!(reads.load(Ordering::Acquire), 0);
    let provider = symbol.rig.client.symbol_provider_admission(&symbol.context)?.into_origin();
    assert_eq!(reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    assert!(provider.check_effect().is_ok());
    Ok(())
}

#[test]
fn provider_partition_failure_reclaims_temporary_claim_without_issuing_selected_share() -> TestResult
{
    let symbol = LiveSymbol::new()?;
    let ledger = symbol.context.native_handles().ok_or("Actual ledger absent")?;
    let committed = ledger.take_partition(SymbolNativePartition::Provider)?;
    assert!(matches!(
        symbol.rig.client.symbol_provider_admission(&symbol.context),
        Err(AdmissionError::NativeResources(_))
    ));
    let all_remaining = symbol.context.memory().claim(8159232 - 4096)?;
    drop(all_remaining);
    let selected = symbol.rig.client.symbol_bundle_admission(&symbol.context)?.into_origin();
    assert!(selected.reserve_output().is_ok());
    assert_eq!(committed.capacity(), 4);
    Ok(())
}

#[test]
fn provider_actual_provider_can_close_scope_and_veto_fresh_native_outputs() -> TestResult {
    let symbol = LiveSymbol::new()?;
    let provider = symbol.rig.client.symbol_provider_admission(&symbol.context)?.into_origin();
    let core = symbol.rig.client.core_for_test();
    let scope = symbol.rig.scope.id();
    symbol.rig.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(scope)));
    assert!(matches!(provider.reserve_output(), Err(AdmissionError::ScopeClosed { .. })));
    assert_eq!(
        symbol.context.native_handles().ok_or("Actual ledger absent")?.observation().snapshot(),
        (0, 0)
    );
    Ok(())
}

#[test]
fn provider_cleanup_after_main_ack_reconstructs_actual_ports_and_never_renews_cutoff() -> TestResult
{
    let mut pair = Pair::new()?;
    let provider = pair.rig.client.symbol_provider_admission(&pair.context)?.into_origin();
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion);
    assert!(provider.check_effect().is_err());
    assert_eq!(pair.rig.scope.snapshot().reserved_bytes, 8159232);
    let (mut cleanup, context, deadline) = pair.activate()?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let fake = supplied(&context, context.memory().clone(), None, ports.clone());
    let first = pair.rig.client.symbol_provider_retirement(&provider, &fake)?;
    assert!(first.matches_origin(&provider));
    assert_eq!(first.original_context().now(), Moment::from_nanos(0));
    assert!(!first.original_context().cancellation_requested());
    pair.rig.clock.advance(10000000000);
    let second = pair.rig.client.symbol_provider_retirement(&provider, &fake)?;
    assert_eq!(first.cleanup_cutoff(), deadline.deadline());
    assert_eq!(first.cleanup_cutoff(), second.cleanup_cutoff());
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    pair.rig.clock.advance(5000000000);
    assert!(matches!(first.check(), Err(AdmissionError::DeadlineExpired { .. })));
    assert!(pair.rig.client.symbol_provider_retirement(&provider, &fake).is_err());
    assert_eq!(pair.rig.scope.snapshot().reserved_bytes, 8159232);
    assert!(matches!(pair.finish_cleanup(&mut cleanup)?.outcome, JobOutcome::TimedOut));
    Ok(())
}
