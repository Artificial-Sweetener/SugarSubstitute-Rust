// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise conversion with the actual central owner and fake external counter facts.
use super::*;
use crate::runtime_test_support::{ClockPort, ManualClock, Rig, TestResult, Wake};
use std::{
    cell::Cell,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, Clock, ExecutionError, ExecutionPort, ScopeId, ScopeSpec, TimeError, WaitFuture,
};

#[test]
fn conversion_reads_the_existing_execution_owner_epoch_without_admission() -> TestResult {
    let rig = Rig::new()?;
    rig.clock.advance(20);
    let before = rig.scope.snapshot();
    let deadline = counter_deadline::encode_sampled(
        || Ok((100, 3)),
        || rig.client.counter_moment(),
        Moment::from_nanos(500_000_020),
    )?;
    assert_eq!((deadline.ticks(), deadline.frequency()), (101, 3));
    rig.clock.advance(40);
    let local = counter_deadline::decode_sampled(
        || rig.client.counter_moment(),
        || Ok((100, 3)),
        deadline,
    )?;
    assert_eq!(local, Moment::from_nanos(333_333_393));
    assert_eq!(rig.scope.snapshot(), before);
    Ok(())
}

struct RegressingClock {
    value: AtomicU64,
}
impl Clock for RegressingClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.value.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[test]
fn serialized_provider_regression_fences_actual_admission_and_counter_metadata() -> TestResult {
    let clock = Arc::new(RegressingClock { value: AtomicU64::new(0) });
    let owner = crate::ExecutionOwner::new(clock.clone(), Arc::new(Wake::default()))?;
    let client = owner.client();
    clock.value.store(10, Ordering::Release);
    let original = Moment::from_nanos(20);
    let encoded = counter_deadline::encode_sampled(
        || Ok((100, 1_000_000_000)),
        || client.counter_moment(),
        original,
    )?;
    assert_eq!(encoded.ticks(), 110);
    clock.value.store(5, Ordering::Release);
    let error = counter_deadline::encode_sampled(
        || Ok((100, 1_000_000_000)),
        || client.counter_moment(),
        original,
    )
    .err()
    .ok_or("Regressed provider extended the encoded deadline")?;
    assert_eq!(error.stage(), "execution_counter_clock");
    let execution = error
        .source()
        .and_then(|cause| cause.downcast_ref::<ExecutionError>())
        .ok_or("Original execution cause was lost")?;
    assert_eq!(
        execution
            .source()
            .and_then(Error::source)
            .and_then(|cause| cause.downcast_ref::<TimeError>()),
        Some(&TimeError::Regression),
    );
    assert!(client.core.clock.failed());
    assert_eq!(client.core.clock.retirement_now(), Moment::from_nanos(10));
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    let counter_calls = Cell::new(0);
    let decode = counter_deadline::decode_sampled(
        || client.counter_moment(),
        || {
            counter_calls.set(counter_calls.get() + 1);
            Ok((100, 1_000_000_000))
        },
        encoded,
    );
    assert!(decode.is_err());
    assert_eq!(counter_calls.get(), 0);
    drop(owner);
    Ok(())
}
struct ClockBoundary {
    clock: Arc<ManualClock>,
    caller: std::thread::ThreadId,
    fail: AtomicBool,
    caller_samples: AtomicU64,
}
impl Clock for ClockBoundary {
    fn now(&self) -> Moment {
        if std::thread::current().id() == self.caller {
            self.caller_samples.fetch_add(1, Ordering::AcqRel);
            if self.fail.load(Ordering::Acquire) {
                std::panic::resume_unwind(Box::new(
                    "counter conversion clock boundary failure".to_owned(),
                ));
            }
        }
        ClockPort(self.clock.clone()).now()
    }
    fn sleep_until(&self, until: Moment) -> WaitFuture {
        ClockPort(self.clock.clone()).sleep_until(until)
    }
}
#[test]
fn failed_central_clock_preserves_original_cause_closes_admission_and_cannot_decode_cached_time()
-> TestResult {
    let clock = Arc::new(ClockBoundary {
        clock: Arc::new(ManualClock::default()),
        caller: std::thread::current().id(),
        fail: AtomicBool::new(false),
        caller_samples: AtomicU64::new(0),
    });
    let owner = crate::ExecutionOwner::new(clock.clone(), Arc::new(Wake::default()))?;
    let client = owner.client();
    clock.fail.store(true, Ordering::Release);
    let error = counter_deadline::encode_sampled(
        || Ok((100, 3)),
        || client.counter_moment(),
        Moment::from_nanos(1_000_000_000),
    )
    .err()
    .ok_or("Failed clock encoded a deadline")?;
    assert_eq!(error.stage(), "execution_counter_clock");
    let original = error
        .source()
        .and_then(|cause| cause.downcast_ref::<ExecutionError>())
        .ok_or("Original execution error lost")?;
    assert!(original.source().is_some_and(|cause| {
        cause.to_string().contains("counter conversion clock boundary failure")
    }));
    let samples = clock.caller_samples.load(Ordering::Acquire);
    let counter_calls = Cell::new(0);
    let error = counter_deadline::decode_sampled(
        || client.counter_moment(),
        || {
            counter_calls.set(counter_calls.get() + 1);
            Ok((100, 3))
        },
        CounterDeadline::new(102, 3)?,
    )
    .err()
    .ok_or("Failed clock decoded cached time")?;
    assert_eq!(error.stage(), "execution_counter_clock");
    assert_eq!(counter_calls.get(), 0);
    assert_eq!(clock.caller_samples.load(Ordering::Acquire), samples);
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    drop(owner);
    assert_eq!(clock.caller_samples.load(Ordering::Acquire), samples);
    Ok(())
}
