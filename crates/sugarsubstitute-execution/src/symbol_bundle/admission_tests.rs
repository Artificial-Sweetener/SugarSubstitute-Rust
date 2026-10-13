// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove actual selected partition authority, same-ledger provenance and original control ports.
use super::test_support::*;
use crate::{runtime_test_support::TestResult, symbol_test_support::LiveSymbol};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;
#[test]
fn selected_share_issues_once_and_final_origin_reclaims_only_original_two_mib() -> TestResult {
    let symbol = LiveSymbol::new()?;
    let origin = symbol.rig.client.symbol_bundle_admission(&symbol.context)?.into_origin();
    assert!(origin.control_storage_bytes() <= 65536);
    assert_eq!(origin.retained_bytes(), 2097152);
    assert!(symbol.rig.client.symbol_bundle_admission(&symbol.context).is_err());
    let ledger = symbol.context.native_handles().ok_or("Actual ledger absent")?;
    let files = ledger.take_partition(SymbolNativePartition::CommittedReader)?;
    let provider = ledger.take_partition(SymbolNativePartition::Provider)?;
    let transport = ledger.take_partition(SymbolNativePartition::Transport)?;
    assert_eq!((files.capacity(), provider.capacity(), transport.capacity()), (140, 4, 20));
    let mut outputs = Vec::new();
    for _ in 0..96 {
        outputs.push(origin.reserve_output()?);
    }
    assert!(matches!(origin.reserve_output(), Err(AdmissionError::NativeResources(_))));
    assert_eq!(ledger.observation().snapshot(), (96, 96));
    drop(outputs);
    assert_eq!(ledger.observation().snapshot(), (0, 96));
    let remainder = symbol.context.memory().claim(8159232 - 2097152 - 4096)?;
    assert!(symbol.context.memory().claim(1).is_err());
    let final_alias = origin.clone();
    assert!(origin.same_origin(&final_alias));
    drop(origin);
    assert!(symbol.context.memory().claim(1).is_err());
    drop(final_alias);
    let reusable = symbol.context.memory().claim(2097152)?;
    drop(reusable);
    drop(remainder);
    assert!(symbol.rig.client.symbol_bundle_admission(&symbol.context).is_err());
    Ok(())
}
#[test]
fn counterfeit_memory_or_same_numeric_native_ledger_rejects_before_provider_and_partition()
-> TestResult {
    let symbol = LiveSymbol::new()?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let reads = Arc::new(AtomicUsize::new(0));
    let callback = reads.clone();
    symbol.rig.clock.on_next_current_thread_observation(Box::new(move || {
        callback.fetch_add(1, Ordering::AcqRel);
    }));
    let fake = supplied(
        &symbol.context,
        MemoryLease::new(8159232, Box::new(ForeignRelease)),
        symbol.context.native_handles().cloned(),
        ports.clone(),
    );
    assert!(symbol.rig.client.symbol_bundle_admission(&fake).is_err());
    let fake = supplied(
        &symbol.context,
        symbol.context.memory().clone(),
        symbol.context.native_handles().cloned(),
        ports.clone(),
    );
    assert!(symbol.rig.client.symbol_bundle_admission(&fake).is_err());
    assert_eq!(reads.load(Ordering::Acquire), 0);
    let origin = symbol.rig.client.symbol_bundle_admission(&symbol.context)?.into_origin();
    assert_eq!(reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    assert!(origin.check_effect().is_ok());
    Ok(())
}
#[test]
fn actual_clock_callback_scope_close_vetoes_original_effect_without_invoking_supplied_ports()
-> TestResult {
    let symbol = LiveSymbol::new()?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let fake = supplied(
        &symbol.context,
        symbol.context.memory().clone(),
        symbol.context.native_handles().cloned(),
        ports.clone(),
    );
    assert!(symbol.rig.client.symbol_bundle_admission(&fake).is_err());
    let origin = symbol.rig.client.symbol_bundle_admission(&symbol.context)?.into_origin();
    assert_eq!(origin.original_context()?.now(), Moment::from_nanos(0));
    assert!(!origin.original_context()?.cancellation_requested());
    let core = symbol.rig.client.core_for_test();
    let scope = symbol.rig.scope.id();
    symbol.rig.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(scope)));
    assert!(matches!(origin.check_effect(), Err(AdmissionError::ScopeClosed { .. })));
    assert!(matches!(origin.reserve_output(), Err(AdmissionError::ScopeClosed { .. })));
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    Ok(())
}
