// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pure observation tests use the real admitted lease/claim owner.

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
fn lease(bytes: u64) -> (MemoryLease, Arc<AtomicU64>) {
    let released = Arc::new(AtomicU64::new(0));
    (MemoryLease::new(bytes, Box::new(Release(released.clone()))), released)
}

#[test]
fn exact_utf16_observation_retains_last_guard_and_redacts_path() -> Result<(), Box<dyn Error>> {
    let (lease, released) = lease(MainEnvironmentFacts::MAX_STORAGE_BYTES);
    let units = vec![b'E' as u16, b':' as u16, b'\\' as u16, 0xd800, b'x' as u16];
    let facts = MainEnvironmentFacts::new(
        units.clone(),
        lease.claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)?,
    )?;
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert_eq!(facts.as_utf16(), units);
    assert_eq!(facts.claimed_bytes(), MainEnvironmentFacts::MAX_STORAGE_BYTES);
    assert!(!format!("{facts:?}").contains("E:"));
    drop(facts);
    assert_eq!(released.load(Ordering::SeqCst), MainEnvironmentFacts::MAX_STORAGE_BYTES);
    Ok(())
}

#[test]
fn malformed_or_excess_capacity_cannot_become_retained_facts() -> Result<(), Box<dyn Error>> {
    for variant in 0..4 {
        let (lease, _) = lease(MainEnvironmentFacts::MAX_STORAGE_BYTES);
        let units = match variant {
            0 => Vec::new(),
            1 => vec![b'x' as u16, 0],
            2 => vec![b'x' as u16; MainEnvironmentFacts::MAX_UNITS + 1],
            _ => {
                let mut units = Vec::with_capacity(MainEnvironmentFacts::MAX_UNITS + 1);
                units.push(b'x' as u16);
                units
            }
        };
        assert!(
            MainEnvironmentFacts::new(units, lease.claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)?)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn actual_capacity_plus_metadata_requires_real_claim_and_exhausted_pool_rejects()
-> Result<(), Box<dyn Error>> {
    let (lease, _) = lease(MainEnvironmentFacts::MAX_STORAGE_BYTES);
    let mut units = Vec::with_capacity(100);
    units.push(b'x' as u16);
    let insufficient = lease.claim(2)?;
    assert!(matches!(
        MainEnvironmentFacts::new(units, insufficient),
        Err(MainEnvironmentError::InsufficientClaim)
    ));
    let held = lease.claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)?;
    assert!(lease.claim(MainEnvironmentFacts::MAX_STORAGE_BYTES).is_err());
    drop(held);
    let maximum = vec![b'x' as u16; MainEnvironmentFacts::MAX_UNITS];
    let facts =
        MainEnvironmentFacts::new(maximum, lease.claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)?)?;
    assert_eq!(facts.as_utf16().len(), MainEnvironmentFacts::MAX_UNITS);
    Ok(())
}
