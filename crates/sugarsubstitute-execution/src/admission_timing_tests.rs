// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise genuine receiving scopes without manufacturing records or native authority.
use super::*;
use crate::{
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake},
    scope::lock,
};
use sugarsubstitute_contracts::execution::{ExecutionId, ScopeSpec};

fn core() -> TestResult<Arc<Core>> {
    Ok(Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
    ))
}

#[test]
fn closed_receiving_ancestor_rejects_even_without_synthetic_cancellation() -> TestResult {
    let core = core()?;
    let parent = core.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let receiving = core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        Some(&parent),
        true,
    )?;
    assert_eq!(receiving_cutoff(&lock(&core.state), receiving.id(), Moment::from_nanos(0))?, None);
    core.close_scope(parent.id());
    assert!(parent.snapshot().closed);
    assert!(!parent.snapshot().cancellation_requested);
    assert!(!receiving.snapshot().closed);
    assert!(!receiving.snapshot().cancellation_requested);
    assert!(
        matches!(receiving_cutoff(&lock(&core.state), receiving.id(), Moment::from_nanos(0)), Err(AdmissionError::ScopeClosed { scope_id }) if scope_id == parent.id())
    );
    assert_eq!(receiving.snapshot().reserved_bytes, 0);
    assert_eq!(receiving.snapshot().queued, 0);
    Ok(())
}

#[test]
fn narrowed_actual_receiving_ancestor_caps_handshake_without_rewriting_scope_spec() -> TestResult {
    let core = core()?;
    let parent = core.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let receiving = core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        Some(&parent),
        true,
    )?;
    let cutoff = Moment::from_nanos(10);
    lock(&core.state).scopes.get_mut(&parent.id()).ok_or("Actual ancestor missing")?.deadline =
        Some(cutoff);
    assert_eq!(
        receiving_cutoff(&lock(&core.state), receiving.id(), Moment::from_nanos(9))?,
        Some(cutoff)
    );
    assert_eq!(receiving.spec().deadline, None);
    assert!(
        matches!(receiving_cutoff(&lock(&core.state), receiving.id(), cutoff), Err(AdmissionError::DeadlineExpired { deadline, now }) if deadline == cutoff && now == cutoff)
    );
    assert_eq!(receiving.snapshot().reserved_bytes, 0);
    Ok(())
}
