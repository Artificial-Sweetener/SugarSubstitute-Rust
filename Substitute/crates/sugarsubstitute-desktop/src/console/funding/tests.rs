// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise genuine claim capacity and physical frame identity through the actual funding owner.
use super::*;
use crate::console::index::tests::{TestResult, frame};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape, FrameValidator},
};
#[test]
fn repeated_exposure_coalesces_without_claiming_twice_and_equal_metadata_new_storage_is_independent()
-> TestResult {
    let (first, first_release) = frame()?;
    let (mut funding, first_copy) = Funding::new(&first)?;
    assert!(funding.next(&first)?.is_none());
    assert!(funding.next(&first.clone())?.is_none());
    assert!(first.claim_storage(1).is_err());
    let (second, second_release) = frame()?;
    assert_eq!(first.identity(), second.identity());
    let second_copy =
        funding.next(&second)?.ok_or("new actual allocation was mistaken for exposure")?;
    assert!(funding.next(&second)?.is_none());
    assert!(second.claim_storage(1).is_err());
    drop(first);
    assert_eq!(first_release.load(Ordering::SeqCst), 0);
    drop(first_copy);
    assert_eq!(first_release.load(Ordering::SeqCst), 1);
    drop((second, second_copy));
    assert_eq!(second_release.load(Ordering::SeqCst), 0);
    drop(funding);
    assert_eq!(second_release.load(Ordering::SeqCst), 1);
    Ok(())
}
struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _bytes: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn missing_new_frame_allowance_rejects_before_copy_and_keeps_original_funding_identity()
-> TestResult {
    let (original, original_release) = frame()?;
    let (mut funding, copy) = Funding::new(&original)?;
    let released = Arc::new(AtomicUsize::new(0));
    // This external-boundary fixture has only its already-admitted16 pixel bytes.
    let lease = MemoryLease::new(16, Box::new(Release(released.clone())));
    let mut validator = FrameValidator::new(
        FrameShape::new(2, 2, 1.0)?,
        FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(2)?,
        },
        vec![0; 16],
        lease,
    )?;
    assert!(validator.validate_next(NonZeroUsize::MIN.saturating_add(3))?);
    let rejected = validator.finish()?;
    let error = funding.next(&rejected).err().ok_or("unadmitted native copy accepted")?;
    assert!(std::error::Error::source(&error).and_then(|e|e.downcast_ref::<sugarsubstitute_contracts::execution::MemoryClaimError>()).is_some());
    assert!(funding.next(&original)?.is_none());
    drop(rejected);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    drop((original, funding));
    assert_eq!(original_release.load(Ordering::SeqCst), 0);
    drop(copy);
    assert_eq!(original_release.load(Ordering::SeqCst), 1);
    Ok(())
}
