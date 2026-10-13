// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual post-admission source metadata through held dispatch, ACK and last alias.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::{
    io,
    sync::atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome, MemoryClaim};
struct Source(Option<Arc<MemoryClaim>>);
impl Clone for Source {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            drop(Arc::into_inner(inner));
        }
    }
}
#[test]
fn original_cleanup_source_claim_precedes_constructor_and_survives_terminal_ack() -> TestResult {
    let rig = Rig::new()?;
    let cleanup_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cleanup_spec = spec(1, JobClass::InfrastructureLifecycle, 32768)?;
    let mut main_spec = spec(2, JobClass::InfrastructureOrdinary, 8159232)?;
    main_spec.deadline = Some(Moment::from_nanos(10000000000));
    let constructors = Arc::new(AtomicUsize::new(0));
    let called = constructors.clone();
    let effects = Arc::new(AtomicUsize::new(0));
    let effects_factory = effects.clone();
    let (source, permit) = rig
        .client
        .reserve_symbol_cleanup_with_source(
            &cleanup_scope,
            cleanup_spec,
            &rig.scope,
            main_spec,
            move |claim| {
                assert_eq!(claim.bytes(), 4096);
                assert_eq!(claim.reserved_bytes(), 32768);
                assert!(size_of::<MemoryClaim>() + 2 * size_of::<usize>() <= 4096);
                called.fetch_add(1, Ordering::AcqRel);
                let source = Source(Some(Arc::new(claim)));
                let held = source.clone();
                (source, move |context: JobContext| {
                    effects_factory.fetch_add(1, Ordering::AcqRel);
                    let remainder =
                        context.memory().claim(32768 - 4096).map_err(io::Error::other)?;
                    assert!(context.memory().claim(1).is_err());
                    drop(remainder);
                    drop(held);
                    Ok::<_, io::Error>(())
                })
            },
        )
        .map_err(|rejected| rejected.error)?;
    assert_eq!(constructors.load(Ordering::Acquire), 1);
    assert_eq!(effects.load(Ordering::Acquire), 0);
    assert_eq!(permit.status().phase, JobPhase::Queued);
    let mut main = rig
        .client
        .submit_symbol_helper(&rig.scope, main_spec, &permit, |_| Ok::<_, io::Error>(()))
        .map_err(|rejected| rejected.error)?;
    drop(rig.completion(&mut main)?);
    let deadline = rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15))?;
    let mut cleanup = permit.begin(deadline).map_err(|rejected| rejected.error)?;
    assert!(matches!(rig.completion(&mut cleanup)?.outcome, JobOutcome::Success(())));
    assert_eq!(effects.load(Ordering::Acquire), 1);
    drop(main);
    drop(cleanup);
    drop(rig.owner);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 32768);
    let last = source.clone();
    drop(source);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 32768);
    drop(last);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn source_constructor_rejection_returns_unexecuted_original_capture_before_source_effect()
-> TestResult {
    let rig = Rig::new()?;
    let cleanup_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cleanup_spec = spec(1, JobClass::InfrastructureOrdinary, 32768)?;
    let mut main_spec = spec(2, JobClass::InfrastructureOrdinary, 8159232)?;
    main_spec.deadline = Some(Moment::from_nanos(10000000000));
    let called = Arc::new(AtomicUsize::new(0));
    let original = called.clone();
    let constructor = move |claim: MemoryClaim| {
        original.fetch_add(1, Ordering::AcqRel);
        (Source(Some(Arc::new(claim))), |_: JobContext| Ok::<_, io::Error>(()))
    };
    let rejected = match rig.client.reserve_symbol_cleanup_with_source(
        &cleanup_scope,
        cleanup_spec,
        &rig.scope,
        main_spec,
        constructor,
    ) {
        Err(rejected) => rejected,
        Ok(_) => return Err("Wrong class admitted source constructor".into()),
    };
    assert_eq!(called.load(Ordering::Acquire), 0);
    assert_eq!(Arc::strong_count(&called), 2);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 0);
    drop(rejected);
    assert_eq!(Arc::strong_count(&called), 1);
    Ok(())
}
