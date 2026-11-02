// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise shared allocation claims and the admission reservation's last owner.

use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct Releases {
    calls: AtomicUsize,
    bytes: AtomicU64,
}

struct ReleaseCounter(Arc<Releases>);
impl MemoryRelease for ReleaseCounter {
    fn release(&self, bytes: u64) {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.bytes.fetch_add(bytes, Ordering::SeqCst);
    }
}

fn lease(bytes: u64, released: &Arc<Releases>) -> MemoryLease {
    MemoryLease::new(bytes, Box::new(ReleaseCounter(released.clone())))
}

#[test]
fn cloned_result_guards_release_one_charge_only_after_last_drop() {
    let released = Arc::new(Releases::default());
    let lease = lease(64, &released);
    let retained_result = lease.clone();
    let second_result = retained_result.clone();
    assert_eq!(second_result.bytes(), 64);
    drop(lease);
    drop(retained_result);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(second_result);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 64);
}

#[test]
fn cloned_leases_share_aggregate_capacity_and_dropped_claims_are_reusable()
-> Result<(), MemoryClaimError> {
    let released = Arc::new(Releases::default());
    let lease = lease(64, &released);
    let other = lease.clone();
    let first = lease.claim(40)?;
    assert_eq!(first.bytes(), 40);
    assert_eq!(first.reserved_bytes(), 64);
    assert!(matches!(
        other.claim(25),
        Err(MemoryClaimError { requested_bytes: 25, available_bytes: 24 })
    ));
    let second = other.claim(24)?;
    assert!(matches!(
        lease.claim(1),
        Err(MemoryClaimError { requested_bytes: 1, available_bytes: 0 })
    ));
    drop(first);
    let replacement = other.claim(40)?;
    drop(lease);
    drop(other);
    drop(second);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(replacement);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 64);
    Ok(())
}

#[test]
fn overflowing_aggregate_is_rejected_without_consuming_capacity() -> Result<(), MemoryClaimError> {
    let released = Arc::new(Releases::default());
    let lease = lease(u64::MAX, &released);
    let full = lease.claim(u64::MAX)?;
    assert!(matches!(
        lease.claim(1),
        Err(MemoryClaimError { requested_bytes: 1, available_bytes: 0 })
    ));
    drop(full);
    let reused = lease.claim(u64::MAX)?;
    drop(reused);
    drop(lease);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), u64::MAX);
    Ok(())
}

#[test]
fn zero_byte_claim_retains_reservation_without_consuming_capacity() -> Result<(), MemoryClaimError>
{
    let released = Arc::new(Releases::default());
    let lease = lease(4, &released);
    let empty = lease.claim(0)?;
    let full = lease.claim(4)?;
    assert_eq!(empty.bytes(), 0);
    drop(lease);
    drop(full);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(empty);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.bytes.load(Ordering::SeqCst), 4);
    Ok(())
}

#[test]
fn borrowed_reservation_identity_rejects_equal_foreign_bytes_and_retains_original_release() {
    let original_releases = Arc::new(Releases::default());
    let foreign_releases = Arc::new(Releases::default());
    let original = lease(64, &original_releases);
    let alias = original.clone();
    let foreign = lease(64, &foreign_releases);
    assert!(original.same_reservation(&alias));
    assert!(!original.same_reservation(&foreign));
    assert_eq!(original_releases.calls.load(Ordering::SeqCst), 0);
    assert_eq!(foreign_releases.calls.load(Ordering::SeqCst), 0);
    drop(original);
    assert_eq!(original_releases.calls.load(Ordering::SeqCst), 0);
    drop(alias);
    assert_eq!(original_releases.calls.load(Ordering::SeqCst), 1);
    drop(foreign);
    assert_eq!(foreign_releases.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn carved_leases_isolate_their_bytes_until_their_last_claim_drops() -> Result<(), MemoryClaimError>
{
    let released = Arc::new(Releases::default());
    let original = lease(100, &released);
    let carved = original.claim(40)?.into_lease();
    assert_eq!(carved.bytes(), 40);
    assert!(!carved.same_reservation(&original));
    // The original can no longer reach the carved bytes, and the carved lease cannot grow.
    assert_eq!(
        original.claim(61).map(|_| ()),
        Err(MemoryClaimError { requested_bytes: 61, available_bytes: 60 })
    );
    let surface = carved.claim(40)?;
    assert!(carved.claim(1).is_err());
    let cache = original.claim(60)?;
    drop(carved);
    assert!(original.claim(1).is_err());
    drop(surface);
    drop(cache);
    assert_eq!(original.claim(100)?.bytes(), 100);
    assert_eq!(released.calls.load(Ordering::SeqCst), 0);
    drop(original);
    assert_eq!(released.calls.load(Ordering::SeqCst), 1);
    Ok(())
}
