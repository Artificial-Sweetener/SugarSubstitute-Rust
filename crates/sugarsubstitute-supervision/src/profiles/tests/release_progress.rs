// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve unheld partial release and require real completion consumption before retirement.
use super::super::*;
use super::support::{Fake, Harness, TestResult};
use std::error::Error;
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// Model a native root-close failure after the original file exclusion has closed.
struct PartialRootClose {
    filesystem: Arc<Fake>,
    failure: Mutex<Option<ClaimError>>,
}
impl storage::ProfilePort for PartialRootClose {
    fn acquire(
        &self,
        state: &mut storage::NativeState,
        path: &std::path::Path,
        run: RunKey,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.filesystem.acquire(state, path, run, context)
    }
    fn validate_environment(
        &self,
        state: &mut storage::NativeState,
        observed: &[u16],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.filesystem.validate_environment(state, observed, context)
    }
    fn release(
        &self,
        state: &mut storage::NativeState,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        storage::check(context)?;
        if let Some(cause) = lock(&self.failure).take() {
            state.held = false;
            state.release_published = true;
            return Err(cause);
        }
        self.filesystem.release(state, context)
    }
}

#[test]
fn unheld_partial_release_can_finish_with_same_cutoff_and_retains_first_outcome() -> TestResult {
    let mut harness = Harness::new()?;
    assert_eq!(harness.acquire()?.kind, TerminalKind::Success);
    let original = ClaimError::new(
        "external_root_close",
        std::io::Error::other("Original partial native close"),
    );
    let original_source = std::ptr::from_ref(original.source().ok_or("Original source absent")?);
    harness.lease.port = Arc::new(PartialRootClose {
        filesystem: harness.fake.clone(),
        failure: Mutex::new(Some(original)),
    });
    let first = harness.release(100)?;
    assert_eq!(first.kind, TerminalKind::Failure);
    assert!(!harness.lease.snapshot().held);
    assert!(!harness.lease.snapshot().pending);
    assert!(!harness.lease.snapshot().retirement_ready());
    assert_eq!(
        std::ptr::from_ref(
            first
                .failure
                .as_ref()
                .ok_or("First cause absent")?
                .source()
                .ok_or("First source absent")?
        ),
        original_source
    );

    let second_spec = harness.spec(100)?;
    harness.lease.submit_release(
        &harness.client,
        &harness.scope,
        second_spec,
        &harness.registry,
    )?;
    let batch = harness.terminal_batch()?;
    assert!(harness.lease.snapshot().released);
    assert!(harness.lease.snapshot().pending);
    assert!(!harness.lease.snapshot().retirement_ready());
    assert_eq!(harness.lease.release_deadline, Some(Moment::from_nanos(100)));
    let second = harness.lease.consume(&batch)?.ok_or("Original second completion absent")?;
    assert_eq!(second.job, second_spec.id);
    assert_eq!(second.kind, TerminalKind::Success);
    assert!(second.failure.is_none());
    assert!(harness.lease.snapshot().retirement_ready());
    assert_eq!(
        std::ptr::from_ref(
            first
                .failure
                .as_ref()
                .ok_or("Retained first cause absent")?
                .source()
                .ok_or("Retained first source absent")?
        ),
        original_source
    );
    assert_eq!(harness.fake.releases.load(Ordering::Acquire), 1);
    harness.finish_workers();
    Ok(())
}

#[test]
fn actual_released_atomic_does_not_retire_an_unconsumed_owned_job() -> TestResult {
    let mut harness = Harness::new()?;
    assert_eq!(harness.acquire()?.kind, TerminalKind::Success);
    let spec = harness.spec(100)?;
    harness.lease.submit_release(&harness.client, &harness.scope, spec, &harness.registry)?;
    let batch = harness.terminal_batch()?;
    assert!(harness.lease.snapshot().released);
    assert!(!harness.lease.snapshot().retirement_ready());
    let rejected = harness.spec(100)?;
    assert!(matches!(
        harness.lease.submit_release(&harness.client, &harness.scope, rejected, &harness.registry),
        Err(ClaimAdmissionError::Busy)
    ));
    let completion = harness.lease.consume(&batch)?.ok_or("Original release completion absent")?;
    assert_eq!(completion.job, spec.id);
    assert_eq!(completion.kind, TerminalKind::Success);
    assert!(harness.lease.snapshot().retirement_ready());
    harness.finish_workers();
    Ok(())
}
