// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve the first actual terminal result and destroy rejected ownership outside Core.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, TerminalKind};

struct ReentrantResult {
    core: Arc<crate::admission::Core>,
    scope_id: ScopeId,
    dropped: Arc<AtomicUsize>,
}
impl Drop for ReentrantResult {
    fn drop(&mut self) {
        let _actual = self.core.scope_snapshot(self.scope_id);
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}

#[test]
fn symbol_duplicate_terminal_preserves_first_actual_result_and_drops_outside_core() -> TestResult {
    let rig = Rig::new()?;
    let context = Arc::new(Mutex::new(None));
    let stored = context.clone();
    let dropped = Arc::new(AtomicUsize::new(0));
    let result = ReentrantResult {
        core: rig.client.core.clone(),
        scope_id: rig.scope.id(),
        dropped: dropped.clone(),
    };
    let mut actual = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        move |context| {
            *lock(&stored) = Some(context);
            Ok::<_, ()>(result)
        },
    )?;
    loop {
        let observed = rig.wake.count();
        if matches!(actual.status().phase, JobPhase::Terminal(_)) {
            break;
        }
        rig.wake.wait_after(observed)?;
    }
    let context = lock(&context).take().ok_or("Actual context absent")?;
    let job = lock(&rig.client.core.state)
        .jobs
        .get(&actual.spec().id)
        .map(|record| record.job.clone())
        .ok_or("Actual terminal owner absent")?;
    job.panicked(&context, context.now(), panic_cause(Box::new("duplicate".to_owned())));
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    assert_eq!(actual.status().phase, JobPhase::Terminal(TerminalKind::Success));
    let completion = rig.completion(&mut actual)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    drop(completion);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    drop(context);
    drop(job);
    drop(actual);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
