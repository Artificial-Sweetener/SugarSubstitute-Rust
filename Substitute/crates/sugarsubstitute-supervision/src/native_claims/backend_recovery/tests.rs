// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recovery admits witnessed predecessors and preserves them until actual replacement.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, NamespaceId, ResourceGeneration, ResourceId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn current() -> Result<Record, Box<dyn std::error::Error>> {
    let mut record = Record::backend(
        ResourceKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
        [3; 32],
    );
    record.version = 2;
    record.custodian = 4;
    Ok(record)
}

#[test]
fn only_matching_witnessed_backend_admits_recovery_under_both_exclusions() -> TestResult {
    let next = current()?;
    let mut old = next.clone();
    old.owner.epoch = CoordinatorEpoch::new([5; 16])?;
    old.custodian = 6;
    for state in
        [RecordState::Held, RecordState::LaunchPending, RecordState::Owned, RecordState::Released]
    {
        old.state = state;
        old.process = if state == RecordState::Owned { 7 } else { 0 };
        assert!(admits(&Record::decode(&old.encode())?, &next));
        let mut legacy = old.clone();
        legacy.version = 1;
        legacy.custodian = 0;
        assert_eq!(admits(&legacy, &next), state == RecordState::Released);
    }
    old.digest = [8; 32];
    assert!(!admits(&old, &next));
    assert!(!admits(&Record::coordinator(next.owner), &next));
    Ok(())
}

#[test]
fn failed_successor_preserves_abandoned_bytes_and_cannot_release_the_old_generation() -> TestResult
{
    let mut next = Some(current()?);
    let mut old = current()?;
    old.owner.epoch = CoordinatorEpoch::new([5; 16])?;
    old.state = RecordState::Owned;
    old.process = 7;
    let bytes = old.encode();
    assert!(!records::storage::prepare_backend_release(
        record_file::PublicationRetirement::Preserved(RecordRead::Present(bytes)),
        &mut next,
        Some(&bytes)
    )?);
    assert!(
        records::storage::prepare_backend_release(
            record_file::PublicationRetirement::Activated(bytes),
            &mut next,
            Some(&bytes)
        )
        .is_err()
    );
    let current = next.as_ref().ok_or("missing successor")?.encode();
    assert!(records::storage::prepare_backend_release(
        record_file::PublicationRetirement::Activated(current),
        &mut next,
        Some(&bytes)
    )?);
    assert_eq!(next.ok_or("missing released successor")?.state, RecordState::Released);
    Ok(())
}
