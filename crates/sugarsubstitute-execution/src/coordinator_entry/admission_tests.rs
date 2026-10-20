// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject persistent-scope deadlines and storage/horizon overflow before native preparation.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::*;
fn no_effect(boundary: &Boundary) {
    assert!(lock(&boundary.memory).is_none());
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.native_closes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
}
#[test]
fn persistent_session_rejects_job_and_inherited_ancestor_deadlines_before_effects() -> TestResult {
    for job_deadline in [true, false] {
        let rig = Rig::new()?;
        let boundary = Boundary::new(&rig, true)?;
        let parent = rig.client.core.create_scope_kind(
            ScopeSpec {
                id: ScopeId::new(2)?,
                deadline: (!job_deadline).then_some(Moment::from_nanos(30_000_000_000)),
            },
            None,
            true,
        )?;
        let child = rig.client.core.create_scope_kind(
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            Some(&parent),
            true,
        )?;
        let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_SERVER_BYTES)?;
        specification.operation = Operation::NativeLifecycle;
        specification.deadline = job_deadline.then_some(Moment::from_nanos(30_000_000_000));
        assert!(matches!(
            reserve_server(
                &rig.client.core,
                &child,
                specification,
                CoordinatorEntryServerInput::new(facts()?)?,
                Moment::from_nanos(30_000_000_000),
                Box::new(Lease(boundary.clone()))
            ),
            Err(EntryTransportAdmissionError::ConflictingDeadline)
        ));
        no_effect(&boundary);
    }
    Ok(())
}
#[test]
fn storage_and_original_preparation_horizon_reject_before_effects() -> TestResult {
    for too_small in [true, false] {
        let rig = Rig::new()?;
        let boundary = Boundary::new(&rig, true)?;
        let scope = native_scope(&rig)?;
        let mut specification =
            spec(1, JobClass::NativeObservation, MIN_ENTRY_SERVER_BYTES - u64::from(too_small))?;
        specification.operation = Operation::NativeLifecycle;
        let outcome = reserve_server(
            &rig.client.core,
            &scope,
            specification,
            CoordinatorEntryServerInput::new(facts()?)?,
            Moment::from_nanos(if too_small { 30_000_000_000 } else { 3_600_000_000_001 }),
            Box::new(Lease(boundary.clone())),
        );
        if too_small {
            assert!(matches!(
                outcome,
                Err(EntryTransportAdmissionError::Admission(
                    AdmissionError::NativeReservationTooSmall { .. }
                ))
            ));
        } else {
            assert!(
                matches!(outcome,Err(EntryTransportAdmissionError::Clock(ref cause)) if cause.stage()=="entry_preparation_horizon")
            );
        }
        no_effect(&boundary);
    }
    Ok(())
}
#[test]
fn client_actual_frequency_mismatch_rejects_before_native_preparation() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    let scope = native_scope(&rig)?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let input = CoordinatorEntryClientInput::new(
        facts()?,
        None,
        request()?,
        CounterDeadline::new(source()?.ticks(), 1_000_000_001)?,
    )?;
    assert!(matches!(
        reserve_client(
            &rig.client.core,
            &scope,
            specification,
            input,
            Box::new(Lease(boundary.clone()))
        ),
        Err(EntryTransportAdmissionError::Clock(_))
    ));
    no_effect(&boundary);
    Ok(())
}
