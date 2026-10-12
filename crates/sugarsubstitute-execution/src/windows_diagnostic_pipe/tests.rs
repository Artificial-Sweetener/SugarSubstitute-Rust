// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject renewable job/ancestor budgets before inherited adoption or native pipe creation.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use sugarsubstitute_contracts::{
    execution::{JobClass, ScopeId, ScopeSpec},
    incident::DiagnosticSessionId,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};
fn key() -> TestResult<DiagnosticKey> {
    Ok(DiagnosticKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        DiagnosticRole::Storage,
        DiagnosticSessionId::new(1)?,
    ))
}
#[test]
fn actual_diagnostic_server_and_client_reject_job_deadline_before_any_effect() -> TestResult {
    let rig = Rig::new()?;
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?;
    let mut job = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    job.deadline = Some(Moment::from_nanos(100));
    assert!(matches!(
        rig.client.reserve_diagnostic_pipe_server(&scope, job, key()?, Moment::from_nanos(1000)),
        Err(LifecyclePipeAdmissionError::ConflictingDeadline)
    ));
    assert!(matches!(
        rig.client.reserve_diagnostic_pipe_client(
            &scope,
            job,
            DiagnosticRole::Storage,
            LifecyclePipeDeadline::new(1000, 100)?
        ),
        Err(LifecyclePipeAdmissionError::ConflictingDeadline)
    ));
    assert_eq!(rig.client.core.scope_snapshot(scope.id()).reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_diagnostic_server_and_client_reject_effective_ancestor_deadline_before_any_effect()
-> TestResult {
    let rig = Rig::new()?;
    let parent = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(100)) },
        None,
        true,
    )?;
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        Some(&parent),
        true,
    )?;
    let job = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    assert!(matches!(
        rig.client.reserve_diagnostic_pipe_server(&scope, job, key()?, Moment::from_nanos(1000)),
        Err(LifecyclePipeAdmissionError::ConflictingDeadline)
    ));
    assert!(matches!(
        rig.client.reserve_diagnostic_pipe_client(
            &scope,
            job,
            DiagnosticRole::Storage,
            LifecyclePipeDeadline::new(1000, 100)?
        ),
        Err(LifecyclePipeAdmissionError::ConflictingDeadline)
    ));
    assert_eq!(rig.client.core.scope_snapshot(scope.id()).reserved_bytes, 0);
    Ok(())
}
