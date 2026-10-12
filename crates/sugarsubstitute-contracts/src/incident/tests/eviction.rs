// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate cumulative ledger consistency and transactional checked arithmetic.
use super::*;

fn eviction(
    kind: SummaryEvictionKind,
    at: i64,
) -> Result<SummaryEviction, Box<dyn std::error::Error>> {
    Ok(SummaryEviction::new(desktop()?, IncidentUtc::from_unix_seconds(at)?, kind))
}

#[test]
fn cumulative_ledger_requires_last_fact_and_positive_matching_category()
-> Result<(), Box<dyn std::error::Error>> {
    let namespace = owner()?.namespace;
    assert_eq!(SummaryEvictions::new(namespace, 0, 0, 0, None)?.last(), None);
    for kind in [
        SummaryEvictionKind::Pending,
        SummaryEvictionKind::Acknowledged,
        SummaryEvictionKind::Quarantined,
    ] {
        let last = eviction(kind, 200)?;
        assert_eq!(
            SummaryEvictions::new(namespace, 0, 0, 0, Some(last)),
            Err(IncidentError::MismatchedFacts)
        );
        for counts in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
            assert_eq!(
                SummaryEvictions::new(namespace, counts.0, counts.1, counts.2, None),
                Err(IncidentError::MismatchedFacts)
            );
            let selected_positive = match kind {
                SummaryEvictionKind::Pending => counts.0 > 0,
                SummaryEvictionKind::Acknowledged => counts.1 > 0,
                SummaryEvictionKind::Quarantined => counts.2 > 0,
            };
            let result = SummaryEvictions::new(namespace, counts.0, counts.1, counts.2, Some(last));
            assert_eq!(result.is_ok(), selected_positive);
            if !selected_positive {
                assert_eq!(result, Err(IncidentError::MismatchedFacts));
            }
        }
    }
    let last = eviction(SummaryEvictionKind::Pending, 200)?;
    let mut foreign_namespace = [1; 32];
    foreign_namespace[31] = 2;
    assert_eq!(
        SummaryEvictions::new(NamespaceId::new(foreign_namespace), 1, 0, 0, Some(last)),
        Err(IncidentError::MismatchedFacts)
    );
    Ok(())
}

#[test]
fn recording_each_category_retains_full_identity_time_and_other_counts()
-> Result<(), Box<dyn std::error::Error>> {
    let namespace = owner()?.namespace;
    let original = SummaryEvictions::new(
        namespace,
        11,
        13,
        17,
        Some(eviction(SummaryEvictionKind::Pending, 200)?),
    )?;
    let snapshot = original;
    let changed_key = IncidentKey::new(IncidentSubject::Desktop(RunKey {
        owner: CoordinatorKey { namespace, epoch: CoordinatorEpoch::new([9; 16])? },
        instance: InstanceId::new(23)?,
        generation: RunGeneration::new(29)?,
    }));
    for (kind, expected) in [
        (SummaryEvictionKind::Pending, (12, 13, 17)),
        (SummaryEvictionKind::Acknowledged, (11, 14, 17)),
        (SummaryEvictionKind::Quarantined, (11, 13, 18)),
    ] {
        let last = SummaryEviction::new(changed_key, IncidentUtc::from_unix_seconds(201)?, kind);
        let updated = original.record(last)?;
        assert_eq!(updated.namespace(), namespace);
        assert_eq!((updated.pending(), updated.acknowledged(), updated.quarantined()), expected);
        assert_eq!(updated.last(), Some(last));
        assert_eq!(last.incident(), changed_key);
        assert_eq!(last.at().unix_seconds(), 201);
        assert_eq!(last.kind(), kind);
        assert_eq!(original, snapshot);
    }
    let supervisor = SummaryEviction::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner()?)),
        IncidentUtc::from_unix_seconds(200)?,
        SummaryEvictionKind::Acknowledged,
    );
    assert_eq!(original.record(supervisor)?.last(), Some(supervisor));
    let empty = SummaryEvictions::new(namespace, 0, 0, 0, None)?;
    let first = eviction(SummaryEvictionKind::Quarantined, 0)?;
    assert_eq!(empty.record(first)?, SummaryEvictions::new(namespace, 0, 0, 1, Some(first))?);
    Ok(())
}

#[test]
fn rejected_namespace_time_and_each_counter_overflow_leave_original_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    let namespace = owner()?.namespace;
    let original = SummaryEvictions::new(
        namespace,
        1,
        2,
        3,
        Some(eviction(SummaryEvictionKind::Pending, 200)?),
    )?;
    let snapshot = original;
    let foreign = IncidentKey::new(IncidentSubject::Supervisor(CoordinatorKey {
        namespace: NamespaceId::new([2; 32]),
        epoch: owner()?.epoch,
    }));
    assert_eq!(
        original.record(SummaryEviction::new(
            foreign,
            IncidentUtc::from_unix_seconds(200)?,
            SummaryEvictionKind::Pending
        )),
        Err(IncidentError::MismatchedFacts)
    );
    assert_eq!(original, snapshot);
    assert_eq!(
        original.record(eviction(SummaryEvictionKind::Acknowledged, 199)?),
        Err(IncidentError::InvalidTime)
    );
    assert_eq!(original, snapshot);
    for kind in [
        SummaryEvictionKind::Pending,
        SummaryEvictionKind::Acknowledged,
        SummaryEvictionKind::Quarantined,
    ] {
        let full = SummaryEvictions::new(
            namespace,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Some(eviction(kind, 200)?),
        )?;
        let snapshot = full;
        assert_eq!(full.record(eviction(kind, 201)?), Err(IncidentError::EvictionCountExhausted));
        assert_eq!(full, snapshot);
        assert_eq!(full.last(), snapshot.last());
    }
    Ok(())
}
