// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise same-allocation data custody with real Execution claims, without native retirement capabilities.
use super::*;
use sugarsubstitute_contracts::lifecycle::{LifecycleFailure, LifecycleFailureCode};
type TestResult = std::result::Result<(), Box<dyn Error + Send + Sync>>;
fn owner() -> std::result::Result<BackendCauses, Box<dyn Error + Send + Sync>> {
    Ok(BackendCauses::new(crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )?)?)
}
fn failure() -> std::result::Result<SettledAttemptFailure, Box<dyn Error + Send + Sync>> {
    let identity = super::super::tests::identity().map_err(|cause| cause.to_string())?;
    let resource = sugarsubstitute_contracts::lifecycle::ResourceKey {
        owner: identity.run.owner,
        id: sugarsubstitute_contracts::lifecycle::ResourceId::new(1)?,
        generation: sugarsubstitute_contracts::lifecycle::ResourceGeneration::new(1)?,
    };
    Ok(SettledAttemptFailure {
        identity,
        resource,
        cause: LifecycleFailure::new(
            LifecycleFailureCode::UnexpectedExit,
            Box::new(std::io::Error::other("Original producer cause")),
        ),
    })
}
#[test]
fn boxed_eight_slot_transfer_preserves_exact_cause_and_original_claim_without_a_second_allocation()
-> TestResult {
    let mut source = Some(owner()?);
    let mut destination = None;
    let original = failure()?;
    let cause = std::ptr::from_ref(original.cause.source().ok_or("Original cause absent")?);
    *source.as_mut().ok_or("Source absent")?.free_slot().ok_or("Free slot absent")? =
        Some(original);
    let allocation = source.as_ref().ok_or("Source absent")?.allocation();
    let reserved = source.as_ref().ok_or("Source absent")?.claim().reserved_bytes();
    BackendCauses::transfer(&mut source, &mut destination)?;
    assert!(source.is_none());
    let moved = destination.as_ref().ok_or("Destination absent")?;
    assert_eq!(moved.allocation(), allocation);
    assert_eq!(moved.claim().reserved_bytes(), reserved);
    assert_eq!(moved.claim().bytes(), ParentLifecycle::storage_bytes());
    assert!(std::ptr::eq(
        std::ptr::from_ref(
            moved.first().ok_or("Cause lost")?.cause.source().ok_or("Producer source lost")?
        ),
        cause
    ));
    assert!(!moved.full());
    assert!(moved.contains(moved.first().ok_or("Metadata lost")?.resource));
    Ok(())
}
#[test]
fn occupied_destination_rejects_before_either_original_box_or_cause_moves() -> TestResult {
    let mut source = Some(owner()?);
    let mut destination = Some(owner()?);
    *source.as_mut().ok_or("Source absent")?.free_slot().ok_or("Slot absent")? = Some(failure()?);
    let before = source.as_ref().ok_or("Source absent")?.allocation();
    let other = destination.as_ref().ok_or("Destination absent")?.allocation();
    assert_eq!(
        BackendCauses::transfer(&mut source, &mut destination),
        Err(ParentError::Overloaded)
    );
    assert_eq!(source.as_ref().ok_or("Source lost")?.allocation(), before);
    assert_eq!(destination.as_ref().ok_or("Destination lost")?.allocation(), other);
    assert!(source.as_ref().ok_or("Cause lost")?.first().is_some());
    Ok(())
}
#[test]
fn unknown_unsettled_parent_refuses_extraction_and_keeps_real_owner() -> TestResult {
    let identity = super::super::tests::identity().map_err(|cause| cause.to_string())?;
    let mut storage = Some(owner()?);
    let original = storage.as_ref().ok_or("Storage absent")?.allocation();
    let mut parent = ParentLifecycle::new(
        identity.run,
        identity.launch,
        sugarsubstitute_contracts::configuration::ProfileId::new("Cause_1".into())?,
        sugarsubstitute_contracts::lifecycle::ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        &mut storage,
    )?;
    assert!(storage.is_none());
    assert!(parent.take_retired_backend_causes().is_err());
    parent.closed = true;
    assert!(parent.take_retired_backend_causes().is_err());
    assert_eq!(
        parent.backend_causes.as_ref().ok_or("Unsettled owner lost")?.allocation(),
        original
    );
    Ok(())
}
