// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve strict fixed schema and distinguish all active generations from release.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, NamespaceId, ResourceGeneration, ResourceId,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn owner() -> Result<CoordinatorKey, ClaimError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16]).map_err(|e| ClaimError::new("fixture_epoch", e))?,
    })
}
#[test]
fn fixed_records_preserve_exact_coordinator_and_backend_generation_fields() -> TestResult {
    let coordinator = Record::coordinator(owner()?);
    let decoded = Record::decode(&coordinator.encode())?;
    assert_eq!(decoded.owner, coordinator.owner);
    assert_eq!(decoded.digest, *coordinator.owner.namespace.bytes());
    let resource = ResourceKey {
        owner: owner()?,
        id: ResourceId::new(17)?,
        generation: ResourceGeneration::new(9)?,
    };
    let mut backend = Record::backend(resource, [4; 32]);
    for state in
        [RecordState::Held, RecordState::LaunchPending, RecordState::Owned, RecordState::Released]
    {
        backend.state = state;
        backend.process = if state == RecordState::Owned { 23 } else { 0 };
        let decoded = Record::decode(&backend.encode())?;
        assert_eq!(
            (decoded.resource, decoded.generation, decoded.process, decoded.state),
            (17, 9, backend.process, state)
        );
        assert_eq!(decoded.digest, [4; 32]);
    }
    Ok(())
}
/// A coordinator record carries the coordinator process that holds it in its otherwise unused
/// identity slots, keeps it through release, and rejects half an identity or an impossible id.
#[test]
fn a_coordinator_record_names_its_holder_through_release() -> TestResult {
    let holder = crate::coordinator_entry::EntryHolder {
        process_id: 4242,
        creation_time_100ns: 133_000_000_000_000_000,
    };
    let mut record = Record::coordinator(owner()?).held_by(holder);
    let decoded = Record::decode(&record.encode())?;
    assert_eq!((decoded.holder, decoded.generation, decoded.process), (Some(holder), 0, 0));
    record.state = RecordState::Released;
    assert_eq!(Record::decode(&record.encode())?.holder, Some(holder));
    assert_eq!(Record::decode(&Record::coordinator(owner()?).encode())?.holder, None);
    let bytes = record.encode();
    for range in [68..76, 76..84] {
        let mut half = bytes;
        half[range].fill(0);
        assert!(Record::decode(&half).is_err(), "half an identity must not decode");
    }
    let mut oversized = bytes;
    oversized[80] = 1;
    assert!(Record::decode(&oversized).is_err(), "a process id wider than 32 bits is foreign");
    Ok(())
}
#[test]
fn schema_unknown_fields_versions_states_and_invalid_epoch_fail_closed() -> TestResult {
    let bytes = Record::coordinator(owner()?).encode();
    for (offset, value) in [(0, 0), (8, 2), (10, 2), (11, 0), (11, 5), (116, 1), (127, 1)] {
        let mut changed = bytes;
        changed[offset] = value;
        assert!(Record::decode(&changed).is_err());
    }
    let mut zero_epoch = bytes;
    zero_epoch[44..60].fill(0);
    assert!(Record::decode(&zero_epoch).is_err());
    Ok(())
}
#[test]
fn record_kind_process_generation_and_namespace_cannot_be_reinterpreted() -> TestResult {
    let bytes = Record::coordinator(owner()?).encode();
    for (offset, value) in [(10, 1), (11, 2), (11, 3), (60, 1), (68, 1), (76, 1), (84, 9)] {
        let mut changed = bytes;
        changed[offset] = value;
        assert!(Record::decode(&changed).is_err());
    }
    let backend = Record::backend(
        ResourceKey {
            owner: owner()?,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
        [3; 32],
    )
    .encode();
    let mut wrong = backend;
    wrong[11] = 3;
    assert!(Record::decode(&wrong).is_err());
    wrong = backend;
    wrong[76] = 1;
    assert!(Record::decode(&wrong).is_err());
    Ok(())
}

#[test]
fn custody_version_requires_original_positive_session_and_preserves_exact_legacy_bytes()
-> TestResult {
    let resource = ResourceKey {
        owner: owner()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let legacy = Record::backend(resource, [3; 32]);
    let original = legacy.encode();
    let mut current = legacy.clone();
    current.version = 2;
    current.custodian = 19;
    let decoded = Record::decode(&current.encode())?;
    assert_eq!((decoded.version, decoded.custodian), (2, 19));
    assert_eq!(Record::decode(&original)?.encode(), original);
    current.custodian = 0;
    assert!(Record::decode(&current.encode()).is_err());
    current.custodian = 19;
    current.version = 1;
    assert!(Record::decode(&current.encode()).is_err());
    current.version = 2;
    let mut corrupt = current.encode();
    corrupt[124] = 1;
    assert!(Record::decode(&corrupt).is_err());
    Ok(())
}
