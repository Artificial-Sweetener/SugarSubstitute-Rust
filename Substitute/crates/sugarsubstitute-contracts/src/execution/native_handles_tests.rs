// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify fixed accounting and retained identity without faking native retirement.

use super::*;
use std::sync::atomic::AtomicUsize;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[derive(Default)]
struct Boundary {
    releases: AtomicUsize,
    changes: AtomicUsize,
}
struct Release(Arc<Boundary>);
impl NativeHandleRelease for Release {
    fn release_symbol_reservation(&self) {
        self.0.releases.fetch_add(1, Ordering::AcqRel);
    }
    fn output_custody_changed(&self) {
        self.0.changes.fetch_add(1, Ordering::AcqRel);
    }
}
fn lease(boundary: &Arc<Boundary>) -> Result<NativeHandleLease, super::super::IdentityError> {
    Ok(NativeHandleLease::new(
        ExecutionId::new(1)?,
        ScopeId::new(2)?,
        JobId::new(3)?,
        Box::new(Release(boundary.clone())),
    ))
}

#[test]
fn named_shares_bound_every_external_output_and_reuse_only_returned_credit() -> TestResult {
    let boundary = Arc::new(Boundary::default());
    let lease = lease(&boundary)?;
    let observed = lease.observation();
    let mut held = Vec::new();
    let mut effects = 0;
    for (partition, expected) in [
        (SymbolNativePartition::CommittedReader, 140),
        (SymbolNativePartition::SelectedRelease, 96),
        (SymbolNativePartition::Provider, 4),
        (SymbolNativePartition::Transport, 20),
    ] {
        let claim = lease.take_partition(partition)?;
        assert_eq!(claim.partition(), partition);
        assert_eq!(claim.capacity(), expected);
        for _ in 0..expected {
            let output = claim.reserve_output()?;
            effects += 1;
            held.push(output);
        }
        assert_eq!(claim.in_custody(), expected);
        assert_eq!(
            claim.reserve_output().err(),
            Some(NativeResourceError::PartitionCapacity { partition, capacity: expected })
        );
        assert_eq!(
            lease.take_partition(partition).err(),
            Some(NativeResourceError::PartitionAlreadyIssued { partition })
        );
        assert_eq!(claim.in_custody(), expected);
    }
    assert_eq!(effects, 260);
    assert_eq!(observed.snapshot(), (260, 260));
    drop(held);
    assert_eq!(observed.snapshot(), (0, 260));
    assert_eq!(boundary.releases.load(Ordering::Acquire), 0);
    drop(lease);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    assert_eq!(observed.snapshot(), (0, 0));
    Ok(())
}

#[test]
fn clone_and_same_number_reconstruction_cannot_duplicate_or_alias_original_ledger() -> TestResult {
    let boundary = Arc::new(Boundary::default());
    let original = lease(&boundary)?;
    let copy = original.clone();
    let other = lease(&boundary)?;
    let share = original.take_partition(SymbolNativePartition::Transport)?;
    let observation = original.observation();
    assert_eq!(original.owner_id(), other.owner_id());
    assert_eq!(original.original_scope_id(), other.original_scope_id());
    assert_eq!(original.original_job_id(), other.original_job_id());
    assert!(share.matches_lease(&copy));
    assert!(!share.matches_lease(&other));
    assert!(observation.matches_lease(&copy));
    assert!(!observation.matches_lease(&other));
    assert!(copy.take_partition(SymbolNativePartition::Transport).is_err());
    drop(other);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    drop(original);
    drop(copy);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    drop(share);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 2);
    Ok(())
}

#[test]
fn final_output_retains_reservation_after_all_lease_and_partition_owners_drop() -> TestResult {
    let boundary = Arc::new(Boundary::default());
    let lease = lease(&boundary)?;
    let observation = lease.observation();
    let share = lease.take_partition(SymbolNativePartition::Provider)?;
    let output = share.reserve_output()?;
    drop(share);
    drop(lease);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 0);
    assert_eq!(observation.snapshot(), (1, 1));
    drop(output);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    assert_eq!(observation.snapshot(), (0, 0));
    Ok(())
}
