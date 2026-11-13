// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real reservation custody and reject underfunded metadata before effects.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
#[test]
fn historical_owner_rejects_small_claim_and_retains_exact_original_until_final_drop()
-> Result<(), Box<dyn std::error::Error>> {
    let released = Arc::new(AtomicU64::new(0));
    let bytes = HistoricalCaptureRecovery::required_storage_bytes();
    let lease = MemoryLease::new(bytes, Box::new(Release(released.clone())));
    let failed = HistoricalCaptureRecovery::new(lease.claim(bytes - 1)?)
        .err()
        .ok_or("small claim accepted")?;
    assert_eq!(failed.requested_bytes, bytes);
    assert_eq!(failed.available_bytes, bytes - 1);
    let mut owner = HistoricalCaptureRecovery::new(lease.claim(bytes)?)?;
    assert!(owner.reset().is_err());
    assert_eq!(owner.state(), None);
    assert!(owner.report().is_none());
    assert!(owner.committed().is_none());
    assert_eq!(owner.frame_failure(), None);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(owner);
    assert_eq!(released.load(Ordering::SeqCst), bytes);
    Ok(())
}
