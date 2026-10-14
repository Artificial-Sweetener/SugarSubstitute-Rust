// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real incomplete/failed preparation cannot create native retirement or accepted disposal.
use super::*;
fn inert_binding() -> Result<ReportBinding> {
    use sugarsubstitute_contracts::lifecycle::*;
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(5)?,
        ReportSessionId::new(6)?,
    ))
}
#[test]
fn retirement_phase_actual_unobserved_preparation_cannot_issue_recycle_or_disposal() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut receiving = IncidentReporting::new();
    receiving.receiving_session.preparing_mut()?.metadata.job =
        Some(client.submit_blocking(&scope, spec()?, |context| {
            Err(delivery::Failure::test_cause(&context, "Original phase failure"))
        })?);
    assert!(receiving.receiving_session.retire_healthy_preparation(inert_binding()?).is_err());
    assert!(receiving.receiving_session.preparing()?.metadata.job.is_some());
    assert!(receiving.receiving_session.recycled().is_none());
    assert!(receiving.receiving_session.retired().is_none());
    assert!(receiving.receiving_session.validate_disposal().is_err());
    assert!(receiving.normal_close_budget().is_none());
    receiving.capture(&batch(&client, &wake)?)?;
    assert!(receiving.receiving_session.retire_healthy_preparation(inert_binding()?).is_err());
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn retirement_phase_repeated_failure_denial_keeps_same_cause_and_original_delivery() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut receiving = IncidentReporting::new();
    receiving.receiving_session.preparing_mut()?.metadata.job =
        Some(client.submit_blocking(&scope, spec()?, |context| {
            Err(delivery::Failure::test_cause(&context, "Original retained phase failure"))
        })?);
    receiving.capture(&batch(&client, &wake)?)?;
    let original =
        std::ptr::from_ref(receiving.failure().ok_or("Actual phase cause missing")?).cast::<()>();
    for _ in 0..2 {
        assert!(receiving.receiving_session.retire_healthy_preparation(inert_binding()?).is_err());
        assert!(receiving.receiving_session.recycled().is_none());
        assert!(receiving.receiving_session.retired().is_none());
        assert!(receiving.receiving_session.validate_disposal().is_err());
        assert_eq!(
            std::ptr::from_ref(receiving.failure().ok_or("Original phase cause lost")?)
                .cast::<()>(),
            original
        );
        assert!(receiving.receiving_session.preparing()?.metadata.completion.is_some());
    }
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    assert_eq!(
        std::ptr::from_ref(receiving.failure().ok_or("Cause lost after join")?).cast::<()>(),
        original
    );
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
