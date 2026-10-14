// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual cancelled unbound worker custody and all original credits.
use super::*;
#[test]
fn cancelled_original_unbound_start_returns_same_job_and_keeps_credits_until_delivery_disposal()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let mut input = Some(session.control.take_worker_permit()?);
    let mut specification =
        spec(2, JobClass::NativeObservation, crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let start = rig.client.reserve_backend_custody_worker(&scope, specification, &mut input)?;
    let original = start.metadata().clone();
    assert_eq!(
        scope.snapshot().reserved_bytes,
        crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES + MIN_LIFECYCLE_PIPE_BYTES
    );
    let mut job = start.cancel().ok_or("Original unbound observer missing")?;
    assert_eq!(job.spec(), specification);
    assert_eq!(job.scope_id(), scope.id());
    let completion = rig.completion(&mut job)?;
    assert_eq!(completion.scope_id, scope.id());
    assert_eq!(completion.spec, specification);
    assert_eq!(completion.memory.bytes(), crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES);
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert!(original.check_registration().is_err());
    drop(original);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES + MIN_LIFECYCLE_PIPE_BYTES
    );
    drop(completion);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    drop(session);
    drop(boundary);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
