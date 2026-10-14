// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real paired cancellation routing and retained charge without a synthetic provider proof.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
};
use sugarsubstitute_execution::ExecutionOwner;
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
#[test]
fn symbol_receiving_unissued_pair_requires_actual_delivery_and_retains_original_credit()
-> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let main = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut service = SymbolReportService::reserve(
        client.clone(),
        &cleanup,
        JobId::new(1)?,
        &main,
        JobId::new(2)?,
        Moment::from_nanos(60_000_000_017),
    )?;
    assert!(service.take_result().is_none());
    assert_eq!(main.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup.snapshot().reserved_bytes, 32 * 1024);
    service.cancel_unsubmitted()?;
    let actual = loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let actual = client.drain_completions(NonZeroUsize::MIN)?;
        if !actual.notifications.is_empty() {
            break actual;
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == before,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *count == before {
            return Err("Actual unissued Symbol pair did not deliver retirement".into());
        }
    };
    let foreign = CompletionBatch {
        notifications: actual
            .notifications
            .iter()
            .map(|notice| CompletionNotification { scope_id: main.id(), ..*notice })
            .collect(),
        remaining: 0,
    };
    assert!(!service.capture(&foreign));
    assert!(service.take_result().is_none());
    assert!(service.capture(&actual));
    let result = service.take_result().ok_or("Actual Symbol pair result missing")?;
    match &result {
        SymbolReportResult::Failed(failure) => {
            assert!(failure.settled());
            assert!(failure.source().is_some());
        }
        SymbolReportResult::Verified(_) => {
            return Err("Unissued pair produced provider proof".into());
        }
    }
    assert!(service.take_result().is_none());
    drop(service);
    drop(owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 32 * 1024);
    drop(result);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
